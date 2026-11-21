// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro isvv <run|synth>`: requirements coverage and verification status from test
//! results, a requirements list and a trace map (docs/ISVV-EVIDENCE.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::isvv;

use super::forensics_cmd::keys;
use super::grid_cmd::{gate, signing};

pub const ISVV_USAGE: &str = "usage: kshana-pro isvv <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <key hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Signed requirements-coverage and verification-status report: per-requirement pass/fail, untested requirements, orphan tests, change since the baseline (Pro)\n\
    synth <dir>   Write SYNTHETIC requirements, trace map, two JUnit runs and requests (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro isvv ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(ISVV_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let d = Path::new(&arg);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let (reqs, trace, before, after) = isvv::synth();
            for (n, t) in [
                ("requirements.csv", reqs.as_str()),
                ("trace.csv", trace.as_str()),
                ("junit-baseline.xml", before.as_str()),
                ("junit-current.xml", after.as_str()),
                (
                    "baseline.toml",
                    "title = \"SYNTHETIC unit: baseline\"\nrequirements = \"requirements.csv\"\ntrace = \"trace.csv\"\nresults = [\"junit-baseline.xml\"]\n",
                ),
                (
                    "current.toml",
                    "title = \"SYNTHETIC unit: current\"\nrequirements = \"requirements.csv\"\ntrace = \"trace.csv\"\nresults = [\"junit-current.xml\"]\nbaseline = \"baseline-pack\"\n",
                ),
            ] {
                std::fs::write(d.join(n), t).map_err(|e| e.to_string())?;
            }
            Ok(format!(
                "[isvv] SYNTHETIC records in {arg}: run baseline.toml with --out {arg}/baseline-pack, then current.toml"
            ))
        })(),
        Some("run") => (|| {
            gate(args, isvv::FEATURE)?;
            let s = signing(args, "isvv run")?;
            let trust = keys(args, "--trust")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: isvv::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let (rep, inputs) = isvv::run(&q, &base, &trust)?;
            isvv::write(Path::new(&s.out), &rep, &src, &inputs)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            let mut m = format!("[isvv] {}", rep.verdict);
            if let Some(d) = &rep.baseline {
                m.push_str(&format!(
                    "\n[isvv] since baseline: {} status changes, {} test regressions, {} fixes; baseline {}",
                    d.status_changes.len(),
                    d.test_regressions.len(),
                    d.test_fixes.len(),
                    d.baseline_signature
                ));
            }
            m.push_str(&format!("\n[isvv] signed report in {}", s.out));
            Ok(m)
        })(),
        _ => return fail(ISVV_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
