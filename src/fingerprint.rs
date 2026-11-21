// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Receiver behaviour fingerprint for the population benchmark (research 2026-11-20 gaps,
//! B2603, Pro side)
//!
//! A customer who has scored a receiver with Kshana Pro can give its behaviour back to the
//! hosted population benchmark (the service's `benchmark` verdict, contributor licences only) and
//! see where that firmware stands among others. This module builds what is given back: a
//! **per-firmware behaviour fingerprint** from the customer's own signed Pro scorecards.
//!
//! * Inputs are signed scorecard directories ([`crate::signed_dossier`]); each signature is
//!   verified against the keys the customer trusts, and a directory that fails is refused.
//!   Recognised scorecards: the receiver scorecard (`kshana-pro-receiver-scorecard/1`: RPCF level
//!   supported, misleading unwarned epochs, monitor detection, receiver flag and recovery rates)
//!   and the OSNMA scorecard (`kshana-pro-osnma-scorecard/1`: share of tests met, per test kind).
//!   Every scorecard must name the same maker, model and firmware.
//! * The fingerprint is a set of named features with the scorecards they came from (format,
//!   SHA-256, signer key) and a stable identifier (SHA-256 of identity and features).
//! * **Consent**: the customer states whether the fingerprint may be given back (`give_back`),
//!   for which scope (`population-benchmark` only), who granted it and when. Without consent the
//!   fingerprint stays local and no contribution file is written. The contribution carries the
//!   firmware identity, the features and a pseudonym of the customer (salted SHA-256), never the
//!   customer's name, sites, logs or scorecards.
//!
//! The pack is signed by the customer. MODELLED: features are taken from the scorecards as
//! supplied; no aggregate is computed here (the hosted service holds it).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::forensics::inputs::round;
use crate::license::KEY_LEN;

/// Pro feature.
pub const FEATURE: &str = "pro-receiver-fingerprint";
/// Format tags.
pub const FORMAT: &str = "kshana-pro-receiver-fingerprint/1";
pub const CONTRIBUTION_FORMAT: &str = "kshana-pro-fingerprint-contribution/1";
/// The only scope consent can be given for.
pub const SCOPE: &str = "population-benchmark";
/// Files.
pub const FINGERPRINT_FILE: &str = "fingerprint.json";
pub const CONTRIBUTION_FILE: &str = "contribution.json";
/// Label.
pub const EVIDENCE: &str = "Features taken from the customer's signed Pro scorecards as \
supplied (MODELLED where those scorecards are); no aggregate is computed here.";

/// The customer's consent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consent {
    pub give_back: bool,
    pub scope: String,
    pub granted_by: String,
    pub granted_utc: String,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// The customer, as named locally (never in the contribution).
    pub customer: String,
    /// Salt for the customer pseudonym (kept by the customer).
    pub pseudonym_salt: String,
    /// Signed scorecard directories.
    pub scorecards: Vec<String>,
    /// Public keys (hex) trusted to have signed them.
    pub trust: Vec<String>,
    pub consent: Consent,
}

/// Firmware identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Identity {
    pub maker: String,
    pub model: String,
    pub firmware: String,
}

/// One source scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SourceCard {
    pub format: String,
    pub file: String,
    pub sha256: String,
    pub signer_key_id: String,
}

/// The fingerprint.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fingerprint {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub identity: Identity,
    pub fingerprint_id: String,
    pub features: BTreeMap<String, f64>,
    pub sources: Vec<SourceCard>,
    pub consent: Consent,
    /// Whether a contribution is written (consent given for the benchmark scope).
    pub contributable: bool,
    pub verdict: String,
}

/// What is given back.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Contribution {
    pub format: &'static str,
    pub domain: &'static str,
    pub identity: Identity,
    pub fingerprint_id: String,
    pub features: BTreeMap<String, f64>,
    pub source_formats: Vec<String>,
    pub contributor: String,
    pub consent_scope: String,
    pub consent_utc: String,
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn rate(n: Option<u64>, d: Option<u64>) -> Option<f64> {
    match (n, d) {
        (Some(n), Some(d)) if d > 0 => Some(round(n as f64 / d as f64, 4)),
        _ => None,
    }
}

