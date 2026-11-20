// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro ais <consistency|sanctions|iuu|subsea|synth>` (research 2026-11-20 gaps, epic
//! E07 Pro parts; docs/AIS-EVIDENCE.md). Each pack needs its Pro feature and writes a signed
//! evidence directory; `synth` is free. Verify with `kshana-pro dossier verify <dir> --trust
//! <key>` (free).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::ais_evidence::{consistency, packs, synth};

use super::grid_cmd::{gate, signing};

pub const AIS_USAGE: &str = "usage: kshana-pro ais <command>\n\
    consistency <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Cross-source consistency of AIS against radar, VTS, LRIT, satellite AIS or imagery detections with declared accuracy; signed (Pro)\n\
    sanctions <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Dark-activity review of named vessels: gaps, encounters, identity changes, jumps, shared MMSI; no list bundled, no determination; signed (Pro)\n\
    iuu <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Fishing-activity review against declared zones: time inside, fishing-like speed and status, gaps near zones, encounters at sea; signed (Pro)\n\
    subsea <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Cable and pipeline corridor activity, anchor-status-while-moving, vessels around a reported fault (presence, not attribution); signed (Pro)\n\
    synth <dir>                                                      Write a SYNTHETIC scenario with requests for all four (no licence needed)";

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

/// `kshana-pro ais ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    if sub == Some("synth") {
        let Some(dir) = args.get(1) else {
            return fail(AIS_USAGE);
        };
        return match synth::write(Path::new(dir)) {
            Ok(()) => {
                eprintln!("[ais] SYNTHETIC scenario and requests in {dir}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        };
    }
    let feature = match sub {
        Some("consistency") => consistency::FEATURE,
        Some("sanctions") => packs::FEATURE_SANCTIONS,
        Some("iuu") => packs::FEATURE_IUU,
        Some("subsea") => packs::FEATURE_SUBSEA,
        _ => return fail(AIS_USAGE),
    };
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(AIS_USAGE);
    };
    if let Err(e) = gate(args, feature) {
        return fail(e);
    }
    let r = (|| -> Result<String, String> {
        let s = signing(args, &format!("ais {}", sub.unwrap_or_default()))?;
        let base = base_of(&path);
        let (json_html, verdict, req) = match sub {
            Some("consistency") => {
                let (q, src): (consistency::Request, _) = parse(&path)?;
                let rep = consistency::run(&q, &base)?;
                (
                    (
                        crate::transport::rounded_json(&rep),
                        consistency::html(&rep),
                    ),
                    rep.verdict,
                    src,
                )
            }
            Some("sanctions") => {
                let (q, src): (packs::sanctions::Request, _) = parse(&path)?;
                let rep = packs::sanctions::run(&q, &base)?;
                (
                    (
                        crate::transport::rounded_json(&rep),
                        packs::sanctions::html(&rep),
                    ),
                    rep.verdict,
                    src,
                )
            }
            Some("iuu") => {
                let (q, src): (packs::iuu::Request, _) = parse(&path)?;
                let rep = packs::iuu::run(&q, &base)?;
                (
                    (crate::transport::rounded_json(&rep), packs::iuu::html(&rep)),
                    rep.verdict,
                    src,
                )
            }
            _ => {
                let (q, src): (packs::subsea::Request, _) = parse(&path)?;
                let rep = packs::subsea::run(&q, &base)?;
                (
                    (
                        crate::transport::rounded_json(&rep),
                        packs::subsea::html(&rep),
                    ),
                    rep.verdict,
                    src,
                )
            }
        };
        let out = Path::new(&s.out);
        let io = |e: std::io::Error| format!("{}: {e}", out.display());
        std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
        std::fs::write(out.join("report.json"), json_html.0).map_err(io)?;
        std::fs::write(out.join("report.html"), json_html.1).map_err(io)?;
        std::fs::write(out.join("inputs").join("request.toml"), req).map_err(io)?;
        crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc).map_err(|e| e.to_string())?;
        Ok(format!(
            "[ais] {verdict}\n[ais] signed evidence directory in {}",
            s.out
        ))
    })();
    match r {
        Ok(v) => {
            eprintln!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
