// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L1 — Quantum-payload digital-twin **framework** (demonstrator)
//!
//! The Pro IP that L1 is described as: *the bridge between Kshana's
//! published-coefficient models and a real device's measured performance.* Given a
//! device's published noise characterisation (an Allan-deviation budget), this
//! **calibrates** an open-core [`kshana::models::ClockModel`] to it and **validates**
//! that the calibrated twin reproduces the published curve — computed with the open
//! core's own [`kshana::allan`] estimator.
//!
//! ## What is built vs still gated
//!
//! - **Built (demonstrator):** the calibration + validation *framework*, exercised
//!   against **published** device data (no NDA needed). This is the reusable moat.
//! - **Still gated:** a twin of a *specific customer's* device needs that customer's
//!   confidential (and likely export-controlled) characterisation data — see
//!   [`customer_instance_gated`]. The framework is the same; only the input data is
//!   gated.
//!
//! ## Method (honest)
//!
//! For the short-to-medium averaging times where most clock data sits, the
//! Allan-deviation variance decomposes as `σ²_y(τ) = a/τ + b`:
//! **white-FM** (`a`, slope τ^-1/2) and a **flicker-FM floor** (`b`, flat). The
//! 2-parameter [`calibrate`] recovers `(a, b)` by least squares; the calibration then
//! uses the open core's own *validated* mappings — `q_wf = a` (i.e. `σ_y(1 s)²`, as
//! documented in the open clock scenarios) and `flicker_floor = √b` (the
//! `with_flicker` ADEV floor).
//!
//! Real Rb / maser / optical references also show a **long-τ random-walk-FM upturn**
//! (ADEV slope +1/2) that a 2-term fit cannot represent. [`calibrate3`] adds that
//! third basis function: `σ²_y(τ) = a/τ + b + c·τ`, mapping the long-τ term to the
//! open clock's random-walk-FM PSD `q_rw = 3·c` (the NIST SP 1065 relation
//! `σ²_y(τ) = q_rw·τ/3`, the same one [`kshana::clock_state::q_from_allan`] uses).
//! Deterministic aging stays the separate `drift_per_s` term (a `½·D·t²` time-error
//! the estimator removes while synced — see [`holdover_uq`]). The 2-parameter path is
//! preserved unchanged (it is the `q_rw = 0` special case).

/// NAV-DT performance/resilience twin pipeline (A10) — the canonical bundle output.
pub mod navdt;

/// Holdover uncertainty-quantification pack (L1 item 3) — wraps the open
/// [`kshana::holdover`] coast physics into a banded prediction + in-spec coast time.
pub mod holdover_uq;
/// Optical-lattice systematic-shift budget table (L1 item 4) — a representative
/// published-budget physics model, summed in quadrature.
pub mod optical_systematics;
/// Named device presets (L1 item 2) — CSAC/OCXO/Rb/PHM/RAFS/optical [`DeviceSpec`]s.
pub mod presets;

use kshana::api::KshanaError;
use kshana::models::{ClockModel, ErrorModel};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// A published device characterisation: its name/provenance and an Allan-deviation
/// budget as `(tau_s, adev)` points. This is the framework's input — public for the
/// demonstrator, NDA-supplied for a real customer instance.
#[derive(Debug, Clone)]
pub struct DeviceSpec {
    pub device: String,
    pub provenance: String,
    /// Published `(tau_s, sigma_y(tau))` points (need >= 2).
    pub adev_points: Vec<(f64, f64)>,
    /// Optional deterministic fractional-frequency aging (per second).
    pub drift_per_s: f64,
}

