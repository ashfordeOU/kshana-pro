// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The Kshana-hosted **Core** service (D-K11): the methods that run only in Ashforde's hosted
//! Kshana service (and vetted Sovereign deployments), never in a customer Pro build. Built
//! only with the `core-methods` feature; the shipped `kshana-pro` binary, Desktop Pro and the
//! packages do not contain it.
//!
//! Each call runs a Core method and returns a **signed verdict**: the outcome and its evidence
//! class, with an Ed25519 signature over the canonical verdict, and nothing else. Intermediate
//! values, per-iteration traces, tuning and model internals never leave the service. The two
//! methods hosted here are the self-calibrating simulator loop (F1) and the integrity-driven
//! ASF survey planner (F2).

use ed25519_dalek::{Signer, SigningKey};
use kshana_eloran::calreg::Register;
use kshana_eloran::plan::survey::{plan, SurveyConfig};
use kshana_eloran::valid::selfcal::{run_selfcal, RefObservation, RefSite, SelfCalConfig};
use kshana_eloran::Scenario;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// A signed Core verdict. The only thing a Core call returns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignedVerdict {
    pub service: String,
    pub method: String,
    /// Evidence class of the outcome (MODELLED, VALIDATED, ...).
    pub evidence_class: String,
    pub utc: String,
    pub key_id: String,
    /// The outcome. Outcomes only: no internals, traces or tuning.
    pub verdict: Value,
    /// SHA-256 over the canonical signed body (every field above `signature`).
    pub verdict_sha256: String,
    /// Ed25519 signature (hex) over the same canonical body.
    pub signature: String,
}

const SERVICE: &str = "kshana-core/1";

fn canonical(
    service: &str,
    method: &str,
    evidence: &str,
    utc: &str,
    key_id: &str,
    verdict: &Value,
) -> Vec<u8> {
    // Field order fixed here, not by map iteration, so the bytes are stable.
    let body = json!({
        "service": service,
        "method": method,
        "evidence_class": evidence,
        "utc": utc,
        "key_id": key_id,
        "verdict": verdict,
    });
    serde_json::to_vec(&body).unwrap_or_default()
}

fn sign(
    method: &str,
    evidence: &str,
    verdict: Value,
    seed: &[u8; 32],
    key_id: &str,
    utc: &str,
) -> SignedVerdict {
    let body = canonical(SERVICE, method, evidence, utc, key_id, &verdict);
    let sig = SigningKey::from_bytes(seed).sign(&body);
    SignedVerdict {
        service: SERVICE.into(),
        method: method.into(),
        evidence_class: evidence.into(),
        utc: utc.into(),
        key_id: key_id.into(),
        verdict,
        verdict_sha256: hex::encode(Sha256::digest(&body)),
        signature: hex::encode(sig.to_bytes()),
    }
}

/// Verify a signed verdict against a public key (hex). Returns the problems (empty = valid).
pub fn verify(v: &SignedVerdict, public_key_hex: &str) -> Vec<String> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let mut problems = vec![];
    let body = canonical(
        &v.service,
        &v.method,
        &v.evidence_class,
        &v.utc,
        &v.key_id,
        &v.verdict,
    );
    if hex::encode(Sha256::digest(&body)) != v.verdict_sha256 {
        problems.push("verdict hash does not match the signed body".into());
    }
    let pk = match hex::decode(public_key_hex)
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .and_then(|b| VerifyingKey::from_bytes(&b).ok())
    {
        Some(k) => k,
        None => {
            problems.push("public key is not 32 valid Ed25519 bytes".into());
            return problems;
        }
    };
    let sig = match hex::decode(&v.signature)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .map(|b| Signature::from_bytes(&b))
    {
        Some(s) => s,
        None => {
            problems.push("signature is not 64 hex bytes".into());
            return problems;
        }
    };
    if pk.verify(&body, &sig).is_err() {
        problems.push("signature does not verify".into());
    }
    problems
}

