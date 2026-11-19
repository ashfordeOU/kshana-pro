// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Cross-system attribution (E1) with the meacon signature detector (E4): Monte Carlo
//! confusion matrix over GNSS spoofing, GNSS jamming, eLoran station faults, eLoran
//! meaconing and local-clock faults, on broadcast-ephemeris GNSS geometry (real GPS
//! ephemerides of 2024-09-10) and six synthetic eLoran stations. MODELLED.

#![cfg(feature = "unfiled-methods")]

use kshana::rinex::parse_nav;
use kshana_eloran::chain::{Station, StationStatus};
use kshana_eloran::geo::{direct, Geo};
use kshana_eloran::guard::meacon::{
    test_station, Expected, MeaconConfig, ModelSigmas, Observed, StationVerdict,
};
use kshana_eloran::prop::ground::{self, GroundConfig, K_LORAN};
use kshana_eloran::prop::{BuildContext, PropagationModel};
use kshana_eloran::signal::Role;
use kshana_eloran::time::AbsTime;
use kshana_eloran::traj::RxState;
use kshana_pro::eloran_attrib::{attribute, AttribConfig, AttribEpoch, Cause, Exclusion};
use kshana_pro::eloran_fusion::{broadcast_ranges, geo_to_ecef, model_eloran_ranges, C};

const NAV: &str = include_str!("fixtures/eloran/brdc_gps_20240910.rnx");
const TOW: f64 = 172_800.0 + 3600.0;

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        (-2.0 * self.u().ln()).sqrt() * (2.0 * std::f64::consts::PI * self.u()).cos()
    }
    fn r(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.u()
    }
    fn sign(&mut self) -> f64 {
        if self.u() < 0.5 {
            -1.0
        } else {
            1.0
        }
    }
    fn pick(&mut self, n: usize) -> usize {
        ((self.u() * n as f64) as usize).min(n - 1)
    }
}

fn stations() -> Vec<(String, Geo)> {
    vec![
        ("A/M".into(), Geo::new(56.5, 1.0)),
        ("A/X".into(), Geo::new(53.5, 1.5)),
        ("A/Y".into(), Geo::new(55.5, 6.5)),
        ("A/Z".into(), Geo::new(53.8, 5.5)),
        ("B/V".into(), Geo::new(57.6, 4.5)),
        ("B/W".into(), Geo::new(54.2, -0.8)),
    ]
}

fn model() -> Box<dyn PropagationModel> {
    let cfg = GroundConfig::SmoothEarth {
        sigma_s_m: 0.005,
        eps_r: None,
        k_factor: K_LORAN,
        temporal: None,
    };
    let ctx = BuildContext::new(
        AbsTime::from_utc_iso("2024-09-10T01:00:00Z").unwrap(),
        1.0,
        0,
    );
    ground::build(&cfg, &ctx).unwrap()
}

fn station(id: &str, pos: Geo) -> Station {
    Station {
        id: id.into(),
        name: id.into(),
        role: Role::Secondary,
        position: pos,
        power_kw: 400.0,
        emission_delay_us: 0.0,
        time_ref: String::new(),
        tx_clock: None,
        ecd_us: 0.0,
        status: StationStatus::On,
        faults: vec![],
    }
}

const SIG_G: f64 = 3.0; // GNSS σ (m)
const SIG_E: f64 = 20.0; // eLoran σ incl. residual ASF error (m)
const SIG_CLK: f64 = 20.0; // local clock prediction σ (ns)
const DELTA: f64 = 50.0; // GPS − eLoran time scale (ns)
const SIG_DELTA: f64 = 10.0;

