// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! eLoran-disciplined oscillator (eDO) bench and GNSS → eLoran switchover transients
//! (NAVISP EL1-145 class work; research 01 gaps 17 and 25).
//!
//! * **Oscillator**: phase x(t) from an initial fractional frequency offset, linear ageing and
//!   the power-law noises white PM, white FM and random-walk FM, set from their Allan
//!   deviation at τ = 1 s (σ_y(τ) = h·τ^(−1) for white PM (approx.), h·τ^(−1/2) for white FM,
//!   h·τ^(+1/2) for random-walk FM).
//! * **References**: GNSS time and eLoran time as observed by the receiver: each has white
//!   measurement noise, an optional constant bias (for eLoran: the residual ASF/calibration
//!   error) and availability windows.
//! * **Discipline**: a second-order type-2 loop (PI on phase) with time constant τ_loop,
//!   steering the oscillator frequency; holdover when no reference is available.
//! * **Switchover**: the reference changes from GNSS to eLoran at a given time (GNSS lost or
//!   declared spoofed). The time error transient is driven by the eLoran–GNSS offset that was
//!   not calibrated out; the report gives its peak, the settling time and MTIE/TDEV.
//!
//! Evidence: **MODELLED** — checked against closed forms (ageing holdover ½·a·t², loop
//! settling of a step for a type-2 loop, white-FM ADEV recovery). Not fitted to a real
//! oscillator or receiver; use measured ADEV and receiver noise for a real assessment.

use super::measure::{mtie_curve, tdev_curve, StabilityPoint};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Oscillator model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Oscillator {
    /// Initial fractional frequency offset.
    pub y0: f64,
    /// Linear ageing (fractional frequency per day).
    pub ageing_per_day: f64,
    /// ADEV(1 s) contribution of white phase noise.
    pub adev1_wpm: f64,
    /// ADEV(1 s) contribution of white frequency noise.
    pub adev1_wfm: f64,
    /// ADEV(1 s) contribution of random-walk frequency noise.
    pub adev1_rwfm: f64,
}

impl Default for Oscillator {
    /// A good OCXO-class oscillator.
    fn default() -> Self {
        Oscillator {
            y0: 1e-9,
            ageing_per_day: 5e-11,
            adev1_wpm: 1e-12,
            adev1_wfm: 1e-12,
            adev1_rwfm: 1e-13,
        }
    }
}

/// A time reference as observed by the receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reference {
    pub name: String,
    /// White measurement noise per epoch (ns, 1σ).
    pub noise_ns: f64,
    /// Constant bias against true time (ns): uncalibrated offset.
    pub bias_ns: f64,
    /// Available from/to (s); `None` = always.
    pub available: Vec<(f64, f64)>,
}

impl Default for Reference {
    fn default() -> Self {
        Reference {
            name: "ref".into(),
            noise_ns: 5.0,
            bias_ns: 0.0,
            available: vec![],
        }
    }
}

impl Reference {
    fn up(&self, t: f64) -> bool {
        self.available.is_empty() || self.available.iter().any(|(a, b)| t >= *a && t < *b)
    }
}

/// Bench configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdoConfig {
    pub tau0_s: f64,
    pub duration_s: f64,
    pub oscillator: Oscillator,
    pub gnss: Reference,
    pub eloran: Reference,
    /// GNSS is used until this time (s); after it eLoran (when available). `None`: GNSS only.
    pub switch_at_s: Option<f64>,
    /// Loop time constant (s).
    pub loop_tau_s: f64,
    /// Damping factor of the loop.
    pub damping: f64,
    /// Time-error limit used for the verdict (ns).
    pub time_error_limit_ns: f64,
    pub seed: u64,
}