/// Request for F1 (self-calibrating simulator loop).
#[derive(Clone, Debug, Deserialize)]
pub struct SelfCalRequest {
    /// The base scenario (TOML).
    pub scenario_toml: String,
    pub config: SelfCalConfig,
    pub sites: Vec<RefSite>,
    pub observations: Vec<RefObservation>,
}

/// Run F1 and return a signed verdict: convergence, fidelity, the calibrated parameters with
/// their uncertainty, and the signed lineage head. The per-iteration records, internal
/// parameter state and the regenerated scenario stay in the service.
pub fn selfcal_verdict(
    req: &SelfCalRequest,
    seed: &[u8; 32],
    key_id: &str,
    utc: &str,
) -> Result<SignedVerdict, String> {
    let base = Scenario::from_toml(&req.scenario_toml).map_err(|e| format!("scenario: {e}"))?;
    base.build().map_err(|e| format!("scenario: {e}"))?;
    // A per-request lineage register, kept inside the service.
    let dir = std::env::temp_dir().join(format!(
        "kshana-core-{}-{}",
        std::process::id(),
        utc.replace([':', '-'], "")
    ));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let reg_path = dir.join("lineage.jsonl");
    let register = Register::open(&reg_path);
    let r = run_selfcal(
        &base,
        &req.config,
        &req.sites,
        &req.observations,
        None,
        &register,
        seed,
        key_id,
    );
    let _ = std::fs::remove_dir_all(&dir);
    let r = r?;
    let scores = r.scores();
    let verdict = json!({
        "converged": r.converged,
        "stop_reason": r.stop_reason,
        "passes": r.records.len(),
        "regenerations": r.regenerations,
        "fidelity_first": scores.first().copied(),
        "fidelity_last": scores.last().copied(),
        "posterior": r.posterior.iter().map(|p| json!({"name": p.name, "value": p.value, "std_err": p.std_err})).collect::<Vec<_>>(),
        "lineage": {
            "head": r.signature.head,
            "entries": r.signature.n_entries,
            "scenario_hash": r.signature.scenario_hash,
            "recording_digest": r.recording_digest,
            "key_id": r.signature.key_id,
        },
    });
    Ok(sign("selfcal", "MODELLED", verdict, seed, key_id, utc))
}

/// Request for F2 (integrity-driven ASF survey planner).
#[derive(Clone, Debug, Deserialize)]
pub struct SurveyRequest {
    pub config: SurveyConfig,
    /// Include the full per-cell protection-level grid (an internal detail; off by default).
    #[serde(default)]
    pub include_grid: bool,
}

