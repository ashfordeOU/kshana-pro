// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Consolidated performance/resilience report (A11)
//!
//! One renderer over the NAV-DT twin bundle ([`crate::twin::navdt`]), the trade-study
//! evidence ([`crate::suite::run_with_evidence`]), the RF-impairment evaluation
//! scores (the open [`kshana::impairment_eval`] testbed), an RTM compliance/margin
//! table, and the open verification-matrix status counts. It emits the consolidated
//! artifact in **Markdown / HTML / SVG** plus a **machine-readable JSON evidence
//! pack**, and renders every modelled-assumption caveat **on the artifact face**.
//!
//! ## Honesty guard (enforced by tests)
//! - Every modelled FoM shows its `MODELLED` label *and* a caveat (holdover floor
//!   assumption, synthetic-scenario provenance, modelled-quantum) on the artifact.
//! - The artifact states Kshana is **one WP component** of the twin and that figures
//!   are modelled unless tied to an external-validation island.
//! - The verification-matrix counts come live from [`kshana::verification`].

use kshana::api::{run_scenario, KshanaError};
use kshana::verification::{summarize, verification_matrix};
use serde::Serialize;

use crate::connectors::GateOp;
use crate::twin::navdt::{bundle_from_json, NavDtBundle};

/// Verification-matrix status counts (live from the open core).
#[derive(Debug, Clone, Serialize)]
pub struct MatrixCounts {
    pub validated: usize,
    pub modelled: usize,
    pub partner: usize,
}

/// Live verification-matrix counts.
pub fn matrix_counts() -> MatrixCounts {
    let s = summarize(&verification_matrix());
    MatrixCounts {
        validated: s.validated,
        modelled: s.modelled,
        partner: s.partner_owned,
    }
}

/// RF-impairment evaluation summary, from the open [`kshana::impairment_eval`] testbed.
#[derive(Debug, Clone, Serialize)]
pub struct ImpairmentSummary {
    /// Threshold-free AUC of the fused detector over the synthetic corpus.
    pub fused_auc: f64,
    pub n_cases: usize,
    pub note: String,
}

/// Run the open RF-impairment testbed and summarise the fused detector's AUC. This
/// is a **separability sanity** figure over a synthetic, parameter-grounded corpus —
/// not a field-performance number.
pub fn run_impairment_summary(seed: u64) -> ImpairmentSummary {
    use kshana::impairment_eval::{evaluate, generate_corpus, CorpusConfig, FusedDetector};
    let corpus = generate_corpus(&CorpusConfig::default(), seed);
    let report = evaluate(&FusedDetector, &corpus, 0.05);
    ImpairmentSummary {
        fused_auc: report.auc,
        n_cases: report.n_cases,
        note: "Synthetic (parameter-grounded, not IQ) corpus; fused published-method \
               baseline. AUC demonstrates pipeline separability, NOT field performance."
            .into(),
    }
}

/// One requirement-traceability/margin row.
#[derive(Debug, Clone, Serialize)]
pub struct RtmRow {
    pub req: String,
    pub target: f64,
    pub achieved: f64,
    pub lower_better: bool,
    pub pass: bool,
    /// A required row that fails fails the CI gate; an optional row only informs.
    pub required: bool,
    pub note: String,
}

impl RtmRow {
    /// Build a **required** row, computing `pass` from the direction.
    pub fn new(req: &str, target: f64, achieved: f64, lower_better: bool, note: &str) -> RtmRow {
        let pass = if lower_better {
            achieved <= target
        } else {
            achieved >= target
        };
        RtmRow {
            req: req.into(),
            target,
            achieved,
            lower_better,
            pass,
            required: true,
            note: note.into(),
        }
    }

