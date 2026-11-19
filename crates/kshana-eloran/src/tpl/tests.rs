// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Monte Carlo coverage tests of the TPL. The bound is always built from one seed and
//! tested on independent "true worlds" drawn with another seed, so a pass is not the bound
//! fitting its own samples. Each claim is a one-sided binomial test at level 1e-3 (fixed
//! seeds: the outcome is reproducible).

use super::*;
use crate::chain::StationStatus;
use crate::prop::conductivity::{ConductivityConfig, GroundConst, PolygonConfig};
use crate::prop::ground::K_LORAN;
use crate::signal::Role;
use crate::timebase::gauss;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::sync::OnceLock;

const ALPHA: f64 = 1e-3;

fn station(id: &str, lat: f64, lon: f64, tref: &str) -> Station {
    Station {
        id: id.into(),
        name: id.into(),
        role: if id == "M" {
            Role::Master
        } else {
            Role::Secondary
        },
        position: Geo::new(lat, lon),
        power_kw: 400.0,
        emission_delay_us: 0.0,
        time_ref: tref.into(),
        tx_clock: None,
        ecd_us: 0.0,
        status: StationStatus::On,
        faults: vec![],
    }
}

/// Four coastal stations around a large land mass (σ = 3 mS/m): every path to an inland
/// receiver crosses the same land class.
fn stations() -> Vec<Station> {
    vec![
        station("M", 49.0, 0.0, "UTC(A)"),
        station("X", 56.5, 0.5, "UTC(A)"),
        station("Y", 52.5, -7.0, "UTC(A)"),
        station("Z", 52.3, 7.0, "UTC(A)"),
    ]
}

fn land() -> ConductivityMap {
    ConductivityMap::build(&ConductivityConfig::Polygons {
        background: GroundConst::seawater(),
        polygons: vec![PolygonConfig {
            name: "land".into(),
            sigma_s_m: 0.003,
            eps_r: Some(15.0),
            vertices: vec![[50.0, -5.0], [55.0, -5.0], [55.0, 5.0], [50.0, 5.0]],
        }],
    })
    .unwrap()
}

fn map() -> &'static ConductivityMap {
    static M: OnceLock<ConductivityMap> = OnceLock::new();
    M.get_or_init(land)
}

const INLAND: Geo = Geo {
    lat_deg: 52.5,
    lon_deg: 0.0,
    h_m: 0.0,
};

/// Integrity risk such that each of the n + 2 hypotheses gets 5 %, and priors such that the
/// station and ASF hypotheses must hold with 10 % missed detection: rates a few hundred
/// trials can test (each propagation sample costs about 0.1 s, so the counts are modest).
fn test_cfg() -> TplConfig {
    TplConfig {
        integrity_risk: 0.3,
        false_alert: 0.02,
        p_station_fault: 0.5,
        p_asf_model_fault: 0.5,
        samples: 300,
        seed: 1,
        ..Default::default()
    }
}

fn problem(points: Vec<Geo>) -> TplProblem<'static> {
    TplProblem {
        stations: stations(),
        points,
        map: map(),
        k_factor: K_LORAN,
        sample_km: 5.0,
    }
}

/// Bound at the inland point (built from seed 1).
fn built() -> &'static (TplResult, Monitor) {
    static B: OnceLock<(TplResult, Monitor)> = OnceLock::new();
    B.get_or_init(|| {
        compute(&problem(vec![INLAND]), &test_cfg())
            .unwrap()
            .remove(0)
    })
}

/// Independent true worlds: propagation errors (other seed) plus receiver and transmitter
/// noise.
fn worlds(seed: u64, n: usize, extra_log10: f64) -> Vec<Vec<f64>> {
    let cfg = test_cfg();
    let mut u = cfg.engine_cfg(seed, extra_log10);
    u.samples = n;
    let es = error_samples(&stations(), &[INLAND], map(), K_LORAN, 5.0, u)
        .unwrap()
        .remove(0);
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xA11CE);
    es.e.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|x| {
                    x + cfg.meas_sigma_ns * gauss(&mut rng) + cfg.tx_sigma_ns * gauss(&mut rng)
                })
                .collect()
        })
        .collect()
}

