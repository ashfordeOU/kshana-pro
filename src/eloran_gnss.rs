// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Software GNSS bridge for hybrid eLoran/GNSS scenes
//!
//! Generates the GNSS half of a hybrid scene **in software**, with the open engine's GNSS IQ
//! layer (`kshana::iq::scene`), on the **same receiver trajectory and the same time base** as
//! an eLoran scenario (`kshana_eloran::Scenario`):
//!
//! * the trajectory is the eLoran scenario's `[receiver]` (static or waypoints), converted to
//!   ECEF with the open engine's exact WGS-84 conversion;
//! * the scene starts at the eLoran scenario's `start_utc`, converted to GPS week and time of
//!   week through TAI (GPST = TAI − 19 s), so sample 0 of both recordings is the same instant;
//! * a GPS-time steering residual in the eLoran scenario (`gnss_time_residual_ns.Gpst`) shifts
//!   every satellite signal, which is the same as a receiver clock offset of the opposite sign;
//! * satellites come from a RINEX 3 broadcast navigation file: GPS L1 C/A (with LNAV built from
//!   the ephemeris) and Galileo E1-B/C (seeded data symbols; MODELLED as a non-decodable
//!   message) at their broadcast positions; the elevation mask and the elevation-based C/N0
//!   model are the open engine's.
//!
//! Output: complex baseband centred on 1575.42 MHz (GPS L1 and Galileo E1 share the carrier)
//! as `ci16_le` SigMF, and the open engine's per-satellite truth CSV. The eLoran recording
//! (real LF samples) and this one (complex L-band) are separate files with a common start
//! instant; a lab feeds the GNSS file to an SDR transmitter or a software receiver.

use std::io::BufWriter;
use std::path::{Path, PathBuf};

use kshana::frames::{geodetic_to_ecef, Geodetic};
use kshana::gps_lnav::LnavConventions;
use kshana::iq::io::format::SampleFormat;
use kshana::iq::io::IqWriter;
use kshana::iq::scene::{
    CsvTruthWriter, NavData, ReceiverClock, SatGeometry, Scene, SceneConfig, SceneSatellite,
    Trajectory as GTraj,
};
use kshana::iq::SampleSpec;
use kshana::rinex::{parse_nav, RinexEphemeris};
use kshana_eloran::time::{days_from_civil, AbsTime, PS_PER_S};
use kshana_eloran::timebase::GnssTime;
use kshana_eloran::traj::Trajectory;
use kshana_eloran::Scenario;
use serde::{Deserialize, Serialize};

/// GPS L1 / Galileo E1 carrier (Hz).
pub const L1_HZ: f64 = 1_575_420_000.0;
const WEEK_S: f64 = 604_800.0;

/// Options of the bridge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssOptions {
    /// Complex sample rate (Hz); 4 MHz covers the main lobes of L1 C/A and E1 BOC(1,1).
    pub fs_hz: f64,
    /// Include GPS L1 C/A.
    pub gps: bool,
    /// Include Galileo E1-B.
    pub galileo: bool,
    /// Elevation mask (deg).
    pub elevation_mask_deg: f64,
    /// Optional fixed C/N0 for every satellite (dB-Hz); `None` uses the elevation model.
    pub cn0_dbhz: Option<f64>,
    /// Optional duration override (s); default: the eLoran scenario's duration.
    pub duration_s: Option<f64>,
    /// Synthesis threads.
    pub threads: usize,
}

impl Default for GnssOptions {
    fn default() -> Self {
        GnssOptions {
            fs_hz: 4_000_000.0,
            gps: true,
            galileo: true,
            elevation_mask_deg: 5.0,
            cn0_dbhz: None,
            duration_s: None,
            threads: 1,
        }
    }
}

/// What the bridge produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssRun {
    pub gps_week: i64,
    pub start_tow_s: f64,
    pub satellites: Vec<String>,
    pub samples: u64,
    pub truth_records: u64,
    pub receiver_clock_bias_ns: f64,
    pub data: PathBuf,
    pub truth: PathBuf,
}

