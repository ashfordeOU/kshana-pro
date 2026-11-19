// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Delay-bounded LDC authentication: authenticated **and on time** (patent candidate E7,
//! research 04 §2; gap 01-27). CONFIDENTIAL: Kshana Pro only.
//!
//! The LDC authentication tag ([`super::rx::LdcDecoder::verify_auth`], HMAC-SHA256 over the
//! covered message blocks, the station and the frame number) proves origin, and the frame
//! number in the MAC input stops a replay **at a later frame**. It does not stop a relay
//! that re-radiates the authentic signal with a small delay inside the same frames: the
//! tag still verifies. This module adds the missing check: the arrival delay of the pulses
//! that carried the authenticated message must fall inside the propagation-delay bound of
//! the claimed station and receiver position,
//!
//! ```text
//! | (pseudo-delay − receiver clock) − (PF + SF + ASF_pred) |  ≤  k · √(σ_ASF² + σ_TOA² + σ_clock²)
//! ```
//!
//! with `σ_ASF` from the Monte Carlo propagation-uncertainty model
//! ([`crate::prop::uncertainty`], [`ExpectedDelay::from_uncertainty`]) or the learnt ASF
//! state, and `k` set from the accepted false-reject probability
//! ([`DelayBound::for_false_reject`]). An authentic message delayed by a relay, or arriving
//! with the wrong ASF, fails although its MAC verifies.
//!
//! The bound is also the **replay protection level**: the largest relay delay (and so the
//! largest timing error a relay attacker can induce through an accepted message) is
//! `bound + |nominal residual|`; [`DelayAuthResult::replay_protection_level_ns`] reports the
//! bound.
//!
//! # Limits (honest)
//!
//! * MODELLED: tested on the crate's HMAC machinery and the crate's propagation Monte Carlo;
//!   no over-the-air LDC data.
//! * Needs the receiver clock against the station time scale (GNSS while trusted, holdover,
//!   or the other stations' common clock) and the receiver position.
//! * A relay delay below the bound is not detected; the bound states how much that is.
//! * HMAC only (the crate has no TESLA key chain); the delay check is independent of the
//!   authentication scheme.

use super::rx::{AuthCheck, DecodeReport, LdcDecoder};
use crate::guard::normal_quantile;
use crate::prop::PathEffect;
use serde::{Deserialize, Serialize};

/// The expected propagation delay of a station at the receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExpectedDelay {
    /// `PF + SF + ASF_pred` (ns).
    pub predicted_ns: f64,
    /// One-sigma ASF (prediction) uncertainty (ns).
    pub sigma_asf_ns: f64,
}

impl ExpectedDelay {
    /// From a groundwave path of the propagation model and a stated ASF σ.
    pub fn from_path(p: &PathEffect, sigma_asf_ns: f64) -> ExpectedDelay {
        ExpectedDelay {
            predicted_ns: p.delay.as_s() * 1e9,
            sigma_asf_ns,
        }
    }
    /// From a Monte Carlo propagation-uncertainty result for the path: the nominal delay
    /// and the spread of the draws.
    pub fn from_uncertainty(r: &crate::prop::uncertainty::PointResult) -> ExpectedDelay {
        ExpectedDelay {
            predicted_ns: r.delay_ns.nominal,
            sigma_asf_ns: r.delay_ns.sd,
        }
    }
}

/// The bound settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelayBound {
    /// Multiplier on the combined σ.
    pub k: f64,
    /// TOA measurement σ of the carrying pulses (ns).
    pub sigma_toa_ns: f64,
    /// Receiver clock σ against the station time scale (ns).
    pub sigma_clock_ns: f64,
}

impl DelayBound {
    /// `k` for a two-sided Gaussian false-reject probability `p_fr`.
    pub fn for_false_reject(p_fr: f64, sigma_toa_ns: f64, sigma_clock_ns: f64) -> DelayBound {
        DelayBound {
            k: normal_quantile(1.0 - p_fr / 2.0),
            sigma_toa_ns,
            sigma_clock_ns,
        }
    }
    /// The bound (ns) for a station.
    pub fn bound_ns(&self, e: &ExpectedDelay) -> f64 {
        self.k
            * (e.sigma_asf_ns.powi(2) + self.sigma_toa_ns.powi(2) + self.sigma_clock_ns.powi(2))
                .sqrt()
    }
}

