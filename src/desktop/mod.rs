// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Desktop Pro functions (roadmap 0.42 U6, U19, U27, U28)
//!
//! What Kshana Desktop Pro asks of the Pro crate beyond running scenarios, as plain functions
//! with no window toolkit, so they are tested here and the app's bridge only forwards to
//! [`call`] (`engine_call` names starting `pro_`). Every answer is JSON.
//!
//! | Name | Arguments | Gate | What it does |
//! |---|---|---|---|
//! | `pro_pack_verify` | dir, [trusted key hex] | free | verify an incident forensics bundle or any signed pack, offline |
//! | `pro_incident_replay` | dir | `pro-incident-replay` | the bundle verified, then its tracks (reported and estimated), findings, area and verdict for the replay on the map |
//! | `pro_sign_files` | dir, {name: text}, signer | `pro-signed-pack` | write a review, report or run export into `dir` and sign it with this device's evidence key |
//! | `pro_sign_pack` | dir, signer | `pro-signed-pack` | sign an existing export directory with this device's evidence key |
//! | `pro_evidence_key` | | free | this device's evidence public key (what a recipient pins) |
//! | `pro_service_connect` | base URL, credential | free | remember the Kshana Pro service and the credential (API key or SSO token) |
//! | `pro_service_status`, `pro_service_disconnect` | | free | the connection (never the credential), or forget it |
//! | `pro_service` | method, path, [body] | the server's | one request to the connected service |
//!
//! A signed pack is the signed-dossier format ([`crate::signed_dossier`]): a recipient with no
//! licence verifies it with `kshana-pro dossier verify --trust <key>` or the standalone
//! `scripts/verify-dossier-signature.py`. Each pack carries `pack.json` naming the licensee,
//! the licence token id and the app; the evidence key is made on this device on first use and
//! never leaves it (`<licence home>/evidence-signing.seed`, owner-only).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::license::{self, Entitlement, KEY_LEN};
use crate::signed_dossier;

pub mod service;

pub use service::{CurlTransport, Transport};

/// The feature that replays an incident.
pub const REPLAY_FEATURE: &str = "pro-incident-replay";
/// The feature that signs packs.
pub const SIGN_FEATURE: &str = "pro-signed-pack";
/// The replay document's format tag.
pub const REPLAY_FORMAT: &str = "kshana-pro-incident-replay/1";
/// The pack description file every signed pack carries.
pub const PACK_FILE: &str = "pack.json";
/// The device's evidence key file, in the licence home.
pub const KEY_FILE: &str = "evidence-signing.seed";

/// Run one Desktop Pro function with the system `curl` for the service. `None`: `name` is not
/// one of these (the bridge then answers as before).
pub fn call(
    name: &str,
    args: &[Value],
    ent: &Entitlement,
    token_id: Option<&str>,
    at: i64,
    home: &Path,
) -> Option<Result<Value, String>> {
    call_with(name, args, ent, token_id, at, home, &CurlTransport)
}

/// [`call`] with a given transport for `pro_service` (tests use the server in process).
pub fn call_with(
    name: &str,
    args: &[Value],
    ent: &Entitlement,
    token_id: Option<&str>,
    at: i64,
    home: &Path,
    transport: &dyn Transport,
) -> Option<Result<Value, String>> {
    let text = |i: usize| -> Result<String, String> {
        args.get(i)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("argument {} must be text", i + 1))
    };
    let gate = |feature: &str| license::check(ent, feature, at).map_err(|e| e.to_string());
    Some(match name {
        "pro_pack_verify" => text(0).and_then(|d| {
            let trust = args.get(1).and_then(Value::as_str).unwrap_or("");
            verify_pack(Path::new(&d), trust)
        }),
        "pro_incident_replay" => gate(REPLAY_FEATURE)
            .and_then(|()| text(0))
            .and_then(|d| replay(Path::new(&d))),
        "pro_sign_pack" => gate(SIGN_FEATURE)
            .and_then(|()| Ok((text(0)?, text(1)?)))
            .and_then(|(d, s)| sign_dir(Path::new(&d), &s, ent, token_id, at, home)),
        "pro_sign_files" => gate(SIGN_FEATURE).and_then(|()| {
            let dir = PathBuf::from(text(0)?);
            let files = args
                .get(1)
                .and_then(Value::as_object)
                .ok_or("argument 2 must be an object of file names and texts")?;
            write_files(&dir, files)?;
            sign_dir(&dir, &text(2)?, ent, token_id, at, home)
        }),
        "pro_evidence_key" => device_key(home).map(|seed| {
            let pk = license::public_key_for(&seed);
            json!({"public_key": hex::encode(pk), "key_id": license::key_id(&pk)})
        }),
        "pro_service_connect" => {
            (|| service::connect(home, &text(0)?, &text(1)?))().map(|c| c.public_json())
        }
        "pro_service_status" => Ok(service::load(home)
            .map(|c| c.public_json())
            .unwrap_or(json!({"connected": false}))),
        "pro_service_disconnect" => {
            service::disconnect(home).map(|gone| json!({"disconnected": gone}))
        }
        "pro_service" => (|| {
            let c = service::load(home).ok_or("no Kshana Pro service is connected")?;
            let body = args.get(2).and_then(Value::as_str);
            c.request(transport, &text(0)?, &text(1)?, body)
        })(),
        _ => return None,
    })
}

