// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Report verification (`kshana-pro/report-verification/v1`): one normalised report
//! cross-checked against the observation data available for its time and place, labelled
//! CORROBORATED, UNCORROBORATED or CONTRADICTED with the reasons, and signed.
//!
//! Sources:
//!
//! * receiver logs in any format the open engine reads (UBX, RINEX, Android, SBF, NMEA,
//!   RTKLIB, NovAtel, RTCM 3, GNSS-SDR), or a CSV of `timestamp,lat,lon[,cn0_dbhz,sats]`.
//!   Role `subject` (the reporter's own receiver) or `independent` (a Station, another
//!   receiver nearby). Indicators against the 30 minutes before the window: C/N0 drop,
//!   satellites lost, loss of fix (jamming-like); C/N0 raised and uniform, position jump,
//!   the receiver's own spoofing flag (spoofing-like);
//! * ADS-B CSV (`timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp`): the subject's own
//!   track (NIC/NACp drop after reporting good values, jumps) and the other aircraft within
//!   `nearby_radius_km`;
//! * AIS CSV (`timestamp,vessel_id,lat,lon[,sog_kn]`): the subject's track (jumps, speeds
//!   over the limit) and the vessels nearby.
//!
//! Label rules (written into every record):
//!
//! * **CORROBORATED**: at least one source shows an anomaly in the window (a crew's
//!   jamming/spoofing call is often wrong, so a corroborated report whose observed kind
//!   differs carries `kind_agreement: false`).
//! * **CONTRADICTED**: no source shows an anomaly, and a direct source (the subject's own
//!   receiver, or for a jamming or unknown report the subject's own ADS-B track with an
//!   integrity baseline) covers at least `coverage_min` of the window. ADS-B integrity cannot
//!   contradict a spoofing report (a spoofed position can carry good NIC), and AIS never
//!   contradicts alone.
//! * **UNCORROBORATED**: everything else (no data, too little coverage, nearby sources only,
//!   or a time too coarse to place the window).
//!
//! Evidence: MODELLED. The thresholds are the incident-forensics method's where they overlap;
//! the rules were checked on synthetic data only.

use crate::timing::selfcheck::SkippedRows;
use std::path::Path;

use kshana::interference_map::grid::haversine_m;
use kshana::receiver_trust::ingest::read_log;
use kshana::receiver_trust::LogFormat;
use serde::{Deserialize, Serialize};

use super::text::{iso, round};
use super::*;
use crate::forensics::inputs::{parse_tracks, PositionReport, Tracks};

pub const FORMAT: &str = "kshana-pro-report-verification/1";
pub const METHOD_ID: &str = "kshana-pro/report-verification/v1";
pub const RESULT_FILE: &str = "verification.json";
pub const EVIDENCE: &str = "MODELLED: rules checked on synthetic observations with known truth; not \
validated on a labelled set of real reports. A label states how the data supplied relates to the report, not whether the report is true.";

/// The label of a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Label {
    Corroborated,
    Uncorroborated,
    Contradicted,
}

impl Label {
    pub fn as_str(self) -> &'static str {
        match self {
            Label::Corroborated => "CORROBORATED",
            Label::Uncorroborated => "UNCORROBORATED",
            Label::Contradicted => "CONTRADICTED",
        }
    }
}

/// The label rules, as written into every output.
pub fn label_rules(p: &Params) -> Vec<String> {
    vec![
        "CORROBORATED: at least one source shows an anomaly in the window; kind_agreement says whether its kind matches the report's.".into(),
        format!(
            "CONTRADICTED: no anomaly in any source, and a direct source covers at least {:.0} % of the window: the subject's own receiver \
             (with a baseline), or for a jamming or unknown report the subject's own ADS-B track with an integrity baseline. \
             ADS-B integrity cannot contradict a spoofing report; AIS never contradicts alone.",
            p.coverage_min * 100.0
        ),
        "UNCORROBORATED: anything else (no data, too little coverage, nearby sources only, time too coarse).".into(),
    ]
}

/// Every threshold of the method.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub window_pad_min: f64,
    pub default_duration_min: f64,
    pub baseline_min: f64,
    pub nearby_radius_km: f64,
    pub coverage_min: f64,
    pub max_time_uncertainty_min: f64,
    pub cn0_drop_db: f64,
    pub sats_lost: f64,
    pub cn0_rise_db: f64,
    pub jump_m_static: f64,
    pub max_speed_kn_moving: f64,
    pub receiver_gap_s: f64,
    pub adsb_nic_min: u8,
    pub adsb_nacp_min: u8,
    pub adsb_jump_km: f64,
    pub adsb_jump_kn: f64,
    pub adsb_gap_s: f64,
    pub ais_jump_m: f64,
    pub ais_max_kn: f64,
    pub ais_gap_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            window_pad_min: 10.0,
            default_duration_min: 5.0,
            baseline_min: 30.0,
            nearby_radius_km: 100.0,
            coverage_min: 0.8,
            max_time_uncertainty_min: 15.0,
            cn0_drop_db: 6.0,
            sats_lost: 4.0,
            cn0_rise_db: 3.0,
            jump_m_static: 50.0,
            max_speed_kn_moving: 600.0,
            receiver_gap_s: 30.0,
            adsb_nic_min: 7,
            adsb_nacp_min: 8,
            adsb_jump_km: 5.0,
            adsb_jump_kn: 900.0,
            adsb_gap_s: 60.0,
            ais_jump_m: 500.0,
            ais_max_kn: 50.0,
            ais_gap_s: 360.0,
        }
    }
}

