// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Combined eLoran and GNSS signal-file scenario with injected faults
//!
//! The eLoran crate renders a 100 kHz scene ([`kshana_eloran::Scenario`]) to SigMF with a
//! per-pulse truth file. The GNSS bridge ([`crate::eloran_gnss_sdr`]) renders GPS L1 C/A and
//! Galileo E1-B complex baseband on the same receiver trajectory and time base, with a C/N0
//! degradation timeline. Both existed as separate runs. This module is the one scenario that
//! drives both from a single request, injects faults on a shared scene clock and writes one
//! **shared truth**:
//!
//! | fault | realised as |
//! |---|---|
//! | `asf-error` | an added delay (ns) on one station's signal over a window; at the receiver this is indistinguishable from an ASF error of that size (the station's emission is unchanged in the station's own truth columns; the shared truth records it as a propagation fault) |
//! | `skywave` | a fixed skywave (delay after the groundwave, sky/ground ratio) over the whole run, through the eLoran propagation model |
//! | `gnss-outage` | no GNSS signal from the named satellites (or all) over a window |
//! | `gnss-interference` | the C/N0 drop that a stated J/S causes, from the effective-C/N0 relation `(C/N0)_eff⁻¹ = (C/N0)⁻¹ + (J/S)/(Q·R_c)` with the reference C/N0 and Q·R_c the request states; no interference waveform is synthesised (the open engine's test-condition format records interference as metadata only, and so does this) |
//!
//! Outputs, all files: `eloran.sigmf-data/-meta` and `eloran.truth.csv` (real LF samples),
//! `gnss.sigmf-data/-meta` and `gnss.truth.csv` (complex L-band baseband), `truth.json` (the
//! common start instant, both files with their sample rates, centre frequencies and SHA-256,
//! every fault with its window and realisation) and `truth-timeline.csv` (which faults are
//! active at each step). **Files only**: this module opens no device and has no transmit path.
//! A lab replaying the GNSS file does so conducted, never radiated
//! ([`crate::eloran_gnss_sdr::GNSS_OUTPUT_POLICY`]). MODELLED.

use std::path::{Path, PathBuf};

use kshana::iq::scene::CsvTruthWriter;
use kshana_eloran::chain::StationFault;
use kshana_eloran::hw::StartTime;
use kshana_eloran::io::{Format, SigmfWriter};
use kshana_eloran::prop::sky::SkyConfig;
use kshana_eloran::scene::truth_csv;
use kshana_eloran::Scenario;
use serde::{Deserialize, Serialize};

use crate::eloran_gnss::GnssOptions;
use crate::eloran_gnss_sdr::{
    self as gsdr, DegradationEvent, DegradationTimeline, Effect, SdrOptions, SigmfComplexSink,
};
use crate::forensics::inputs::{round, sha256_hex};

/// Pro feature.
pub const FEATURE: &str = "pro-hybrid-signal-sim";
/// Format of `truth.json`.
pub const FORMAT: &str = "kshana-pro-hybrid-sim-truth/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: synthetic eLoran and GNSS signals from the eLoran crate \
and the open engine's GNSS IQ scene; faults as stated in the request.";
/// File-only statement.
pub const FILES_ONLY: &str = "Files only: no device is opened and there is no transmit path. \
GNSS files are for conducted replay or software receivers, never radiated.";

/// GNSS settings of the request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GnssSection {
    #[serde(default = "d_fs")]
    pub fs_hz: f64,
    #[serde(default = "d_true")]
    pub gps: bool,
    #[serde(default)]
    pub galileo: bool,
    #[serde(default = "d_mask")]
    pub elevation_mask_deg: f64,
    #[serde(default)]
    pub cn0_dbhz: Option<f64>,
    #[serde(default = "d_true")]
    pub noise: bool,
    /// Reference C/N0 for the J/S-to-C/N0 relation (dB-Hz).
    #[serde(default = "d_ref_cn0")]
    pub reference_cn0_dbhz: f64,
    /// Q·R_c of the J/S relation (Hz): 1.023e6 for broadband noise on L1 C/A (Q = 1).
    #[serde(default = "d_qrc")]
    pub q_rc_hz: f64,
}

fn d_fs() -> f64 {
    2_000_000.0
}
fn d_true() -> bool {
    true
}
fn d_mask() -> f64 {
    5.0
}
fn d_ref_cn0() -> f64 {
    45.0
}
fn d_qrc() -> f64 {
    1.023e6
}

