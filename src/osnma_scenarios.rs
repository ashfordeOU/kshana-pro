// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Pro OSNMA scenario pack (research 2026-11-20 gaps, B1402)
//!
//! The OSNMA scorecard ([`crate::osnma_conformance`]) scores a receiver log under bars. This
//! pack adds the scenarios a lab runs to produce such logs, each with the authentication state
//! a receiver is expected to report, subframe by subframe, and a **reference model** that
//! derives those states from the scenario's conditions, so that every expected table is
//! checked rather than asserted:
//!
//! | scenario | what it exercises |
//! |---|---|
//! | `startup-cold` | public key (DSM-PKR) and root key (DSM-KROOT) acquired before any tag verifies |
//! | `key-renewal` | TESLA chain renewal announced in advance: no outage |
//! | `chain-revocation` | chain revoked: nothing authenticated on it until the new root key |
//! | `subframe-loss` | three subframes lost: their data stay unauthenticated; the next key verifies across the gap by chain stepping |
//! | `replay-delayed` | replay delayed beyond the key disclosure: tags arrive after their key, rejected |
//! | `meaconing-short` | a 4 s relay: OSNMA authenticates the data; it does not detect a short-delay meaconer |
//! | `time-sync-loss` | the receiver's time uncertainty grows past the allowed bound: authentication stops |
//! | `time-sync-attack` | the receiver's clock pushed ahead so that tags look late: authentication denied |
//! | `pk-revocation` | public key revoked: nothing authenticated until a new key and root key |
//!
//! **Reference model** ([`expected_states`]; parameters in [`Model`], illustrative): the data of
//! subframe *k* are authenticated (`A`) when the subframe was received; the public key is known
//! and not revoked; the root key of the subframe's chain is known and the chain is not revoked;
//! the receiver's time uncertainty *U* is within `max_uncertainty_s`; the apparent age of the
//! signal (delay plus the receiver's clock error) plus *U* is less than the key disclosure delay
//! (one subframe, 30 s); and a later subframe is received while that chain and key are still
//! valid (its key verifies the earlier one by stepping the chain). Otherwise `N`. The open
//! engine's TESLA and Merkle functions check the cryptographic premises
//! ([`crypto_checks`]): a key verifies across lost subframes, a key of a revoked chain fails
//! against the new root, and the scorecard's stimulus check.
//!
//! References: Galileo OSNMA SIS ICD Issue 1.1 and OSNMA Receiver Guidelines Issue 1.3, as the
//! open engine implements them. The model's parameters are illustrative, not quoted from either
//! document. Not covered: DO-401A avionics cases and SBAS authentication (not public).
//! The pack describes test conditions for a lab's own simulator; it generates no signal.
//! Scores are test evidence under the stated model, never a statement that a receiver meets a
//! standard.
//! **MODELLED**: checked on synthetic receiver logs.

use std::collections::BTreeMap;

use kshana::osnma::tables::HashFn;
use kshana::osnma::tesla::{step, verify_key, SUBFRAME_S};
use serde::{Deserialize, Serialize};

use crate::osnma_conformance::{ReceiverLog, Status};

/// Pro feature.
pub const FEATURE: &str = "pro-osnma-scenario-pack";
/// Formats.
pub const PACK_FORMAT: &str = "kshana-pro-osnma-scenario-pack/1";
pub const FORMAT: &str = "kshana-pro-osnma-scenario-score/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: expected states from the pack's reference model with \
illustrative parameters; scorer checked on synthetic receiver logs.";
/// Wording.
pub const WORDING: &str = "Test evidence against the expected states of the stated reference \
model. Not a statement that a receiver meets OSNMA, the ICD or any standard.";

/// Reference-model parameters (illustrative).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Model {
    /// Key disclosure delay (s): one subframe.
    pub disclosure_s: f64,
    /// Largest time uncertainty the receiver may hold and still authenticate (s).
    pub max_uncertainty_s: f64,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            disclosure_s: SUBFRAME_S as f64,
            max_uncertainty_s: 15.0,
        }
    }
}