/// A scorecard's firmware identity and features.
type Features = (Identity, BTreeMap<String, f64>);

/// Features and identity of one scorecard JSON; `None` for a format not recognised.
fn features_of(v: &Value) -> Result<Option<Features>, String> {
    let s = |p: &Value| p.as_str().unwrap_or("").trim().to_string();
    let mut f = BTreeMap::new();
    match v["format"].as_str() {
        Some(crate::scorecards::FORMAT) => {
            let r = &v["receiver"];
            let id = Identity {
                maker: s(&r["maker"]),
                model: s(&r["model"]),
                firmware: s(&r["firmware"]),
            };
            let t = &v["totals"];
            let ev = t["events"].as_u64();
            if let Some(l) = v["levels"]["highest_supported"].as_u64() {
                f.insert("rx.rpcf_level".into(), l.min(4) as f64);
            }
            if let Some(m) = t["misleading_unwarned_epochs"].as_u64() {
                f.insert("rx.misleading_unwarned_epochs".into(), m as f64);
            }
            for (k, n) in [
                ("rx.monitor_detection_rate", "monitor_detected"),
                ("rx.receiver_flag_rate", "receiver_flagged"),
                ("rx.recovery_rate", "recovered"),
            ] {
                if let Some(x) = rate(t[n].as_u64(), ev) {
                    f.insert(k.into(), x);
                }
            }
            Ok(Some((id, f)))
        }
        Some(crate::osnma_conformance::FORMAT) => {
            let r = &v["receiver"];
            let id = Identity {
                maker: s(&r["make"]),
                model: s(&r["model"]),
                firmware: s(&r["firmware"]),
            };
            let (met, nm) = (v["met"].as_u64(), v["not_met"].as_u64());
            if let (Some(a), Some(b)) = (met, nm) {
                if a + b > 0 {
                    f.insert(
                        "osnma.met_share".into(),
                        round(a as f64 / (a + b) as f64, 4),
                    );
                }
            }
            for t in v["tests"].as_array().into_iter().flatten() {
                let (Some(k), Some(vd)) = (t["kind"].as_str(), t["verdict"].as_str()) else {
                    continue;
                };
                let val = match vd {
                    "met" => 1.0,
                    "not met" => 0.0,
                    _ => continue,
                };
                // The worst result per kind.
                let e = f.entry(format!("osnma.{k}")).or_insert(val);
                *e = f64::min(*e, val);
            }
            Ok(Some((id, f)))
        }
        _ => Ok(None),
    }
}

