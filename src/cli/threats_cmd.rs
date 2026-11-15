// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro threats <list|show|export|feed>` (roadmap 0.42, E2). `list` needs no
//! licence (ids and titles only); `show`, `export` and `feed build` need the
//! `pro-threat-library` feature; `feed verify` needs none.

use std::path::Path;
use std::process::ExitCode;

use crate::license;
use crate::threat_library::{self, export, feed};

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::entitlement;
use super::{flag, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro threats <command>\n\
    list                                   Library scenarios: id, family, title (no licence needed)\n\
    show <id> [--license <token>] [--dev]  Provenance, licence notes and how to fetch the original data (Pro)\n\
    export <id> --out <dir> [--license <token>] [--dev]   The scenario, its analogue stream, receiver-trust scenario, test-bench files and timing profile (Pro)\n\
    feed build --out <dir> --edition <YYYY-MM> --signer <name> --key-file <file> [--previous <feed.json>] [--utc <iso>] [--license <token>] [--dev]   Build and sign a feed edition (vendor)\n\
    feed verify <dir> --trust <publisher key hex>[,...] [--previous <feed.json>]   Verify a feed edition offline (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn gate(args: &[String]) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, threat_library::FEATURE, now).map_err(|e| e.to_string())
}

pub fn run(args: &[String]) -> ExitCode {
    let arg1 = args.get(1).filter(|a| !a.starts_with("--")).cloned();
    match (args.first().map(String::as_str), arg1.as_deref()) {
        (Some("list"), _) => match threat_library::library() {
            Ok(l) => {
                println!(
                    "Kshana Pro threat library, edition {}",
                    threat_library::EDITION
                );
                for s in l {
                    println!("{:<42} {:<11} {}", s.id, s.family.as_str(), s.title);
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        (Some("show"), Some(id)) => {
            if let Err(e) = gate(args) {
                return fail(e);
            }
            match threat_library::get(id) {
                Ok(s) => {
                    print!("{}", export::provenance_text(&s));
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        (Some("export"), Some(id)) => {
            if let Err(e) = gate(args) {
                return fail(e);
            }
            let Some(out) = flag(args, "--out") else {
                return fail("export needs --out <dir>");
            };
            match threat_library::get(id).and_then(|s| export::write(&s, Path::new(&out))) {
                Ok(files) => {
                    eprintln!("[threats] wrote {} files to {out}", files.len());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        (Some("feed"), Some("build")) => {
            if let Err(e) = gate(args) {
                return fail(e);
            }
            let (Some(out), Some(edition), Some(signer), Some(kf)) = (
                flag(args, "--out"),
                flag(args, "--edition"),
                flag(args, "--signer"),
                flag(args, "--key-file"),
            ) else {
                return fail("feed build needs --out, --edition, --signer and --key-file");
            };
            let seed = match read_seed(&kf) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let prev = match flag(args, "--previous").map(std::fs::read) {
                Some(Ok(b)) => Some(b),
                Some(Err(e)) => return fail(format!("--previous: {e}")),
                None => None,
            };
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            match feed::build(
                Path::new(&out),
                threat_library::LIBRARY,
                &edition,
                prev.as_deref(),
                &seed,
                &signer,
                &utc,
            ) {
                Ok(m) => {
                    eprintln!(
                        "[threats] feed edition {} with {} scenarios signed in {out}",
                        m.edition,
                        m.scenarios.len()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        (Some("feed"), Some("verify")) => {
            let Some(dir) = args.get(2).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let trust = match keys(args, "--trust") {
                Ok(k) => k,
                Err(e) => return fail(e),
            };
            let prev = match flag(args, "--previous").map(std::fs::read) {
                Some(Ok(b)) => Some(b),
                Some(Err(e)) => return fail(format!("--previous: {e}")),
                None => None,
            };
            match feed::verify(Path::new(dir), &trust, prev.as_deref()) {
                Ok(v) => {
                    println!(
                        "feed edition {}: {} scenarios, signer {} ({}), signature {}, key {}",
                        v.edition,
                        v.scenarios,
                        v.signer,
                        v.key_id,
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
        _ => fail(USAGE),
    }
}
