// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro adsb-phantom <check|synth>` (research 2026-11-20 gaps, B1002;
//! docs/ADSB-PHANTOM.md).

use std::path::Path;
use std::process::ExitCode;

use crate::adsb_phantom as ap;
use crate::transport::{page, write_pack};

use super::flag;
use super::grid_cmd::{gate, signing};

pub const ADSB_PHANTOM_USAGE: &str = "usage: kshana-pro adsb-phantom <command>\n\
    check <adsb.csv> <receivers.toml> --out <dir> --signer <name> --key-file <file> [--reference <tracks.csv>] [--params <params.toml>] [--pseudonymise <salt>] [--utc <iso>] [--license <token>] [--dev]\n\
        Per-target plausibility score with reasons from the interference-map ADS-B CSV, the declared receivers and an optional independent track file; signed (Pro). Never locates a transmitter.\n\
    synth <dir>   A SYNTHETIC scene with injected phantoms (no licence needed)";

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

/// `kshana-pro adsb-phantom ...`.
pub fn run(args: &[String]) -> ExitCode {
    let pos: Vec<&String> = args
        .iter()
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let r: Result<String, String> = match (args.first().map(String::as_str), pos.as_slice()) {
        (Some("synth"), [d]) => (|| {
            let dir = Path::new(d.as_str());
            std::fs::create_dir_all(dir).map_err(|e| format!("{d}: {e}"))?;
            let (csv, rx, rf, inj) = ap::synth();
            for (n, t) in [
                ("adsb.csv", csv),
                ("receivers.toml", rx),
                ("reference.csv", rf),
                ("injected.txt", inj.join("\n") + "\n"),
            ] {
                std::fs::write(dir.join(n), t).map_err(|e| format!("{d}: {e}"))?;
            }
            Ok(format!(
                "[adsb-phantom] SYNTHETIC scene in {d} ({} injected addresses in injected.txt)",
                inj.len()
            ))
        })(),
        (Some("check"), [a, rx]) => (|| {
            gate(args, ap::FEATURE)?;
            let s = signing(args, "adsb-phantom check")?;
            let csv = read(a)?;
            let rxs = read(rx)?;
            let receivers: ap::Receivers = crate::transport::parse_toml(&rxs, rx)?;
            let reference = flag(args, "--reference").map(|p| read(&p)).transpose()?;
            let params: ap::Params = match flag(args, "--params") {
                Some(p) => crate::transport::parse_toml(&read(&p)?, &p)?,
                None => ap::Params::default(),
            };
            let salt = flag(args, "--pseudonymise");
            let r = ap::check(
                &csv,
                &receivers.receiver,
                reference.as_deref(),
                &params,
                salt.as_deref(),
            )?;
            let rows = r
                .targets
                .iter()
                .filter(|t| !t.reasons.is_empty())
                .map(|t| {
                    vec![
                        t.address.clone(),
                        format!("{:.2}", t.score),
                        if t.flagged { "review" } else { "-" }.into(),
                        t.reports.to_string(),
                        format!("{} to {}", t.first_utc, t.last_utc),
                        t.reasons
                            .iter()
                            .map(|x| format!("{}: {}", x.code, x.detail))
                            .collect::<Vec<_>>()
                            .join("; "),
                    ]
                })
                .collect();
            let counts = r
                .reason_counts
                .iter()
                .map(|(k, v)| vec![k.clone(), v.to_string()])
                .collect();
            let html = page(
                "ADS-B phantom-target plausibility evidence",
                r.label,
                &r.verdict,
                &[
                    r.not_done.to_string(),
                    format!(
                        "Method {}; {} receiver(s) declared; reference points {}.",
                        r.method,
                        r.receivers.len(),
                        r.stats.reference_points
                    ),
                ],
                &[
                    (
                        "Targets with a reason".into(),
                        vec!["address", "score", "status", "reports", "heard", "reasons"],
                        rows,
                    ),
                    ("Reasons".into(), vec!["reason", "targets"], counts),
                ],
            );
            let out = Path::new(&s.out);
            write_pack(out, &r, &html, "receivers.toml", &rxs)?;
            let io = |e: std::io::Error| format!("{}: {e}", out.display());
            // Pseudonymised packs keep the input's hash, not its addresses.
            if salt.is_some() {
                std::fs::write(
                    out.join("inputs").join("adsb.csv.sha256"),
                    crate::forensics::inputs::sha256_hex(csv.as_bytes()) + "\n",
                )
                .map_err(io)?;
            } else {
                std::fs::write(out.join("inputs").join("adsb.csv"), &csv).map_err(io)?;
            }
            if let Some(rf) = &reference {
                std::fs::write(out.join("inputs").join("reference.csv"), rf).map_err(io)?;
            }
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[adsb-phantom] {}\n[adsb-phantom] signed pack in {}",
                r.verdict, s.out
            ))
        })(),
        _ => Err(ADSB_PHANTOM_USAGE.into()),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