/// Whose receiver it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObsRole {
    /// The reporter's own receiver (direct).
    #[default]
    Subject,
    /// A Station or any other receiver (independent, not direct).
    Independent,
}

/// One receiver log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverSpec {
    pub label: String,
    pub path: String,
    /// An open-engine log format (`ubx`, `rinex`, `android`, `sbf`, `nmea`, `rtklib-pos`,
    /// `novatel`, `rtcm3`, `gnss-sdr`) or `csv`.
    pub format: String,
    #[serde(default)]
    pub role: ObsRole,
    /// A fixed receiver's surveyed position `[lat, lon]`.
    #[serde(default)]
    pub static_position: Option<[f64; 2]>,
    /// A fixed receiver without a surveyed position (the baseline median is used).
    #[serde(default, rename = "static")]
    pub is_static: bool,
    #[serde(default)]
    pub max_speed_kn: Option<f64>,
    /// For a `subject` receiver: whose it is (registration, IMO, MMSI). It is direct evidence
    /// only for that subject's reports; for other reports it counts as an independent
    /// receiver when it was within `nearby_radius_km`. Without it, a subject receiver is
    /// direct for every report.
    #[serde(default)]
    pub subject_id: Option<String>,
}

/// One ADS-B or AIS file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackSpec {
    pub path: String,
    /// The subject's id in this file, when it is not the report's registration / IMO / MMSI
    /// (an ICAO 24-bit address, say).
    #[serde(default)]
    pub subject_id: Option<String>,
}

/// The verification request (`verify.toml`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// A normalised reports file (`kshana-pro reports normalise`).
    pub reports: String,
    /// The reports to verify; all of them when empty.
    #[serde(default)]
    pub report_ids: Vec<String>,
    #[serde(default)]
    pub receiver: Vec<ReceiverSpec>,
    #[serde(default)]
    pub adsb: Vec<TrackSpec>,
    #[serde(default)]
    pub ais: Vec<TrackSpec>,
    /// Other ids of a subject: `"800ABC" = "VT-ABC"` (ICAO address and registration,
    /// MMSI and IMO). Either side matches the other.
    #[serde(default)]
    pub aliases: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub params: Params,
}

/// One epoch of a receiver log, reduced to what the checks use.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Epoch {
    pub t: f64,
    pub fix: Option<(f64, f64)>,
    pub cn0_mean: Option<f64>,
    pub cn0_std: Option<f64>,
    pub n_trk: usize,
    pub self_spoof: bool,
    pub self_jam: bool,
}

/// Read a receiver log (any open-engine format, or `csv`) into epochs with UTC times.
pub fn load_receiver(format: &str, bytes: &[u8]) -> Result<Vec<Epoch>, String> {
    load_receiver_counted(format, bytes).map(|(e, _)| e)
}

/// [`load_receiver`] with the skipped rows of a `csv` log counted (the open-engine formats
/// report their own).
pub fn load_receiver_counted(
    format: &str,
    bytes: &[u8],
) -> Result<(Vec<Epoch>, SkippedRows), String> {
    if format == "csv" {
        return load_csv_positions_counted(&String::from_utf8_lossy(bytes));
    }
    let f: LogFormat = serde_json::from_value(serde_json::Value::String(format.into()))
        .map_err(|_| format!("unknown receiver log format '{format}'"))?;
    let tl = read_log(f, bytes)?;
    let mut out = vec![];
    for e in &tl.epochs {
        let Some(t) = e.time_label.as_deref().and_then(text::parse_utc) else {
            continue;
        };
        let cn: Vec<f64> = e
            .cn0
            .iter()
            .map(|s| s.cn0_dbhz)
            .filter(|c| *c > 0.0)
            .collect();
        let (m, s) = mean_std(&cn).map_or((None, None), |(m, s)| (Some(m), Some(s)));
        let valid = e
            .marine
            .as_ref()
            .and_then(|m| m.fix_valid)
            .unwrap_or(e.fix.is_some());
        out.push(Epoch {
            t,
            fix: if valid {
                e.fix.as_ref().map(|f| (f.lat_deg, f.lon_deg))
            } else {
                None
            },
            cn0_mean: m,
            cn0_std: s,
            n_trk: cn.len(),
            self_spoof: e
                .self_report
                .as_ref()
                .is_some_and(|r| r.spoofing.is_some_and(|v| v >= 2)),
            self_jam: e
                .self_report
                .as_ref()
                .is_some_and(|r| r.jamming.is_some_and(|v| v >= 2)),
        });
    }
    if out.is_empty() {
        return Err("the log has no epoch with a UTC time".into());
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((out, SkippedRows::default()))
}

/// A CSV of positions: `timestamp,lat,lon[,cn0_dbhz,sats,fix]`. An empty lat/lon (or
/// `fix` 0) is an epoch without a fix.
pub fn load_csv_positions(text_in: &str) -> Result<Vec<Epoch>, String> {
    load_csv_positions_counted(text_in).map(|(e, _)| e)
}

/// [`load_csv_positions`] with the rows whose timestamp could not be read counted.
pub fn load_csv_positions_counted(text_in: &str) -> Result<(Vec<Epoch>, SkippedRows), String> {
    let mut skipped = SkippedRows::new("csv");
    let recs = text::csv_records(text_in);
    let head: Vec<String> = recs
        .first()
        .ok_or("empty CSV")?
        .split(',')
        .map(text::norm_key)
        .collect();
    let col = |names: &[&str]| head.iter().position(|h| names.contains(&h.as_str()));
    let ct =
        col(&["timestamp", "time", "utc", "time utc"]).ok_or("CSV has no 'timestamp' column")?;
    let (cla, clo) = (
        col(&["lat", "latitude"]).ok_or("CSV has no 'lat' column")?,
        col(&["lon", "longitude", "long"]).ok_or("CSV has no 'lon' column")?,
    );
    let (ccn, cs, cf) = (
        col(&["cn0 dbhz", "cn0", "cn0 mean"]),
        col(&["sats", "satellites", "n sats", "sats used"]),
        col(&["fix", "fix valid"]),
    );
    let mut out = vec![];
    for (k, line) in recs[1..].iter().enumerate() {
        let f = text::csv_fields(line);
        let num = |c: Option<usize>| {
            c.and_then(|c| f.get(c))
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|x| x.is_finite())
        };
        let Some(t) = f.get(ct).and_then(|v| text::parse_utc(v)) else {
            skipped.add(k + 2, "timestamp not readable");
            continue;
        };
        let fix_ok = num(cf).is_none_or(|v| v > 0.0);
        let fix = match (num(Some(cla)), num(Some(clo))) {
            (Some(a), Some(b))
                if fix_ok && (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b) =>
            {
                Some((a, b))
            }
            _ => None,
        };
        out.push(Epoch {
            t,
            fix,
            cn0_mean: num(ccn),
            cn0_std: None,
            n_trk: num(cs).map_or(0, |v| v.max(0.0) as usize),
            ..Default::default()
        });
    }
    if out.is_empty() {
        return Err("no CSV row with a readable timestamp".into());
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((out, skipped))
}

fn mean_std(v: &[f64]) -> Option<(f64, f64)> {
    if v.is_empty() {
        return None;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    Some((
        m,
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt(),
    ))
}

pub(crate) fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// An anomaly a source shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Indicator {
    pub name: String,
    /// The kind it is like.
    pub kind: Kind,
    pub first_utc: String,
    pub value: f64,
    pub detail: String,
}

/// What one source showed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceCheck {
    pub label: String,
    /// `receiver`, `adsb` or `ais`.
    pub source_type: String,
    /// The subject's own data.
    pub direct: bool,
    pub file: String,
    /// Share of the window the source covers.
    #[serde(default)]
    pub coverage: Option<f64>,
    /// Whether it has the baseline its checks need (a receiver's C/N0 before the window, an
    /// aircraft's good NIC/NACp before degrading).
    pub baseline: bool,
    pub indicators: Vec<Indicator>,
    pub notes: Vec<String>,
}

