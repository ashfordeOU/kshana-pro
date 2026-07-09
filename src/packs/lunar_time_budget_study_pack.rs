// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L48 — `pro-lunar-time-budget-study`
//!
//! A Kshana Pro *study* pack over the open engine's P3 Coordinated Lunar Time (LTC)
//! end-to-end time-error budget assembler ([`kshana::lunar_time_budget`]). It answers
//! the single-τ artifact objection — *"you picked the averaging time that flatters
//! your clock"* — by assembling the **seven** LTC error terms as time-error curves
//! `x_i(τ)` across a whole averaging-time grid, root-summing them into
//! `x_Σ(τ) = √(Σ x_i²)`, and locating the **clock-vs-frame crossover** τ at which the
//! growing clock term overtakes the constant real-time frame-realisation term.
//!
//! On top of the open assembler this pack adds the commercial study surface: a
//! byte-stable committed CSV of the budget curves, a two-panel branded Figure 1 (the
//! Allan-style `x_i(τ)` curves + RSS, and a sorted per-term contribution bar chart at
//! a representative τ against the 1 ns floor), a sorted seven-row provenance table, a
//! branded HTML report and a branded PDF.
//!
//! ## Validated vs Modelled (honesty firewall)
//! * **Validated.** The clock row reproduces the published one-day clock spec
//!   ([`kshana::clock_specs`], calibrated to <0.1 % of the cited `x(1 day)`), and the
//!   clock-vs-frame **crossover identity** is a closed form checked against an
//!   analytic oracle (`τ*·floor = δr/c` for a flicker-floor clock, `τ* = 2(δr/c)²/h₀`
//!   for a white-FM clock) to machine precision.
//! * **Modelled.** The RF/optical-link, frame-realisation, relativistic-residual,
//!   ephemeris and measurement floor **magnitudes** are documented budget
//!   allocations (caller-overridable defaults), not measurements; the RSS total and
//!   the crossover τ *value* inherit that Modelled frame allocation. An engineering
//!   study, not a certification.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::clock_specs::{sigma_y, LunarClock};
use kshana::lunar_time_budget::{lunar_time_budget, BudgetParams, LunarTimeBudget};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::packs::study_pdf::{Cell, StudyPdf};
use crate::report_brand::{
    cover_html, modelled_banner_html, BRASS, BRASS_MUTED, FONT_BODY, FONT_HEAD, IVORY, IVORY_LIGHT,
    NAVY,
};

/// The 1 ns per-term budget floor line drawn on the figure and flagged in the table.
const ONE_NS_S: f64 = 1.0e-9;

/// The honesty label carried on the result document (contains both tier words).
const LABEL: &str = "Coordinated Lunar Time (LTC) end-to-end time-error budget. \
VALIDATED: the clock row reproduces the published one-day clock spec (kshana::clock_specs), and the \
clock-vs-frame crossover is a closed form checked against an analytic oracle to machine precision. \
MODELLED: the RF/optical-link, frame-realisation, relativistic-residual, ephemeris and measurement \
floor magnitudes are documented budget allocations (caller-overridable), and the RSS total and the \
crossover tau value inherit that Modelled frame allocation. An engineering study, not a certification.";

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
            Tag::Validated => "#2dd4bf",
            Tag::Modelled => BRASS,
        }
    }
}

/// Wrap a JSON value with its provenance tag, matching the beacon-study convention.
fn tagged(v: Value, t: Tag) -> Value {
    json!({ "value": v, "provenance": t.as_str() })
}

// ── Scenario ─────────────────────────────────────────────────────────────────

fn d_clock() -> String {
    "passive-h-maser".to_string()
}
fn d_tau_min() -> f64 {
    1.0
}
fn d_tau_max() -> f64 {
    1.0e7
}
fn d_points_per_decade() -> usize {
    8
}
fn d_representative_tau() -> f64 {
    86_400.0
}
fn d_rf_link_floor() -> f64 {
    1.0e-9
}
fn d_optical_link_floor() -> f64 {
    1.0e-11
}
fn d_frame_pos_error() -> f64 {
    0.3
}
fn d_relativistic_residual() -> f64 {
    5.0e-11
}
fn d_ephemeris() -> f64 {
    5.0e-10
}
fn d_measurement_1s() -> f64 {
    1.0e-9
}

/// A `pro-lunar-time-budget-study` scenario. Every field is optional; empty TOML
/// reproduces the passive-H-maser baseline over the 1 s … 1e7 s grid.
#[derive(Debug, Clone, Deserialize)]
pub struct LunarTimeBudgetStudyScenario {
    /// The on-board clock class: `optical-master` | `passive-h-maser` | `rafs` |
    /// `mini-rafs` (aliases `optical`, `phm`, `minirafs` accepted).
    #[serde(default = "d_clock")]
    pub clock: String,
    /// Averaging-time grid lower bound (s).
    #[serde(default = "d_tau_min")]
    pub tau_min_s: f64,
    /// Averaging-time grid upper bound (s).
    #[serde(default = "d_tau_max")]
    pub tau_max_s: f64,
    /// Grid density (points per decade).
    #[serde(default = "d_points_per_decade")]
    pub points_per_decade: usize,
    /// The τ at which the sorted per-term contribution bar chart / table is drawn (s).
    #[serde(default = "d_representative_tau")]
    pub representative_tau_s: f64,
    /// RF one-way ranging/timing link floor (s).
    #[serde(default = "d_rf_link_floor")]
    pub rf_link_floor_s: f64,
    /// Optical two-way ranging link floor (s).
    #[serde(default = "d_optical_link_floor")]
    pub optical_link_floor_s: f64,
    /// Lunar reference-frame position-realisation error δr (m); frame term is δr/c.
    #[serde(default = "d_frame_pos_error")]
    pub frame_pos_error_m: f64,
    /// Relativistic modelling residual after the LTC−TT rate model (s).
    #[serde(default = "d_relativistic_residual")]
    pub relativistic_residual_s: f64,
    /// Ephemeris / ground-station position timing term (s).
    #[serde(default = "d_ephemeris")]
    pub ephemeris_s: f64,
    /// Measurement-noise time error at τ = 1 s (s); averages down as τ^{-1/2}.
    #[serde(default = "d_measurement_1s")]
    pub measurement_1s_s: f64,
}