/// The calibrated twin: the open-core clock parameters fitted to the device.
#[derive(Debug, Clone)]
pub struct CalibratedTwin {
    pub id: String,
    pub provenance: String,
    /// White-FM intensity, `q_wf = sigma_y(1 s)^2`.
    pub q_wf: f64,
    /// Flicker-FM ADEV floor.
    pub flicker_floor: f64,
    /// Random-walk-FM PSD, `q_rw = 3·c` from the long-τ `c·τ` ADEV-variance term
    /// (`σ²_y(τ) = q_rw·τ/3`). Zero for the 2-parameter [`calibrate`] path.
    pub q_rw: f64,
    /// Deterministic aging (per second).
    pub drift_per_s: f64,
}

impl CalibratedTwin {
    /// Materialise the twin as an open-core [`ClockModel`] — proving the calibrated
    /// twin *is* a Kshana `ErrorModel` and runs in the open engine unchanged. The
    /// random-walk-FM PSD `q_rw` (long-τ upturn) flows straight into the open clock's
    /// own `q_rw` slot; `q_rw = 0` reproduces the 2-parameter twin exactly.
    pub fn clock_model(&self) -> ClockModel {
        let mut m = ClockModel::new(&self.id, &self.provenance, 0.0, self.q_wf, self.q_rw)
            .with_flicker(self.flicker_floor);
        if self.drift_per_s != 0.0 {
            m = m.with_drift(self.drift_per_s);
        }
        m
    }

    /// The twin's **analytic** Allan deviation at averaging time `tau_s`, summing the
    /// fitted power-law terms `σ²_y(τ) = q_wf/τ + flicker_floor² + (q_rw/3)·τ` (the
    /// same basis [`calibrate3`] fits). This is the closed-form fit curve — distinct
    /// from [`validate`], which synthesises the open `ClockModel` and re-estimates the
    /// ADEV with [`kshana::allan`]. Reproduction of a published curve this way is a
    /// **calibration fit (Modelled)**, not an externally-validated quantity.
    pub fn model_adev(&self, tau_s: f64) -> f64 {
        let avar =
            self.q_wf / tau_s + self.flicker_floor * self.flicker_floor + (self.q_rw / 3.0) * tau_s;
        avar.max(0.0).sqrt()
    }
}

/// One row of the validation report.
#[derive(Debug, Clone)]
pub struct ValidationPoint {
    pub tau_s: f64,
    pub published_adev: f64,
    pub twin_adev: f64,
    pub rel_err: f64,
}

/// The validation verdict: how closely the calibrated twin reproduces the published
/// curve, measured via the open-core Allan estimator.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub points: Vec<ValidationPoint>,
    pub max_rel_err: f64,
    pub tolerance: f64,
    pub passed: bool,
}

