// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro eloran ...`: the eLoran/GNSS signal simulator (crate `kshana-eloran`).
//!
//! * `run` (feature `pro-eloran`, Enterprise): run a scenario, write the multi-channel
//!   SigMF recording, the truth table and the run record (scenario SHA-256, statistics).
//! * `validate` and `example` need no licence.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use kshana_eloran::io::{Format, SigmfWriter};
use kshana_eloran::scene::{truth_csv, NullSink, SampleSink};
use kshana_eloran::Scenario;

use super::scorecard_cmd::entitlement;
use super::{flag, now_unix};

/// Licence feature of the simulator.
pub const FEATURE: &str = "pro-eloran";

pub const USAGE: &str = "usage: kshana-pro eloran <command>\n\
    run <scenario.toml> --out <base> [--format ri16|rf32|none] [--license <token>] [--dev]\n\
        Generate the eLoran signal of a scenario: <base>.sigmf-data/.sigmf-meta (one channel per\n\
        [[outputs]] antenna element), <base>.truth.csv (time of arrival, delay parts, ECD, field,\n\
        SNR, time-reference offset per station per group) and <base>.run.json (Enterprise)\n\
    gnss <scenario.toml> --nav <rinex-3 nav> --out <base> [--fs <hz>] [--no-galileo] [--no-gps] [--cn0 <dB-Hz>] [--threads <n>] [--license <token>] [--dev]\n\
        Generate the GNSS half of a hybrid scene in software: GPS L1 C/A and Galileo E1-B complex baseband\n\
        (<base>.gnss.sigmf-data/-meta, ci16_le at 1575.42 MHz) and per-satellite truth, on the scenario's\n\
        trajectory and start instant (Enterprise)\n\
    validate <scenario.toml>\n\
        Check a scenario and print its SHA-256 (no licence needed)\n\
    example\n\
        Print an example scenario (two chains, two UTC(k), crossed loops, moving receiver)";

const EXAMPLE: &str = include_str!("../../crates/kshana-eloran/scenarios/synthetic-two-chain.toml");

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("run") => run_cmd(args),
        Some("validate") => validate_cmd(args),
        Some("gnss") => gnss_cmd(args),
        Some("example") => {
            print!("{EXAMPLE}");
            ExitCode::SUCCESS
        }
        _ => fail(USAGE),
    }
}

fn load(path: &str) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Scenario::from_toml(&text).map_err(|e| format!("{path}: {e}"))
}

fn validate_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1) else {
        return fail(USAGE);
    };
    match load(path).and_then(|s| s.build().map(|_| s)) {
        Ok(s) => {
            println!("ok {} sha256={}", path, s.hash());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn run_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, FEATURE, now) {
        return fail(e);
    }
    let sc = match load(path) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let fmt_s = flag(args, "--format").unwrap_or_else(|| sc.file.format.clone());
    let mut scene = match sc.build() {
        Ok(s) => s,
        Err(e) => return fail(format!("{path}: {e}")),
    };
    let base = PathBuf::from(&out);
    if let Some(dir) = base.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return fail(format!("{}: {e}", dir.display()));
        }
    }
    let hash = sc.hash();
    let extra = serde_json::json!({
        "scenario_sha256": hash,
        "output_scale_uv_m": sc.scene.params.output_scale_uv_m,
        "outputs": sc.outputs,
        "start_utc": sc.scene.start_utc,
        "engine": format!("kshana-eloran {}", env!("CARGO_PKG_VERSION")),
    });
    let mut sink: Box<dyn SampleSink> = if fmt_s == "none" {
        Box::new(NullSink::default())
    } else {
        let fmt = match Format::parse(&fmt_s) {
            Ok(f) => f,
            Err(e) => return fail(e),
        };
        match SigmfWriter::create(&base, fmt, sc.outputs.len(), sc.scene.params.fs_hz, extra.clone()) {
            Ok(w) => Box::new(w),
            Err(e) => return fail(e),
        }
    };
    let mut truth = Vec::new();
    let stats = match scene.run(sink.as_mut(), &mut truth) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let truth_path = with_suffix(&base, "truth.csv");
    if let Err(e) = std::fs::write(&truth_path, truth_csv(&truth)) {
        return fail(format!("{}: {e}", truth_path.display()));
    }
    let record = serde_json::json!({
        "format": "kshana-eloran-run",
        "version": 1,
        "scenario": path,
        "scenario_sha256": hash,
        "licensee": ent.licensee,
        "stats": stats,
        "evidence_label": "MODELLED",
        "files": {
            "samples": if fmt_s == "none" { serde_json::Value::Null } else { serde_json::json!(base.with_extension("sigmf-data")) },
            "truth": truth_path,
        },
    });
    let run_path = with_suffix(&base, "run.json");
    if let Err(e) = std::fs::write(&run_path, serde_json::to_string_pretty(&record).unwrap()) {
        return fail(format!("{}: {e}", run_path.display()));
    }
    println!(
        "eloran: {} s, {} channel(s), {} pulses, {} truth rows, peak {:.3} FS, clipped {}, {:.1}x real time; sha256={}",
        sc.scene.params.duration_s,
        stats.channels,
        stats.pulses,
        stats.truth_records,
        stats.peak_abs,
        stats.clipped_samples,
        stats.realtime_factor,
        &hash[..16]
    );
    ExitCode::SUCCESS
}

fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut s = base.as_os_str().to_os_string();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

fn gnss_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let (Some(nav), Some(out)) = (flag(args, "--nav"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, FEATURE, now) {
        return fail(e);
    }
    let sc = match load(path).and_then(|s| s.build().map(|_| s)) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let nav_text = match std::fs::read_to_string(&nav) {
        Ok(t) => t,
        Err(e) => return fail(format!("{nav}: {e}")),
    };
    let mut opts = crate::eloran_gnss::GnssOptions::default();
    if let Some(v) = flag(args, "--fs") {
        match v.parse() {
            Ok(x) => opts.fs_hz = x,
            Err(_) => return fail("--fs needs a number"),
        }
    }
    if let Some(v) = flag(args, "--cn0") {
        match v.parse() {
            Ok(x) => opts.cn0_dbhz = Some(x),
            Err(_) => return fail("--cn0 needs a number"),
        }
    }
    if let Some(v) = flag(args, "--threads") {
        match v.parse() {
            Ok(x) => opts.threads = x,
            Err(_) => return fail("--threads needs a whole number"),
        }
    }
    opts.gps = !args.iter().any(|a| a == "--no-gps");
    opts.galileo = !args.iter().any(|a| a == "--no-galileo");
    let base = PathBuf::from(&out);
    if let Some(dir) = base.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return fail(format!("{}: {e}", dir.display()));
        }
    }
    match crate::eloran_gnss::generate(&sc, &nav_text, &opts, &base) {
        Ok(r) => {
            println!(
                "gnss: GPS week {} TOW {:.3} s, {} satellite(s), {} samples, {} truth rows -> {}",
                r.gps_week,
                r.start_tow_s,
                r.satellites.len(),
                r.samples,
                r.truth_records,
                r.data.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
