// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L49 + L50 — `pro-realtime-frame-eop-study`
//!
//! A Kshana Pro *study* pack that recasts P4's real-time lunar frame / Earth-
//! orientation check as a **data-forward decomposition** — the direct answer to a
//! hostile reviewer's "this is circular" objection.
//!
//! The pack computes the frame error **forward from real data**: it takes the
//! data-derived UT1 prediction error `ΔUT1(h)` at horizon `h` — the L18 curve read
//! off the real IERS `finals2000A` series bundled by the open `realtime-frame-eop`
//! scenario ([`kshana::realtime_frame_eop`]) — and pushes it through the L19 lever arm
//! `Δr = D_EM·ω⊕·ΔUT1` to a position / timing error **at the Moon**. It then presents
//! the resulting ~14.4 m (real-time / multi-day) and ~0.27 m (post-processed / final
//! floor) figures against the *independently* modelled OD-covariance budget
//! ([`kshana::lunar_frame_predict::predict_frame_error`]) as an **independent
//! prediction, not a consistency identity**.
//!
//! The two roads are structurally disjoint — the data-forward road uses **no** OD
//! covariance, the modelled-OD road uses **no** EOP data — so their agreement is a
//! prediction. The pack makes that explicit in the JSON and the report, and its
//! `independence_*` tests prove it: perturbing the OD covariance leaves the
//! data-forward positions unchanged, and perturbing the EOP horizons leaves the
//! modelled-OD budget unchanged.
//!
//! ## Validated vs Modelled (honesty firewall)
//! * **Validated.** The data-forward Moon positions are the L18 real-data UT1 curve
//!   mapped through the L19 closed-form lever arm (`1 ms ↔ 28.03 m ↔ 93.5 ns`, `ω⊕`
//!   cross-checked against the CIO Earth-rotation angle) — real data + exact closed
//!   form.
//! * **Modelled.** The OD-covariance budget magnitudes (0.27 m post-processed, ~14.4 m
//!   real-time at 1 h latency) and the frame-realisation floor are representative
//!   allocations, and the persistence predictor is not IERS's operational Bulletin A
//!   algorithm. Not a certified real-time frame product.

use kshana::api::{run_toml, ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::frame_eop::{lunar_position_to_ut1, LEVER_M_PER_S, OMEGA_EARTH_RAD_S};
use kshana::lunar_frame_predict::{
    predict_frame_error, OdCovariance, POSTPROC_POS_SIGMA_M, REALTIME_LATENCY_S,
    REPRESENTATIVE_VEL_SIGMA_MPS,
};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::packs::study_pdf::{Cell, StudyPdf};
use crate::report_brand::{
    cover_html, modelled_banner_html, BRASS, BRASS_MUTED, FONT_BODY, FONT_HEAD, INK, IVORY,
    IVORY_LIGHT, NAVY,
};

/// The explicit anti-circularity narrative carried on every surface.
const NARRATIVE: &str = "data → dUT1(h) → lever arm → Moon-frame position; the agreement with the \
independently-modelled OD budget is a prediction, not a tautology. The data-forward road uses no OD \
covariance and the modelled-OD road uses no EOP data, so neither computation feeds the other.";

/// The honesty label carried on the result document (contains both tier words).
const LABEL: &str = "MODELLED data-forward real-time lunar frame / Earth-orientation prediction decomposition. \
VALIDATED: the data-forward Moon positions are the L18 real-data UT1 prediction-error curve (real IERS \
finals2000A) mapped through the L19 closed-form lever arm (1 ms ↔ 28.03 m ↔ 93.5 ns, omega_earth \
cross-checked against the CIO Earth-rotation angle). MODELLED: the OD-covariance budget magnitudes \
(0.27 m post-processed, ~14.4 m real-time at 1 h latency) and the frame-realisation floor are \
representative allocations, and the multi-day predictor is persistence, not IERS's operational Bulletin \
A algorithm. Not a certified real-time frame product.";

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
}

fn tagged(v: Value, t: Tag) -> Value {
    json!({ "value": v, "provenance": t.as_str() })
}

// ── Scenario ─────────────────────────────────────────────────────────────────

fn d_horizons() -> Vec<u32> {
    vec![1, 2, 3]
}
fn d_latency() -> f64 {
    REALTIME_LATENCY_S
}
fn d_pos_sigma() -> f64 {
    POSTPROC_POS_SIGMA_M
}
fn d_vel_sigma() -> f64 {
    REPRESENTATIVE_VEL_SIGMA_MPS
}
fn d_floor() -> f64 {
    0.2
}
fn d_ut1_ms() -> f64 {
    0.5
}
fn d_zero() -> f64 {
    0.0
}
fn d_epoch() -> String {
    "2022-01-01".to_string()
}

/// A `pro-realtime-frame-eop-study` scenario. Every field is optional; empty TOML
/// runs the representative lunar-relay OD budget over the bundled real `finals2000A`
/// fixture for the 1/2/3-day horizons.
#[derive(Debug, Clone, Deserialize)]
pub struct RealtimeFrameEopStudyScenario {
    /// Informational epoch label (UTC date) for the report.
    #[serde(default = "d_epoch")]
    pub epoch: String,
    /// Persistence-predictor lead times (integer days) for the data-forward curve, in
    /// addition to the always-present rapid-minus-final floor.
    #[serde(default = "d_horizons")]
    pub horizons_days: Vec<u32>,
    /// Real-time prediction latency (s) for the modelled OD budget.
    #[serde(default = "d_latency")]
    pub latency_s: f64,
    /// Post-processed lunar-relay OD position 1σ (m).
    #[serde(default = "d_pos_sigma")]
    pub ephemeris_pos_sigma_m: f64,
    /// Lunar-relay OD velocity 1σ (m/s).
    #[serde(default = "d_vel_sigma")]
    pub ephemeris_vel_sigma_mps: f64,
    /// Frame-realisation (datum-recovery) floor (m) for the L21 RSS budget.
    #[serde(default = "d_floor")]
    pub frame_realization_floor_m: f64,
    /// UT1 prediction error (ms) driving the EOP term of the L21 budget.
    #[serde(default = "d_ut1_ms")]
    pub delta_ut1_ms: f64,
    /// Polar-motion x-pole prediction error (milliarcseconds) for the L21 budget.
    #[serde(default = "d_zero")]
    pub delta_xp_mas: f64,
    /// Polar-motion y-pole prediction error (milliarcseconds) for the L21 budget.
    #[serde(default = "d_zero")]
    pub delta_yp_mas: f64,
}

