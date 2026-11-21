// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Traceability through GNSS time transfer
//!
//! The eLoran timing chain is traceable to UTC through a laboratory's local reference (the
//! clock that also steers or monitors the eLoran station). This module measures that local
//! reference against GPS time with a geodetic receiver, forms track averages, differences two
//! sites in common view and expresses the reference against UTC(k) with an uncertainty.
//!
//! ## 1. Receiver clock against GPS time (position fixed)
//!
//! Input: a RINEX 3 observation file of the receiver driven by the local reference, the
//! broadcast navigation file and the **known antenna position** (ECEF). Each epoch's
//! measurements are assembled by the open engine ([`kshana::pvt::assemble_epoch`]):
//! satellite position at transmission (Sagnac rotated), broadcast satellite clock with the
//! relativistic term, Saastamoinen–Niell troposphere, and either the **ionosphere-free**
//! L1/L2 code combination (dual frequency) or the broadcast Klobuchar ionosphere with `−TGD`
//! (single frequency). With the position held, each satellite gives
//!
//! ```text
//! REFSYS_i = (P_i − |r_sat − r_ant| + c·δt_sv − I − T)/c − D_rx       (reference − GPS time)
//! REFSV_i  = REFSYS_i − δt_sv                                          (reference − satellite)
//! ```
//!
//! with `D_rx` the receiver's total delay (internal + cable − reference cable). Per epoch the
//! satellites are combined with `sin²(elevation)` weights after rejecting those more than
//! `outlier_m` from the median. A receiver's clock offset (up to milliseconds) does not need
//! iteration for a static antenna: the transmit time comes from the tag minus the pseudorange,
//! which already holds the clock. The epoch's GPS time is the tag minus the estimate.
//!
//! ## 2. CGGTTS-like 13-minute tracks (simplified vs CGGTTS V2E)
//!
//! Tracks follow the BIPM common-view schedule: 780 s tracks every 16 min, the first at
//! 00:02 UTC on MJD 50722 and 4 min earlier each following day (one sidereal-day repeat), so
//! two laboratories observe the same satellite over the same interval. Per satellite and
//! track, a straight line is fitted to the epoch values and evaluated at the track midpoint
//! (`REFSV`, `REFSYS`, slopes `SRSV`, `SRSYS`, `DSG` = RMS of the fit residuals, `ELV`,
//! `AZTH`, `MDTR` and `MDIO` model delays at the midpoint).
//!
//! **Differences from the CGGTTS V2E specification** (Defraigne & Petit, BIPM, 2015):
//! V2E fits a quadratic to 30 s code measurements smoothed over 15 s blocks, then a line to
//! those values; here the line goes straight through the epoch values at the file's rate
//! (any rate; at least half the expected points are required). `MSIO`/`SMSI`/`ISG` (measured
//! ionosphere) are written as missing (all 9s); `IOE` is the issue of ephemeris used only
//! nominally (written as 0); the header is a reduced set (`LAB`, `X/Y/Z`, `FRAME`, delays,
//! `REF`) and its `CKSUM` is the byte sum modulo 256 of every header character before the
//! checksum value (newlines excluded), which is this module's own rule; `FRC` is `L3P` for
//! the ionosphere-free combination and `L1C` for C/A single frequency. Data lines carry the
//! V2E `CK` checksum, so the open engine's reader ([`kshana::cggtts::read_tracks`]) accepts
//! them; common view and all-in-view use the engine's [`kshana::cggtts::common_view`] and
//! [`kshana::cggtts::all_in_view`].
//!
//! ## 3. Against UTC(k)
//!
//! `REF − UTC(k) = (REF − GPS time) − (UTC(k) − GPS time)`. The second term comes from BIPM
//! Circular T (section 1 gives `UTC − UTC(k)`; the `UTC − GPS time` values give
//! `UTC(k) − GPS time = (UTC − GPS time) − (UTC − UTC(k))`), or from the GPS broadcast UTC
//! parameters (`UTC(USNO) − GPS time = −(Δt_LS + A0 + A1·(t − t_ot))`, see IS-GPS-200
//! 20.3.3.5.2.4). [`UncertaintyBudget`] combines the statistical part with the receiver
//! calibration, model and offset-input contributions (root-sum-square, k = 1).
//!
//! MODELLED: the measurement path is the open engine's (validated there against a surveyed
//! IGS station); the track and common-view arithmetic are exact; the budget values are the
//! user's. Not a substitute for a BIPM-calibrated CGGTTS chain.