/// The verification of one report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub report_id: String,
    pub report: Report,
    #[serde(default)]
    pub window_start_utc: Option<String>,
    #[serde(default)]
    pub window_end_utc: Option<String>,
    pub label: Label,
    pub reasons: Vec<String>,
    /// For a corroborated report: whether an observed kind matches the reported one.
    #[serde(default)]
    pub kind_agreement: Option<bool>,
    pub observed_kinds: Vec<Kind>,
    /// For a vessel's spoofing report: `gnss` when the vessel's own receiver shows spoofing,
    /// `ais-only` when only its AIS track does while its receiver covers the window cleanly.
    #[serde(default)]
    pub observed_layer: Option<String>,
    pub checks: Vec<SourceCheck>,
}

/// The verification document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationDoc {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub label_rules: Vec<String>,
    pub params: Params,
    pub params_sha256: String,
    pub inputs: Vec<InputFile>,
    pub records: Vec<Record>,
    /// Rows of the inputs that could not be read ("N rows skipped").
    #[serde(default)]
    pub skipped_rows: Vec<SkippedRows>,
}

fn coverage(times: &[f64], w0: f64, w1: f64, max_gap: f64) -> f64 {
    let ts: Vec<f64> = times
        .iter()
        .copied()
        .filter(|t| *t >= w0 && *t <= w1)
        .collect();
    let covered: f64 = ts
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|g| *g <= max_gap)
        .sum();
    if w1 > w0 {
        round((covered / (w1 - w0)).min(1.0), 3)
    } else {
        0.0
    }
}