impl Default for RealtimeFrameEopStudyScenario {
    fn default() -> Self {
        Self {
            epoch: d_epoch(),
            horizons_days: d_horizons(),
            latency_s: d_latency(),
            ephemeris_pos_sigma_m: d_pos_sigma(),
            ephemeris_vel_sigma_mps: d_vel_sigma(),
            frame_realization_floor_m: d_floor(),
            delta_ut1_ms: d_ut1_ms(),
            delta_xp_mas: d_zero(),
            delta_yp_mas: d_zero(),
        }
    }
}

/// One data-forward horizon row: a measured UT1 prediction-error statistic mapped to
/// the Moon by the L19 lever arm. All Validated (real data + closed form).
#[derive(Clone)]
struct DataRow {
    label: String,
    horizon_days: f64,
    n: u64,
    ut1_rms_ms: f64,
    ut1_p50_ms: f64,
    ut1_p95_ms: f64,
    moon_position_m: f64,
    moon_light_time_ns: f64,
}

/// The independently-modelled OD-covariance budget (Modelled magnitudes; the range→
/// time mapping is closed form).
struct ModelledOd {
    pos_sigma_m: f64,
    vel_sigma_mps: f64,
    latency_s: f64,
    postproc_pos_m: f64,
    realtime_pos_m: f64,
    postproc_ns: f64,
    realtime_ns: f64,
}

/// The L21 root-sum-square real-time frame-error budget (from the open scenario).
struct L21Budget {
    delta_ut1_ms: f64,
    delta_xp_mas: f64,
    delta_yp_mas: f64,
    eop_term_m: f64,
    ephemeris_term_m: f64,
    frame_realization_floor_m: f64,
    total_m: f64,
    total_time_ns: f64,
}

struct StudyModel {
    epoch: String,
    eop_source: String,
    scenario_hash: String,
    data_rows: Vec<DataRow>,
    final_floor: Option<DataRow>,
    far_horizon: Option<DataRow>,
    modelled_od: ModelledOd,
    l21: L21Budget,
    /// data-forward final-floor position / modelled post-processed position.
    postproc_ratio: Option<f64>,
    /// data-forward far-horizon position / modelled real-time position.
    realtime_ratio: Option<f64>,
    /// The open scenario's two-panel P4 chart (reused, validated rendering).
    open_svg: String,
}

