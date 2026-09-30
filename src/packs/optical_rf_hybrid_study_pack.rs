// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L51 — `pro-optical-rf-hybrid-study`
//!
//! A Kshana Pro *study* pack over the open engine's P5 heterogeneous (optical + RF)
//! PNT core. It assembles the P5 commercial study surface end-to-end by driving five
//! open modules through their public contracts:
//!
//! * [`kshana::optical_linkbudget`] (L26) — the 1550 nm optical link budget: the
//!   diffraction footprint `(λ/D)·L`, the detected-photon rate, and the photon-limited
//!   ranging/timing CRLB `σ_τ/√N`.
//! * [`kshana::linkbudget`] — the RF column: the CCSDS/DSN link equation `C/N₀`, from
//!   which a code-tracking (DLL) thermal ranging jitter is formed.
//! * [`kshana::cross_raim`] (L22) — cross-modality RAIM: the χ² solution-separation
//!   detector and the position + timing protection levels from disparate RF/optical
//!   covariances.
//! * [`kshana::handoff`] (L23) — the optical↔RF state/covariance handoff with the
//!   bit-for-bit no-jump mean invariant and the NEES consistency gate.
//! * [`kshana::optical_availability`] (L24) — the per-site clear-sky climatology, whose
//!   clear-sky fractions are externally anchored to Cavazzani et al. 2011 (GOES12
//!   clear-night fractions), and the `N`-station diversity combination (independent union
//!   + spatially-correlated union).
//!
//! On top of these the pack adds the deliverable surface: two byte-stable committed
//! CSVs — **P5 Table 1 (precision)**: the RF vs optical vs hybrid ranging/timing 1σ, and
//! **P5 Table 2 (availability)**: single-site vs `N`-station diversity — a two-panel
//! branded Figure 1 (ranging precision on a log scale, and availability vs number of
//! stations), a branded HTML report and a branded PDF.
//!
//! ## Validated vs Modelled (honesty firewall)
//! * **Validated (closed form, checked to machine precision against an independent
//!   oracle).** The photon-limited ranging/timing CRLB equals `σ_τ/√N`; the diffraction
//!   footprint equals `(λ/D)·L`; the inverse-variance fused precision equals
//!   `1/√(1/σ_rf² + 1/σ_opt²)`; and the `N`-station independent-union availability equals
//!   `1 − Π(1−a_i)`.
//! * **Modelled.** The RF and optical link-budget *magnitudes* (the `C/N₀`, the
//!   detected-photon count `N`), the RF DLL ranging model, the per-site clear-sky
//!   availabilities and the correlation `ρ`, and the cross-RAIM / handoff covariance
//!   *magnitudes* are representative allocations; the precision, protection-level and
//!   availability *values* inherit them. An engineering study, not a certification.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::cross_raim::{run_cross_raim, AxisRole, CrossAxis, CrossRaimResult};
use kshana::handoff::{optical_rf_handoff, HandoffOutcome, HandoffState};
use kshana::linkbudget::{link_budget, LinkParams, LinkResult};
use kshana::optical_availability::{
    default_network, run_optical_availability, OpticalAvailabilityResult,
};
use kshana::optical_linkbudget::{
    detected_photons, optical_link_budget, photon_limited_range_crlb_m, photon_limited_toa_crlb_s,
    OpticalLinkParams, OpticalLinkResult,
};
use kshana::radiometric::Band;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::packs::study_pdf::{Cell, StudyPdf};
use crate::report_brand::{
    cover_html, modelled_banner_html, BRASS, BRASS_MUTED, FONT_BODY, FONT_HEAD, IVORY, IVORY_LIGHT,
    NAVY,
};

/// Speed of light (m/s) — for the RF range↔time conversion and the CRLB range oracle.
const C_M_S: f64 = 299_792_458.0;

/// Teal — the Validated-provenance accent (matches the sibling study packs).
const TEAL: &str = "#2dd4bf";

/// The honesty label carried on the result document (contains both tier words).
const LABEL: &str = "MODELLED P5 heterogeneous optical + RF PNT study. \
VALIDATED: the photon-limited ranging/timing CRLB equals the closed form sigma_tau/sqrt(N), the \
optical diffraction footprint equals (lambda/D)*L, the inverse-variance fused precision equals \
1/sqrt(1/sigma_rf^2 + 1/sigma_opt^2), and the N-station independent-union availability equals \
1 - prod(1 - a_i) — each checked against an independent closed form to machine precision. \
MODELLED: the RF and optical link-budget magnitudes (C/N0, detected-photon count N), the RF DLL \
ranging model, the per-site clear-sky availabilities and the spatial correlation, and the \
cross-RAIM / handoff covariance magnitudes are representative allocations; the precision, \
protection-level and availability values inherit them. An engineering study, not a certification.";

// ── Provenance tag ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Validated,
    Modelled,
}

impl Tag {
    fn as_str(self) -> &'static str {
        match self {
            Tag::Validated => "Validated",
            Tag::Modelled => "Modelled",
        }
    }
    fn color(self) -> &'static str {
        match self {
            Tag::Validated => TEAL,
            Tag::Modelled => BRASS,
        }
    }
}

/// Wrap a JSON value with its provenance tag, matching the sibling-study convention.
fn tagged(v: Value, t: Tag) -> Value {
    json!({ "value": v, "provenance": t.as_str() })
}

// ── Defaults (cited, reproduce the P5 headline) ──────────────────────────────

fn d_wavelength_m() -> f64 {
    1.55e-6
}
fn d_tx_power_w() -> f64 {
    1.0e-3
}
fn d_tx_aperture_m() -> f64 {
    0.85
}
fn d_rx_aperture_m() -> f64 {
    0.85
}
fn d_range_m() -> f64 {
    3.84e8
}
fn d_optics_efficiency() -> f64 {
    0.5
}
fn d_detector_efficiency() -> f64 {
    0.7
}
fn d_atmospheric_loss_db() -> f64 {
    3.0
}
fn d_pointing_loss_db() -> f64 {
    3.0
}
fn d_pulse_rms_width_s() -> f64 {
    50.0e-12
}
fn d_integration_s() -> f64 {
    1.0
}
fn d_two_way() -> bool {
    true
}
fn d_rf_band() -> String {
    "x".to_string()
}
fn d_rf_eirp_dbw() -> f64 {
    30.0
}
fn d_rf_g_over_t_db() -> f64 {
    12.0
}
fn d_rf_range_km() -> f64 {
    384_000.0
}
fn d_rf_data_rate_bps() -> f64 {
    1.0e3
}
fn d_rf_other_losses_db() -> f64 {
    3.0
}
fn d_rf_required_eb_n0_db() -> f64 {
    4.5
}
fn d_rf_chip_rate_hz() -> f64 {
    1.023e6
}
fn d_rf_dll_bw_hz() -> f64 {
    1.0
}
fn d_rf_correlator_spacing_chips() -> f64 {
    0.5
}
fn d_p_fa() -> f64 {
    1.0e-5
}
fn d_p_md() -> f64 {
    1.0e-3
}
fn d_handoff_inflation() -> f64 {
    0.2
}
fn d_correlation() -> f64 {
    0.15
}