fn h0_worlds() -> &'static Vec<Vec<f64>> {
    static W: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    W.get_or_init(|| worlds(9_001, 600, 0.0))
}

/// Weights and claimed σ of a solution that assumes independent station errors.
fn independent(r: &TplResult) -> (Vec<f64>, f64) {
    let inv: Vec<f64> = r
        .stations
        .iter()
        .map(|s| 1.0 / (s.asf_sigma_ns.powi(2) + 50.0))
        .collect();
    let t: f64 = inv.iter().sum();
    (inv.iter().map(|x| x / t).collect(), (1.0 / t).sqrt())
}

#[test]
fn normal_quantile_and_binomial_critical_values() {
    assert!((norm_quantile(0.975) - 1.959_963_984_540_054).abs() < 1e-8);
    assert!((k_two_sided(1e-7) - 5.326_723_886_278_399).abs() < 1e-6);
    assert!((norm_quantile(0.01) + 2.326_347_874_040_841).abs() < 1e-8);
    assert!((norm_cdf(1.96) - 0.975_002_104_851_78).abs() < 2e-7);
    assert_eq!(k_two_sided(1.0), 0.0);
    // Bin(1000, 0.01): P(X ≥ 22) ≤ 1e-3 < P(X ≥ 21) (reference computed exactly)
    assert_eq!(binomial_critical(1000, 0.01, 1e-3), 21);
    // a Gaussian sample is overbounded with inflation near 1; a heavy tail inflates
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let g: Vec<f64> = (0..4000).map(|_| gauss(&mut rng)).collect();
    let (_, s, infl) = overbound(&g);
    assert!(infl < 1.15 && (s - 1.0).abs() < 0.15, "{infl} {s}");
    let heavy: Vec<f64> = g.iter().map(|x| x * x.abs()).collect();
    assert!(overbound(&heavy).2 > 1.3);
}

#[test]
fn shared_ground_classes_correlate_station_errors() {
    let (r, _) = built();
    assert_eq!(r.stations.len(), 4);
    // every path crosses the one land class: strongly correlated delay errors
    for i in 0..4 {
        for j in 0..4 {
            if i != j {
                assert!(r.correlation[i][j] > 0.5, "{:?}", r.correlation);
            }
        }
        assert!(r.stations[i].asf_sigma_ns > 5.0, "{:?}", r.stations[i]);
    }
    // the independent-error solution's true σ (with the correlations) is well above the σ
    // it claims
    let (wd, claimed) = independent(r);
    let n = r.stations.len();
    let mut true_var = 0.0;
    for i in 0..n {
        for j in 0..n {
            let (si, sj) = (r.stations[i].asf_sigma_ns, r.stations[j].asf_sigma_ns);
            let c = r.correlation[i][j] * si * sj + if i == j { 50.0 } else { 0.0 };
            true_var += wd[i] * wd[j] * c;
        }
    }
    assert!(
        true_var.sqrt() > 1.5 * claimed,
        "{} vs {claimed}",
        true_var.sqrt()
    );
    assert!(r.tpl_ns.is_finite() && r.tpl_ns > r.sigma_ns);
    assert_eq!(r.hypotheses.len(), 6);
    assert!(r.hypotheses.iter().all(|h| h.tpl_ns <= r.tpl_ns));
    // JSON round trip
    let j = serde_json::to_string(r).unwrap();
    let back: TplResult = serde_json::from_str(&j).unwrap();
    assert_eq!(back.tpl_ns, r.tpl_ns);
}

