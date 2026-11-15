// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Running a library scenario's analogue and exporting it (roadmap 0.42, E2).
//!
//! The analogue is generated with the open engine's NMEA generator (`kshana::nmea_synth`),
//! scored with the open `receiver-trust` monitors, and exported through the open I4 writers
//! (`kshana::interop::testbench`). Nothing here models a radio signal: the analogue is a text
//! stream of what a receiver would report, and the exports are motion, events and text.

use std::path::Path;

use kshana::api::KshanaError;
use kshana::frames::{geodetic_to_ecef, Geodetic};
use kshana::interop::testbench::{EventKind as TbKind, MotionEvent, MotionSample, Trajectory};
use kshana::interop::UtcEpoch;
use kshana::nmea_synth::log::InstructorLog;
use kshana::nmea_synth::Generated;
use kshana::receiver_trust::scenario::{
    run_receiver_trust_bytes, ReceiverTrustResult, ReceiverTrustScenario,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{ThreatScenario, EDITION, LABEL};

/// Calibration the analogue's monitors use before the first event, s.
pub const ANALOGUE_CALIBRATION_S: f64 = 120.0;

/// One analogue event and how the open monitors scored it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalogueEvent {
    pub label: String,
    /// `jamming`, `spoofing` or `other`, as receiver-trust scores it.
    pub kind: String,
    /// The generator's event kind (`jamming`, `drag-off`, `time-spoof`, `replay-delay`).
    pub injected: String,
    pub onset_s: f64,
    pub end_s: f64,
    pub outcome: String,
    pub latency_s: Option<f64>,
    pub first_monitors: Vec<String>,
}

/// The analogue run of one scenario.
#[derive(Debug, Clone, Serialize)]
pub struct AnalogueRun {
    pub nmea_sha256: String,
    pub epochs: usize,
    pub duration_s: f64,
    /// `vessel` or `static`.
    pub platform: &'static str,
    pub events: Vec<AnalogueEvent>,
    pub events_detected: usize,
    pub events_evaluable: usize,
    pub verdict: String,
}

/// A generated analogue: the stream, the instructor log, the trust scenario and its result.
pub struct Analogue {
    pub generated: Generated,
    pub trust_toml: String,
    pub trust: ReceiverTrustScenario,
    pub result: ReceiverTrustResult,
}

impl Analogue {
    pub fn nmea(&self) -> String {
        self.generated.nmea_text()
    }
    pub fn log(&self) -> &InstructorLog {
        &self.generated.log
    }
}

