// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D3: Earth–Moon TWTT holdover calculator (part of the tests/lunar_pro.rs test binary).

use kshana_pro::lunar_pro::budget::std_normal;
use kshana_pro::lunar_pro::holdover::*;
use rand::SeedableRng;

fn quiet_link() -> Link {
    Link {
        sigma_s: 0.0,
        n_per_contact: 1,
        calibration_u_s: 0.0,
    }
}

#[test]
fn d3_phase_only_time_error_is_tau_sigma_y_for_each_noise_type() {
    // White FM, random-walk FM (exact) and flicker FM (by the IEEE 1139 convention):
    // σ_TE(τ) = τ·σ_y(τ), σ_y from the open engine's Allan deviation.
    for clock in [
        ClockNoise {
            h_m2: 0.0,
            h_m1: 0.0,
            h_0: 2e-22,
        },
        ClockNoise {
            h_m2: 3e-30,
            h_m1: 0.0,
            h_0: 0.0,
        },
        ClockNoise {
            h_m2: 0.0,
            h_m1: 9.6e-29,
            h_0: 0.0,
        },
    ] {
        let h = Holdover {
            clock,
            link: quiet_link(),
            strategy: Strategy::PhaseOnly,
        };
        for tau in [10.0, 1e3, 86_400.0, 1e6] {
            let te = h.sigma(tau, tau);
            let want = tau * clock.adev(tau);
            assert!(
                (te / want - 1.0).abs() < 1e-9,
                "{clock:?} tau {tau}: {te:e} vs {want:e}"
            );
        }
    }
}

#[test]
fn d3_named_lunar_clocks_reproduce_their_cited_one_day_time_error() {
    use kshana::clock_specs::LunarClock;
    for c in LunarClock::all() {
        let h = Holdover {
            clock: ClockNoise::named(c.name()).unwrap(),
            link: quiet_link(),
            strategy: Strategy::PhaseOnly,
        };
        let ns = h.sigma(86_400.0, 86_400.0) * 1e9;
        assert!(
            (ns / c.cited_one_day_ns() - 1.0).abs() < 2e-3,
            "{}: {ns}",
            c.name()
        );
    }
    assert!(ClockNoise::named("ocxo").is_ok());
    assert!(ClockNoise::named("sundial").is_err());
}

#[test]
fn d3_required_interval_matches_the_white_fm_closed_form() {
    // Phase only, white FM: k²(σ_L² + q_wf T) = B²  ⇒  T = ((B/k)² − σ_L²)/q_wf.
    let clock = ClockNoise::named("rafs").unwrap();
    let link = Link {
        sigma_s: 0.5e-9,
        n_per_contact: 4,
        calibration_u_s: 0.0,
    };
    let h = Holdover {
        clock,
        link,
        strategy: Strategy::PhaseOnly,
    };
    let (b, k) = (10e-9_f64, 2.0_f64);
    let want = ((b / k).powi(2) - link.phase_var()) / clock.q_wf();
    let got = h.required_interval(b, k, 1e9).unwrap();
    assert!((got / want - 1.0).abs() < 1e-9, "{got} vs {want}");
    // At that interval the worst time error equals the budget.
    let r = h.report(got, k, 50);
    assert!((r.max_time_error_s / b - 1.0).abs() < 1e-9);
    // A budget below k·σ_L cannot be met at any cadence.
    assert!(h.required_interval(0.4e-9, 2.0, 1e9).is_none());
}

#[test]
fn d3_random_walk_fm_required_interval_matches_the_closed_form() {
    // Phase only, RWFM, noise-free link: k² q_rw T³/3 = B² ⇒ T = (3B²/(k² q_rw))^(1/3).
    let clock = ClockNoise {
        h_m2: 1e-31,
        h_m1: 0.0,
        h_0: 0.0,
    };
    let h = Holdover {
        clock,
        link: quiet_link(),
        strategy: Strategy::PhaseOnly,
    };
    let (b, k) = (100e-9_f64, 3.0_f64);
    let want = (3.0 * b * b / (k * k * clock.q_rw())).cbrt();
    let got = h.required_interval(b, k, 1e12).unwrap();
    assert!((got / want - 1.0).abs() < 1e-9, "{got} vs {want}");
}