#[test]
fn fault_free_monte_carlo_coverage() {
    let (r, mon) = built();
    let h0 = r.hypothesis("H0 fault-free").unwrap();
    let w = h0_worlds();
    let n = w.len();
    let err: Vec<f64> = w.iter().map(|e| mon.error(e)).collect();
    let exceed = |b: f64| err.iter().filter(|x| x.abs() > b).count();
    // H0 component: |error| > TPL₀ at no more than its allocated 5 %
    let k0 = exceed(h0.tpl_ns);
    let crit0 = binomial_critical(n, h0.p_md, ALPHA);
    assert!(k0 <= crit0, "H0: {k0} > {crit0} of {n}");
    // full TPL at the stated integrity risk
    let k = exceed(r.tpl_ns);
    assert!(k <= binomial_critical(n, r.integrity_risk, ALPHA), "{k}");
    // not vacuous: half the bound is exceeded more often than the allocation (the symmetric
    // Gaussian overbound of a skewed log-normal-driven error is conservative, not absurd)
    assert!(
        exceed(h0.tpl_ns / 2.0) as f64 > h0.p_md * n as f64,
        "bound too loose: {}",
        exceed(h0.tpl_ns / 2.0)
    );
    // a receiver assuming independent station errors (diagonal covariance, its own
    // weights and σ) under-bounds: shared ground classes make the errors move together
    let (wd, sd) = independent(r);
    let bias: f64 = wd
        .iter()
        .zip(&r.stations)
        .map(|(w, s)| w * s.asf_bias_ns)
        .sum();
    let naive = k_two_sided(h0.p_md) * sd + bias.abs();
    let kn = w
        .iter()
        .filter(|e| e.iter().zip(&wd).map(|(a, b)| a * b).sum::<f64>().abs() > naive)
        .count();
    assert!(
        kn > crit0,
        "independent-error bound should fail coverage: {kn} ≤ {crit0} (naive {naive}, TPL₀ {})",
        h0.tpl_ns
    );
}

#[test]
fn station_fault_is_detected_or_bounded() {
    let (r, mon) = built();
    let w = h0_worlds();
    let n = w.len();
    let hi = r.hypothesis("H1 station M faulty").unwrap();
    let t = hi.threshold_ns.unwrap();
    let levels = [0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 10.0];
    let crit = binomial_critical(n, hi.p_md, ALPHA / (2 * levels.len()) as f64);
    // the separation of station M scales the bias by |w_M − w_M⁽ᴹ⁾| = w_M
    let scale = 1.0 / r.stations[0].weight.max(1e-3);
    for sgn in [1.0, -1.0] {
        for l in levels {
            let b = sgn * l * t * scale;
            let mut hmi = 0;
            let mut detected = 0;
            for e in w.iter().take(n) {
                let mut e = e.clone();
                e[0] += b;
                let alarm = mon.alarms(&e).iter().any(|a| *a);
                detected += alarm as usize;
                if !alarm && mon.error(&e).abs() > r.tpl_ns {
                    hmi += 1;
                }
            }
            assert!(hmi <= crit, "bias {b:.1} ns: {hmi} missed > {crit}");
            if l >= 5.0 {
                assert!(detected as f64 > 0.99 * n as f64, "bias {b}: {detected}");
            }
        }
    }
    // fault-free false alerts stay near the allowed rate
    let fa = w
        .iter()
        .filter(|e| mon.alarms(e).iter().any(|a| *a))
        .count();
    assert!(
        fa <= binomial_critical(w.len(), test_cfg().false_alert, ALPHA),
        "false alerts {fa}"
    );
}

#[test]
fn asf_model_error_is_bounded() {
    let (r, mon) = built();
    let ha = r.hypothesis("H_ASF model error").unwrap();
    let h0 = r.hypothesis("H0 fault-free").unwrap();
    let w = worlds(31_337, 300, test_cfg().asf_model_error_log10);
    let err: Vec<f64> = w.iter().map(|e| mon.error(e)).collect();
    let k = err.iter().filter(|x| x.abs() > r.tpl_ns).count();
    assert!(k <= binomial_critical(w.len(), ha.p_md, ALPHA), "{k}");
    // the model-error worlds really are wider than the fault-free ones, and so is the term
    let sd = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
    };
    let e0: Vec<f64> = h0_worlds().iter().map(|e| mon.error(e)).collect();
    assert!(sd(&err) > 1.2 * sd(&e0), "{} vs {}", sd(&err), sd(&e0));
    assert!(ha.sigma_ns > h0.sigma_ns);
    assert!(ha.tpl_ns > 0.0 && ha.p_md < 1.0);
    let _ = h0;
    // common mode: the station monitor cannot see it (alarms near the false-alert rate)
    let alarms = w
        .iter()
        .filter(|e| mon.alarms(e).iter().any(|a| *a))
        .count();
    assert!((alarms as f64) < 0.2 * w.len() as f64, "{alarms}");
}