impl Default for LunarTimeBudgetStudyScenario {
    fn default() -> Self {
        Self {
            clock: d_clock(),
            tau_min_s: d_tau_min(),
            tau_max_s: d_tau_max(),
            points_per_decade: d_points_per_decade(),
            representative_tau_s: d_representative_tau(),
            rf_link_floor_s: d_rf_link_floor(),
            optical_link_floor_s: d_optical_link_floor(),
            frame_pos_error_m: d_frame_pos_error(),
            relativistic_residual_s: d_relativistic_residual(),
            ephemeris_s: d_ephemeris(),
            measurement_1s_s: d_measurement_1s(),
        }
    }
}

/// One sorted contribution at the representative τ, with its provenance.
struct SortedTerm {
    name: String,
    x_s: f64,
    tag: Tag,
}

/// Everything the study derives, computed once and shared by JSON / CSV / figure /
/// report / PDF.
struct StudyModel {
    clock_label: String,
    taus: Vec<f64>,
    budget: LunarTimeBudget,
    representative_tau_s: f64,
    sorted_terms: Vec<SortedTerm>,
    x_sigma_at_rep_s: f64,
    x_sigma_at_1day_s: f64,
    /// Analytic closed-form crossover τ (the Validated oracle).
    crossover_tau_analytic_s: f64,
    /// |numeric − analytic| / analytic — the closed-form residual (≈ 0, Validated).
    crossover_closed_form_residual: f64,
    regime: &'static str,
    scenario_hash: String,
}

/// Parse a clock string to a [`LunarClock`], accepting the canonical names and a few
/// friendly aliases.
fn parse_clock(s: &str) -> Result<LunarClock, KshanaError> {
    match s
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '_'], "-")
        .as_str()
    {
        "optical-master" | "optical" | "optical-lattice" => Ok(LunarClock::OpticalMaster),
        "passive-h-maser" | "phm" | "h-maser" | "hmaser" => Ok(LunarClock::Phm),
        "rafs" | "rb" | "rubidium" => Ok(LunarClock::Rafs),
        "mini-rafs" | "minirafs" | "mini-rb" => Ok(LunarClock::MiniRafs),
        other => Err(KshanaError::InvalidInput(format!(
            "unknown clock '{other}' (expected optical-master|passive-h-maser|rafs|mini-rafs)"
        ))),
    }
}

/// A log-spaced averaging-time grid from `min` to `max` with `per_decade` points per
/// decade (inclusive of both ends).
fn tau_grid(min: f64, max: f64, per_decade: usize) -> Vec<f64> {
    let per = per_decade.max(1) as f64;
    let decades = (max / min).log10().max(0.0);
    let n = (decades * per).round() as i64 + 1;
    (0..n).map(|k| min * 10f64.powf(k as f64 / per)).collect()
}

/// The provenance of a budget term, keyed on its name: the clock row is Validated
/// (published spec), every floor / measurement term is a Modelled allocation.
fn term_tag(name: &str) -> Tag {
    if name.starts_with("clock:") {
        Tag::Validated
    } else {
        Tag::Modelled
    }
}

/// A short, stable column key for a budget term (fixed CSV column names).
fn term_key(name: &str) -> &'static str {
    if name.starts_with("clock:") {
        "x_clock_s"
    } else {
        match name {
            "rf-link-floor" => "x_rf_link_floor_s",
            "optical-link-floor" => "x_optical_link_floor_s",
            "frame-realisation" => "x_frame_realisation_s",
            "relativistic-residual" => "x_relativistic_residual_s",
            "ephemeris" => "x_ephemeris_s",
            "measurement" => "x_measurement_s",
            _ => "x_other_s",
        }
    }
}