/// **Calibrate**: least-squares fit of `σ²_y(τ) = a/τ + b` (white FM + flicker
/// floor) to the published points, mapped to open-core clock parameters.
pub fn calibrate(spec: &DeviceSpec) -> Result<CalibratedTwin, KshanaError> {
    if spec.adev_points.len() < 2 {
        return Err(KshanaError::InvalidInput(
            "twin calibration needs >= 2 published ADEV points".into(),
        ));
    }
    if spec
        .adev_points
        .iter()
        .any(|&(t, s)| t <= 0.0 || s <= 0.0 || !t.is_finite() || !s.is_finite())
    {
        return Err(KshanaError::InvalidInput(
            "ADEV points must be finite and positive (tau_s > 0, adev > 0)".into(),
        ));
    }

    // Linear least squares for y = a*x1 + b*x2 with x1 = 1/tau, x2 = 1, y = sigma^2.
    let (mut s11, mut s12, mut s22, mut sy1, mut sy2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(tau, adev) in &spec.adev_points {
        let x1 = 1.0 / tau;
        let x2 = 1.0;
        let y = adev * adev;
        s11 += x1 * x1;
        s12 += x1 * x2;
        s22 += x2 * x2;
        sy1 += x1 * y;
        sy2 += x2 * y;
    }
    let det = s11 * s22 - s12 * s12;
    let (a, b) = if det.abs() < f64::EPSILON {
        // Degenerate (e.g. all tau equal) — fall back to pure white FM at the
        // first point: a = sigma^2 * tau, b = 0.
        let (t0, s0) = spec.adev_points[0];
        (s0 * s0 * t0, 0.0)
    } else {
        let a = (sy1 * s22 - sy2 * s12) / det;
        let b = (s11 * sy2 - s12 * sy1) / det;
        (a, b)
    };
    // Physical coefficients are non-negative; clamp fit noise.
    let a = a.max(0.0);
    let b = b.max(0.0);

    Ok(CalibratedTwin {
        id: format!("twin:{}", spec.device),
        provenance: format!(
            "Kshana Pro L1 twin calibrated to: {} (white-FM q_wf=sigma_y(1s)^2 + flicker floor)",
            spec.provenance
        ),
        q_wf: a,
        flicker_floor: b.sqrt(),
        q_rw: 0.0,
        drift_per_s: spec.drift_per_s,
    })
}

/// **Calibrate (3-parameter)**: least-squares fit of `σ²_y(τ) = a/τ + b + c·τ`
/// (white FM + flicker floor + **random-walk-FM** long-τ upturn) to the published
/// points, mapped to open-core clock parameters. The extra `c·τ` term is the one a
/// real Rb / maser / optical reference shows past its flicker floor that the
/// 2-parameter [`calibrate`] cannot represent; it maps to `q_rw = 3·c` via the NIST
/// SP 1065 relation `σ²_y(τ) = q_rw·τ/3` (identical to
/// [`kshana::clock_state::q_from_allan`]'s `q_rw = 3·b_rwfm²`).
///
/// Needs **≥ 3** points (three basis functions). A degenerate design (e.g. all `τ`
/// equal) falls back to the 2-parameter [`calibrate`]. The fit is done in
/// dimensionless, well-conditioned coordinates (`τ` scaled by its geometric mean,
/// `σ²` by its max) so the basis spans — `1/τ` to `τ` over decades — do not lose
/// precision in the normal equations.
pub fn calibrate3(spec: &DeviceSpec) -> Result<CalibratedTwin, KshanaError> {
    if spec.adev_points.len() < 3 {
        return Err(KshanaError::InvalidInput(
            "3-parameter twin calibration needs >= 3 published ADEV points".into(),
        ));
    }
    if spec
        .adev_points
        .iter()
        .any(|&(t, s)| t <= 0.0 || s <= 0.0 || !t.is_finite() || !s.is_finite())
    {
        return Err(KshanaError::InvalidInput(
            "ADEV points must be finite and positive (tau_s > 0, adev > 0)".into(),
        ));
    }

    // Dimensionless scaling: tau_ref = geometric mean of tau, y_ref = max sigma^2.
    let n = spec.adev_points.len() as f64;
    let ln_tau_sum: f64 = spec.adev_points.iter().map(|&(t, _)| t.ln()).sum();
    let tau_ref = (ln_tau_sum / n).exp();
    let y_ref = spec
        .adev_points
        .iter()
        .map(|&(_, s)| s * s)
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);

    // Normal equations for w = A/u + B + C*u with u = tau/tau_ref, w = sigma^2/y_ref.
    // Basis g(u) = [1/u, 1, u]; weighted by W = 1/w² so the fit minimises the
    // *relative* variance residual `Σ ((σ²_pred − σ²_obs)/σ²_obs)²`. Ordinary least
    // squares in σ² over many decades is dominated by the largest (short-τ) point and
    // ignores the small long-τ floor/upturn entirely; relative (log-space) weighting
    // is the standard way ADEV curves are fitted (Riley, NIST SP 1065), and is what
    // lets the long-τ random-walk term be recovered at all.
    let mut m = [[0.0_f64; 3]; 3];
    let mut v = [0.0_f64; 3];
    for &(tau, adev) in &spec.adev_points {
        let u = tau / tau_ref;
        let w = adev * adev / y_ref;
        let wt = 1.0 / (w * w); // relative weight (w > 0 guaranteed above)
        let g = [1.0 / u, 1.0, u];
        for i in 0..3 {
            v[i] += wt * g[i] * w;
            for j in 0..3 {
                m[i][j] += wt * g[i] * g[j];
            }
        }
    }

    // Solve the 3×3 system; a singular design (collinear basis, e.g. all tau equal)
    // falls back to the 2-parameter fit.
    let cand3 = match solve3(m, v) {
        Some([cap_a, cap_b, cap_c]) => {
            // Unscale back to physical (a/τ + b + c·τ) coefficients, clamp fit noise.
            let a = (y_ref * cap_a * tau_ref).max(0.0);
            let b = (y_ref * cap_b).max(0.0);
            let c = (y_ref * cap_c / tau_ref).max(0.0);
            Some(CalibratedTwin {
                id: format!("twin:{}", spec.device),
                provenance: format!(
                    "Kshana Pro L1 twin (3-param) calibrated to: {} (white-FM q_wf + flicker floor + random-walk-FM q_rw=3c long-tau upturn)",
                    spec.provenance
                ),
                q_wf: a,
                flicker_floor: b.sqrt(),
                q_rw: 3.0 * c,
                drift_per_s: spec.drift_per_s,
            })
        }
        None => None,
    };

    // The 2-parameter fit is the q_rw = 0 special case; never return a 3-parameter fit
    // that reproduces the published budget *worse* than it. Clamping `c ≥ 0` can leave
    // the 3-term `(a, b)` mismatched on white-dominated data, so select by the analytic
    // residual: the long-τ term is used only when it genuinely helps (model selection).
    let cand2 = calibrate(spec)?;
    let max_rel = |t: &CalibratedTwin| -> f64 {
        spec.adev_points
            .iter()
            .map(|&(tau, published)| ((t.model_adev(tau) - published) / published).abs())
            .fold(0.0_f64, f64::max)
    };
    match cand3 {
        Some(c3) if max_rel(&c3) <= max_rel(&cand2) => Ok(c3),
        _ => Ok(cand2),
    }
}