/// Build the fingerprint (and the contribution when consent allows). `base` resolves paths.
pub fn build(q: &Request, base: &Path) -> Result<(Fingerprint, Option<Contribution>), String> {
    for (n, v) in [
        ("customer", &q.customer),
        ("pseudonym_salt", &q.pseudonym_salt),
        ("consent.granted_by", &q.consent.granted_by),
    ] {
        if v.trim().is_empty() {
            return Err(format!("{n} must be stated"));
        }
    }
    if q.consent.scope != SCOPE {
        return Err(format!(
            "consent.scope must be '{SCOPE}': a fingerprint is given back for nothing else"
        ));
    }
    if crate::forensics::inputs::parse_utc(&q.consent.granted_utc).is_none() {
        return Err("consent.granted_utc is not a UTC time".into());
    }
    if q.trust.is_empty() {
        return Err("trust: name the keys that signed the scorecards".into());
    }
    let trust: Vec<[u8; KEY_LEN]> = q
        .trust
        .iter()
        .map(|k| crate::license::parse_key_hex(k).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let mut identity: Option<Identity> = None;
    let mut features = BTreeMap::new();
    let mut sources = vec![];
    for d in &q.scorecards {
        let dir = if Path::new(d).is_absolute() {
            d.into()
        } else {
            base.join(d)
        };
        let v = crate::signed_dossier::verify(&dir, &trust).map_err(|e| format!("{d}: {e}"))?;
        if !v.passed() {
            return Err(format!(
                "{d}: the signature does not verify against the trusted keys ({})",
                v.problems.join("; ")
            ));
        }
        let mut found = false;
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{d}: {e}"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
            .collect();
        names.sort();
        for p in names {
            let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let Ok(js) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            let Some((id, f)) = features_of(&js)? else {
                continue;
            };
            if id.maker.is_empty() || id.model.is_empty() || id.firmware.is_empty() {
                return Err(format!(
                    "{}: maker, model and firmware must all be named",
                    p.display()
                ));
            }
            match &identity {
                Some(i) if *i != id => {
                    return Err(format!(
                    "{}: scorecard is for {} {} {}, not {} {} {}: a fingerprint is per firmware",
                    p.display(),
                    id.maker,
                    id.model,
                    id.firmware,
                    i.maker,
                    i.model,
                    i.firmware
                ))
                }
                None => identity = Some(id),
                _ => {}
            }
            for (k, x) in f {
                if features.insert(k.clone(), x).is_some() {
                    return Err(format!(
                        "feature {k} given by two scorecards: supply one per kind"
                    ));
                }
            }
            sources.push(SourceCard {
                format: js["format"].as_str().unwrap_or("").into(),
                file: format!(
                    "{d}/{}",
                    p.file_name()
                        .map(|x| x.to_string_lossy())
                        .unwrap_or_default()
                ),
                sha256: sha(&bytes),
                signer_key_id: v.key_id.clone(),
            });
            found = true;
        }
        if !found {
            return Err(format!("{d}: no recognised Pro scorecard in the directory"));
        }
    }
    let identity = identity.ok_or("no scorecard given")?;
    let fid = sha(serde_json::to_string(&(&identity, &features))
        .map_err(|e| e.to_string())?
        .as_bytes());
    let contributable = q.consent.give_back;
    let contribution = contributable.then(|| Contribution {
        format: CONTRIBUTION_FORMAT,
        domain: "receiver-fingerprint",
        identity: identity.clone(),
        fingerprint_id: fid.clone(),
        features: features.clone(),
        source_formats: sources.iter().map(|s| s.format.clone()).collect(),
        contributor: crate::transport::pseudonym(&q.pseudonym_salt, &q.customer),
        consent_scope: q.consent.scope.clone(),
        consent_utc: q.consent.granted_utc.clone(),
    });
    let verdict = format!(
        "{} {} {}: {} feature(s) from {} scorecard(s); {}",
        identity.maker,
        identity.model,
        identity.firmware,
        features.len(),
        sources.len(),
        if contributable {
            "consent given: contribution written for the population benchmark"
        } else {
            "no consent: kept local, nothing to give back"
        }
    );
    Ok((
        Fingerprint {
            format: FORMAT,
            evidence: EVIDENCE,
            title: q.title.clone(),
            identity,
            fingerprint_id: fid,
            features,
            sources,
            consent: q.consent.clone(),
            contributable,
            verdict,
        },
        contribution,
    ))
}

/// HTML page.
pub fn html(f: &Fingerprint) -> String {
    crate::transport::page(
        &f.title,
        f.evidence,
        &f.verdict,
        &[format!(
            "Fingerprint {} of {} {} {}. Consent: give back {} ({}), granted by {} at {}.",
            f.fingerprint_id,
            f.identity.maker,
            f.identity.model,
            f.identity.firmware,
            f.consent.give_back,
            f.consent.scope,
            f.consent.granted_by,
            f.consent.granted_utc
        )],
        &[
            (
                "Features".into(),
                vec!["feature", "value"],
                f.features
                    .iter()
                    .map(|(k, v)| vec![k.clone(), format!("{v}")])
                    .collect(),
            ),
            (
                "Sources".into(),
                vec!["format", "file", "SHA-256", "signer key"],
                f.sources
                    .iter()
                    .map(|s| {
                        vec![
                            s.format.clone(),
                            s.file.clone(),
                            s.sha256.clone(),
                            s.signer_key_id.clone(),
                        ]
                    })
                    .collect(),
            ),
        ],
    )
}
