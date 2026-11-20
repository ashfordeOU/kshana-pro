// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro simlab <command>`: simulation fidelity and lab tooling (`crate::simlab`,
//! research backlog E32). Producing commands write a signed result directory.
//!
//! Licence: `chain` and `sync` need `pro-simlab-replay`, `selftest` `pro-simlab-selftest`,
//! `report` and `compare` `pro-simlab-report` (all Pro). `tco`, `synth` and `check` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::aviation::{write_result, Inputs};
use crate::simlab::{self, chain, labreport, selftest, sync, tco};

use super::evidence_cmd::{producing, sign};
use super::forensics_cmd::keys;
use super::{flag, has_flag};

pub const USAGE: &str = "usage: kshana-pro simlab <command>\n\
    chain <chain.toml> --out <dir> --signer <name> --key-file <file> [--register <register.jsonl>] [--utc <iso>] [--license <token>] [--dev]\n\
        Recorder-chain register: cascade NF, gain, bandwidth, group delay, bit depth, findings, change since the last registration, capture.toml for testkit fidelity; signed (Pro)\n\
    sync <replay-set.toml> --out <dir> --signer <name> --key-file <file> [...]\n\
        Synchronous multi-sensor replay check: per-stream offset, drift, jitter, gaps and residual at shared events, and the replay manifest; signed (Pro)\n\
    selftest <selftest.toml> --out <dir> --signer <name> --key-file <file> [...]\n\
        Simulator self-test or replay-rig check from reference receivers, with the deviation attributed to the source or a receiver; signed (Pro)\n\
    report <lab-results.toml> --out <dir> --signer <name> --key-file <file> [...]\n\
        One lab's results in the common test-report format, with the EN 16803 topic index; signed (Pro)\n\
    compare <report dir> <report dir>... --out <dir> --signer <name> --key-file <file> [--trust <key hex>[,...]] [...]\n\
        Results of two or more labs side by side with the En number; signed (Pro)\n\
    tco <tco.toml> [--json] [--markdown <file>]\n\
        Simulator total cost of ownership over your own quotes (free)\n\
    synth <dir>\n\
        Write synthetic inputs for every command (free)\n\
    check <dir> --trust <signer public key hex>[,...]\n\
        Check a signed result directory, offline (free)\n\
  MODELLED. Reads and writes files only; drives no instrument and emits no signal.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

const WITH_VALUE: [&str; 9] = [
    "--out",
    "--signer",
    "--key-file",
    "--utc",
    "--license",
    "--register",
    "--trust",
    "--markdown",
    "--json-out",
];

fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if WITH_VALUE.contains(&args[i].as_str()) {
            i += 2;
            continue;
        }
        if !args[i].starts_with("--") {
            out.push(args[i].clone());
        }
        i += 1;
    }
    out
}

