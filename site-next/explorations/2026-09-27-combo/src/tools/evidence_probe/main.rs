// Evidence probe: runs the engine's own validated modules against the same external oracles
// the engine's tests use, and prints every compared value (the tests print only a pass or the
// worst case). Built and run by src/tools/gen_evidence_data.py against a Kshana source tree;
// it links the `kshana` crate by path and never writes into that tree.
//
// usage: evidence_probe <kshana-tree> <raim-cases.txt> [<cs5071a-phase.txt>]
// output: one JSON object on stdout.
use kshana::allan::{hadamard_adev, modified_adev, overlapping_adev, time_deviation};
use kshana::propagator::{propagate, ForceModel};
use kshana::integrator::Tolerance;
use kshana::raim::{chi2_cdf, chi2_quantile, noncentral_chi2_cdf, normal_cdf, normal_quantile, pbias};
use kshana::sgp4::wgs72;
use kshana::slot_timing::{slot_budget, ClockNoise, SlotConditions};
use kshana::tle::parse_tle;
use std::fmt::Write as _;

fn num(x: f64) -> String {
    if x.is_finite() { format!("{:e}", x) } else { "null".into() }
}

fn arr(v: &[f64]) -> String {
    let s: Vec<String> = v.iter().map(|x| num(*x)).collect();
    format!("[{}]", s.join(","))
}