use kshana::cggtts::{all_in_view, common_view as engine_common_view, Epoch, Track};
use kshana::frames::{ecef_to_geodetic, look_angles};
use kshana::gnss_sim::{klobuchar_delay_m, tropo_delay_m, KlobucharCoeffs, Meteo, C_M_PER_S};
use kshana::pvt::{assemble_epoch, klobuchar_from_nav_header, sagnac_rotate, AtmosModel};
use kshana::rinex::{parse_nav, RinexEphemeris};
use kshana::rinex_obs::{parse_obs, RinexObs};
use kshana_eloran::time::{civil_from_days, days_from_civil};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

const L1_HZ: f64 = 1_575_420_000.0;
const L2_HZ: f64 = 1_227_600_000.0;
/// MJD of the GPS epoch 1980-01-06.
pub const MJD_GPS_EPOCH: i64 = 44_244;
/// Track length (s) of the BIPM schedule.
pub const TRACK_S: u32 = 780;

/// Settings of the clock estimation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimexferConfig {
    /// Elevation mask (deg).
    pub mask_deg: f64,
    /// Use the ionosphere-free L1/L2 combination when both codes are present.
    pub dual_freq: bool,
    /// Reject satellites further than this from the epoch median (m).
    pub outlier_m: f64,
    /// Receiver total delay: internal + antenna cable − reference cable (ns).
    pub receiver_delay_ns: f64,
    /// GPS time − UTC (leap seconds) for the track schedule and MJD (s). The default, 18, holds
    /// from 2017-01-01; it is not taken from the date or the RINEX `LEAP SECONDS` header, so
    /// set it for data before 2017 (17 from 2015-07-01, 16 from 2012-07-01) or after a future
    /// leap second, or every track is that many seconds off.
    pub leap_seconds: f64,
    /// Fewest satellites for an epoch.
    pub min_sats: usize,
}

impl Default for TimexferConfig {
    fn default() -> Self {
        TimexferConfig {
            mask_deg: 10.0,
            dual_freq: true,
            outlier_m: 30.0,
            receiver_delay_ns: 0.0,
            leap_seconds: 18.0,
            min_sats: 4,
        }
    }
}

/// One satellite's clock estimate at one epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SatClock {
    pub sat: String,
    pub el_deg: f64,
    pub az_deg: f64,
    /// Reference − GPS time (s).
    pub refsys_s: f64,
    /// Reference − satellite clock (s).
    pub refsv_s: f64,
    /// Model troposphere and ionosphere delays (m).
    pub tropo_m: f64,
    pub iono_m: f64,
    pub iono_free: bool,
    pub used: bool,
}

/// One epoch's reference − GPS time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClockEpoch {
    /// GPS time of the measurement, seconds from 1980-01-06 (tag minus the estimate).
    pub t_gps_s: f64,
    /// Weighted mean reference − GPS time (s).
    pub refsys_s: f64,
    /// Formal one-sigma from the satellite scatter (s).
    pub sigma_s: f64,
    pub n_used: usize,
    pub sats: Vec<SatClock>,
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Reference − GPS time per epoch of `obs` at the known antenna position (ECEF).
pub fn clock_offsets(
    obs: &RinexObs,
    ephs: &[RinexEphemeris],
    iono: Option<KlobucharCoeffs>,
    antenna_ecef: [f64; 3],
    cfg: &TimexferConfig,
) -> Vec<ClockEpoch> {
    let atmos = AtmosModel {
        iono: iono.unwrap_or_default(),
        meteo: Meteo::default(),
    };
    let station = ecef_to_geodetic(antenna_ecef);
    let mut out = Vec::new();
    for (i, ep) in obs.epochs.iter().enumerate() {
        if ep.flag > 1 {
            continue;
        }
        let meas = assemble_epoch(
            obs,
            i,
            ephs,
            antenna_ecef,
            &atmos,
            cfg.mask_deg,
            cfg.dual_freq,
        );
        let mut sats: Vec<SatClock> = meas
            .iter()
            .map(|(sat, m)| {
                let d = [
                    m.sat_ecef[0] - antenna_ecef[0],
                    m.sat_ecef[1] - antenna_ecef[1],
                    m.sat_ecef[2] - antenna_ecef[2],
                ];
                let geo = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let refsys = (m.pseudorange_m - geo + m.sat_clock_m - m.iono_m - m.tropo_m)
                    / C_M_PER_S
                    - cfg.receiver_delay_ns * 1e-9;
                let look = look_angles(station, m.sat_ecef);
                SatClock {
                    sat: sat.clone(),
                    el_deg: look.el_rad.to_degrees(),
                    az_deg: look.az_rad.to_degrees().rem_euclid(360.0),
                    refsys_s: refsys,
                    refsv_s: refsys - m.sat_clock_m / C_M_PER_S,
                    tropo_m: m.tropo_m,
                    iono_m: m.iono_m,
                    iono_free: cfg.dual_freq && m.iono_m == 0.0,
                    used: false,
                }
            })
            .collect();
        if sats.len() < cfg.min_sats {
            continue;
        }
        let med = median(sats.iter().map(|s| s.refsys_s).collect());
        let (mut sw, mut swx) = (0.0, 0.0);
        for s in &mut sats {
            if ((s.refsys_s - med) * C_M_PER_S).abs() <= cfg.outlier_m {
                s.used = true;
                let w = s.el_deg.to_radians().sin().powi(2).max(1e-3);
                sw += w;
                swx += w * s.refsys_s;
            }
        }
        let used: Vec<&SatClock> = sats.iter().filter(|s| s.used).collect();
        if used.len() < cfg.min_sats {
            continue;
        }
        let mean = swx / sw;
        let n = used.len() as f64;
        let var = used
            .iter()
            .map(|s| (s.refsys_s - mean).powi(2))
            .sum::<f64>()
            / (n - 1.0);
        let n_used = used.len();
        out.push(ClockEpoch {
            t_gps_s: ep.time.seconds_from_gps_epoch() - mean,
            refsys_s: mean,
            sigma_s: (var / n).sqrt(),
            n_used,
            sats,
        });
    }
    out
}

