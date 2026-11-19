// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro dossier <keygen|sign|verify|migrate|osnma-status>` (roadmap F7; OSNMA status
//! block B1404). Signing and migrating need the `pro-signed-dossier` feature; verifying and
//! making an OSNMA status block need no licence.

use std::path::Path;
use std::process::ExitCode;

use crate::license::{self, Entitlement, Product, KEY_LEN};
use crate::signed_dossier;

use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

const USAGE: &str = "usage: kshana-pro dossier <command>\n\
    keygen                                         New dossier signing key pair (keep the private key secret)\n\
    sign <dir> --signer <name> --key-file <file> [--osnma-status <block.json>] [--utc <iso>] [--license <token>] [--dev]   Sign every file under <dir> (Pro)\n\
    verify <dir> [--trust <public key hex>[,<more>]]   Check every file and the signature, offline (no licence needed)\n\
    migrate <dir> --trust <old signer key hex> --signer <name> --key-file <file> [--osnma-status <block.json>] [--utc <iso>] [--license <token>] [--dev]\n\
        Re-sign a verified format-1 pack as format 2 with an OSNMA status block (default: not recorded) (Pro)\n\
    osnma-status <pages|stream.ubx> --out <block.json> [--merkle-root <hex>] [--public-key <id:type:hex>[,<more>]] [--reference-time <gst-s>]\n\
        Run the open OSNMA verifier and write the status block a signature can carry (free)\n\
    Every signing command (dossier sign, scorecards, forensics, timing, frameworks, exposure, ...) takes\n\
    --osnma-status <block.json>: the signature is then format 2 and carries the block (status, key id,\n\
    chain id, Merkle root, verifier version, freshness checked). Not a time or position authentication.";

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
            let (seed, signer, utc) = match signing(args) {
                Ok(x) => x,
                Err(e) => return fail(e),
            };
            match signed_dossier::sign(Path::new(dir), &seed, &signer, &utc) {
                Ok(p) => {
                    println!(
                        "signed {} as {signer} (key {}){}",
                        p.display(),
                        license::key_id(&license::public_key_for(&seed)),
                        osnma_note()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("migrate") => {
            let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let trust = match trust_keys(args) {
                Ok(t) => t,
                Err(e) => return fail(e),
            };
            if trust.is_empty() {
                return fail("dossier migrate needs --trust <the old signer's public key>: an unverified pack is never re-signed");
            }
            let (seed, signer, utc) = match signing(args) {
                Ok(x) => x,
                Err(e) => return fail(e),
            };
            let block = crate::osnma_evidence::ambient();
            match signed_dossier::migrate(
                Path::new(dir),
                &trust,
                &seed,
                &signer,
                &utc,
                block.as_ref(),
            ) {
                Ok(p) => {
                    println!(
                        "migrated {} to {} as {signer} (key {}); OSNMA status {}",
                        p.display(),
                        signed_dossier::FORMAT_V2,
                        license::key_id(&license::public_key_for(&seed)),
                        block.map_or("not_recorded".to_string(), |b| b.status)
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("osnma-status") => osnma_status(args),
        Some("verify") => {
            let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let trust = match trust_keys(args) {
                Ok(t) => t,
                Err(e) => return fail(e),
            };
            match signed_dossier::verify(Path::new(dir), &trust) {
                Ok(v) => {
                    println!("signer:    {}", v.signer);
                    println!("signed at: {}", v.utc);
                    println!("key id:    {}", v.key_id);
                    println!("files:     {}", v.files);
                    println!("format:    {}", v.format);
                    println!("osnma:     {}", osnma_line(v.osnma.as_ref()));
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

/// The signing seed, signer and time of a signing command, after the licence check.
fn signing(args: &[String]) -> Result<([u8; KEY_LEN], String, String), String> {
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
                Err(e) => return Err(format!("license invalid: {e}")),
            },
            None => license_cmd::stored_or_evaluation(now, Product::Cli),
        }
    };
    if !ent.allows("pro-signed-dossier", now) {
        return Err(format!(
            "signing a dossier requires the '{}' tier (verifying does not)",
            license::feature_min_tier("pro-signed-dossier").as_str()
        ));
    }
    let Some(signer) = flag(args, "--signer") else {
        return Err("dossier sign needs --signer <name>".into());
    };
    let Some(kf) = flag(args, "--key-file") else {
        return Err(
            "dossier sign needs --key-file <file> (from `kshana-pro dossier keygen`)".into(),
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
        Err(e) => return Err(format!("{kf}: {e}")),
    };
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    Ok((seed, signer, utc))
}

fn trust_keys(args: &[String]) -> Result<Vec<[u8; KEY_LEN]>, String> {
    let mut trust = Vec::new();
    for k in flag(args, "--trust")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.trim().is_empty())
    {
        trust.push(license::parse_key_hex(k).map_err(|e| format!("--trust: {e}"))?);
    }
    Ok(trust)
}

fn osnma_note() -> String {
    match crate::osnma_evidence::ambient() {
        Some(b) => format!("; format 2 with OSNMA status {}", b.status),
        None => String::new(),
    }
}

fn osnma_line(b: Option<&crate::osnma_evidence::OsnmaBlock>) -> String {
    match b {
        None => "not recorded (format 1)".into(),
        Some(b) if b.status == "not_recorded" => "not recorded (migrated pack)".into(),
        Some(b) => format!(
            "{} (key id {}, chain id {}, Merkle root {}, {}, freshness {})",
            b.status,
            b.key_id.map_or("-".into(), |k| k.to_string()),
            b.chain_id.map_or("-".into(), |c| c.to_string()),
            b.merkle_root.as_deref().unwrap_or("-"),
            b.verifier,
            if b.freshness_checked {
                "checked"
            } else {
                "NOT checked"
            }
        ),
    }
}

/// `dossier osnma-status <input> --out <block.json> ...`: run the open verifier this build
/// embeds and write the block.
fn osnma_status(args: &[String]) -> ExitCode {
    let Some(input) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail("dossier osnma-status needs --out <block.json>");
    };
    let root = match flag(args, "--merkle-root") {
        Some(h) => match hex::decode(h.trim())
            .ok()
            .and_then(|b| <[u8; 32]>::try_from(b).ok())
        {
            Some(r) => Some(r),
            None => return fail("--merkle-root must be 64 hex digits"),
        },
        None => None,
    };
    let keys: Vec<String> = flag(args, "--public-key")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .collect();
    let reference = match flag(args, "--reference-time").map(|t| t.parse::<u32>()) {
        Some(Ok(t)) => Some(t),
        Some(Err(_)) => return fail("--reference-time must be GST seconds"),
        None => None,
    };
    let raw = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => return fail(format!("{input}: {e}")),
    };
    match crate::osnma_evidence::OsnmaBlock::verify_input(raw, input, root, &keys, reference) {
        Ok(b) => {
            let text = serde_json::to_string_pretty(&b).unwrap_or_default() + "\n";
            if let Err(e) = std::fs::write(&out, text) {
                return fail(format!("{out}: {e}"));
            }
            eprintln!("{}", kshana::osnma::ADVISORY);
            println!("osnma:     {}", osnma_line(Some(&b)));
            println!("wrote {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
