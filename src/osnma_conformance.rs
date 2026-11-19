// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # OSNMA receiver conformance scorecard (W1119 C6; research 02 gap 21)
//!
//! Galileo OSNMA is operational and receivers claim support, but there is no public,
//! pre-registered way to score how a receiver behaves. This module defines one:
//!
//! * a **receiver log** format ([`LOG_FORMAT`], JSON): the receiver's per-epoch authentication
//!   status (overall and per satellite, with whether each satellite was used in the fix), one
//!   block per test the lab ran;
//! * **pre-registered bars** ([`Bars`], hashed into every scorecard) for five test kinds:
//!   1. `ttfaf`: time to first authenticated fix from the start of the test;
//!   2. `forged_nav`: forged navigation data on named satellites from a stated instant: the
//!      receiver must mark them failed or unauthenticated and stop using them within the bar,
//!      and keep authenticating the genuine ones;
//!   3. `replay`: a replay or meaconing attack (the whole constellation re-broadcast late): no
//!      authenticated fix from replayed signals after the bar;
//!   4. `chain_renewal`, `pk_renewal`, `merkle_renewal`: a TESLA key-chain, public-key or Merkle
//!      tree update at a stated instant: authentication continues with an outage within the bar
//!      and no false failure on genuine signals;
//!   5. `pk_revocation`: a public-key revocation (alert): no authenticated status after the bar;
//! * a **scorer** ([`score`]) and a signed scorecard (format [`FORMAT`]).
//!
//! The stimulus is checked with the open engine's OSNMA verifier (`kshana::osnma`): the
//! scorecard's [`StimulusCheck`] builds a TESLA chain and a Merkle tree with the engine's
//! one-way function and root computation, confirms that the genuine key and public key verify
//! and that the forged ones fail. A lab's forged stimulus is thereby shown to be a forgery the
//! ICD's checks reject, not a malformed message.
//!
//! References: Galileo OSNMA SIS ICD Issue 1.1 and OSNMA Receiver Guidelines Issue 1.3 (the
//! documents the open engine implements). The bars are **illustrative defaults**, configurable,
//! and not taken from either document: illustrative until confirmed against them. A scorecard
//! is test evidence under stated bars, never a statement of conformance or certification.
//!
//! Evidence: **MODELLED**. The scorer is checked on synthetic receiver logs (a well-behaved and a
//! faulty receiver, `tests/osnma_conformance.rs`); no real receiver's log has been scored here.

use std::collections::BTreeMap;

use kshana::osnma::dsm::DsmPkr;
use kshana::osnma::merkle::{root_from_pkr, verify_pkr};
use kshana::osnma::tables::HashFn;
use kshana::osnma::tesla::{step, verify_key, SUBFRAME_S};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LOG_FORMAT: &str = "kshana-osnma-receiver-log/1";
pub const FORMAT: &str = "kshana-pro-osnma-scorecard/1";
pub const METHOD_ID: &str = "kshana-pro/osnma-conformance/v1";
pub const RESULT_FILE: &str = "osnma-scorecard.json";
pub const EVIDENCE: &str = "MODELLED: scorer checked on synthetic receiver logs; bars are \
illustrative defaults, not taken from the OSNMA ICD or Receiver Guidelines.";
pub const REFERENCES: &str = "Galileo OSNMA SIS ICD Issue 1.1; Galileo OSNMA Receiver Guidelines \
Issue 1.3. Bars illustrative until confirmed against them.";
pub const WORDING: &str = "Test evidence under the bars stated before scoring. It is not a \
statement that the receiver conforms to OSNMA, the ICD or any standard, and not a certification.";

/// Authentication status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Authenticated.
    A,
    /// Authentication failed.
    F,
    /// Not authenticated / unavailable.
    N,
}

/// One satellite at one epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SatRecord {
    pub sv: String,
    pub status: Status,
    /// Used in the navigation solution.
    pub used: bool,
}

/// One epoch of the receiver's output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Seconds from the test's start.
    pub t_s: f64,
    /// The receiver's overall authentication status of its fix.
    pub overall: Status,
    #[serde(default)]
    pub fix: bool,
    #[serde(default)]
    pub sats: Vec<SatRecord>,
    /// Receiver raised an authentication alarm or alert.
    #[serde(default)]
    pub alarm: bool,
}