fn monitor_name(m: &kshana::receiver_trust::monitors::Monitor) -> String {
    serde_json::to_value(m)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The `receiver-trust` scenario that scores the analogue stream at `log_path`.
pub fn trust_toml(s: &ThreatScenario, log_path: &str) -> Option<String> {
    let a = s.analogue.as_ref()?;
    let mut t = format!(
        "# receiver-trust scenario for the analogue of threat scenario '{}' (Kshana Pro threat library {EDITION}).\n\
         # MODELLED analogue, not a recording. Run with the open engine: kshana run <this file>\n\
         kind = \"receiver-trust\"\nname = {:?}\n[log]\nformat = \"nmea\"\npath = {:?}\n\
         [monitors]\ncalibration_s = {ANALOGUE_CALIBRATION_S:.1}\n",
        s.id, s.id, log_path
    );
    if !s.analogue_is_static() {
        t.push_str(&format!(
            "[platform]\nkind = \"vessel\"\nmax_speed_kn = 30.0\nantenna_height_m = {:.1}\nheading_sensor = true\n",
            a.receiver.antenna_height_m
        ));
    }
    for e in &a.events {
        t.push_str(&format!(
            "[[events]]\nlabel = {:?}\nkind = \"{}\"\nonset_s = {:.1}\nend_s = {:.1}\n",
            e.label.clone().unwrap_or_else(|| e.kind.name().to_string()),
            ThreatScenario::event_kind(e.kind),
            e.start_s,
            e.start_s + e.duration_s + e.recovery_s
        ));
    }
    Some(t)
}

/// Generate the analogue and score it with the open monitors (`None` for a timing-only
/// scenario).
pub fn analogue(s: &ThreatScenario) -> Result<Option<Analogue>, KshanaError> {
    let Some(a) = &s.analogue else {
        return Ok(None);
    };
    let bad = |m: String| KshanaError::InvalidInput(format!("threat scenario '{}': {m}", s.id));
    let mut a = a.clone();
    // One instructor-log row per epoch: the truth the exports and the scorecards need.
    a.output.log_interval_s = 1.0 / a.scenario.rate_hz.max(1.0);
    let generated = kshana::nmea_synth::generate(&a).map_err(bad)?;
    let trust_toml = trust_toml(s, "receiver.nmea").unwrap_or_default();
    let trust: ReceiverTrustScenario =
        toml::from_str(&trust_toml).map_err(|e| bad(format!("trust scenario: {e}")))?;
    let result = run_receiver_trust_bytes(&trust, generated.nmea_text().as_bytes(), None)
        .map_err(|e| bad(format!("receiver-trust: {e}")))?;
    Ok(Some(Analogue {
        generated,
        trust_toml,
        trust,
        result,
    }))
}

/// The summary of an analogue run.
pub fn summarise(s: &ThreatScenario, an: &Analogue) -> AnalogueRun {
    let a = s.analogue.as_ref().expect("an analogue run has an analogue");
    let nmea = an.nmea();
    let events = an
        .result
        .events
        .iter()
        .zip(&a.events)
        .map(|(sc, e)| AnalogueEvent {
            label: sc.label.clone(),
            kind: ThreatScenario::event_kind(e.kind).into(),
            injected: e.kind.name().into(),
            onset_s: sc.onset_s,
            end_s: sc.window_end_s,
            outcome: sc.outcome.clone(),
            latency_s: sc.latency_s.map(|l| (l * 10.0).round() / 10.0),
            first_monitors: sc.first_monitors.iter().map(monitor_name).collect(),
        })
        .collect();
    AnalogueRun {
        nmea_sha256: hex::encode(Sha256::digest(nmea.as_bytes())),
        epochs: an.result.log.epochs,
        duration_s: an.result.log.duration_s,
        platform: if s.analogue_is_static() {
            "static"
        } else {
            "vessel"
        },
        events,
        events_detected: an.result.events_detected,
        events_evaluable: an.result.events_evaluable,
        verdict: an.result.verdict.clone(),
    }
}

/// [`analogue`] and [`summarise`] in one call.
pub fn run_analogue(s: &ThreatScenario) -> Result<Option<AnalogueRun>, KshanaError> {
    Ok(analogue(s)?.map(|an| summarise(s, &an)))
}

/// The events of a run as CSV.
pub fn events_csv(r: &AnalogueRun) -> String {
    let mut out = String::from("label,kind,injected,onset_s,end_s,outcome,latency_s,first_monitors\n");
    for e in &r.events {
        out.push_str(&format!(
            "{},{},{},{:.1},{:.1},{},{},{}\n",
            e.label.replace(',', ";"),
            e.kind,
            e.injected,
            e.onset_s,
            e.end_s,
            e.outcome,
            e.latency_s.map(|l| format!("{l:.1}")).unwrap_or_default(),
            e.first_monitors.join(" ")
        ));
    }
    out
}

fn ned_to_ecef(lat: f64, lon: f64, v: [f64; 3]) -> [f64; 3] {
    let (sl, cl) = (lat.sin(), lat.cos());
    let (so, co) = (lon.sin(), lon.cos());
    let [n, e, d] = v;
    [
        -sl * co * n - so * e - cl * co * d,
        -sl * so * n + co * e - cl * so * d,
        cl * n - sl * d,
    ]
}

/// The analogue's true motion and its events as an I4 test-bench trajectory.
pub fn trajectory(s: &ThreatScenario, an: &Analogue) -> Result<Trajectory, KshanaError> {
    let a = s.analogue.as_ref().expect("an analogue run has an analogue");
    let epoch = UtcEpoch::parse_iso(&a.scenario.start_utc)
        .map_err(|e| KshanaError::InvalidInput(format!("start_utc: {e}")))?;
    let h = a.receiver.antenna_height_m;
    let kn = 1852.0 / 3600.0;
    let samples = an
        .log()
        .track
        .iter()
        .map(|r| {
            let g = Geodetic {
                lat_rad: r.true_lat_deg.to_radians(),
                lon_rad: r.true_lon_deg.to_radians(),
                alt_m: h,
            };
            let v = r.true_sog_kn * kn;
            let c = r.true_cog_deg.to_radians();
            let v_ned = [v * c.cos(), v * c.sin(), 0.0];
            MotionSample {
                t_s: r.t_s,
                lat_deg: r.true_lat_deg,
                lon_deg: r.true_lon_deg,
                h_m: h,
                ecef_m: geodetic_to_ecef(g),
                v_ecef_m_s: ned_to_ecef(g.lat_rad, g.lon_rad, v_ned),
                heading_deg: r.true_heading_deg.rem_euclid(360.0),
                pitch_deg: 0.0,
                roll_deg: 0.0,
            }
        })
        .collect();
    let events = a
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| MotionEvent {
            label: format!("event-{}", i + 1),
            kind: match ThreatScenario::event_kind(e.kind) {
                "jamming" => TbKind::Jamming,
                "spoofing" => TbKind::Spoofing,
                _ => TbKind::Other,
            },
            onset_s: e.start_s,
            end_s: e.start_s + e.duration_s + e.recovery_s,
            description: kshana::nmea_synth::log::describe(e, None),
        })
        .collect();
    Ok(Trajectory {
        kind: "pro-threat-scenario".into(),
        scenario_hash: hex::encode(Sha256::digest(super::source_of(&s.id).unwrap_or("").as_bytes()))
            [..12]
            .to_string(),
        epoch,
        note: format!(
            "the true motion of the MODELLED analogue of threat scenario '{}' (Kshana Pro threat \
             library {EDITION}): the open NMEA generator's vessel track, height the receiver \
             antenna height, pitch and roll zero; events are the analogue's scripted intervals, \
             labelled time intervals only, not a signal; no source data",
            s.id
        ),
        samples,
        events,
    })
}

