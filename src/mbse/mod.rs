// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # ECSS/MBSE artifact generation (PR16) — requirements traceability & margins
//!
//! Turns a requirements set (each with a target and an achieved value) into the
//! systems-engineering artifacts a study deliverable needs: a **margins table**
//! (target vs achieved vs margin, PASS/FAIL), a **traceability matrix** (CSV), and
//! a **ReqIF-lite** XML export that DOORS / Polarion / Capella can ingest. This is
//! the L3 assurance suite's artifact generator (closes the DOORS/ReqIF follow-on).

use kshana::api::KshanaError;

/// One requirement with its verification status.
#[derive(Debug, Clone)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    /// Verification method (ECSS: Test / Analysis / Review-of-Design / Inspection).
    pub verification: String,
    pub target: f64,
    pub achieved: f64,
    /// True if a lower achieved value is better (e.g. mass, error); false for
    /// quantities where higher is better (e.g. availability).
    pub lower_is_better: bool,
}

/// A computed margin row.
#[derive(Debug, Clone)]
pub struct MarginRow {
    pub id: String,
    pub target: f64,
    pub achieved: f64,
    /// Fractional margin: positive = compliant with headroom.
    pub margin: f64,
    pub status: &'static str, // "PASS" | "FAIL"
}

/// Compute the compliance margin for a requirement. For lower-is-better, margin =
/// (target − achieved)/|target|; for higher-is-better, (achieved − target)/|target|.
/// A non-finite target/achieved or a zero target yields a `FAIL`/`N/A`-style 0 margin.
pub fn margin_of(req: &Requirement) -> MarginRow {
    let (t, a) = (req.target, req.achieved);
    let denom = t.abs();
    let (margin, compliant) = if !t.is_finite() || !a.is_finite() || denom == 0.0 {
        (0.0, false)
    } else if req.lower_is_better {
        ((t - a) / denom, a <= t)
    } else {
        ((a - t) / denom, a >= t)
    };
    MarginRow {
        id: req.id.clone(),
        target: t,
        achieved: a,
        margin,
        status: if compliant { "PASS" } else { "FAIL" },
    }
}

/// Compute margins for every requirement.
pub fn margins(reqs: &[Requirement]) -> Vec<MarginRow> {
    reqs.iter().map(margin_of).collect()
}

/// True if every requirement is compliant.
pub fn all_compliant(rows: &[MarginRow]) -> bool {
    !rows.is_empty() && rows.iter().all(|r| r.status == "PASS")
}

/// Traceability + margins matrix as CSV (DOORS/Excel ingestible).
pub fn to_csv(reqs: &[Requirement]) -> String {
    let mut s = String::from("id,requirement,verification,target,achieved,margin_pct,status\n");
    for (req, m) in reqs.iter().zip(margins(reqs)) {
        s.push_str(&format!(
            "{},{},{},{},{},{:.2},{}\n",
            csv_field(&req.id),
            csv_field(&req.text),
            csv_field(&req.verification),
            csv_num(req.target),
            csv_num(req.achieved),
            m.margin * 100.0,
            m.status,
        ));
    }
    s
}