/// A `pro-optical-rf-hybrid-study` scenario. Every field is optional; empty TOML
/// reproduces the 1550 nm Earth–Moon optical terminal + X-band RF baseline that yields
/// the P5 headline numbers (≈ 0.7 km footprint; single-site ≈ 53 %, three-station
/// ≈ 90 %, four-station ≈ 95 %, five-station correlated ≈ 96 % availability).
#[derive(Debug, Clone, Deserialize)]
pub struct OpticalRfHybridStudyScenario {
    /// Optical carrier wavelength (m).
    #[serde(default = "d_wavelength_m")]
    pub wavelength_m: f64,
    /// Optical transmit power (W).
    #[serde(default = "d_tx_power_w")]
    pub tx_power_w: f64,
    /// Optical transmit aperture diameter (m) — sets the `(λ/D)·L` footprint.
    #[serde(default = "d_tx_aperture_m")]
    pub tx_aperture_m: f64,
    /// Optical receive aperture diameter (m) — sets the geometric capture fraction.
    #[serde(default = "d_rx_aperture_m")]
    pub rx_aperture_m: f64,
    /// Optical one-way link range (m).
    #[serde(default = "d_range_m")]
    pub range_m: f64,
    /// Combined transmit + receive optics throughput (0..1): a Modelled allocation.
    #[serde(default = "d_optics_efficiency")]
    pub optics_efficiency: f64,
    /// Detector quantum efficiency (0..1): a Modelled allocation.
    #[serde(default = "d_detector_efficiency")]
    pub detector_efficiency: f64,
    /// Lumped one-way atmospheric transmission loss (dB ≥ 0): a Modelled allocation.
    #[serde(default = "d_atmospheric_loss_db")]
    pub atmospheric_loss_db: f64,
    /// Lumped pointing / jitter loss (dB ≥ 0): a Modelled allocation.
    #[serde(default = "d_pointing_loss_db")]
    pub pointing_loss_db: f64,
    /// Optical pulse RMS temporal width σ_τ (s) — the per-photon timing information.
    #[serde(default = "d_pulse_rms_width_s")]
    pub pulse_rms_width_s: f64,
    /// Optical integration time (s) over which photons are accumulated into one ToA.
    #[serde(default = "d_integration_s")]
    pub integration_s: f64,
    /// Two-way (round-trip) ranging when `true`; one-way when `false`.
    #[serde(default = "d_two_way")]
    pub two_way: bool,
    /// RF carrier band: `s` | `x` | `ka`.
    #[serde(default = "d_rf_band")]
    pub rf_band: String,
    /// RF transmit EIRP (dBW).
    #[serde(default = "d_rf_eirp_dbw")]
    pub rf_eirp_dbw: f64,
    /// RF receive figure of merit G/T (dB/K).
    #[serde(default = "d_rf_g_over_t_db")]
    pub rf_g_over_t_db: f64,
    /// RF one-way link range (km).
    #[serde(default = "d_rf_range_km")]
    pub rf_range_km: f64,
    /// RF information bit rate (bit/s) — for the Eb/N₀ / closure context.
    #[serde(default = "d_rf_data_rate_bps")]
    pub rf_data_rate_bps: f64,
    /// RF lumped non-free-space losses (dB ≥ 0).
    #[serde(default = "d_rf_other_losses_db")]
    pub rf_other_losses_db: f64,
    /// RF required Eb/N₀ for closure (dB).
    #[serde(default = "d_rf_required_eb_n0_db")]
    pub rf_required_eb_n0_db: f64,
    /// RF ranging-code chip rate (chip/s) — sets the chip length `c / R_c`.
    #[serde(default = "d_rf_chip_rate_hz")]
    pub rf_chip_rate_hz: f64,
    /// RF DLL (code-tracking loop) noise bandwidth (Hz).
    #[serde(default = "d_rf_dll_bw_hz")]
    pub rf_dll_bw_hz: f64,
    /// RF early–late correlator spacing (chips).
    #[serde(default = "d_rf_correlator_spacing_chips")]
    pub rf_correlator_spacing_chips: f64,
    /// Cross-RAIM false-alarm probability P_fa.
    #[serde(default = "d_p_fa")]
    pub p_fa: f64,
    /// Cross-RAIM missed-detection probability P_md.
    #[serde(default = "d_p_md")]
    pub p_md: f64,
    /// Optical→RF handoff covariance-inflation factor.
    #[serde(default = "d_handoff_inflation")]
    pub handoff_inflation: f64,
    /// Spatial correlation ρ ∈ [0, 1] for the correlated availability union.
    #[serde(default = "d_correlation")]
    pub correlation: f64,
}

impl Default for OpticalRfHybridStudyScenario {
    fn default() -> Self {
        Self {
            wavelength_m: d_wavelength_m(),
            tx_power_w: d_tx_power_w(),
            tx_aperture_m: d_tx_aperture_m(),
            rx_aperture_m: d_rx_aperture_m(),
            range_m: d_range_m(),
            optics_efficiency: d_optics_efficiency(),
            detector_efficiency: d_detector_efficiency(),
            atmospheric_loss_db: d_atmospheric_loss_db(),
            pointing_loss_db: d_pointing_loss_db(),
            pulse_rms_width_s: d_pulse_rms_width_s(),
            integration_s: d_integration_s(),
            two_way: d_two_way(),
            rf_band: d_rf_band(),
            rf_eirp_dbw: d_rf_eirp_dbw(),
            rf_g_over_t_db: d_rf_g_over_t_db(),
            rf_range_km: d_rf_range_km(),
            rf_data_rate_bps: d_rf_data_rate_bps(),
            rf_other_losses_db: d_rf_other_losses_db(),
            rf_required_eb_n0_db: d_rf_required_eb_n0_db(),
            rf_chip_rate_hz: d_rf_chip_rate_hz(),
            rf_dll_bw_hz: d_rf_dll_bw_hz(),
            rf_correlator_spacing_chips: d_rf_correlator_spacing_chips(),
            p_fa: d_p_fa(),
            p_md: d_p_md(),
            handoff_inflation: d_handoff_inflation(),
            correlation: d_correlation(),
        }
    }
}

/// One modality's precision column (a Table 1 row).
struct ModalityRow {
    name: &'static str,
    range_sigma_m: f64,
    time_sigma_s: f64,
    tag: Tag,
}

/// Everything the study derives, computed once and shared by JSON / CSV / figure /
/// report / PDF.
struct StudyModel {
    optical: OpticalLinkResult,
    n_photons: f64,
    rf: LinkResult,
    cn0_linear: f64,
    /// Precision rows in fixed order: [RF, Optical, Hybrid].
    rows: Vec<ModalityRow>,
    cross: CrossRaimResult,
    handoff: HandoffOutcome,
    avail: OpticalAvailabilityResult,
    // Validated closed-form oracle residuals (≈ 0).
    footprint_residual: f64,
    toa_crlb_residual: f64,
    range_crlb_residual: f64,
    fusion_residual: f64,
    availability_residual: f64,
    scenario_hash: String,
}

/// Parse an RF band string to a [`Band`].
fn parse_band(s: &str) -> Result<Band, KshanaError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "s" => Ok(Band::S),
        "x" => Ok(Band::X),
        "ka" => Ok(Band::Ka),
        other => Err(KshanaError::InvalidInput(format!(
            "unknown rf_band '{other}' (expected s|x|ka)"
        ))),
    }
}

/// Coherent early–late DLL thermal code-tracking ranging jitter (m):
/// `σ_R = (c/R_c)·√(B_dll·d / (2·C/N₀))` (Kaplan & Hegarty, *Understanding GPS*). The
/// `C/N₀` and loop parameters are Modelled, so the returned magnitude is Modelled.
fn rf_code_tracking_sigma_m(cn0_dbhz: f64, chip_rate_hz: f64, dll_bw_hz: f64, spacing: f64) -> f64 {
    let cn0 = 10f64.powf(cn0_dbhz / 10.0);
    let chip_len_m = C_M_S / chip_rate_hz;
    let sigma_chips = (dll_bw_hz * spacing / (2.0 * cn0)).sqrt();
    chip_len_m * sigma_chips
}

