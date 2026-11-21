// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Requirements cross-walk for assured-PNT user equipment, L-band and C-band (research
//! 2026-11-20 gaps, B2101)
//!
//! Each step of the published LEO-PNT procedures (`docs/leo-procedures/`, frozen at 1.0) is
//! mapped to the public requirement documents that bear on it, for user equipment receiving in
//! the L-band RNSS allocations (1 164-1 300 MHz, 1 559-1 610 MHz) and the C-band RNSS allocation
//! (5 010-5 030 MHz). Every row cites a source read for this table, or says **to confirm**.
//! A row states how the step relates to the clause (it measures the same quantity, a related
//! one, evidence for an expectation, or that no public requirement was found); it never says a
//! receiver meets a document. Rows also list clauses no published procedure exercises (gaps).
//!
//! Documents behind paywalls (RTCA, EUROCAE, IEC) are not cited: a row that would need one
//! says "to confirm".

use serde::Serialize;

use super::sims::{Procedure, PUBLISHED};

/// Pro feature.
pub const FEATURE: &str = "pro-leo-ue-crosswalk";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-leo-ue-crosswalk/1";
/// What the table says about itself.
pub const WORDING: &str = "A cross-walk from published test steps to public requirement \
documents. Each row cites the source it was read from, or says to confirm. It states how a step \
relates to a clause, never that any receiver meets a document; that is for the receiver's \
assessor and authority.";
/// The marker of a row with no read source.
pub const TO_CONFIRM: &str = "to confirm";

/// A source document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Source {
    pub key: &'static str,
    pub citation: &'static str,
    pub url: &'static str,
    /// What was read, and when.
    pub read: &'static str,
}

/// The sources.
pub const SOURCES: &[Source] = &[
    Source {
        key: "EN303413",
        citation: "ETSI EN 303 413 V1.2.1 (2021-04), Satellite Earth Stations and Systems (SES); Global Navigation Satellite System (GNSS) receivers; Radio equipment operating in the 1 164 MHz to 1 300 MHz and 1 559 MHz to 1 610 MHz frequency bands; Harmonised Standard for access to radio spectrum",
        url: "https://www.etsi.org/deliver/etsi_en/303400_303499/303413/01.02.01_60/en_303413v010201p.pdf",
        read: "Read 2026-11-21: clause 4.2 (4.2.1 receiver blocking, tables 4-1 to 4-4; 4.2.2 receiver spurious emissions), clause 5.4 (blocking test methods per band).",
    },
    Source {
        key: "ITU-R-M.2031",
        citation: "Recommendation ITU-R M.2031-0 (12/2012), Characteristics and protection criteria of receiving earth stations and characteristics of transmitting space stations of the radionavigation-satellite service (space-to-Earth) operating in the band 5 010-5 030 MHz",
        url: "https://www.itu.int/dms_pubrec/itu-r/rec/m/R-REC-M.2031-0-201212-S!!PDF-E.pdf",
        read: "Read 2026-11-21: recommends 1 and 2; Annex 1, Table 1-1 (tracking and acquisition interference thresholds at the passive antenna output); Annex 2 thresholds (Table 2-4) noted.",
    },
    Source {
        key: "DHS-RPCF",
        citation: "US DHS, Resilient Positioning, Navigation, and Timing (PNT) framework (RPCF), Version 2.0, 26 April 2022 (resilience levels 1 to 4 and architecture expectations)",
        url: "https://www.dhs.gov/sites/default/files/2022-05/22_0531_st_resilient_pnt_conformance_framework_v2.0.pdf",
        read: "As recorded in report-templates/frameworks/dhs-rpcf.json (document read in full for the open engine's mapping).",
    },
    Source {
        key: "NIST-IR-8323r1",
        citation: "NIST IR 8323r1, Foundational PNT Profile: Applying the Cybersecurity Framework for the Responsible Use of PNT Services, January 2023",
        url: "https://nvlpubs.nist.gov/nistpubs/ir/2023/NIST.IR.8323r1.pdf",
        read: "As recorded in report-templates/frameworks/nist-ir-8323r1.json (Profile tables 3 to 25 read for the subcategories cited).",
    },
];

/// How a step relates to a clause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    /// The step measures the quantity the clause sets a requirement on.
    MeasuresSameQuantity,
    /// The step measures a related quantity; the clause's own test is different.
    Related,
    /// The step's result is evidence a reviewer can weigh for the expectation.
    EvidenceFor,
    /// The cited document sets no requirement on what the step measures.
    NoRequirementInSource,
    /// No public requirement was found or read.
    NoneFound,
    /// A clause that no published procedure exercises.
    Gap,
}

/// One row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    /// `procedure/step`, or `-` for a gap row.
    pub step: &'static str,
    /// `L`, `C` or `L+C`.
    pub band: &'static str,
    /// Source key, or [`TO_CONFIRM`].
    pub source: &'static str,
    pub clause: &'static str,
    /// The requirement or expectation, paraphrased.
    pub requirement: &'static str,
    pub relation: Relation,
    pub note: &'static str,
}