// ------------------------------------------------------------------ verification (free)

/// Verify a forensics bundle or a signed pack in `dir`. `trust_hex`: a key the user pinned
/// (empty: the signature is checked against the key the pack names, and reported untrusted).
pub fn verify_pack(dir: &Path, trust_hex: &str) -> Result<Value, String> {
    let trust: Vec<[u8; KEY_LEN]> = if trust_hex.trim().is_empty() {
        Vec::new()
    } else {
        vec![license::parse_key_hex(trust_hex.trim()).map_err(|e| e.to_string())?]
    };
    let is_forensics = std::fs::read(dir.join("report.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|r| r["format"] == crate::forensics::FORMAT);
    if is_forensics {
        let v =
            crate::forensics::bundle::verify_bundle(dir, &trust, &[]).map_err(|e| e.to_string())?;
        return Ok(json!({
            "kind": "incident-forensics",
            "ok": v.signature_ok && v.problems.is_empty(),
            "trusted": v.trusted,
            "signer": v.signer, "signed_utc": v.signed_utc, "key_id": v.key_id, "files": v.files,
            "case_id": v.case_id, "variant": v.variant, "status": v.status,
            "inputs_checked": v.inputs_checked, "evidence_packs_ok": v.evidence_packs_ok,
            "review_ok": v.review_ok, "reviewer": v.reviewer,
            "problems": v.problems,
        }));
    }
    let v = signed_dossier::verify(dir, &trust).map_err(|e| e.to_string())?;
    let pack = std::fs::read(dir.join(PACK_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    Ok(json!({
        "kind": "signed-pack",
        "ok": v.signature_ok && v.problems.is_empty(),
        "trusted": v.trusted,
        "signer": v.signer, "signed_utc": v.utc, "key_id": v.key_id, "files": v.files,
        "pack": pack,
        "problems": v.problems,
    }))
}

// ------------------------------------------------------------------ incident replay (Pro)

fn unix(utc: &str) -> Option<f64> {
    crate::forensics::inputs::parse_utc(utc)
}

/// The replay document of a forensics bundle: verified first; a bundle that does not verify
/// is still replayed, with `verification.ok` false and its problems listed, so the user sees
/// why it should not be relied on.
pub fn replay(dir: &Path) -> Result<Value, String> {
    let verification = verify_pack(dir, "")?;
    if verification["kind"] != "incident-forensics" {
        return Err(
            "this directory is not an incident forensics bundle (no report.json of that format)"
                .into(),
        );
    }
    let r: Value = serde_json::from_slice(
        &std::fs::read(dir.join("report.json")).map_err(|e| format!("report.json: {e}"))?,
    )
    .map_err(|e| format!("report.json: {e}"))?;
    let tracks: Vec<Value> = r["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            let class = t["class"].as_str().unwrap_or("");
            let points: Vec<Value> = t["points"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    Some(json!({"t": unix(p["utc"].as_str()?)?, "lat": p["lat"], "lon": p["lon"], "flagged": p["flagged"]}))
                })
                .collect();
            json!({
                "source": t["source"],
                "class": class,
                // A receiver's own fix is the position it estimated; AIS and ADS-B carry what
                // the vessel or aircraft broadcast.
                "kind": if class == "measured" { "estimated" } else { "reported" },
                "points": points,
            })
        })
        .collect();
    let episodes: Vec<Value> = r["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            json!({
                "id": f["id"], "category": f["category"], "source": f["source"],
                "confidence": f["confidence"]["level"],
                "start": unix(f["onset"]["latest_utc"].as_str().unwrap_or("")),
                "start_earliest": unix(f["onset"]["earliest_utc"].as_str().unwrap_or("")),
                "end": unix(f["end"]["earliest_utc"].as_str().unwrap_or("")),
                "end_latest": unix(f["end"]["latest_utc"].as_str().unwrap_or("")),
                "about_subject": f["about_subject"], "supported": f["supported"],
                "statement": f["statement"],
                "position": f["position_at_onset"],
            })
        })
        .collect();
    Ok(json!({
        "format": REPLAY_FORMAT,
        "verification": verification,
        "case_id": r["incident"]["id"],
        "title": r["incident"]["title"],
        "window": {"start": unix(r["incident"]["start_utc"].as_str().unwrap_or("")),
                   "end": unix(r["incident"]["end_utc"].as_str().unwrap_or(""))},
        "status": r["status"],
        "verdict": r["verdict"],
        "tracks": tracks,
        "episodes": episodes,
        "area": r["area"],
        "incident_area": r["incident"]["area"],
        "label": r["label"],
        "validation_status": r["validation_status"],
        "limits": r["limits"],
    }))
}

