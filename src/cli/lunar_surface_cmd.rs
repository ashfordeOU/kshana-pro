// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro lunar-surface <plume|traverse|rf-forensics|synth>` (research 2026-11-20 gaps,
//! E24 B2404 second slice; docs/LUNAR-SURFACE.md). Each analysis needs its Pro feature and
//! prints JSON or writes a signed evidence directory; `synth` is free.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::lunar_surface::{plume, rf, synth, traverse};

use super::grid_cmd::{gate, signing};

pub const LUNAR_SURFACE_USAGE: &str = "usage: kshana-pro lunar-surface <command>\n\
    plume <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   Dust-plume visibility: MODELLED parametric ejecta-sheet extinction along declared lines of sight, every assumption stated (Pro)\n\
    traverse <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   LTV/EVA navigation safety evidence: sigma honesty, hazard keep-out margins with PL, walk-back margin (Pro)\n\
    rf-forensics <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   RF self-interference forensics from a SigMF recording: spurs, harmonic match to declared on-board sources, time correlation, in-band C/N0 loss; analysis only (Pro)\n\
    synth <dir>                                                      Write SYNTHETIC inputs for all three (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn base_of(path: &str) -> PathBuf {
    Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn parse<T: serde::de::DeserializeOwned>(path: &str) -> Result<(T, String), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok((crate::transport::parse_toml(&src, path)?, src))
}

/// `kshana-pro lunar-surface ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    if sub == Some("synth") {
        let Some(dir) = args.get(1) else {
            return fail(LUNAR_SURFACE_USAGE);
        };
        return match synth::write(Path::new(dir)) {
            Ok(()) => {
                eprintln!("[lunar-surface] SYNTHETIC inputs in {dir}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        };
    }
    let feature = match sub {
        Some("plume") => plume::FEATURE,
        Some("traverse") => traverse::FEATURE,
        Some("rf-forensics") => rf::FEATURE,
        _ => return fail(LUNAR_SURFACE_USAGE),
    };
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(LUNAR_SURFACE_USAGE);
    };
    if let Err(e) = gate(args, feature) {
        return fail(e);
    }
    let r = (|| -> Result<String, String> {
        let base = base_of(&path);
        let (json, html, verdict, req) = match sub {
            Some("plume") => {
                let (q, src): (plume::Request, _) = parse(&path)?;
                let rep = plume::run(&q)?;
                (
                    crate::transport::rounded_json(&rep),
                    plume::html(&rep),
                    rep.verdict,
                    src,
                )
            }
            Some("traverse") => {
                let (q, src): (traverse::Request, _) = parse(&path)?;
                let rep = traverse::run(&q, &base)?;
                (
                    crate::transport::rounded_json(&rep),
                    traverse::html(&rep),
                    rep.verdict,
                    src,
                )
            }
            _ => {
                let (q, src): (rf::Request, _) = parse(&path)?;
                let rep = rf::run(&q, &base)?;
                (
                    crate::transport::rounded_json(&rep),
                    rf::html(&rep),
                    rep.verdict,
                    src,
                )
            }
        };
        if let Some(o) = super::flag(args, "--json") {
            std::fs::write(&o, &json).map_err(|e| format!("{o}: {e}"))?;
        }
        if super::flag(args, "--out").is_some() {
            let s = signing(args, &format!("lunar-surface {}", sub.unwrap_or_default()))?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &serde_json::from_str::<serde_json::Value>(&json).map_err(|e| e.to_string())?,
                &html,
                "request.toml",
                &req,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            return Ok(format!(
                "[lunar-surface] {verdict}\n[lunar-surface] signed evidence directory in {}",
                s.out
            ));
        }
        if super::flag(args, "--json").is_none() {
            println!("{json}");
        }
        Ok(format!("[lunar-surface] {verdict}"))
    })();
    match r {
        Ok(v) => {
            eprintln!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