impl OpticalRfHybridStudyScenario {
    pub const KIND: &'static str = "pro-optical-rf-hybrid-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P5/L51 study: the heterogeneous optical + RF PNT surface. Drives the open optical (1550 nm) link budget (kshana::optical_linkbudget), the RF link equation (kshana::linkbudget), cross-modality RAIM (kshana::cross_raim), the optical<->RF no-jump handoff (kshana::handoff) and the N-station optical availability (kshana::optical_availability). Emits P5 Table 1 (RF vs optical vs hybrid ranging/timing 1-sigma) and P5 Table 2 (single-site vs N-station diversity availability) as byte-stable CSV, a two-panel Figure 1 (ranging precision on a log scale; availability vs number of stations), a branded HTML report and PDF. Validated: the photon-limited CRLB = sigma_tau/sqrt(N), the diffraction footprint = (lambda/D)*L, the fused precision = 1/sqrt(1/sigma_rf^2 + 1/sigma_opt^2), and the independent-union availability = 1 - prod(1 - a_i), each a closed form checked to machine precision. Modelled: the link-budget magnitudes, the DLL ranging model, the per-site availabilities and the covariance magnitudes. An engineering study, not a certification.",
            required_fields: &[],
            optional_fields: &[
                "wavelength_m",
                "tx_power_w",
                "tx_aperture_m",
                "rx_aperture_m",
                "range_m",
                "optics_efficiency",
                "detector_efficiency",
                "atmospheric_loss_db",
                "pointing_loss_db",
                "pulse_rms_width_s",
                "integration_s",
                "two_way",
                "rf_band",
                "rf_eirp_dbw",
                "rf_g_over_t_db",
                "rf_range_km",
                "rf_data_rate_bps",
                "rf_other_losses_db",
                "rf_required_eb_n0_db",
                "rf_chip_rate_hz",
                "rf_dll_bw_hz",
                "rf_correlator_spacing_chips",
                "p_fa",
                "p_md",
                "handoff_inflation",
                "correlation",
            ],
        }
    }

    /// A deterministic 12-hex scenario fingerprint over the canonical config.
    fn compute_hash(&self) -> String {
        let canonical = format!(
            "optical-rf-hybrid-study|opt=[{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{}]|rf=[{},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e}]|int=[{:.6e},{:.6e},{:.6e}]|corr={:.6e}",
            self.wavelength_m,
            self.tx_power_w,
            self.tx_aperture_m,
            self.rx_aperture_m,
            self.range_m,
            self.optics_efficiency,
            self.detector_efficiency,
            self.atmospheric_loss_db,
            self.pointing_loss_db,
            self.pulse_rms_width_s,
            self.integration_s,
            self.two_way,
            self.rf_band.trim().to_ascii_lowercase(),
            self.rf_eirp_dbw,
            self.rf_g_over_t_db,
            self.rf_range_km,
            self.rf_data_rate_bps,
            self.rf_other_losses_db,
            self.rf_required_eb_n0_db,
            self.rf_chip_rate_hz,
            self.rf_dll_bw_hz,
            self.rf_correlator_spacing_chips,
            self.p_fa,
            self.p_md,
            self.handoff_inflation,
            self.correlation,
        );
        let mut h = Sha256::new();
        h.update(canonical.as_bytes());
        hex::encode(h.finalize()).chars().take(12).collect()
    }

    fn compute(&self) -> Result<StudyModel, KshanaError> {
        // ── Input validation (fail closed, honest errors) ──
        let positives = [
            ("wavelength_m", self.wavelength_m),
            ("tx_power_w", self.tx_power_w),
            ("tx_aperture_m", self.tx_aperture_m),
            ("rx_aperture_m", self.rx_aperture_m),
            ("range_m", self.range_m),
            ("pulse_rms_width_s", self.pulse_rms_width_s),
            ("integration_s", self.integration_s),
            ("rf_range_km", self.rf_range_km),
            ("rf_data_rate_bps", self.rf_data_rate_bps),
            ("rf_chip_rate_hz", self.rf_chip_rate_hz),
            ("rf_dll_bw_hz", self.rf_dll_bw_hz),
            (
                "rf_correlator_spacing_chips",
                self.rf_correlator_spacing_chips,
            ),
        ];
        for (name, v) in positives {
            if !v.is_finite() || v <= 0.0 {
                return Err(KshanaError::InvalidInput(format!(
                    "{name} must be finite and positive (got {v})"
                )));
            }
        }
        let band = parse_band(&self.rf_band)?;

        // ── Optical column (L26): link budget → detected photons → CRLB ──
        let opt_params = OpticalLinkParams {
            wavelength_m: self.wavelength_m,
            tx_power_w: self.tx_power_w,
            tx_aperture_m: self.tx_aperture_m,
            rx_aperture_m: self.rx_aperture_m,
            range_m: self.range_m,
            optics_efficiency: self.optics_efficiency,
            detector_efficiency: self.detector_efficiency,
            atmospheric_loss_db: self.atmospheric_loss_db,
            pointing_loss_db: self.pointing_loss_db,
        };
        let optical = optical_link_budget(&opt_params);
        let n_photons = detected_photons(optical.photon_rate_hz, self.integration_s);
        if n_photons <= 0.0 || !n_photons.is_finite() {
            return Err(KshanaError::InvalidInput(
                "optical link yields no detected photons (check tx_power_w / efficiencies / losses)"
                    .into(),
            ));
        }
        let opt_time_sigma_s = photon_limited_toa_crlb_s(self.pulse_rms_width_s, n_photons);
        let opt_range_sigma_m =
            photon_limited_range_crlb_m(self.pulse_rms_width_s, n_photons, self.two_way);

        // ── RF column: link equation → C/N₀ → DLL thermal ranging jitter ──
        let rf_params = LinkParams {
            band,
            eirp_dbw: self.rf_eirp_dbw,
            g_over_t_db: self.rf_g_over_t_db,
            range_m: self.rf_range_km * 1000.0,
            data_rate_bps: self.rf_data_rate_bps,
            other_losses_db: self.rf_other_losses_db,
        };
        let rf = link_budget(&rf_params, self.rf_required_eb_n0_db);
        let cn0_linear = 10f64.powf(rf.cn0_dbhz / 10.0);
        let rf_range_sigma_m = rf_code_tracking_sigma_m(
            rf.cn0_dbhz,
            self.rf_chip_rate_hz,
            self.rf_dll_bw_hz,
            self.rf_correlator_spacing_chips,
        );
        let rf_time_sigma_s = rf_range_sigma_m / C_M_S;

        // ── Hybrid column: inverse-variance fusion of the two modalities ──
        let (_, hyb_range_sigma_m) = kshana::cross_raim::inverse_variance_fuse(
            0.0,
            rf_range_sigma_m,
            0.0,
            opt_range_sigma_m,
        );
        let (_, hyb_time_sigma_s) =
            kshana::cross_raim::inverse_variance_fuse(0.0, rf_time_sigma_s, 0.0, opt_time_sigma_s);

        let rows = vec![
            ModalityRow {
                name: "rf",
                range_sigma_m: rf_range_sigma_m,
                time_sigma_s: rf_time_sigma_s,
                tag: Tag::Modelled,
            },
            ModalityRow {
                name: "optical",
                range_sigma_m: opt_range_sigma_m,
                time_sigma_s: opt_time_sigma_s,
                tag: Tag::Modelled,
            },
            ModalityRow {
                name: "hybrid",
                range_sigma_m: hyb_range_sigma_m,
                time_sigma_s: hyb_time_sigma_s,
                tag: Tag::Modelled,
            },
        ];

        // ── Cross-modality RAIM (L22): consistent (fault-free) heterogeneous solution ──
        let axes = vec![
            CrossAxis {
                name: "east".into(),
                role: AxisRole::Horizontal,
                rf_value: 0.0,
                rf_sigma: rf_range_sigma_m,
                opt_value: 0.0,
                opt_sigma: opt_range_sigma_m,
            },
            CrossAxis {
                name: "north".into(),
                role: AxisRole::Horizontal,
                rf_value: 0.0,
                rf_sigma: rf_range_sigma_m,
                opt_value: 0.0,
                opt_sigma: opt_range_sigma_m,
            },
            CrossAxis {
                name: "up".into(),
                role: AxisRole::Vertical,
                rf_value: 0.0,
                rf_sigma: rf_range_sigma_m,
                opt_value: 0.0,
                opt_sigma: opt_range_sigma_m,
            },
            CrossAxis {
                name: "clock".into(),
                role: AxisRole::Timing,
                rf_value: 0.0,
                rf_sigma: rf_time_sigma_s,
                opt_value: 0.0,
                opt_sigma: opt_time_sigma_s,
            },
        ];
        let cross = run_cross_raim(&axes, self.p_fa, self.p_md);

        // ── Optical → RF handoff (L23): no-jump mean + NEES consistency ──
        let truth = [0.0_f64; 4];
        let p0 = [
            rf_range_sigma_m.powi(2),
            rf_range_sigma_m.powi(2),
            rf_range_sigma_m.powi(2),
            rf_time_sigma_s.powi(2),
        ];
        let x0: Vec<f64> = p0.iter().map(|&p| p.sqrt()).collect(); // 1σ initial error
        let init = HandoffState::new(x0, p0.to_vec());
        // A consistent measurement draw: tight optical then loose RF, each 1σ from truth.
        let opt_r: Vec<f64> = p0.iter().map(|&p| p * 1e-4).collect();
        let rf_r: Vec<f64> = p0.iter().map(|&p| p * 4.0).collect();
        let optical_updates: Vec<(usize, f64, f64)> = (0..4)
            .map(|i| (i, truth[i] + opt_r[i].sqrt(), opt_r[i]))
            .collect();
        let rf_updates: Vec<(usize, f64, f64)> = (0..4)
            .map(|i| (i, truth[i] + rf_r[i].sqrt(), rf_r[i]))
            .collect();
        let handoff = optical_rf_handoff(
            init,
            &truth,
            &optical_updates,
            &rf_updates,
            self.handoff_inflation,
        );

        // ── N-station availability (L24) ──
        let sites = default_network();
        let avail = run_optical_availability(&sites, self.correlation);

        // ── Validated closed-form oracle residuals ──
        let footprint_hand = self.wavelength_m / self.tx_aperture_m * self.range_m;
        let footprint_residual = (optical.footprint_m - footprint_hand).abs();
        let toa_hand = self.pulse_rms_width_s / n_photons.sqrt();
        let toa_crlb_residual = (opt_time_sigma_s - toa_hand).abs();
        let scale = if self.two_way { 0.5 } else { 1.0 };
        let range_hand = scale * C_M_S * opt_time_sigma_s;
        let range_crlb_residual = (opt_range_sigma_m - range_hand).abs();
        let fuse_hand =
            (1.0 / (1.0 / rf_range_sigma_m.powi(2) + 1.0 / opt_range_sigma_m.powi(2))).sqrt();
        let fusion_residual = (hyb_range_sigma_m - fuse_hand).abs();
        let hand_indep = 1.0
            - sites
                .iter()
                .map(|s| 1.0 - s.availability())
                .product::<f64>();
        let availability_residual = (avail.independent_union - hand_indep).abs();

        Ok(StudyModel {
            optical,
            n_photons,
            rf,
            cn0_linear,
            rows,
            cross,
            handoff,
            avail,
            footprint_residual,
            toa_crlb_residual,
            range_crlb_residual,
            fusion_residual,
            availability_residual,
            scenario_hash: self.compute_hash(),
        })
    }

    /// The committed, byte-stable **Table 1 (precision)** CSV. Fixed-precision scientific
    /// notation so the bytes are reproducible across platforms.
    fn precision_csv(&self, m: &StudyModel) -> String {
        let mut s = String::new();
        s.push_str(
            "# kshana-pro optical/RF hybrid study (L51) — P5 Table 1: ranging/timing precision\n",
        );
        s.push_str(&format!(
            "# scenario_hash={} optical_footprint_m={:.6e} detected_photons={:.6e} rf_cn0_dbhz={:.6e}\n",
            m.scenario_hash, m.optical.footprint_m, m.n_photons, m.rf.cn0_dbhz,
        ));
        s.push_str(
            "# provenance: all magnitudes Modelled (link-budget + DLL inputs); the CRLB and fusion FORMS are Validated closed forms (see oracle residuals in report)\n",
        );
        s.push_str("modality,ranging_sigma_m,timing_sigma_s,provenance\n");
        for r in &m.rows {
            s.push_str(&format!(
                "{},{:.6e},{:.6e},{}\n",
                r.name,
                r.range_sigma_m,
                r.time_sigma_s,
                r.tag.as_str(),
            ));
        }
        s
    }

    /// The committed, byte-stable **Table 2 (availability)** CSV: the `N`-station
    /// diversity curve (single-site through five-station), independent and correlated.
    fn availability_csv(&self, m: &StudyModel) -> String {
        let mut s = String::new();
        s.push_str(
            "# kshana-pro optical/RF hybrid study (L51) — P5 Table 2: N-station optical availability\n",
        );
        s.push_str(&format!(
            "# scenario_hash={} single_site_mean={:.6e} independent_union={:.6e} correlated_union={:.6e} correlation={:.6e}\n",
            m.scenario_hash,
            m.avail.single_site_mean,
            m.avail.independent_union,
            m.avail.correlated_union,
            m.avail.correlation,
        ));
        s.push_str(
            "# provenance: independent-union = 1 - prod(1 - a_i) is a Validated closed form. Each per-site a_i = clear_sky x pointing: the clear-sky fraction is externally anchored (Cavazzani et al. 2011, GOES12 clear-night fractions, MNRAS; arXiv:1011.4815), the pointing/acquisition factor and rho are Modelled allocations, so a_i stays Modelled\n",
        );
        s.push_str("n_sites,availability_independent,availability_correlated,provenance\n");
        for p in &m.avail.diversity_curve {
            s.push_str(&format!(
                "{},{:.6e},{:.6e},{}\n",
                p.n_sites,
                p.independent,
                p.correlated,
                Tag::Modelled.as_str(),
            ));
        }
        s
    }

    fn json(&self, m: &StudyModel) -> Value {
        let precision_rows: Vec<Value> = m
            .rows
            .iter()
            .map(|r| {
                json!({
                    "modality": r.name,
                    "ranging_sigma_m": r.range_sigma_m,
                    "timing_sigma_s": r.time_sigma_s,
                    "provenance": r.tag.as_str(),
                })
            })
            .collect();
        let diversity: Vec<Value> = m
            .avail
            .diversity_curve
            .iter()
            .map(|p| {
                json!({
                    "n_sites": p.n_sites,
                    "independent": p.independent,
                    "correlated": p.correlated,
                })
            })
            .collect();
        let per_site: Vec<Value> = m
            .avail
            .per_site
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "clear_sky_prob": s.clear_sky_prob,
                    "pointing_acquisition_factor": s.pointing_acquisition_factor,
                    "availability": s.availability,
                })
            })
            .collect();

        json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "paper": "P5 — heterogeneous optical + RF PNT: precision, integrity, availability (L51)",
            "scenario_hash": m.scenario_hash,
            "label": LABEL,
            "provenance_legend": {
                "Validated": "the photon-limited CRLB (sigma_tau/sqrt(N)), the diffraction footprint ((lambda/D)*L), the inverse-variance fused precision, and the independent-union availability (1 - prod(1-a_i)) are closed forms checked to machine precision against independent hand values.",
                "Modelled": "the RF and optical link-budget magnitudes (C/N0, detected-photon count N), the RF DLL ranging model, the per-site clear-sky availabilities and the spatial correlation, and the cross-RAIM / handoff covariance magnitudes.",
            },
            "table1_precision": {
                "rows": precision_rows,
                "csv_filename": "optical_rf_precision.csv",
            },
            "table2_availability": {
                "single_site_mean": tagged(json!(m.avail.single_site_mean), Tag::Modelled),
                "best_single": tagged(json!(m.avail.best_single), Tag::Modelled),
                "independent_union": tagged(json!(m.avail.independent_union), Tag::Modelled),
                "correlated_union": tagged(json!(m.avail.correlated_union), Tag::Modelled),
                "correlation": m.avail.correlation,
                "per_site": per_site,
                "diversity_curve": diversity,
                "csv_filename": "optical_availability.csv",
            },
            "optical_link": {
                "divergence_rad": tagged(json!(m.optical.divergence_rad), Tag::Validated),
                "footprint_m": tagged(json!(m.optical.footprint_m), Tag::Validated),
                "geometric_loss_db": tagged(json!(m.optical.geometric_loss_db), Tag::Modelled),
                "received_power_w": tagged(json!(m.optical.received_power_w), Tag::Modelled),
                "photon_energy_j": tagged(json!(m.optical.photon_energy_j), Tag::Validated),
                "photon_rate_hz": tagged(json!(m.optical.photon_rate_hz), Tag::Modelled),
                "detected_photons": tagged(json!(m.n_photons), Tag::Modelled),
            },
            "rf_link": {
                "fsl_db": tagged(json!(m.rf.fsl_db), Tag::Validated),
                "cn0_dbhz": tagged(json!(m.rf.cn0_dbhz), Tag::Modelled),
                "cn0_linear_hz": tagged(json!(m.cn0_linear), Tag::Modelled),
                "eb_n0_db": tagged(json!(m.rf.eb_n0_db), Tag::Modelled),
                "margin_db": tagged(json!(m.rf.margin_db), Tag::Modelled),
                "closes": m.rf.closes,
            },
            "cross_raim": {
                "n_axes": m.cross.n_axes,
                "chi2_statistic": tagged(json!(m.cross.chi2_statistic), Tag::Validated),
                "chi2_threshold": tagged(json!(m.cross.chi2_threshold), Tag::Validated),
                "fault_detected": m.cross.fault_detected,
                "hpl_m": tagged(json!(m.cross.hpl_m), Tag::Modelled),
                "vpl_m": tagged(json!(m.cross.vpl_m), Tag::Modelled),
                "tpl_s": tagged(json!(m.cross.tpl_s), Tag::Modelled),
            },
            "handoff": {
                "mean_continuous": m.handoff.mean_continuous,
                "max_mean_jump": tagged(json!(m.handoff.max_mean_jump), Tag::Validated),
                "variance_after_optical": tagged(json!(m.handoff.variance_after_optical), Tag::Modelled),
                "variance_after_handoff": tagged(json!(m.handoff.variance_after_handoff), Tag::Modelled),
                "variance_after_rf": tagged(json!(m.handoff.variance_after_rf), Tag::Modelled),
                "final_nees": tagged(json!(m.handoff.final_nees), Tag::Modelled),
                "nees_gate": [m.handoff.nees_gate.0, m.handoff.nees_gate.1],
                "nees_in_gate": m.handoff.nees_in_gate,
                "dof": m.handoff.dof,
            },
            "oracles": {
                "footprint_lambda_over_d_residual": tagged(json!(m.footprint_residual), Tag::Validated),
                "toa_crlb_sigma_over_sqrt_n_residual": tagged(json!(m.toa_crlb_residual), Tag::Validated),
                "range_crlb_residual": tagged(json!(m.range_crlb_residual), Tag::Validated),
                "inverse_variance_fusion_residual": tagged(json!(m.fusion_residual), Tag::Validated),
                "independent_union_product_rule_residual": tagged(json!(m.availability_residual), Tag::Validated),
            },
            "figure": { "fig1_svg": fig1_svg(m) },
            "report_html": study_report_html(self, m),
            "csv_precision": self.precision_csv(m),
            "csv_availability": self.availability_csv(m),
        })
    }

    fn summary(&self, m: &StudyModel) -> String {
        let rf = &m.rows[0];
        let opt = &m.rows[1];
        let hyb = &m.rows[2];
        format!(
            "Kshana Pro optical/RF hybrid PNT study (P5/L51) | optical footprint {fp:.1} m ((lambda/D)*L, Validated) | \
             ranging 1sigma: RF {rfm:.3e} m / optical {optm:.3e} m / hybrid {hybm:.3e} m | \
             timing 1sigma hybrid {hybs:.3e} s | cross-RAIM HPL {hpl:.3e} m TPL {tpl:.3e} s (fault {det}) | \
             handoff mean-continuous {cont} NEES-in-gate {ing} | availability single-site {ss:.1}% -> \
             5-station {corr:.1}% correlated ({ind:.1}% independent) | Tables byte-stable",
            fp = m.optical.footprint_m,
            rfm = rf.range_sigma_m,
            optm = opt.range_sigma_m,
            hybm = hyb.range_sigma_m,
            hybs = hyb.time_sigma_s,
            hpl = m.cross.hpl_m,
            tpl = m.cross.tpl_s,
            det = m.cross.fault_detected,
            cont = m.handoff.mean_continuous,
            ing = m.handoff.nees_in_gate,
            ss = m.avail.single_site_mean * 100.0,
            corr = m.avail.correlated_union * 100.0,
            ind = m.avail.independent_union * 100.0,
        )
    }

    /// Render the branded PDF deliverable (self-contained `%PDF-…`).
    fn to_pdf(&self, m: &StudyModel) -> Result<Vec<u8>, String> {
        let mut pdf = StudyPdf::new();
        pdf.cover(
            "Heterogeneous Optical + RF PNT",
            "P5 / L51 — precision, cross-modality integrity, and N-station availability",
        );
        pdf.modelled_banner(&[
            "The photon-limited CRLB, the diffraction footprint, the fused precision and the",
            "N-station availability are Validated closed forms; the link/DLL/climatology magnitudes are Modelled.",
        ]);

        pdf.heading("Executive Summary");
        pdf.paragraph(&self.summary(m));
        pdf.paragraph(
            "A heterogeneous PNT service pairs a tight but weather-limited optical link with a \
             loose but always-on RF link. This study quantifies the three axes that decide whether \
             the pairing is worth it: precision (RF vs optical vs the inverse-variance hybrid), \
             cross-modality integrity (a chi-square solution-separation RAIM plus a provably no-jump \
             optical<->RF handoff), and availability (the diversity gain of an N-station optical \
             ground network).",
        );

        pdf.heading("Provenance & Traceability");
        pdf.kv("Overlay", &format!("kshana-pro {}", crate::version()));
        pdf.kv("Scenario hash", &m.scenario_hash);
        pdf.kv(
            "Optical footprint",
            &format!(
                "{:.1} m  ((lambda/D)*L, Validated; residual {:.2e})",
                m.optical.footprint_m, m.footprint_residual
            ),
        );
        pdf.kv("Detected photons N", &format!("{:.3e}", m.n_photons));
        pdf.kv("RF C/N0", &format!("{:.2} dB-Hz", m.rf.cn0_dbhz));

        pdf.heading("Table 1 — Ranging / timing precision (1 sigma)");
        pdf.header_row(&[
            (0.0, "Modality"),
            (50.0, "Ranging (m)"),
            (100.0, "Timing (s)"),
            (145.0, "Prov."),
        ]);
        for r in &m.rows {
            let rm = format!("{:.3e}", r.range_sigma_m);
            let ts = format!("{:.3e}", r.time_sigma_s);
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: r.name,
                    bold: false,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 50.0,
                    text: &rm,
                    bold: true,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 100.0,
                    text: &ts,
                    bold: false,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 145.0,
                    text: r.tag.as_str(),
                    bold: true,
                    colour: BRASS_MUTED,
                },
            ]);
        }

        pdf.heading("Table 2 — N-station optical availability");
        pdf.header_row(&[
            (0.0, "Stations"),
            (40.0, "Independent"),
            (95.0, "Correlated"),
        ]);
        for p in &m.avail.diversity_curve {
            let n = format!("{}", p.n_sites);
            let ind = format!("{:.1}%", p.independent * 100.0);
            let cor = format!("{:.1}%", p.correlated * 100.0);
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: &n,
                    bold: false,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 40.0,
                    text: &ind,
                    bold: true,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 95.0,
                    text: &cor,
                    bold: false,
                    colour: crate::report_brand::INK,
                },
            ]);
        }

        pdf.heading("Cross-modality integrity");
        pdf.kv(
            "Cross-RAIM",
            &format!(
                "HPL {:.3e} m, VPL {:.3e} m, TPL {:.3e} s; fault_detected = {}",
                m.cross.hpl_m, m.cross.vpl_m, m.cross.tpl_s, m.cross.fault_detected
            ),
        );
        pdf.kv(
            "Handoff",
            &format!(
                "mean_continuous = {} (max jump {:.2e}); NEES {:.3} in gate [{:.2}, {:.2}] = {}",
                m.handoff.mean_continuous,
                m.handoff.max_mean_jump,
                m.handoff.final_nees,
                m.handoff.nees_gate.0,
                m.handoff.nees_gate.1,
                m.handoff.nees_in_gate
            ),
        );

        pdf.heading("Caveats (read first)");
        pdf.paragraph(
            "- The CRLB, footprint, fusion and availability FORMS are Validated closed forms; every \
             magnitude is a Modelled allocation, and the precision / protection-level / availability \
             values inherit them.",
        );
        pdf.paragraph(
            "- The photon-limited ranging bound assumes shot-noise-limited detection; real terminals \
             hit systematic floors well above it. This is a reproducible study, not a certification.",
        );

        let footer =
            "Ashforde OU  |  Kshana Pro  |  MODELLED engineering study - not a certification";
        pdf.render("Kshana Pro - Optical + RF hybrid PNT (L51)", footer)
    }

    /// Write the L51 paper artifacts (two committed CSVs + figure SVG + HTML report +
    /// branded PDF) into `papers/optical-rf-hybrid/` (a NEW dir — the founder's other
    /// paper dirs are untouched). Invoked out-of-band, never from `run`.
    pub fn emit_paper_artifacts(&self) -> std::io::Result<std::path::PathBuf> {
        let m = self
            .compute()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("papers/optical-rf-hybrid");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("optical_rf_precision.csv"), self.precision_csv(&m))?;
        std::fs::write(
            dir.join("optical_availability.csv"),
            self.availability_csv(&m),
        )?;
        std::fs::write(dir.join("fig1_optical_rf_hybrid.svg"), fig1_svg(&m))?;
        std::fs::write(dir.join("report.html"), study_report_html(self, &m))?;
        let pdf = self.to_pdf(&m).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("report.pdf"), pdf)?;
        Ok(dir)
    }
}