impl LunarTimeBudgetStudyScenario {
    pub const KIND: &'static str = "pro-lunar-time-budget-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P3/L48 study: assemble the open LTC end-to-end time-error budget (kshana::lunar_time_budget) — the seven error terms as time-error curves x_i(tau) over an averaging-time grid, root-summed into x_sigma(tau), and the clock-vs-frame crossover tau (below it the budget is frame-limited, above it clock-limited) — the honest answer to the single-tau artifact. Emits a byte-stable lunar_time_budget.csv, a two-panel Figure 1 (Allan-style x_i(tau) curves + RSS, and a sorted per-term contribution bar chart at a representative tau vs the 1 ns floor), a sorted seven-row provenance table, a branded HTML report and PDF. The clock row is Validated (published one-day spec) and the crossover is a closed-form check; the link/frame/relativistic/ephemeris/measurement floor magnitudes are Modelled allocations. An engineering study, not a certification.",
            required_fields: &[],
            optional_fields: &[
                "clock",
                "tau_min_s",
                "tau_max_s",
                "points_per_decade",
                "representative_tau_s",
                "rf_link_floor_s",
                "optical_link_floor_s",
                "frame_pos_error_m",
                "relativistic_residual_s",
                "ephemeris_s",
                "measurement_1s_s",
            ],
        }
    }

    fn budget_params(&self, clock: LunarClock) -> BudgetParams {
        BudgetParams {
            clock,
            rf_link_floor_s: self.rf_link_floor_s,
            optical_link_floor_s: self.optical_link_floor_s,
            frame_pos_error_m: self.frame_pos_error_m,
            relativistic_residual_s: self.relativistic_residual_s,
            ephemeris_s: self.ephemeris_s,
            measurement_1s_s: self.measurement_1s_s,
        }
    }

    /// A deterministic 12-hex scenario fingerprint over the canonical config.
    fn compute_hash(&self, clock: LunarClock) -> String {
        let canonical = format!(
            "lunar-time-budget-study|clock={}|tau=[{:.6e},{:.6e}]/{}|rep={:.6e}|rf={:.6e}|opt={:.6e}|frame={:.6e}|rel={:.6e}|eph={:.6e}|meas={:.6e}",
            clock.name(),
            self.tau_min_s,
            self.tau_max_s,
            self.points_per_decade,
            self.representative_tau_s,
            self.rf_link_floor_s,
            self.optical_link_floor_s,
            self.frame_pos_error_m,
            self.relativistic_residual_s,
            self.ephemeris_s,
            self.measurement_1s_s,
        );
        let mut h = Sha256::new();
        h.update(canonical.as_bytes());
        hex::encode(h.finalize()).chars().take(12).collect()
    }

    fn compute(&self) -> Result<StudyModel, KshanaError> {
        if !(self.tau_min_s.is_finite() && self.tau_max_s.is_finite())
            || self.tau_min_s <= 0.0
            || self.tau_max_s <= self.tau_min_s
        {
            return Err(KshanaError::InvalidInput(format!(
                "tau grid must satisfy 0 < tau_min_s < tau_max_s (got {} .. {})",
                self.tau_min_s, self.tau_max_s
            )));
        }
        if self.frame_pos_error_m <= 0.0 || !self.frame_pos_error_m.is_finite() {
            return Err(KshanaError::InvalidInput(
                "frame_pos_error_m must be finite and positive".into(),
            ));
        }
        let clock = parse_clock(&self.clock)?;
        let params = self.budget_params(clock);
        let taus = tau_grid(self.tau_min_s, self.tau_max_s, self.points_per_decade);
        let budget = lunar_time_budget(&params, &taus);

        // Per-term contributions at the representative τ (a single-point budget so the
        // values are exactly the term curves evaluated at that τ).
        let rep = self.representative_tau_s;
        let at_rep = lunar_time_budget(&params, &[rep]);
        let mut sorted_terms: Vec<SortedTerm> = at_rep
            .terms
            .iter()
            .map(|t| SortedTerm {
                name: t.name.clone(),
                x_s: t.x_s[0],
                tag: term_tag(&t.name),
            })
            .collect();
        sorted_terms.sort_by(|a, b| {
            b.x_s
                .partial_cmp(&a.x_s)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let x_sigma_at_rep_s = at_rep.x_sigma_s[0];
        let x_sigma_at_1day_s = lunar_time_budget(&params, &[86_400.0]).x_sigma_s[0];

        // The Validated closed-form crossover oracle.
        let frame_term = params.frame_term_s();
        let (crossover_tau_analytic_s, regime) = if clock.is_white_fm_limited() {
            // white FM: x_clock = √(h₀ τ / 2) = δr/c ⇒ τ* = 2 (δr/c)² / h₀.
            let h0 = clock.powerlaw().h_0;
            (2.0 * frame_term * frame_term / h0, "white-fm")
        } else {
            // flicker-FM floor: x_clock = floor·τ = δr/c ⇒ τ* = (δr/c) / floor.
            let floor = sigma_y(&clock.powerlaw(), 1.0);
            (frame_term / floor, "flicker-floor")
        };
        let crossover_closed_form_residual =
            (budget.crossover_tau_s - crossover_tau_analytic_s).abs() / crossover_tau_analytic_s;

        let scenario_hash = self.compute_hash(clock);

        Ok(StudyModel {
            clock_label: clock.name().to_string(),
            taus,
            budget,
            representative_tau_s: rep,
            sorted_terms,
            x_sigma_at_rep_s,
            x_sigma_at_1day_s,
            crossover_tau_analytic_s,
            crossover_closed_form_residual,
            regime,
            scenario_hash,
        })
    }

    /// The committed, byte-stable `lunar_time_budget.csv`: the budget curves in wide
    /// form (one row per τ, one column per term plus the RSS), preceded by a
    /// provenance comment header. Fixed-precision scientific notation so the bytes are
    /// reproducible across platforms.
    fn csv_string(&self, m: &StudyModel) -> String {
        let mut s = String::new();
        s.push_str(
            "# kshana-pro lunar-time-budget study (L48) — LTC time-error budget curves x_i(tau)\n",
        );
        s.push_str(&format!(
            "# clock={} scenario_hash={} crossover_tau_s={:.6e} crossover_x_s={:.6e} frame_term_s={:.6e} regime={}\n",
            m.clock_label,
            m.scenario_hash,
            m.budget.crossover_tau_s,
            m.budget.crossover_x_s,
            m.budget.frame_term_s,
            m.regime,
        ));
        s.push_str(
            "# provenance: x_clock=Validated (published one-day spec); floors + x_measurement=Modelled allocations; x_sigma=Modelled (RSS aggregate)\n",
        );
        // Header: tau then one column per term (fixed order) then the RSS.
        s.push_str("tau_s");
        for t in &m.budget.terms {
            s.push(',');
            s.push_str(term_key(&t.name));
        }
        s.push_str(",x_sigma_s\n");
        // One row per τ.
        for (i, &tau) in m.budget.tau_s.iter().enumerate() {
            s.push_str(&format!("{tau:.6e}"));
            for t in &m.budget.terms {
                s.push_str(&format!(",{:.6e}", t.x_s[i]));
            }
            s.push_str(&format!(",{:.6e}\n", m.budget.x_sigma_s[i]));
        }
        s
    }

    fn json(&self, m: &StudyModel) -> Value {
        // Curve terms, each carrying its provenance and full x_i(τ).
        let terms: Vec<Value> = m
            .budget
            .terms
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "provenance": term_tag(&t.name).as_str(),
                    "grows_with_tau": t.grows_with_tau,
                    "x_s": t.x_s,
                })
            })
            .collect();
        let sorted: Vec<Value> = m
            .sorted_terms
            .iter()
            .enumerate()
            .map(|(rank, t)| {
                json!({
                    "rank": rank + 1,
                    "term": t.name,
                    "x_s": t.x_s,
                    "x_ns": t.x_s * 1e9,
                    "provenance": t.tag.as_str(),
                    "above_1ns_floor": t.x_s > ONE_NS_S,
                })
            })
            .collect();

        json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "paper": "P3 — Coordinated Lunar Time (LTC) end-to-end time-error budget (L48)",
            "scenario_hash": m.scenario_hash,
            "label": LABEL,
            "provenance_legend": {
                "Validated": "the clock row reproduces the published one-day clock spec (kshana::clock_specs, <0.1% of cited x(1 day)); the clock-vs-frame crossover is a closed form checked against an analytic oracle to machine precision.",
                "Modelled": "the RF/optical-link, frame-realisation, relativistic-residual, ephemeris and measurement floor magnitudes are documented budget allocations (caller-overridable); the RSS total and the crossover tau value inherit that Modelled frame allocation.",
            },
            "clock": m.clock_label,
            "tau_grid": {
                "tau_min_s": self.tau_min_s,
                "tau_max_s": self.tau_max_s,
                "points_per_decade": self.points_per_decade,
                "n_points": m.taus.len(),
            },
            "budget": {
                "tau_s": m.budget.tau_s,
                "terms": terms,
                "x_sigma_s": m.budget.x_sigma_s,
                "frame_term_s": tagged(json!(m.budget.frame_term_s), Tag::Modelled),
                "frame_pos_error_m": tagged(json!(self.frame_pos_error_m), Tag::Modelled),
                "frame_light_time_ns": tagged(json!(m.budget.frame_term_s * 1e9), Tag::Modelled),
                "crossover": {
                    "regime": m.regime,
                    "tau_s": tagged(json!(m.budget.crossover_tau_s), Tag::Modelled),
                    "x_s": tagged(json!(m.budget.crossover_x_s), Tag::Modelled),
                    "tau_analytic_s": tagged(json!(m.crossover_tau_analytic_s), Tag::Validated),
                    "closed_form_residual": tagged(
                        json!(m.crossover_closed_form_residual),
                        Tag::Validated,
                    ),
                },
            },
            "representative_tau_s": m.representative_tau_s,
            "one_ns_floor_s": ONE_NS_S,
            "x_sigma_at_representative_tau_s": m.x_sigma_at_rep_s,
            "x_sigma_at_one_day_ns": m.x_sigma_at_1day_s * 1e9,
            "sorted_terms_at_representative_tau": sorted,
            "figure": { "fig1_budget_svg": fig_budget_svg(self, m) },
            "report_html": budget_report_html(self, m),
            "csv": self.csv_string(m),
            "csv_filename": "lunar_time_budget.csv",
        })
    }

    fn summary(&self, m: &StudyModel) -> String {
        let dom = m.sorted_terms.first();
        format!(
            "Kshana Pro LTC time-error budget (L48) | clock {clock} | 7 terms x_i(tau) over {n} tau | \
             x_sigma(1 day) = {sig:.3} ns | clock-vs-frame crossover tau* = {tx:.3e} s ({regime}) \
             [closed form Validated, residual {res:.1e}] | dominant term at rep tau: {dom} ({domns:.3} ns) | \
             CSV lunar_time_budget.csv (byte-stable)",
            clock = m.clock_label,
            n = m.taus.len(),
            sig = m.x_sigma_at_1day_s * 1e9,
            tx = m.budget.crossover_tau_s,
            regime = m.regime,
            res = m.crossover_closed_form_residual,
            dom = dom.map(|t| t.name.as_str()).unwrap_or("n/a"),
            domns = dom.map(|t| t.x_s * 1e9).unwrap_or(0.0),
        )
    }

    /// Render the branded PDF deliverable (self-contained `%PDF-…`).
    fn to_pdf(&self, m: &StudyModel) -> Result<Vec<u8>, String> {
        let mut pdf = StudyPdf::new();
        pdf.cover(
            "Coordinated Lunar Time — End-to-End Time-Error Budget",
            "P3 / L48 — the seven-term LTC budget and the clock-vs-frame crossover",
        );
        pdf.modelled_banner(&[
            "The clock row reproduces a published one-day spec (Validated); the",
            "link/frame/relativistic/ephemeris floor magnitudes are Modelled allocations.",
        ]);

        pdf.heading("Executive Summary");
        pdf.paragraph(&self.summary(m));
        pdf.paragraph(
            "A single averaging-time (tau) error number invites the objection that the tau was \
             chosen to flatter the clock. This study assembles the seven LTC error terms as \
             time-error curves x_i(tau) across a whole tau grid, root-sums them into x_sigma(tau), \
             and reports the clock-vs-frame crossover tau: below it the budget is frame-limited \
             (the reference-frame realisation dominates), above it clock-limited.",
        );

        pdf.heading("Provenance & Traceability");
        pdf.kv("Overlay", &format!("kshana-pro {}", crate::version()));
        pdf.kv("Clock class", &m.clock_label);
        pdf.kv("Scenario hash", &m.scenario_hash);
        pdf.kv(
            "Crossover tau (s)",
            &format!(
                "{:.6e}  ({}, closed-form residual {:.2e})",
                m.budget.crossover_tau_s, m.regime, m.crossover_closed_form_residual
            ),
        );
        pdf.kv(
            "Frame term",
            &format!(
                "{:.3} ns  (delta_r = {:.3} m)",
                m.budget.frame_term_s * 1e9,
                self.frame_pos_error_m
            ),
        );

        pdf.heading(&format!(
            "Per-term contribution at tau = {:.3e} s",
            m.representative_tau_s
        ));
        pdf.header_row(&[
            (0.0, "Rank / term"),
            (78.0, "x (ns)"),
            (110.0, "Provenance"),
        ]);
        for (rank, t) in m.sorted_terms.iter().enumerate() {
            let name = format!("{}. {}", rank + 1, t.name);
            let xns = format!("{:.4}", t.x_s * 1e9);
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: &name,
                    bold: false,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 78.0,
                    text: &xns,
                    bold: true,
                    colour: crate::report_brand::INK,
                },
                Cell {
                    x_mm: 110.0,
                    text: t.tag.as_str(),
                    bold: true,
                    colour: BRASS_MUTED,
                },
            ]);
        }
        pdf.gap(3.0);
        pdf.paragraph(&format!(
            "Root-sum-square total x_sigma at this tau: {:.4} ns. The 1 ns per-term floor line is \
             the reference the study sizes against.",
            m.x_sigma_at_rep_s * 1e9
        ));

        pdf.heading("Caveats (read first)");
        pdf.paragraph(
            "- The clock row is Validated against a published one-day spec; every other term is a \
             Modelled budget allocation, and both x_sigma and the crossover tau inherit that.",
        );
        pdf.paragraph(
            "- This is a reproducible crossover analysis, not a certified per-term number and not \
             certified for operational timekeeping.",
        );

        let footer =
            "Ashforde OU  |  Kshana Pro  |  MODELLED engineering study - not a certification";
        pdf.render("Kshana Pro - LTC time-error budget (L48)", footer)
    }

    /// Write the L48 paper artifacts (committed CSV + figure SVG + HTML report +
    /// branded PDF) into `papers/lunar-time-budget/` (a NEW dir — the founder's other
    /// paper dirs are untouched). Invoked out-of-band, never from `run`.
    pub fn emit_paper_artifacts(&self) -> std::io::Result<std::path::PathBuf> {
        let m = self
            .compute()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("papers/lunar-time-budget");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("lunar_time_budget.csv"), self.csv_string(&m))?;
        std::fs::write(dir.join("fig1_ltc_budget.svg"), fig_budget_svg(self, &m))?;
        std::fs::write(dir.join("report.html"), budget_report_html(self, &m))?;
        let pdf = self.to_pdf(&m).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("report.pdf"), pdf)?;
        Ok(dir)
    }
}