const fn row(
    step: &'static str,
    band: &'static str,
    source: &'static str,
    clause: &'static str,
    requirement: &'static str,
    relation: Relation,
    note: &'static str,
) -> Row {
    Row {
        step,
        band,
        source,
        clause,
        requirement,
        relation,
        note,
    }
}

use Relation::*;

/// The cross-walk.
pub const ROWS: &[Row] = &[
    row("leo-doppler-stress/static-reference", "L", "EN303413", "4.2.1.2, Annex C",
        "The C/N0 the receiver reports for each supported signal shall not degrade by more than 1 dB under the blocking signals of tables 4-2 and 4-3.",
        Related, "The reference run is the unstressed baseline; EN 303 413 measures C/N0 degradation against its own baseline, this step horizontal error. Record C/N0 in the run to make the two comparable."),
    row("leo-doppler-stress/static-reference", "C", "ITU-R-M.2031", "Annex 1, Table 1-1",
        "Aggregate interference thresholds at the passive antenna output for tracking (-157.1 dBW narrow-band, -147.1 dB(W/MHz) wideband) and acquisition (-160.1 dBW, -150.1 dB(W/MHz)).",
        Related, "Protection criteria for spectrum studies, not an equipment requirement; the reference run fixes the baseline against which a run at these levels is compared."),
    row("leo-doppler-stress/doppler-max", "L+C", TO_CONFIRM, "-",
        "Receiver acquisition and tracking under LEO Doppler.",
        NoneFound, "No public requirement on LEO Doppler dynamics for PNT user equipment was found; EN 303 413 V1.2.1 clause 4.2 holds only receiver blocking and spurious emissions."),
    row("leo-doppler-stress/doppler-rate-max", "L+C", TO_CONFIRM, "-",
        "Tracking at the largest Doppler rate.", NoneFound, "As for doppler-max."),
    row("leo-doppler-stress/doppler-rate-max-accuracy", "L+C", TO_CONFIRM, "-",
        "Horizontal accuracy under high Doppler rate.", NoneFound, "As for doppler-max."),
    row("leo-acquisition/cold-start", "L", "EN303413", "4.2",
        "Receiver blocking and receiver spurious emissions only.",
        NoRequirementInSource, "EN 303 413 V1.2.1 sets no time-to-first-fix requirement."),
    row("leo-acquisition/cold-start", "C", "ITU-R-M.2031", "Annex 1, Table 1-1",
        "Acquisition-mode interference thresholds are 3 dB below the tracking-mode thresholds.",
        Related, "Acquisition is the more interference-sensitive mode in the protection criteria; a cold start under interference at the acquisition-mode levels exercises it. The recommendation sets no time to first fix."),
    row("leo-acquisition/warm-start", "L+C", TO_CONFIRM, "-",
        "Time to first fix with almanac and coarse time.", NoneFound,
        "No public time-to-first-fix requirement for LEO-PNT user equipment was found."),
    row("leo-acquisition/steady-state", "L+C", TO_CONFIRM, "-",
        "Open-sky horizontal accuracy.", NoneFound,
        "Accuracy requirements sit in application standards (aviation, maritime, timing) not read for this table."),
    row("leo-acquisition/steady-state-time", "L+C", "NIST-IR-8323r1", "Protect (PR.DS, PR.IP)",
        "Protect PNT data integrity and plan for diverse sources and holdover.",
        EvidenceFor, "The receiver clock error in steady state is input to a holdover plan; the Profile sets no number."),
    row("leo-signal-profiles/profile-nominal", "L+C", TO_CONFIRM, "-",
        "Performance with the signal profile as described.", NoneFound,
        "No public profile-level requirement for LEO-PNT signals was found."),
    row("leo-signal-profiles/profile-low-power", "L", "EN303413", "4.2.1.2, tables 4-2 and 4-3",
        "C/N0 degradation of at most 1 dB under adjacent-band blocking signals at stated powers (e.g. -95 dBm at 1 548 MHz).",
        Related, "Lower received power and adjacent-band blocking both erode C/N0 margin; the low-power step is not the blocking test."),
    row("leo-signal-profiles/profile-low-power", "C", "ITU-R-M.2031", "Annex 1, Table 1-1",
        "Tracking-mode aggregate wideband interference threshold -147.1 dB(W/MHz) at the passive antenna output.",
        Related, "A low-power run sets the margin that these interference levels consume."),
    row("leo-signal-profiles/profile-few-satellites", "L+C", TO_CONFIRM, "-",
        "Accuracy with the fewest satellites the profile allows.", NoneFound, "No public requirement found."),
    row("leo-signal-profiles/profile-with-gnss", "L+C", "DHS-RPCF", "Architecture expectations (diversity, verification, recovery)",
        "Use diverse, cross-checked sources and verify PNT data before use.",
        EvidenceFor, "A LEO-plus-GNSS run is evidence on source diversity; the framework's levels are assessed under threat, not in a nominal run."),
    row("leo-signal-profiles/profile-with-gnss", "L+C", "NIST-IR-8323r1", "Protect (PR.DS, PR.IP)",
        "Plan for diverse sources.", EvidenceFor, "As above; the Profile sets no number."),
    row("-", "L", "EN303413", "4.2.2",
        "Receiver spurious emissions limits.",
        Gap, "No published LEO procedure measures receiver spurious emissions; it is an equipment emission test for a test house."),
    row("-", "L", "EN303413", "4.2.1, tables 4-2 and 4-3",
        "Adjacent-band receiver blocking at stated frequencies and powers.",
        Gap, "No published LEO procedure runs the blocking test itself."),
    row("-", "C", TO_CONFIRM, "Radio Regulations No. 5.328B and No. 5.443B",
        "Conditions on RNSS use of 5 010-5 030 MHz and an aggregate pfd limit on RNSS space stations to protect MLS above 5 030 MHz.",
        Gap, "Read only in secondary sources (national allocation tables); to confirm against the Radio Regulations text. The pfd limit applies to space stations, not user equipment."),
];