    /// Mark this row optional (informational; never fails the gate).
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// One CI metric-threshold check over a report FoM.
#[derive(Debug, Clone)]
pub struct GateCheck {
    /// FoM key or human name to look up in the bundle (e.g. `"availability"`).
    pub metric: String,
    pub op: GateOp,
    pub threshold: f64,
}

impl GateCheck {
    pub fn new(metric: &str, op: GateOp, threshold: f64) -> GateCheck {
        GateCheck {
            metric: metric.into(),
            op,
            threshold,
        }
    }
}

/// The outcome of a CI gate over a report.
#[derive(Debug, Clone, Serialize)]
pub struct GateOutcome {
    pub passed: bool,
    pub failures: Vec<String>,
    /// The honesty note: a passing gate is reproducibility, NOT external validation.
    pub note: String,
}

/// The consolidated report. `Serialize` so the struct *is* the machine-readable
/// evidence pack ([`ConsolidatedReport::to_evidence_json`]).
#[derive(Debug, Clone, Serialize)]
pub struct ConsolidatedReport {
    pub title: String,
    pub bundle: NavDtBundle,
    /// Resilience-vs-time series `[t_s, |error|]` extracted from the open run.
    pub resilience_series: Vec<[f64; 2]>,
    /// Spec threshold for the resilience curve, if the scenario had one.
    pub threshold: Option<f64>,
    pub matrix: MatrixCounts,
    pub trade_evidence_md: Option<String>,
    pub impairment: Option<ImpairmentSummary>,
    pub rtm_rows: Vec<RtmRow>,
    /// Modelled-assumption caveats rendered on the artifact face.
    pub caveats: Vec<String>,
}

/// The standard caveat set every consolidated artifact must carry.
fn default_caveats() -> Vec<String> {
    vec![
        "Kshana provides ONE work-package component (the performance/resilience spine) of a \
         partner-led NAV-DT — not the whole twin."
            .into(),
        "Figures are MODELLED (simulation) unless tied to an external-validation island \
         (SGP4/Allan/IGS/frames/Mars-DE440)."
            .into(),
        "Holdover is grid-bounded (a lower bound at the time-step) and carries a clock-noise \
         floor assumption."
            .into(),
        "Scenarios here are synthetic / parameter-grounded, not flight or in-orbit data.".into(),
        "For 13503: a quantum benefit is QUANTIFIED, never validated — the device stays \
         partner-owned."
            .into(),
    ]
}

/// Build the consolidated report by running the open spine once for `scenario_toml`.
pub fn build_report(
    title: &str,
    scenario_toml: &str,
    subject: &str,
) -> Result<ConsolidatedReport, KshanaError> {
    let out = run_scenario(scenario_toml)?;
    let json: serde_json::Value = serde_json::from_str(&out.json)
        .map_err(|e| KshanaError::InvalidInput(format!("result JSON did not parse: {e}")))?;
    let bundle = bundle_from_json(&json, subject);

    // Resilience-vs-time series from the subject's error series, if present.
    let mut resilience_series = Vec::new();
    if let Some(series) = json
        .get(subject)
        .and_then(|s| s.get("series"))
        .and_then(|s| s.as_array())
    {
        for pt in series {
            if let (Some(t), Some(e)) = (
                pt.get("t").and_then(|v| v.as_f64()),
                pt.get("error_ns").and_then(|v| v.as_f64()),
            ) {
                resilience_series.push([t, e.abs()]);
            }
        }
    }
    let threshold = json.get("threshold_ns").and_then(|v| v.as_f64());

    Ok(ConsolidatedReport {
        title: title.into(),
        bundle,
        resilience_series,
        threshold,
        matrix: matrix_counts(),
        trade_evidence_md: None,
        impairment: None,
        rtm_rows: Vec::new(),
        caveats: default_caveats(),
    })
}

impl ConsolidatedReport {
    /// Attach trade-study evidence-pack markdown (from [`crate::suite::run_with_evidence`]).
    pub fn with_trade_evidence(mut self, md: String) -> Self {
        self.trade_evidence_md = Some(md);
        self
    }
    /// Attach an RF-impairment evaluation summary.
    pub fn with_impairment(mut self, s: ImpairmentSummary) -> Self {
        self.impairment = Some(s);
        self
    }
    /// Attach RTM compliance/margin rows.
    pub fn with_rtm(mut self, rows: Vec<RtmRow>) -> Self {
        self.rtm_rows = rows;
        self
    }