/// Conditions of one subframe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conditions {
    pub received: bool,
    /// Delay of the signal against true time (replay, meaconing), s.
    pub delay_s: f64,
    /// Receiver clock error (ahead positive), s.
    pub clock_error_s: f64,
    /// The receiver's own time uncertainty, s.
    pub uncertainty_s: f64,
    /// TESLA chain broadcast.
    pub chain: u8,
    pub chain_root_known: bool,
    pub chain_revoked: bool,
    pub pk_known: bool,
    pub pk_revoked: bool,
    /// The chain before this one was revoked (directly, or with its public key).
    pub previous_chain_revoked: bool,
}

impl Default for Conditions {
    fn default() -> Self {
        Conditions {
            received: true,
            delay_s: 0.0,
            clock_error_s: 0.0,
            uncertainty_s: 5.0,
            chain: 1,
            chain_root_known: true,
            chain_revoked: false,
            pk_known: true,
            pk_revoked: false,
            previous_chain_revoked: false,
        }
    }
}

/// A scenario.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scenario {
    pub id: &'static str,
    pub title: &'static str,
    pub what: &'static str,
    /// The lab's stimulus, at test-condition level.
    pub stimulus: &'static str,
    pub subframes: Vec<Conditions>,
    /// Expected state per subframe (`A` or `N`), as the pack states it.
    pub expected: &'static str,
    pub rationale: &'static str,
}

const N_SF: usize = 40;

fn build(f: impl Fn(usize, &mut Conditions)) -> Vec<Conditions> {
    (0..N_SF)
        .map(|k| {
            let mut c = Conditions::default();
            f(k, &mut c);
            c
        })
        .collect()
}

/// The pack's scenarios.
pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            id: "startup-cold",
            title: "Cold start: public key and root key acquisition",
            what: "No stored public key: DSM-PKR complete at subframe 4, DSM-KROOT verified at subframe 6.",
            stimulus: "Nominal OSNMA broadcast from a cold start; the receiver holds only the Merkle root.",
            subframes: build(|k, c| {
                c.pk_known = k >= 4;
                c.chain_root_known = k >= 6;
            }),
            expected: "NNNNNNAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAN",
            rationale: "No tag verifies before both keys are held; the last subframe awaits its key.",
        },
        Scenario {
            id: "key-renewal",
            title: "TESLA chain renewal announced in advance",
            what: "Chain 2's root key broadcast from subframe 15; chain 2 in use from subframe 20.",
            stimulus: "Chain renewal (CREN) with the new DSM-KROOT broadcast five subframes before the switch.",
            subframes: build(|k, c| {
                c.chain = if k >= 20 { 2 } else { 1 };
            }),
            expected: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAN",
            rationale: "The new root key is held before the switch: authentication continues without an outage.",
        },
        Scenario {
            id: "chain-revocation",
            title: "TESLA chain revocation",
            what: "Chain 1 revoked at subframe 20 (CREV); chain 2's root key verified at subframe 24.",
            stimulus: "Chain revocation status from subframe 20; new DSM-KROOT from subframe 24.",
            subframes: build(|k, c| {
                if k >= 20 {
                    c.chain = 2;
                    c.chain_root_known = k >= 24;
                    c.previous_chain_revoked = true;
                }
            }),
            expected: "AAAAAAAAAAAAAAAAAAANNNNNAAAAAAAAAAAAAAAN",
            rationale: "Keys of a revoked chain are not used, so subframe 19 stays unauthenticated; chain 2 authenticates once its root key is verified.",
        },
        Scenario {
            id: "subframe-loss",
            title: "Three subframes lost",
            what: "Subframes 15 to 17 not received (blockage or loss of lock).",
            stimulus: "Signal interrupted for 90 s from subframe 15.",
            subframes: build(|k, c| c.received = !(15..18).contains(&k)),
            expected: "AAAAAAAAAAAAAAANNNAAAAAAAAAAAAAAAAAAAAAN",
            rationale: "Lost subframes carry no data to authenticate; the key of subframe 18 verifies subframe 14's key by stepping the chain across the gap.",
        },
        Scenario {
            id: "replay-delayed",
            title: "Replay delayed beyond the key disclosure",
            what: "From subframe 20 the receiver sees a 60 s replay of the constellation.",
            stimulus: "Whole-constellation re-broadcast with 60 s delay from subframe 20.",
            subframes: build(|k, c| {
                if k >= 20 {
                    c.delay_s = 60.0;
                }
            }),
            expected: "AAAAAAAAAAAAAAAAAAAANNNNNNNNNNNNNNNNNNNN",
            rationale: "Tags arrive after their key was disclosed: the time check rejects them.",
        },
        Scenario {
            id: "meaconing-short",
            title: "Short-delay meaconing",
            what: "From subframe 20 a 4 s relay of the constellation.",
            stimulus: "Whole-constellation re-broadcast with 4 s delay from subframe 20.",
            subframes: build(|k, c| {
                if k >= 20 {
                    c.delay_s = 4.0;
                }
            }),
            expected: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAN",
            rationale: "OSNMA authenticates the navigation data, which a relay carries unchanged; a delay within the disclosure margin is not detected by OSNMA alone. A receiver reporting N here is using another check, which the scorer notes but does not count as unsafe.",
        },
        Scenario {
            id: "time-sync-loss",
            title: "Receiver time uncertainty grows past the bound",
            what: "No time source update from subframe 20: uncertainty 5 s plus 10 s per subframe.",
            stimulus: "External time reference withdrawn at subframe 20.",
            subframes: build(|k, c| {
                if k >= 20 {
                    c.uncertainty_s = 5.0 + 10.0 * (k - 20) as f64;
                }
            }),
            expected: "AAAAAAAAAAAAAAAAAAAAAANNNNNNNNNNNNNNNNNN",
            rationale: "Uncertainty passes the 15 s bound at subframe 22: the receiver cannot show tags arrived before their keys.",
        },
        Scenario {
            id: "time-sync-attack",
            title: "Receiver clock pushed ahead",
            what: "From subframe 10 the receiver's clock is 28 s ahead while it believes its uncertainty is 5 s.",
            stimulus: "Manipulated external time source from subframe 10 (receiver input, not a GNSS signal).",
            subframes: build(|k, c| {
                if k >= 10 {
                    c.clock_error_s = 28.0;
                }
            }),
            expected: "AAAAAAAAAANNNNNNNNNNNNNNNNNNNNNNNNNNNNNN",
            rationale: "Tags look 28 s old; with 5 s uncertainty they cannot be shown to precede their keys: denial of authentication, not false authentication.",
        },
        Scenario {
            id: "pk-revocation",
            title: "Public key revocation",
            what: "Public key revoked at subframe 20 (PKREV); new public key at subframe 26, its root key at subframe 28.",
            stimulus: "Public-key revocation alert from subframe 20; DSM-PKR from 26; DSM-KROOT from 28.",
            subframes: build(|k, c| {
                if k >= 20 {
                    c.pk_revoked = k < 26;
                    c.previous_chain_revoked = true;
                    c.chain = 2;
                    c.chain_root_known = k >= 28;
                }
            }),
            expected: "AAAAAAAAAAAAAAAAAAANNNNNNNNNAAAAAAAAAAAN",
            rationale: "Nothing verifies under a revoked public key; authentication resumes with the new key and root key.",
        },
    ]
}