/// Parse the files and run [`clock_offsets`] (Klobuchar from the navigation header).
pub fn clock_offsets_from_files(
    obs_text: &str,
    nav_text: &str,
    antenna_ecef: [f64; 3],
    cfg: &TimexferConfig,
) -> Result<Vec<ClockEpoch>, String> {
    let obs = parse_obs(obs_text)?;
    let ephs = parse_nav(nav_text)?;
    Ok(clock_offsets(
        &obs,
        &ephs,
        klobuchar_from_nav_header(nav_text),
        antenna_ecef,
        cfg,
    ))
}

// ------------------------------------------------------------------ tracks

/// One CGGTTS-like track (values in SI units; see the module docs for the simplifications).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CggttsTrack {
    pub sat: String,
    /// MJD of the track start (UTC).
    pub mjd: i64,
    /// Start time of day (s, UTC).
    pub start_s: u32,
    pub trkl_s: u32,
    pub elv_deg: f64,
    pub azth_deg: f64,
    /// Reference − satellite clock at the midpoint (s), and its slope (s/s).
    pub refsv_s: f64,
    pub srsv: f64,
    /// Reference − GPS time at the midpoint (s), and its slope (s/s).
    pub refsys_s: f64,
    pub srsys: f64,
    /// RMS of the REFSYS fit residuals (s).
    pub dsg_s: f64,
    /// Model troposphere and ionosphere delays at the midpoint (s).
    pub mdtr_s: f64,
    pub mdio_s: f64,
    /// Points in the fit.
    pub n: usize,
    /// `L3P` (ionosphere-free) or `L1C`.
    pub frc: String,
}

/// Start times (s of the UTC day) of the BIPM schedule's tracks on `mjd`.
pub fn track_starts(mjd: i64) -> Vec<u32> {
    let first_min = (2 - 4 * (mjd - 50_722)).rem_euclid(16) as u32;
    (0..)
        .map(|k| (first_min + 16 * k) * 60)
        .take_while(|s| s + TRACK_S <= 86_400)
        .collect()
}

fn line_fit(pts: &[(f64, f64)], t0: f64) -> (f64, f64, f64) {
    let n = pts.len() as f64;
    let tb = pts.iter().map(|p| p.0 - t0).sum::<f64>() / n;
    let yb = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pts.iter().map(|p| (p.0 - t0 - tb).powi(2)).sum();
    let sxy: f64 = pts.iter().map(|p| (p.0 - t0 - tb) * (p.1 - yb)).sum();
    let b = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    let a = yb - b * tb; // value at t0
    let rms = (pts
        .iter()
        .map(|p| (p.1 - (a + b * (p.0 - t0))).powi(2))
        .sum::<f64>()
        / n)
        .sqrt();
    (a, b, rms)
}