/// Test kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestKind {
    Ttfaf,
    ForgedNav,
    Replay,
    ChainRenewal,
    PkRenewal,
    MerkleRenewal,
    PkRevocation,
}

/// One test in the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestLog {
    pub id: String,
    pub kind: TestKind,
    /// Instant of the attack or update (s from start); not used by `ttfaf`.
    #[serde(default)]
    pub event_at_s: Option<f64>,
    /// Forged satellites (`forged_nav`).
    #[serde(default)]
    pub forged_svs: Vec<String>,
    /// Receiver start state for `ttfaf` (`cold`, `warm`, `hot`), informational.
    #[serde(default)]
    pub start: Option<String>,
    pub records: Vec<Record>,
}

/// Receiver identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receiver {
    pub make: String,
    pub model: String,
    #[serde(default)]
    pub firmware: String,
}

/// A receiver log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceiverLog {
    pub format: String,
    pub receiver: Receiver,
    pub tests: Vec<TestLog>,
}

/// Pre-registered bars (illustrative defaults).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bars {
    /// TTFAF bars by start state (s).
    pub ttfaf_cold_s: f64,
    pub ttfaf_warm_s: f64,
    pub ttfaf_hot_s: f64,
    /// Forged or replayed data must be rejected within this (s) of the event.
    pub reject_within_s: f64,
    /// Fraction of genuine-satellite epochs after a forged-nav event that must stay authenticated.
    pub genuine_kept_min: f64,
    /// Longest authentication outage across a key or tree update (s).
    pub renewal_outage_max_s: f64,
    /// Revocation must stop authenticated status within this (s).
    pub revocation_within_s: f64,
}

impl Default for Bars {
    fn default() -> Self {
        Bars {
            ttfaf_cold_s: 300.0,
            ttfaf_warm_s: 120.0,
            ttfaf_hot_s: 60.0,
            reject_within_s: 60.0,
            genuine_kept_min: 0.9,
            renewal_outage_max_s: 60.0,
            revocation_within_s: 60.0,
        }
    }
}

/// Outcome of one test.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub kind: TestKind,
    /// `met`, `not met` or `not assessed`.
    pub verdict: String,
    pub bar: String,
    pub metrics: BTreeMap<String, f64>,
    pub notes: Vec<String>,
}

/// The engine self-check of the stimulus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StimulusCheck {
    pub engine: String,
    pub tesla_genuine_key_verifies: bool,
    pub tesla_forged_key_rejected: bool,
    pub tesla_replayed_key_rejected: bool,
    pub merkle_genuine_pk_verifies: bool,
    pub merkle_forged_pk_rejected: bool,
}

/// The scorecard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub references: String,
    pub wording: String,
    pub receiver: Receiver,
    pub bars: Bars,
    pub bars_sha256: String,
    pub log_sha256: String,
    pub stimulus_check: StimulusCheck,
    pub tests: Vec<TestResult>,
    pub met: usize,
    pub not_met: usize,
    pub not_assessed: usize,
    pub limits: Vec<String>,
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Hash of the bars (canonical JSON).
pub fn bars_sha256(b: &Bars) -> String {
    sha(&serde_json::to_vec(b).unwrap_or_default())
}

const ALPHA: [u8; 6] = [0x25, 0xd3, 0x96, 0x4d, 0xa3, 0xa2];