/// The reference model: expected state per subframe.
pub fn expected_states(sf: &[Conditions], m: &Model) -> String {
    let pk_ok = |c: &Conditions| c.pk_known && !c.pk_revoked;
    let timely = |c: &Conditions| {
        c.uncertainty_s <= m.max_uncertainty_s
            && c.delay_s + c.clock_error_s + c.uncertainty_s < m.disclosure_s
    };
    // Whether the key that subframe `j` discloses for chain `chain` can be used.
    let key_ok = |j: &Conditions, chain: u8| {
        j.received
            && pk_ok(j)
            && if j.chain == chain {
                j.chain_root_known && !j.chain_revoked
            } else {
                // Across a switch the old chain's last key is still usable after a renewal,
                // never after a revocation of that chain or of its public key.
                !j.previous_chain_revoked
            }
    };
    (0..sf.len())
        .map(|k| {
            let c = &sf[k];
            let ok = c.received
                && pk_ok(c)
                && c.chain_root_known
                && !c.chain_revoked
                && timely(c)
                && sf[k + 1..].iter().any(|j| key_ok(j, c.chain));
            if ok {
                'A'
            } else {
                'N'
            }
        })
        .collect()
}

/// The cryptographic premises, checked with the open engine.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CryptoChecks {
    pub key_verifies_across_three_lost_subframes: bool,
    pub revoked_chain_key_fails_against_new_root: bool,
    pub replayed_old_key_fails_as_current: bool,
    pub scorecard_stimulus_check: crate::osnma_conformance::StimulusCheck,
}

