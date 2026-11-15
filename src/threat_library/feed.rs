// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The signed subscription feed of the threat library (roadmap 0.42, E2).
//!
//! A feed edition is a directory:
//!
//! ```text
//! feed.json              the manifest: edition, previous edition's manifest hash, every
//!                        scenario's id, version, SHA-256 and the edition it changed in
//! scenarios/<id>.toml    the scenario files
//! README.txt
//! signature.json         the dossier signature over every file (Ed25519)
//! ```
//!
//! Building needs the `pro-threat-library` feature; [`verify`] needs nothing: it checks the
//! signature, that `feed.json` lists exactly the scenario files present with their hashes,
//! that each scenario passes the library rules (provenance, licence notes, no source data),
//! and that nothing else is in the directory.

use std::path::Path;

use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ThreatScenario, LABEL};
use crate::license::KEY_LEN;

/// The manifest's format tag.
pub const FEED_FORMAT: &str = "kshana-pro-threat-feed/1";
/// The manifest's file name.
pub const MANIFEST: &str = "feed.json";

/// The previous edition a feed follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Previous {
    pub edition: String,
    /// SHA-256 of the previous edition's `feed.json` bytes.
    pub manifest_sha256: String,
}

/// One scenario of an edition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedEntry {
    pub id: String,
    pub version: u32,
    pub family: String,
    pub title: String,
    pub path: String,
    pub sha256: String,
    /// The edition in which this exact file first appeared.
    pub changed_in: String,
}

/// The manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedManifest {
    pub format: String,
    pub edition: String,
    pub published_utc: String,
    pub label: String,
    /// The library rule every edition keeps.
    pub rule: String,
    #[serde(default)]
    pub previous: Option<Previous>,
    pub scenarios: Vec<FeedEntry>,
}

/// The rule written into every manifest.
pub const RULE: &str = "No source data is redistributed: TEXBAT recordings, JammerTest data and \
any other source material are fetched by the user from the publisher; scenarios carry \
descriptions, parameters, provenance and licence notes only.";

fn is_edition(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7
        && b[4] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..].iter().all(u8::is_ascii_digit)
        && (1..=12).contains(&s[5..].parse::<u32>().unwrap_or(0))
}