/// The timing profile as CSV (`u_s,offset_s`), one row per `step_s`.
pub fn timing_csv(p: &super::TimingProfile, step_s: f64) -> String {
    let mut out = String::from("# Kshana Pro threat library timing profile: counterfeit time offset a disciplined clock follows, u seconds after onset. MODELLED.\nu_s,offset_s\n");
    let n = (p.duration_s / step_s).ceil().min(200_000.0) as usize;
    for k in 0..=n {
        let u = (k as f64 * step_s).min(p.duration_s);
        out.push_str(&format!("{u:.1},{:.6e}\n", p.offset(u)));
    }
    out
}

/// The provenance and licence notes of a scenario as plain text.
pub fn provenance_text(s: &ThreatScenario) -> String {
    let mut t = format!(
        "{}\n{}\n\nThreat scenario {} (version {}), Kshana Pro threat library {EDITION}\n\
         Family: {}   Period: {}\n\n{}\n\n{LABEL}\n\nSOURCES ({:?})\n",
        s.title,
        "=".repeat(s.title.chars().count().min(78)),
        s.id,
        s.version,
        s.family.as_str(),
        s.period,
        s.summary.trim(),
        s.provenance.class
    );
    for (i, src) in s.provenance.sources.iter().enumerate() {
        t.push_str(&format!(
            "[{}] {}. {}, {}.{}{}\n    {}\n",
            i + 1,
            src.title,
            src.publisher,
            src.date,
            src.url
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default(),
            src.citation
                .as_deref()
                .map(|c| format!(" {c}"))
                .unwrap_or_default(),
            src.says.trim()
        ));
    }
    t.push_str(&format!(
        "\nPARAMETER BASIS\n{}\n",
        s.provenance.parameter_basis.trim()
    ));
    if let Some(v) = &s.provenance.verify_before_use {
        t.push_str(&format!("\nVERIFY BEFORE USE\n{}\n", v.trim()));
    }
    t.push_str(&format!(
        "\nLICENCE NOTES\nSource material: {}\nSource data included in this library: no.\nThis scenario and its analogue: {}\n",
        s.licence.source_terms.trim(),
        s.licence.library_terms.trim()
    ));
    if let Some(r) = &s.replay {
        t.push_str(&format!(
            "\nREPLAY WITH THE ORIGINAL DATA (fetched by you, never included)\nDataset: {}\nWhere: {}\nFiles: {}\nFormat: {}\nHow: {}\nEvent times: {}\n",
            r.dataset,
            r.fetch,
            if r.files.is_empty() {
                "as the publisher lists them".to_string()
            } else {
                r.files.join(", ")
            },
            r.format,
            r.how.trim(),
            r.events_from.trim()
        ));
    }
    t
}