impl Scenario for OpticalRfHybridStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let doc = self.json(&m).to_string();
        let svg = fig1_svg(&m);
        let summary = self.summary(&m);
        Ok(RunOutput {
            json: doc,
            svg,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for OpticalRfHybridStudyScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(Self::KIND));
    }
}

// ── Figure 1 — two panels: precision bars (log) + availability vs N stations ──

fn fig1_svg(m: &StudyModel) -> String {
    let (w, h) = (760.0_f64, 720.0_f64);
    let (ml, mr) = (78.0_f64, 40.0_f64);
    let pw = w - ml - mr;
    let (x0, x1) = (ml, ml + pw);

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"28\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"18\" font-weight=\"bold\">Figure 1 — Heterogeneous optical + RF PNT</text>");

    // ---- Panel (a): ranging precision (log scale) RF vs optical vs hybrid ----
    let (at, ah) = (70.0_f64, 250.0_f64);
    let ab = at + ah;
    s += &format!("<text x=\"20\" y=\"52\" fill=\"{IVORY}\">(a) ranging precision 1&#963; (log scale) &#8226; optical = <tspan fill=\"{TEAL}\">photon-limited CRLB &#963;&#964;/&#8730;N</tspan></text>");

    let vals: Vec<f64> = m.rows.iter().map(|r| r.range_sigma_m).collect();
    let vmin = vals
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min)
        .max(1e-300);
    let vmax = vals.iter().cloned().fold(0.0_f64, f64::max);
    let amin_log = vmin.log10().floor();
    let amax_log = vmax.log10().ceil();
    let span = (amax_log - amin_log).max(1.0);
    let ay = |v: f64| {
        let vl = v.max(1e-300).log10();
        at + ah * (1.0 - (vl - amin_log) / span)
    };
    // Axes + decade gridlines.
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{ab}\" x2=\"{x1}\" y2=\"{ab}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{at}\" x2=\"{x0}\" y2=\"{ab}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    let mut d = amin_log as i64;
    while (d as f64) <= amax_log {
        let y = ay(10f64.powi(d as i32));
        s += &format!(
            "<line x1=\"{x0}\" y1=\"{y:.1}\" x2=\"{x1}\" y2=\"{y:.1}\" stroke=\"#1c2637\"/>"
        );
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\" font-size=\"9\">1e{d} m</text>", x0 - 6.0, y + 3.0);
        d += 1;
    }
    let n = m.rows.len().max(1);
    let slot = pw / n as f64;
    let bar_w = slot * 0.5;
    let bar_colors = [BRASS, TEAL, IVORY_LIGHT];
    for (i, r) in m.rows.iter().enumerate() {
        let cx = x0 + slot * (i as f64 + 0.5);
        let top = ay(r.range_sigma_m);
        let col = bar_colors[i.min(bar_colors.len() - 1)];
        s += &format!(
            "<rect x=\"{:.1}\" y=\"{top:.1}\" width=\"{bar_w:.1}\" height=\"{:.1}\" fill=\"{col}\"/>",
            cx - bar_w / 2.0,
            (ab - top).max(0.0),
        );
        s += &format!("<text x=\"{cx:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"9\" text-anchor=\"middle\">{:.2e} m</text>", top - 4.0, r.range_sigma_m);
        s += &format!("<text x=\"{cx:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\" text-anchor=\"middle\">{}</text>", ab + 16.0, r.name);
    }
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">ranging 1&#963; (m, log; lower is better)</text>", (x0 + x1) / 2.0, ab + 36.0);

    // ---- Panel (b): availability vs number of ground stations ----
    let (bt, bh) = (430.0_f64, 210.0_f64);
    let bb = bt + bh;
    s += &format!("<text x=\"20\" y=\"{:.1}\" fill=\"{IVORY}\">(b) optical availability vs number of ground stations &#8226; <tspan fill=\"{IVORY_LIGHT}\">independent union</tspan> vs <tspan fill=\"{TEAL}\">spatially-correlated</tspan></text>", bt - 20.0);

    let nsta = m.avail.diversity_curve.len().max(1);
    let bx = |k: usize| {
        if nsta <= 1 {
            (x0 + x1) / 2.0
        } else {
            x0 + (k as f64) / ((nsta - 1) as f64) * pw
        }
    };
    let by = |a: f64| bb - a.clamp(0.0, 1.0) * bh;
    // Axes + y gridlines at 0/50/90/95/100 %.
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{bb}\" x2=\"{x1}\" y2=\"{bb}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{bt}\" x2=\"{x0}\" y2=\"{bb}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    for pct in [0.0_f64, 0.5, 0.9, 0.95, 1.0] {
        let y = by(pct);
        s += &format!(
            "<line x1=\"{x0}\" y1=\"{y:.1}\" x2=\"{x1}\" y2=\"{y:.1}\" stroke=\"#1c2637\"/>"
        );
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\" font-size=\"9\">{:.0}%</text>", x0 - 6.0, y + 3.0, pct * 100.0);
    }
    // x labels.
    for (k, p) in m.avail.diversity_curve.iter().enumerate() {
        let x = bx(k);
        s += &format!("<text x=\"{x:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"middle\" font-size=\"9\">{}</text>", bb + 15.0, p.n_sites);
    }
    // Independent + correlated polylines.
    let ind_pts: String = m
        .avail
        .diversity_curve
        .iter()
        .enumerate()
        .map(|(k, p)| format!("{:.1},{:.1}", bx(k), by(p.independent)))
        .collect::<Vec<_>>()
        .join(" ");
    let cor_pts: String = m
        .avail
        .diversity_curve
        .iter()
        .enumerate()
        .map(|(k, p)| format!("{:.1},{:.1}", bx(k), by(p.correlated)))
        .collect::<Vec<_>>()
        .join(" ");
    s += &format!("<polyline points=\"{ind_pts}\" fill=\"none\" stroke=\"{IVORY_LIGHT}\" stroke-width=\"2.4\"/>");
    s += &format!("<polyline points=\"{cor_pts}\" fill=\"none\" stroke=\"{TEAL}\" stroke-width=\"2.4\" stroke-dasharray=\"4 3\"/>");
    for (k, p) in m.avail.diversity_curve.iter().enumerate() {
        let x = bx(k);
        s += &format!(
            "<circle cx=\"{x:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"{IVORY_LIGHT}\"/>",
            by(p.independent)
        );
        s += &format!(
            "<circle cx=\"{x:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"{TEAL}\"/>",
            by(p.correlated)
        );
    }
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">number of optical ground stations</text>", (x0 + x1) / 2.0, bb + 34.0);

    s += "</svg>";
    s
}