/// The cross-walk document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Crosswalk {
    pub format: &'static str,
    pub wording: &'static str,
    pub procedures: Vec<String>,
    pub sources: Vec<Source>,
    pub rows: Vec<Row>,
    /// Steps of the published procedures with at least one row with a cited source.
    pub steps_cited: usize,
    pub steps_to_confirm_only: Vec<String>,
}

/// Every `procedure/step` of the published procedures.
pub fn published_steps() -> Result<Vec<String>, String> {
    let mut out = vec![];
    for (id, text) in PUBLISHED {
        let p: Procedure = toml::from_str(text).map_err(|e| format!("{id}: {e}"))?;
        out.extend(p.steps.iter().map(|s| format!("{}/{}", p.id, s.id)));
    }
    Ok(out)
}

/// Build the cross-walk.
pub fn build() -> Result<Crosswalk, String> {
    let steps = published_steps()?;
    let mut cited = 0;
    let mut only = vec![];
    for s in &steps {
        let rs: Vec<&Row> = ROWS.iter().filter(|r| r.step == s).collect();
        if rs.is_empty() {
            return Err(format!("published step {s} has no cross-walk row"));
        }
        if rs.iter().any(|r| r.source != TO_CONFIRM) {
            cited += 1;
        } else {
            only.push(s.clone());
        }
    }
    for r in ROWS {
        if r.step != "-" && !steps.iter().any(|s| s == r.step) {
            return Err(format!("row for unknown step {}", r.step));
        }
        if r.source != TO_CONFIRM && !SOURCES.iter().any(|s| s.key == r.source) {
            return Err(format!("row cites unknown source {}", r.source));
        }
    }
    Ok(Crosswalk {
        format: FORMAT,
        wording: WORDING,
        procedures: PUBLISHED.iter().map(|p| p.0.to_string()).collect(),
        sources: SOURCES.to_vec(),
        rows: ROWS.to_vec(),
        steps_cited: cited,
        steps_to_confirm_only: only,
    })
}

/// The cross-walk as Markdown (the published `docs/leo-procedures/UE-REQUIREMENTS-CROSSWALK.md`).
pub fn markdown(c: &Crosswalk) -> String {
    let mut s = String::from(
        "# LEO-PNT procedures: requirements cross-walk for user equipment (L-band and C-band)\n\n\
Generated by `kshana-pro leo-eval crosswalk --markdown`; a test keeps this file in step with the code.\n\n",
    );
    s += c.wording;
    s += "\n\n| step | band | source | clause | requirement (paraphrased) | relation | note |\n|---|---|---|---|---|---|---|\n";
    for r in &c.rows {
        let rel = serde_json::to_value(r.relation)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        s += &format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            r.step, r.band, r.source, r.clause, r.requirement, rel, r.note
        );
    }
    s += &format!(
        "\n{} of {} published steps have a row with a cited source; to confirm only: {}.\n\n## Sources\n\n",
        c.steps_cited,
        c.steps_cited + c.steps_to_confirm_only.len(),
        if c.steps_to_confirm_only.is_empty() {
            "none".to_string()
        } else {
            c.steps_to_confirm_only.join(", ")
        }
    );
    for src in &c.sources {
        s += &format!(
            "* **{}**: {}. <{}>. {}\n",
            src.key, src.citation, src.url, src.read
        );
    }
    s += "\nNot cited (not public): RTCA, EUROCAE and IEC equipment standards. Rows that would need them say \"to confirm\".\n";
    s
}
