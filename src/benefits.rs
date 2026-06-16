// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Benefit-vs-baseline ratio engine (A02)
//!
//! A thin comparison/labelling wrapper over the already-built trade-study core
//! ([`crate::suite`]). Given a "new technology" candidate and an
//! established/competing baseline — each a full scenario the **open engine** runs
//! unchanged — it computes the per-figure-of-merit benefit ratio and attaches to
//! every FoM its honest validated-or-modelled provenance, pulled live from the open
//! core's machine-checked verification matrix ([`kshana::verification`]).
//!
//! ## Discipline (the honesty guard, enforced in code)
//! - **No new physics.** Both candidate and baseline run through the *same* neutral
//!   open code path ([`kshana::api::run_scenario`]); this module only compares and
//!   labels. It cannot make one architecture look better by scoring it differently.
//! - **Both labels always render.** Each row carries the candidate's *and* the
//!   baseline's verification status + oracle, and a [`FomBenefit::crosses_label`]
//!   flag is set when they differ — so a MODELLED number can never be silently
//!   presented as out-performing a VALIDATED one.
//! - **Never auto-upgrade to Validated.** A FoM is labelled `VALIDATED` only when
//!   its mapped verification-matrix row is genuinely validated against an external
//!   dataset; any unmapped FoM falls back to the conservative `MODELLED` label.
//! - The benefit is a *modelled* comparison, never flight-demonstrated.

use kshana::api::{run_scenario, KshanaError};
use kshana::verification::{verification_matrix, OracleKind, VerificationStatus};
use serde::Serialize;

use crate::suite::Variant;
use crate::util::dot_path_number;

/// One figure of merit to compare between the candidate and the baseline.
#[derive(Debug, Clone)]
pub struct FomSpec {
    /// Human-readable FoM name for the rendered table (e.g. `"holdover (s)"`).
    pub name: String,
    /// Dot-path into the **candidate** result JSON (e.g. `quantum.fom.holdover_s`).
    pub candidate_key: String,
    /// Dot-path into the **baseline** result JSON (e.g. `classical.fom.holdover_s`).
    pub baseline_key: String,
    /// True if a *lower* value is better for this FoM (e.g. position error).
    pub lower_better: bool,
}

/// The comparison outcome for one FoM, with both sides' honest provenance.
#[derive(Debug, Clone, Serialize)]
pub struct FomBenefit {
    pub name: String,
    pub candidate_key: String,
    pub baseline_key: String,
    pub candidate: Option<f64>,
    pub baseline: Option<f64>,
    pub lower_better: bool,
    /// Benefit ratio normalised so `> 1.0` always means "candidate is better",
    /// regardless of direction. `None` if either side is missing or the divisor is
    /// zero.
    pub ratio: Option<f64>,
    /// Verification status of the candidate FoM (`VALIDATED` / `MODELLED` / `PARTNER`).
    pub candidate_label: String,
    pub candidate_oracle: String,
    /// Verification status of the baseline FoM.
    pub baseline_label: String,
    pub baseline_oracle: String,
    /// True when candidate and baseline carry **different** verification statuses —
    /// the loud flag that this row mixes evidence tiers (e.g. modelled vs validated).
    pub crosses_label: bool,
}

/// A full benefit-vs-baseline case: a reproducible, labelled comparison table.
#[derive(Debug, Clone, Serialize)]
pub struct BenefitsCase {
    pub title: String,
    pub candidate_label: String,
    pub baseline_label: String,
    /// The open engine's reproducible scenario hash for each side (`None` if the
    /// run failed).
    pub candidate_hash: Option<String>,
    pub baseline_hash: Option<String>,
    pub rows: Vec<FomBenefit>,
    /// True if any row mixes evidence tiers — surfaced at the case level so a
    /// reviewer sees it without scanning every row.
    pub any_crosses_label: bool,
}