/// Build the edition `edition` (`YYYY-MM`) of the feed from `scenarios` (id and file text)
/// into the new or empty directory `out`, following `previous` (the previous edition's
/// `feed.json` bytes) when given, and sign it with `seed` as `signer` at `utc`.
pub fn build(
    out: &Path,
    scenarios: &[(&str, &str)],
    edition: &str,
    previous: Option<&[u8]>,
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
) -> Result<FeedManifest, KshanaError> {
    let bad = |m: String| KshanaError::InvalidInput(format!("threat feed: {m}"));
    if !is_edition(edition) {
        return Err(bad(format!("edition '{edition}' is not YYYY-MM")));
    }
    let prev: Option<(FeedManifest, String)> = match previous {
        Some(bytes) => {
            let m: FeedManifest = serde_json::from_slice(bytes)
                .map_err(|e| bad(format!("previous feed.json: {e}")))?;
            if m.format != FEED_FORMAT {
                return Err(bad("previous feed.json has another format".into()));
            }
            if m.edition.as_str() >= edition {
                return Err(bad(format!(
                    "edition {edition} does not follow the previous edition {}",
                    m.edition
                )));
            }
            Some((m, hex::encode(Sha256::digest(bytes))))
        }
        None => None,
    };
    let io = |e: std::io::Error| bad(format!("{}: {e}", out.display()));
    if out.exists() && std::fs::read_dir(out).map_err(io)?.next().is_some() {
        return Err(bad(format!("{} is not empty", out.display())));
    }
    std::fs::create_dir_all(out.join("scenarios")).map_err(io)?;
    let mut entries = Vec::new();
    for (id, text) in scenarios {
        let s = ThreatScenario::parse(text)?;
        if s.id != *id {
            return Err(bad(format!("file '{id}' carries id '{}'", s.id)));
        }
        let sha = hex::encode(Sha256::digest(text.as_bytes()));
        let changed_in = prev
            .as_ref()
            .and_then(|(m, _)| m.scenarios.iter().find(|e| e.id == s.id && e.sha256 == sha))
            .map(|e| e.changed_in.clone())
            .unwrap_or_else(|| edition.to_string());
        let path = format!("scenarios/{}.toml", s.id);
        std::fs::write(out.join(&path), text).map_err(io)?;
        entries.push(FeedEntry {
            id: s.id.clone(),
            version: s.version,
            family: s.family.as_str().into(),
            title: s.title.clone(),
            path,
            sha256: sha,
            changed_in,
        });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    if entries.windows(2).any(|w| w[0].id == w[1].id) {
        return Err(bad("a scenario id is listed twice".into()));
    }
    let m = FeedManifest {
        format: FEED_FORMAT.into(),
        edition: edition.into(),
        published_utc: utc.into(),
        label: LABEL.into(),
        rule: RULE.into(),
        previous: prev.map(|(m, h)| Previous {
            edition: m.edition,
            manifest_sha256: h,
        }),
        scenarios: entries,
    };
    let json = serde_json::to_string_pretty(&m).map_err(|e| bad(e.to_string()))? + "\n";
    std::fs::write(out.join(MANIFEST), json).map_err(io)?;
    let changed = m
        .scenarios
        .iter()
        .filter(|e| e.changed_in == m.edition)
        .count();
    std::fs::write(
        out.join("README.txt"),
        format!(
            "Kshana Pro threat library, feed edition {}.\n{} scenarios, {} new or changed in this edition.\n\n{RULE}\n\n\
             Verify (no licence needed): kshana-pro threats feed verify <this directory> --trust <publisher key>\n",
            m.edition,
            m.scenarios.len(),
            changed
        ),
    )
    .map_err(io)?;
    crate::signed_dossier::sign(out, seed, signer, utc)?;
    Ok(m)
}

/// The result of verifying a feed edition.
#[derive(Debug, Clone, Default)]
pub struct FeedVerification {
    pub edition: String,
    pub scenarios: usize,
    pub signer: String,
    pub key_id: String,
    pub signature_ok: bool,
    pub trusted: Option<bool>,
    pub problems: Vec<String>,
}

impl FeedVerification {
    pub fn passed(&self) -> bool {
        self.signature_ok && self.trusted == Some(true) && self.problems.is_empty()
    }
}

/// Verify a feed edition directory against the publisher keys `trust`. When `previous` (the
/// previous edition's `feed.json` bytes) is given, the chain to it is checked too.
pub fn verify(
    dir: &Path,
    trust: &[[u8; KEY_LEN]],
    previous: Option<&[u8]>,
) -> Result<FeedVerification, KshanaError> {
    let sig = crate::signed_dossier::verify(dir, trust)?;
    let mut v = FeedVerification {
        signer: sig.signer.clone(),
        key_id: sig.key_id.clone(),
        signature_ok: sig.signature_ok,
        trusted: sig.trusted,
        problems: sig.problems.clone(),
        ..Default::default()
    };
    let raw = match std::fs::read(dir.join(MANIFEST)) {
        Ok(r) => r,
        Err(e) => {
            v.problems.push(format!("{MANIFEST}: {e}"));
            return Ok(v);
        }
    };
    let m: FeedManifest = match serde_json::from_slice(&raw) {
        Ok(m) => m,
        Err(e) => {
            v.problems.push(format!("{MANIFEST}: {e}"));
            return Ok(v);
        }
    };
    v.edition = m.edition.clone();
    v.scenarios = m.scenarios.len();
    if m.format != FEED_FORMAT || !is_edition(&m.edition) || m.rule != RULE {
        v.problems
            .push(format!("{MANIFEST}: unknown format, edition or rule"));
    }
    let mut listed = std::collections::BTreeSet::new();
    for e in &m.scenarios {
        listed.insert(e.path.clone());
        match std::fs::read(dir.join(&e.path)) {
            Ok(b) => {
                if hex::encode(Sha256::digest(&b)) != e.sha256 {
                    v.problems
                        .push(format!("{}: hash differs from {MANIFEST}", e.path));
                }
                match std::str::from_utf8(&b)
                    .map_err(|e| KshanaError::InvalidInput(e.to_string()))
                    .and_then(ThreatScenario::parse)
                {
                    Ok(s) if s.id == e.id && s.version == e.version => {}
                    Ok(_) => v
                        .problems
                        .push(format!("{}: id or version differs", e.path)),
                    Err(err) => v.problems.push(format!("{}: {err}", e.path)),
                }
            }
            Err(err) => v.problems.push(format!("{}: {err}", e.path)),
        }
    }
    for f in crate::signed_dossier::list_files(dir)? {
        let allowed = f.path == MANIFEST || f.path == "README.txt" || listed.contains(&f.path);
        if !allowed {
            v.problems.push(format!(
                "{}: not a feed file (a feed carries scenario files only, never data)",
                f.path
            ));
        }
    }
    if let Some(p) = previous {
        let want = hex::encode(Sha256::digest(p));
        let pm: Option<FeedManifest> = serde_json::from_slice(p).ok();
        match (&m.previous, pm) {
            (Some(prev), Some(pm))
                if prev.manifest_sha256 == want && prev.edition == pm.edition => {}
            _ => v
                .problems
                .push("the feed does not follow the previous edition given".into()),
        }
    }
    Ok(v)
}
