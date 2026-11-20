// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Evidence infrastructure commands (research 2026-11-20 gaps, E25):
//!
//! * `kshana-pro obs-provenance <keygen|sign|verify|keyring-add>`: signed feeder provenance
//!   on `kshana.obs/1` batches (B2501, [`crate::obs_provenance`]);
//! * `kshana-pro longterm <key-genesis|key-rotate|request|add-token|stamp|history|verify>`:
//!   long-term records with key histories and archive time-stamp chains (B2503,
//!   [`crate::longterm`]).
//!
//! Signing a batch, recording keys and adding stamps need a licence tier that unlocks the
//! feature; keygen, request, keyring-add and verify need none.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use crate::license::{self, KEY_LEN};
use crate::longterm::{self, KeyHistory};
use crate::obs_provenance::{self as prov, FeederKey, Keyring};

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix, utc_iso8601_now};

pub const PROVENANCE_USAGE: &str = "usage: kshana-pro obs-provenance <command>\n\
    keygen\n\
        New feeder signing key pair (free; keep the private key on the feeder)\n\
    sign <batch.jsonl> --feeder <id> --key-file <file> [--seq <n>] [--prev <previous batch>] [--lat <deg> --lon <deg> [--height <m>]] [--utc <iso>] [--license <token>] [--dev]\n\
        Sign a kshana.obs/1 batch: writes <batch>.prov.json beside it, the batch unchanged (Pro)\n\
    verify <batch.jsonl|dir> --keyring <keyring.json> [--require-signed] [--json]\n\
        Check batches against their sidecars and the feeder keyring, and each feeder's sequence (free)\n\
    keyring-add <keyring.json> --feeder <id> --public-key <hex> --valid-from <iso> [--valid-until <iso>]\n\
        Add a feeder key to a keyring, creating the file if needed (free)";

pub const LONGTERM_USAGE: &str = "usage: kshana-pro longterm <command>\n\
    key-genesis --key-file <file> --holder <name> --valid-from <iso> --out <history.json> [--license <token>] [--dev]\n\
        Start the signer's key history with its first key (Pro)\n\
    key-rotate <history.json> --from-key-file <old> --to-key-file <new> --effective <iso> [--reason scheduled|compromise] [--license <token>] [--dev]\n\
        Record a key rotation, signed by both keys (Pro)\n\
    request <dir> --out <request.tsq>\n\
        The RFC 3161 request for the next archive stamp of a signed directory (free)\n\
    add-token <dir> --token <token.tsr> --tsa-anchor <root.pem|.der> [--chain <certs.pem>] [--tsa-name <name>] [--history <history.json>] [--license <token>] [--dev]\n\
        Add an authority's token as the next archive stamp; the first needs --history (Pro)\n\
    stamp <dir> --tsa-url <url> --tsa-anchor <root> [--chain <certs.pem>] [--tsa-name <name>] [--history <history.json>] [--license <token>] [--dev]\n\
        Ask a time-stamp authority over HTTP (system curl), then add the token (Pro)\n\
    history <dir> --history <history.json> [--license <token>] [--dev]\n\
        Replace the record's key history with an extension of it, after a rotation (Pro)\n\
    verify <dir> --trust <key hex>[,...] [--tsa-anchor <file>[,...]] [--tsa-fingerprint <sha256 hex>[,...]] [--at <iso>] [--json]\n\
        Check the dossier, the key history and every archive stamp, offline (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn positional(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

fn num(args: &[String], name: &str) -> Result<Option<f64>, String> {
    flag(args, name)
        .map(|s| {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("{name}: not a number"))
        })
        .transpose()
}

fn keygen() -> ExitCode {
    use rand::RngCore;
    let mut seed = [0u8; KEY_LEN];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let pk = license::public_key_for(&seed);
    eprintln!("[new signing key: store the private key in a file readable only by its holder; publish the public key]");
    println!("private_key={}", hex::encode(seed));
    println!("public_key={}", hex::encode(pk));
    println!("key_id={}", license::key_id(&pk));
    ExitCode::SUCCESS
}

// ── obs-provenance ────────────────────────────────────────────────────────────