/// Arrival of the pulses that carried a message.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arrival {
    /// Arrival minus nominal emission of the carrying groups (ns).
    pub pseudo_delay_ns: f64,
    /// Receiver clock minus the station's time scale (ns).
    pub clock_bias_ns: f64,
}

/// Outcome of the combined check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Authentic and on time.
    Accepted,
    /// The tag did not verify (forged, wrong key, or replayed at another frame).
    Forged,
    /// The tag could not be checked (covered messages missing).
    Unverifiable,
    /// Authentic but late beyond the bound (relay / delayed replay).
    Late,
    /// Authentic but early beyond the bound (clock or position inconsistency).
    Early,
}

/// The combined check of one auth message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DelayAuthResult {
    /// Index of the auth message in the decode report.
    pub message: usize,
    pub verdict: Verdict,
    /// The MAC verified.
    pub authentic: bool,
    /// Arrival residual against the expected delay (ns).
    pub residual_ns: f64,
    pub bound_ns: f64,
    /// The largest relay delay an accepted message can hide (ns): the bound.
    pub replay_protection_level_ns: f64,
}

/// Combine one authentication check with the arrival-delay bound.
pub fn check(
    auth: &AuthCheck,
    arrival: Arrival,
    expected: &ExpectedDelay,
    bound: &DelayBound,
) -> DelayAuthResult {
    let residual = arrival.pseudo_delay_ns - arrival.clock_bias_ns - expected.predicted_ns;
    let b = bound.bound_ns(expected);
    let verdict = match auth.verified {
        None => Verdict::Unverifiable,
        Some(false) => Verdict::Forged,
        Some(true) if residual > b => Verdict::Late,
        Some(true) if residual < -b => Verdict::Early,
        Some(true) => Verdict::Accepted,
    };
    DelayAuthResult {
        message: auth.message,
        verdict,
        authentic: auth.verified == Some(true),
        residual_ns: residual,
        bound_ns: b,
        replay_protection_level_ns: b,
    }
}