/// One simulated epoch of class `truth`; returns the attribution and the attacked station.
fn case(
    rng: &mut Rng,
    ephs: &[kshana::rinex::RinexEphemeris],
    m: &dyn PropagationModel,
    truth: Cause,
) -> (kshana_pro::eloran_attrib::Attribution, Option<String>) {
    let rx = Geo {
        lat_deg: 55.0,
        lon_deg: 3.0,
        h_m: 30.0,
    };
    let tow = TOW + rng.r(0.0, 7200.0);
    let b_true_ns = rng.r(-1e5, 1e5);
    let delta_true = DELTA + SIG_DELTA * rng.n();
    let mut clock_pred = b_true_ns + SIG_CLK * rng.n();
    // GNSS
    let mut g_pos = rx;
    let mut push_ns = 0.0;
    if truth == Cause::GnssSpoof {
        let kind = rng.pick(3);
        if kind != 1 {
            push_ns = rng.sign() * rng.r(500.0, 3000.0);
        }
        if kind != 0 {
            g_pos = direct(rx, rng.r(0.0, std::f64::consts::TAU), rng.r(300.0, 3000.0));
            g_pos.h_m = rx.h_m;
        }
    }
    let mut gnss = broadcast_ranges(
        ephs,
        tow,
        geo_to_ecef(g_pos),
        (b_true_ns + push_ns) * 1e-9 * C,
        10.0,
        SIG_G,
    );
    let degrade = if truth == Cause::GnssJam && rng.u() < 0.5 {
        rng.r(8.0, 30.0)
    } else {
        1.0
    };
    for g in &mut gnss {
        g.range_m += SIG_G * degrade * rng.n();
    }
    if truth == Cause::GnssJam && degrade == 1.0 {
        let keep = rng.pick(4);
        gnss.truncate(keep);
    }
    // eLoran
    let sts = stations();
    let mut eloran = model_eloran_ranges(&sts, rx, (b_true_ns + delta_true) * 1e-9 * C, SIG_E);
    for e in &mut eloran {
        e.range_m += SIG_E * rng.n();
    }
    let target = rng.pick(sts.len());
    let mut attacked = None;
    match truth {
        Cause::EloranStationFault => {
            eloran[target].range_m += rng.sign() * rng.r(600.0, 2500.0) * 1e-9 * C;
            attacked = Some(sts[target].0.clone());
        }
        Cause::EloranMeacon => {
            eloran[target].range_m += rng.r(500.0, 25_000.0) * 1e-9 * C;
            attacked = Some(sts[target].0.clone());
        }
        Cause::ClockFault => clock_pred += rng.sign() * rng.r(500.0, 2000.0),
        _ => {}
    }
    // E4 propagation signatures (ECD and field; the threat model's regenerated replica on
    // a meaconed station)
    let rs = RxState {
        t_s: 0.0,
        pos: rx,
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let e4: Vec<StationVerdict> = sts
        .iter()
        .enumerate()
        .map(|(i, (id, pos))| {
            let (ch, sid) = id.split_once('/').unwrap();
            let st = station(sid, *pos);
            let ex = Expected::from_model(m, ch, &st, &rs, ModelSigmas::default()).unwrap();
            let s = ex.sigma.clone();
            let mut ecd = ex.ecd_us + s.ecd_us * rng.n();
            let mut field = ex.field_dbuv_m + s.field_db * rng.n();
            if truth == Cause::EloranMeacon && i == target {
                ecd = st.ecd_us;
                field += rng.r(12.0, 24.0);
            }
            let ob = Observed {
                station: id.clone(),
                ecd_us: Some(ecd + 0.05 * rng.n()),
                sigma_ecd_us: 0.05,
                field_dbuv_m: Some(field + 0.3 * rng.n()),
                sigma_field_db: 0.3,
                ..Default::default()
            };
            test_station(&ob, &ex, None, &MeaconConfig::default())
        })
        .collect();
    let cfg = AttribConfig {
        offset_ref_ns: (DELTA, SIG_DELTA),
        ..Default::default()
    };
    let ep = AttribEpoch {
        gnss: &gnss,
        eloran: &eloran,
        clock_ns: (clock_pred, SIG_CLK),
        e4: &e4,
        approx_ecef: None,
    };
    (attribute(&ep, &cfg).unwrap(), attacked)
}

/// Confusion matrix, N cases per class. Stated rates: every attack class identified at
/// ≥ 95 % (station classes: with the right station); clean false alarm (any cause other
/// than clean) ≤ 2 %.
#[test]
fn confusion_matrix() {
    let ephs = parse_nav(NAV).unwrap();
    let m = model();
    let mut rng = Rng(0xE1);
    let n = 400;
    let mut table = vec![[0usize; 6]; 6];
    let mut right_station = [0usize; 6];
    for (ti, truth) in Cause::ALL.iter().enumerate() {
        for _ in 0..n {
            let (a, st) = case(&mut rng, &ephs, m.as_ref(), *truth);
            let pi = Cause::ALL.iter().position(|c| *c == a.cause).unwrap();
            table[ti][pi] += 1;
            if a.cause == *truth && (st.is_none() || a.station == st) {
                right_station[ti] += 1;
            }
            let sum: f64 = a.scores.iter().map(|s| s.1).sum();
            assert!((sum - 1.0).abs() < 1e-9);
        }
    }
    println!("truth \\ attributed: clean spoof jam fault meacon clock");
    for (ti, row) in table.iter().enumerate() {
        println!(
            "{:<22} {:?}  (right station {})",
            Cause::ALL[ti].label(),
            row,
            right_station[ti]
        );
    }
    let fa = 1.0 - table[0][0] as f64 / n as f64;
    assert!(fa <= 0.02, "clean false-alarm rate {fa}");
    for ti in 1..6 {
        let rate = right_station[ti] as f64 / n as f64;
        assert!(
            rate >= 0.95,
            "{}: identified {rate} ({:?})",
            Cause::ALL[ti].label(),
            table[ti]
        );
    }
}

#[test]
fn explanation_and_exclusion_follow_the_cause() {
    let ephs = parse_nav(NAV).unwrap();
    let m = model();
    let mut rng = Rng(7);
    let (a, _) = case(&mut rng, &ephs, m.as_ref(), Cause::Clean);
    assert_eq!(a.cause, Cause::Clean);
    assert!(!a.alarm);
    assert_eq!(a.exclusion, Exclusion::Nothing);
    let (a, st) = case(&mut rng, &ephs, m.as_ref(), Cause::EloranMeacon);
    assert_eq!(a.cause, Cause::EloranMeacon);
    assert!(a.alarm);
    assert_eq!(a.exclusion, Exclusion::Station(st.clone().unwrap()));
    assert!(a
        .explanation
        .iter()
        .any(|l| l.contains("propagation signature")));
    assert!(a
        .explanation
        .iter()
        .any(|l| l.contains(st.as_deref().unwrap())));
    let (a, _) = case(&mut rng, &ephs, m.as_ref(), Cause::ClockFault);
    assert_eq!(a.exclusion, Exclusion::Clock);
    let (a, _) = case(&mut rng, &ephs, m.as_ref(), Cause::GnssSpoof);
    assert_eq!(a.exclusion, Exclusion::Gnss);
    assert!(a.score(Cause::GnssSpoof) > 0.5);
}

/// E3 wired to E1: the ASF learner takes GNSS-derived truth and the attribution alarm as
/// its gate. A GNSS time drag (0.5 ns/s) after 2 h of clean learning: E1 attributes it to
/// GNSS spoofing once the clock disagreement is significant, the learner rolls back, and
/// the learnt ASF moves ≤ 15 ns (stated) while an ungated learner would follow the drag.
#[test]
fn asf_learning_gated_by_the_attribution_alarm() {
    use kshana_eloran::geo::inverse;
    use kshana_eloran::guard::asf_learn::{AsfLearnConfig, AsfLearner, AsfObs, Gate, StationPrior};
    let ephs = parse_nav(NAV).unwrap();
    let mut rng = Rng(0xE3);
    let rx = Geo {
        lat_deg: 55.0,
        lon_deg: 3.0,
        h_m: 30.0,
    };
    let sts = stations();
    let eta = kshana_eloran::prop::ground::N_ATM;
    // map ASF (ns) and the true ASF (map + 40 ns σ error)
    let map: Vec<f64> = (0..sts.len()).map(|i| 200.0 + 90.0 * i as f64).collect();
    let truth: Vec<f64> = map.iter().map(|m| m + 40.0 * rng.n()).collect();
    let priors: Vec<StationPrior> = sts
        .iter()
        .zip(&map)
        .map(|((id, p), m)| StationPrior {
            id: id.clone(),
            asf_ns: *m,
            sigma_ns: 40.0,
            bearing_rad: inverse(rx, *p).az1_rad,
        })
        .collect();
    // rely on E1 alone here (CUSUM off)
    let cfg = AsfLearnConfig {
        cusum_h: 1e9,
        ..Default::default()
    };
    let mut learner = AsfLearner::new(priors, cfg).unwrap();
    let acfg = AttribConfig {
        offset_ref_ns: (DELTA, SIG_DELTA),
        ..Default::default()
    };
    let t0 = 7200.0;
    let mut pre = vec![];
    let mut first_alarm = None;
    for k in 0..1080 {
        let t = 10.0 * k as f64;
        if t == t0 {
            pre = learner.asf_ns().to_vec();
        }
        let push = if t >= t0 { 0.5 * (t - t0) } else { 0.0 };
        let b_ns = 1000.0 + 0.01 * t; // receiver clock − GPS time (a predictable oscillator)
        let mut gnss = broadcast_ranges(
            &ephs,
            TOW + t,
            geo_to_ecef(rx),
            (b_ns + push) * 1e-9 * C,
            10.0,
            SIG_G,
        );
        for g in &mut gnss {
            g.range_m += SIG_G * rng.n();
        }
        // eLoran ranges with the true ASF, corrected with the map for E1
        let mut el = model_eloran_ranges(&sts, rx, (b_ns + DELTA) * 1e-9 * C, SIG_E);
        let mut pd_ns = vec![];
        for (i, e) in el.iter_mut().enumerate() {
            let meas = e.range_m + (truth[i] + 15.0 * rng.n()) * 1e-9 * C;
            pd_ns.push(meas / C * 1e9);
            e.range_m = meas - map[i] * 1e-9 * C;
        }
        let ep = AttribEpoch {
            gnss: &gnss,
            eloran: &el,
            clock_ns: (b_ns, SIG_CLK),
            e4: &[],
            approx_ecef: None,
        };
        let a = attribute(&ep, &acfg).unwrap();
        if a.alarm && first_alarm.is_none() {
            first_alarm = Some((t, a.cause));
        }
        // observed ASF from GNSS-derived truth: pd − PF·d − (b_G + δ_ref)
        let b_g = b_ns + push + 3.0 * rng.n();
        let obs: Vec<AsfObs> = sts
            .iter()
            .enumerate()
            .map(|(i, (_, p))| AsfObs {
                index: i,
                asf_ns: pd_ns[i] - eta * inverse(rx, *p).distance_m / C * 1e9 - (b_g + DELTA),
                sigma_ns: 20.0,
            })
            .collect();
        learner.update(
            t,
            &obs,
            Gate {
                tpl_ns: 40.0,
                attribution_alarm: a.alarm,
            },
        );
    }
    let (ta, cause) = first_alarm.expect("E1 raised no alarm during the drag");
    assert!(ta >= t0, "false alarm before the drag at {ta}");
    assert_eq!(cause, Cause::GnssSpoof);
    let shift = learner
        .asf_ns()
        .iter()
        .zip(&pre)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!(
        "E1→E3: first alarm {:.0} s after drag start ({cause:?}); max ASF shift {shift:.1} ns",
        ta - t0
    );
    assert!(shift <= 15.0, "learnt ASF moved {shift} ns");
}

/// `kshana-pro eloran guard attribute` on a JSON epoch: a self-consistent GNSS time push
/// is attributed to GNSS spoofing in the written report.
#[test]
fn cli_guard_attribute() {
    let ephs = parse_nav(NAV).unwrap();
    let rx = Geo {
        lat_deg: 55.0,
        lon_deg: 3.0,
        h_m: 30.0,
    };
    let b_ns = 2500.0;
    let gnss = broadcast_ranges(
        &ephs,
        TOW,
        geo_to_ecef(rx),
        (b_ns + 1500.0) * 1e-9 * C,
        10.0,
        SIG_G,
    );
    let eloran = model_eloran_ranges(&stations(), rx, (b_ns + DELTA) * 1e-9 * C, SIG_E);
    let input = serde_json::json!({
        "gnss": gnss,
        "eloran": eloran,
        "clock_ns": [b_ns, SIG_CLK],
        "config": AttribConfig { offset_ref_ns: (DELTA, SIG_DELTA), ..Default::default() },
    });
    let dir = std::env::temp_dir().join(format!("kshana-guard-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let inp = dir.join("epoch.json");
    let out = dir.join("report.json");
    std::fs::write(&inp, serde_json::to_string(&input).unwrap()).unwrap();
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(["eloran", "guard", "attribute"])
        .arg(&inp)
        .arg("--out")
        .arg(&out)
        .arg("--dev")
        .status()
        .unwrap();
    assert!(st.success());
    let rep: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(rep["cause"], "gnss_spoof");
    assert_eq!(rep["alarm"], true);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The patent-candidate `guard` method is node-locked (`require_bound`): with neither a
/// `--license` token nor the debug `--dev` bypass it refuses to run and names the
/// machine-bound requirement, so a hardened on-prem build cannot execute it off a
/// licensed, machine-bound seat. (`--dev` success is covered by `cli_guard_attribute`.)
#[test]
fn cli_guard_requires_bound_licence() {
    let dir = std::env::temp_dir().join(format!("kshana-guard-nolic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let inp = dir.join("epoch.json");
    let out = dir.join("report.json");
    // A minimal, well-formed epoch: the gate must fire before the body parses it.
    std::fs::write(&inp, "{}").unwrap();
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(["eloran", "guard", "attribute"])
        .arg(&inp)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!o.status.success(), "unlicensed patent method must fail");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("machine-bound"),
        "error should name the machine-bound requirement, got: {err}"
    );
    assert!(!out.exists(), "no report should be written when the gate fails");
    let _ = std::fs::remove_dir_all(&dir);
}