/// Write the export of `s` into the empty or new directory `out`: the scenario file, the
/// provenance and licence notes, the analogue stream with its instructor log and
/// receiver-trust scenario, the I4 test-bench files and the timing profile. Returns the
/// written paths, relative to `out`.
pub fn write(s: &ThreatScenario, out: &Path) -> Result<Vec<String>, KshanaError> {
    let io = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", out.display()));
    if out.exists() && std::fs::read_dir(out).map_err(io)?.next().is_some() {
        return Err(KshanaError::InvalidInput(format!(
            "{}: not empty; an export goes into a new or empty directory",
            out.display()
        )));
    }
    std::fs::create_dir_all(out).map_err(io)?;
    let mut written = Vec::new();
    let mut put = |rel: &str, bytes: &[u8]| -> Result<(), KshanaError> {
        let p = out.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(io)?;
        }
        std::fs::write(&p, bytes).map_err(io)?;
        written.push(rel.to_string());
        Ok(())
    };
    let src = super::source_of(&s.id).map(str::to_string).unwrap_or_else(|| {
        toml::to_string(s).unwrap_or_default()
    });
    put("scenario.toml", src.as_bytes())?;
    put("PROVENANCE.txt", provenance_text(s).as_bytes())?;
    if let Some(an) = analogue(s)? {
        put("analogue/receiver.nmea", an.nmea().as_bytes())?;
        put("analogue/instructor.json", an.log().to_json().as_bytes())?;
        put("analogue/receiver-trust.toml", an.trust_toml.as_bytes())?;
        let run = summarise(s, &an);
        put(
            "analogue/monitors.json",
            (serde_json::to_string_pretty(&run).map_err(|e| KshanaError::InvalidInput(e.to_string()))? + "\n")
                .as_bytes(),
        )?;
        let t = trajectory(s, &an)?;
        let (files, notes) = kshana::interop::testbench::files_and_notes_of(&t);
        for f in files {
            put(&format!("testbench/{}{}", s.id, f.suffix), &f.bytes)?;
        }
        if !notes.is_empty() {
            put("testbench/NOTES.txt", (notes.join("\n") + "\n").as_bytes())?;
        }
    }
    if let Some(p) = &s.timing {
        put("timing/profile.csv", timing_csv(p, 1.0).as_bytes())?;
    }
    let readme = format!(
        "Kshana Pro threat library {EDITION}: export of '{}'.\n\n{LABEL}\n\n\
         scenario.toml            the library scenario file\n\
         PROVENANCE.txt           sources, parameter basis, licence notes, how to fetch the original data\n\
         analogue/receiver.nmea   the analogue receiver's NMEA 0183 stream (synthetic; never feed it to live navigation)\n\
         analogue/instructor.json what was injected when, and the true track\n\
         analogue/receiver-trust.toml  score the stream with the open engine: kshana run analogue/receiver-trust.toml\n\
         testbench/               the open engine's test-bench files (motion, waypoints, truth NMEA, events) for a laboratory simulator\n\
         timing/profile.csv       the counterfeit time offset a disciplined clock follows (timing scenarios)\n\n\
         No TEXBAT, JammerTest or other source data is included. Fetch it from the publisher as PROVENANCE.txt says.\n",
        s.id
    );
    put("README.txt", readme.as_bytes())?;
    Ok(written)
}
