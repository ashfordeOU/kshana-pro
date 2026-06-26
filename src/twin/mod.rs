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
//! **white-FM** (`a`, slope τ^-1/2) and a **flicker-FM floor** (`b`, flat). The fit
//! recovers `(a, b)` by least squares; the calibration then uses the open core's own
//! *validated* mappings — `q_wf = a` (i.e. `σ_y(1 s)²`, as documented in the open
//! clock scenarios) and `flicker_floor = √b` (the `with_flicker` ADEV floor).
//! Random-walk FM / aging (long-τ) is a straightforward extension, intentionally out
//! of this demonstrator's scope.

/// NAV-DT performance/resilience twin pipeline (A10) — the canonical bundle output.
pub mod navdt;

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
    /// Deterministic aging (per second).
    pub drift_per_s: f64,
}

impl CalibratedTwin {
    /// Materialise the twin as an open-core [`ClockModel`] — proving the calibrated
    /// twin *is* a Kshana `ErrorModel` and runs in the open engine unchanged.
    pub fn clock_model(&self) -> ClockModel {
        let mut m = ClockModel::new(&self.id, &self.provenance, 0.0, self.q_wf, 0.0)
            .with_flicker(self.flicker_floor);
        if self.drift_per_s != 0.0 {
            m = m.with_drift(self.drift_per_s);
        }
        m
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
        drift_per_s: spec.drift_per_s,
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
}