/// Track averages of per-epoch satellite clocks on the BIPM schedule. `interval_s` is the
/// observation interval (a track needs at least half of `780/interval` points).
#[allow(clippy::type_complexity)]
pub fn tracks(epochs: &[ClockEpoch], cfg: &TimexferConfig, interval_s: f64) -> Vec<CggttsTrack> {
    use std::collections::BTreeMap;
    // (mjd, start, sat) -> points (t_utc_of_day, sat clock)
    let mut bins: BTreeMap<(i64, u32, String), Vec<(f64, &SatClock)>> = BTreeMap::new();
    for ep in epochs {
        let t_utc = ep.t_gps_s - cfg.leap_seconds;
        let mjd = MJD_GPS_EPOCH + (t_utc / 86_400.0).floor() as i64;
        let sod = t_utc - (mjd - MJD_GPS_EPOCH) as f64 * 86_400.0;
        let Some(start) = track_starts(mjd)
            .into_iter()
            .find(|&s| sod >= s as f64 && sod < (s + TRACK_S) as f64)
        else {
            continue;
        };
        for s in ep.sats.iter().filter(|s| s.used) {
            bins.entry((mjd, start, s.sat.clone()))
                .or_default()
                .push((sod, s));
        }
    }
    let need = ((TRACK_S as f64 / interval_s) / 2.0).ceil().max(2.0) as usize;
    let mut out = Vec::new();
    for ((mjd, start, sat), pts) in bins {
        if pts.len() < need {
            continue;
        }
        let mid = start as f64 + TRACK_S as f64 / 2.0;
        let (refsys, srsys, dsg) = line_fit(
            &pts.iter().map(|p| (p.0, p.1.refsys_s)).collect::<Vec<_>>(),
            mid,
        );
        let (refsv, srsv, _) = line_fit(
            &pts.iter().map(|p| (p.0, p.1.refsv_s)).collect::<Vec<_>>(),
            mid,
        );
        let (elv, _, _) = line_fit(
            &pts.iter().map(|p| (p.0, p.1.el_deg)).collect::<Vec<_>>(),
            mid,
        );
        let (mdtr, _, _) = line_fit(
            &pts.iter().map(|p| (p.0, p.1.tropo_m)).collect::<Vec<_>>(),
            mid,
        );
        let (mdio, _, _) = line_fit(
            &pts.iter().map(|p| (p.0, p.1.iono_m)).collect::<Vec<_>>(),
            mid,
        );
        // azimuth: circular mean
        let (sx, cx) = pts.iter().fold((0.0, 0.0), |a, p| {
            let r = p.1.az_deg.to_radians();
            (a.0 + r.sin(), a.1 + r.cos())
        });
        let iono_free = pts.iter().all(|p| p.1.iono_free);
        out.push(CggttsTrack {
            sat,
            mjd,
            start_s: start,
            trkl_s: TRACK_S,
            elv_deg: elv,
            azth_deg: sx.atan2(cx).to_degrees().rem_euclid(360.0),
            refsv_s: refsv,
            srsv,
            refsys_s: refsys,
            srsys,
            dsg_s: dsg,
            mdtr_s: mdtr / C_M_PER_S,
            mdio_s: mdio / C_M_PER_S,
            n: pts.len(),
            frc: if iono_free {
                "L3P".into()
            } else {
                "L1C".into()
            },
        });
    }
    out.sort_by(|a, b| (a.mjd, a.start_s, &a.sat).cmp(&(b.mjd, b.start_s, &b.sat)));
    out
}

/// The open engine's track type (for its common-view and all-in-view functions).
pub fn to_engine_tracks(t: &[CggttsTrack]) -> Vec<Track> {
    t.iter()
        .map(|x| Track {
            sat: x.sat.clone(),
            mjd: x.mjd,
            start_s: x.start_s,
            trkl_s: x.trkl_s,
            t: (x.mjd - MJD_GPS_EPOCH) as f64 * 86_400.0 + x.start_s as f64 + x.trkl_s as f64 / 2.0,
            elv_deg: (x.elv_deg * 10.0).round() / 10.0,
            refsv_s: Some(x.refsv_s),
            refsys_s: Some(x.refsys_s),
        })
        .collect()
}

/// Laboratory description for the file header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CggttsHeader {
    pub lab: String,
    pub receiver: String,
    pub antenna_ecef: [f64; 3],
    pub int_dly_ns: f64,
    pub cab_dly_ns: f64,
    pub ref_dly_ns: f64,
    pub reference: String,
}

fn tenths(v_s: f64) -> i64 {
    (v_s * 1e10).round() as i64
}