/// The checks of one receiver log over `[w0, w1]`.
pub fn check_receiver(
    spec: &ReceiverSpec,
    file: &str,
    ep: &[Epoch],
    w0: f64,
    w1: f64,
    p: &Params,
) -> SourceCheck {
    let base: Vec<&Epoch> = ep
        .iter()
        .filter(|e| e.t < w0 && e.t >= w0 - p.baseline_min * 60.0)
        .collect();
    let win: Vec<&Epoch> = ep.iter().filter(|e| e.t >= w0 && e.t <= w1).collect();
    let times: Vec<f64> = ep.iter().map(|e| e.t).collect();
    let mut c = SourceCheck {
        label: spec.label.clone(),
        source_type: "receiver".into(),
        direct: spec.role == ObsRole::Subject,
        file: file.into(),
        coverage: Some(coverage(&times, w0, w1, p.receiver_gap_s)),
        baseline: base.len() >= 5,
        indicators: vec![],
        notes: vec![],
    };
    if win.is_empty() {
        c.notes.push("no epoch in the window".into());
        return c;
    }
    let mut push = |name: &str, kind: Kind, hits: &[&Epoch], value: f64, detail: String| {
        if hits.len() >= 2 {
            c.indicators.push(Indicator {
                name: name.into(),
                kind,
                first_utc: iso(hits[0].t),
                value: round(value, 2),
                detail,
            });
        }
    };
    let bm = median(base.iter().filter_map(|e| e.cn0_mean).collect());
    let bs = median(base.iter().filter_map(|e| e.cn0_std).collect());
    let btrk = median(base.iter().map(|e| e.n_trk as f64).collect()).unwrap_or(0.0);
    let bfix = if base.is_empty() {
        0.0
    } else {
        base.iter().filter(|e| e.fix.is_some()).count() as f64 / base.len() as f64
    };
    if base.len() >= 5 {
        if let Some(bm) = bm {
            let hits: Vec<&Epoch> = win
                .iter()
                .copied()
                .filter(|e| e.cn0_mean.is_some_and(|m| bm - m >= p.cn0_drop_db))
                .collect();
            let worst = hits
                .iter()
                .filter_map(|e| e.cn0_mean)
                .map(|m| bm - m)
                .fold(0.0, f64::max);
            push(
                "cn0-drop",
                Kind::Jamming,
                &hits,
                worst,
                format!("mean C/N0 down {worst:.1} dB from the baseline {bm:.1} dB-Hz"),
            );
            if let Some(bs) = bs {
                let hits: Vec<&Epoch> = win
                    .iter()
                    .copied()
                    .filter(|e| {
                        e.cn0_mean.is_some_and(|m| m - bm >= p.cn0_rise_db)
                            && e.cn0_std.is_some_and(|s| s < 0.5 * bs)
                    })
                    .collect();
                let rise = hits
                    .iter()
                    .filter_map(|e| e.cn0_mean)
                    .map(|m| m - bm)
                    .fold(0.0, f64::max);
                push(
                    "cn0-raised-uniform",
                    Kind::Spoofing,
                    &hits,
                    rise,
                    format!(
                        "C/N0 up {rise:.1} dB with spread under half the baseline's {bs:.1} dB"
                    ),
                );
            }
        }
        if btrk >= p.sats_lost {
            let hits: Vec<&Epoch> = win
                .iter()
                .copied()
                .filter(|e| btrk - e.n_trk as f64 >= p.sats_lost)
                .collect();
            let lost = hits
                .iter()
                .map(|e| btrk - e.n_trk as f64)
                .fold(0.0, f64::max);
            push(
                "satellites-lost",
                Kind::Jamming,
                &hits,
                lost,
                format!("{lost:.0} of {btrk:.0} baseline satellites lost"),
            );
        }
        if bfix >= 0.9 {
            let hits: Vec<&Epoch> = win.iter().copied().filter(|e| e.fix.is_none()).collect();
            let n = hits.len() as f64;
            push(
                "fix-lost",
                Kind::Jamming,
                &hits,
                n,
                format!(
                    "{n:.0} epoch(s) without a fix (a spoofer's take-over can also cause this)"
                ),
            );
        }
    } else {
        c.notes.push(format!(
            "fewer than 5 epochs in the {} min before the window: C/N0, satellite and fix checks not assessed",
            p.baseline_min
        ));
    }
    // Position checks.
    let anchor = spec.static_position.map(|a| (a[0], a[1])).or_else(|| {
        if spec.is_static {
            let la = median(base.iter().filter_map(|e| e.fix.map(|f| f.0)).collect())?;
            let lo = median(base.iter().filter_map(|e| e.fix.map(|f| f.1)).collect())?;
            Some((la, lo))
        } else {
            None
        }
    });
    if let Some((la, lo)) = anchor {
        let hits: Vec<&Epoch> = win
            .iter()
            .copied()
            .filter(|e| {
                e.fix
                    .is_some_and(|f| haversine_m(la, lo, f.0, f.1) > p.jump_m_static)
            })
            .collect();
        let far = hits
            .iter()
            .filter_map(|e| e.fix.map(|f| haversine_m(la, lo, f.0, f.1)))
            .fold(0.0, f64::max);
        push(
            "position-off-fixed-site",
            Kind::Spoofing,
            &hits,
            far,
            format!("fixed receiver's position up to {far:.0} m from its site"),
        );
    } else {
        let max_kn = spec.max_speed_kn.unwrap_or(p.max_speed_kn_moving);
        let mut hits: Vec<&Epoch> = vec![];
        let mut worst = 0.0f64;
        let fixes: Vec<&Epoch> = ep
            .iter()
            .filter(|e| e.fix.is_some() && e.t >= w0 - 60.0 && e.t <= w1)
            .collect();
        for w in fixes.windows(2) {
            let (a, b) = (w[0].fix.expect("fix"), w[1].fix.expect("fix"));
            let d = haversine_m(a.0, a.1, b.0, b.1);
            let kn = d / (w[1].t - w[0].t).max(1e-3) / 0.514_444;
            if kn > max_kn && d > 3.0 * p.jump_m_static && w[1].t >= w0 {
                hits.push(w[1]);
                worst = worst.max(d);
            }
        }
        if hits.len() == 1 {
            hits.push(hits[0]);
        }
        push(
            "position-jump",
            Kind::Spoofing,
            &hits,
            worst,
            format!("position jump up to {worst:.0} m, above {max_kn:.0} kn implied"),
        );
    }
    let sp: Vec<&Epoch> = win.iter().copied().filter(|e| e.self_spoof).collect();
    push(
        "receiver-spoofing-flag",
        Kind::Spoofing,
        &sp,
        sp.len() as f64,
        "the receiver's own spoofing state 2 or more".into(),
    );
    let jm: Vec<&Epoch> = win.iter().copied().filter(|e| e.self_jam).collect();
    push(
        "receiver-jamming-flag",
        Kind::Jamming,
        &jm,
        jm.len() as f64,
        "the receiver's own jamming state 2 or more".into(),
    );
    c
}