fn jf(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
fn ju(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

impl RealtimeFrameEopStudyScenario {
    pub const KIND: &'static str = "pro-realtime-frame-eop-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P4/L49+L50 study: recast the real-time lunar frame / Earth-orientation check as a data-forward decomposition — the direct answer to a 'this is circular' objection. Computes frame error FORWARD from real data (the L18 UT1 prediction-error curve read off the real IERS finals2000A series bundled by the open realtime-frame-eop scenario) through the L19 lever arm to a Moon-frame position, and presents the resulting ~15 m (real-time / multi-day) and ~0.27 m (post-processed / final floor) figures against the INDEPENDENTLY modelled OD-covariance budget as a prediction, NOT a tautology (the data-forward road uses no OD covariance; the modelled-OD road uses no EOP data). Emits a byte-stable CSV, a branded 'two roads' figure, a branded HTML report and PDF. Data-forward positions are Validated (real data + closed-form lever arm); the OD-covariance magnitudes and frame-realisation floor are Modelled allocations. Not a certified real-time frame product.",
            required_fields: &[],
            optional_fields: &[
                "epoch",
                "horizons_days",
                "latency_s",
                "ephemeris_pos_sigma_m",
                "ephemeris_vel_sigma_mps",
                "frame_realization_floor_m",
                "delta_ut1_ms",
                "delta_xp_mas",
                "delta_yp_mas",
            ],
        }
    }

    /// The open `realtime-frame-eop` scenario TOML built from this study's knobs. All
    /// floats are emitted with `{:?}` so every value carries a decimal point (TOML
    /// f64) and the source string is byte-stable.
    fn open_scenario_toml(&self) -> String {
        let horizons = self
            .horizons_days
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "kind = \"realtime-frame-eop\"\n\
             epoch = \"{epoch}\"\n\
             horizons_days = [{horizons}]\n\
             latency_s = {latency:?}\n\
             ephemeris_pos_sigma_m = {pos:?}\n\
             ephemeris_vel_sigma_mps = {vel:?}\n\
             frame_realization_floor_m = {floor:?}\n\
             delta_ut1_ms = {ut1:?}\n\
             delta_xp_mas = {xp:?}\n\
             delta_yp_mas = {yp:?}\n",
            epoch = self.epoch,
            latency = self.latency_s,
            pos = self.ephemeris_pos_sigma_m,
            vel = self.ephemeris_vel_sigma_mps,
            floor = self.frame_realization_floor_m,
            ut1 = self.delta_ut1_ms,
            xp = self.delta_xp_mas,
            yp = self.delta_yp_mas,
        )
    }

    fn compute_hash(&self, open_toml: &str) -> String {
        let mut h = Sha256::new();
        h.update(b"realtime-frame-eop-study|");
        h.update(open_toml.as_bytes());
        hex::encode(h.finalize()).chars().take(12).collect()
    }

    fn compute(&self) -> Result<StudyModel, KshanaError> {
        if !self.latency_s.is_finite() || self.latency_s < 0.0 {
            return Err(KshanaError::InvalidInput(format!(
                "latency_s must be finite and non-negative, got {}",
                self.latency_s
            )));
        }
        if self.ephemeris_pos_sigma_m < 0.0 || self.ephemeris_vel_sigma_mps < 0.0 {
            return Err(KshanaError::InvalidInput(
                "ephemeris 1σ values must be non-negative".into(),
            ));
        }

        // ROAD A — data-forward: run the open scenario (bundled real finals2000A) and
        // read its L18→L19 Table 2 (the data-derived Moon positions).
        let open_toml = self.open_scenario_toml();
        let scenario_hash = self.compute_hash(&open_toml);
        let open = run_toml(&open_toml).map_err(KshanaError::InvalidInput)?;
        let root: Value = serde_json::from_str(&open.json)
            .map_err(|e| KshanaError::InvalidInput(format!("open scenario JSON parse: {e}")))?;

        let epoch = root
            .get("epoch")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.epoch)
            .to_string();
        let eop_source = root
            .get("eop_source")
            .and_then(|v| v.as_str())
            .unwrap_or("bundled fixture")
            .to_string();

        let mut data_rows: Vec<DataRow> = Vec::new();
        if let Some(rows) = root
            .get("table2_error_vs_horizon")
            .and_then(|v| v.as_array())
        {
            for r in rows {
                data_rows.push(DataRow {
                    label: r
                        .get("horizon")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string(),
                    horizon_days: jf(r, "horizon_days"),
                    n: ju(r, "n"),
                    ut1_rms_ms: jf(r, "ut1_rms_ms"),
                    ut1_p50_ms: jf(r, "ut1_p50_ms"),
                    ut1_p95_ms: jf(r, "ut1_p95_ms"),
                    moon_position_m: jf(r, "moon_position_m"),
                    moon_light_time_ns: jf(r, "moon_light_time_ns"),
                });
            }
        }
        if data_rows.is_empty() {
            return Err(KshanaError::InvalidInput(
                "open realtime-frame-eop produced no data-forward horizon rows".into(),
            ));
        }
        let final_floor = data_rows.iter().find(|r| r.label == "final").cloned();
        let far_horizon = data_rows
            .iter()
            .filter(|r| r.label != "final")
            .max_by(|a, b| {
                a.horizon_days
                    .partial_cmp(&b.horizon_days)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();

        // ROAD B — modelled OD budget: propagate the representative OD covariance
        // through the latency. Computed HERE, from the library, using NO EOP data.
        let cov = OdCovariance::new(
            self.ephemeris_pos_sigma_m,
            self.ephemeris_vel_sigma_mps,
            0.0,
        );
        let predict = predict_frame_error(cov, self.latency_s);
        let modelled_od = ModelledOd {
            pos_sigma_m: self.ephemeris_pos_sigma_m,
            vel_sigma_mps: self.ephemeris_vel_sigma_mps,
            latency_s: self.latency_s,
            postproc_pos_m: predict.postproc_pos_sigma_m,
            realtime_pos_m: predict.predicted_pos_sigma_m,
            postproc_ns: predict.postproc_time_ns,
            realtime_ns: predict.predicted_time_ns,
        };

        // L21 RSS budget (from the open scenario).
        let b = root
            .get("realtime_frame_error_budget")
            .cloned()
            .unwrap_or(Value::Null);
        let l21 = L21Budget {
            delta_ut1_ms: jf(&b, "delta_ut1_ms"),
            delta_xp_mas: jf(&b, "delta_xp_mas"),
            delta_yp_mas: jf(&b, "delta_yp_mas"),
            eop_term_m: jf(&b, "eop_term_m"),
            ephemeris_term_m: jf(&b, "ephemeris_term_m"),
            frame_realization_floor_m: jf(&b, "frame_realization_floor_m"),
            total_m: jf(&b, "total_m"),
            total_time_ns: jf(&b, "total_time_ns"),
        };

        // The prediction: how close does each independent road land to the other?
        let postproc_ratio = final_floor
            .as_ref()
            .filter(|_| modelled_od.postproc_pos_m > 0.0)
            .map(|r| r.moon_position_m / modelled_od.postproc_pos_m);
        let realtime_ratio = far_horizon
            .as_ref()
            .filter(|_| modelled_od.realtime_pos_m > 0.0)
            .map(|r| r.moon_position_m / modelled_od.realtime_pos_m);

        Ok(StudyModel {
            epoch,
            eop_source,
            scenario_hash,
            data_rows,
            final_floor,
            far_horizon,
            modelled_od,
            l21,
            postproc_ratio,
            realtime_ratio,
            open_svg: open.svg,
        })
    }

    /// The committed, byte-stable CSV: the data-forward horizon rows (Road A) then the
    /// modelled OD budget (Road B), preceded by the anti-circularity narrative header.
    fn csv_string(&self, m: &StudyModel) -> String {
        let mut s = String::new();
        s.push_str("# kshana-pro realtime-frame-eop study (L49/L50) — data-forward frame-error decomposition\n");
        s.push_str(&format!(
            "# scenario_hash={} eop_source={} latency_s={:.6e} lever_arm_m_per_s={:.6e}\n",
            m.scenario_hash, m.eop_source, m.modelled_od.latency_s, LEVER_M_PER_S,
        ));
        s.push_str("# narrative: data -> dUT1(h) -> lever arm -> Moon-frame position; agreement with the independently-modelled OD budget is a PREDICTION, not a tautology\n");
        s.push_str(
            "section,label,horizon_days,n,ut1_ms,moon_position_m,light_time_ns,provenance\n",
        );
        for r in &m.data_rows {
            s.push_str(&format!(
                "data_forward,{},{:.0},{},{:.6},{:.6},{:.6},Validated\n",
                r.label, r.horizon_days, r.n, r.ut1_rms_ms, r.moon_position_m, r.moon_light_time_ns,
            ));
        }
        // Modelled OD road — position expressed as its L19-equivalent UT1 error too.
        let pp_ut1 = lunar_position_to_ut1(m.modelled_od.postproc_pos_m) * 1e3;
        let rt_ut1 = lunar_position_to_ut1(m.modelled_od.realtime_pos_m) * 1e3;
        s.push_str(&format!(
            "modelled_od,post-processed,,,{:.6},{:.6},{:.6},Modelled\n",
            pp_ut1, m.modelled_od.postproc_pos_m, m.modelled_od.postproc_ns,
        ));
        s.push_str(&format!(
            "modelled_od,real-time,,,{:.6},{:.6},{:.6},Modelled\n",
            rt_ut1, m.modelled_od.realtime_pos_m, m.modelled_od.realtime_ns,
        ));
        s
    }

    fn json(&self, m: &StudyModel) -> Value {
        let data_rows: Vec<Value> = m
            .data_rows
            .iter()
            .map(|r| {
                json!({
                    "horizon": r.label,
                    "horizon_days": r.horizon_days,
                    "n": r.n,
                    "ut1_rms_ms": r.ut1_rms_ms,
                    "ut1_p50_ms": r.ut1_p50_ms,
                    "ut1_p95_ms": r.ut1_p95_ms,
                    "moon_position_m": r.moon_position_m,
                    "moon_light_time_ns": r.moon_light_time_ns,
                    "provenance": Tag::Validated.as_str(),
                })
            })
            .collect();

        json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "paper": "P4 — Real-time lunar frame / EOP prediction, data-forward decomposition (L49/L50)",
            "scenario_hash": m.scenario_hash,
            "label": LABEL,
            "narrative": NARRATIVE,
            "provenance_legend": {
                "Validated": "the data-forward Moon positions are the L18 real-data UT1 prediction-error curve (real IERS finals2000A) mapped through the L19 closed-form lever arm; omega_earth is cross-checked against the CIO Earth-rotation angle.",
                "Modelled": "the OD-covariance budget magnitudes (0.27 m post-processed, ~14.4 m real-time at 1 h latency) and the frame-realisation floor are representative allocations; the multi-day predictor is persistence, not IERS's Bulletin A algorithm.",
            },
            "epoch": m.epoch,
            "eop_source": m.eop_source,
            "lever_arm": {
                "lever_arm_m_per_s": tagged(json!(LEVER_M_PER_S), Tag::Validated),
                "omega_earth_rad_s": tagged(json!(OMEGA_EARTH_RAD_S), Tag::Validated),
            },
            "road_a_data_forward": {
                "note": "real IERS finals2000A → dUT1(h) → L19 lever arm → Moon-frame position. Uses NO OD covariance.",
                "rows": data_rows,
                "final_floor_position_m": m.final_floor.as_ref().map(|r| tagged(json!(r.moon_position_m), Tag::Validated)),
                "far_horizon_position_m": m.far_horizon.as_ref().map(|r| tagged(json!(r.moon_position_m), Tag::Validated)),
                "far_horizon_days": m.far_horizon.as_ref().map(|r| r.horizon_days),
            },
            "road_b_modelled_od": {
                "note": "representative lunar-relay OD covariance propagated through the latency. Uses NO EOP data.",
                "pos_sigma_m": tagged(json!(m.modelled_od.pos_sigma_m), Tag::Modelled),
                "vel_sigma_mps": tagged(json!(m.modelled_od.vel_sigma_mps), Tag::Modelled),
                "latency_s": m.modelled_od.latency_s,
                "postproc_position_m": tagged(json!(m.modelled_od.postproc_pos_m), Tag::Modelled),
                "realtime_position_m": tagged(json!(m.modelled_od.realtime_pos_m), Tag::Modelled),
                "postproc_light_time_ns": tagged(json!(m.modelled_od.postproc_ns), Tag::Modelled),
                "realtime_light_time_ns": tagged(json!(m.modelled_od.realtime_ns), Tag::Modelled),
            },
            "prediction": {
                "statement": "Two independent roads (real EOP data through the lever arm; a modelled OD covariance) land on the same Moon-frame scale. Because neither computation feeds the other, the agreement is a prediction, not a tautology.",
                "postproc_data_over_model_ratio": m.postproc_ratio,
                "realtime_data_over_model_ratio": m.realtime_ratio,
            },
            "l21_realtime_frame_error_budget": {
                "delta_ut1_ms": m.l21.delta_ut1_ms,
                "delta_xp_mas": m.l21.delta_xp_mas,
                "delta_yp_mas": m.l21.delta_yp_mas,
                "eop_term_m": tagged(json!(m.l21.eop_term_m), Tag::Modelled),
                "ephemeris_term_m": tagged(json!(m.l21.ephemeris_term_m), Tag::Modelled),
                "frame_realization_floor_m": tagged(json!(m.l21.frame_realization_floor_m), Tag::Modelled),
                "total_m": tagged(json!(m.l21.total_m), Tag::Modelled),
                "total_time_ns": tagged(json!(m.l21.total_time_ns), Tag::Modelled),
            },
            "figure": {
                "fig1_two_roads_svg": fig_two_roads_svg(m),
                "fig2_open_frame_eop_svg": m.open_svg,
            },
            "report_html": report_html(self, m),
            "csv": self.csv_string(m),
            "csv_filename": "realtime_frame_eop.csv",
        })
    }

    fn summary(&self, m: &StudyModel) -> String {
        let ff = m
            .final_floor
            .as_ref()
            .map(|r| r.moon_position_m)
            .unwrap_or(0.0);
        let far = m
            .far_horizon
            .as_ref()
            .map(|r| (r.horizon_days, r.moon_position_m))
            .unwrap_or((0.0, 0.0));
        format!(
            "Kshana Pro data-forward frame/EOP decomposition (L49/L50) | ROAD A (real data → lever arm): \
             final-floor {ff:.2} m, {fd:.0}-day {frm:.2} m [Validated] | ROAD B (modelled OD): post-proc \
             {pp:.2} m, real-time {rt:.1} m [Modelled] | prediction (data/model): post-proc {ppr}, real-time {rtr} | \
             agreement is a PREDICTION, not a tautology | CSV realtime_frame_eop.csv (byte-stable)",
            fd = far.0,
            frm = far.1,
            pp = m.modelled_od.postproc_pos_m,
            rt = m.modelled_od.realtime_pos_m,
            ppr = m
                .postproc_ratio
                .map(|r| format!("{r:.2}x"))
                .unwrap_or_else(|| "n/a".into()),
            rtr = m
                .realtime_ratio
                .map(|r| format!("{r:.2}x"))
                .unwrap_or_else(|| "n/a".into()),
        )
    }

    /// Render the branded PDF deliverable (self-contained `%PDF-…`).
    fn to_pdf(&self, m: &StudyModel) -> Result<Vec<u8>, String> {
        let mut pdf = StudyPdf::new();
        pdf.cover(
            "Real-Time Lunar Frame / EOP — A Data-Forward Decomposition",
            "P4 / L49+L50 — answering the 'this is circular' objection",
        );
        pdf.modelled_banner(&[
            "The data-forward Moon positions are real IERS data through a closed-form lever",
            "arm (Validated); the OD-covariance magnitudes and floor are Modelled allocations.",
        ]);

        pdf.heading("Executive Summary");
        pdf.paragraph(&self.summary(m));
        pdf.paragraph(NARRATIVE);

        pdf.heading("Provenance & Traceability");
        pdf.kv("Overlay", &format!("kshana-pro {}", crate::version()));
        pdf.kv("EOP source", &m.eop_source);
        pdf.kv("Scenario hash", &m.scenario_hash);
        pdf.kv(
            "Lever arm",
            &format!(
                "{:.3} m per s of UT1 (omega_earth {:.6e} rad/s)",
                LEVER_M_PER_S, OMEGA_EARTH_RAD_S
            ),
        );

        pdf.heading("Road A - data-forward (real IERS finals2000A -> lever arm)");
        pdf.header_row(&[
            (0.0, "Horizon"),
            (40.0, "n"),
            (58.0, "dUT1 rms (ms)"),
            (105.0, "Moon pos (m)"),
            (140.0, "Prov."),
        ]);
        for r in &m.data_rows {
            let n = r.n.to_string();
            let ut1 = format!("{:.4}", r.ut1_rms_ms);
            let pos = format!("{:.3}", r.moon_position_m);
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: &r.label,
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 40.0,
                    text: &n,
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 58.0,
                    text: &ut1,
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 105.0,
                    text: &pos,
                    bold: true,
                    colour: INK,
                },
                Cell {
                    x_mm: 140.0,
                    text: "Validated",
                    bold: true,
                    colour: "#2f6f63",
                },
            ]);
        }

        pdf.heading("Road B - modelled OD-covariance budget (no EOP data)");
        pdf.kv(
            "Post-processed",
            &format!(
                "{:.3} m  ({:.3} ns)",
                m.modelled_od.postproc_pos_m, m.modelled_od.postproc_ns
            ),
        );
        pdf.kv(
            "Real-time (1 h latency)",
            &format!(
                "{:.2} m  ({:.2} ns)",
                m.modelled_od.realtime_pos_m, m.modelled_od.realtime_ns
            ),
        );

        pdf.heading("The prediction");
        pdf.paragraph(
            "Two independent roads land on the same Moon-frame scale: the real EOP data through the \
             lever arm (Road A) and a modelled OD covariance (Road B). Neither computation feeds the \
             other - Road A uses no OD covariance and Road B uses no EOP data - so the agreement is a \
             prediction, not a tautology.",
        );
        if let Some(r) = m.postproc_ratio {
            pdf.kv("Post-processed data/model", &format!("{r:.3}x"));
        }
        if let Some(r) = m.realtime_ratio {
            pdf.kv("Real-time data/model", &format!("{r:.3}x"));
        }

        pdf.heading("Caveats (read first)");
        pdf.paragraph(
            "- The data-forward positions are Validated (real IERS finals2000A + the closed-form L19 \
             lever arm). The OD-covariance magnitudes and the frame-realisation floor are Modelled \
             representative allocations; the multi-day predictor is persistence, not IERS Bulletin A.",
        );
        pdf.paragraph("- Not a certified real-time frame product.");

        let footer =
            "Ashforde OU  |  Kshana Pro  |  MODELLED engineering study - not a certification";
        pdf.render(
            "Kshana Pro - data-forward frame/EOP decomposition (L49/L50)",
            footer,
        )
    }

    /// Write the L49/L50 paper artifacts (committed CSV + figure SVGs + HTML report +
    /// branded PDF) into `papers/realtime-frame-eop/` (a NEW dir — the founder's other
    /// paper dirs are untouched). Invoked out-of-band, never from `run`.
    pub fn emit_paper_artifacts(&self) -> std::io::Result<std::path::PathBuf> {
        let m = self
            .compute()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("papers/realtime-frame-eop");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("realtime_frame_eop.csv"), self.csv_string(&m))?;
        std::fs::write(dir.join("fig1_two_roads.svg"), fig_two_roads_svg(&m))?;
        std::fs::write(dir.join("fig2_frame_eop.svg"), &m.open_svg)?;
        std::fs::write(dir.join("report.html"), report_html(self, &m))?;
        let pdf = self.to_pdf(&m).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("report.pdf"), pdf)?;
        Ok(dir)
    }
}