const ALPHA: [u8; 6] = [0x25, 0xd3, 0x96, 0x4d, 0xa3, 0xa2];

fn chain(seed: u8, gst0: u32, n: u32) -> Vec<Vec<u8>> {
    let mut seq: Vec<Vec<u8>> = vec![(0..16u8).map(|i| i.wrapping_mul(31) ^ seed).collect()];
    for i in (0..n).rev() {
        let g = gst0 + SUBFRAME_S * i - SUBFRAME_S;
        let next = step(HashFn::Sha256, seq.last().expect("seed"), g, &ALPHA, 128);
        seq.push(next);
    }
    seq.reverse();
    seq
}

/// Run the checks.
pub fn crypto_checks() -> CryptoChecks {
    let gst0 = 1300 * kshana::osnma::WEEK_S + 86_400;
    let root_gst = gst0 - SUBFRAME_S;
    let c1 = chain(0x5a, gst0, 24);
    let c2 = chain(0xa5, gst0, 24);
    let g = |i: u32| gst0 + SUBFRAME_S * (i - 1);
    // Key 18 verifies key 14 across lost 15, 16, 17.
    let across = verify_key(HashFn::Sha256, &c1[18], g(18), &c1[14], g(14), &ALPHA).is_ok();
    // A key of chain 1 presented against chain 2's root fails.
    let revoked = verify_key(HashFn::Sha256, &c1[12], g(12), &c2[0], root_gst, &ALPHA).is_err();
    let replay = verify_key(HashFn::Sha256, &c1[9], g(10), &c1[0], root_gst, &ALPHA).is_err();
    CryptoChecks {
        key_verifies_across_three_lost_subframes: across,
        revoked_chain_key_fails_against_new_root: revoked,
        replayed_old_key_fails_as_current: replay,
        scorecard_stimulus_check: crate::osnma_conformance::stimulus_check(),
    }
}

/// The exported pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub evidence: &'static str,
    pub wording: &'static str,
    pub model: Model,
    pub scenarios: Vec<Scenario>,
    /// Scenario id → whether the reference model reproduces the stated expected states.
    pub model_agrees: BTreeMap<String, bool>,
    pub crypto: CryptoChecks,
    pub not_covered: Vec<&'static str>,
}

/// Build the pack.
pub fn pack() -> Pack {
    let m = Model::default();
    let sc = scenarios();
    let model_agrees = sc
        .iter()
        .map(|s| {
            (
                s.id.to_string(),
                expected_states(&s.subframes, &m) == s.expected,
            )
        })
        .collect();
    Pack {
        format: PACK_FORMAT,
        evidence: EVIDENCE,
        wording: WORDING,
        model: m,
        scenarios: sc,
        model_agrees,
        crypto: crypto_checks(),
        not_covered: vec![
            "DO-401A avionics OSNMA cases (not public).",
            "SBAS authentication (not public).",
        ],
    }
}

/// One scenario's score.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScenarioScore {
    pub scenario: String,
    pub expected: String,
    pub reported: String,
    pub agreement: f64,
    /// Subframes reported `A` where `N` is expected (the safety-relevant disagreement).
    pub unsafe_subframes: Vec<usize>,
    /// Subframes reported not `A` where `A` is expected (availability).
    pub missed_subframes: Vec<usize>,
    /// `met` when every subframe agrees, else `not met`; `noted` for meaconing-only extra checks.
    pub verdict: String,
}

/// The score.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Score {
    pub format: &'static str,
    pub evidence: &'static str,
    pub wording: &'static str,
    pub receiver: crate::osnma_conformance::Receiver,
    pub log_sha256: String,
    pub scenarios: Vec<ScenarioScore>,
    pub not_run: Vec<String>,
    pub verdict: String,
}

