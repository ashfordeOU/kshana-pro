// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The evidence product commands of roadmap 0.42 E4 to E7:
//!
//! * `kshana-pro integrity <score|verify>`: AIS and position integrity scoring (E5);
//! * `kshana-pro verify-service <run|template|verify>`: PNT service verification studies (E4);
//! * `kshana-pro frameworks <list|show|pack|verify>`: framework evidence packs (E6);
//! * `kshana-pro exposure <run|verify>`: insurer exposure summary (E7).
//!
//! Producing a signed result needs a licence tier that unlocks its kind or feature; `verify`,
//! `template`, `list` and `show` need none.

use std::path::Path;
use std::process::ExitCode;

use crate::license;

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::{entitlement, verify_dir};
use super::{flag, now_unix, utc_iso8601_now};

pub const INTEGRITY_USAGE: &str = "usage: kshana-pro integrity <command>\n\
    score <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Score AIS identities per fleet and area, with the RF cross-check when fixes are given (Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a scoring's signature and every file, offline (no licence needed)";

pub const STUDY_USAGE: &str = "usage: kshana-pro verify-service <command>\n\
    template\n\
        Print an empty study (free)\n\
    run <study.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Run a pre-registered user-side verification study of a PNT service (Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a study's signature and every file, offline (no licence needed)";

pub const FRAMEWORKS_USAGE: &str = "usage: kshana-pro frameworks <command>\n\
    list\n\
        The framework templates (free)\n\
    show <id>\n\
        One template as JSON (free)\n\
    pack <id> --evidence <dir>[,<dir>...] --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Map signed Kshana Pro evidence to a framework's duties (\"mapped to\" only) and sign the pack (Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a pack's signature, offline (no licence needed)";

pub const EXPOSURE_USAGE: &str = "usage: kshana-pro exposure <command>\n\
    run <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Periodic signed exposure summary of a fleet or route from signed forensics and integrity evidence (Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a summary's signature, offline (no licence needed)\n\
    equipment <equipment.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--dev]\n\
        Equipment-resilience rating from scorecards or declared capabilities (Pro)\n\
    voyage <voyage.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--dev]\n\
        Voyage risk score from route exposure to historic events and the equipment rating (Pro)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// The licence check, the output directory, the signing seed and signer of a producing command.
pub(super) fn producing(
    args: &[String],
    gate: &str,
) -> Result<(String, [u8; 32], String, String), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, gate, now).map_err(|e| e.to_string())?;
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return Err("needs --out, --signer and --key-file".into());
    };
    let seed = read_seed(&kf)?;
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    Ok((out, seed, signer, utc))
}