impl Scenario for RealtimeFrameEopStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let doc = self.json(&m).to_string();
        let svg = fig_two_roads_svg(&m);
        let summary = self.summary(&m);
        Ok(RunOutput {
            json: doc,
            svg,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for RealtimeFrameEopStudyScenario {
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

// ── Figure — "two roads meet" grouped bars ───────────────────────────────────

fn fig_two_roads_svg(m: &StudyModel) -> String {
    let (w, h) = (720.0_f64, 420.0_f64);
    let (ml, _mr, mt, mb) = (66.0_f64, 30.0_f64, 74.0_f64, 62.0_f64);
    let data_pp = m
        .final_floor
        .as_ref()
        .map(|r| r.moon_position_m)
        .unwrap_or(0.0);
    let data_rt = m
        .far_horizon
        .as_ref()
        .map(|r| r.moon_position_m)
        .unwrap_or(0.0);
    let groups = [
        ("post-processed", data_pp, m.modelled_od.postproc_pos_m),
        ("real-time", data_rt, m.modelled_od.realtime_pos_m),
    ];
    let vmax = groups
        .iter()
        .flat_map(|(_, a, b)| [*a, *b])
        .fold(1.0_f64, f64::max)
        * 1.18;
    let plot_h = h - mt - mb;
    let py = |v: f64| mt + plot_h * (1.0 - (v / vmax).clamp(0.0, 1.0));
    let group_w = 260.0_f64;
    let bar_w = 70.0_f64;

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"28\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"18\" font-weight=\"bold\">Figure 1 — two independent roads meet</text>");
    s += &format!("<text x=\"20\" y=\"46\" fill=\"{IVORY}\">Moon-frame position: real EOP data through the lever arm vs the modelled OD budget</text>");
    s += &format!("<text x=\"20\" y=\"62\" fill=\"{BRASS_MUTED}\" font-size=\"10\">agreement is a PREDICTION, not a tautology &#8226; data-forward <tspan fill=\"#2dd4bf\">[Validated]</tspan>, OD budget <tspan fill=\"{BRASS}\">[Modelled]</tspan></text>");
    let yb = h - mb;
    s += &format!(
        "<line x1=\"{ml}\" y1=\"{yb}\" x2=\"{:.1}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\"/>",
        w - 20.0
    );
    // y ticks (m).
    for k in 0..=4 {
        let v = vmax * k as f64 / 4.0;
        let y = py(v);
        s += &format!(
            "<line x1=\"{ml}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"#1c2637\"/>",
            w - 20.0
        );
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\" font-size=\"9\">{v:.1} m</text>", ml - 6.0, y + 3.0);
    }
    for (gi, (label, a, b)) in groups.iter().enumerate() {
        let gx = ml + 30.0 + gi as f64 * group_w;
        // Road A — data-forward (teal, Validated).
        let ya = py(*a);
        s += &format!("<rect x=\"{gx:.1}\" y=\"{ya:.1}\" width=\"{bar_w}\" height=\"{:.1}\" fill=\"#2dd4bf\"/>", yb - ya);
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\" text-anchor=\"middle\">{a:.2} m</text>", gx + bar_w / 2.0, ya - 4.0);
        // Road B — modelled OD (brass, Modelled).
        let bx = gx + bar_w + 12.0;
        let yv = py(*b);
        s += &format!("<rect x=\"{bx:.1}\" y=\"{yv:.1}\" width=\"{bar_w}\" height=\"{:.1}\" fill=\"{BRASS}\"/>", yb - yv);
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"10\" text-anchor=\"middle\">{b:.2} m</text>", bx + bar_w / 2.0, yv - 4.0);
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY_LIGHT}\" font-size=\"12\" text-anchor=\"middle\">{label}</text>", gx + bar_w + 6.0, yb + 18.0);
    }
    // Legend.
    s += &format!(
        "<rect x=\"{:.1}\" y=\"70\" width=\"12\" height=\"12\" fill=\"#2dd4bf\"/>",
        w - 230.0
    );
    s += &format!("<text x=\"{:.1}\" y=\"80\" fill=\"{IVORY}\" font-size=\"10\">Road A · real data → lever arm</text>", w - 214.0);
    s += &format!(
        "<rect x=\"{:.1}\" y=\"70\" width=\"12\" height=\"12\" fill=\"{BRASS}\"/>",
        w - 230.0 + 0.0
    );
    // Second legend row.
    s += &format!(
        "<rect x=\"{:.1}\" y=\"88\" width=\"12\" height=\"12\" fill=\"{BRASS}\"/>",
        w - 230.0
    );
    s += &format!("<text x=\"{:.1}\" y=\"98\" fill=\"{IVORY}\" font-size=\"10\">Road B · modelled OD budget</text>", w - 214.0);
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">frame-error regime</text>", ml + (w - ml - 20.0) / 2.0, h - 12.0);
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