/// A fault.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Fault {
    AsfError {
        chain: String,
        station: String,
        from_s: f64,
        to_s: f64,
        error_ns: f64,
    },
    Skywave {
        delay_us: f64,
        ratio_db: f64,
    },
    GnssOutage {
        from_s: f64,
        to_s: f64,
        #[serde(default)]
        sats: Option<Vec<u32>>,
    },
    GnssInterference {
        from_s: f64,
        to_s: f64,
        js_db: f64,
        #[serde(default)]
        sats: Option<Vec<u32>>,
    },
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub data_source: String,
    /// The eLoran scenario (TOML).
    pub eloran_scenario: String,
    /// A RINEX 3 navigation file for the GNSS half.
    pub nav: String,
    pub gnss: GnssSection,
    #[serde(default, rename = "fault")]
    pub faults: Vec<Fault>,
    /// Step of `truth-timeline.csv` (s).
    #[serde(default = "d_step")]
    pub timeline_step_s: f64,
}

fn d_step() -> f64 {
    0.1
}

/// One output file.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OutFile {
    pub file: String,
    pub sha256: String,
}

/// A fault as realised.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FaultTruth {
    pub fault: Fault,
    pub from_s: f64,
    pub to_s: f64,
    pub layer: &'static str,
    pub realisation: String,
}

/// `truth.json`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Truth {
    pub format: &'static str,
    pub evidence: &'static str,
    pub files_only: &'static str,
    pub title: String,
    pub data_source: String,
    pub start_utc: String,
    pub duration_s: f64,
    pub gps_week: i64,
    pub start_tow_s: f64,
    pub eloran_scenario_sha256: String,
    pub eloran: Stream,
    pub gnss: Stream,
    pub gnss_satellites: Vec<String>,
    pub faults: Vec<FaultTruth>,
    pub files: Vec<OutFile>,
}

/// One stream.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Stream {
    pub data: String,
    pub sample_rate_hz: f64,
    pub center_hz: f64,
    pub samples: u64,
    /// Sample 0 is `start_utc` for both streams.
    pub first_sample_utc: String,
}

/// The C/N0 drop (dB) a J/S causes at the reference C/N0.
pub fn cn0_drop_db(js_db: f64, reference_cn0_dbhz: f64, q_rc_hz: f64) -> f64 {
    let c = 10f64.powf(reference_cn0_dbhz / 10.0);
    let js = 10f64.powf(js_db / 10.0);
    10.0 * (1.0 + c * js / q_rc_hz).log10()
}

fn window(f: &Fault, dur: f64) -> (f64, f64) {
    match f {
        Fault::AsfError { from_s, to_s, .. }
        | Fault::GnssOutage { from_s, to_s, .. }
        | Fault::GnssInterference { from_s, to_s, .. } => (*from_s, *to_s),
        Fault::Skywave { .. } => (0.0, dur),
    }
}

/// Apply the eLoran faults to a scenario and build the GNSS timeline; check every fault.
pub fn plan(
    sc: &mut Scenario,
    q: &Request,
) -> Result<(DegradationTimeline, Vec<FaultTruth>), String> {
    let dur = sc.scene.params.duration_s;
    let mut tl = DegradationTimeline::default();
    let mut truth = vec![];
    for f in &q.faults {
        let (a, b) = window(f, dur);
        if !(a.is_finite() && b.is_finite() && b > a && a >= 0.0 && b <= dur + 1e-9) {
            return Err(format!(
                "fault {f:?}: window must lie inside the run (0 to {dur} s) and end after it starts"
            ));
        }
        let (layer, realisation) = match f {
            Fault::AsfError {
                chain,
                station,
                error_ns,
                ..
            } => {
                let c = sc
                    .chains
                    .iter_mut()
                    .find(|c| &c.chain.name == chain)
                    .ok_or_else(|| format!("asf-error: no chain '{chain}'"))?;
                let s = c
                    .chain
                    .stations
                    .iter_mut()
                    .find(|s| &s.id == station)
                    .ok_or_else(|| format!("asf-error: no station '{station}' in '{chain}'"))?;
                s.faults.push(StationFault::TimingStep {
                    from_s: a,
                    to_s: b,
                    error_ns: *error_ns,
                });
                (
                    "eloran",
                    format!("{error_ns} ns added delay on {chain}/{station}: an ASF error of that size at the receiver"),
                )
            }
            Fault::Skywave { delay_us, ratio_db } => {
                if sc.propagation.sky.is_some() {
                    return Err("skywave: the eLoran scenario already sets a skywave model".into());
                }
                sc.propagation.sky = Some(SkyConfig::Fixed {
                    delay_us: *delay_us,
                    ratio_db: *ratio_db,
                    phase_deg: 0.0,
                });
                (
                    "eloran",
                    format!("fixed skywave {delay_us} µs after the groundwave at {ratio_db} dB sky/ground, whole run"),
                )
            }
            Fault::GnssOutage { sats, .. } => {
                tl.events.push(DegradationEvent {
                    sats: sats.clone(),
                    start_s: a,
                    end_s: b,
                    effect: Effect::Outage,
                });
                ("gnss", "no signal from the affected satellites".to_string())
            }
            Fault::GnssInterference { js_db, sats, .. } => {
                let d = cn0_drop_db(*js_db, q.gnss.reference_cn0_dbhz, q.gnss.q_rc_hz);
                tl.events.push(DegradationEvent {
                    sats: sats.clone(),
                    start_s: a,
                    end_s: b,
                    effect: Effect::Cn0Drop { db: d },
                });
                (
                    "gnss",
                    format!(
                        "J/S {js_db} dB → C/N0 drop {:.2} dB at {} dB-Hz, Q·Rc {} Hz (no interference waveform synthesised)",
                        d, q.gnss.reference_cn0_dbhz, q.gnss.q_rc_hz
                    ),
                )
            }
        };
        truth.push(FaultTruth {
            fault: f.clone(),
            from_s: a,
            to_s: b,
            layer,
            realisation,
        });
    }
    tl.validate()?;
    Ok((tl, truth))
}

