// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro hybrid-sim <run|synth>` (eLoran and GNSS signal files with faults and a shared
//! truth; docs/HYBRID-SIM.md).

use std::path::Path;
use std::process::ExitCode;

use crate::hybrid_sim;

use super::grid_cmd::{gate, signing};

pub const HYBRID_USAGE: &str = "usage: kshana-pro hybrid-sim <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   One scenario writing time-aligned eLoran (100 kHz) and GNSS baseband SigMF files with injected faults (ASF error, skywave, GNSS outage, GNSS interference as C/N0 drop) and a shared truth; files only, no transmit path; signed (Pro)\n\
    synth <dir> --nav <rinex>   Write a SYNTHETIC scenario and request using a RINEX 3 navigation file you supply (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro hybrid-sim ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(HYBRID_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let nav = super::flag(args, "--nav").ok_or(HYBRID_USAGE)?;
            let d = Path::new(&arg);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let (s, q) = hybrid_sim::synth();
            std::fs::write(d.join("scenario.toml"), s).map_err(|e| e.to_string())?;
            std::fs::write(d.join("request.toml"), q).map_err(|e| e.to_string())?;
            std::fs::copy(&nav, d.join("nav.rnx")).map_err(|e| format!("{nav}: {e}"))?;
            Ok(format!(
                "[hybrid-sim] SYNTHETIC scenario and request in {arg}"
            ))
        })(),
        Some("run") => (|| {
            gate(args, hybrid_sim::FEATURE)?;
            let s = signing(args, "hybrid-sim run")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: hybrid_sim::Request = toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = hybrid_sim::base_of(&arg);
            let out = Path::new(&s.out);
            let t = hybrid_sim::run(&q, &base, out)?;
            std::fs::create_dir_all(out.join("inputs")).map_err(|e| e.to_string())?;
            std::fs::write(out.join("inputs").join("request.toml"), &src)
                .map_err(|e| e.to_string())?;
            for f in [&q.eloran_scenario, &q.nav] {
                let p = base.join(f);
                let n = p.file_name().map(|x| x.to_owned()).unwrap_or_default();
                std::fs::copy(&p, out.join("inputs").join(n)).map_err(|e| format!("{f}: {e}"))?;
            }
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[hybrid-sim] {} fault(s); eLoran {} samples, GNSS {} samples from {}; signed in {}",
                t.faults.len(),
                t.eloran.samples,
                t.gnss.samples,
                t.start_utc,
                s.out
            ))
        })(),
        _ => return fail(HYBRID_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