/// Verify every auth message of a decode report (HMAC with the decoder's key) and apply
/// the delay bound to each, with the arrival of the carrying pulses.
pub fn verify_report(
    dec: &LdcDecoder,
    station: usize,
    rep: &DecodeReport,
    first_frame: i64,
    arrival: Arrival,
    expected: &ExpectedDelay,
    bound: &DelayBound,
) -> Vec<DelayAuthResult> {
    dec.verify_auth(station, rep, first_frame)
        .iter()
        .map(|a| check(a, arrival, expected, bound))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{Station, StationStatus};
    use crate::geo::Geo;
    use crate::ldc::msg::DloranSpecEntry;
    use crate::ldc::LdcConfig;
    use crate::prop::conductivity::{ConductivityConfig, ConductivityMap};
    use crate::prop::ground::K_LORAN;
    use crate::prop::uncertainty::{Engine, Problem, UncertaintyConfig};
    use crate::signal::Role;
    use crate::threat::{DataChannelAttack, ForgedAuth};
    use crate::timebase::gauss;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const KEY: &[u8] = b"genuine-station-key-0001";

    fn decoder() -> LdcDecoder {
        LdcDecoder::new(&LdcConfig::ninth_pulse(Default::default()))
            .unwrap()
            .unwrap()
            .with_auth_key(KEY)
    }

    /// A message set as the station sends it, tagged with `key` (the genuine key for an
    /// authentic set), and its decode report at frame `first_frame`.
    fn messages(key: &[u8], first_frame: i64) -> (DecodeReport, i64) {
        let att = DataChannelAttack::forged_dloran(
            7,
            vec![DloranSpecEntry {
                station_code: 1,
                correction_ns: 120.0,
            }],
            ForgedAuth::WrongKey(key.to_vec()),
            0,
            first_frame,
        );
        (att.decode_report(), first_frame)
    }

    fn expected() -> ExpectedDelay {
        ExpectedDelay {
            predicted_ns: 2_345_678.0,
            sigma_asf_ns: 80.0,
        }
    }

    fn bound() -> DelayBound {
        DelayBound::for_false_reject(1e-3, 20.0, 15.0)
    }

    #[test]
    fn authentic_on_time_message_is_accepted() {
        let (rep, ff) = messages(KEY, 100);
        let e = expected();
        let arr = Arrival {
            pseudo_delay_ns: e.predicted_ns + 731.0 + 60.0,
            clock_bias_ns: 731.0,
        };
        let r = verify_report(&decoder(), 0, &rep, ff, arr, &e, &bound());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].verdict, Verdict::Accepted, "{r:?}");
        assert!(r[0].authentic);
        // k for 1e-3 two-sided is 3.29; bound = 3.29·√(80² + 20² + 15²)
        assert!((r[0].bound_ns - 3.2905 * (80f64 * 80.0 + 400.0 + 225.0).sqrt()).abs() < 0.5);
        assert_eq!(r[0].replay_protection_level_ns, r[0].bound_ns);
    }

    #[test]
    fn relayed_authentic_message_passes_the_mac_but_fails_the_delay_bound() {
        // a relay re-radiates the authentic frames with a delay: the frame numbers (and so
        // the MAC input) are unchanged, the tag verifies, the arrival is late
        let (rep, ff) = messages(KEY, 100);
        let e = expected();
        let b = bound();
        let bound_ns = b.bound_ns(&e);
        for relay_ns in [bound_ns + 50.0, 2_000.0, 25_000.0] {
            let arr = Arrival {
                pseudo_delay_ns: e.predicted_ns + 731.0 + relay_ns,
                clock_bias_ns: 731.0,
            };
            let r = verify_report(&decoder(), 0, &rep, ff, arr, &e, &b);
            assert!(r[0].authentic, "the MAC alone would accept the relay");
            assert_eq!(r[0].verdict, Verdict::Late, "{relay_ns}");
            assert!((r[0].residual_ns - relay_ns).abs() < 1e-6);
        }
        // an early arrival beyond the bound is rejected too
        let arr = Arrival {
            pseudo_delay_ns: e.predicted_ns - 2.0 * bound_ns,
            clock_bias_ns: 0.0,
        };
        let r = verify_report(&decoder(), 0, &rep, ff, arr, &e, &b);
        assert!(r[0].authentic);
        assert_eq!(r[0].verdict, Verdict::Early);
    }

    #[test]
    fn forged_and_frame_replayed_messages_are_rejected() {
        let e = expected();
        let on_time = Arrival {
            pseudo_delay_ns: e.predicted_ns,
            clock_bias_ns: 0.0,
        };
        // wrong key, on time: rejected by the MAC
        let (rep, ff) = messages(b"attacker-key", 100);
        let r = verify_report(&decoder(), 0, &rep, ff, on_time, &e, &bound());
        assert_eq!(r[0].verdict, Verdict::Forged);
        assert!(!r[0].authentic);
        // authentic set replayed one GRI-frame later: the frame number in the MAC input
        // no longer matches, so the tag fails even on time
        let (rep, _) = messages(KEY, 100);
        let r = verify_report(&decoder(), 0, &rep, 101, on_time, &e, &bound());
        assert_eq!(r[0].verdict, Verdict::Forged);
        // absent authentication: nothing to accept
        let absent = DataChannelAttack::forged_dloran(
            7,
            vec![DloranSpecEntry {
                station_code: 1,
                correction_ns: 120.0,
            }],
            ForgedAuth::Absent,
            0,
            100,
        );
        let r = verify_report(
            &decoder(),
            0,
            &absent.decode_report(),
            100,
            on_time,
            &e,
            &bound(),
        );
        assert!(r.is_empty(), "no auth message, nothing accepted");
        // coverage missing: unverifiable
        let un = check(
            &AuthCheck {
                message: 0,
                verified: None,
            },
            on_time,
            &e,
            &bound(),
        );
        assert_eq!(un.verdict, Verdict::Unverifiable);
    }

    fn station() -> Station {
        Station {
            id: "M".into(),
            name: "M".into(),
            role: Role::Master,
            position: Geo::new(54.0, -2.0),
            power_kw: 400.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: StationStatus::On,
            faults: vec![],
        }
    }

    /// The bound must not reject clean messages. The true delay of each clean message is a
    /// draw of the propagation Monte Carlo (conductivity class, permittivity, k-factor);
    /// the prediction is the nominal model and σ_ASF is the spread of an independent run
    /// (different seed). Stated: false-reject rate ≤ 1 % at the chosen bound (k for
    /// p_fr = 1e-3) over 300 draws × 2 paths; a bound that is too tight (k = 1) rejects
    /// > 20 % (so the bound is doing work and the σ is not inflated).
    #[test]
    fn false_reject_rate_over_random_asf_draws_is_low() {
        let map = ConductivityMap::build(&ConductivityConfig::Uniform {
            sigma_s_m: 0.003,
            eps_r: Some(15.0),
        })
        .unwrap();
        let points = vec![Geo::new(52.0, 0.5), Geo::new(51.0, -3.5)];
        let mk = |seed: u64, n: usize| {
            let mut cfg = UncertaintyConfig::default();
            cfg.samples = n;
            cfg.seed = seed;
            // a mapped area: conductivity known to ±0.15 decades, ε_r to ±10 %
            cfg.sigma_log10_sd = Some(0.15);
            cfg.eps_rel_sd = Some(0.1);
            Engine::new(
                Problem::new(vec![station()], points.clone(), &map, K_LORAN),
                cfg,
            )
            .unwrap()
        };
        let cal = mk(11, 120).run().unwrap();
        let draws = mk(99, 300).samples();
        let (sig_toa, sig_clk) = (20.0, 15.0);
        let chosen = DelayBound::for_false_reject(1e-3, sig_toa, sig_clk);
        let tight = DelayBound {
            k: 1.0,
            ..chosen.clone()
        };
        let mut rng = ChaCha8Rng::seed_from_u64(5);
        let (mut n, mut rej, mut rej_tight) = (0, 0, 0);
        let (rep, ff) = messages(KEY, 100);
        let dec = decoder();
        for d in &draws {
            for (pi, res) in cal.results.iter().enumerate() {
                let e = ExpectedDelay::from_uncertainty(res);
                assert!(e.sigma_asf_ns > 5.0, "ASF spread {}", e.sigma_asf_ns);
                let clock = rng_clock(&mut rng);
                let arr = Arrival {
                    pseudo_delay_ns: d[pi][0]
                        + clock
                        + sig_toa * gauss(&mut rng)
                        + sig_clk * gauss(&mut rng),
                    clock_bias_ns: clock,
                };
                n += 1;
                if verify_report(&dec, 0, &rep, ff, arr, &e, &chosen)[0].verdict
                    != Verdict::Accepted
                {
                    rej += 1;
                }
                if verify_report(&dec, 0, &rep, ff, arr, &e, &tight)[0].verdict != Verdict::Accepted
                {
                    rej_tight += 1;
                }
            }
        }
        let fr = rej as f64 / n as f64;
        let fr_t = rej_tight as f64 / n as f64;
        println!(
            "E7 false-reject over {n} ASF draws: k={:.2} → {fr:.4}, k=1 → {fr_t:.3}; σ_ASF {:.1}/{:.1} ns",
            chosen.k, cal.results[0].delay_ns.sd, cal.results[1].delay_ns.sd
        );
        assert!(fr <= 0.01, "false-reject rate {fr}");
        assert!(
            fr_t > 0.2,
            "a too-tight bound should reject clean messages: {fr_t}"
        );
    }

    fn rng_clock(rng: &mut ChaCha8Rng) -> f64 {
        use rand::Rng;
        rng.gen_range(-1e4..1e4)
    }
}