pub fn provenance(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("keygen") => keygen(),
        Some("sign") => match provenance_sign(args) {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Some("verify") => provenance_verify(args),
        Some("keyring-add") => match keyring_add(args) {
            Ok(msg) => {
                println!("{msg}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        _ => fail(PROVENANCE_USAGE),
    }
}

fn provenance_sign(args: &[String]) -> Result<String, String> {
    let batch = positional(args).ok_or(PROVENANCE_USAGE)?;
    gate(args, prov::KIND)?;
    let (Some(feeder), Some(kf)) = (flag(args, "--feeder"), flag(args, "--key-file")) else {
        return Err("sign needs --feeder <id> and --key-file <file>".into());
    };
    let seed = read_seed(&kf)?;
    let bytes = std::fs::read(batch).map_err(|e| format!("{batch}: {e}"))?;
    let seq = flag(args, "--seq")
        .map(|s| s.parse::<u64>().map_err(|_| "--seq: not a whole number"))
        .transpose()?
        .unwrap_or(0);
    let prev = flag(args, "--prev")
        .map(|p| {
            std::fs::read(&p)
                .map(|b| {
                    use sha2::Digest;
                    hex::encode(sha2::Sha256::digest(b))
                })
                .map_err(|e| format!("{p}: {e}"))
        })
        .transpose()?;
    let declared = match (num(args, "--lat")?, num(args, "--lon")?) {
        (Some(lat), Some(lon)) => Some((lat, lon, num(args, "--height")?)),
        (None, None) => None,
        _ => return Err("--lat and --lon go together".into()),
    };
    let p = prov::sign_batch(
        &bytes,
        &seed,
        &prov::SignOptions {
            feeder_id: feeder,
            signed_utc: flag(args, "--utc").unwrap_or_else(utc_iso8601_now),
            seq,
            prev_batch_sha256: prev,
            declared_location: declared,
        },
    )?;
    let out = prov::sidecar_path(Path::new(batch));
    let text = serde_json::to_string_pretty(&p).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&out, text).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(format!(
        "signed {batch} as feeder {} (key {}, sequence {}): {}",
        p.statement.feeder_id,
        p.statement.key_id,
        p.statement.seq,
        out.display()
    ))
}

fn provenance_verify(args: &[String]) -> ExitCode {
    let Some(target) = positional(args) else {
        return fail(PROVENANCE_USAGE);
    };
    let Some(kr) = flag(args, "--keyring") else {
        return fail("verify needs --keyring <keyring.json>");
    };
    let keyring = match Keyring::from_file(Path::new(&kr)) {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    let files = match prov::batch_files(Path::new(target)) {
        Ok(f) if !f.is_empty() => f,
        Ok(_) => return fail(format!("{target}: no .jsonl batch")),
        Err(e) => return fail(e),
    };
    let mut results = Vec::new();
    let mut provs = Vec::new();
    for f in &files {
        let name = f.display().to_string();
        let bytes = match std::fs::read(f) {
            Ok(b) => b,
            Err(e) => return fail(format!("{name}: {e}")),
        };
        let side = prov::sidecar_path(f);
        let p = if side.exists() {
            match prov::read_sidecar(&side) {
                Ok(p) => Some(p),
                Err(e) => {
                    let mut v = prov::verify_batch(&name, &bytes, None, &keyring);
                    v.status = prov::Status::Rejected;
                    v.problems.push(e);
                    results.push(v);
                    provs.push(None);
                    continue;
                }
            }
        } else {
            None
        };
        results.push(prov::verify_batch(&name, &bytes, p.as_ref(), &keyring));
        provs.push(p);
    }
    let refs: Vec<Option<&prov::Provenance>> = provs.iter().map(Option::as_ref).collect();
    let seq_problems = prov::check_sequences(&results, &refs);
    let require = has_flag(args, "--require-signed");
    let ok = seq_problems.is_empty()
        && results.iter().all(|r| match r.status {
            prov::Status::Signed => true,
            prov::Status::Unsigned => !require,
            prov::Status::Rejected => false,
        });
    if has_flag(args, "--json") {
        let doc = serde_json::json!({
            "format": "kshana-obs-provenance-verification/1",
            "batches": results,
            "sequence_problems": seq_problems,
            "require_signed": require,
            "passed": ok,
        });
        println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
    } else {
        for r in &results {
            println!(
                "{}: {} (station {}, {} epochs{}{})",
                r.batch,
                r.status.as_str().to_uppercase(),
                r.station_id.as_deref().unwrap_or("?"),
                r.epochs,
                r.feeder_id
                    .as_deref()
                    .map(|f| format!(", feeder {f}"))
                    .unwrap_or_default(),
                r.seq.map(|s| format!(", sequence {s}")).unwrap_or_default(),
            );
            for p in &r.problems {
                println!("  problem: {p}");
            }
        }
        for p in &seq_problems {
            println!("sequence problem: {p}");
        }
        println!("{}", if ok { "VERIFIED" } else { "NOT VERIFIED" });
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn keyring_add(args: &[String]) -> Result<String, String> {
    let path = positional(args).ok_or(PROVENANCE_USAGE)?;
    let (Some(feeder), Some(pk), Some(from)) = (
        flag(args, "--feeder"),
        flag(args, "--public-key"),
        flag(args, "--valid-from"),
    ) else {
        return Err("keyring-add needs --feeder, --public-key and --valid-from".into());
    };
    let p = Path::new(path);
    let mut k = if p.exists() {
        Keyring::from_file(p)?
    } else {
        Keyring::default()
    };
    k.add(FeederKey {
        feeder_id: feeder.clone(),
        public_key: pk.trim().to_ascii_lowercase(),
        valid_from: from,
        valid_until: flag(args, "--valid-until"),
        revoked_since: None,
    })?;
    let text = serde_json::to_string_pretty(&k).map_err(|e| e.to_string())? + "\n";
    std::fs::write(p, text).map_err(|e| format!("{path}: {e}"))?;
    Ok(format!(
        "{path}: {} key(s); added one for feeder {feeder}",
        k.feeders.len()
    ))
}

// ── longterm ──────────────────────────────────────────────────────────────────

pub fn longterm(args: &[String]) -> ExitCode {
    let r = match args.first().map(String::as_str) {
        Some("key-genesis") => key_genesis(args),
        Some("key-rotate") => key_rotate(args),
        Some("request") => request(args),
        Some("add-token") => add_token(args, None),
        Some("stamp") => stamp(args),
        Some("history") => history(args),
        Some("verify") => return lt_verify(args),
        _ => return fail(LONGTERM_USAGE),
    };
    match r {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn write_history(path: &str, h: &KeyHistory) -> Result<(), String> {
    let text = serde_json::to_string_pretty(h).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{path}: {e}"))
}

fn key_genesis(args: &[String]) -> Result<String, String> {
    gate(args, longterm::KIND)?;
    let (Some(kf), Some(holder), Some(from), Some(out)) = (
        flag(args, "--key-file"),
        flag(args, "--holder"),
        flag(args, "--valid-from"),
        flag(args, "--out"),
    ) else {
        return Err("key-genesis needs --key-file, --holder, --valid-from and --out".into());
    };
    if Path::new(&out).exists() {
        return Err(format!("{out} exists; a history is started once"));
    }
    let h = KeyHistory::genesis(&read_seed(&kf)?, &holder, &from)?;
    write_history(&out, &h)?;
    Ok(format!(
        "{out}: key history started with key {}",
        h.events[0].key_id
    ))
}

fn key_rotate(args: &[String]) -> Result<String, String> {
    let path = positional(args).ok_or(LONGTERM_USAGE)?;
    gate(args, longterm::KIND)?;
    let (Some(from), Some(to), Some(eff)) = (
        flag(args, "--from-key-file"),
        flag(args, "--to-key-file"),
        flag(args, "--effective"),
    ) else {
        return Err("key-rotate needs --from-key-file, --to-key-file and --effective".into());
    };
    let mut h = KeyHistory::from_file(Path::new(path))?;
    let reason = flag(args, "--reason").unwrap_or_else(|| "scheduled".into());
    h.rotate(&read_seed(&from)?, &read_seed(&to)?, &eff, &reason)?;
    write_history(path, &h)?;
    let last = h.events.last().expect("rotated");
    Ok(format!(
        "{path}: key {} retired from {eff} ({reason}); key {} in service",
        last.from_key_id.as_deref().unwrap_or("?"),
        last.key_id
    ))
}

fn request(args: &[String]) -> Result<String, String> {
    let dir = positional(args).ok_or(LONGTERM_USAGE)?;
    let out = flag(args, "--out").ok_or("request needs --out <request.tsq>")?;
    let (_, digest) = longterm::next_preimage(Path::new(dir))?;
    std::fs::write(&out, longterm::timestamp_request(&digest))
        .map_err(|e| format!("{out}: {e}"))?;
    Ok(format!(
        "{out}: RFC 3161 request over SHA-256 {}; send it to the authority (e.g. curl -H 'Content-Type: application/timestamp-query' --data-binary @{out} <url>) and add the reply with `longterm add-token`",
        hex::encode(digest)
    ))
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{path}: {e}"))
}

fn add_token(args: &[String], token: Option<Vec<u8>>) -> Result<String, String> {
    let dir = positional(args).ok_or(LONGTERM_USAGE)?;
    gate(args, longterm::KIND)?;
    let token = match token {
        Some(t) => t,
        None => read_file(&flag(args, "--token").ok_or("add-token needs --token <file>")?)?,
    };
    let anchor_file = flag(args, "--tsa-anchor").ok_or("needs --tsa-anchor <root certificate>")?;
    let anchors = longterm::read_certificates(&read_file(&anchor_file)?)?;
    if anchors.len() != 1 {
        return Err("--tsa-anchor: give exactly one root certificate".into());
    }
    let chain = match flag(args, "--chain") {
        Some(c) => longterm::read_certificates(&read_file(&c)?)?,
        None => Vec::new(),
    };
    let history = flag(args, "--history")
        .map(|h| KeyHistory::from_file(Path::new(&h)))
        .transpose()?;
    let s = longterm::add_stamp(
        Path::new(dir),
        history,
        &token,
        &chain,
        &anchors[0],
        &flag(args, "--tsa-name").unwrap_or_default(),
    )?;
    Ok(format!(
        "archive stamp {} added ({} at {}); {}/{}",
        s.seq,
        s.tsa,
        s.gen_time,
        dir,
        longterm::LONGTERM_FILE
    ))
}

fn stamp(args: &[String]) -> Result<String, String> {
    let dir = positional(args).ok_or(LONGTERM_USAGE)?;
    gate(args, longterm::KIND)?;
    let url = flag(args, "--tsa-url").ok_or("stamp needs --tsa-url <url>")?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("--tsa-url must be an http(s) URL".into());
    }
    let (_, digest) = longterm::next_preimage(Path::new(dir))?;
    let req = longterm::timestamp_request(&digest);
    let mut child = std::process::Command::new("curl")
        .args([
            "-sS",
            "--fail",
            "--max-time",
            "60",
            "-H",
            "Content-Type: application/timestamp-query",
            "--data-binary",
            "@-",
            &url,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run curl: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&req).map_err(|e| format!("curl: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the time-stamp authority did not answer: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    add_token(args, Some(out.stdout))
}

fn history(args: &[String]) -> Result<String, String> {
    let dir = positional(args).ok_or(LONGTERM_USAGE)?;
    gate(args, longterm::KIND)?;
    let h = flag(args, "--history").ok_or("history needs --history <file>")?;
    let h = KeyHistory::from_file(Path::new(&h))?;
    let n = h.events.len();
    longterm::update_history(Path::new(dir), h)?;
    Ok(format!("{dir}: key history updated ({n} events)"))
}

fn iso(t: Option<i64>) -> String {
    t.map(|t| crate::forensics::inputs::iso(t as f64))
        .unwrap_or_else(|| "-".into())
}

fn lt_verify(args: &[String]) -> ExitCode {
    let Some(dir) = positional(args) else {
        return fail(LONGTERM_USAGE);
    };
    let r = (|| -> Result<longterm::VerifyOptions, String> {
        let mut anchors = Vec::new();
        for f in flag(args, "--tsa-anchor")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
        {
            anchors.extend(longterm::read_certificates(&read_file(f.trim())?)?);
        }
        let at_unix = match flag(args, "--at") {
            Some(a) => prov::parse_time(&a)?,
            None => now_unix(),
        };
        Ok(longterm::VerifyOptions {
            trust_keys: keys(args, "--trust")?,
            tsa_anchors: anchors,
            tsa_fingerprints: flag(args, "--tsa-fingerprint")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            at_unix,
        })
    })()
    .and_then(|o| longterm::verify(Path::new(dir), &o));
    let v = match r {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    if has_flag(args, "--json") {
        let mut j = serde_json::to_value(&v).unwrap_or_default();
        j["passed"] = serde_json::Value::Bool(v.passed());
        println!("{}", serde_json::to_string_pretty(&j).unwrap_or_default());
    } else {
        println!("signer:        {} (key {})", v.signer, v.key_id);
        println!("files:         {}", v.files);
        println!(
            "dossier:       {}",
            if v.dossier_ok {
                "intact, signature valid"
            } else {
                "NOT INTACT"
            }
        );
        println!(
            "key:           {}",
            if v.key_trusted {
                "in the key history, reached from a pinned key"
            } else {
                "NOT TRUSTED"
            }
        );
        println!("existed by:    {}", iso(v.existed_by_unix));
        for s in &v.stamps {
            println!(
                "stamp {}:       {} at {}, certificates until {}: {}",
                s.seq,
                s.tsa,
                iso(s.gen_time_unix),
                iso(s.not_after_unix),
                if s.pass {
                    "valid".to_string()
                } else {
                    format!("INVALID ({})", s.reason)
                }
            );
        }
        println!(
            "renew by:      {}{}",
            iso(v.renew_by_unix),
            if v.renew_soon {
                " (within a year: add a stamp)"
            } else {
                ""
            }
        );
        for p in &v.problems {
            println!("problem:       {p}");
        }
        println!(
            "{}",
            if v.passed() {
                "VERIFIED"
            } else {
                "NOT VERIFIED"
            }
        );
    }
    if v.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
