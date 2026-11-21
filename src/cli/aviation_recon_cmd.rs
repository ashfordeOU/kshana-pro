// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro aviation-recon <run|synth>` (research 2026-11-20 gaps, B0602;
//! docs/AVIATION-RECON.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::aviation::event_recon;

use super::grid_cmd::{gate, signing};

pub const RECON_USAGE: &str = "usage: kshana-pro aviation-recon <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Aviation or drone event reconstruction: clock alignment of FDM/QAR, ADS-B and tracker records, GNSS vs baro vs ADS-B divergences explained, event timeline, signed investigator pack (Pro)\n\
    synth <dir>   Write a SYNTHETIC King Air-type flight with an altitude divergence and its request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro aviation-recon ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(RECON_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let d = Path::new(&arg);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let (f, a, t, q) = event_recon::synth::make();
            for (n, s) in [
                ("fdm.csv", f),
                ("adsb.csv", a),
                ("tracker.csv", t),
                ("request.toml", q),
            ] {
                std::fs::write(d.join(n), s).map_err(|e| e.to_string())?;
            }
            Ok(format!(
                "[aviation-recon] SYNTHETIC King Air-type flight and request in {arg}"
            ))
        })(),
        Some("run") => (|| {
            gate(args, event_recon::FEATURE)?;
            let s = signing(args, "aviation-recon run")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: event_recon::Request =
                toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let (r, fdm) = event_recon::run(&q, &base)?;
            let out = Path::new(&s.out);
            crate::transport::write_pack(out, &r, &event_recon::html(&r), "request.toml", &src)?;
            let name = Path::new(&q.fdm)
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_else(|| "fdm.csv".into());
            std::fs::write(out.join("inputs").join(name), fdm).map_err(|e| e.to_string())?;
            for f in [q.adsb.as_ref(), q.tracker.as_ref()].into_iter().flatten() {
                let p = base.join(f);
                let n = p.file_name().map(|x| x.to_owned()).unwrap_or_default();
                std::fs::copy(&p, out.join("inputs").join(n)).map_err(|e| format!("{f}: {e}"))?;
            }
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[aviation-recon] {}\n[aviation-recon] signed investigator pack in {}",
                r.verdict, s.out
            ))
        })(),
        _ => return fail(RECON_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