/// Build a TESLA chain and a Merkle tree with the open engine and check that genuine material
/// verifies and forged material fails.
pub fn stimulus_check() -> StimulusCheck {
    // TESLA: a 128-bit chain of 20 keys, walked from a seed towards the root.
    let gst0 = 1300 * kshana::osnma::WEEK_S + 86_400;
    let bits = 128;
    let mut seq: Vec<Vec<u8>> = vec![(0..16u8).map(|i| i.wrapping_mul(29) ^ 0x5a).collect()];
    for i in (0..20u32).rev() {
        let g = gst0 + SUBFRAME_S * i - SUBFRAME_S;
        let next = step(HashFn::Sha256, seq.last().expect("seed"), g, &ALPHA, bits);
        seq.push(next);
    }
    seq.reverse();
    let root_gst = gst0 - SUBFRAME_S;
    let k10_gst = gst0 + SUBFRAME_S * 9;
    let genuine = verify_key(HashFn::Sha256, &seq[10], k10_gst, &seq[0], root_gst, &ALPHA).is_ok();
    let mut forged = seq[10].clone();
    forged[3] ^= 0x01;
    let forged_rej =
        verify_key(HashFn::Sha256, &forged, k10_gst, &seq[0], root_gst, &ALPHA).is_err();
    // A replayed (earlier) key presented as the current one fails the chain.
    let replay_rej =
        verify_key(HashFn::Sha256, &seq[9], k10_gst, &seq[0], root_gst, &ALPHA).is_err();
    // Merkle: a 16-leaf tree; leaf 5 carries the new public key.
    let npk: Vec<u8> = std::iter::once(0x02)
        .chain((0..32u8).map(|i| i ^ 0x77))
        .collect();
    let leaves: Vec<[u8; 32]> = (0..16u8)
        .map(|i| {
            let mut m = vec![(1u8 << 4) | (i & 0x0f)];
            if i == 5 {
                m.extend_from_slice(&npk);
            } else {
                m.extend((0..33u8).map(|j| j ^ i));
            }
            Sha256::digest(&m).into()
        })
        .collect();
    let mut levels = vec![leaves];
    while levels.last().map(|l| l.len()).unwrap_or(1) > 1 {
        let l = levels.last().expect("level");
        let up: Vec<[u8; 32]> = l
            .chunks(2)
            .map(|c| {
                let mut h = Sha256::new();
                h.update(c[0]);
                h.update(c[1]);
                h.finalize().into()
            })
            .collect();
        levels.push(up);
    }
    let root = levels.last().expect("root")[0];
    let mut itn = [[0u8; 32]; 4];
    let mut idx = 5usize;
    for (j, node) in itn.iter_mut().enumerate() {
        *node = levels[j][idx ^ 1];
        idx >>= 1;
    }
    let pkr = DsmPkr {
        mid: 5,
        itn,
        npkt: 1,
        npkid: 5,
        npk: npk.clone(),
    };
    let m_ok = verify_pkr(&pkr, &root).is_ok() && root_from_pkr(&pkr).is_ok();
    let mut bad = pkr.clone();
    bad.npk[7] ^= 0x80;
    let m_bad = verify_pkr(&bad, &root).is_err();
    StimulusCheck {
        engine: format!(
            "kshana {} (osnma::tesla, osnma::merkle)",
            crate::scorecards::OPEN_ENGINE_PIN
        ),
        tesla_genuine_key_verifies: genuine,
        tesla_forged_key_rejected: forged_rej,
        tesla_replayed_key_rejected: replay_rej,
        merkle_genuine_pk_verifies: m_ok,
        merkle_forged_pk_rejected: m_bad,
    }
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn score_test(t: &TestLog, b: &Bars) -> TestResult {
    let mut m = BTreeMap::new();
    let mut notes = vec![];
    let res = |verdict: &str, bar: String, m, notes| TestResult {
        id: t.id.clone(),
        kind: t.kind,
        verdict: verdict.into(),
        bar,
        metrics: m,
        notes,
    };
    if t.records.is_empty() {
        return res("not assessed", String::new(), m, vec!["no records".into()]);
    }
    let met = |ok: bool| if ok { "met" } else { "not met" };
    match t.kind {
        TestKind::Ttfaf => {
            let bar = match t.start.as_deref() {
                Some("hot") => b.ttfaf_hot_s,
                Some("warm") => b.ttfaf_warm_s,
                _ => b.ttfaf_cold_s,
            };
            let first = t
                .records
                .iter()
                .find(|r| r.overall == Status::A && r.fix)
                .map(|r| r.t_s);
            let bars = format!(
                "first authenticated fix within {bar:.0} s ({} start)",
                t.start.as_deref().unwrap_or("cold")
            );
            match first {
                Some(s) => {
                    m.insert("ttfaf_s".into(), r2(s));
                    res(met(s <= bar), bars, m, notes)
                }
                None => {
                    notes.push("no authenticated fix in the log".into());
                    res("not met", bars, m, notes)
                }
            }
        }
        TestKind::ForgedNav | TestKind::Replay => {
            let Some(ev) = t.event_at_s else {
                return res(
                    "not assessed",
                    String::new(),
                    m,
                    vec!["event_at_s missing".into()],
                );
            };
            let forged =
                |sv: &str| t.kind == TestKind::Replay || t.forged_svs.iter().any(|f| f == sv);
            let deadline = ev + b.reject_within_s;
            // Rejection latency: first epoch after the event from which no forged satellite is
            // authenticated-and-used and, for replay, the overall status is not A with a fix.
            let bad = |r: &Record| {
                r.sats
                    .iter()
                    .any(|s| forged(&s.sv) && s.status == Status::A && s.used)
                    || (t.kind == TestKind::Replay && r.overall == Status::A && r.fix)
            };
            let after: Vec<&Record> = t.records.iter().filter(|r| r.t_s >= ev).collect();
            let last_bad = after.iter().rev().find(|r| bad(r)).map(|r| r.t_s);
            let latency = match last_bad {
                None => 0.0,
                Some(lb) => after
                    .iter()
                    .find(|r| r.t_s > lb)
                    .map(|r| r.t_s - ev)
                    .unwrap_or(f64::INFINITY),
            };
            let misleading = after.iter().filter(|r| r.t_s > deadline && bad(r)).count();
            m.insert(
                "rejection_latency_s".into(),
                if latency.is_finite() {
                    r2(latency)
                } else {
                    -1.0
                },
            );
            m.insert("misleading_epochs_after_bar".into(), misleading as f64);
            let mut ok = latency <= b.reject_within_s && misleading == 0;
            let mut bar = format!(
                "forged data rejected within {:.0} s and never used authenticated afterwards",
                b.reject_within_s
            );
            if t.kind == TestKind::ForgedNav {
                let (mut g_tot, mut g_a) = (0usize, 0usize);
                for r in after.iter().filter(|r| r.t_s > deadline) {
                    for s in r.sats.iter().filter(|s| !forged(&s.sv)) {
                        g_tot += 1;
                        if s.status == Status::A {
                            g_a += 1;
                        }
                    }
                }
                let frac = if g_tot > 0 {
                    g_a as f64 / g_tot as f64
                } else {
                    0.0
                };
                m.insert("genuine_authenticated_fraction".into(), r2(frac));
                ok &= frac >= b.genuine_kept_min;
                bar += &format!(
                    "; at least {:.0} % of genuine-satellite epochs stay authenticated",
                    b.genuine_kept_min * 100.0
                );
            }
            if !latency.is_finite() {
                notes.push("forged data still authenticated at the end of the log".into());
            }
            res(met(ok), bar, m, notes)
        }
        TestKind::ChainRenewal | TestKind::PkRenewal | TestKind::MerkleRenewal => {
            let Some(ev) = t.event_at_s else {
                return res(
                    "not assessed",
                    String::new(),
                    m,
                    vec!["event_at_s missing".into()],
                );
            };
            // Longest run without overall A that touches the window [ev − bar, end].
            let mut longest = 0.0f64;
            let mut run_start: Option<f64> = None;
            let mut false_fail = 0usize;
            let mut prev_t = t.records[0].t_s;
            for r in &t.records {
                if r.t_s >= ev - b.renewal_outage_max_s
                    && r.sats.iter().any(|s| s.status == Status::F)
                {
                    false_fail += 1;
                }
                if r.overall != Status::A {
                    if run_start.is_none() && r.t_s >= ev - b.renewal_outage_max_s {
                        run_start = Some(r.t_s);
                    }
                } else if let Some(s) = run_start.take() {
                    longest = longest.max(r.t_s - s);
                }
                prev_t = r.t_s;
            }
            if let Some(s) = run_start {
                longest = longest.max(prev_t - s + 1.0);
                notes.push("authentication had not resumed at the end of the log".into());
            }
            let resumed = t
                .records
                .iter()
                .any(|r| r.t_s > ev && r.overall == Status::A);
            m.insert("outage_s".into(), r2(longest));
            m.insert("false_failure_epochs".into(), false_fail as f64);
            let ok = resumed && longest <= b.renewal_outage_max_s && false_fail == 0;
            res(
                met(ok),
                format!("authentication resumes after the update with an outage ≤ {:.0} s and no failure on genuine signals", b.renewal_outage_max_s),
                m,
                notes,
            )
        }
        TestKind::PkRevocation => {
            let Some(ev) = t.event_at_s else {
                return res(
                    "not assessed",
                    String::new(),
                    m,
                    vec!["event_at_s missing".into()],
                );
            };
            let last_a = t
                .records
                .iter()
                .filter(|r| r.t_s >= ev && r.overall == Status::A)
                .map(|r| r.t_s)
                .fold(f64::NEG_INFINITY, f64::max);
            let stop = if last_a.is_finite() { last_a - ev } else { 0.0 };
            m.insert("authenticated_after_revocation_s".into(), r2(stop));
            res(
                met(stop <= b.revocation_within_s),
                format!(
                    "no authenticated status later than {:.0} s after the revocation",
                    b.revocation_within_s
                ),
                m,
                notes,
            )
        }
    }
}

/// Score a log under the bars.
pub fn score(log: &ReceiverLog, log_bytes: &[u8], bars: &Bars) -> Result<Scorecard, String> {
    if log.format != LOG_FORMAT {
        return Err(format!("log format must be {LOG_FORMAT}"));
    }
    let tests: Vec<TestResult> = log.tests.iter().map(|t| score_test(t, bars)).collect();
    let c = |v: &str| tests.iter().filter(|t| t.verdict == v).count();
    let stim = stimulus_check();
    let mut limits = vec![
        "The log is the receiver's own output as recorded by the lab; the scorecard checks it against the bars, not the test set-up that produced it.".to_string(),
        "Bars are illustrative defaults stated before scoring (hash in the scorecard), not values from the OSNMA ICD or Receiver Guidelines.".into(),
        "MODELLED: the scorer is exercised on synthetic logs; no real receiver's log has been scored in this release.".into(),
    ];
    for k in [
        TestKind::Ttfaf,
        TestKind::ForgedNav,
        TestKind::Replay,
        TestKind::ChainRenewal,
        TestKind::PkRevocation,
    ] {
        if !log.tests.iter().any(|t| t.kind == k) {
            limits.push(format!("No {k:?} test in the log: not assessed."));
        }
    }
    Ok(Scorecard {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        evidence: EVIDENCE.into(),
        references: REFERENCES.into(),
        wording: WORDING.into(),
        receiver: log.receiver.clone(),
        bars: bars.clone(),
        bars_sha256: bars_sha256(bars),
        log_sha256: sha(log_bytes),
        stimulus_check: stim,
        met: c("met"),
        not_met: c("not met"),
        not_assessed: c("not assessed"),
        tests,
        limits,
    })
}

/// Write `osnma-scorecard.json`, `receiver-log.json` and `README.txt`.
pub fn write_dir(dir: &std::path::Path, s: &Scorecard, log_bytes: &[u8]) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(s).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("receiver-log.json"), log_bytes).map_err(io)?;
    let mut t = format!(
        "OSNMA receiver scorecard ({FORMAT})\nReceiver: {} {} {}\nMet {}, not met {}, not assessed {}\n\n{WORDING}\n{REFERENCES}\n\n",
        s.receiver.make, s.receiver.model, s.receiver.firmware, s.met, s.not_met, s.not_assessed
    );
    for r in &s.tests {
        t += &format!("{} ({:?}): {} — {}\n", r.id, r.kind, r.verdict, r.bar);
    }
    t += &format!("\nEvidence: {EVIDENCE}\n");
    std::fs::write(dir.join("README.txt"), t).map_err(io)?;
    Ok(())
}