fn report_html(_scn: &RealtimeFrameEopStudyScenario, m: &StudyModel) -> String {
    // Every Kshana Pro document is an Ashforde document: the shared furniture of
    // crate::report_brand goes around the renderer's own page, which is unchanged.
    let body = report_html_unbranded(_scn, m);
    let id = crate::report_brand::content_id(&body);
    crate::report_brand::finish_html(
        &body,
        &crate::report_brand::DocMark::new("Real-time frame and Earth-orientation study", id),
    )
}

fn report_html_unbranded(_scn: &RealtimeFrameEopStudyScenario, m: &StudyModel) -> String {
    let data_rows: String = m
        .data_rows
        .iter()
        .map(|r| {
            format!(
                "<tr>{}{}{}{}{}</tr>",
                td(&r.label),
                td(&r.n.to_string()),
                td(&format!("{:.4}", r.ut1_rms_ms)),
                td(&format!("{:.3}", r.moon_position_m)),
                td(&format!(
                    "<span style=\"color:#2dd4bf;font-weight:600\">{}</span>",
                    Tag::Validated.as_str()
                )),
            )
        })
        .collect();
    let ppr = m
        .postproc_ratio
        .map(|r| format!("{r:.2}×"))
        .unwrap_or_else(|| "n/a".into());
    let rtr = m
        .realtime_ratio
        .map(|r| format!("{r:.2}×"))
        .unwrap_or_else(|| "n/a".into());

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — Data-Forward Frame/EOP Decomposition (L49/L50)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:940px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Answering \"this is circular\"</h2>\
         <p style=\"border-left:3px solid {BRASS};padding-left:12px\"><em>{narrative}</em></p>\
         {fig1}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Road A — data-forward (real IERS finals2000A → lever arm)</h2>\
         <table style=\"border-collapse:collapse;width:100%\">\
         <tr>{h_hz}{h_n}{h_ut1}{h_pos}{h_prov}</tr>{data_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Road B — modelled OD-covariance budget</h2>\
         <p>Post-processed <strong>{pp:.2} m</strong> ({ppns:.2} ns), real-time (1 h latency) \
         <strong>{rt:.2} m</strong> ({rtns:.2} ns) <span style=\"color:{BRASS}\">[Modelled]</span>. \
         This road propagates a representative lunar-relay OD covariance through the latency and uses \
         <strong>no EOP data</strong>.</p>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">The prediction</h2>\
         <p>The data-forward final floor lands at <strong>{ppr}</strong> the modelled post-processed \
         figure, and the data-forward multi-day horizon lands at <strong>{rtr}</strong> the modelled \
         real-time figure. Because Road A carries no OD covariance and Road B carries no EOP data, this \
         agreement is a <strong>prediction</strong>, not a tautology.</p>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">L21 real-time frame-error budget (RSS)</h2>\
         <p>EOP {eop:.2} m &#8853; ephemeris {eph:.2} m &#8853; floor {floor:.2} m = total \
         <strong>{tot:.2} m</strong> ({totns:.1} ns) <span style=\"color:{BRASS}\">[Modelled]</span>.</p>\
         <p style=\"font-size:12px;opacity:.85\">The full decomposition is committed to \
         <code>papers/realtime-frame-eop/realtime_frame_eop.csv</code> (byte-stable). The two-panel P4 \
         chart is embedded above as Figure 2 in the JSON result.</p>\
         </body></html>",
        cover = cover_html(
            "Real-Time Lunar Frame / EOP — Data-Forward",
            "P4 / L49+L50 — a prediction, not a tautology"
        ),
        banner = modelled_banner_html(
            "The data-forward Moon positions are real IERS data through the closed-form L19 lever arm \
             (Validated); the OD-covariance magnitudes and the frame-realisation floor are Modelled \
             representative allocations."
        ),
        narrative = NARRATIVE,
        fig1 = fig_two_roads_svg(m),
        pp = m.modelled_od.postproc_pos_m,
        ppns = m.modelled_od.postproc_ns,
        rt = m.modelled_od.realtime_pos_m,
        rtns = m.modelled_od.realtime_ns,
        eop = m.l21.eop_term_m,
        eph = m.l21.ephemeris_term_m,
        floor = m.l21.frame_realization_floor_m,
        tot = m.l21.total_m,
        totns = m.l21.total_time_ns,
        h_hz = th("Horizon"),
        h_n = th("n"),
        h_ut1 = th("dUT1 rms (ms)"),
        h_pos = th("Moon pos (m)"),
        h_prov = th("Provenance"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::frame_eop::ut1_error_to_lunar;

    /// Speed of light (m/s) — for the light-time oracle.
    const C_M_S: f64 = 299_792_458.0;

    fn model() -> StudyModel {
        RealtimeFrameEopStudyScenario::default().compute().unwrap()
    }

    #[test]
    fn runs_and_carries_label_and_narrative() {
        let out = RealtimeFrameEopStudyScenario::default().run().unwrap();
        let v: Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["kind"], RealtimeFrameEopStudyScenario::KIND);
        let label = v["label"].as_str().unwrap();
        assert!(label.contains("VALIDATED"));
        assert!(label.contains("MODELLED"));
        assert!(v["narrative"].as_str().unwrap().contains("not a tautology"));
        assert!(out.summary.contains("PREDICTION"));
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let scn = RealtimeFrameEopStudyScenario::default();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn data_forward_positions_equal_l19_image_of_measured_ut1() {
        // ORACLE: each data-forward Moon position is exactly the L19 lever-arm image of
        // the measured dUT1 RMS — the whole data-forward road, checked point by point.
        let m = model();
        assert!(!m.data_rows.is_empty());
        for r in &m.data_rows {
            let expect = ut1_error_to_lunar(r.ut1_rms_ms * 1e-3).0;
            assert!(
                (r.moon_position_m - expect).abs() < 1e-9,
                "{}: moon {} != L19 image {}",
                r.label,
                r.moon_position_m,
                expect
            );
            // Light-time is the position / c.
            assert!((r.moon_light_time_ns - r.moon_position_m / C_M_S * 1e9).abs() < 1e-6);
        }
    }

    #[test]
    fn modelled_od_reproduces_p4_magnitudes() {
        // ORACLE: the representative OD covariance propagated through 1 h of latency.
        // Hand-computed here from the published constants rather than re-run through the
        // same expression, so this cannot pass by sharing the code's own arithmetic:
        //   √(0.27² + (3600 · 0.004)²) = √(0.0729 + 207.36) = 14.402531… m
        //
        // The band used to be a slack (13.0..17.0) around a "~15 m" headline. That slack
        // is exactly why a real change slipped through: the open core replaced a velocity
        // 1σ that had been back-solved to land on a round 15 m with an honestly-chosen
        // 4 mm/s, the figure moved to 14.4 m, and this test stayed green. Pinned tight.
        let m = model();
        assert!((m.modelled_od.postproc_pos_m - 0.27).abs() < 1e-6);
        assert!(
            (m.modelled_od.realtime_pos_m - 14.402_531).abs() < 1e-3,
            "real-time {} is not the 14.4025 m the representative covariance implies",
            m.modelled_od.realtime_pos_m
        );
    }

    /// INVARIANT: the committed paper CSV in `papers/realtime-frame-eop/` is exactly what
    /// the code regenerates today.
    ///
    /// The optical/RF pack has carried this guard from the start and it caught its own
    /// drift the moment the engine's climatology was re-anchored. This pack had no such
    /// guard, so when the open core replaced the back-solved velocity 1σ the committed
    /// artifact silently kept the superseded 15.0 m column — stale, and nothing failed.
    #[test]
    fn committed_csv_matches_regeneration() {
        let scn = RealtimeFrameEopStudyScenario::default();
        let m = scn.compute().unwrap();
        let committed = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/papers/realtime-frame-eop/realtime_frame_eop.csv"
        ))
        .expect("committed realtime_frame_eop.csv must exist (run emit_paper_artifacts)");
        assert_eq!(
            scn.csv_string(&m),
            committed,
            "committed realtime_frame_eop.csv drifted from regeneration — rerun \
             `cargo test --lib -- --ignored emit_paper_artifacts`"
        );
    }

    #[test]
    fn independence_od_perturbation_leaves_data_forward_unchanged() {
        // The data-forward road uses NO OD covariance: changing the OD velocity 1σ must
        // move the modelled real-time figure but NOT the data-forward positions. This is
        // the machine-checkable meaning of "prediction, not tautology".
        let base = RealtimeFrameEopStudyScenario::default().compute().unwrap();
        let perturbed = RealtimeFrameEopStudyScenario {
            ephemeris_vel_sigma_mps: REPRESENTATIVE_VEL_SIGMA_MPS * 2.0,
            ..Default::default()
        }
        .compute()
        .unwrap();
        // Road B (modelled OD real-time) MOVED.
        assert!(
            (base.modelled_od.realtime_pos_m - perturbed.modelled_od.realtime_pos_m).abs() > 1.0,
            "OD budget should react to the covariance change"
        );
        // Road A (data-forward positions) did NOT move.
        assert_eq!(base.data_rows.len(), perturbed.data_rows.len());
        for (a, b) in base.data_rows.iter().zip(perturbed.data_rows.iter()) {
            assert!(
                (a.moon_position_m - b.moon_position_m).abs() < 1e-12,
                "data-forward {} moved with the OD covariance",
                a.label
            );
        }
    }

    #[test]
    fn independence_horizon_perturbation_leaves_od_budget_unchanged() {
        // The modelled OD road uses NO EOP data: changing the EOP horizons must NOT move
        // the modelled OD budget figures.
        let base = RealtimeFrameEopStudyScenario::default().compute().unwrap();
        let perturbed = RealtimeFrameEopStudyScenario {
            horizons_days: vec![1, 2],
            ..Default::default()
        }
        .compute()
        .unwrap();
        assert!(
            (base.modelled_od.postproc_pos_m - perturbed.modelled_od.postproc_pos_m).abs() < 1e-12
        );
        assert!(
            (base.modelled_od.realtime_pos_m - perturbed.modelled_od.realtime_pos_m).abs() < 1e-12
        );
    }

    #[test]
    fn csv_is_well_formed_and_deterministic() {
        let scn = RealtimeFrameEopStudyScenario::default();
        let m = scn.compute().unwrap();
        let a = scn.csv_string(&m);
        assert_eq!(a, scn.csv_string(&m));
        assert!(a.contains(
            "section,label,horizon_days,n,ut1_ms,moon_position_m,light_time_ns,provenance"
        ));
        assert!(a.contains("data_forward,final,"));
        assert!(a.contains("modelled_od,post-processed,"));
        assert!(a.contains("modelled_od,real-time,"));
        assert!(a.contains("scenario_hash="));
        assert!(a.contains("PREDICTION, not a tautology"));
    }

    #[test]
    fn svg_and_pdf_are_well_formed() {
        let scn = RealtimeFrameEopStudyScenario::default();
        let m = scn.compute().unwrap();
        let svg = fig_two_roads_svg(&m);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(svg, fig_two_roads_svg(&m));
        let pdf = scn.to_pdf(&m).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    #[ignore = "writes the committed paper artifacts to papers/realtime-frame-eop/; run with --ignored"]
    fn emit_paper_artifacts() {
        let dir = RealtimeFrameEopStudyScenario::default()
            .emit_paper_artifacts()
            .unwrap();
        assert!(dir.join("realtime_frame_eop.csv").exists());
        assert!(dir.join("fig1_two_roads.svg").exists());
        assert!(dir.join("fig2_frame_eop.svg").exists());
        assert!(dir.join("report.html").exists());
        assert!(dir.join("report.pdf").exists());
    }

    #[test]
    fn far_horizon_reaches_real_time_scale() {
        // Honest sanity: the data-forward multi-day position lands within an order of
        // magnitude of the modelled real-time figure (the prediction), and the final
        // floor lands within an order of magnitude of the post-processed figure.
        let m = model();
        let ppr = m.postproc_ratio.expect("post-processed ratio present");
        let rtr = m.realtime_ratio.expect("real-time ratio present");
        assert!((0.05..20.0).contains(&ppr), "post-proc ratio {ppr}");
        assert!((0.05..20.0).contains(&rtr), "real-time ratio {rtr}");
    }
}