/// Solve a 3×3 linear system `M·x = v` by Gaussian elimination with partial pivoting.
/// Returns `None` when the design is (near-)singular — a pivot small relative to the
/// matrix scale — so the caller can fall back. Partial pivoting (not Cramer's rule) is
/// used because the relative-weighted normal matrix spans many decades, where the
/// determinant-of-products form loses all precision to cancellation.
fn solve3(mut m: [[f64; 3]; 3], mut v: [f64; 3]) -> Option<[f64; 3]> {
    let scale = m
        .iter()
        .flatten()
        .fold(0.0_f64, |acc, &x| acc.max(x.abs()))
        .max(1e-300);
    for col in 0..3 {
        // Partial pivot: largest |entry| in this column at or below the diagonal.
        let mut piv = col;
        for r in (col + 1)..3 {
            if m[r][col].abs() > m[piv][col].abs() {
                piv = r;
            }
        }
        if m[piv][col].abs() < 1e-12 * scale {
            return None; // singular within scale-relative tolerance
        }
        m.swap(col, piv);
        v.swap(col, piv);
        let pivot_row = m[col]; // [f64; 3] is Copy
        for r in (col + 1)..3 {
            let f = m[r][col] / pivot_row[col];
            for (mrc, &prc) in m[r].iter_mut().zip(pivot_row.iter()).skip(col) {
                *mrc -= f * prc;
            }
            v[r] -= f * v[col];
        }
    }
    // Back-substitution.
    let mut x = [0.0_f64; 3];
    for i in (0..3).rev() {
        let mut s = v[i];
        for j in (i + 1)..3 {
            s -= m[i][j] * x[j];
        }
        x[i] = s / m[i][i];
    }
    Some(x)
}

