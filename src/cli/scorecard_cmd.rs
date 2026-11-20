// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro scorecard <run|verify|osnma|flags|units>` (roadmap 0.42, E3; W1119 C6; E13). `run` needs a licence tier that
//! unlocks `pro-receiver-scorecard`; `verify` needs none.

use std::path::Path;
use std::process::ExitCode;

use crate::license::{self, Entitlement, Product};
use crate::scorecards::{self, report, Campaign};

use super::forensics_cmd::{keys, read_seed};
use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro scorecard <command>\n\
    run <campaign.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Score a receiver test campaign; write the signed scorecard and test report (Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a report's signature and every file, offline (no licence needed)\n\
    osnma <receiver-log.json> --out <dir> --signer <name> --key-file <file> [--bars <bars.toml>] [--dev]\n\
        OSNMA receiver conformance scorecard against pre-registered bars (`kshana-pro scorecard osnma` for details)\n\
    flags <request.toml> --out <dir> --signer <name> --key-file <file> [--dev] | flags-synth <dir>\n\
        Spoof-, jamming- and integrity-flag efficacy per model and firmware (Pro; `kshana-pro scorecard flags` for details)\n\
    units <request.toml> --out <dir> --signer <name> --key-file <file> [--dev] | units-synth <dir>\n\
        Low-cost receiver, board and handset scorecards with firmware regression alerts (Pro; `kshana-pro scorecard units` for details)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// The entitlement of a command line: `--dev` (debug builds only), `--license`, or the
/// stored licence.
pub(super) fn entitlement(args: &[String], now: i64) -> Result<Entitlement, String> {
    if has_flag(args, "--dev") && cfg!(debug_assertions) {
        return Ok(Entitlement {
            licensee: "(dev)".into(),
            tier: license::Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
            arksa: false,
        });
    }
    match flag(args, "--license") {
        Some(t) => license_cmd::verify_token_for(&t, now, Product::Cli)
            .map_err(|e| format!("license invalid: {e}")),
        None => Ok(license_cmd::stored_or_evaluation(now, Product::Cli)),
    }
}

/// Verify a signed report directory and print the outcome.
pub(super) fn verify_dir(args: &[String]) -> ExitCode {
    let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let trust = match keys(args, "--trust") {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    match crate::signed_dossier::verify(Path::new(dir), &trust) {
        Ok(v) => {
            println!(
                "signer: {} ({}), {} files, signature {}, key {}",
                v.signer,
                v.key_id,
                v.files,
                if v.signature_ok { "valid" } else { "INVALID" },
                match v.trusted {
                    Some(true) => "trusted",
                    Some(false) => "NOT trusted",
                    None => "not pinned (give --trust)",
                }
            );
            for p in &v.problems {
                println!("  problem: {p}");
            }
            if v.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => fail(e),
    }
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("run") => {
            let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let now = now_unix();
            let ent = match entitlement(args, now) {
                Ok(e) => e,
                Err(e) => return fail(e),
            };
            if let Err(e) = license::check(&ent, scorecards::KIND, now) {
                return fail(e);
            }
            let (Some(out), Some(signer), Some(kf)) = (
                flag(args, "--out"),
                flag(args, "--signer"),
                flag(args, "--key-file"),
            ) else {
                return fail("run needs --out, --signer and --key-file");
            };
            let seed = match read_seed(&kf) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let (c, src) = match Campaign::from_file(Path::new(file)) {
                Ok(c) => c,
                Err(e) => return fail(e),
            };
            let s = match scorecards::score(&c, &src) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let out = Path::new(&out);
            if let Err(e) = report::write_dir(out, &s, &src) {
                return fail(e);
            }
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            if let Err(e) = crate::signed_dossier::sign(out, &seed, &signer, &utc) {
                return fail(e);
            }
            eprintln!(
                "[scorecard] {} cases, {} events; highest level supported by the evidence: {}; signed report in {}",
                s.totals.cases,
                s.totals.events,
                s.levels.highest_supported,
                out.display()
            );
            ExitCode::SUCCESS
        }
        Some("verify") => verify_dir(args),
        Some("osnma") => super::pnt_cmd::osnma(args),
        Some("osnma-synth") => super::pnt_cmd::osnma_synth(args),
        Some("flags") => super::receiver_cmd::flags(args),
        Some("flags-synth") => super::receiver_cmd::flags_synth(args),
        Some("units") => super::receiver_cmd::units(args),
        Some("units-synth") => super::receiver_cmd::units_synth(args),
        _ => fail(USAGE),
    }
}