#[test]
fn d3_phase_and_frequency_variance_matches_monte_carlo() {
    // Simulate a WFM + RWFM clock, TWTT contacts every T with white link noise, a two-contact
    // frequency estimate, and the time error just before the next contact.
    let clock = ClockNoise {
        h_m2: 2e-29,
        h_m1: 0.0,
        h_0: 2e-22,
    };
    let link = Link {
        sigma_s: 1e-9,
        n_per_contact: 1,
        calibration_u_s: 0.0,
    };
    let h = Holdover {
        clock,
        link,
        strategy: Strategy::PhaseAndFrequency,
    };
    let t = 3_600.0;
    let steps = 60;
    let dt = t / steps as f64;
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(11);
    let (qwf, qrw) = (clock.q_wf(), clock.q_rw());
    // Exact discretisation of the two-state (phase, frequency) clock over one step.
    let vy = qrw * dt;
    let cxy = qrw * dt * dt / 2.0;
    let vx = qrw * dt.powi(3) / 3.0;
    let step = |x: &mut f64, y: &mut f64, rng: &mut rand_chacha::ChaCha20Rng| {
        let z1 = std_normal(rng);
        let z2 = std_normal(rng);
        let dy = vy.sqrt() * z1;
        let dx_rw = cxy / vy.sqrt() * z1 + (vx - cxy * cxy / vy).max(0.0).sqrt() * z2;
        let dx_wf = (qwf * dt).sqrt() * std_normal(rng);
        *x += *y * dt + dx_rw + dx_wf;
        *y += dy;
    };
    let trials = 6_000;
    let mut s2 = 0.0;
    for _ in 0..trials {
        let (mut x, mut y) = (0.0f64, 0.0f64);
        let m0 = x + link.sigma_s * std_normal(&mut rng);
        for _ in 0..steps {
            step(&mut x, &mut y, &mut rng);
        }
        let m1 = x + link.sigma_s * std_normal(&mut rng);
        for _ in 0..steps {
            step(&mut x, &mut y, &mut rng);
        }
        let pred = m1 + (m1 - m0);
        s2 += (x - pred).powi(2);
    }
    let emp = (s2 / trials as f64).sqrt();
    let model = h.sigma(t, t);
    // 6000 trials: the rms has a relative standard error of about 1 %.
    assert!(
        (emp / model - 1.0).abs() < 0.04,
        "MC {emp:e} vs model {model:e}"
    );
}

#[test]
fn d3_clock_from_adev_points_round_trips_through_the_engine_fit() {
    let truth = ClockNoise {
        h_m2: 1e-32,
        h_m1: 5e-30,
        h_0: 1e-23,
    };
    let taus: Vec<f64> = (0..12).map(|i| 10f64.powf(i as f64 * 0.5)).collect();
    let adevs: Vec<f64> = taus.iter().map(|&t| truth.adev(t)).collect();
    let fit = ClockNoise::from_adev_points(&taus, &adevs).unwrap();
    for (a, b) in [
        (fit.h_0, truth.h_0),
        (fit.h_m1, truth.h_m1),
        (fit.h_m2, truth.h_m2),
    ] {
        assert!((a / b - 1.0).abs() < 1e-6, "{a:e} vs {b:e}");
    }
}

#[test]
fn d3_report_profile_is_monotone_and_peaks_at_the_interval() {
    let h = Holdover {
        clock: ClockNoise::named("passive-h-maser").unwrap(),
        link: Link {
            sigma_s: 0.2e-9,
            n_per_contact: 10,
            calibration_u_s: 0.3e-9,
        },
        strategy: Strategy::PhaseAndFrequency,
    };
    let r = h.report(86_400.0, 2.0, 25);
    assert!(r.profile.windows(2).all(|w| w[1].sigma_s >= w[0].sigma_s));
    assert!((r.max_time_error_s - 2.0 * r.profile.last().unwrap().sigma_s).abs() < 1e-18);
    assert!(r.rms_time_error_s < r.profile.last().unwrap().sigma_s);
    let c = r.at_interval;
    let sum = c.link + c.white_fm + c.flicker_fm + c.random_walk_fm + c.calibration;
    assert!((sum - c.total).abs() <= 1e-15 * c.total);
}