fn read(base: &Path, p: &str) -> Result<String, String> {
    crate::timing::grid::read_text(base, p)
}

fn file_sha(p: &Path) -> Result<String, String> {
    std::fs::read(p)
        .map(|b| sha256_hex(&b))
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// Run the scenario into `out`.
pub fn run(q: &Request, base: &Path, out: &Path) -> Result<Truth, String> {
    if q.data_source.trim().is_empty() {
        return Err("data_source must be stated".into());
    }
    if q.timeline_step_s.is_nan() || q.timeline_step_s <= 0.0 {
        return Err("timeline_step_s must be positive".into());
    }
    let mut sc = Scenario::from_toml(&read(base, &q.eloran_scenario)?)?;
    let nav = read(base, &q.nav)?;
    let (tl, faults) = plan(&mut sc, q)?;
    let dur = sc.scene.params.duration_s;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;

    // eLoran half.
    let fmt = Format::parse(&sc.file.format)?;
    let mut scene = sc.build()?;
    let eb = out.join("eloran");
    let mut w = SigmfWriter::create(
        &eb,
        fmt,
        sc.outputs.len(),
        sc.scene.params.fs_hz,
        serde_json::json!({
            "scenario_sha256": sc.hash(),
            "start_utc": sc.scene.start_utc,
            "kshana:hybrid_sim": FORMAT,
        }),
    )?;
    let mut etruth = Vec::new();
    let stats = scene.run(&mut w, &mut etruth)?;
    drop(w);
    std::fs::write(out.join("eloran.truth.csv"), truth_csv(&etruth)).map_err(|e| e.to_string())?;

    // GNSS half (file sink, unpaced).
    let opts = SdrOptions {
        gnss: GnssOptions {
            fs_hz: q.gnss.fs_hz,
            gps: q.gnss.gps,
            galileo: q.gnss.galileo,
            elevation_mask_deg: q.gnss.elevation_mask_deg,
            cn0_dbhz: q.gnss.cn0_dbhz,
            duration_s: Some(dur),
            threads: 1,
        },
        noise: q.gnss.noise,
        only: None,
        chunk_samples: 1 << 16,
        start: StartTime::Now,
        timeline: (!tl.events.is_empty()).then_some(tl),
        truth_interval_s: q.timeline_step_s,
    };
    let gb = out.join("gnss");
    let mut sink = SigmfComplexSink::new(&gb, false);
    sink.extra_meta
        .insert("kshana:hybrid_sim".into(), FORMAT.into());
    let tf = std::fs::File::create(out.join("gnss.truth.csv")).map_err(|e| e.to_string())?;
    let mut tsink = CsvTruthWriter::new(std::io::BufWriter::new(tf));
    let rep = gsdr::stream(&sc, &nav, &opts, &mut sink, &mut tsink)?;

    // Shared truth.
    let mut files = vec![];
    for f in [
        "eloran.sigmf-data",
        "eloran.sigmf-meta",
        "eloran.truth.csv",
        "gnss.sigmf-data",
        "gnss.sigmf-meta",
        "gnss.truth.csv",
    ] {
        files.push(OutFile {
            file: f.into(),
            sha256: file_sha(&out.join(f))?,
        });
    }
    let mut csv = String::from("t_s,eloran_faults,gnss_state,gnss_cn0_offset_db\n");
    let n = (dur / q.timeline_step_s).floor() as usize;
    let gtl = opts.timeline.clone().unwrap_or_default();
    for k in 0..=n {
        let t = k as f64 * q.timeline_step_s;
        let el: Vec<String> = faults
            .iter()
            .filter(|f| f.layer == "eloran" && t >= f.from_s && t < f.to_s)
            .map(|f| match &f.fault {
                Fault::AsfError { chain, station, .. } => format!("asf-error:{chain}/{station}"),
                _ => "skywave".to_string(),
            })
            .collect();
        // Whole-constellation state (satellite-specific events are in gnss.truth.csv).
        let (state, off) = match gtl.offset_db(u32::MAX, t) {
            None => ("outage", String::new()),
            Some(o) if o != 0.0 => ("interference", format!("{:.2}", o)),
            Some(_) => ("nominal", "0".into()),
        };
        csv += &format!("{:.3},{},{state},{off}\n", round(t, 3), el.join(";"));
    }
    std::fs::write(out.join("truth-timeline.csv"), &csv).map_err(|e| e.to_string())?;
    let truth = Truth {
        format: FORMAT,
        evidence: EVIDENCE,
        files_only: FILES_ONLY,
        title: q.title.clone(),
        data_source: q.data_source.clone(),
        start_utc: sc.scene.start_utc.clone(),
        duration_s: dur,
        gps_week: rep.plan.gps_week,
        start_tow_s: rep.plan.start_tow_s,
        eloran_scenario_sha256: sc.hash(),
        eloran: Stream {
            data: "eloran.sigmf-data".into(),
            sample_rate_hz: sc.scene.params.fs_hz,
            center_hz: 100_000.0,
            samples: stats.samples_per_channel,
            first_sample_utc: sc.scene.start_utc.clone(),
        },
        gnss: Stream {
            data: "gnss.sigmf-data".into(),
            sample_rate_hz: q.gnss.fs_hz,
            center_hz: crate::eloran_gnss::L1_HZ,
            samples: rep.samples,
            first_sample_utc: sc.scene.start_utc.clone(),
        },
        gnss_satellites: rep.satellites,
        faults,
        files,
    };
    std::fs::write(
        out.join("truth.json"),
        serde_json::to_string_pretty(&truth).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(truth)
}

/// Path helper for the CLI.
pub fn base_of(p: &str) -> PathBuf {
    Path::new(p)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// A SYNTHETIC request over the bundled RINEX fixture's epoch: one eLoran chain of two stations,
/// an ASF error on the secondary from 0.2 to 0.4 s, a fixed skywave, a GNSS outage from 0.1 to
/// 0.2 s and interference (J/S 30 dB) from 0.3 to 0.45 s. Returns (scenario TOML, request TOML).
pub fn synth() -> (String, String) {
    let scenario = r#"[scene]
start_utc = "2024-09-10T01:00:00Z"
duration_s = 0.5
fs_hz = 400000
seed = 7

[receiver]
type = "static"
lat_deg = 69.27
lon_deg = 15.98

[[outputs]]
type = "e_field"

[[chains]]
name = "SYN"
gri_us = 99300

[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 70.0, lon_deg = 20.0 }

[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 67.0, lon_deg = 14.0 }
emission_delay_us = 13000.0
"#;
    let request = r#"title = "SYNTHETIC hybrid eLoran and GNSS scenario"
data_source = "SYNTHETIC (kshana-pro hybrid-sim synth)"
eloran_scenario = "scenario.toml"
nav = "nav.rnx"
timeline_step_s = 0.05

[gnss]
fs_hz = 1000000
gps = true
galileo = false
cn0_dbhz = 45.0

[[fault]]
kind = "asf-error"
chain = "SYN"
station = "X"
from_s = 0.2
to_s = 0.4
error_ns = 500.0

[[fault]]
kind = "skywave"
delay_us = 35.0
ratio_db = -6.0

[[fault]]
kind = "gnss-outage"
from_s = 0.1
to_s = 0.2

[[fault]]
kind = "gnss-interference"
from_s = 0.3
to_s = 0.45
js_db = 30.0
"#;
    (scenario.into(), request.into())
}
