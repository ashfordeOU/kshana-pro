// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro forensics <run|review|verify|pack-payload>` (roadmap 0.42, E1).
//!
//! `run` needs a licence tier that unlocks `pro-incident-forensics` and the forensics Pro pack
//! loaded (from the Desktop app data folder with `--app-data`, or already loaded by the
//! host); `--dev` skips both in a debug build only. `review` signs a reviewer's statement
//! (no licence: the reviewer is an outside expert). `verify` needs no licence.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::forensics::{self, bundle, IncidentScenario, Variant};
use crate::license::{self, Entitlement, Product, KEY_LEN};

use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

const USAGE: &str = "usage: kshana-pro forensics <command>\n\
    run <incident.toml> --out <dir> --key-file <file> --signer <name> [--utc <iso>] [--variant standard|claim-grade] [--allow-incomplete] [--app-data <dir>] [--license <token>] [--dev]\n\
        Signed forensic report bundle for an incident (Pro pack). Exit 3 when a claim-grade checklist is not met (bundle still written, marked so)\n\
    review <bundle> --reviewer <name> --qualification <text> --organisation <text> --independence <text> --decision <concur|concur-with-reservations|do-not-concur> --statement-file <file> --key-file <reviewer key> [--utc <iso>]\n\
        Add a signed expert review to a claim-grade bundle; the issuer then re-signs it (`kshana-pro dossier sign`)\n\
    verify <bundle> [--trust <issuer public key hex>[,...]] [--reviewer-trust <hex>[,...]]\n\
        Check the signature, every input hash, the evidence packs and the review, offline (no licence needed)\n\
    pack-payload <dir>\n\
        Write the forensics pack's payload files into <dir> for `kshana-pro packs build --provides pro-incident-forensics`";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn read_seed(path: &str) -> Result<[u8; KEY_LEN], String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("{path}: {e}"))
        .and_then(|s| {
            let s = s
                .lines()
                .find(|l| l.trim().starts_with("private_key="))
                .unwrap_or(s.trim());
            let s = s.trim();
            let s = s.strip_prefix("private_key=").unwrap_or(s);
            license::parse_key_hex(s).map_err(|e| format!("{path}: {e}"))
        })
}

fn keys(args: &[String], name: &str) -> Result<Vec<[u8; KEY_LEN]>, String> {
    flag(args, name)
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|k| license::parse_key_hex(k).map_err(|e| format!("{name}: {e}")))
        .collect()
}

/// Load every installed pack that provides the forensics kind from the app data folder.
fn load_packs(app: &Path, now: i64) -> Result<Entitlement, String> {
    let st = license_cmd::status_in(&app.join("licence"), now, Product::Desktop)
        .map_err(|e| e.to_string())?;
    let device = license::machine::this_machine_hash().unwrap_or_default();
    match forensics::load_installed(app, &st, &device, now) {
        Ok(true) => Ok(st.entitlement),
        Ok(false) => Err("no installed pack provides pro-incident-forensics".into()),
        Err(e) => Err(e.to_string()),
    }
}