impl Default for EdoConfig {
    fn default() -> Self {
        EdoConfig {
            tau0_s: 1.0,
            duration_s: 20_000.0,
            oscillator: Oscillator::default(),
            gnss: Reference {
                name: "GNSS".into(),
                noise_ns: 10.0,
                ..Default::default()
            },
            eloran: Reference {
                name: "eLoran".into(),
                noise_ns: 30.0,
                ..Default::default()
            },
            switch_at_s: None,
            loop_tau_s: 1000.0,
            damping: 0.707,
            time_error_limit_ns: 100.0,
            seed: 1,
        }
    }
}

/// Bench result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdoReport {
    /// Time error of the disciplined output against true time (ns), one per epoch.
    pub time_error_ns: Vec<f64>,
    /// Reference in use per epoch: "GNSS", "eLoran" or "holdover".
    pub source: Vec<String>,
    pub max_abs_te_ns: f64,
    pub rms_te_after_settle_ns: f64,
    /// Peak |TE| after the switchover and the time to settle within 2× the pre-switch RMS.
    pub switchover_peak_ns: Option<f64>,
    pub switchover_settle_s: Option<f64>,
    /// Mean time error the output settles to after the switchover (ns): the uncalibrated
    /// eLoran–GNSS offset as seen at the output.
    pub switchover_offset_ns: Option<f64>,
    pub mtie: Vec<StabilityPoint>,
    pub tdev: Vec<StabilityPoint>,
    pub within_limit: bool,
    pub evidence: String,
}

fn gauss(r: &mut ChaCha8Rng) -> f64 {
    let (u1, u2): (f64, f64) = (r.gen::<f64>().max(1e-300), r.gen());
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Free-running phase (ns) of the oscillator at each epoch (no discipline).
pub fn free_run(o: &Oscillator, tau0: f64, n: usize, seed: u64) -> Vec<f64> {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    let a = o.ageing_per_day / 86_400.0;
    let (mut x, mut y_rw) = (0.0f64, 0.0f64);
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let t = k as f64 * tau0;
        // White FM: frequency white with ADEV(τ0) = adev1·τ0^-1/2.
        let y_wfm = o.adev1_wfm / tau0.sqrt() * gauss(&mut r);
        // RWFM: frequency random walk, ADEV(τ) = adev1·sqrt(τ) → step σ = adev1·sqrt(3/τ0)... per epoch
        y_rw +=
            o.adev1_rwfm * (3.0 * tau0).sqrt() / tau0 * gauss(&mut r) * tau0.sqrt() / 3f64.sqrt();
        let y = o.y0 + a * t + y_wfm + y_rw;
        out.push(x * 1e9 + o.adev1_wpm * tau0 / 3f64.sqrt() * 1e9 * gauss(&mut r));
        x += y * tau0;
    }
    out
}