// ------------------------------------------------------------------ SGP4 vs AIAA 2006-6753
fn sgp4(tree: &str) -> String {
    let tle = std::fs::read_to_string(format!("{tree}/tests/fixtures/sgp4/SGP4-VER.TLE")).unwrap();
    let out = std::fs::read_to_string(format!("{tree}/tests/fixtures/sgp4/tcppver.out")).unwrap();
    let mut cases = Vec::new();
    let mut l1: Option<String> = None;
    for line in tle.lines() {
        if line.starts_with('#') { continue; }
        if line.starts_with("1 ") { l1 = Some(line.to_string()); }
        else if line.starts_with("2 ") { if let Some(a) = l1.take() { cases.push((a, line.to_string())); } }
    }
    let mut blocks: Vec<Vec<(f64, [f64; 3])>> = Vec::new();
    for line in out.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() == 2 && t[1] == "xx" { blocks.push(Vec::new()); continue; }
        if t.len() >= 7 {
            let p: Vec<f64> = t[..4].iter().filter_map(|x| x.parse().ok()).collect();
            if p.len() == 4 { if let Some(b) = blocks.last_mut() { b.push((p[0], [p[1], p[2], p[3]])); } }
        }
    }
    assert_eq!(cases.len(), blocks.len());
    let grav = wgs72();
    let mut s = String::from("[");
    let mut first = true;
    for ((a, b), rows) in cases.iter().zip(blocks.iter()) {
        let satnum = a[2..7].trim().to_string();
        // mean motion (rev/day), TLE line 2 columns 53-63: period >= 225 min is deep space (SDP4)
        let n: f64 = b[52..63].trim().parse().unwrap_or(0.0);
        let deep = n > 0.0 && 1440.0 / n >= 225.0;
        let prop = parse_tle(a, b).unwrap().to_sgp4(grav, false);
        let mut errs = Vec::new();
        for (ts, p) in rows {
            if let Ok((q, _)) = prop.propagate(*ts) {
                errs.push(((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt() * 1000.0);
            }
        }
        if errs.is_empty() { continue; }
        if !first { s.push(','); }
        first = false;
        let _ = write!(s, "{{\"sat\":\"{satnum}\",\"deep\":{deep},\"err_m\":{}}}", arr(&errs));
    }
    s.push(']');
    s
}

// ------------------------------------------------------------------ Cowell vs Orekit 12.2 DP853
fn cowell(tree: &str) -> String {
    let txt = std::fs::read_to_string(format!(
        "{tree}/tests/fixtures/numerical_cowell_propagator/numerical_cowell_propagator_reference.txt")).unwrap();
    let csv3 = |s: &str| -> [f64; 3] {
        let v: Vec<f64> = s.trim().split(',').map(|x| x.trim().parse().unwrap()).collect();
        [v[0], v[1], v[2]]
    };
    let tol = Tolerance { rtol: 1e-12, atol: 1e-9, ..Tolerance::default() };
    let model = |tier: &str| -> ForceModel {
        let e = 2_451_545.0;
        match tier {
            "T1" => ForceModel::two_body(),
            "T2" => ForceModel::with_j2(),
            "T3" => ForceModel::with_zonals_j2_j6(),
            "T4" => ForceModel::with_zonals_j2_j6().third_body(true, true, e),
            "T5" => ForceModel::with_zonals_j2_j6().third_body(true, true, e).solar_radiation(1.5, 0.02),
            _ => ForceModel::with_zonals_j2_j6().third_body(true, true, e).solar_radiation(1.5, 0.02).drag(0.02),
        }
    };
    let mut s = String::from("[");
    let mut first = true;
    for line in txt.lines().filter(|l| l.starts_with("CASE ")) {
        let f: Vec<&str> = line.split('|').collect();
        let head: Vec<&str> = f[0].split_whitespace().collect();
        let (tier, regime) = (head[1], head[2]);
        let r0 = csv3(f[1].trim().trim_start_matches("r0="));
        let v0 = csv3(f[2].trim().trim_start_matches("v0="));
        let m = model(tier);
        let prefix = format!("STATE {tier} {regime} ");
        let (mut ts, mut er) = (Vec::new(), Vec::new());
        for st in txt.lines().filter(|l| l.starts_with(&prefix)) {
            let g: Vec<&str> = st.split('|').collect();
            let t: f64 = g[0].split_whitespace().nth(4).unwrap().parse().unwrap();
            let rr = csv3(g[1]);
            let (rk, _) = propagate(r0, v0, t, &m, &tol);
            ts.push(t);
            er.push(((rk[0] - rr[0]).powi(2) + (rk[1] - rr[1]).powi(2) + (rk[2] - rr[2]).powi(2)).sqrt());
        }
        if !first { s.push(','); }
        first = false;
        let _ = write!(s, "{{\"tier\":\"{tier}\",\"regime\":\"{regime}\",\"t_s\":{},\"err_m\":{}}}", arr(&ts), arr(&er));
    }
    s.push(']');
    s
}

// ------------------------------------------------------------------ Allan family on NIST SP 1065 1000-point set
fn allan() -> String {
    let (md, mul) = (2_147_483_647i64, 16_807i64);
    let mut n = 1_234_567_890i64;
    let mut phase = vec![0.0];
    let mut acc = 0.0;
    for _ in 0..1000 {
        acc += n as f64 / md as f64;
        phase.push(acc);
        n = (mul * n) % md;
    }
    let ms: Vec<usize> = vec![1, 2, 3, 4, 5, 7, 10, 14, 20, 30, 40, 50, 70, 100, 140, 200, 250, 300, 333];
    let mut o = Vec::new(); let mut mo = Vec::new(); let mut td = Vec::new(); let mut hd = Vec::new();
    for &m in &ms {
        o.push(overlapping_adev(&phase, 1.0, m));
        mo.push(modified_adev(&phase, 1.0, m));
        td.push(time_deviation(&phase, 1.0, m));
        hd.push(if 3 * m < phase.len() { hadamard_adev(&phase, 1.0, m) } else { f64::NAN });
    }
    let msf: Vec<f64> = ms.iter().map(|m| *m as f64).collect();
    format!("{{\"m\":{},\"oadev\":{},\"mdev\":{},\"tdev\":{},\"ohdev\":{}}}", arr(&msf), arr(&o), arr(&mo), arr(&td), arr(&hd))
}

// ------------------------------------------------------------------ RAIM statistical kernel vs SciPy
fn raim(cases: &str) -> String {
    let mut s = String::from("[");
    let mut first = true;
    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let v: Vec<f64> = t[1..].iter().map(|x| x.parse().unwrap()).collect();
        let (abst, relt) = (v[v.len() - 2], v[v.len() - 1]);
        let want = v[v.len() - 3];
        let got = match t[0] {
            "chi2_cdf" => chi2_cdf(v[0], v[1]),
            "chi2_quantile" => chi2_quantile(v[0], v[1]),
            "normal_cdf" => normal_cdf(v[0]),
            "normal_quantile" => normal_quantile(v[0]),
            "noncentral_chi2_cdf" => noncentral_chi2_cdf(v[0], v[1], v[2]),
            "pbias" => pbias(v[0], v[1], v[2]),
            f => panic!("unknown kernel {f}"),
        };
        let band = abst + relt * want.abs();
        if !first { s.push(','); }
        first = false;
        let _ = write!(s, "{{\"fn\":\"{}\",\"want\":{},\"got\":{},\"band\":{}}}", t[0], num(want), num(got), num(band));
    }
    s.push(']');
    s
}