pub(super) fn sign(out: &str, seed: &[u8; 32], signer: &str, utc: &str) -> Result<(), String> {
    crate::signed_dossier::sign(Path::new(out), seed, signer, utc)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

pub fn integrity(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("score") => {
            let Some(file) = file_arg(args) else {
                return fail(INTEGRITY_USAGE);
            };
            let r = producing(args, crate::integrity::KIND).and_then(|(out, seed, signer, utc)| {
                let (s, _) = crate::integrity::IntegrityScenario::from_file(Path::new(file))?;
                let r = crate::integrity::score(&s)?;
                crate::integrity::write_dir(Path::new(&out), &r)?;
                sign(&out, &seed, &signer, &utc)?;
                Ok((r, out))
            });
            match r {
                Ok((r, out)) => {
                    let g = &r.groups[0];
                    eprintln!(
                        "[integrity] {} identities, integrity score {}; signed scoring in {out}",
                        g.vessels,
                        g.integrity_score
                            .map(|s| format!("{s:.4}"))
                            .unwrap_or_else(|| "n/a".into())
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("verify") => verify_dir(args),
        _ => fail(INTEGRITY_USAGE),
    }
}

pub fn verify_service(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("template") => {
            print!("{}", crate::pnt_study::TEMPLATE);
            ExitCode::SUCCESS
        }
        Some("run") => {
            let Some(file) = file_arg(args) else {
                return fail(STUDY_USAGE);
            };
            let r = producing(args, crate::pnt_study::KIND).and_then(|(out, seed, signer, utc)| {
                let (s, src) = crate::pnt_study::StudySpec::from_file(Path::new(file))?;
                let r = crate::pnt_study::run_study(&s)?;
                crate::pnt_study::write_dir(Path::new(&out), &r, &src)?;
                sign(&out, &seed, &signer, &utc)?;
                Ok((r, out))
            });
            match r {
                Ok((r, out)) => {
                    eprintln!(
                        "[verify-service] {} of {} bars met; study {}; signed in {out}",
                        r.bars.iter().filter(|b| b.met).count(),
                        r.bars.len(),
                        if r.passed { "passes" } else { "does not pass" }
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("verify") => verify_dir(args),
        _ => fail(STUDY_USAGE),
    }
}

pub fn frameworks(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("list") => match crate::frameworks::templates() {
            Ok(ts) => {
                for t in ts {
                    println!(
                        "{:<20} {} ({} duties){}",
                        t.id,
                        t.title,
                        t.duties.len(),
                        t.status.map(|s| format!(" [{s}]")).unwrap_or_default()
                    );
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Some("show") => match file_arg(args).map(|id| crate::frameworks::template(id)) {
            Some(Ok(t)) => {
                println!("{}", serde_json::to_string_pretty(&t).expect("plain data"));
                ExitCode::SUCCESS
            }
            Some(Err(e)) => fail(e),
            None => fail(FRAMEWORKS_USAGE),
        },
        Some("pack") => {
            let Some(id) = file_arg(args) else {
                return fail(FRAMEWORKS_USAGE);
            };
            let r =
                producing(args, crate::frameworks::FEATURE).and_then(|(out, seed, signer, utc)| {
                    let t = crate::frameworks::template(id)?;
                    let trust = keys(args, "--trust")?;
                    let ev = flag(args, "--evidence").ok_or("pack needs --evidence")?;
                    let items = crate::frameworks::paths(&ev)
                        .iter()
                        .map(|d| crate::frameworks::read_evidence(d, &trust))
                        .collect::<Result<Vec<_>, _>>()?;
                    let p = crate::frameworks::build(&t, items);
                    crate::frameworks::write_dir(Path::new(&out), &p)?;
                    sign(&out, &seed, &signer, &utc)?;
                    Ok((p, out))
                });
            match r {
                Ok((p, out)) => {
                    eprintln!(
                        "[frameworks] {}: {} duties with evidence mapped, {} without evidence supplied, {} not covered; signed in {out}",
                        p.framework, p.counts[0], p.counts[1], p.counts[2]
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("verify") => verify_dir(args),
        _ => fail(FRAMEWORKS_USAGE),
    }
}

pub fn exposure(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("run") => {
            let Some(file) = file_arg(args) else {
                return fail(EXPOSURE_USAGE);
            };
            let r =
                producing(args, crate::exposure::FEATURE).and_then(|(out, seed, signer, utc)| {
                    let trust = keys(args, "--trust")?;
                    let (q, src) = crate::exposure::ExposureRequest::from_file(Path::new(file))?;
                    let x = crate::exposure::build(&q, &trust)?;
                    crate::exposure::write_dir(Path::new(&out), &x, &src)?;
                    sign(&out, &seed, &signer, &utc)?;
                    Ok((x, out))
                });
            match r {
                Ok((x, out)) => {
                    eprintln!(
                        "[exposure] {} incidents ({} jamming, {} spoofing indicated); signed in {out}",
                        x.totals.incidents, x.totals.jamming_indicated, x.totals.spoofing_indicated
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("verify") => verify_dir(args),
        Some("equipment") => super::pnt_cmd::equipment(args),
        Some("voyage") => super::pnt_cmd::voyage(args),
        _ => fail(EXPOSURE_USAGE),
    }
}