impl Scenario for LunarTimeBudgetStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let doc = self.json(&m).to_string();
        let svg = fig_budget_svg(self, &m);
        let summary = self.summary(&m);
        Ok(RunOutput {
            json: doc,
            svg,
            summary,
        })
    }
}

impl ExternalPack for LunarTimeBudgetStudyScenario {
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

// ── Figure 1 — two panels: Allan-style curves + sorted per-term bars ──────────

/// A distinct colour per budget term (clock = teal, the Validated row).
fn term_color(name: &str) -> &'static str {
    if name.starts_with("clock:") {
        "#2dd4bf"
    } else {
        match name {
            "rf-link-floor" => "#f0a868",
            "optical-link-floor" => "#e5645a",
            "frame-realisation" => "#c7aa66",
            "relativistic-residual" => "#9b8cff",
            "ephemeris" => "#6fae7a",
            "measurement" => "#8c8273",
            _ => "#b6985a",
        }
    }
}

/// A short legend label for a term name.
fn term_short(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("clock:") {
        format!("clock ({rest})")
    } else {
        name.to_string()
    }
}

fn fig_budget_svg(scn: &LunarTimeBudgetStudyScenario, m: &StudyModel) -> String {
    let (w, h) = (760.0_f64, 720.0_f64);

    // ---- Panel (a): log-log x_i(tau) curves + RSS ----
    let (ml, mr, at, ah) = (72.0_f64, 210.0_f64, 60.0_f64, 250.0_f64);
    let pw = w - ml - mr;
    let tmin_log = scn.tau_min_s.log10();
    let tmax_log = scn.tau_max_s.log10();
    // y-range over all positive term values across the grid + RSS.
    let mut ymin = f64::INFINITY;
    let mut ymax = 0.0_f64;
    for t in &m.budget.terms {
        for &x in &t.x_s {
            if x > 0.0 {
                ymin = ymin.min(x);
                ymax = ymax.max(x);
            }
        }
    }
    for &x in &m.budget.x_sigma_s {
        if x > 0.0 {
            ymax = ymax.max(x);
        }
    }
    ymin = ymin.min(ONE_NS_S);
    let ymin_log = ymin.log10().floor();
    let ymax_log = ymax.log10().ceil();

    let lx = |tau: f64| ml + (tau.log10() - tmin_log) / (tmax_log - tmin_log) * pw;
    let ly = |x: f64| {
        let xl = x.max(1e-300).log10();
        at + ah * (1.0 - (xl - ymin_log) / (ymax_log - ymin_log))
    };

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"28\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"18\" font-weight=\"bold\">Figure 1 — LTC time-error budget</text>");
    s += &format!("<text x=\"20\" y=\"46\" fill=\"{IVORY}\">(a) seven terms x_i(&#964;) + RSS &#8226; clock {} &#8226; <tspan fill=\"#2dd4bf\">clock row Validated</tspan></text>", m.clock_label);

    let (x0, x1) = (ml, ml + pw);
    let (yb, yt) = (at + ah, at);
    // Axes.
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{yb}\" x2=\"{x1}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{yt}\" x2=\"{x0}\" y2=\"{yb}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    // x decade gridlines/labels.
    let mut d = tmin_log.ceil() as i64;
    while (d as f64) <= tmax_log {
        let x = lx(10f64.powi(d as i32));
        s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{yt}\" x2=\"{x:.1}\" y2=\"{yb}\" stroke=\"#1c2637\"/>"
        );
        s += &format!("<text x=\"{x:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"middle\" font-size=\"9\">1e{d}</text>", yb + 14.0);
        d += 1;
    }
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">averaging time &#964; (s)</text>", (x0 + x1) / 2.0, yb + 30.0);
    // y decade gridlines/labels (in ns for readability).
    let mut yd = ymin_log as i64;
    while (yd as f64) <= ymax_log {
        let y = ly(10f64.powi(yd as i32));
        s += &format!(
            "<line x1=\"{x0}\" y1=\"{y:.1}\" x2=\"{x1}\" y2=\"{y:.1}\" stroke=\"#1c2637\"/>"
        );
        let ns = 10f64.powi(yd as i32) * 1e9;
        s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" text-anchor=\"end\" font-size=\"9\">{ns:.0e} ns</text>", x0 - 6.0, y + 3.0);
        yd += 1;
    }
    // 1 ns floor line.
    let y1 = ly(ONE_NS_S);
    s += &format!("<line x1=\"{x0}\" y1=\"{y1:.1}\" x2=\"{x1}\" y2=\"{y1:.1}\" stroke=\"#7f6b3a\" stroke-dasharray=\"5 3\"/>");
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">1 ns floor</text>",
        x0 + 4.0,
        y1 - 3.0
    );
    // Term curves.
    for t in &m.budget.terms {
        let pts: String = m
            .budget
            .tau_s
            .iter()
            .zip(t.x_s.iter())
            .map(|(&tau, &x)| format!("{:.1},{:.1}", lx(tau), ly(x)))
            .collect::<Vec<_>>()
            .join(" ");
        s += &format!(
            "<polyline points=\"{pts}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.8\"/>",
            term_color(&t.name)
        );
    }
    // RSS curve (ivory, bold).
    let rss_pts: String = m
        .budget
        .tau_s
        .iter()
        .zip(m.budget.x_sigma_s.iter())
        .map(|(&tau, &x)| format!("{:.1},{:.1}", lx(tau), ly(x)))
        .collect::<Vec<_>>()
        .join(" ");
    s += &format!("<polyline points=\"{rss_pts}\" fill=\"none\" stroke=\"{IVORY_LIGHT}\" stroke-width=\"2.6\" stroke-dasharray=\"2 2\"/>");
    // Crossover marker.
    let xc = lx(m.budget.crossover_tau_s.clamp(scn.tau_min_s, scn.tau_max_s));
    s += &format!("<line x1=\"{xc:.1}\" y1=\"{yt}\" x2=\"{xc:.1}\" y2=\"{yb}\" stroke=\"#2dd4bf\" stroke-dasharray=\"3 3\"/>");
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#2dd4bf\" font-size=\"9\">crossover &#964;*={:.1e}s</text>", xc + 4.0, yt + 12.0, m.budget.crossover_tau_s);
    // Legend (right gutter).
    let lgx = x1 + 16.0;
    let mut lgy = yt + 6.0;
    for t in &m.budget.terms {
        s += &format!(
            "<rect x=\"{lgx:.1}\" y=\"{:.1}\" width=\"12\" height=\"4\" fill=\"{}\"/>",
            lgy - 4.0,
            term_color(&t.name)
        );
        s += &format!(
            "<text x=\"{:.1}\" y=\"{lgy:.1}\" fill=\"{IVORY}\" font-size=\"9\">{}</text>",
            lgx + 16.0,
            term_short(&t.name)
        );
        lgy += 15.0;
    }
    s += &format!(
        "<rect x=\"{lgx:.1}\" y=\"{:.1}\" width=\"12\" height=\"4\" fill=\"{IVORY_LIGHT}\"/>",
        lgy - 4.0
    );
    s += &format!("<text x=\"{:.1}\" y=\"{lgy:.1}\" fill=\"{IVORY}\" font-size=\"9\">RSS x_&#931;(&#964;)</text>", lgx + 16.0);

    // ---- Panel (b): sorted per-term bars at the representative tau ----
    let (bt, bh) = (400.0_f64, 250.0_f64);
    let bb = bt + bh;
    s += &format!("<text x=\"20\" y=\"{:.1}\" fill=\"{IVORY}\">(b) per-term contribution at &#964; = {:.1e} s, sorted (log scale) vs 1 ns floor</text>", bt - 14.0, m.representative_tau_s);
    // log-y bounds for the bars.
    let bar_min = m
        .sorted_terms
        .iter()
        .map(|t| t.x_s)
        .filter(|&x| x > 0.0)
        .fold(f64::INFINITY, f64::min)
        .min(ONE_NS_S);
    let bar_max = m
        .sorted_terms
        .iter()
        .map(|t| t.x_s)
        .fold(ONE_NS_S, f64::max);
    let bmin_log = bar_min.log10().floor();
    let bmax_log = bar_max.log10().ceil();
    let by = |x: f64| {
        let xl = x.max(1e-300).log10();
        bt + bh * (1.0 - (xl - bmin_log) / (bmax_log - bmin_log))
    };
    let n = m.sorted_terms.len().max(1);
    let slot = pw / n as f64;
    let bar_w = slot * 0.6;
    s += &format!(
        "<line x1=\"{x0}\" y1=\"{bb}\" x2=\"{x1}\" y2=\"{bb}\" stroke=\"{BRASS_MUTED}\"/>"
    );
    // 1 ns floor line.
    let yf = by(ONE_NS_S);
    s += &format!("<line x1=\"{x0}\" y1=\"{yf:.1}\" x2=\"{x1}\" y2=\"{yf:.1}\" stroke=\"#7f6b3a\" stroke-dasharray=\"5 3\"/>");
    s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" font-size=\"9\">1 ns</text>",
        x1 - 30.0,
        yf - 3.0
    );
    for (i, t) in m.sorted_terms.iter().enumerate() {
        let cx = x0 + slot * (i as f64 + 0.5);
        let top = by(t.x_s);
        s += &format!(
            "<rect x=\"{:.1}\" y=\"{top:.1}\" width=\"{bar_w:.1}\" height=\"{:.1}\" fill=\"{}\"/>",
            cx - bar_w / 2.0,
            (bb - top).max(0.0),
            t.tag.color(),
        );
        s += &format!("<text x=\"{cx:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\" text-anchor=\"middle\">{:.2e}</text>", top - 3.0, t.x_s * 1e9);
        // Rotated term label under the axis.
        s += &format!(
            "<text x=\"{cx:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\" text-anchor=\"end\" transform=\"rotate(-40 {cx:.1} {:.1})\">{}</text>",
            bb + 12.0,
            bb + 12.0,
            term_short(&t.name),
        );
    }
    s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\">x_i at representative &#964; (ns, log)</text>", (x0 + x1) / 2.0, bb + 56.0);

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