/// **Fit report** (analytic): how closely the twin's closed-form [`CalibratedTwin::model_adev`]
/// curve reproduces the published points. Unlike [`validate`] (which synthesises the
/// open `ClockModel` and re-estimates the ADEV through [`kshana::allan`]), this is the
/// deterministic least-squares residual — fast, seedless, and the right check for a
/// device preset's whole ADEV budget including its long-τ branch. Still **Modelled**:
/// a calibration fit, not an externally-validated quantity.
pub fn fit_report(
    twin: &CalibratedTwin,
    spec: &DeviceSpec,
    tolerance: f64,
) -> Result<ValidationReport, KshanaError> {
    if spec.adev_points.is_empty() {
        return Err(KshanaError::InvalidInput(
            "fit report needs >= 1 published ADEV point".into(),
        ));
    }
    let mut points = Vec::new();
    let mut max_rel_err: f64 = 0.0;
    for &(tau, published) in &spec.adev_points {
        let twin_adev = twin.model_adev(tau);
        let rel_err = ((twin_adev - published) / published).abs();
        max_rel_err = max_rel_err.max(rel_err);
        points.push(ValidationPoint {
            tau_s: tau,
            published_adev: published,
            twin_adev,
            rel_err,
        });
    }
    Ok(ValidationReport {
        points,
        max_rel_err,
        tolerance,
        passed: max_rel_err <= tolerance,
    })
}