/// Score a receiver log whose test ids are the scenario ids.
pub fn score(log: &ReceiverLog, log_bytes: &[u8]) -> Result<Score, String> {
    let sc = scenarios();
    let mut out = vec![];
    let mut not_run = vec![];
    for s in &sc {
        let Some(t) = log.tests.iter().find(|t| t.id == s.id) else {
            not_run.push(s.id.to_string());
            continue;
        };
        let reported: String = (0..s.subframes.len())
            .map(|k| {
                let (a, b) = (
                    k as f64 * SUBFRAME_S as f64,
                    (k + 1) as f64 * SUBFRAME_S as f64,
                );
                match t.records.iter().rev().find(|r| r.t_s >= a && r.t_s < b) {
                    Some(r) if r.overall == Status::A => 'A',
                    Some(_) => 'N',
                    None => '-',
                }
            })
            .collect();
        let e: Vec<char> = s.expected.chars().collect();
        let r: Vec<char> = reported.chars().collect();
        let unsafe_subframes: Vec<usize> = (0..e.len())
            .filter(|&k| r[k] == 'A' && e[k] != 'A')
            .collect();
        let missed: Vec<usize> = (0..e.len())
            .filter(|&k| e[k] == 'A' && r[k] != 'A')
            .collect();
        let agree = (0..e.len())
            .filter(|&k| (r[k] == 'A') == (e[k] == 'A'))
            .count();
        let verdict = if unsafe_subframes.is_empty() && missed.is_empty() {
            "met"
        } else if s.id == "meaconing-short" && unsafe_subframes.is_empty() {
            "noted"
        } else {
            "not met"
        };
        out.push(ScenarioScore {
            scenario: s.id.into(),
            expected: s.expected.into(),
            reported,
            agreement: (agree as f64 / e.len() as f64 * 1000.0).round() / 1000.0,
            unsafe_subframes,
            missed_subframes: missed,
            verdict: verdict.into(),
        });
    }
    let unsafe_n: usize = out.iter().map(|s| s.unsafe_subframes.len()).sum();
    let verdict = format!(
        "{} scenario(s) scored: {} met, {} not met, {} noted; {} unsafe subframe(s); {} not run (MODELLED)",
        out.len(),
        out.iter().filter(|s| s.verdict == "met").count(),
        out.iter().filter(|s| s.verdict == "not met").count(),
        out.iter().filter(|s| s.verdict == "noted").count(),
        unsafe_n,
        not_run.len()
    );
    Ok(Score {
        format: FORMAT,
        evidence: EVIDENCE,
        wording: WORDING,
        receiver: log.receiver.clone(),
        log_sha256: crate::forensics::inputs::sha256_hex(log_bytes),
        scenarios: out,
        not_run,
        verdict,
    })
}

/// HTML page.
pub fn html(s: &Score) -> String {
    crate::transport::page(
        &format!(
            "OSNMA scenario pack: {} {}",
            s.receiver.make, s.receiver.model
        ),
        s.evidence,
        &s.verdict,
        &[s.wording.to_string()],
        &[(
            "Scenarios".into(),
            vec![
                "scenario",
                "expected",
                "reported",
                "agreement",
                "unsafe",
                "missed",
                "verdict",
            ],
            s.scenarios
                .iter()
                .map(|x| {
                    vec![
                        x.scenario.clone(),
                        x.expected.clone(),
                        x.reported.clone(),
                        format!("{:.3}", x.agreement),
                        format!("{:?}", x.unsafe_subframes),
                        format!("{:?}", x.missed_subframes),
                        x.verdict.clone(),
                    ]
                })
                .collect(),
        )],
    )
}

/// A SYNTHETIC receiver log over every scenario: `good` follows the expected states; the faulty
/// one keeps authenticating after the delayed replay and on the revoked chain.
pub fn synth_log(good: bool) -> ReceiverLog {
    use crate::osnma_conformance::{Receiver, Record, TestKind, TestLog, LOG_FORMAT};
    let tests = scenarios()
        .into_iter()
        .map(|s| {
            let mut st: Vec<char> = s.expected.chars().collect();
            if !good && (s.id == "replay-delayed" || s.id == "chain-revocation") {
                for c in st.iter_mut().take(N_SF - 1) {
                    *c = 'A';
                }
            }
            TestLog {
                id: s.id.into(),
                kind: TestKind::Replay,
                event_at_s: None,
                forged_svs: vec![],
                start: None,
                records: (0..N_SF * SUBFRAME_S as usize)
                    .step_by(5)
                    .map(|t| Record {
                        t_s: t as f64,
                        overall: if st[t / SUBFRAME_S as usize] == 'A' {
                            Status::A
                        } else {
                            Status::N
                        },
                        fix: true,
                        sats: vec![],
                        alarm: false,
                    })
                    .collect(),
            }
        })
        .collect();
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