    /// CI metric-threshold gate: every [`GateCheck`] must pass and every **required**
    /// RTM row must be compliant. Reuses the connectors' [`GateOp`]. A failing gate
    /// returns the reasons (for a non-zero pipeline exit).
    pub fn gate(&self, checks: &[GateCheck]) -> GateOutcome {
        let mut failures = Vec::new();
        for c in checks {
            let found = self
                .bundle
                .all_foms()
                .into_iter()
                .find(|f| f.key == c.metric || f.name == c.metric);
            match found.and_then(|f| f.value) {
                Some(v) if c.op.passes(v, c.threshold) => {}
                Some(v) => failures.push(format!(
                    "metric '{}' = {v} fails {} {}",
                    c.metric,
                    c.op.as_str(),
                    c.threshold
                )),
                None => failures.push(format!("metric '{}' not found / no value", c.metric)),
            }
        }
        for r in &self.rtm_rows {
            if r.required && !r.pass {
                failures.push(format!(
                    "required RTM row '{}' non-compliant (achieved {}, target {})",
                    r.req, r.achieved, r.target
                ));
            }
        }
        GateOutcome {
            passed: failures.is_empty(),
            failures,
            note: "A passing gate pins reproducible thresholds on our own MODELLED outputs. \
                   It is NOT external validation, ESA acceptance, or TRL maturation."
                .into(),
        }
    }

    /// Machine-readable evidence pack (the struct serialised).
    pub fn to_evidence_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("report serializes")
    }

    /// A deterministic content fingerprint (FNV-1a 64-bit over the evidence JSON),
    /// platform-independent — the reproducibility stamp for the whole artifact.
    pub fn fingerprint(&self) -> String {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &b in self.to_evidence_json().as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{h:016x}")
    }

    /// Consolidated Markdown artifact.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", self.title));
        md.push_str(&format!(
            "_Kshana Pro v{} consolidated performance/resilience report. Scenario hash \
             `{}`; report fingerprint `{}`._\n\n",
            crate::version(),
            self.bundle.scenario_hash.as_deref().unwrap_or("—"),
            self.fingerprint()
        ));