fn produce(args: &[String], gate: &str) -> Result<String, String> {
    let pos = positional(args);
    let file = pos.first().ok_or(USAGE)?;
    let (out, seed, signer, utc) = producing(args, gate)?;
    let dir = Path::new(&out);
    if dir.join(crate::signed_dossier::SIGNATURE_FILE).exists() || dir.join("summary.txt").exists()
    {
        return Err(format!("{out} already holds a result"));
    }
    let headline = match args.first().map(String::as_str) {
        Some("chain") => {
            let (src, inputs) = simlab::read_request(Path::new(file), "recorder-chain")?;
            let c = chain::Chain::parse(&src)?;
            let mut rep = chain::assess(&c, &crate::forensics::inputs::sha256_hex(src.as_bytes()));
            if let Some(reg) = flag(args, "--register") {
                let reg = kshana_eloran::calreg::Register::open(Path::new(&reg));
                chain::register(&mut rep, &reg, &c.operator, c.interval_days)?;
            }
            let h = format!(
                "{}: NF {} dB, gain {} dB, {:.3} MHz, {} bit, AGC {}; {} finding(s) not ok; {} change(s) since the last registration",
                rep.chain_id,
                rep.cascade_nf_db,
                rep.cascade_gain_db,
                rep.bandwidth_hz / 1e6,
                rep.bit_depth,
                rep.agc,
                rep.findings.iter().filter(|f| f.severity != "ok").count(),
                rep.changes_since_previous.len()
            );
            write_result(
                dir,
                "chain.json",
                &rep,
                &simlab::summary("chain", &h),
                &[("capture.toml", rep.capture_toml.clone())],
                &inputs,
            )?;
            h
        }
        Some("sync") => {
            let l = sync::load(Path::new(file))?;
            let rep = sync::check(&l);
            let h = format!(
                "{}: {}; window {}; sensors synchronised: {}",
                rep.id,
                rep.verdict,
                rep.window_s
                    .map(|w| format!("{:.3}..{:.3} s", w[0], w[1]))
                    .unwrap_or_else(|| "none".into()),
                rep.sensors_synchronised
            );
            write_result(
                dir,
                "sync.json",
                &rep,
                &simlab::summary("sync", &h),
                &[],
                &l.inputs,
            )?;
            h
        }
        Some("selftest") => {
            let l = selftest::load(Path::new(file))?;
            let rep = selftest::assess(&l);
            let h = format!("{}: {}", rep.id, rep.verdict);
            write_result(
                dir,
                "selftest.json",
                &rep,
                &simlab::summary("selftest", &h),
                &[],
                &l.inputs,
            )?;
            h
        }
        Some("report") => {
            let l = labreport::load(Path::new(file))?;
            let rep = labreport::normalise(&l)?;
            let h = format!(
                "{} {}: {} result(s), {} unmapped column(s), core metrics missing: {}",
                rep.lab,
                rep.report_id,
                rep.results.len(),
                rep.unmapped_columns.len(),
                if rep.missing_core_metrics.is_empty() {
                    "none".into()
                } else {
                    rep.missing_core_metrics.join(", ")
                }
            );
            write_result(
                dir,
                "lab-report.json",
                &rep,
                &simlab::summary("report", &h),
                &[("lab-report.md", labreport::markdown(&rep))],
                &l.inputs,
            )?;
            h
        }
        _ => {
            let trust = match flag(args, "--trust") {
                Some(_) => Some(keys(args, "--trust")?),
                None => None,
            };
            let mut reps = Vec::new();
            let mut inputs = Inputs::default();
            for d in &pos {
                if let Some(t) = &trust {
                    let v = crate::signed_dossier::verify(Path::new(d), t)
                        .map_err(|e| e.to_string())?;
                    if !v.passed() {
                        return Err(format!("{d}: signature check failed: {:?}", v.problems));
                    }
                }
                let text = inputs.read(Path::new(d), "lab-report.json", "lab-report")?;
                let rp: labreport::LabReport =
                    serde_json::from_str(&text).map_err(|e| format!("{d}: {e}"))?;
                if rp.format != labreport::REPORT_FORMAT {
                    return Err(format!("{d}: not a {} report", labreport::REPORT_FORMAT));
                }
                reps.push(rp);
            }
            let c = labreport::compare(&reps)?;
            let h = format!(
                "{} reports: {} consistent, {} inconsistent, {} not comparable{}",
                reps.len(),
                c.consistent,
                c.inconsistent,
                c.not_comparable,
                if trust.is_some() {
                    "; signatures verified"
                } else {
                    "; signatures not checked (give --trust)"
                }
            );
            write_result(
                dir,
                "comparison.json",
                &c,
                &simlab::summary("compare", &h),
                &[("comparison.md", labreport::compare_markdown(&c))],
                &inputs,
            )?;
            h
        }
    };
    sign(&out, &seed, &signer, &utc)?;
    Ok(format!("[simlab] {headline}; signed in {out}"))
}

fn run_tco(args: &[String]) -> Result<(), String> {
    let pos = positional(args);
    let p = pos.first().ok_or(USAGE)?;
    let t = tco::Tco::parse(&std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?)?;
    let rep = tco::compute(&t);
    let md = tco::markdown(&rep);
    if let Some(m) = flag(args, "--markdown") {
        std::fs::write(&m, &md).map_err(|e| format!("{m}: {e}"))?;
    }
    if has_flag(args, "--json") {
        println!("{}", serde_json::to_string_pretty(&rep).unwrap());
    } else {
        print!("{md}");
    }
    Ok(())
}

/// `kshana-pro simlab ...`.
pub fn run(args: &[String]) -> ExitCode {
    let gate = match args.first().map(String::as_str) {
        Some("chain" | "sync") => simlab::FEATURE_REPLAY,
        Some("selftest") => simlab::FEATURE_SELFTEST,
        Some("report" | "compare") => simlab::FEATURE_REPORT,
        Some("check") => return super::scorecard_cmd::verify_dir(args),
        Some("tco") => {
            return match run_tco(args) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => fail(e),
            }
        }
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            return match simlab::synth::write_all(Path::new(dir)) {
                Ok(()) => {
                    println!("wrote synthetic chains, replay sets, self-tests, lab results and a TCO file under {dir}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            };
        }
        _ => return fail(USAGE),
    };
    match produce(args, gate) {
        Ok(msg) => {
            eprintln!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