fn norm_id(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

/// The ids a report's subject goes by, normalised, with their aliases.
fn subject_ids(r: &Report, aliases: &[(String, String)]) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    v.extend(r.subject_id.clone());
    v.extend(r.flight_or_voyage.clone());
    v.extend(r.extra.get("mmsi").cloned());
    v.extend(r.extra.get("imo").cloned());
    v.extend(r.extra.get("registration").cloned());
    let mut out: Vec<String> = v
        .iter()
        .map(|s| norm_id(s))
        .filter(|s| !s.is_empty())
        .collect();
    for (a, b) in aliases {
        if out.contains(a) && !out.contains(b) {
            out.push(b.clone());
        } else if out.contains(b) && !out.contains(a) {
            out.push(a.clone());
        }
    }
    out
}

fn ids_for(r: &Report, spec: &TrackSpec, aliases: &[(String, String)]) -> Vec<String> {
    let mut v = subject_ids(r, aliases);
    v.extend(spec.subject_id.as_deref().map(norm_id));
    v
}

fn adsb_degraded(r: &PositionReport, p: &Params) -> Option<bool> {
    match (r.nic, r.nacp) {
        (None, None) => None,
        (a, b) => {
            Some(a.is_some_and(|v| v < p.adsb_nic_min) || b.is_some_and(|v| v < p.adsb_nacp_min))
        }
    }
}

/// Jumps in a track within the window: (time, distance m, knots).
fn jumps(v: &[PositionReport], w0: f64, w1: f64, min_m: f64, max_kn: f64) -> Vec<(f64, f64, f64)> {
    v.windows(2)
        .filter(|w| w[1].t >= w0 && w[1].t <= w1)
        .filter_map(|w| {
            let d = haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
            let kn = d / (w[1].t - w[0].t).max(1e-3) / 0.514_444;
            (d >= min_m && kn > max_kn).then_some((w[1].t, d, kn))
        })
        .collect()
}

/// The ADS-B checks: the subject's own track and the aircraft nearby.
fn check_adsb(
    r: &Report,
    spec: &TrackSpec,
    file: &str,
    tr: &Tracks,
    (w0, w1): (f64, f64),
    p: &Params,
    obs_aliases: &[(String, String)],
) -> Vec<SourceCheck> {
    let ids = ids_for(r, spec, obs_aliases);
    let mut out = vec![];
    let subject = tr.by_id.iter().find(|(k, _)| ids.contains(&norm_id(k)));
    if let Some((id, v)) = subject {
        let good_before = v
            .iter()
            .any(|x| x.t < w0 && adsb_degraded(x, p) == Some(false));
        let times: Vec<f64> = v.iter().map(|x| x.t).collect();
        let mut c = SourceCheck {
            label: format!("ADS-B {id} (subject)"),
            source_type: "adsb".into(),
            direct: true,
            file: file.into(),
            coverage: Some(coverage(&times, w0, w1, p.adsb_gap_s)),
            baseline: good_before,
            indicators: vec![],
            notes: vec![],
        };
        let deg: Vec<&PositionReport> = v
            .iter()
            .filter(|x| x.t >= w0 && x.t <= w1 && adsb_degraded(x, p) == Some(true))
            .collect();
        if good_before && deg.len() >= 2 {
            let nic = deg.iter().filter_map(|x| x.nic).min();
            let nacp = deg.iter().filter_map(|x| x.nacp).min();
            c.indicators.push(Indicator {
                name: "adsb-integrity-drop".into(),
                kind: Kind::Jamming,
                first_utc: iso(deg[0].t),
                value: deg.len() as f64,
                detail: format!(
                    "{} sample(s) with NIC/NACp below {}/{} (lowest NIC {nic:?}, NACp {nacp:?})",
                    deg.len(),
                    p.adsb_nic_min,
                    p.adsb_nacp_min
                ),
            });
        } else if !good_before {
            c.notes
                .push("no good NIC/NACp before the window: integrity drop not assessable".into());
        }
        for (t, d, kn) in jumps(v, w0, w1, p.adsb_jump_km * 1000.0, p.adsb_jump_kn) {
            c.indicators.push(Indicator {
                name: "adsb-position-jump".into(),
                kind: Kind::Spoofing,
                first_utc: iso(t),
                value: round(d, 0),
                detail: format!("position jump {:.1} km at {kn:.0} kn implied", d / 1000.0),
            });
        }
        out.push(c);
    }
    if let Some(pos) = &r.position {
        let (mut assessed, mut degraded, mut jumped) = (0usize, vec![], vec![]);
        for (id, v) in &tr.by_id {
            if subject.is_some_and(|(s, _)| s == id) {
                continue;
            }
            let near = v.iter().any(|x| {
                x.t >= w0
                    && x.t <= w1
                    && haversine_m(pos.lat_deg, pos.lon_deg, x.lat, x.lon)
                        <= p.nearby_radius_km * 1000.0
            });
            if !near {
                continue;
            }
            let good_before = v
                .iter()
                .any(|x| x.t < w0 && adsb_degraded(x, p) == Some(false));
            if good_before {
                assessed += 1;
                if v.iter()
                    .filter(|x| x.t >= w0 && x.t <= w1 && adsb_degraded(x, p) == Some(true))
                    .count()
                    >= 2
                {
                    degraded.push(id.clone());
                }
            }
            if !jumps(v, w0, w1, p.adsb_jump_km * 1000.0, p.adsb_jump_kn).is_empty() {
                jumped.push(id.clone());
            }
        }
        let mut c = SourceCheck {
            label: format!("ADS-B aircraft within {} km", p.nearby_radius_km),
            source_type: "adsb".into(),
            direct: false,
            file: file.into(),
            coverage: None,
            baseline: assessed > 0,
            indicators: vec![],
            notes: vec![format!(
                "{assessed} nearby aircraft with an integrity baseline"
            )],
        };
        if !degraded.is_empty() {
            c.indicators.push(Indicator {
                name: "nearby-adsb-integrity-drop".into(),
                kind: Kind::Jamming,
                first_utc: iso(w0),
                value: degraded.len() as f64,
                detail: format!(
                    "{} of {assessed} nearby aircraft lost NIC/NACp: {}",
                    degraded.len(),
                    degraded.join(", ")
                ),
            });
        }
        if !jumped.is_empty() {
            c.indicators.push(Indicator {
                name: "nearby-adsb-position-jump".into(),
                kind: Kind::Spoofing,
                first_utc: iso(w0),
                value: jumped.len() as f64,
                detail: format!(
                    "{} nearby aircraft with position jumps: {}",
                    jumped.len(),
                    jumped.join(", ")
                ),
            });
        }
        out.push(c);
    }
    out
}