/// GPS week and time of week of an absolute instant (TAI since 1958).
pub fn gps_week_tow(t: AbsTime) -> (i64, f64) {
    // GPS epoch 1980-01-06T00:00:00 UTC = TAI − 19 s; TAI − UTC was 19 s then.
    let days = days_from_civil(1980, 1, 6) - days_from_civil(1958, 1, 1);
    let epoch_tai_ps = (days as i128 * 86_400 + 19) * PS_PER_S as i128;
    let gpst_ps = t.0 - epoch_tai_ps;
    let week_ps = (WEEK_S as i128) * PS_PER_S as i128;
    let week = gpst_ps.div_euclid(week_ps);
    let rem = gpst_ps.rem_euclid(week_ps);
    (week as i64, rem as f64 / PS_PER_S as f64)
}

fn ecef(lat_deg: f64, lon_deg: f64, h_m: f64) -> [f64; 3] {
    geodetic_to_ecef(Geodetic {
        lat_rad: lat_deg.to_radians(),
        lon_rad: lon_deg.to_radians(),
        alt_m: h_m,
    })
}

/// The eLoran trajectory as an open-engine GNSS trajectory.
pub fn to_gnss_trajectory(t: &Trajectory) -> GTraj {
    match t {
        Trajectory::Static {
            lat_deg,
            lon_deg,
            h_m,
            ..
        } => GTraj::Static(ecef(*lat_deg, *lon_deg, *h_m)),
        Trajectory::Waypoints { points } => GTraj::Waypoints(
            points
                .iter()
                .map(|p| (p.t_s, ecef(p.lat_deg, p.lon_deg, p.h_m)))
                .collect(),
        ),
    }
}

/// Pick, per satellite of `system`, the healthy ephemeris whose `toe` is nearest the scene
/// start (same week, ±2 h).
pub fn select_ephemerides(
    ephs: &[RinexEphemeris],
    system: char,
    week: i64,
    tow: f64,
) -> Vec<RinexEphemeris> {
    let mut best: std::collections::BTreeMap<u8, (f64, RinexEphemeris)> = Default::default();
    for e in ephs
        .iter()
        .filter(|e| e.system == system && e.sv_health == 0.0)
    {
        // Galileo RINEX weeks are GST weeks aligned with GPS weeks (+1024 in some files):
        // compare by time of week only when the week fields disagree by a whole cycle.
        let dweek = (e.gps_week as i64 - week).rem_euclid(1024);
        if dweek != 0 && dweek != 1023 && dweek != 1 {
            continue;
        }
        let dt = (e.toe - tow).abs().min(WEEK_S - (e.toe - tow).abs());
        if dt > 7200.0 {
            continue;
        }
        match best.get(&e.prn) {
            Some((d, _)) if *d <= dt => {}
            _ => {
                best.insert(e.prn, (dt, *e));
            }
        }
    }
    best.into_values().map(|(_, e)| e).collect()
}