#[test]
fn single_station_time_reference_is_unmonitorable() {
    let mut st = stations();
    st[3].time_ref = "UTC(B)".into();
    let mut cfg = test_cfg();
    cfg.samples = 100;
    cfg.time_ref = Some("UTC(B)".into());
    let p = TplProblem {
        stations: st.clone(),
        points: vec![INLAND],
        map: map(),
        k_factor: K_LORAN,
        sample_km: 5.0,
    };
    let (r, _) = compute(&p, &cfg).unwrap().remove(0);
    assert_eq!(r.stations.len(), 1);
    assert_eq!(r.stations[0].station, "Z");
    assert!(r.tpl_ns.is_infinite() && !r.available);
    // a station prior within the allocation needs no monitor
    cfg.p_station_fault = 1e-4;
    let (r, _) = compute(&p, &cfg).unwrap().remove(0);
    assert!(r.tpl_ns.is_finite());
    // UTC(A) uses the other three
    cfg.time_ref = Some("UTC(A)".into());
    let (r, _) = compute(&p, &cfg).unwrap().remove(0);
    assert_eq!(r.stations.len(), 3);
    cfg.time_ref = Some("UTC(C)".into());
    assert!(compute(&p, &cfg).is_err());
}

#[test]
fn seasonal_variation_widens_the_bound() {
    let mut cfg = test_cfg();
    cfg.samples = 150;
    cfg.seasonal_sigma_frac = 0.0;
    let a = compute(&problem(vec![INLAND]), &cfg).unwrap().remove(0).0;
    cfg.seasonal_sigma_frac = 0.4;
    let b = compute(&problem(vec![INLAND]), &cfg).unwrap().remove(0).0;
    assert!(b.sigma_ns > a.sigma_ns, "{} {}", b.sigma_ns, a.sigma_ns);
    // and the default is deterministic
    let c = compute(&problem(vec![INLAND]), &cfg).unwrap().remove(0).0;
    assert_eq!(b, c);
}

#[test]
fn availability_map_follows_the_land_path() {
    let mut cfg = test_cfg();
    cfg.samples = 80;
    cfg.integrity_risk = 1e-7;
    cfg.p_station_fault = 1e-5;
    cfg.p_asf_model_fault = 1e-4;
    // a north–south line from near the south coast (50.2°) to deep inland (52.6°)
    let m = availability_map(
        &stations(),
        [50.2, 52.6, 0.0, 0.0],
        0.8,
        map(),
        K_LORAN,
        5.0,
        &cfg,
    )
    .unwrap();
    assert_eq!(m.points.len(), 4);
    assert!(
        m.points.last().unwrap().tpl_ns > m.points[0].tpl_ns,
        "{:?}",
        m.points
    );
    // availability is monotone in the alert limit
    let tpls: Vec<f64> = m.points.iter().map(|p| p.tpl_ns).collect();
    let mid = (tpls[0] + tpls[3]) / 2.0;
    cfg.alert_limit_ns = mid;
    let m2 = availability_map(
        &stations(),
        [50.2, 52.6, 0.0, 0.0],
        0.8,
        map(),
        K_LORAN,
        5.0,
        &cfg,
    )
    .unwrap();
    assert!(m2.availability > 0.0 && m2.availability < 1.0, "{:?}", m2);
    cfg.alert_limit_ns = 1e9;
    let m3 = availability_map(
        &stations(),
        [50.2, 52.6, 0.0, 0.0],
        0.8,
        map(),
        K_LORAN,
        5.0,
        &cfg,
    )
    .unwrap();
    assert_eq!(m3.availability, 1.0);
    assert!(availability_map(
        &stations(),
        [0.0, 30.0, 0.0, 30.0],
        0.1,
        map(),
        K_LORAN,
        5.0,
        &cfg
    )
    .is_err());
}