/// Map a FoM dot-path to the verification-matrix `requirement` whose status backs
/// it. Returns `None` for an unmapped FoM (which then takes the conservative
/// MODELLED fallback — never an unearned VALIDATED). The mapping is by capability,
/// not by guesswork: each target requirement is asserted to exist in the live
/// matrix by [`tests::every_mapping_resolves_to_a_live_matrix_row`].
fn mapped_requirement(fom_key: &str) -> Option<&'static str> {
    let k = fom_key.to_ascii_lowercase();
    // Order matters: more specific substrings first.
    if k.contains("holdover")
        || k.contains("timing_")
        || k.contains("resilience_slope")
        || k.contains("availability")
    {
        Some("GNSS-denied clock holdover")
    } else if k.contains("pos_") || k.contains("drift_m") {
        Some("Quantum inertial dead-reckoning resilience")
    } else if k.contains("hpl") || k.contains("vpl") {
        // Aviation protection levels = the validated RAIM/ARAIM engine.
        Some("Integrity (RAIM/ARAIM/SBAS)")
    } else if k.contains("integrity") {
        // FoMScores.integrity is *filter self-consistency* — a MODELLED estimator
        // health figure, NOT aviation HPL/VPL. Mapping it here (not to the validated
        // RAIM row) is the honest classification; mislabelling it VALIDATED would
        // overclaim.
        Some("Onboard clock state estimation")
    } else if k.contains("security") || k.contains("spoof") {
        Some("Spoofing detection")
    } else {
        None
    }
}

/// Short stable string for an oracle kind (for the JSON/SVG artifacts).
fn oracle_str(o: OracleKind) -> &'static str {
    match o {
        OracleKind::ExternalDataset => "ExternalDataset",
        OracleKind::ReferenceImpl => "ReferenceImpl",
        OracleKind::InternalConsistency => "InternalConsistency",
        OracleKind::NoneKind => "None",
    }
}

/// The honest (status, oracle) for a FoM, pulled live from the verification matrix.
/// Unmapped or unmatched FoMs get the conservative MODELLED / InternalConsistency
/// fallback so nothing is ever silently upgraded to VALIDATED.
pub fn fom_provenance(fom_key: &str) -> (VerificationStatus, OracleKind) {
    if let Some(req) = mapped_requirement(fom_key) {
        if let Some(item) = verification_matrix()
            .into_iter()
            .find(|it| it.requirement.contains(req))
        {
            return (item.status, item.oracle_kind);
        }
    }
    (
        VerificationStatus::Modelled,
        OracleKind::InternalConsistency,
    )
}

/// Benefit ratio normalised so `> 1.0` means "candidate better" in both directions.
fn benefit_ratio(candidate: Option<f64>, baseline: Option<f64>, lower_better: bool) -> Option<f64> {
    let (c, b) = (candidate?, baseline?);
    if lower_better {
        // Lower is better → ratio = baseline / candidate (smaller candidate ⇒ >1).
        if c == 0.0 {
            return None;
        }
        Some(b / c)
    } else {
        // Higher is better → ratio = candidate / baseline.
        if b == 0.0 {
            return None;
        }
        Some(c / b)
    }
}