/// The AIS checks: the subject's own track and the vessels nearby.
fn check_ais(
    r: &Report,
    spec: &TrackSpec,
    file: &str,
    tr: &Tracks,
    (w0, w1): (f64, f64),
    p: &Params,
    obs_aliases: &[(String, String)],
) -> Vec<SourceCheck> {
    let ids = ids_for(r, spec, obs_aliases);
    let mut out = vec![];
    let subject = tr.by_id.iter().find(|(k, _)| ids.contains(&norm_id(k)));
    let anomalies = |v: &[PositionReport]| {
        let mut ind = vec![];
        let js = jumps(v, w0, w1, p.ais_jump_m, p.ais_max_kn);
        if let Some((t, _, _)) = js.first() {
            let d = js.iter().map(|j| j.1).fold(0.0, f64::max);
            ind.push(Indicator {
                name: "ais-position-jump".into(),
                kind: Kind::Spoofing,
                first_utc: iso(*t),
                value: round(d, 0),
                detail: format!(
                    "{} AIS jump(s), up to {:.0} m above {} kn implied",
                    js.len(),
                    d,
                    p.ais_max_kn
                ),
            });
        }
        let fast: Vec<&PositionReport> = v
            .iter()
            .filter(|x| x.t >= w0 && x.t <= w1 && x.sog_kn.is_some_and(|s| s > p.ais_max_kn))
            .collect();
        if let Some(f) = fast.first() {
            ind.push(Indicator {
                name: "ais-implausible-speed".into(),
                kind: Kind::Spoofing,
                first_utc: iso(f.t),
                value: fast.len() as f64,
                detail: format!(
                    "{} report(s) with speed over {} kn",
                    fast.len(),
                    p.ais_max_kn
                ),
            });
        }
        ind
    };
    if let Some((id, v)) = subject {
        let times: Vec<f64> = v.iter().map(|x| x.t).collect();
        out.push(SourceCheck {
            label: format!("AIS {id} (subject)"),
            source_type: "ais".into(),
            direct: true,
            file: file.into(),
            coverage: Some(coverage(&times, w0, w1, p.ais_gap_s)),
            baseline: true,
            indicators: anomalies(v),
            notes: vec!["an AIS track carries the vessel's GNSS position: it can show GNSS spoofing or AIS manipulation, and never contradicts alone".into()],
        });
    }
    if let Some(pos) = &r.position {
        let mut hit = vec![];
        let mut n = 0;
        for (id, v) in &tr.by_id {
            if subject.is_some_and(|(s, _)| s == id) {
                continue;
            }
            if !v.iter().any(|x| {
                x.t >= w0
                    && x.t <= w1
                    && haversine_m(pos.lat_deg, pos.lon_deg, x.lat, x.lon)
                        <= p.nearby_radius_km * 1000.0
            }) {
                continue;
            }
            n += 1;
            if !anomalies(v).is_empty() {
                hit.push(id.clone());
            }
        }
        let mut c = SourceCheck {
            label: format!("AIS vessels within {} km", p.nearby_radius_km),
            source_type: "ais".into(),
            direct: false,
            file: file.into(),
            coverage: None,
            baseline: n > 0,
            indicators: vec![],
            notes: vec![format!("{n} nearby vessel(s) in the window")],
        };
        if !hit.is_empty() {
            c.indicators.push(Indicator {
                name: "nearby-ais-anomaly".into(),
                kind: Kind::Spoofing,
                first_utc: iso(w0),
                value: hit.len() as f64,
                detail: format!(
                    "{} of {n} nearby vessel(s) with jumps or implausible speeds: {}",
                    hit.len(),
                    hit.join(", ")
                ),
            });
        }
        out.push(c);
    }
    out
}

/// Loaded observation data.
pub struct Observations {
    /// Normalised alias pairs (see [`Request::aliases`]).
    pub aliases: Vec<(String, String)>,
    pub receivers: Vec<(ReceiverSpec, String, Vec<Epoch>)>,
    pub adsb: Vec<(TrackSpec, String, Tracks)>,
    pub ais: Vec<(TrackSpec, String, Tracks)>,
    /// Rows of the receiver logs that could not be read.
    pub skipped: Vec<SkippedRows>,
}