/// Format tracks as a CGGTTS-like text file (simplified header; V2E data lines with `CK`).
pub fn write_cggtts(h: &CggttsHeader, tracks: &[CggttsTrack]) -> String {
    let mut head = String::new();
    head.push_str("CGGTTS     GENERIC DATA FORMAT VERSION = 2E\n");
    head.push_str("REV DATE = 2026-11-18\n");
    head.push_str(&format!("RCVR = {}\n", h.receiver));
    head.push_str("CH = 99\n");
    head.push_str("IMS = 99999\n");
    head.push_str(&format!("LAB = {}\n", h.lab));
    head.push_str(&format!("X = {:+.2} m\n", h.antenna_ecef[0]));
    head.push_str(&format!("Y = {:+.2} m\n", h.antenna_ecef[1]));
    head.push_str(&format!("Z = {:+.2} m\n", h.antenna_ecef[2]));
    head.push_str("FRAME = ITRF\n");
    head.push_str(
        "COMMENTS = SIMPLIFIED CGGTTS (kshana_pro::eloran_timexfer): linear fit of epoch values\n",
    );
    head.push_str(&format!(
        "INT DLY = {:.1} ns (GPS C1), {:.1} ns (GPS P2)     CAL_ID = NA\n",
        h.int_dly_ns, h.int_dly_ns
    ));
    head.push_str(&format!("CAB DLY = {:.1} ns\n", h.cab_dly_ns));
    head.push_str(&format!("REF DLY = {:.1} ns\n", h.ref_dly_ns));
    head.push_str(&format!("REF = {}\n", h.reference));
    head.push_str("CKSUM = ");
    let ck = head
        .bytes()
        .filter(|&b| b != b'\n')
        .fold(0u8, |a, b| a.wrapping_add(b));
    let mut out = head;
    out.push_str(&format!("{ck:02X}\n\n"));
    out.push_str("SAT CL  MJD  STTIME TRKL ELV AZTH   REFSV      SRSV     REFSYS    SRSYS  DSG IOE MDTR SMDT MDIO SMDI MSIO SMSI ISG FR HC FRC CK\n");
    out.push_str("             hhmmss  s  .1dg .1dg    .1ns     .1ps/s     .1ns    .1ps/s .1ns     .1ns.1ps/s.1ns.1ps/s.1ns.1ps/s.1ns\n");
    for t in tracks {
        let (hh, mm, ss) = (t.start_s / 3600, t.start_s / 60 % 60, t.start_s % 60);
        let body = format!(
            "{} FF {:5} {:02}{:02}{:02} {:4} {:3} {:4} {:11} {:6} {:11} {:6} {:4} {:3} {:4} {:4} {:4} {:4} {:4} {:4} {:3} {:2} {:2} {:3} ",
            t.sat,
            t.mjd,
            hh,
            mm,
            ss,
            t.trkl_s,
            (t.elv_deg * 10.0).round() as i64,
            (t.azth_deg * 10.0).round() as i64,
            tenths(t.refsv_s),
            (t.srsv * 1e13).round() as i64,
            tenths(t.refsys_s),
            (t.srsys * 1e13).round() as i64,
            tenths(t.dsg_s).min(9999),
            0,
            tenths(t.mdtr_s).min(9999),
            0,
            tenths(t.mdio_s).min(9999),
            0,
            9999,
            9999,
            999,
            0,
            0,
            t.frc
        );
        let ck = kshana::cggtts::checksum(&body);
        out.push_str(&format!("{body}{ck:02X}\n"));
    }
    out
}

/// Common view of two laboratories' tracks (engine): per track start, the median over common
/// satellites of `REFSV(a) − REFSV(b)` = `ref(a) − ref(b)`.
pub fn common_view(a: &[CggttsTrack], b: &[CggttsTrack]) -> Vec<Epoch> {
    engine_common_view(&to_engine_tracks(a), &to_engine_tracks(b))
}

/// All-in-view series (engine): per track start, the median `REFSYS` (reference − GPS time).
pub fn all_in_view_series(t: &[CggttsTrack]) -> Vec<Epoch> {
    all_in_view(&to_engine_tracks(t), None)
}

// ------------------------------------------------------------------ against UTC(k)

/// `UTC(k) − GPS time` at an MJD, with its standard uncertainty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UtckGpsOffset {
    pub mjd: f64,
    /// UTC(k) − GPS time, excluding the integer leap seconds (ns).
    pub utck_minus_gpst_ns: f64,
    pub u_ns: f64,
    /// Where the value came from.
    pub source: String,
}

impl UtckGpsOffset {
    /// From BIPM Circular T: `UTC − UTC(k)` (section 1) and `UTC − GPS time` (ns, leap seconds
    /// excluded), with the user's uncertainty of the combination.
    pub fn from_circular_t(
        mjd: f64,
        utc_minus_utck_ns: f64,
        utc_minus_gpst_ns: f64,
        u_ns: f64,
    ) -> UtckGpsOffset {
        UtckGpsOffset {
            mjd,
            utck_minus_gpst_ns: utc_minus_gpst_ns - utc_minus_utck_ns,
            u_ns,
            source: "BIPM Circular T".into(),
        }
    }