// ------------------------------------------------------------------ signed packs (Pro)

/// This device's evidence signing seed, made on first use.
pub fn device_key(home: &Path) -> Result<[u8; KEY_LEN], String> {
    let p = home.join(KEY_FILE);
    if let Ok(t) = std::fs::read_to_string(&p) {
        let b = hex::decode(t.trim()).map_err(|_| format!("{} is damaged", p.display()))?;
        return b
            .try_into()
            .map_err(|_| format!("{} is damaged", p.display()));
    }
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let seed: [u8; KEY_LEN] = rand::random();
    write_private(&p, hex::encode(seed).as_bytes())?;
    Ok(seed)
}

/// Write a file readable by its owner only (on Unix).
pub(crate) fn write_private(p: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(p)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        f.write_all(bytes).map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(p, bytes).map_err(|e| format!("{}: {e}", p.display()))
    }
}

/// True for a relative file name inside a pack: no absolute path, no `..`, no backslash, not
/// the signature or pack description.
pub fn safe_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 200
        && !n.starts_with('/')
        && !n.contains('\\')
        && !n.contains(':')
        && n.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
        && n != signed_dossier::SIGNATURE_FILE
        && n != PACK_FILE
}

fn write_files(dir: &Path, files: &serde_json::Map<String, Value>) -> Result<(), String> {
    if files.is_empty() {
        return Err("a pack needs at least one file".into());
    }
    for (name, v) in files {
        if !safe_name(name) {
            return Err(format!("'{name}' is not a file name a pack may hold"));
        }
        let text = v
            .as_str()
            .ok_or_else(|| format!("{name}: the content must be text"))?;
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

fn sign_dir(
    dir: &Path,
    signer: &str,
    ent: &Entitlement,
    token_id: Option<&str>,
    at: i64,
    home: &Path,
) -> Result<Value, String> {
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    let seed = device_key(home)?;
    let pk = license::public_key_for(&seed);
    let utc = crate::forensics::inputs::iso(at as f64);
    let pack = json!({
        "format": "kshana-pro-signed-pack/1",
        "licensee": ent.licensee,
        "edition": ent.tier.as_str(),
        "licence_token": token_id,
        "app": "Kshana Desktop Pro",
        "created_utc": utc,
        "evidence_key_id": license::key_id(&pk),
        "note": "Verify without a licence: kshana-pro dossier verify <dir> --trust <public key>, or scripts/verify-dossier-signature.py",
    });
    std::fs::write(
        dir.join(PACK_FILE),
        serde_json::to_string_pretty(&pack).unwrap_or_default() + "\n",
    )
    .map_err(|e| e.to_string())?;
    signed_dossier::sign(dir, &seed, signer, &utc).map_err(|e| e.to_string())?;
    let v = verify_pack(dir, &hex::encode(pk))?;
    Ok(
        json!({"dir": dir.display().to_string(), "public_key": hex::encode(pk), "key_id": license::key_id(&pk), "verification": v}),
    )
}