        md.push_str("## Executive FoM summary\n\n");
        md.push_str("| FoM | Value | Label | Oracle |\n|-----|-------|-------|--------|\n");
        for f in self.bundle.all_foms() {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                f.name,
                f.value
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_else(|| "—".into()),
                f.label,
                f.oracle
            ));
        }

        md.push_str(&format!(
            "\n## Verification posture\n\n**{} validated** (external oracle) · **{} modelled** · \
             **{} partner-owned** (live from the open verification matrix).\n",
            self.matrix.validated, self.matrix.modelled, self.matrix.partner
        ));

        md.push_str(&format!(
            "\n## Resilience-vs-time\n\n{} samples over the run",
            self.resilience_series.len()
        ));
        if let Some(t) = self.threshold {
            md.push_str(&format!("; spec threshold {t:.1} ns"));
        }
        md.push_str(".\n");

        if let Some(im) = &self.impairment {
            md.push_str(&format!(
                "\n## RF-impairment evaluation\n\nFused-detector AUC **{:.3}** over {} synthetic \
                 cases. _{}_\n",
                im.fused_auc, im.n_cases, im.note
            ));
        }

        if !self.rtm_rows.is_empty() {
            md.push_str("\n## RTM compliance / margins\n\n");
            md.push_str(
                "| Requirement | Target | Achieved | Pass | Note |\n|---|---|---|---|---|\n",
            );
            for r in &self.rtm_rows {
                md.push_str(&format!(
                    "| {} | {:.4} | {:.4} | {} | {} |\n",
                    r.req,
                    r.target,
                    r.achieved,
                    if r.pass { "PASS" } else { "FAIL" },
                    r.note
                ));
            }
        }

        if let Some(t) = &self.trade_evidence_md {
            md.push_str("\n## Trade-study evidence\n\n");
            md.push_str(t);
            md.push('\n');
        }

        md.push_str("\n## Caveats (read first)\n\n");
        for c in &self.caveats {
            md.push_str(&format!("- {c}\n"));
        }
        md
    }

    /// Self-contained HTML wrapping the Markdown body + the resilience SVG.
    pub fn to_html(&self) -> String {
        let body = xml(&self.to_markdown());
        format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title></head>\
             <body style=\"background:#0b0e14;color:#e6edf3;font-family:system-ui;max-width:1000px;margin:auto;padding:24px\">\
             {}\
             <pre style=\"white-space:pre-wrap;font:13px/1.5 ui-monospace,monospace\">{}</pre>\
             </body></html>",
            xml(&self.title),
            self.to_svg(),
            body
        )
    }

    /// SVG resilience-vs-time curve (|error| vs t) with the spec threshold line and a
    /// modelled caveat stamped on the face.
    pub fn to_svg(&self) -> String {
        let (w, h) = (820.0_f64, 360.0_f64);
        let (ml, mr, mt, mb) = (60.0, 20.0, 30.0, 50.0);
        let pw = w - ml - mr;
        let ph = h - mt - mb;
        let t_max = self
            .resilience_series
            .iter()
            .map(|p| p[0])
            .fold(1.0, f64::max);
        let mut y_max = self.threshold.unwrap_or(1.0) * 1.3;
        for p in &self.resilience_series {
            y_max = y_max.max(p[1]);
        }
        if y_max <= 0.0 {
            y_max = 1.0;
        }
        let xof = |t: f64| ml + (t / t_max) * pw;
        let yof = |e: f64| mt + ph - (e.min(y_max) / y_max) * ph;
        let pts = self
            .resilience_series
            .iter()
            .map(|p| format!("{:.1},{:.1}", xof(p[0]), yof(p[1])))
            .collect::<Vec<_>>()
            .join(" ");
        let mut s = String::new();
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" \
             font-family=\"sans-serif\" font-size=\"12\" fill=\"#e6edf3\">"
        ));
        s.push_str(&format!(
            "<rect width=\"{w:.0}\" height=\"{h:.0}\" fill=\"#0b0e14\"/>"
        ));
        s.push_str(&format!(
            "<text x=\"{ml:.0}\" y=\"18\" font-size=\"14\" font-weight=\"bold\">Resilience vs time (|error| ns)</text>"
        ));
        if let Some(t) = self.threshold {
            let ty = yof(t);
            s.push_str(&format!(
                "<line x1=\"{ml:.0}\" y1=\"{ty:.1}\" x2=\"{:.0}\" y2=\"{ty:.1}\" stroke=\"#f87171\" stroke-dasharray=\"6 4\"/>",
                ml + pw
            ));
            s.push_str(&format!(
                "<text x=\"{:.0}\" y=\"{:.1}\" fill=\"#f87171\">spec {t:.0} ns</text>",
                ml + 4.0,
                ty - 4.0
            ));
        }
        if !pts.is_empty() {
            s.push_str(&format!(
                "<polyline fill=\"none\" stroke=\"#5eead4\" stroke-width=\"2\" points=\"{pts}\"/>"
            ));
        }
        s.push_str(&format!(
            "<text x=\"{ml:.0}\" y=\"{:.0}\" fill=\"#eab308\">MODELLED — simulation, not flight-validated; holdover grid-bounded with a clock-noise floor assumption.</text>",
            h - 14.0
        ));
        s.push_str("</svg>");
        s
    }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock_scenario() -> String {
        "seed = 42\n\
         threshold_ns = 20.0\n\
         [time]\n\
         step_s = 10.0\n\
         duration_s = 7200.0\n\
         [gnss]\n\
         windows = [\n\
           { t0 = 0.0,   t1 = 600.0,  state = \"nominal\" },\n\
           { t0 = 600.0, t1 = 7200.0, state = \"denied\" },\n\
         ]\n\
         [clock_quantum]\n\
         id = \"optical-sr-lattice\"\n\
         provenance = \"demo; q_wf=sigma_y(1s)^2\"\n\
         y0   = 5.0e-17\n\
         q_wf = 1.0e-30\n\
         q_rw = 0.0\n\
         [clock_classical]\n\
         id = \"csac-sa45s\"\n\
         provenance = \"demo; q_wf=sigma_y(1s)^2\"\n\
         y0   = 5.0e-10\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n"
            .into()
    }

    fn report() -> ConsolidatedReport {
        build_report("NAV-DT demo", &clock_scenario(), "quantum").unwrap()
    }

    #[test]
    fn matrix_counts_match_the_live_matrix() {
        let c = matrix_counts();
        // The honesty ledger's headline: a handful validated, the rest modelled/partner.
        assert!(c.validated >= 4 && c.validated <= 8);
        assert!(c.modelled >= 8);
        assert!(c.partner >= 1);
    }

    #[test]
    fn report_has_fom_summary_resilience_series_and_caveats() {
        let r = report();
        assert!(!r.bundle.all_foms().is_empty());
        assert!(
            !r.resilience_series.is_empty(),
            "resilience series populated from the run"
        );
        assert_eq!(r.threshold, Some(20.0));
        assert!(r.caveats.len() >= 4);
    }

    #[test]
    fn markdown_renders_labels_counts_and_caveats_on_face() {
        let r = report();
        let md = r.to_markdown();
        assert!(md.contains("Executive FoM summary"));
        assert!(md.contains("MODELLED"));
        assert!(md.contains("validated")); // matrix counts line
        assert!(md.contains("ONE work-package component"));
        assert!(md.contains("Caveats"));
        // The holdover floor caveat is on the artifact face.
        assert!(md.contains("floor assumption"));
    }

    #[test]
    fn impairment_and_trade_and_rtm_compose() {
        let r = report()
            .with_impairment(run_impairment_summary(7))
            .with_trade_evidence("trade evidence body".into())
            .with_rtm(vec![RtmRow::new(
                "timing RMS (ns)",
                20.0,
                5.0,
                true,
                "in spec",
            )]);
        let md = r.to_markdown();
        assert!(md.contains("RF-impairment evaluation"));
        assert!(md.contains("Fused-detector AUC"));
        assert!(md.contains("Trade-study evidence"));
        assert!(md.contains("RTM compliance"));
        assert!(md.contains("PASS"));
    }

    #[test]
    fn rtm_row_pass_logic_both_directions() {
        assert!(RtmRow::new("a", 20.0, 5.0, true, "").pass); // lower-better, under
        assert!(!RtmRow::new("a", 20.0, 25.0, true, "").pass);
        assert!(RtmRow::new("b", 0.9, 0.95, false, "").pass); // higher-better, over
        assert!(!RtmRow::new("b", 0.9, 0.8, false, "").pass);
    }

    #[test]
    fn svg_and_html_render_with_modelled_caveat() {
        let r = report();
        let svg = r.to_svg();
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(svg.contains("MODELLED"));
        assert!(svg.contains("spec 20 ns"));
        let html = r.to_html();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<svg"));
    }

    #[test]
    fn evidence_json_and_fingerprint_are_deterministic() {
        let a = report();
        let b = report();
        assert_eq!(a.to_evidence_json(), b.to_evidence_json());
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint().len(), 16);
    }

    #[test]
    fn gate_passes_and_fails_on_metric_and_required_rtm() {
        let r = report().with_rtm(vec![
            RtmRow::new("timing RMS (ns)", 20.0, 5.0, true, "in spec"),
            RtmRow::new("coverage", 0.99, 0.5, false, "informational").optional(),
        ]);
        // availability is high in this scenario → a >= 0.5 check passes.
        let ok = r.gate(&[GateCheck::new("availability", GateOp::Ge, 0.5)]);
        assert!(ok.passed, "expected pass, failures: {:?}", ok.failures);
        assert!(ok.note.contains("NOT external validation"));

        // An impossible threshold fails; the optional failing RTM row does NOT.
        let bad = r.gate(&[GateCheck::new("availability", GateOp::Ge, 2.0)]);
        assert!(!bad.passed);
        assert!(bad.failures.iter().any(|f| f.contains("availability")));

        // A missing metric is a failure.
        let missing = r.gate(&[GateCheck::new("does_not_exist", GateOp::Lt, 1.0)]);
        assert!(!missing.passed);
    }

    #[test]
    fn gate_fails_on_noncompliant_required_rtm_row() {
        let r = report().with_rtm(vec![RtmRow::new(
            "holdover (s)",
            6000.0,
            10.0, // achieved far below a higher-better target → fails
            false,
            "required",
        )]);
        let out = r.gate(&[]);
        assert!(!out.passed);
        assert!(out.failures.iter().any(|f| f.contains("required RTM row")));
    }
}