    /// From the GPS broadcast UTC parameters (IS-GPS-200 20.3.3.5.2.4) at GPS time `t_gps_s`
    /// (seconds from 1980-01-06): `UTC(USNO) − GPS time = −(A0 + A1·(t − t_ot))`, the leap
    /// seconds excluded. `t_ot_gps_s` is the reference time on the same scale.
    pub fn from_broadcast_utc(
        t_gps_s: f64,
        a0_s: f64,
        a1: f64,
        t_ot_gps_s: f64,
        u_ns: f64,
    ) -> UtckGpsOffset {
        UtckGpsOffset {
            mjd: MJD_GPS_EPOCH as f64 + t_gps_s / 86_400.0,
            utck_minus_gpst_ns: -(a0_s + a1 * (t_gps_s - t_ot_gps_s)) * 1e9,
            u_ns,
            source: "GPS broadcast UTC parameters (UTC(USNO))".into(),
        }
    }
}

/// Linear interpolation of `UTC(k) − GPS time` at `mjd` (held flat outside the table).
pub fn interpolate_offset(table: &[UtckGpsOffset], mjd: f64) -> Option<(f64, f64)> {
    let mut t = table.to_vec();
    t.sort_by(|a, b| a.mjd.total_cmp(&b.mjd));
    let first = t.first()?;
    if mjd <= first.mjd || t.len() == 1 {
        return Some((first.utck_minus_gpst_ns, first.u_ns));
    }
    let last = t.last()?;
    if mjd >= last.mjd {
        return Some((last.utck_minus_gpst_ns, last.u_ns));
    }
    let j = t.partition_point(|x| x.mjd <= mjd);
    let (a, b) = (&t[j - 1], &t[j]);
    let w = (mjd - a.mjd) / (b.mjd - a.mjd);
    Some((
        a.utck_minus_gpst_ns + w * (b.utck_minus_gpst_ns - a.utck_minus_gpst_ns),
        a.u_ns.max(b.u_ns),
    ))
}

/// Type-B contributions (ns, k = 1) to the reference − UTC(k) uncertainty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyBudget {
    /// Receiver delay calibration (e.g. 2.5 ns for a BIPM group 1 calibration; tens of ns
    /// uncalibrated).
    pub calibration_ns: f64,
    /// Residual ionosphere: ≈ 0 for L3P, a few ns for Klobuchar single frequency.
    pub iono_ns: f64,
    pub tropo_ns: f64,
    pub multipath_ns: f64,
    /// Broadcast orbit and clock (signal-in-space error) per satellite, reduced by `√n`.
    pub sis_ns: f64,
    /// Antenna coordinate error expressed in time.
    pub position_ns: f64,
}

impl Default for UncertaintyBudget {
    fn default() -> Self {
        UncertaintyBudget {
            calibration_ns: 5.0,
            iono_ns: 0.0,
            tropo_ns: 0.5,
            multipath_ns: 1.0,
            sis_ns: 2.0,
            position_ns: 0.3,
        }
    }
}

/// One value of the local reference against UTC(k).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefVsUtck {
    pub mjd: f64,
    /// Reference − GPS time (ns), as measured.
    pub ref_minus_gpst_ns: f64,
    /// UTC(k) − GPS time (ns), as input.
    pub utck_minus_gpst_ns: f64,
    /// Reference − UTC(k) (ns).
    pub ref_minus_utck_ns: f64,
    /// Combined standard uncertainty (ns, k = 1) and its parts.
    pub u_ns: f64,
    pub u_statistical_ns: f64,
    pub u_type_b_ns: f64,
    pub u_offset_ns: f64,
    pub n_sats: usize,
}