/// Generate the GNSS recording of `sc` from the navigation file `nav_text` into `<base>.gnss.*`.
pub fn generate(
    sc: &Scenario,
    nav_text: &str,
    opts: &GnssOptions,
    base: &Path,
) -> Result<GnssRun, String> {
    let start = AbsTime::from_utc_iso(&sc.scene.start_utc)?;
    let (week, tow) = gps_week_tow(start);
    let duration = opts.duration_s.unwrap_or(sc.scene.params.duration_s);
    let ephs = parse_nav(nav_text)?;
    let residual_s = sc
        .gnss_time_residual_ns
        .get(&GnssTime::Gpst)
        .copied()
        .unwrap_or(0.0)
        * 1e-9;
    let spec = SampleSpec {
        fs_hz: opts.fs_hz,
        center_hz: L1_HZ,
        if_hz: 0.0,
    };
    let mut cfg = SceneConfig::new(spec, duration);
    cfg.start_tow_s = tow;
    cfg.receiver = to_gnss_trajectory(&sc.receiver);
    cfg.clock = ReceiverClock {
        bias_s: -residual_s,
        drift_s_per_s: 0.0,
    };
    cfg.seed = sc.scene.params.seed;
    cfg.elevation_mask_deg = opts.elevation_mask_deg;
    cfg.truth_interval_s = 0.1;
    cfg.threads = opts.threads.max(1);
    let mut scene = Scene::new(cfg).map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    if opts.gps {
        for e in select_ephemerides(&ephs, 'G', week, tow) {
            let s =
                SceneSatellite::gps_l1ca_broadcast(&e, opts.cn0_dbhz, LnavConventions::default())
                    .map_err(|x| x.to_string())?;
            names.push(format!("G{:02} L1C/A", e.prn));
            scene.add_satellite(s);
        }
    }
    if opts.galileo {
        for e in select_ephemerides(&ephs, 'E', week, tow) {
            let code =
                kshana::iq::signals::galileo::e1b(e.prn as u16).map_err(|x| x.to_string())?;
            names.push(format!("E{:02} E1-B", e.prn));
            scene.add_satellite(SceneSatellite {
                id: 1000 + e.prn as u32,
                code: Box::new(code),
                geometry: SatGeometry::Broadcast(Box::new(e)),
                cn0_dbhz: opts.cn0_dbhz,
                nav: NavData::Seeded {
                    seed: sc.scene.params.seed ^ e.prn as u64,
                },
            });
        }
    }
    if names.is_empty() {
        return Err(format!(
            "no usable ephemeris for GPS week {week} TOW {tow:.0} s (±2 h) in the navigation file"
        ));
    }
    let data = with_suffix(base, "gnss.sigmf-data");
    let truth = with_suffix(base, "gnss.truth.csv");
    let f = std::fs::File::create(&data).map_err(|e| format!("{}: {e}", data.display()))?;
    let mut sink = IqWriter::new(BufWriter::new(f), SampleFormat::CI16_LE).with_scale(2000.0);
    let tf = std::fs::File::create(&truth).map_err(|e| format!("{}: {e}", truth.display()))?;
    let mut tsink = CsvTruthWriter::new(BufWriter::new(tf));
    let summary = scene
        .generate(&mut sink, &mut tsink)
        .map_err(|e| e.to_string())?;
    let meta = serde_json::json!({
        "global": {
            "core:datatype": "ci16_le",
            "core:sample_rate": opts.fs_hz,
            "core:version": "1.0.0",
            "core:description": "Kshana Pro hybrid scene: GNSS half (GPS L1 C/A, Galileo E1-B) on the eLoran scenario's trajectory and time base",
            "kshana:eloran_scenario_sha256": sc.hash(),
            "kshana:gps_week": week,
            "kshana:start_tow_s": tow,
            "kshana:satellites": names,
        },
        "captures": [{ "core:sample_start": 0, "core:frequency": L1_HZ }],
        "annotations": []
    });
    let meta_path = with_suffix(base, "gnss.sigmf-meta");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap())
        .map_err(|e| format!("{}: {e}", meta_path.display()))?;
    Ok(GnssRun {
        gps_week: week,
        start_tow_s: tow,
        satellites: names,
        samples: summary.samples,
        truth_records: summary.truth_records,
        receiver_clock_bias_ns: -residual_s * 1e9,
        data,
        truth,
    })
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut s = base.as_os_str().to_os_string();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gps_week_and_tow_of_a_known_instant() {
        // 2024-09-10T00:00:00 UTC: GPS week 2331, TOW 2 days + 18 leap seconds = 172 818 s.
        let t = AbsTime::from_utc_iso("2024-09-10T00:00:00Z").unwrap();
        let (w, tow) = gps_week_tow(t);
        assert_eq!(w, 2331);
        assert!((tow - 172_818.0).abs() < 1e-9, "{tow}");
    }
}