/// Run the candidate and baseline once each through the open engine, then build the
/// labelled benefit table. Both runs use the identical neutral code path, so the
/// comparison cannot be gamed by scoring the two sides differently.
pub fn benefits_case(
    title: &str,
    candidate: &Variant,
    baseline: &Variant,
    foms: &[FomSpec],
) -> Result<BenefitsCase, KshanaError> {
    if foms.is_empty() {
        return Err(KshanaError::InvalidInput(
            "a benefits case needs >= 1 figure of merit".into(),
        ));
    }

    // Run both sides; capture hash + parsed JSON. A failed run yields a null JSON so
    // every FoM row is captured as missing (not fatal).
    let run = |toml: &str| -> (Option<String>, serde_json::Value) {
        match run_scenario(toml) {
            Ok(out) => {
                let json: serde_json::Value =
                    serde_json::from_str(&out.json).unwrap_or(serde_json::Value::Null);
                let hash = json
                    .get("scenario_hash")
                    .and_then(|h| h.as_str())
                    .map(String::from);
                (hash, json)
            }
            Err(_) => (None, serde_json::Value::Null),
        }
    };
    let (candidate_hash, cjson) = run(&candidate.scenario_toml);
    let (baseline_hash, bjson) = run(&baseline.scenario_toml);

    let mut rows = Vec::with_capacity(foms.len());
    let mut any_crosses = false;
    for f in foms {
        let cval = dot_path_number(&cjson, &f.candidate_key);
        let bval = dot_path_number(&bjson, &f.baseline_key);
        let (cs, co) = fom_provenance(&f.candidate_key);
        let (bs, bo) = fom_provenance(&f.baseline_key);
        let crosses = cs != bs;
        any_crosses |= crosses;
        rows.push(FomBenefit {
            name: f.name.clone(),
            candidate_key: f.candidate_key.clone(),
            baseline_key: f.baseline_key.clone(),
            candidate: cval,
            baseline: bval,
            lower_better: f.lower_better,
            ratio: benefit_ratio(cval, bval, f.lower_better),
            candidate_label: cs.tag().to_string(),
            candidate_oracle: oracle_str(co).to_string(),
            baseline_label: bs.tag().to_string(),
            baseline_oracle: oracle_str(bo).to_string(),
            crosses_label: crosses,
        });
    }

    Ok(BenefitsCase {
        title: title.to_string(),
        candidate_label: candidate.label.clone(),
        baseline_label: baseline.label.clone(),
        candidate_hash,
        baseline_hash,
        rows,
        any_crosses_label: any_crosses,
    })
}