// ── Branded HTML report ──────────────────────────────────────────────────────

fn td(s: &str) -> String {
    format!("<td style=\"border:1px solid #26324a;padding:4px 8px\">{s}</td>")
}
fn th(s: &str) -> String {
    format!("<th style=\"border:1px solid #26324a;padding:4px 8px;text-align:left;color:{IVORY_LIGHT}\">{s}</th>")
}

fn study_report_html(scn: &OpticalRfHybridStudyScenario, m: &StudyModel) -> String {
    // Every Kshana Pro document is an Ashforde document: the shared furniture of
    // crate::report_brand goes around the renderer's own page, which is unchanged.
    let body = study_report_html_unbranded(scn, m);
    let id = crate::report_brand::content_id(&body);
    crate::report_brand::finish_html(
        &body,
        &crate::report_brand::DocMark::new("Optical and radio-frequency hybrid study", id),
    )
}

fn study_report_html_unbranded(scn: &OpticalRfHybridStudyScenario, m: &StudyModel) -> String {
    let fig = fig1_svg(m);
    let prec_rows: String = m
        .rows
        .iter()
        .map(|r| {
            let tagc = r.tag.color();
            format!(
                "<tr>{}{}{}{}</tr>",
                td(r.name),
                td(&format!("{:.3e}", r.range_sigma_m)),
                td(&format!("{:.3e}", r.time_sigma_s)),
                td(&format!(
                    "<span style=\"color:{tagc};font-weight:600\">{}</span>",
                    r.tag.as_str()
                )),
            )
        })
        .collect();
    let avail_rows: String = m
        .avail
        .diversity_curve
        .iter()
        .map(|p| {
            format!(
                "<tr>{}{}{}</tr>",
                td(&p.n_sites.to_string()),
                td(&format!("{:.1}%", p.independent * 100.0)),
                td(&format!("{:.1}%", p.correlated * 100.0)),
            )
        })
        .collect();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — Optical + RF Hybrid PNT (L51)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:920px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Precision, integrity, availability</h2>\
         <p>The 1550 nm ({mode}) optical link spreads to a <strong>{fp:.1} m</strong> footprint at range \
         (&#8776; (&#955;/D)&#183;L, <span style=\"color:{TEAL}\">Validated</span>), delivering \
         <strong>{np:.2e}</strong> detected photons and a photon-limited ranging bound far below the \
         RF code-tracking jitter. The inverse-variance <em>hybrid</em> inherits the tighter optical \
         precision while the RF link supplies the always-on integrity and availability. Cross-modality \
         RAIM reports HPL {hpl:.2e} m / TPL {tpl:.2e} s (fault_detected = {det}), and the \
         optical&#8596;RF handoff is <strong>mean-continuous = {cont}</strong> \
         (<span style=\"color:{TEAL}\">Validated no-jump invariant</span>).</p>\
         {fig}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Table 1 — Ranging / timing precision (1&#963;)</h2>\
         <table style=\"border-collapse:collapse;width:100%\">\
         <tr>{h_mod}{h_r}{h_t}{h_prov}</tr>{prec_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Table 2 — N-station optical availability</h2>\
         <p>Single-site mean <strong>{ss:.1}%</strong> &#8594; three-station \
         <strong>{three:.1}%</strong> &#8594; four-station <strong>{four:.1}%</strong> &#8594; \
         five-station <strong>{corr:.1}%</strong> (spatially correlated; the independent union \
         optimistically reads {ind:.1}%).</p>\
         <table style=\"border-collapse:collapse;width:100%\">\
         <tr>{h_n}{h_ind}{h_cor}</tr>{avail_rows}</table>\
         <p style=\"font-size:12px;opacity:.85\">Validated closed-form residuals: footprint \
         {rfp:.1e}, CRLB(&#964;) {rtoa:.1e}, CRLB(range) {rrng:.1e}, fusion {rfus:.1e}, \
         availability {rav:.1e} — all &#8776; 0. The independent-union availability is the \
         <span style=\"color:{TEAL}\">Validated</span> closed form 1 &#8722; &#928;(1&#8722;a<sub>i</sub>); \
         every link-budget, DLL and climatology magnitude is <span style=\"color:{BRASS}\">Modelled</span>. \
         Both tables are committed byte-stable to <code>papers/optical-rf-hybrid/</code>.</p>\
         </body></html>",
        cover = cover_html(
            "Heterogeneous Optical + RF PNT",
            "P5 / L51 — precision, cross-modality integrity, and N-station availability"
        ),
        banner = modelled_banner_html(
            "The photon-limited CRLB, the diffraction footprint, the inverse-variance fusion and \
             the N-station availability are Validated closed forms; the link-budget, DLL and \
             climatology magnitudes are Modelled allocations."
        ),
        fp = m.optical.footprint_m,
        np = m.n_photons,
        hpl = m.cross.hpl_m,
        tpl = m.cross.tpl_s,
        det = m.cross.fault_detected,
        cont = m.handoff.mean_continuous,
        ss = m.avail.single_site_mean * 100.0,
        three = m.avail.diversity_curve.get(2).map(|p| p.independent).unwrap_or(0.0) * 100.0,
        four = m.avail.diversity_curve.get(3).map(|p| p.independent).unwrap_or(0.0) * 100.0,
        corr = m.avail.correlated_union * 100.0,
        ind = m.avail.independent_union * 100.0,
        rfp = m.footprint_residual,
        rtoa = m.toa_crlb_residual,
        rrng = m.range_crlb_residual,
        rfus = m.fusion_residual,
        rav = m.availability_residual,
        h_mod = th("Modality"),
        h_r = th("Ranging 1&#963; (m)"),
        h_t = th("Timing 1&#963; (s)"),
        h_prov = th("Provenance"),
        h_n = th("Stations"),
        h_ind = th("Independent"),
        h_cor = th("Correlated"),
        mode = if scn.two_way { "two-way" } else { "one-way" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_default() -> (Value, String) {
        let scn = OpticalRfHybridStudyScenario::default();
        let out = scn.run().unwrap();
        (serde_json::from_str(&out.json).unwrap(), out.summary)
    }

    #[test]
    fn runs_and_carries_the_honesty_label() {
        let (v, summary) = run_default();
        assert_eq!(v["kind"], OpticalRfHybridStudyScenario::KIND);
        let label = v["label"].as_str().unwrap();
        // Both tier words must literally appear in the document.
        assert!(label.contains("VALIDATED"));
        assert!(label.contains("MODELLED"));
        assert!(summary.contains("availability"));
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let scn = OpticalRfHybridStudyScenario::default();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn csv_tables_are_well_formed_and_deterministic() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        let p = scn.precision_csv(&m);
        let a = scn.availability_csv(&m);
        assert_eq!(p, scn.precision_csv(&m));
        assert_eq!(a, scn.availability_csv(&m));
        assert!(p.contains("modality,ranging_sigma_m,timing_sigma_s,provenance\n"));
        assert!(p.contains("scenario_hash="));
        // Three modality rows (rf/optical/hybrid).
        let prows = p
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("modality"))
            .count();
        assert_eq!(prows, 3);
        assert!(a.contains("n_sites,availability_independent,availability_correlated,provenance\n"));
        let arows = a
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("n_sites"))
            .count();
        assert_eq!(arows, m.avail.diversity_curve.len());
    }

    /// Reproducibility drift-guard: a fresh regeneration of the P5 Table-1 (precision)
    /// and Table-2 (availability) CSVs from the SAME default config that
    /// `emit_paper_artifacts` uses must byte-match the COMMITTED paper artifacts under
    /// `papers/optical-rf-hybrid/`. This is REPRODUCIBILITY (fresh code output ==
    /// committed golden), NOT external validation — it does not assert flight validation,
    /// certification, or ESA acceptance, only that our own deterministic emit still
    /// reproduces the committed bytes. If it drifts, refresh the committed CSVs with
    /// `cargo test -p kshana-pro emit_paper_artifacts -- --ignored`.
    #[test]
    fn committed_csv_tables_match_regeneration() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();

        let fresh_precision = scn.precision_csv(&m);
        let committed_precision = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/papers/optical-rf-hybrid/optical_rf_precision.csv"
        ))
        .expect("committed optical_rf_precision.csv must exist (run emit_paper_artifacts)");
        assert_eq!(
            fresh_precision, committed_precision,
            "committed optical_rf_precision.csv drifted from regeneration"
        );

        let fresh_availability = scn.availability_csv(&m);
        let committed_availability = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/papers/optical-rf-hybrid/optical_availability.csv"
        ))
        .expect("committed optical_availability.csv must exist (run emit_paper_artifacts)");
        assert_eq!(
            fresh_availability, committed_availability,
            "committed optical_availability.csv drifted from regeneration"
        );
    }

    #[test]
    fn svg_is_well_formed_and_deterministic() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        let a = fig1_svg(&m);
        assert!(a.starts_with("<svg"));
        assert!(a.ends_with("</svg>"));
        assert_eq!(a, fig1_svg(&m));
    }

    #[test]
    fn pdf_renders_to_valid_bytes() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        let pdf = scn.to_pdf(&m).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    /// ORACLE: the optical timing precision equals the closed-form σ_τ/√N, and the range
    /// bound equals (c/2 or c)·σ_ToA, both to machine precision.
    #[test]
    fn oracle_photon_limited_crlb_equals_sigma_over_sqrt_n() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        let opt = &m.rows[1];
        let hand_toa = scn.pulse_rms_width_s / m.n_photons.sqrt();
        assert!(
            (opt.time_sigma_s - hand_toa).abs() < 1e-24,
            "optical toa {} vs σ_τ/√N {hand_toa}",
            opt.time_sigma_s
        );
        let scale = if scn.two_way { 0.5 } else { 1.0 };
        let hand_range = scale * C_M_S * hand_toa;
        assert!((opt.range_sigma_m - hand_range).abs() < 1e-12);
        assert!(m.toa_crlb_residual < 1e-24 && m.range_crlb_residual < 1e-12);
    }

    /// ORACLE: the N-station independent-union availability equals 1 − Π(1−a_i) to machine
    /// precision.
    #[test]
    fn oracle_n_station_availability_equals_product_rule() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        let sites = default_network();
        let hand = 1.0
            - sites
                .iter()
                .map(|s| 1.0 - s.availability())
                .product::<f64>();
        assert!((m.avail.independent_union - hand).abs() < 1e-12);
        assert!(m.availability_residual < 1e-12);
    }

    /// ORACLE: the diffraction footprint equals (λ/D)·L, and the inverse-variance fused
    /// precision equals 1/√(1/σ_rf² + 1/σ_opt²).
    #[test]
    fn oracle_footprint_and_fusion_closed_forms() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        let hand_fp = scn.wavelength_m / scn.tx_aperture_m * scn.range_m;
        assert!((m.optical.footprint_m - hand_fp).abs() < 1e-6);
        assert!(m.footprint_residual < 1e-6);
        let rf = m.rows[0].range_sigma_m;
        let opt = m.rows[1].range_sigma_m;
        let hyb = m.rows[2].range_sigma_m;
        let hand = (1.0 / (1.0 / rf.powi(2) + 1.0 / opt.powi(2))).sqrt();
        assert!((hyb - hand).abs() < 1e-18 * hand.max(1.0));
        assert!(m.fusion_residual < 1e-18 * hand.max(1.0));
        // The hybrid is at least as tight as the tighter (optical) modality.
        assert!(hyb <= opt * (1.0 + 1e-12));
    }

    /// Table 2 reproduces the diversity progression of the **externally anchored**
    /// climatology: mean single-site ≈ 69 %, three-station ≈ 98 %, four-station ≈ 99 %,
    /// five-station correlated ≈ 99.5 %.
    ///
    /// These bands moved. Until the open core's `7761e0a`, `default_network()` carried
    /// illustrative per-site numbers giving ≈ 53 % / 90 % / 95 % / 96 %, and this test
    /// pinned them. That commit re-based the per-site clear-sky fractions on a published
    /// external dataset (Cavazzani et al. 2011, GOES12 clear-night fractions, MNRAS;
    /// arXiv:1011.4815) and renamed its own test off the old headline. The old bands were
    /// therefore pinning a superseded illustrative figure against a dataset-anchored
    /// engine — so the bands were moved to the anchored values rather than the engine
    /// being held back to reproduce them.
    #[test]
    fn table2_reproduces_the_externally_anchored_diversity_progression() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        assert!(
            (0.66..0.72).contains(&m.avail.single_site_mean),
            "mean single-site {} not ≈ 69%",
            m.avail.single_site_mean
        );
        let three = m.avail.diversity_curve[2].independent;
        assert!((0.96..0.99).contains(&three), "3-station {three} not ≈ 98%");
        let four = m.avail.diversity_curve[3].independent;
        assert!((0.98..0.995).contains(&four), "4-station {four} not ≈ 99%");
        assert!(
            (0.99..0.999).contains(&m.avail.correlated_union),
            "5-station correlated {} not ≈ 99.5%",
            m.avail.correlated_union
        );
        assert!(m.avail.correlated_union < m.avail.independent_union);
    }

    /// Cross-RAIM does not flag a consistent heterogeneous solution, and the optical→RF
    /// handoff is bit-continuous in the mean (the Validated no-jump invariant) and stays
    /// inside its NEES consistency gate.
    #[test]
    fn cross_raim_clean_and_handoff_continuous_and_consistent() {
        let scn = OpticalRfHybridStudyScenario::default();
        let m = scn.compute().unwrap();
        assert!(
            !m.cross.fault_detected,
            "consistent modalities must not fault"
        );
        assert!(m.cross.hpl_m > 0.0 && m.cross.tpl_s > 0.0);
        assert!(
            m.handoff.mean_continuous,
            "handoff must carry the mean bit-for-bit"
        );
        assert_eq!(m.handoff.max_mean_jump, 0.0);
        assert!(
            m.handoff.nees_in_gate,
            "final NEES must sit in the consistency gate"
        );
    }

    #[test]
    fn bad_config_is_rejected() {
        let bad_band = OpticalRfHybridStudyScenario {
            rf_band: "q".into(),
            ..Default::default()
        };
        assert!(bad_band.run().is_err());
        let bad_power = OpticalRfHybridStudyScenario {
            tx_power_w: 0.0,
            ..Default::default()
        };
        assert!(bad_power.run().is_err());
        let bad_range = OpticalRfHybridStudyScenario {
            range_m: -1.0,
            ..Default::default()
        };
        assert!(bad_range.run().is_err());
    }

    #[test]
    #[ignore = "writes the committed paper artifacts to papers/optical-rf-hybrid/; run with --ignored"]
    fn emit_paper_artifacts() {
        let dir = OpticalRfHybridStudyScenario::default()
            .emit_paper_artifacts()
            .unwrap();
        assert!(dir.join("optical_rf_precision.csv").exists());
        assert!(dir.join("optical_availability.csv").exists());
        assert!(dir.join("fig1_optical_rf_hybrid.svg").exists());
        assert!(dir.join("report.html").exists());
        assert!(dir.join("report.pdf").exists());
    }
}