/// Label one report.
pub fn verify_report(r: &Report, obs: &Observations, p: &Params) -> Record {
    let mut rec = Record {
        report_id: r.id.clone(),
        report: r.clone(),
        window_start_utc: None,
        window_end_utc: None,
        label: Label::Uncorroborated,
        reasons: vec![],
        kind_agreement: None,
        observed_kinds: vec![],
        observed_layer: None,
        checks: vec![],
    };
    let (Some(s), Some(e)) = (r.start_t(), r.end_t(p.default_duration_min)) else {
        rec.reasons
            .push("the report has no start time: nothing to cross-check".into());
        return rec;
    };
    if r.time_uncertainty_s > p.max_time_uncertainty_min * 60.0 {
        rec.reasons.push(format!(
            "the report's time is uncertain by {:.0} min (over {} min): too coarse to cross-check",
            r.time_uncertainty_s / 60.0,
            p.max_time_uncertainty_min
        ));
        return rec;
    }
    let pad = p.window_pad_min * 60.0 + r.time_uncertainty_s;
    let (w0, w1) = (s - pad, e + pad);
    rec.window_start_utc = Some(iso(w0));
    rec.window_end_utc = Some(iso(w1));
    let ids = subject_ids(r, &obs.aliases);
    for (spec, file, ep) in &obs.receivers {
        let mine = spec
            .subject_id
            .as_deref()
            .is_none_or(|s| ids.contains(&norm_id(s)));
        if spec.role == ObsRole::Subject && !mine {
            // Another subject's receiver: an independent observer when it was nearby.
            let near = r.position.as_ref().is_some_and(|pos| {
                ep.iter().any(|e| {
                    e.t >= w0
                        && e.t <= w1
                        && e.fix.is_some_and(|f| {
                            haversine_m(pos.lat_deg, pos.lon_deg, f.0, f.1)
                                <= p.nearby_radius_km * 1000.0
                        })
                })
            });
            if near {
                let mut c = check_receiver(spec, file, ep, w0, w1, p);
                c.direct = false;
                c.notes.push(format!(
                    "another subject's receiver ({}), within {} km: independent",
                    spec.subject_id.clone().unwrap_or_default(),
                    p.nearby_radius_km
                ));
                rec.checks.push(c);
            }
            continue;
        }
        rec.checks.push(check_receiver(spec, file, ep, w0, w1, p));
    }
    for (spec, file, tr) in &obs.adsb {
        rec.checks
            .extend(check_adsb(r, spec, file, tr, (w0, w1), p, &obs.aliases));
    }
    for (spec, file, tr) in &obs.ais {
        rec.checks
            .extend(check_ais(r, spec, file, tr, (w0, w1), p, &obs.aliases));
    }
    if r.position.is_none() {
        rec.reasons
            .push("no position in the report: nearby sources not checked".into());
    }
    let mut kinds: Vec<Kind> = rec
        .checks
        .iter()
        .flat_map(|c| c.indicators.iter().map(|i| i.kind))
        .collect();
    kinds.sort();
    kinds.dedup();
    rec.observed_kinds = kinds.clone();
    let any = rec.checks.iter().any(|c| !c.indicators.is_empty());
    if any {
        rec.label = Label::Corroborated;
        for c in rec.checks.iter().filter(|c| !c.indicators.is_empty()) {
            for i in &c.indicators {
                rec.reasons.push(format!(
                    "{} ({}): {} from {}: {}",
                    c.label,
                    if c.direct { "direct" } else { "independent" },
                    i.name,
                    i.first_utc,
                    i.detail
                ));
            }
        }
        let agree = r.kind == Kind::Unknown || kinds.contains(&r.kind);
        rec.kind_agreement = Some(agree);
        if !agree {
            rec.reasons.push(format!(
                "the data shows {}-like anomalies, not the reported {}",
                kinds
                    .iter()
                    .map(|k| k.as_str())
                    .collect::<Vec<_>>()
                    .join("/"),
                r.kind.as_str()
            ));
        }
    } else {
        let contradicting: Vec<&SourceCheck> = rec
            .checks
            .iter()
            .filter(|c| c.direct && c.baseline && c.coverage.is_some_and(|v| v >= p.coverage_min))
            .filter(|c| match c.source_type.as_str() {
                "receiver" => true,
                "adsb" => r.kind != Kind::Spoofing,
                _ => false,
            })
            .collect();
        if let Some(c) = contradicting.first() {
            rec.label = Label::Contradicted;
            rec.reasons.push(format!(
                "{} covers {:.0} % of the window with its baseline and shows no anomaly",
                c.label,
                c.coverage.unwrap_or(0.0) * 100.0
            ));
        } else if rec.checks.is_empty() {
            rec.reasons
                .push("no observation data for the window".into());
        } else {
            for c in &rec.checks {
                let why = if !c.direct {
                    "independent source, no anomaly (interference can be local)".to_string()
                } else if !c.baseline {
                    "no baseline before the window".to_string()
                } else if c.source_type == "ais" {
                    "AIS never contradicts alone".to_string()
                } else if c.source_type == "adsb" && r.kind == Kind::Spoofing {
                    "ADS-B integrity cannot contradict a spoofing report".to_string()
                } else {
                    format!(
                        "covers {:.0} % of the window",
                        c.coverage.unwrap_or(0.0) * 100.0
                    )
                };
                rec.reasons.push(format!("{}: {why}", c.label));
            }
        }
    }
    // GNSS or AIS layer for a vessel's spoofing report.
    if r.platform == Platform::Vessel && r.kind == Kind::Spoofing {
        let rx_spoof = rec.checks.iter().any(|c| {
            c.source_type == "receiver"
                && c.direct
                && c.indicators.iter().any(|i| i.kind == Kind::Spoofing)
        });
        let rx_clean = rec.checks.iter().any(|c| {
            c.source_type == "receiver"
                && c.direct
                && c.indicators.is_empty()
                && c.coverage.is_some_and(|v| v >= p.coverage_min)
        });
        let ais_spoof = rec
            .checks
            .iter()
            .any(|c| c.source_type == "ais" && c.direct && !c.indicators.is_empty());
        rec.observed_layer = if rx_spoof {
            Some("gnss".into())
        } else if ais_spoof && rx_clean {
            rec.reasons.push(
                "the vessel's AIS track shows anomalies while its own receiver covers the window cleanly: consistent with AIS-layer manipulation, not GNSS spoofing of this vessel".into(),
            );
            Some("ais-only".into())
        } else {
            None
        };
    }
    rec
}