/// Run the bench.
pub fn run(cfg: &EdoConfig) -> Result<EdoReport, String> {
    if cfg.tau0_s <= 0.0 || cfg.duration_s <= cfg.tau0_s || cfg.loop_tau_s <= cfg.tau0_s {
        return Err("tau0, duration and loop time constant must be positive and ordered".into());
    }
    let n = (cfg.duration_s / cfg.tau0_s) as usize;
    let osc = free_run(&cfg.oscillator, cfg.tau0_s, n, cfg.seed);
    let mut r = ChaCha8Rng::seed_from_u64(cfg.seed ^ 0x9e37_79b9_7f4a_7c15);
    // Type-2 loop gains (phase in ns, steering in ns/s): natural frequency from τ_loop.
    let wn = 1.0 / cfg.loop_tau_s;
    let kp = 2.0 * cfg.damping * wn;
    let ki = wn * wn;
    let (mut steer_ns_per_s, mut integ, mut corr_ns) = (0.0f64, 0.0f64, 0.0f64);
    let mut te = Vec::with_capacity(n);
    let mut src = Vec::with_capacity(n);
    for (k, &xo) in osc.iter().enumerate() {
        let t = k as f64 * cfg.tau0_s;
        let out_ns = xo + corr_ns; // disciplined output phase − true time
        te.push(out_ns);
        let use_gnss = cfg.switch_at_s.is_none_or(|s| t < s) && cfg.gnss.up(t);
        let use_el = !use_gnss && cfg.switch_at_s.is_some_and(|s| t >= s) && cfg.eloran.up(t);
        let meas = if use_gnss {
            src.push("GNSS".to_string());
            Some(out_ns - (cfg.gnss.bias_ns + cfg.gnss.noise_ns * gauss(&mut r)))
        } else if use_el {
            src.push("eLoran".to_string());
            Some(out_ns - (cfg.eloran.bias_ns + cfg.eloran.noise_ns * gauss(&mut r)))
        } else {
            src.push("holdover".to_string());
            None
        };
        if let Some(e) = meas {
            // Phase error e = output − reference; drive it to zero.
            integ += ki * e * cfg.tau0_s;
            steer_ns_per_s = -(kp * e + integ);
        }
        corr_ns += steer_ns_per_s * cfg.tau0_s;
    }
    // The initial frequency offset decays as exp(−ζω_n t): 10 τ_loop leaves < 0.1 %.
    let settle = (10.0 * cfg.loop_tau_s / cfg.tau0_s) as usize;
    let tail: Vec<f64> = te
        .iter()
        .skip(settle.min(n.saturating_sub(1)))
        .cloned()
        .collect();
    let rms = (tail.iter().map(|x| x * x).sum::<f64>() / tail.len().max(1) as f64).sqrt();
    let (mut peak, mut settle_s, mut offset) = (None, None, None);
    if let Some(s) = cfg.switch_at_s {
        let ks = (s / cfg.tau0_s) as usize;
        if ks < n && ks > settle {
            let pre = &te[settle..ks];
            let pre_rms = (pre.iter().map(|x| x * x).sum::<f64>() / pre.len() as f64).sqrt();
            let p = te[ks..].iter().map(|x| x.abs()).fold(0.0, f64::max);
            peak = Some(p);
            // Settled: a 100-epoch moving RMS stays within max(2×pre RMS, eLoran noise floor).
            let w = 100usize;
            let lim = (2.0 * pre_rms)
                .max(2.0 * cfg.eloran.noise_ns * (cfg.tau0_s / cfg.loop_tau_s).sqrt());
            // Settling is judged around the level the output converges to (the new reference,
            // bias included); the bias itself is reported as `switchover_offset_ns`.
            let tail_n = ((n - ks) / 10).max(1);
            let final_mean = te[n - tail_n..].iter().sum::<f64>() / tail_n as f64;
            offset = Some(final_mean);
            let mut last_bad = ks;
            for j in ks..n.saturating_sub(w) {
                let m = (te[j..j + w]
                    .iter()
                    .map(|x| (x - final_mean).powi(2))
                    .sum::<f64>()
                    / w as f64)
                    .sqrt();
                if m > lim {
                    last_bad = j;
                }
            }
            settle_s = Some((last_bad - ks) as f64 * cfg.tau0_s);
        }
    }
    let max_abs = tail.iter().map(|x| x.abs()).fold(0.0, f64::max);
    Ok(EdoReport {
        mtie: mtie_curve(&tail, cfg.tau0_s),
        tdev: tdev_curve(&tail, cfg.tau0_s),
        within_limit: max_abs <= cfg.time_error_limit_ns,
        max_abs_te_ns: max_abs,
        rms_te_after_settle_ns: rms,
        switchover_peak_ns: peak,
        switchover_settle_s: settle_s,
        switchover_offset_ns: offset,
        time_error_ns: te,
        source: src,
        evidence: "MODELLED: power-law oscillator noise, white reference noise, type-2 PI loop; checked against closed forms".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ageing_only_holdover_follows_half_a_t_squared() {
        let o = Oscillator {
            y0: 0.0,
            ageing_per_day: 1e-10,
            adev1_wpm: 0.0,
            adev1_wfm: 0.0,
            adev1_rwfm: 0.0,
        };
        let x = free_run(&o, 1.0, 86_401, 1);
        let a = 1e-10 / 86_400.0;
        let want = 0.5 * a * 86_400f64.powi(2) * 1e9; // ns after one day
        assert!(
            (x[86_400] - want).abs() / want < 1e-3,
            "{} vs {want}",
            x[86_400]
        );
    }

    #[test]
    fn white_fm_adev_is_recovered() {
        let o = Oscillator {
            y0: 0.0,
            ageing_per_day: 0.0,
            adev1_wpm: 0.0,
            adev1_wfm: 1e-11,
            adev1_rwfm: 0.0,
        };
        let x = free_run(&o, 1.0, 20_000, 3);
        // Overlapping ADEV at τ = 1 s from phase.
        let n = x.len();
        let mut s = 0.0;
        for i in 0..n - 2 {
            let d = (x[i + 2] - 2.0 * x[i + 1] + x[i]) * 1e-9;
            s += d * d;
        }
        let adev = (s / (2.0 * (n - 2) as f64)).sqrt();
        assert!((adev / 1e-11 - 1.0).abs() < 0.05, "ADEV(1 s) {adev}");
    }

    #[test]
    fn discipline_holds_time_error_near_reference_noise() {
        let c = EdoConfig {
            duration_s: 30_000.0,
            ..Default::default()
        };
        let r = run(&c).unwrap();
        // Loop averages the 10 ns GNSS noise over ~τ_loop: TE RMS well below the raw noise.
        assert!(
            r.rms_te_after_settle_ns < 5.0,
            "{}",
            r.rms_te_after_settle_ns
        );
        assert!(r.within_limit);
        assert!(r.source.iter().all(|s| s == "GNSS"));
    }

    #[test]
    fn uncalibrated_eloran_offset_appears_as_the_switchover_step() {
        let mut c = EdoConfig {
            duration_s: 40_000.0,
            switch_at_s: Some(15_000.0),
            ..Default::default()
        };
        c.eloran.bias_ns = 80.0;
        let r = run(&c).unwrap();
        let peak = r.switchover_peak_ns.unwrap();
        // The output walks to the eLoran bias: final TE ≈ bias; overshoot of a ζ = 0.707 loop is
        // about 20 %, so the peak lies between bias and 1.35 × bias (+ noise).
        assert!(peak > 70.0 && peak < 80.0 * 1.35 + 15.0, "peak {peak}");
        let late: f64 = r.time_error_ns[38_000..].iter().sum::<f64>() / 2000.0;
        assert!((late - 80.0).abs() < 8.0, "late mean {late}");
        assert!(r.source[16_000] == "eLoran");
        assert!((r.switchover_offset_ns.unwrap() - 80.0).abs() < 8.0);
        // A ζ = 0.707 loop with τ = 1000 s settles in a few loop time constants.
        let st = r.switchover_settle_s.unwrap();
        assert!(st < 8_000.0, "settle {st} s");
        // Calibrated (bias 0): the switchover is invisible apart from the noise change.
        c.eloran.bias_ns = 0.0;
        let r = run(&c).unwrap();
        assert!(
            r.switchover_peak_ns.unwrap() < 20.0,
            "{:?}",
            r.switchover_peak_ns
        );
    }

    #[test]
    fn holdover_when_both_references_are_lost() {
        let mut c = EdoConfig {
            duration_s: 30_000.0,
            switch_at_s: Some(10_000.0),
            ..Default::default()
        };
        c.eloran.available = vec![(20_000.0, 30_000.0)];
        let r = run(&c).unwrap();
        assert_eq!(r.source[15_000], "holdover");
        assert_eq!(r.source[25_000], "eLoran");
        // Over 10 000 s of holdover the error grows by the frozen frequency error and ageing.
        let drift = (r.time_error_ns[19_999] - r.time_error_ns[10_000]).abs();
        assert!(drift > 1.0, "holdover drift {drift}");
    }
}