// ------------------------------------------------------------------ holdover prediction on the held-out Cs 5071A record
fn holdover(path: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else { return "null".into() };
    let x: Vec<f64> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| l.parse().unwrap()).collect();
    let split = x.len() / 3;
    let noise = ClockNoise::from_phase_record(&x[..split], 1.0).unwrap();
    let fix = noise.white_pm_var.sqrt();
    let cond = |g: f64, f: f64| SlotConditions { guard_s: g, k_sigma: 1.0, fix_sigma_s: f, fix_frequency_sigma: 0.0,
        elapsed_since_sync_s: 0.0, fix_latency_s: 0.0, residual_frequency_offset: 0.0, temperature_excursion_k: 0.0 };
    let (t_max, t_step, sync_step) = (80_000usize, 50usize, 997usize);
    let syncs: Vec<usize> = (split..x.len() - t_max).step_by(sync_step).collect();
    let curve: Vec<(f64, f64)> = (1..=t_max / t_step).map(|i| {
        let t = i * t_step;
        let ss: f64 = syncs.iter().map(|&s| { let yh = (x[s] - x[s - split]) / split as f64; let e = x[s + t] - x[s] - yh * t as f64; e * e }).sum();
        (t as f64, (ss / syncs.len() as f64).sqrt())
    }).collect();
    let breach = |thr: f64| -> f64 {
        let mut prev = (0.0, 0.0);
        for &(t, r) in &curve { if r >= thr { return prev.0 + (thr - prev.1) / (r - prev.1) * (t - prev.0); } prev = (t, r); }
        f64::NAN
    };
    let thr = [0.5e-9, 0.75e-9, 1.0e-9, 1.5e-9, 2.0e-9, 2.5e-9];
    let mut rows = Vec::new();
    for &g in &thr {
        let p = slot_budget(&noise, &cond(g, fix)).unwrap().breach_after_sync_s;
        let m = breach(g);
        rows.push(format!("{{\"thr_ns\":{},\"predicted_s\":{},\"measured_s\":{},\"ratio\":{}}}", num(g * 1e9), num(p), num(m), num(p / m)));
    }
    // the predicted one-sigma curve: breach time for a dense ladder of thresholds
    let mut pc = Vec::new();
    let mut g = 0.2e-9;
    while g <= 3.0e-9 { let p = slot_budget(&noise, &cond(g, fix)).unwrap().breach_after_sync_s; if p.is_finite() && p > 0.0 && p <= t_max as f64 { pc.push(format!("[{},{}]", num(p), num(g * 1e9))); } g *= 1.08; }
    let a1 = overlapping_adev(&x[..split], 1.0, 1);
    let naive = ClockNoise::from_ieee1139(0.0, 0.0, 2.0 * a1 * a1);
    let nb = slot_budget(&naive, &cond(1.0e-9, 0.0)).unwrap().breach_after_sync_s;
    let mc: Vec<String> = curve.iter().step_by(8).map(|(t, r)| format!("[{},{}]", num(*t), num(r * 1e9))).collect();
    format!("{{\"samples\":{},\"split\":{},\"syncs\":{},\"rows\":[{}],\"naive_ratio_1ns\":{},\"measured\":[{}],\"predicted\":[{}]}}",
        x.len(), split, syncs.len(), rows.join(","), num(nb / breach(1.0e-9)), mc.join(","), pc.join(","))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let tree = &a[1];
    let cases = std::fs::read_to_string(&a[2]).unwrap();
    let hold = a.get(3).map(|p| holdover(p)).unwrap_or_else(|| "null".into());
    println!("{{\"sgp4\":{},\"cowell\":{},\"allan\":{},\"raim\":{},\"holdover\":{}}}", sgp4(tree), cowell(tree), allan(), raim(&cases), hold);
}