impl BenefitsCase {
    /// Reproducible JSON artifact.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("BenefitsCase serializes")
    }

    /// Standalone SVG benefit table — every row shows both evidence-tier labels and
    /// the modelled-comparison caveat is stamped on the artifact face.
    pub fn to_svg(&self) -> String {
        let row_h = 26.0_f64;
        let header_h = 64.0_f64;
        let foot_h = 40.0_f64;
        let w = 880.0_f64;
        let h = header_h + row_h * (self.rows.len() as f64 + 1.0) + foot_h;
        let mut s = String::new();
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" \
             font-family=\"sans-serif\" font-size=\"12\" fill=\"#e6edf3\">"
        ));
        s.push_str(&format!(
            "<rect width=\"{w:.0}\" height=\"{h:.0}\" fill=\"#0b0e14\"/>"
        ));
        s.push_str(&format!(
            "<text x=\"16\" y=\"24\" font-size=\"15\" font-weight=\"bold\">Benefit vs baseline: {} vs {}</text>",
            xml(&self.candidate_label),
            xml(&self.baseline_label)
        ));
        s.push_str(&format!(
            "<text x=\"16\" y=\"44\" fill=\"#8b97ad\">{}</text>",
            xml(&self.title)
        ));
        // Column headers.
        let cols = [
            "FoM",
            "candidate",
            "baseline",
            "benefit×",
            "cand label",
            "base label",
        ];
        let xs = [16.0, 250.0, 360.0, 470.0, 560.0, 710.0];
        let y0 = header_h;
        for (c, x) in cols.iter().zip(xs.iter()) {
            s.push_str(&format!(
                "<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"#8b97ad\" font-weight=\"bold\">{}</text>",
                y0, c
            ));
        }
        for (i, r) in self.rows.iter().enumerate() {
            let y = y0 + row_h * (i as f64 + 1.0);
            let fmt = |v: Option<f64>| v.map(|x| format!("{x:.4}")).unwrap_or_else(|| "—".into());
            let ratio = r
                .ratio
                .map(|x| format!("{x:.3}"))
                .unwrap_or_else(|| "—".into());
            let cells = [
                (xs[0], xml(&r.name)),
                (xs[1], fmt(r.candidate)),
                (xs[2], fmt(r.baseline)),
                (xs[3], ratio),
                (xs[4], label_glyph(&r.candidate_label)),
                (xs[5], label_glyph(&r.baseline_label)),
            ];
            // A crossed-label row is tinted to make the mixed-tier comparison loud.
            if r.crosses_label {
                s.push_str(&format!(
                    "<rect x=\"8\" y=\"{:.0}\" width=\"{:.0}\" height=\"{row_h:.0}\" fill=\"#3a1f12\"/>",
                    y - 16.0,
                    w - 16.0
                ));
            }
            for (x, t) in cells {
                s.push_str(&format!("<text x=\"{x:.0}\" y=\"{y:.0}\">{t}</text>"));
            }
        }
        let caveat = if self.any_crosses_label {
            "MODELLED comparison — one or more rows mix evidence tiers (highlighted). Not flight-validated."
        } else {
            "MODELLED comparison — figures are simulation results, not flight-validated."
        };
        s.push_str(&format!(
            "<text x=\"16\" y=\"{:.0}\" fill=\"#eab308\">{}</text>",
            h - 14.0,
            xml(caveat)
        ));
        s.push_str("</svg>");
        s
    }

    /// Float-free provenance/structure table for the committed D1.1 deliverable.
    ///
    /// Renders ONLY platform-stable content — FoM names, the better-when direction,
    /// both evidence-tier labels + oracles, the mixed-tier flag, and the input
    /// scenario hashes — deliberately NOT the numeric ratios. The open core's
    /// reproducibility doctrine anchors cross-platform stability on the scenario
    /// hash, not on result floats (those are only guaranteed reproducible
    /// in-process), so the *committed* deliverable carries the provenance skeleton
    /// and the numeric table is emitted on demand by `kshana-pro benefits --json`.
    /// `tests/benefits_deliverable_in_sync.rs` holds the committed deliverable
    /// byte-identical to this output.
    pub fn to_provenance_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# Benefit-vs-baseline — provenance skeleton (A02, D1.1)\n\n");
        md.push_str(&format!(
            "**Comparison:** {} (candidate) vs {} (baseline)\n\n",
            self.candidate_label, self.baseline_label
        ));
        md.push_str(&format!("**Case:** {}\n\n", self.title));
        md.push_str(&format!(
            "**Input scenario hashes (cross-platform reproducibility anchors):** \
             candidate `{}` · baseline `{}`\n\n",
            self.candidate_hash.as_deref().unwrap_or("—"),
            self.baseline_hash.as_deref().unwrap_or("—"),
        ));
        md.push_str(
            "| FoM | Better when | Candidate evidence | Baseline evidence | Mixed tier? |\n",
        );
        md.push_str(
            "|-----|-------------|--------------------|-------------------|-------------|\n",
        );
        for r in &self.rows {
            md.push_str(&format!(
                "| {} | {} | {} ({}) | {} ({}) | {} |\n",
                r.name,
                if r.lower_better { "lower" } else { "higher" },
                r.candidate_label,
                r.candidate_oracle,
                r.baseline_label,
                r.baseline_oracle,
                if r.crosses_label { "YES (mixed)" } else { "no" },
            ));
        }
        md.push_str(&format!(
            "\n_{}_\n",
            if self.any_crosses_label {
                "MODELLED comparison — one or more rows mix evidence tiers (flagged). \
                 Numeric ratios: run `kshana-pro benefits --json`. Not flight-validated."
            } else {
                "MODELLED comparison — numeric ratios emitted by `kshana-pro benefits --json` \
                 (reproducible in process). Not flight-validated."
            }
        ));
        md
    }
}