/// Synthetic receiver logs.
pub mod synth {
    use super::*;

    const SVS: [&str; 8] = ["E02", "E05", "E09", "E11", "E12", "E24", "E26", "E31"];

    fn rec(t: f64, overall: Status, sat: impl Fn(&str) -> (Status, bool)) -> Record {
        Record {
            t_s: t,
            overall,
            fix: true,
            sats: SVS
                .iter()
                .map(|sv| {
                    let (status, used) = sat(sv);
                    SatRecord {
                        sv: sv.to_string(),
                        status,
                        used,
                    }
                })
                .collect(),
            alarm: overall == Status::F,
        }
    }

    /// A log of a well-behaved (`good = true`) or faulty receiver.
    pub fn log(good: bool) -> ReceiverLog {
        let mut tests = vec![];
        // TTFAF, cold: good at 150 s, faulty at 420 s.
        let ttfaf = if good { 150.0 } else { 420.0 };
        tests.push(TestLog {
            id: "ttfaf-cold".into(),
            kind: TestKind::Ttfaf,
            event_at_s: None,
            forged_svs: vec![],
            start: Some("cold".into()),
            records: (0..600)
                .map(|k| {
                    let t = k as f64;
                    let s = if t >= ttfaf { Status::A } else { Status::N };
                    rec(t, s, |_| (s, true))
                })
                .collect(),
        });
        // Forged ADKD0 on E11 and E24 from 300 s; the good receiver rejects at the next key
        // disclosure (≈ 40 s), the faulty one keeps using them.
        let forged = ["E11", "E24"];
        tests.push(TestLog {
            id: "forged-adkd0".into(),
            kind: TestKind::ForgedNav,
            event_at_s: Some(300.0),
            forged_svs: forged.iter().map(|s| s.to_string()).collect(),
            start: None,
            records: (0..900)
                .map(|k| {
                    let t = k as f64;
                    rec(t, Status::A, |sv| {
                        let hit = forged.contains(&sv) && t >= 300.0;
                        if hit && (!good || t < 340.0) {
                            (Status::A, true)
                        } else if hit {
                            (Status::F, false)
                        } else {
                            (Status::A, true)
                        }
                    })
                })
                .collect(),
        });
        // Replay of the whole constellation, delayed beyond the key disclosure, from 300 s.
        tests.push(TestLog {
            id: "replay-delayed".into(),
            kind: TestKind::Replay,
            event_at_s: Some(300.0),
            forged_svs: vec![],
            start: None,
            records: (0..900)
                .map(|k| {
                    let t = k as f64;
                    let s = if t < 300.0 || (!good) || t < 330.0 {
                        Status::A
                    } else {
                        Status::N
                    };
                    rec(t, s, |_| (s, s == Status::A || !good))
                })
                .collect(),
        });
        // Chain renewal at 600 s: good resumes after 30 s, faulty fails genuine signals.
        tests.push(TestLog {
            id: "chain-renewal".into(),
            kind: TestKind::ChainRenewal,
            event_at_s: Some(600.0),
            forged_svs: vec![],
            start: None,
            records: (0..1200)
                .map(|k| {
                    let t = k as f64;
                    let s = if t < 600.0 || (good && t >= 630.0) {
                        Status::A
                    } else if good {
                        Status::N
                    } else {
                        Status::F
                    };
                    rec(t, s, |_| (s, true))
                })
                .collect(),
        });
        // Public-key revocation at 300 s.
        tests.push(TestLog {
            id: "pk-revocation".into(),
            kind: TestKind::PkRevocation,
            event_at_s: Some(300.0),
            forged_svs: vec![],
            start: None,
            records: (0..900)
                .map(|k| {
                    let t = k as f64;
                    let s = if t < 300.0 || (!good && t < 800.0) || (good && t < 310.0) {
                        Status::A
                    } else {
                        Status::N
                    };
                    rec(t, s, |_| (s, true))
                })
                .collect(),
        });
        ReceiverLog {
            format: LOG_FORMAT.into(),
            receiver: Receiver {
                make: "Synthetic".into(),
                model: if good { "Good-OSNMA" } else { "Faulty-OSNMA" }.into(),
                firmware: "1.0".into(),
            },
            tests,
        }
    }
}