fn run_cmd(args: &[String]) -> ExitCode {
    let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let now = now_unix();
    let dev = has_flag(args, "--dev") && cfg!(debug_assertions);
    let ent = if dev {
        Entitlement {
            licensee: "(dev)".into(),
            tier: license::Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
            arksa: false,
        }
    } else if let Some(app) = flag(args, "--app-data") {
        match load_packs(Path::new(&app), now) {
            Ok(e) => e,
            Err(e) => return fail(format!("the forensics Pro pack is not available: {e}")),
        }
    } else {
        match flag(args, "--license") {
            Some(t) => match license_cmd::verify_token_for(&t, now, Product::Cli) {
                Ok(e) => e,
                Err(e) => return fail(format!("license invalid: {e}")),
            },
            None => license_cmd::stored_or_evaluation(now, Product::Cli),
        }
    };
    if let Err(e) = license::check(&ent, forensics::KIND, now) {
        return fail(e);
    }
    if !dev && !forensics::is_loaded() {
        return fail(
            "incident forensics comes in a Pro pack: install it with `kshana-pro packs install` and pass --app-data <dir>",
        );
    }
    let mut scn = match IncidentScenario::from_file(Path::new(file)) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    if let Some(v) = flag(args, "--variant") {
        scn.variant = match v.as_str() {
            "standard" => Variant::Standard,
            "claim-grade" => Variant::ClaimGrade,
            other => {
                return fail(format!(
                    "--variant must be standard or claim-grade, got '{other}'"
                ))
            }
        };
    }
    let Some(out) = flag(args, "--out") else {
        return fail("forensics run needs --out <dir>");
    };
    let Some(kf) = flag(args, "--key-file") else {
        return fail("forensics run needs --key-file <file> (from `kshana-pro dossier keygen`)");
    };
    let Some(signer) = flag(args, "--signer") else {
        return fail("forensics run needs --signer <name>");
    };
    let seed = match read_seed(&kf) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    let req = bundle::BundleRequest {
        scenario: &scn,
        out: &PathBuf::from(&out),
        utc: &utc,
        seed: &seed,
        signer: &signer,
    };
    match bundle::build_bundle(&req) {
        Ok(o) => {
            println!("{}", o.report.verdict.sentence);
            println!(
                "wrote {} ({} finding(s), status {}), signed by {signer} (key {})",
                o.dir.display(),
                o.report.findings.len(),
                o.report.status,
                license::key_id(&license::public_key_for(&seed))
            );
            if o.report.status == "claim-grade-checklist-not-met"
                && !has_flag(args, "--allow-incomplete")
            {
                eprintln!("claim-grade checklist not met: see report.json `checklist`");
                return ExitCode::from(3);
            }
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn review_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let need = |n: &str| flag(args, n).ok_or_else(|| format!("forensics review needs {n}"));
    let r = (|| -> Result<PathBuf, String> {
        let statement =
            std::fs::read_to_string(need("--statement-file")?).map_err(|e| e.to_string())?;
        let seed = read_seed(&need("--key-file")?)?;
        let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
        bundle::add_review(
            Path::new(dir),
            &bundle::ReviewInput {
                reviewer_name: &need("--reviewer")?,
                qualification: &need("--qualification")?,
                organisation: &need("--organisation")?,
                independence_statement: &need("--independence")?,
                decision: &need("--decision")?,
                statement: &statement,
                reviewed_utc: &utc,
                seed: &seed,
            },
        )
        .map_err(|e| e.to_string())
    })();
    match r {
        Ok(p) => {
            println!(
                "wrote {}; the issuer now re-signs the bundle: kshana-pro dossier sign {dir} --signer <issuer> --key-file <issuer key>",
                p.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn verify_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let (trust, rtrust) = match (keys(args, "--trust"), keys(args, "--reviewer-trust")) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => return fail(e),
    };
    match bundle::verify_bundle(Path::new(dir), &trust, &rtrust) {
        Ok(v) => {
            println!(
                "case:      {} ({}, status {})",
                v.case_id, v.variant, v.status
            );
            println!(
                "signer:    {} at {} (key {})",
                v.signer, v.signed_utc, v.key_id
            );
            println!(
                "signature: {}",
                if v.signature_ok { "valid" } else { "INVALID" }
            );
            println!(
                "key:       {}",
                match v.trusted {
                    Some(true) => "trusted (pinned with --trust)",
                    Some(false) => "NOT TRUSTED",
                    None => "not pinned: pass --trust <public key> obtained from the issuer by another route",
                }
            );
            println!(
                "files:     {} signed; {} input(s) match the report",
                v.files, v.inputs_checked
            );
            println!(
                "evidence:  {} open evidence pack(s) verify",
                v.evidence_packs_ok
            );
            match (&v.review_ok, &v.reviewer) {
                (None, _) => println!("review:    none"),
                (Some(ok), r) => println!(
                    "review:    {} by {} ({})",
                    if *ok { "valid" } else { "NOT VALID" },
                    r.as_deref().unwrap_or("?"),
                    v.review_decision.as_deref().unwrap_or("")
                ),
            }
            for p in &v.problems {
                println!("problem:   {p}");
            }
            if v.passed() {
                println!(
                    "VERIFIED{}",
                    if v.claim_grade_complete() {
                        " (claim-grade, reviewed)"
                    } else {
                        ""
                    }
                );
                ExitCode::SUCCESS
            } else {
                println!("NOT VERIFIED");
                ExitCode::FAILURE
            }
        }
        Err(e) => fail(e),
    }
}

/// `args`: the arguments after `forensics`.
pub fn run(rest: &[String]) -> ExitCode {
    match rest.first().map(String::as_str) {
        Some("run") => run_cmd(rest),
        Some("review") => review_cmd(rest),
        Some("verify") => verify_cmd(rest),
        Some("pack-payload") => {
            let Some(dir) = rest.get(1) else {
                return fail(USAGE);
            };
            for (name, bytes) in forensics::pack_payload_files() {
                let p = Path::new(dir).join(&name);
                if let Some(parent) = p.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return fail(e);
                    }
                }
                if let Err(e) = std::fs::write(&p, bytes) {
                    return fail(format!("{}: {e}", p.display()));
                }
                println!("wrote {}", p.display());
            }
            ExitCode::SUCCESS
        }
        _ => fail(USAGE),
    }
}