/// The canonical reference benefit case: an optical-lattice (quantum) clock candidate
/// vs a CSAC (classical) baseline, both run through the identical dual-clock,
/// GNSS-denied scenario on the open engine (the same neutral code path, so neither
/// side is scored differently). This is the reproducible D1.1 benefit-table
/// deliverable emitted by `kshana-pro benefits` and drift-guarded by
/// `tests/benefits_deliverable_in_sync.rs`.
pub fn reference_case() -> Result<BenefitsCase, KshanaError> {
    const SCENARIO: &str = "seed = 42\n\
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
        q_rw = 0.0\n";
    let candidate = Variant {
        label: "optical-sr-lattice (quantum)".into(),
        scenario_toml: SCENARIO.into(),
    };
    let baseline = Variant {
        label: "csac-sa45s (classical)".into(),
        scenario_toml: SCENARIO.into(),
    };
    let foms = vec![
        FomSpec {
            name: "holdover (s)".into(),
            candidate_key: "quantum.fom.holdover_s".into(),
            baseline_key: "classical.fom.holdover_s".into(),
            lower_better: false,
        },
        FomSpec {
            name: "timing RMS (ns)".into(),
            candidate_key: "quantum.fom.timing_rms_ns".into(),
            baseline_key: "classical.fom.timing_rms_ns".into(),
            lower_better: true,
        },
        FomSpec {
            name: "availability".into(),
            candidate_key: "quantum.fom.availability".into(),
            baseline_key: "classical.fom.availability".into(),
            lower_better: false,
        },
    ];
    benefits_case(
        "Quantum vs classical clock — GNSS-denied benefit (reference)",
        &candidate,
        &baseline,
        &foms,
    )
}

/// One-char glyph + tag for a label, for the compact SVG cell.
fn label_glyph(label: &str) -> String {
    match label {
        "VALIDATED" => "✓ VALIDATED".into(),
        "MODELLED" => "~ MODELLED".into(),
        other => other.into(),
    }
}