/// Minimal ReqIF-lite XML: one SPEC-OBJECT per requirement with id/text/verification/
/// status attributes. Structurally importable by ReqIF-aware tools (DOORS, Polarion).
pub fn to_reqif(title: &str, reqs: &[Requirement]) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<REQ-IF xmlns=\"http://www.omg.org/spec/ReqIF/20110401/reqif.xsd\">\n");
    s.push_str(&format!(
        "  <THE-HEADER><REQ-IF-HEADER><TITLE>{}</TITLE><SOURCE-TOOL-ID>Kshana Pro</SOURCE-TOOL-ID></REQ-IF-HEADER></THE-HEADER>\n",
        xml(title)
    ));
    s.push_str("  <CORE-CONTENT><REQ-IF-CONTENT><SPEC-OBJECTS>\n");
    for (req, m) in reqs.iter().zip(margins(reqs)) {
        s.push_str(&format!(
            "    <SPEC-OBJECT IDENTIFIER=\"{}\">\n      <VALUES>\n        <ATTRIBUTE-VALUE-STRING THE-VALUE=\"{}\"/>\n        <ATTRIBUTE-VALUE-STRING THE-VALUE=\"{}\"/>\n        <ATTRIBUTE-VALUE-STRING THE-VALUE=\"{}\"/>\n      </VALUES>\n    </SPEC-OBJECT>\n",
            xml(&req.id),
            xml(&req.text),
            xml(&req.verification),
            m.status,
        ));
    }
    s.push_str("  </SPEC-OBJECTS></REQ-IF-CONTENT></CORE-CONTENT>\n");
    s.push_str("</REQ-IF>\n");
    s
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Format a number for CSV; non-finite values become empty (never `NaN`/`inf`,
/// which would corrupt downstream numeric parsers) — defensive for direct callers.
fn csv_num(x: f64) -> String {
    if x.is_finite() {
        x.to_string()
    } else {
        String::new()
    }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Reject a requirements set with non-finite numbers (so artifacts never carry NaN).
pub fn validate(reqs: &[Requirement]) -> Result<(), KshanaError> {
    if reqs.is_empty() {
        return Err(KshanaError::InvalidInput("no requirements supplied".into()));
    }
    for r in reqs {
        if !r.target.is_finite() || !r.achieved.is_finite() {
            return Err(KshanaError::InvalidInput(format!(
                "requirement '{}' has a non-finite target/achieved",
                r.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str, target: f64, achieved: f64, lower: bool) -> Requirement {
        Requirement {
            id: id.into(),
            text: format!("Requirement {id}"),
            verification: "Analysis".into(),
            target,
            achieved,
            lower_is_better: lower,
        }
    }

    #[test]
    fn lower_is_better_margin_and_status() {
        // mass budget 100 kg, achieved 80 → 20% margin, PASS.
        let m = margin_of(&req("R1", 100.0, 80.0, true));
        assert!((m.margin - 0.20).abs() < 1e-9);
        assert_eq!(m.status, "PASS");
        // achieved 120 → over budget → FAIL, negative margin.
        let m2 = margin_of(&req("R2", 100.0, 120.0, true));
        assert_eq!(m2.status, "FAIL");
        assert!(m2.margin < 0.0);
    }

    #[test]
    fn higher_is_better_margin_and_status() {
        // availability target 0.99, achieved 0.995 → PASS.
        let m = margin_of(&req("R3", 0.99, 0.995, false));
        assert_eq!(m.status, "PASS");
        assert!(m.margin > 0.0);
        let m2 = margin_of(&req("R4", 0.99, 0.98, false));
        assert_eq!(m2.status, "FAIL");
    }

    #[test]
    fn zero_target_and_nonfinite_are_fail() {
        assert_eq!(margin_of(&req("R", 0.0, 1.0, true)).status, "FAIL");
        assert_eq!(margin_of(&req("R", f64::NAN, 1.0, true)).status, "FAIL");
    }

    #[test]
    fn all_compliant_logic() {
        let reqs = [req("a", 100.0, 80.0, true), req("b", 0.99, 0.995, false)];
        assert!(all_compliant(&margins(&reqs)));
        let mixed = [req("a", 100.0, 120.0, true)];
        assert!(!all_compliant(&margins(&mixed)));
        assert!(!all_compliant(&[]));
    }

    #[test]
    fn csv_has_header_and_escapes() {
        let mut r = req("R1", 100.0, 80.0, true);
        r.text = "needs, a comma".into();
        let csv = to_csv(&[r]);
        assert!(csv.starts_with("id,requirement,verification,"));
        assert!(
            csv.contains("\"needs, a comma\""),
            "comma field must be quoted: {csv}"
        );
        assert!(csv.contains("PASS"));
    }

    #[test]
    fn reqif_is_well_formed_and_escapes() {
        let mut r = req("R1", 100.0, 80.0, true);
        r.text = "value < limit & ok".into();
        let xml = to_reqif("Demo SRD", &[r]);
        assert!(xml.contains("<REQ-IF"));
        assert!(xml.contains("<TITLE>Demo SRD</TITLE>"));
        assert!(xml.contains("SPEC-OBJECT IDENTIFIER=\"R1\""));
        assert!(
            xml.contains("value &lt; limit &amp; ok"),
            "must xml-escape: {xml}"
        );
        assert!(xml.trim_end().ends_with("</REQ-IF>"));
    }

    #[test]
    fn validate_rejects_empty_and_nonfinite() {
        assert!(validate(&[]).is_err());
        assert!(validate(&[req("R", f64::INFINITY, 1.0, true)]).is_err());
        assert!(validate(&[req("R", 100.0, 80.0, true)]).is_ok());
    }
}