/// **Validate**: synthesise the twin's phase series and compare its Allan deviation
/// (via the open-core estimator) against the published points.
pub fn validate(
    twin: &CalibratedTwin,
    spec: &DeviceSpec,
    tolerance: f64,
    tau0_s: f64,
    n_samples: usize,
    seed: u64,
) -> Result<ValidationReport, KshanaError> {
    if tau0_s <= 0.0 || !tau0_s.is_finite() || n_samples < 16 {
        return Err(KshanaError::InvalidInput(
            "validation needs tau0_s > 0 and n_samples >= 16".into(),
        ));
    }
    // Synthesise the twin's phase (time-error) series through the open ErrorModel.
    let mut model = twin.clock_model();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut phase = Vec::with_capacity(n_samples);
    for _ in 0..n_samples {
        model.step(tau0_s, &mut rng);
        phase.push(model.phase());
    }
    // The open core's own Allan estimator — the twin is validated by the same tool
    // the open engine validates itself with.
    let curve = kshana::allan::overlapping_adev_curve(&phase, tau0_s);
    if curve.is_empty() {
        return Err(KshanaError::NonConvergence(
            "Allan estimator produced no points (series too short for any tau)".into(),
        ));
    }

    let mut points = Vec::new();
    let mut max_rel_err: f64 = 0.0;
    for &(tau, published) in &spec.adev_points {
        // Nearest available averaging time in the estimated curve (log distance).
        let nearest = curve
            .iter()
            .min_by(|p, q| {
                let dp = (p.tau_s.ln() - tau.ln()).abs();
                let dq = (q.tau_s.ln() - tau.ln()).abs();
                dp.partial_cmp(&dq).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        let twin_adev = nearest.adev;
        let rel_err = ((twin_adev - published) / published).abs();
        max_rel_err = max_rel_err.max(rel_err);
        points.push(ValidationPoint {
            tau_s: tau,
            published_adev: published,
            twin_adev,
            rel_err,
        });
    }

    Ok(ValidationReport {
        points,
        max_rel_err,
        tolerance,
        passed: max_rel_err <= tolerance,
    })
}

/// The seam guard for the **gated** part of L1: a twin of a *specific customer's*
/// device. The framework above is built and runs on published data; only the
/// confidential/export-controlled device data is gated.
pub fn customer_instance_gated() -> KshanaError {
    KshanaError::Unsupported(
        "A twin of a specific customer device requires that customer's confidential \
         (likely export-controlled) characterisation data under NDA; that instance is \
         contract/partner-gated. The calibration framework itself is built — feed it \
         published data. See BUILD-TRACKER PR8–PR11."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // A representative CSAC-class published ADEV (white-FM dominated): sigma_y(1s)
    // ~ 3e-10 rolling down ~tau^-1/2 toward a flicker floor. Representative values
    // for the demonstrator — substitute a target device's datasheet in real use.
    fn csac_spec() -> DeviceSpec {
        DeviceSpec {
            device: "CSAC (representative)".into(),
            provenance: "representative CSAC-class published ADEV (white FM + flicker floor)"
                .into(),
            adev_points: vec![(1.0, 3.0e-10), (10.0, 1.0e-10), (100.0, 3.2e-11)],
            drift_per_s: 0.0,
        }
    }

    #[test]
    fn calibrate_recovers_white_fm_from_sigma_at_1s() {
        // For a pure white-FM curve sigma_y(tau) = s1 * tau^-1/2, q_wf must equal s1^2.
        let s1 = 3.0e-10;
        let spec = DeviceSpec {
            device: "pure-white-fm".into(),
            provenance: "synthetic".into(),
            adev_points: vec![(1.0, s1), (4.0, s1 / 2.0), (16.0, s1 / 4.0)],
            drift_per_s: 0.0,
        };
        let twin = calibrate(&spec).unwrap();
        assert!(
            (twin.q_wf - s1 * s1).abs() / (s1 * s1) < 0.05,
            "q_wf {} should be ~sigma_y(1s)^2 {}",
            twin.q_wf,
            s1 * s1
        );
        assert!(
            twin.flicker_floor < s1 * 0.2,
            "pure white FM => negligible flicker floor"
        );
    }

    #[test]
    fn calibrate_rejects_too_few_points() {
        let spec = DeviceSpec {
            device: "x".into(),
            provenance: "y".into(),
            adev_points: vec![(1.0, 1e-10)],
            drift_per_s: 0.0,
        };
        assert!(matches!(
            calibrate(&spec),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn twin_reproduces_published_adev_within_tolerance() {
        let spec = csac_spec();
        let twin = calibrate(&spec).unwrap();
        // Validate against the open-core Allan estimator. 25% tolerance accounts for
        // finite-realisation ADEV scatter at the longer averaging times.
        let report = validate(&twin, &spec, 0.25, 1.0, 200_000, 7).unwrap();
        assert!(
            report.passed,
            "twin should reproduce published ADEV within 25%: max_rel_err={:.3}, points={:?}",
            report.max_rel_err, report.points
        );
        assert_eq!(report.points.len(), 3);
    }

    #[test]
    fn calibrated_twin_is_an_open_error_model() {
        // The twin materialises as a kshana ClockModel (which implements ErrorModel)
        // — proving L1 plugs into the open seam, not a parallel reimplementation.
        let twin = calibrate(&csac_spec()).unwrap();
        let mut model = twin.clock_model();
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        model.step(1.0, &mut rng);
        // spec() is the ErrorModel trait method; kind must be the open clock kind.
        assert_eq!(model.spec().kind, "clock");
    }

    #[test]
    fn customer_instance_is_gated() {
        assert!(matches!(
            customer_instance_gated(),
            KshanaError::Unsupported(_)
        ));
    }

    // ── 3-parameter calibration (white FM + flicker + random-walk-FM) ──────────

    #[test]
    fn calibrate_2param_path_leaves_q_rw_zero() {
        // Back-compat: the original 2-parameter calibrate() must keep producing a
        // q_rw = 0 twin (the q_rw field is the only addition; the path is unchanged).
        let twin = calibrate(&csac_spec()).unwrap();
        assert_eq!(twin.q_rw, 0.0, "2-param calibrate must not introduce q_rw");
        // …and it still materialises as the open clock model unchanged.
        assert_eq!(twin.clock_model().spec().kind, "clock");
    }

    #[test]
    fn calibrate3_recovers_random_walk_upturn() {
        // Build an exact three-term curve σ²_y(τ) = q_wf/τ + ff² + (q_rw/3)·τ and
        // confirm the 3-parameter fit recovers all three coefficients. This is the
        // long-τ term the 2-parameter fit structurally cannot represent.
        let q_wf = 9.0e-24; // σ_y(1 s) white-FM = 3e-12
        let ff = 2.0e-13; // flicker floor
        let q_rw = 3.0e-30; // random-walk-FM PSD (c = q_rw/3 = 1e-30)
        let model = |tau: f64| (q_wf / tau + ff * ff + (q_rw / 3.0) * tau).sqrt();
        let taus = [1.0, 10.0, 100.0, 1_000.0, 10_000.0, 100_000.0];
        let spec = DeviceSpec {
            device: "synthetic-3term".into(),
            provenance: "synthetic white+flicker+RWFM".into(),
            adev_points: taus.iter().map(|&t| (t, model(t))).collect(),
            drift_per_s: 0.0,
        };
        let twin = calibrate3(&spec).unwrap();
        assert!(
            (twin.q_wf - q_wf).abs() / q_wf < 0.05,
            "q_wf {} vs {q_wf}",
            twin.q_wf
        );
        assert!(
            (twin.flicker_floor - ff).abs() / ff < 0.05,
            "flicker_floor {} vs {ff}",
            twin.flicker_floor
        );
        assert!(
            (twin.q_rw - q_rw).abs() / q_rw < 0.05,
            "q_rw {} vs {q_rw} (the long-τ upturn the 2-param fit cannot see)",
            twin.q_rw
        );
        // The 2-parameter fit on the SAME data leaves q_rw at zero by construction —
        // it cannot represent the upturn, which is the whole point of calibrate3.
        assert_eq!(calibrate(&spec).unwrap().q_rw, 0.0);
    }

    #[test]
    fn calibrate3_rejects_too_few_points() {
        let spec = DeviceSpec {
            device: "x".into(),
            provenance: "y".into(),
            adev_points: vec![(1.0, 1e-10), (10.0, 3e-11)],
            drift_per_s: 0.0,
        };
        assert!(matches!(
            calibrate3(&spec),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn calibrate3_falls_back_on_degenerate_design() {
        // All-equal τ makes the {1/τ, 1, τ} basis collinear (singular normal matrix);
        // calibrate3 must fall back to the 2-parameter fit rather than divide by a
        // noise-level determinant — and not panic.
        let spec = DeviceSpec {
            device: "degenerate".into(),
            provenance: "all tau equal".into(),
            adev_points: vec![(5.0, 1e-11), (5.0, 1.1e-11), (5.0, 0.9e-11)],
            drift_per_s: 0.0,
        };
        let twin = calibrate3(&spec).unwrap();
        assert!(twin.q_wf.is_finite() && twin.q_rw.is_finite());
    }

    #[test]
    fn fit_report_passes_on_three_term_curve() {
        // The analytic fit-report (model_adev vs published) must reproduce a genuine
        // three-term curve to well within a tight tolerance — a deterministic,
        // seedless calibration-fit check (Modelled, not externally validated).
        let q_wf = 4.0e-24;
        let ff = 1.0e-13;
        let q_rw = 6.0e-30;
        let model = |tau: f64| (q_wf / tau + ff * ff + (q_rw / 3.0) * tau).sqrt();
        let taus = [1.0, 10.0, 100.0, 1_000.0, 10_000.0];
        let spec = DeviceSpec {
            device: "synthetic".into(),
            provenance: "synthetic".into(),
            adev_points: taus.iter().map(|&t| (t, model(t))).collect(),
            drift_per_s: 0.0,
        };
        let twin = calibrate3(&spec).unwrap();
        let report = fit_report(&twin, &spec, 0.05).unwrap();
        assert!(
            report.passed,
            "analytic fit should reproduce its own basis: max_rel_err={:.4}",
            report.max_rel_err
        );
    }
}