/// Minimal XML escape for text nodes.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A clock-holdover scenario (the open engine's default kind): the canonical
    // quantum-vs-classical FoM source. Format mirrors scenarios/clock-holdover.toml.
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
         provenance = \"demo optical lattice; q_wf=sigma_y(1s)^2\"\n\
         y0   = 5.0e-17\n\
         q_wf = 1.0e-30\n\
         q_rw = 0.0\n\
         [clock_classical]\n\
         id = \"csac-sa45s\"\n\
         provenance = \"demo CSAC datasheet; q_wf=sigma_y(1s)^2\"\n\
         y0   = 5.0e-10\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n"
            .into()
    }

    fn variants() -> (Variant, Variant) {
        (
            Variant {
                label: "optical clock".into(),
                scenario_toml: clock_scenario(),
            },
            Variant {
                label: "CSAC".into(),
                scenario_toml: clock_scenario(),
            },
        )
    }

    fn holdover_foms() -> Vec<FomSpec> {
        vec![FomSpec {
            name: "holdover (s)".into(),
            candidate_key: "quantum.fom.holdover_s".into(),
            baseline_key: "classical.fom.holdover_s".into(),
            lower_better: false,
        }]
    }

    #[test]
    fn benefit_ratio_normalises_direction() {
        // Higher-better: candidate 10, baseline 5 → 2.0.
        assert_eq!(benefit_ratio(Some(10.0), Some(5.0), false), Some(2.0));
        // Lower-better: candidate 5, baseline 10 → 2.0 (candidate is twice as good).
        assert_eq!(benefit_ratio(Some(5.0), Some(10.0), true), Some(2.0));
        // Missing side → None; zero divisor → None.
        assert_eq!(benefit_ratio(None, Some(5.0), false), None);
        assert_eq!(benefit_ratio(Some(5.0), Some(0.0), false), None);
        assert_eq!(benefit_ratio(Some(0.0), Some(5.0), true), None);
    }

    #[test]
    fn every_mapping_resolves_to_a_live_matrix_row() {
        // Each requirement the mapping points at MUST exist in the live matrix, so
        // the labels can never silently drift from the open core's honesty ledger.
        let matrix = verification_matrix();
        for probe in [
            "holdover_s",
            "timing_rms_ns",
            "availability",
            "pos_drift_m",
            "quantum.fom.integrity",
            "user.hpl_m",
            "security",
        ] {
            let req = mapped_requirement(probe)
                .unwrap_or_else(|| panic!("{probe} should map to a requirement"));
            assert!(
                matrix.iter().any(|it| it.requirement.contains(req)),
                "mapped requirement '{req}' (for FoM '{probe}') not found in the live matrix"
            );
        }
    }

    #[test]
    fn holdover_is_labelled_modelled_not_validated() {
        // The honesty bar: a quantum holdover benefit must NOT be labelled VALIDATED
        // (its matrix row is Modelled/ReferenceImpl).
        let (cs, _) = fom_provenance("quantum.fom.holdover_s");
        assert_eq!(cs, VerificationStatus::Modelled);
    }

    #[test]
    fn unmapped_fom_falls_back_to_modelled_never_validated() {
        let (s, o) = fom_provenance("some.unmapped.metric");
        assert_eq!(s, VerificationStatus::Modelled);
        assert_eq!(o, OracleKind::InternalConsistency);
    }

    #[test]
    fn aviation_hpl_pulls_the_validated_label() {
        // A genuinely-validated capability (RAIM/ARAIM protection level) DOES surface
        // as VALIDATED.
        let (s, o) = fom_provenance("user.hpl_m");
        assert_eq!(s, VerificationStatus::Validated);
        assert_eq!(o, OracleKind::ExternalDataset);
    }

    #[test]
    fn fom_integrity_is_modelled_self_consistency_not_validated() {
        // The honesty fix: FoMScores.integrity is filter self-consistency (MODELLED),
        // NOT aviation HPL/VPL — it must never carry the VALIDATED RAIM label.
        let (s, _) = fom_provenance("quantum.fom.integrity");
        assert_eq!(s, VerificationStatus::Modelled);
    }

    #[test]
    fn benefits_case_runs_and_labels_both_sides() {
        let (c, b) = variants();
        let case = benefits_case("clock holdover trade", &c, &b, &holdover_foms()).unwrap();
        assert_eq!(case.rows.len(), 1);
        let row = &case.rows[0];
        // Both runs produced a reproducible hash and a holdover value.
        assert!(case.candidate_hash.is_some());
        assert!(case.baseline_hash.is_some());
        assert!(row.candidate.is_some() && row.baseline.is_some());
        // Both labels rendered; same FoM ⇒ same tier ⇒ no cross-label.
        assert_eq!(row.candidate_label, "MODELLED");
        assert_eq!(row.baseline_label, "MODELLED");
        assert!(!row.crosses_label);
        assert!(!case.any_crosses_label);
    }

    #[test]
    fn json_and_svg_render_with_caveat_and_labels() {
        let (c, b) = variants();
        let case = benefits_case("clock holdover trade", &c, &b, &holdover_foms()).unwrap();
        let json = case.to_json();
        assert!(json.contains("candidate_label"));
        assert!(json.contains("MODELLED"));
        let svg = case.to_svg();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("MODELLED"));
        assert!(svg.contains("Not flight-validated") || svg.contains("not flight-validated"));
    }

    #[test]
    fn crossed_label_is_flagged_when_tiers_differ() {
        // Construct a synthetic cross-tier comparison: candidate on a Modelled FoM,
        // baseline on a Validated FoM — the guard must light up.
        let (c, b) = variants();
        let foms = vec![FomSpec {
            name: "mixed".into(),
            candidate_key: "quantum.fom.holdover_s".into(), // Modelled
            baseline_key: "user.hpl_m".into(),              // aviation HPL → RAIM → Validated
            lower_better: false,
        }];
        let case = benefits_case("mixed-tier", &c, &b, &foms).unwrap();
        assert!(case.rows[0].crosses_label);
        assert!(case.any_crosses_label);
        assert_eq!(case.rows[0].candidate_label, "MODELLED");
        assert_eq!(case.rows[0].baseline_label, "VALIDATED");
    }

    #[test]
    fn empty_foms_is_an_error() {
        let (c, b) = variants();
        assert!(matches!(
            benefits_case("x", &c, &b, &[]),
            Err(KshanaError::InvalidInput(_))
        ));
    }
}