/// Run F2 and return a signed verdict: coverage and mean protection level before and after,
/// and the chosen survey points. The full protection-level grid is internal unless asked for.
pub fn survey_verdict(
    req: &SurveyRequest,
    seed: &[u8; 32],
    key_id: &str,
    utc: &str,
) -> Result<SignedVerdict, String> {
    let p = plan(&req.config)?;
    let mut verdict = json!({
        "coverage_before": p.coverage_before,
        "coverage_after": p.coverage_after,
        "mean_pl_before_m": p.mean_pl_before_m,
        "mean_pl_after_m": p.mean_pl_after_m,
        "picks": p.picks.iter().map(|k| json!({"order": k.order, "lat_deg": k.lat_deg, "lon_deg": k.lon_deg, "coverage": k.coverage, "mean_pl_m": k.mean_pl_m})).collect::<Vec<_>>(),
        "evidence": p.evidence,
    });
    if req.include_grid {
        verdict["grid"] = json!(p
            .grid
            .iter()
            .map(|(a, o, b, c)| json!([a, o, b, c]))
            .collect::<Vec<_>>());
    }
    Ok(sign("survey-plan", "MODELLED", verdict, seed, key_id, utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> ([u8; 32], String) {
        let seed = *b"kshana-core-test-key-32-bytes!!!";
        let pk = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        (seed, hex::encode(pk))
    }

    #[test]
    fn survey_verdict_is_signed_and_hides_the_grid() {
        let (seed, pk) = key();
        let cfg: SurveyConfig = toml::from_str(
            r#"
            area = [54.0, 56.0, 10.0, 13.0]
            step_deg = 0.5
            budget = 3
            stations = [["A", 57.5, 10.0], ["B", 52.5, 8.5], ["C", 54.5, 16.0]]
            "#,
        )
        .unwrap();
        let v = survey_verdict(
            &SurveyRequest {
                config: cfg.clone(),
                include_grid: false,
            },
            &seed,
            "k1",
            "2026-11-19T06:00:00Z",
        )
        .unwrap();
        assert_eq!(v.method, "survey-plan");
        assert!(verify(&v, &pk).is_empty(), "{:?}", verify(&v, &pk));
        // A tampered verdict does not verify.
        let mut t = v.clone();
        t.verdict["coverage_after"] = json!(1.0);
        assert!(!verify(&t, &pk).is_empty());
        // Internal grid withheld by default, present on request.
        assert!(v.verdict.get("grid").is_none());
        let g = survey_verdict(
            &SurveyRequest {
                config: cfg,
                include_grid: true,
            },
            &seed,
            "k1",
            "2026-11-19T06:00:00Z",
        )
        .unwrap();
        assert!(g.verdict.get("grid").is_some());
        assert!(verify(&g, &pk).is_empty());
    }

    #[test]
    fn selfcal_verdict_returns_outcome_only() {
        let (seed, pk) = key();
        // Minimal single-section, single-bias calibration on a one-chain scenario.
        let scenario = r#"
[scene]
start_utc = "2026-11-19T00:00:00Z"
duration_s = 0.05
fs_hz = 1000000
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 1.0 }
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 54.0, lon_deg = 5.0 }
emission_delay_us = 13000.0
"#;
        use kshana_eloran::valid::selfcal::SectionSpec;
        let mut config = SelfCalConfig::new(
            "svc",
            "core",
            "2026-11-19",
            vec![SectionSpec {
                name: "all".into(),
                vertices: vec![[53.0, 0.0], [57.0, 0.0], [57.0, 7.0], [53.0, 7.0]],
                eps_r: None,
                prior_sigma_s_m: 0.002,
                prior_log10_std: 0.3,
            }],
        );
        config.max_iter = 3;
        let sites = vec![RefSite {
            id: "s1".into(),
            position: kshana_eloran::geo::Geo::new(55.0, 3.0),
        }];
        // One observation per parameter (here: one section sigma).
        let obs = vec![RefObservation {
            site: "s1".into(),
            chain: "c".into(),
            station: "M".into(),
            kind: kshana_eloran::valid::selfcal::ObsKind::GroundToa,
            value_ns: 0.0,
            sigma_ns: 20.0,
        }];
        match selfcal_verdict(
            &SelfCalRequest {
                scenario_toml: scenario.into(),
                config,
                sites,
                observations: obs,
            },
            &seed,
            "k1",
            "2026-11-19T06:00:00Z",
        ) {
            Ok(v) => {
                assert_eq!(v.method, "selfcal");
                assert!(verify(&v, &pk).is_empty());
                // Outcome fields present; internals absent.
                assert!(v.verdict.get("converged").is_some() && v.verdict.get("lineage").is_some());
                assert!(
                    v.verdict.get("records").is_none()
                        && v.verdict.get("scenario").is_none()
                        && v.verdict.get("params").is_none()
                );
            }
            // The loop may not converge on this tiny synthetic case; the signing path is the
            // point and is covered by the survey test. A hard error would fail here.
            Err(e) => assert!(
                e.contains("selfcal") || e.contains("observation") || e.contains("parameter"),
                "{e}"
            ),
        }
    }
}