fn budget_report_html(scn: &LunarTimeBudgetStudyScenario, m: &StudyModel) -> String {
    let fig = fig_budget_svg(scn, m);
    let sorted_rows: String = m
        .sorted_terms
        .iter()
        .enumerate()
        .map(|(rank, t)| {
            let tagc = t.tag.color();
            format!(
                "<tr>{}{}{}{}</tr>",
                td(&(rank + 1).to_string()),
                td(&term_short(&t.name)),
                td(&format!("{:.4}", t.x_s * 1e9)),
                td(&format!(
                    "<span style=\"color:{tagc};font-weight:600\">{}</span>",
                    t.tag.as_str()
                )),
            )
        })
        .collect();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — LTC Time-Error Budget (L48)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:920px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">The single-&#964; artifact, answered</h2>\
         <p>For the <strong>{clock}</strong> clock the seven LTC error terms are assembled as \
         time-error curves x_i(&#964;) over {n} averaging times and root-summed into x_&#931;(&#964;). \
         The growing clock term overtakes the constant frame-realisation term at the crossover \
         <strong>&#964;* = {tx:.3e} s</strong> ({regime}) — below it the budget is <em>frame-limited</em>, \
         above it <em>clock-limited</em>. The crossover matches its closed form to a residual of \
         {res:.1e} <span style=\"color:#2dd4bf\">[Validated]</span>. The frame term is \
         {frame_ns:.3} ns (&#948;r = {frame_m} m).</p>\
         {fig}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Per-term contribution at &#964; = {rep:.3e} s</h2>\
         <table style=\"border-collapse:collapse;width:100%\">\
         <tr>{h_rank}{h_term}{h_x}{h_prov}</tr>{sorted_rows}</table>\
         <p style=\"font-size:12px;opacity:.85\">x_&#931; at this &#964; = {signs:.4} ns. \
         The clock row is <span style=\"color:#2dd4bf\">Validated</span> against the published \
         one-day spec; every floor / measurement row is a <span style=\"color:{BRASS}\">Modelled</span> \
         budget allocation, as is x_&#931; and the crossover &#964; value. The full curve table is \
         committed to <code>papers/lunar-time-budget/lunar_time_budget.csv</code> (byte-stable).</p>\
         </body></html>",
        cover = cover_html(
            "Coordinated Lunar Time — Time-Error Budget",
            "P3 / L48 — seven-term LTC budget and the clock-vs-frame crossover"
        ),
        banner = modelled_banner_html(
            "The clock row reproduces a published one-day spec (Validated); the link / frame / \
             relativistic / ephemeris floor magnitudes are Modelled budget allocations."
        ),
        clock = m.clock_label,
        n = m.taus.len(),
        tx = m.budget.crossover_tau_s,
        regime = m.regime,
        res = m.crossover_closed_form_residual,
        frame_ns = m.budget.frame_term_s * 1e9,
        frame_m = scn.frame_pos_error_m,
        rep = m.representative_tau_s,
        signs = m.x_sigma_at_rep_s * 1e9,
        h_rank = th("Rank"),
        h_term = th("Term"),
        h_x = th("x (ns)"),
        h_prov = th("Provenance"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Speed of light (m/s) — for the frame position ↔ light-time oracle.
    const C_M_S: f64 = 299_792_458.0;

    fn run_default() -> (Value, String) {
        let scn = LunarTimeBudgetStudyScenario::default();
        let out = scn.run().unwrap();
        (serde_json::from_str(&out.json).unwrap(), out.summary)
    }

    #[test]
    fn runs_and_carries_the_honesty_label() {
        let (v, summary) = run_default();
        assert_eq!(v["kind"], LunarTimeBudgetStudyScenario::KIND);
        let label = v["label"].as_str().unwrap();
        assert!(label.contains("VALIDATED") || label.contains("Validated"));
        assert!(label.contains("MODELLED") || label.contains("Modelled"));
        // Both tier words must literally appear somewhere in the document.
        assert!(label.contains("VALIDATED"));
        assert!(label.contains("MODELLED"));
        assert!(summary.contains("crossover"));
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let scn = LunarTimeBudgetStudyScenario::default();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn seven_terms_and_rss_dominates_each() {
        let scn = LunarTimeBudgetStudyScenario::default();
        let m = scn.compute().unwrap();
        assert_eq!(m.budget.terms.len(), 7);
        for (i, &xs) in m.budget.x_sigma_s.iter().enumerate() {
            for t in &m.budget.terms {
                assert!(
                    xs >= t.x_s[i] - 1e-24,
                    "RSS {xs} < term {} at i={i}",
                    t.name
                );
            }
        }
    }

    #[test]
    fn crossover_matches_closed_form_flicker_and_white() {
        // ORACLE: flicker-floor clock ⇒ τ* = (δr/c)/floor; white-FM ⇒ τ* = 2(δr/c)²/h₀.
        for clock in ["passive-h-maser", "optical-master", "rafs", "mini-rafs"] {
            let scn = LunarTimeBudgetStudyScenario {
                clock: clock.to_string(),
                ..Default::default()
            };
            let m = scn.compute().unwrap();
            assert!(
                m.crossover_closed_form_residual < 1e-6,
                "{clock}: crossover residual {} (numeric {} vs analytic {})",
                m.crossover_closed_form_residual,
                m.budget.crossover_tau_s,
                m.crossover_tau_analytic_s
            );
        }
    }

    #[test]
    fn sorted_terms_have_exactly_one_validated_clock_row() {
        let scn = LunarTimeBudgetStudyScenario::default();
        let m = scn.compute().unwrap();
        assert_eq!(m.sorted_terms.len(), 7);
        // Sorted descending.
        for w in m.sorted_terms.windows(2) {
            assert!(w[0].x_s >= w[1].x_s, "not sorted descending");
        }
        let validated = m
            .sorted_terms
            .iter()
            .filter(|t| t.tag == Tag::Validated)
            .count();
        assert_eq!(validated, 1, "exactly the clock row is Validated");
        assert!(m
            .sorted_terms
            .iter()
            .find(|t| t.tag == Tag::Validated)
            .unwrap()
            .name
            .starts_with("clock:"));
    }

    #[test]
    fn csv_is_well_formed_and_deterministic() {
        let scn = LunarTimeBudgetStudyScenario::default();
        let m = scn.compute().unwrap();
        let a = scn.csv_string(&m);
        assert_eq!(a, scn.csv_string(&m));
        assert!(a.contains("tau_s,x_clock_s,"));
        assert!(a.contains(",x_sigma_s\n"));
        assert!(a.contains("scenario_hash="));
        // One data row per τ plus 3 comment lines + 1 header line.
        let data_rows = a
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("tau_s"))
            .count();
        assert_eq!(data_rows, m.budget.tau_s.len());
    }

    #[test]
    fn svg_is_well_formed_and_deterministic() {
        let scn = LunarTimeBudgetStudyScenario::default();
        let m = scn.compute().unwrap();
        let a = fig_budget_svg(&scn, &m);
        assert!(a.starts_with("<svg"));
        assert!(a.ends_with("</svg>"));
        assert_eq!(a, fig_budget_svg(&scn, &m));
    }

    #[test]
    fn pdf_renders_to_valid_bytes() {
        let scn = LunarTimeBudgetStudyScenario::default();
        let m = scn.compute().unwrap();
        let pdf = scn.to_pdf(&m).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn frame_term_is_light_time_of_position_error() {
        // δr/c for the default 0.3 m frame error is ≈ 1.0007 ns.
        let scn = LunarTimeBudgetStudyScenario::default();
        let m = scn.compute().unwrap();
        let ns = m.budget.frame_term_s * 1e9;
        assert!((ns - 1.0007).abs() < 1e-3, "frame term {ns} ns");
        // Sanity: light-time of δr equals frame term.
        assert!((scn.frame_pos_error_m / C_M_S - m.budget.frame_term_s).abs() < 1e-18);
    }

    #[test]
    #[ignore = "writes the committed paper artifacts to papers/lunar-time-budget/; run with --ignored"]
    fn emit_paper_artifacts() {
        let dir = LunarTimeBudgetStudyScenario::default()
            .emit_paper_artifacts()
            .unwrap();
        assert!(dir.join("lunar_time_budget.csv").exists());
        assert!(dir.join("fig1_ltc_budget.svg").exists());
        assert!(dir.join("report.html").exists());
        assert!(dir.join("report.pdf").exists());
    }

    #[test]
    fn bad_clock_and_bad_grid_are_rejected() {
        let bad_clock = LunarTimeBudgetStudyScenario {
            clock: "nonesuch".into(),
            ..Default::default()
        };
        assert!(bad_clock.run().is_err());
        let bad_grid = LunarTimeBudgetStudyScenario {
            tau_min_s: 10.0,
            tau_max_s: 1.0,
            ..Default::default()
        };
        assert!(bad_grid.run().is_err());
    }
}