/// Express the all-in-view tracks (per track start, the median REFSYS over satellites) against
/// UTC(k). The statistical part of each value is the standard deviation of the satellites'
/// REFSYS at that track start divided by `√n` (10 ns when only one satellite).
pub fn ref_vs_utck(
    tracks: &[CggttsTrack],
    offsets: &[UtckGpsOffset],
    budget: &UncertaintyBudget,
) -> Vec<RefVsUtck> {
    use std::collections::BTreeMap;
    let mut by: BTreeMap<(i64, u32), Vec<f64>> = BTreeMap::new();
    for t in tracks {
        by.entry((t.mjd, t.start_s)).or_default().push(t.refsys_s);
    }
    by.into_iter()
        .filter_map(|((mjd, start), v)| {
            let n = v.len();
            let mjd_f = mjd as f64 + (start as f64 + TRACK_S as f64 / 2.0) / 86_400.0;
            let (off, u_off) = interpolate_offset(offsets, mjd_f)?;
            let med = median(v.clone());
            let stat = if n > 1 {
                let mean = v.iter().sum::<f64>() / n as f64;
                (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
                    / (n as f64).sqrt()
                    * 1e9
            } else {
                f64::NAN
            };
            let stat = if stat.is_finite() { stat } else { 10.0 };
            let b = (budget.calibration_ns.powi(2)
                + budget.iono_ns.powi(2)
                + budget.tropo_ns.powi(2)
                + budget.multipath_ns.powi(2) / n as f64
                + budget.sis_ns.powi(2) / n as f64
                + budget.position_ns.powi(2))
            .sqrt();
            let u = (stat * stat + b * b + u_off * u_off).sqrt();
            Some(RefVsUtck {
                mjd: mjd_f,
                ref_minus_gpst_ns: med * 1e9,
                utck_minus_gpst_ns: off,
                ref_minus_utck_ns: med * 1e9 - off,
                u_ns: u,
                u_statistical_ns: stat,
                u_type_b_ns: b,
                u_offset_ns: u_off,
                n_sats: n,
            })
        })
        .collect()
}

// ------------------------------------------------------------------ observation synthesis

/// Settings of [`synthesize_obs`].
#[derive(Clone, Debug)]
pub struct ObsSynth {
    pub antenna_ecef: [f64; 3],
    /// First epoch (GPS time) as calendar fields.
    pub start: (i32, u32, u32, u32, u32, f64),
    pub epochs: usize,
    pub interval_s: f64,
    /// Write C2W as well as C1C.
    pub dual_freq: bool,
    /// White code noise (m, one sigma) on each code.
    pub noise_m: f64,
    pub seed: u64,
    pub mask_deg: f64,
    /// Klobuchar coefficients of the truth ionosphere.
    pub iono: KlobucharCoeffs,
    /// Scale of the truth ionosphere against the Klobuchar model (1 = exact model).
    pub iono_scale: f64,
}

fn gps_to_calendar(t_gps_s: f64) -> (i64, i64, i64, i64, i64, f64) {
    let days = (t_gps_s / 86_400.0).floor();
    let sod = t_gps_s - days * 86_400.0;
    let (y, m, d) = civil_from_days(days as i64 + days_from_civil(1980, 1, 6));
    let h = (sod / 3600.0).floor();
    let mi = ((sod - h * 3600.0) / 60.0).floor();
    (y, m, d, h as i64, mi as i64, sod - h * 3600.0 - mi * 60.0)
}

fn normal(rng: &mut ChaCha8Rng) -> f64 {
    use rand::Rng;
    let u1: f64 = rng.gen::<f64>().max(1e-300);
    let u2: f64 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// A RINEX 3.04 observation file of GPS code pseudoranges a receiver driven by a reference
/// with `ref_minus_gpst(t)` (s, `t` seconds since the first epoch) would record at a static
/// antenna: geometric range at the true transmit time (Sagnac), broadcast satellite clock and
/// `TGD` (L2 with `γ·TGD`), Klobuchar ionosphere (scaled by `1/f²` on L2), Saastamoinen–Niell
/// troposphere, white noise. Uses the open engine's measurement models
/// (`gnss_sim::klobuchar_delay_m`, `gnss_sim::tropo_delay_m`). Epoch tags are receiver time.
pub fn synthesize_obs(
    ephs: &[RinexEphemeris],
    cfg: &ObsSynth,
    ref_minus_gpst: &dyn Fn(f64) -> f64,
) -> String {
    let (y, mo, d, h, mi, s) = cfg.start;
    let t0 = (days_from_civil(y as i64, mo as i64, d as i64) - days_from_civil(1980, 1, 6)) as f64
        * 86_400.0
        + h as f64 * 3600.0
        + mi as f64 * 60.0
        + s;
    let station = ecef_to_geodetic(cfg.antenna_ecef);
    let meteo = Meteo::default();
    let gamma = (L1_HZ / L2_HZ).powi(2);
    let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed);
    let codes: Vec<&str> = if cfg.dual_freq {
        vec!["C1C", "C2W"]
    } else {
        vec!["C1C"]
    };
    let mut out = String::new();
    let hl = |out: &mut String, body: String, label: &str| {
        out.push_str(&format!("{body:<60}{label}\n"));
    };
    hl(
        &mut out,
        format!(
            "{:>9}{:11}{:<20}{:<20}",
            "3.04", "", "OBSERVATION DATA", "G"
        ),
        "RINEX VERSION / TYPE",
    );
    hl(
        &mut out,
        format!(
            "{:<20}{:<20}{:<20}",
            "kshana-pro", "synthetic", "20261118 000000 UTC"
        ),
        "PGM / RUN BY / DATE",
    );
    hl(&mut out, "SYNT".into(), "MARKER NAME");
    hl(
        &mut out,
        format!(
            "{:14.4}{:14.4}{:14.4}",
            cfg.antenna_ecef[0], cfg.antenna_ecef[1], cfg.antenna_ecef[2]
        ),
        "APPROX POSITION XYZ",
    );
    hl(
        &mut out,
        format!(
            "G{:>5} {}",
            codes.len(),
            codes
                .iter()
                .map(|c| format!("{c:>3}"))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        "SYS / # / OBS TYPES",
    );
    hl(&mut out, format!("{:10.3}", cfg.interval_s), "INTERVAL");
    hl(
        &mut out,
        format!("{:6}{:6}{:6}{:6}{:6}{:13.7}     GPS", y, mo, d, h, mi, s),
        "TIME OF FIRST OBS",
    );
    hl(&mut out, String::new(), "END OF HEADER");
    let mut prns: Vec<u8> = ephs
        .iter()
        .filter(|e| e.system == 'G')
        .map(|e| e.prn)
        .collect();
    prns.sort();
    prns.dedup();
    for k in 0..cfg.epochs {
        let tag = t0 + k as f64 * cfg.interval_s;
        let b = ref_minus_gpst(tag - t0);
        let t_true = tag - b;
        let tow = t_true.rem_euclid(604_800.0);
        let doy = {
            let (yy, mm, dd, ..) = gps_to_calendar(t_true);
            (days_from_civil(yy, mm, dd) - days_from_civil(yy, 1, 1) + 1) as f64
        };
        let mut recs = Vec::new();
        for &prn in &prns {
            let Some(eph) = kshana::pvt::select_ephemeris(ephs, 'G', prn, tow) else {
                continue;
            };
            let mut tau = 0.075;
            let mut sat = eph.sv_position_ecef(tow - tau);
            for _ in 0..4 {
                sat = sagnac_rotate(eph.sv_position_ecef(tow - tau), tau);
                let dd = [
                    sat[0] - cfg.antenna_ecef[0],
                    sat[1] - cfg.antenna_ecef[1],
                    sat[2] - cfg.antenna_ecef[2],
                ];
                tau = (dd[0] * dd[0] + dd[1] * dd[1] + dd[2] * dd[2]).sqrt() / C_M_PER_S;
            }
            let look = look_angles(station, sat);
            if look.el_rad.to_degrees() < cfg.mask_deg {
                continue;
            }
            let geo = tau * C_M_PER_S;
            let dtsv = eph.sv_clock_bias_s(tow - tau);
            let i1 = cfg.iono_scale
                * klobuchar_delay_m(
                    &cfg.iono,
                    station.lat_rad,
                    station.lon_rad,
                    look.el_rad,
                    look.az_rad,
                    tow.rem_euclid(86_400.0),
                );
            let tr = tropo_delay_m(&meteo, station.lat_rad, station.alt_m, look.el_rad, doy);
            let common = geo + C_M_PER_S * (b - dtsv) + tr;
            let p1 = common + C_M_PER_S * eph.tgd + i1 + cfg.noise_m * normal(&mut rng);
            let mut rec = format!("G{prn:02}{p1:14.3}  ");
            if cfg.dual_freq {
                let p2 = common
                    + gamma * C_M_PER_S * eph.tgd
                    + gamma * i1
                    + cfg.noise_m * normal(&mut rng);
                rec.push_str(&format!("{p2:14.3}  "));
            }
            recs.push(rec.trim_end().to_string());
        }
        let (yy, mm, dd, hh, mn, ss) = gps_to_calendar(tag);
        out.push_str(&format!(
            "> {yy:4} {mm:02} {dd:02} {hh:02} {mn:02}{ss:11.7}  0{:3}\n",
            recs.len()
        ));
        for r in recs {
            out.push_str(&r);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_repeats_four_minutes_earlier_each_day() {
        assert_eq!(track_starts(50_722)[0], 120);
        assert_eq!(track_starts(50_723)[0], 14 * 60);
        assert_eq!(track_starts(50_726)[0], 2 * 60 + 16 * 60 - 16 * 60);
        assert!(track_starts(60_000).len() >= 89);
    }

    #[test]
    fn calendar_round_trip() {
        let (y, m, d, h, mi, s) = gps_to_calendar(1_410_000_000.5);
        let back = (days_from_civil(y, m, d) - days_from_civil(1980, 1, 6)) as f64 * 86_400.0
            + (h * 3600 + mi * 60) as f64
            + s;
        assert!((back - 1_410_000_000.5).abs() < 1e-6);
    }
}
