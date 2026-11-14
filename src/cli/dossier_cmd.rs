// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro dossier <keygen|sign|verify>` (roadmap F7). Signing needs the
//! `pro-signed-dossier` feature; verifying needs no licence.

use std::path::Path;
use std::process::ExitCode;

use crate::license::{self, Entitlement, Product, KEY_LEN};
use crate::signed_dossier;

use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

const USAGE: &str = "usage: kshana-pro dossier <command>\n\
    keygen                                         New dossier signing key pair (keep the private key secret)\n\
    sign <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Sign every file under <dir> (Pro)\n\
    verify <dir> [--trust <public key hex>[,<more>]]   Check every file and the signature, offline (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("keygen") => {
            use rand::RngCore;
            let mut seed = [0u8; KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut seed);
            let pk = license::public_key_for(&seed);
            eprintln!("[new dossier signing key: store the private key in a file readable only by you; publish the public key to recipients]");
            println!("private_key={}", hex::encode(seed));
            println!("public_key={}", hex::encode(pk));
            println!("key_id={}", license::key_id(&pk));
            ExitCode::SUCCESS
        }
        Some("sign") => {
            let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let now = now_unix();
            let ent = if has_flag(args, "--dev") && cfg!(debug_assertions) {
                Entitlement {
                    licensee: "(dev)".into(),
                    tier: license::Tier::Enterprise,
                    expires_unix: 0,
                    features: vec![],
                    arksa: true,
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
            if !ent.allows("pro-signed-dossier", now) {
                return fail(format!(
                    "signing a dossier requires the '{}' tier (verifying does not)",
                    license::feature_min_tier("pro-signed-dossier").as_str()
                ));
            }
            let Some(signer) = flag(args, "--signer") else {
                return fail("dossier sign needs --signer <name>");
            };
            let Some(kf) = flag(args, "--key-file") else {
                return fail(
                    "dossier sign needs --key-file <file> (from `kshana-pro dossier keygen`)",
                );
            };
            let seed = match std::fs::read_to_string(&kf)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    let s = s.trim();
                    let s = s.strip_prefix("private_key=").unwrap_or(s);
                    license::parse_key_hex(s).map_err(|e| e.to_string())
                }) {
                Ok(s) => s,
                Err(e) => return fail(format!("{kf}: {e}")),
            };
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            match signed_dossier::sign(Path::new(dir), &seed, &signer, &utc) {
                Ok(p) => {
                    println!(
                        "signed {} as {signer} (key {})",
                        p.display(),
                        license::key_id(&license::public_key_for(&seed))
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("verify") => {
            let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let mut trust = Vec::new();
            for k in flag(args, "--trust")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.trim().is_empty())
            {
                match license::parse_key_hex(k) {
                    Ok(k) => trust.push(k),
                    Err(e) => return fail(format!("--trust: {e}")),
                }
            }
            match signed_dossier::verify(Path::new(dir), &trust) {
                Ok(v) => {
                    println!("signer:    {}", v.signer);
                    println!("signed at: {}", v.utc);
                    println!("key id:    {}", v.key_id);
                    println!("files:     {}", v.files);
                    println!(
                        "signature: {}",
                        if v.signature_ok { "valid" } else { "INVALID" }
                    );
                    println!(
                        "key:       {}",
                        match v.trusted {
                            Some(true) => "trusted (pinned with --trust)",
                            Some(false) => "NOT TRUSTED",
                            None => "not pinned: pass --trust <public key> obtained from the signer by another route",
                        }
                    );
                    for p in &v.problems {
                        println!("problem:   {p}");
                    }
                    if v.passed() {
                        println!("VERIFIED");
                        ExitCode::SUCCESS
                    } else {
                        println!("NOT VERIFIED");
                        ExitCode::FAILURE
                    }
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(USAGE),
    }
}