/// The reports, the observations and the input files (role, name, bytes).
pub type Loaded = (ReportsDoc, Observations, Vec<(String, String, Vec<u8>)>);

/// Read a request's data (paths relative to `base`), returning the observations and the
/// input files (role, name, bytes).
pub fn load(req: &Request, base: &Path) -> Result<Loaded, String> {
    let rd = |p: &str| {
        let path = base.join(p);
        std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
    };
    let mut files = vec![];
    let rb = rd(&req.reports)?;
    let doc = ReportsDoc::from_json(&String::from_utf8_lossy(&rb))
        .map_err(|e| format!("{}: {e}", req.reports))?;
    files.push(("reports".to_string(), req.reports.clone(), rb));
    let mut obs = Observations {
        aliases: req
            .aliases
            .iter()
            .map(|(a, b)| (norm_id(a), norm_id(b)))
            .collect(),
        receivers: vec![],
        adsb: vec![],
        ais: vec![],
        skipped: vec![],
    };
    for s in &req.receiver {
        let b = rd(&s.path)?;
        let (ep, mut sk) =
            load_receiver_counted(&s.format, &b).map_err(|e| format!("{}: {e}", s.path))?;
        sk.source = format!("receiver {}", s.label);
        obs.skipped.extend(sk.nonzero());
        obs.receivers.push((s.clone(), safe_name(&s.path), ep));
        files.push((format!("receiver:{}", s.label), s.path.clone(), b));
    }
    for (list, col, role, dst) in [
        (&req.adsb, "aircraft_id", "adsb", &mut obs.adsb),
        (&req.ais, "vessel_id", "ais", &mut obs.ais),
    ] {
        for s in list {
            let b = rd(&s.path)?;
            let tr = parse_tracks(&String::from_utf8_lossy(&b), col)
                .map_err(|e| format!("{}: {e}", s.path))?;
            dst.push((s.clone(), safe_name(&s.path), tr));
            files.push((role.to_string(), s.path.clone(), b));
        }
    }
    Ok((doc, obs, files))
}

/// Run a request read from `path`.
pub fn run(path: &Path) -> Result<(VerificationDoc, Copies), String> {
    let text_in = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&text_in).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let (doc, obs, files) = load(&req, base)?;
    let chosen: Vec<&Report> = if req.report_ids.is_empty() {
        doc.reports.iter().collect()
    } else {
        let mut v = vec![];
        for id in &req.report_ids {
            v.push(
                doc.reports
                    .iter()
                    .find(|r| &r.id == id)
                    .ok_or_else(|| format!("report '{id}' is not in {}", req.reports))?,
            );
        }
        v
    };
    let records = chosen
        .iter()
        .map(|r| verify_report(r, &obs, &req.params))
        .collect();
    let mut inputs: Vec<InputFile> = files
        .iter()
        .map(|(role, f, b)| InputFile {
            role: role.clone(),
            file: safe_name(f),
            sha256: sha256_hex(b),
        })
        .collect();
    inputs.push(InputFile {
        role: "request".into(),
        file: safe_name(&path.to_string_lossy()),
        sha256: sha256_hex(text_in.as_bytes()),
    });
    let mut copies: Vec<(String, Vec<u8>)> = files.into_iter().map(|(_, f, b)| (f, b)).collect();
    copies.push((path.to_string_lossy().to_string(), text_in.into_bytes()));
    Ok((
        VerificationDoc {
            format: FORMAT.into(),
            method: METHOD_ID.into(),
            evidence: EVIDENCE.into(),
            label_rules: label_rules(&req.params),
            params_sha256: params_hash(&req.params),
            params: req.params,
            inputs,
            records,
            skipped_rows: obs.skipped.clone(),
        },
        copies,
    ))
}

/// Write `verification.json`, `verification.csv` and the inputs. The caller signs it.
pub fn write_dir(
    dir: &Path,
    doc: &VerificationDoc,
    inputs: &[(String, Vec<u8>)],
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join(RESULT_FILE), json_text(doc)).map_err(|e| e.to_string())?;
    let mut csv = String::from("report_id,label,kind_reported,observed_kinds,kind_agreement,window_start_utc,window_end_utc,sources\n");
    for r in &doc.records {
        csv += &format!(
            "{},{},{},{},{},{},{},{}\n",
            r.report_id,
            r.label.as_str(),
            r.report.kind.as_str(),
            r.observed_kinds
                .iter()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join("/"),
            r.kind_agreement.map(|b| b.to_string()).unwrap_or_default(),
            r.window_start_utc.clone().unwrap_or_default(),
            r.window_end_utc.clone().unwrap_or_default(),
            r.checks.len()
        );
    }
    std::fs::write(dir.join("verification.csv"), csv).map_err(|e| e.to_string())?;
    write_inputs(dir, inputs)
}
