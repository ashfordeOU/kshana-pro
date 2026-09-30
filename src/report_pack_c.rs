// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Pack-C study report — the scenario-subscription CI study section
//!
//! Renders a [`CiReport`](crate::pack_c::run::CiReport) (the scored output of a
//! Pack-C CI run) as a branded, self-contained HTML face that reuses the same brand
//! furniture ([`crate::report_brand`]) and the same cover / provenance copy types
//! ([`crate::report::StudyCover`] / [`crate::report::StudyProvenance`]) as the rest
//! of the study line, so a Pack-C deliverable looks identical in hand.
//!
//! The headline is the **CI verdict**: an informational run when no baseline was
//! supplied, a PASS when the regression gate held, or a count of REGRESSIONS when
//! it did not. Below it sit the reproducibility provenance (pack id + version +
//! content hash, engine version, scenario-hash count), the full sorted metrics
//! table (with the signed Δ on any regressed metric), and — when the verdict found
//! regressions — a dedicated regressions sub-table.
//!
//! ## Honesty (THE MOAT — non-optional)
//! The MODELLED banner and the literal "not a certification" disclaimer are ALWAYS
//! present, and the verdict is framed as **decision-support for the customer's own
//! V&V**, never a certification of fitness. The pack identity + content hash, the
//! engine version, and the scenario hashes are surfaced for reproducibility.
//!
//! ## Determinism
//! The renderer reads no clock and no randomness; it iterates metrics via the sorted
//! [`MetricSet`](crate::pack_c::metrics::MetricSet) (a `BTreeMap`), so the HTML is
//! byte-identical across runs for the same [`CiReport`].

use crate::pack_c::run::CiReport;
use crate::report::{StudyCover, StudyProvenance};
use crate::report_brand as brand;

/// A warning-red used for the FAIL verdict banner and regression marks (defined
/// inline here — the brand palette carries no alarm colour).
const WARN_RED: &str = "#b3402f";

/// A full Pack-C study report face: cover, the always-on MODELLED banner, the CI
/// verdict + provenance + metrics (and, when present, the regressions table), an
/// optional provenance block, and the seal footer.
#[derive(Debug, Clone)]
pub struct PackCReport {
    /// Study title (rendered in the cover).
    pub title: String,
    /// The scored CI report this study renders.
    pub report: CiReport,
    /// Optional branded cover copy (sub-title + executive summary).
    pub cover: Option<StudyCover>,
    /// Optional audit-grade provenance block.
    pub provenance: Option<StudyProvenance>,
}

impl PackCReport {
    /// A Pack-C report over a scored [`CiReport`] (clones the report; no cover or
    /// provenance attached).
    pub fn from_ci(title: impl Into<String>, report: &CiReport) -> Self {
        Self {
            title: title.into(),
            report: report.clone(),
            cover: None,
            provenance: None,
        }
    }

    /// Attach branded cover copy.
    pub fn with_cover(mut self, cover: StudyCover) -> Self {
        self.cover = Some(cover);
        self
    }

    /// Attach an audit-grade provenance block.
    pub fn with_provenance(mut self, prov: StudyProvenance) -> Self {
        self.provenance = Some(prov);
        self
    }

    /// The resolved engine-version string for display (the report's value, or "n/a").
    fn engine_version(&self) -> String {
        self.report
            .engine_version
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "n/a".to_string())
    }

    /// The resolved handling classification (caller-supplied or the conservative
    /// [`DEFAULT_CLASSIFICATION`](crate::report::DEFAULT_CLASSIFICATION)).
    fn classification(&self) -> String {
        self.provenance
            .as_ref()
            .and_then(|p| p.classification.clone())
            .unwrap_or_else(|| crate::report::DEFAULT_CLASSIFICATION.to_string())
    }

    /// The cover sub-title (caller-supplied or an honest default).
    fn subtitle(&self) -> String {
        self.cover
            .as_ref()
            .and_then(|c| c.subtitle.clone())
            .unwrap_or_else(|| "Scenario-Subscription CI Study".to_string())
    }

    /// The MODELLED-banner detail clause for the Pack-C face.
    fn banner_detail(&self) -> String {
        "the CI verdict is decision-support for your own V&V over MODELLED scenario \
         runs, never a certification of fitness."
            .to_string()
    }

    /// The verdict banner: an informational run when no baseline was gated against, a
    /// PASS when the gate held, or a REGRESSIONS count when it did not. Pass and fail
    /// are visually distinct (brass-on-ink vs warning-red-on-ivory).
    fn verdict_banner_html(&self) -> String {
        if let Some(w) = self.report.watch.as_ref().filter(|w| w.watched) {
            let failed = w.outcomes.iter().filter(|o| o.failed()).count();
            let (bg, fg, text) = if failed == 0 {
                (
                    brand::BRASS,
                    brand::INK,
                    format!("PASS &mdash; {} watch(es) hold", w.outcomes.len()),
                )
            } else {
                (
                    WARN_RED,
                    brand::IVORY_LIGHT,
                    format!("WATCH FAILURES: {failed} of {}", w.outcomes.len()),
                )
            };
            return format!(
                "<div style=\"background:{bg};color:{fg};font-family:{head};font-weight:700;\
                 font-size:22px;text-align:center;padding:14px 16px;border-radius:4px;\
                 margin:18px 0;letter-spacing:.06em\">CI Verdict &mdash; {text}</div>",
                head = brand::FONT_HEAD,
            );
        }
        let (bg, fg, text) = match &self.report.verdict {
            None => (
                brand::NAVY_PANEL,
                brand::IVORY,
                "Informational run (no baseline)".to_string(),
            ),
            Some(v) if v.passed => (brand::BRASS, brand::INK, "PASS".to_string()),
            Some(v) => (
                WARN_RED,
                brand::IVORY_LIGHT,
                format!("REGRESSIONS: {}", v.regressions.len()),
            ),
        };
        format!(
            "<div style=\"background:{bg};color:{fg};font-family:{head};font-weight:700;\
             font-size:22px;text-align:center;padding:14px 16px;border-radius:4px;\
             margin:18px 0;letter-spacing:.06em\">CI Verdict &mdash; {text}</div>",
            head = brand::FONT_HEAD,
            text = xml(&text),
        )
    }

    /// A single-line reproducibility provenance summary: pack id + version, the short
    /// (first-12-char) pack hash, the engine version (or "n/a"), and the count of
    /// scenario hashes.
    fn provenance_line_html(&self) -> String {
        let short_hash: String = self.report.pack_hash.chars().take(12).collect();
        let line = format!(
            "Pack {id} v{ver} · hash {hash} · engine {engine} · {n} scenario hash(es)",
            id = self.report.pack_id,
            ver = self.report.pack_version,
            hash = short_hash,
            engine = self.engine_version(),
            n = self.report.scenario_hashes.len(),
        );
        format!(
            "<p style=\"font-family:{body};font-size:13px;color:{bm};margin:6px 0 0;\
             letter-spacing:.02em\">{line}</p>",
            body = brand::FONT_BODY,
            bm = brand::BRASS_MUTED,
            line = xml(&line),
        )
    }

    /// The signed relative-change as a percent string (e.g. `+12.3%` / `-4.0%`).
    fn rel_pct(rel_change: f64) -> String {
        format!("{:+.1}%", rel_change * 100.0)
    }

    /// The metrics table: every current metric (key | value) in sorted order; a
    /// regressed metric is marked and carries its signed Δ (baseline → current).
    fn metrics_table_html(&self) -> String {
        // Map regressed key -> its Regression (for the Δ annotation).
        let regressions: std::collections::BTreeMap<&str, &crate::pack_c::gate::Regression> = self
            .report
            .verdict
            .as_ref()
            .map(|v| v.regressions.iter().map(|r| (r.key.as_str(), r)).collect())
            .unwrap_or_default();

        let mut rows = String::new();
        for (key, value) in self.report.metrics.0.iter() {
            let delta = match regressions.get(key.as_str()) {
                Some(r) => format!(
                    "&nbsp;<span style=\"font-family:{body};font-size:11px;font-weight:700;\
                     letter-spacing:.04em;color:{ivl};background:{warn};padding:1px 6px;\
                     border-radius:3px\">REGRESSED {b} &rarr; {c} ({pct})</span>",
                    body = brand::FONT_BODY,
                    ivl = brand::IVORY_LIGHT,
                    warn = WARN_RED,
                    b = fmt_num(r.baseline),
                    c = fmt_num(r.current),
                    pct = xml(&Self::rel_pct(r.rel_change)),
                ),
                None => String::new(),
            };
            rows.push_str(&format!(
                "<tr style=\"border-top:1px solid {bm}\">\
                 <td style=\"padding:7px 12px 7px 0;color:{iv}\">{key}</td>\
                 <td style=\"padding:7px 0;color:{ivl};font-weight:700;white-space:nowrap\">{val}{delta}</td></tr>",
                bm = brand::BRASS_MUTED,
                iv = brand::IVORY,
                ivl = brand::IVORY_LIGHT,
                key = xml(key),
                val = fmt_num(*value),
            ));
        }
        if rows.is_empty() {
            rows.push_str(&format!(
                "<tr><td colspan=\"2\" style=\"padding:7px 0;color:{iv};opacity:.8\">\
                 No metrics collected.</td></tr>",
                iv = brand::IVORY,
            ));
        }
        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:22px;font-weight:700;\
             border-bottom:1px solid {bm};padding-bottom:6px\">Figures of Merit</h2>\
             <table style=\"border-collapse:collapse;width:100%;font-family:{body};font-size:15px\">\
             <thead><tr style=\"text-align:left;color:{brass}\">\
             <th style=\"padding:0 12px 6px 0\">Metric</th>\
             <th style=\"padding:0 0 6px\">Value</th></tr></thead>\
             <tbody>{rows}</tbody></table></section>",
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
        )
    }

    /// The regressions sub-table (key | baseline | current | Δ% | threshold%), only
    /// rendered when the verdict found at least one regression.
    fn regressions_table_html(&self) -> String {
        let regressions = match &self.report.verdict {
            Some(v) if !v.regressions.is_empty() => &v.regressions,
            _ => return String::new(),
        };
        let mut rows = String::new();
        for r in regressions {
            rows.push_str(&format!(
                "<tr style=\"border-top:1px solid {bm}\">\
                 <td style=\"padding:7px 12px 7px 0;color:{iv}\">{key}</td>\
                 <td style=\"padding:7px 12px;color:{ivl};white-space:nowrap\">{b}</td>\
                 <td style=\"padding:7px 12px;color:{ivl};white-space:nowrap\">{c}</td>\
                 <td style=\"padding:7px 12px;color:{ivl};white-space:nowrap\">{pct}</td>\
                 <td style=\"padding:7px 0;color:{iv}\">{thr}</td></tr>",
                bm = brand::BRASS_MUTED,
                iv = brand::IVORY,
                ivl = brand::IVORY_LIGHT,
                key = xml(&r.key),
                b = fmt_num(r.baseline),
                c = fmt_num(r.current),
                pct = xml(&Self::rel_pct(r.rel_change)),
                thr = xml(&Self::rel_pct(r.threshold)),
            ));
        }
        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{warn};font-size:22px;font-weight:700;\
             border-bottom:1px solid {warn};padding-bottom:6px\">Regressions</h2>\
             <table style=\"border-collapse:collapse;width:100%;font-family:{body};font-size:15px\">\
             <thead><tr style=\"text-align:left;color:{brass}\">\
             <th style=\"padding:0 12px 6px 0\">Metric</th>\
             <th style=\"padding:0 12px 6px\">Baseline</th>\
             <th style=\"padding:0 12px 6px\">Current</th>\
             <th style=\"padding:0 12px 6px\">&Delta;%</th>\
             <th style=\"padding:0 0 6px\">Threshold%</th></tr></thead>\
             <tbody>{rows}</tbody></table></section>",
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            warn = WARN_RED,
        )
    }

    /// The campaign-watch table: one row per watch with its value, baseline, change,
    /// status, requirement verdict and statistical verdict, then the statistical test,
    /// the figure rule and the `--exact` comparison. Empty for a run without watches.
    fn watch_section_html(&self) -> String {
        let Some(w) = &self.report.watch else {
            return String::new();
        };
        let cell = |t: String| {
            format!(
                "<td style=\"padding:7px 10px 7px 0;color:{iv};vertical-align:top\">{t}</td>",
                iv = brand::IVORY
            )
        };
        let num = |v: Option<f64>, u: &Option<String>| match v {
            Some(x) => format!(
                "{}{}",
                fmt_num(x),
                u.as_ref()
                    .filter(|u| !u.is_empty() && u.as_str() != "1")
                    .map(|u| format!(" {}", xml(u)))
                    .unwrap_or_default()
            ),
            None => "&mdash;".into(),
        };
        let mut rows = String::new();
        for o in &w.outcomes {
            let colour = if o.failed() { WARN_RED } else { brand::BRASS };
            let req = match &o.requirement {
                Some(r) => format!(
                    "<span style=\"color:{c};font-weight:700\">{v}</span><br><code>{crit}</code>{why}",
                    c = if r.verdict == "met" { brand::BRASS } else { WARN_RED },
                    v = xml(&r.verdict),
                    crit = xml(&r.criterion),
                    why = r
                        .reason
                        .as_ref()
                        .map(|x| format!("<br>{}", xml(x)))
                        .unwrap_or_default()
                ),
                None => "&mdash;".into(),
            };
            let stat = match &o.stat {
                Some(sv) => format!(
                    "mean p = {:.3e}; spread F = {:.3}, p = {:.3e}; alpha {}",
                    sv.p_mean, sv.f, sv.p_spread, sv.alpha
                ),
                None => "&mdash;".into(),
            };
            let change = match (o.change, o.rel_change) {
                (Some(c), Some(r)) => format!("{} ({})", fmt_num(c), xml(&Self::rel_pct(r))),
                (Some(c), None) => fmt_num(c),
                _ => "&mdash;".into(),
            };
            rows.push_str(&format!(
                "<tr style=\"border-top:1px solid {bm}\">{}{}{}{}{}{}{}{}</tr>",
                cell(format!(
                    "{}<br><span style=\"font-size:12px;opacity:.8\">{} &middot; {}</span>",
                    xml(&o.id),
                    xml(&o.metric),
                    xml(&o.direction.describe())
                )),
                cell(format!(
                    "{}<br><span style=\"font-size:12px\">{}</span>",
                    xml(&o.entry),
                    xml(&o.evidence)
                )),
                cell(num(o.value, &o.unit)),
                cell(num(o.baseline, &o.unit)),
                cell(change),
                cell(format!(
                    "<span style=\"color:{colour};font-weight:700\">{}</span>{}",
                    xml(&o.status),
                    o.reason
                        .as_ref()
                        .filter(|_| o.stat.is_none())
                        .map(|r| format!("<br><span style=\"font-size:12px\">{}</span>", xml(r)))
                        .unwrap_or_default()
                )),
                cell(req),
                cell(stat),
                bm = brand::BRASS_MUTED,
            ));
        }
        let mut notes = String::new();
        if let Some(t) = &w.stat_test {
            notes.push_str(&format!("<p>Statistical gate: {}.</p>", xml(t)));
        }
        notes.push_str(&format!("<p>Figures: {}.</p>", xml(&w.figures_rule)));
        if !w.fallback_entries.is_empty() {
            notes.push_str(&format!(
                "<p>No units block, every numeric leaf taken: {}.</p>",
                xml(&w.fallback_entries.join(", "))
            ));
        }
        if let Some(ex) = &w.exact {
            notes.push_str(&if ex.compared {
                format!(
                    "<p>Exact comparison of member result SHA-256 (Secure Hash Algorithm, 256-bit) digests: {} changed{}; {} unchanged.</p>",
                    ex.changed.len(),
                    if ex.changed.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", xml(&ex.changed.join(", ")))
                    },
                    ex.unchanged.len()
                )
            } else {
                "<p>Exact comparison requested, but the baseline carries no member hashes.</p>"
                    .into()
            });
        }
        let table = if w.outcomes.is_empty() {
            String::new()
        } else {
            format!(
                "<table style=\"border-collapse:collapse;width:100%;font-family:{body};font-size:14px\">\
                 <thead><tr style=\"text-align:left;color:{brass}\">\
                 <th>Watch</th><th>Entry</th><th>Value</th><th>Baseline</th><th>Change</th>\
                 <th>Status</th><th>Requirement</th><th>Statistics</th></tr></thead><tbody>{rows}</tbody></table>",
                body = brand::FONT_BODY,
                brass = brand::BRASS,
            )
        };
        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:22px;font-weight:700;\
             border-bottom:1px solid {bm};padding-bottom:6px\">Campaign Watch</h2>{table}\
             <div style=\"font-family:{body};font-size:13px;color:{bm};margin-top:10px\">{notes}</div></section>",
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
        )
    }

    /// The inline MODELLED / "not a certification" note carried inside the section
    /// fragment, so the honesty framing travels with the section even when embedded.
    fn modelled_note_html(&self) -> String {
        format!(
            "<p style=\"font-family:{body};font-size:13px;color:{bm};font-style:italic;\
             margin:14px 0 0\">MODELLED &mdash; not a certification; decision-support for \
             your own V&amp;V.</p>",
            body = brand::FONT_BODY,
            bm = brand::BRASS_MUTED,
        )
    }

    /// The optional executive-summary block (only when a cover summary is supplied).
    fn exec_summary_html(&self) -> String {
        let summary = match self
            .cover
            .as_ref()
            .and_then(|c| c.executive_summary.clone())
        {
            Some(s) if !s.is_empty() => s,
            _ => return String::new(),
        };
        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:22px;font-weight:700;\
             border-bottom:1px solid {bm};padding-bottom:6px\">Executive Summary</h2>\
             <p style=\"font-family:{body};font-size:16px;line-height:1.6;color:{iv}\">{summary}</p>\
             </section>",
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
            iv = brand::IVORY,
            summary = xml(&summary),
        )
    }

    /// The audit-grade provenance block (rendered only when a [`StudyProvenance`] is
    /// attached). Reuses the consolidated/Pack-A provenance rendering style.
    fn provenance_html(&self) -> String {
        let Some(p) = self.provenance.clone() else {
            return String::new();
        };
        let dash = "&mdash;".to_string();
        let cell = |v: Option<String>| {
            v.filter(|s| !s.is_empty())
                .map(|s| xml(&s))
                .unwrap_or_else(|| dash.clone())
        };
        let row = |label: &str, value: String| {
            format!(
                "<tr><th style=\"text-align:left;font-family:{body};font-weight:700;color:{brass};\
                 padding:4px 14px 4px 0;white-space:nowrap;vertical-align:top\">{label}</th>\
                 <td style=\"font-family:{body};color:{iv};padding:4px 0\">{value}</td></tr>",
                body = brand::FONT_BODY,
                brass = brand::BRASS,
                iv = brand::IVORY,
            )
        };
        // Prefer the provenance hashes; otherwise fall back to the report's own.
        let hashes_src = if p.scenario_hashes.is_empty() {
            self.report.scenario_hashes.clone()
        } else {
            p.scenario_hashes.clone()
        };
        let hashes = if hashes_src.is_empty() {
            dash.clone()
        } else {
            xml(&hashes_src.join(", "))
        };
        // Prefer the provenance engine version; otherwise the report's resolved one.
        let engine = p
            .engine_version
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.engine_version());
        format!(
            "<section style=\"margin:22px 0;background:{panel};border:1px solid {bm};\
             border-radius:4px;padding:16px 20px\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:20px;font-weight:700;margin-top:0\">\
             Provenance &amp; Traceability</h2>\
             <table style=\"border-collapse:collapse;font-size:14px\">{rows}</table></section>",
            panel = brand::NAVY_PANEL,
            head = brand::FONT_HEAD,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
            rows = [
                row("Study ID", cell(p.study_id)),
                row("Customer", cell(p.customer.or_else(|| self.report.customer.clone()))),
                row("Prepared for", cell(p.prepared_for)),
                row("Prepared (UTC)", cell(p.prepared_utc.or_else(|| self.report.generated_utc.clone()))),
                row("Pack", xml(&format!("{} v{}", self.report.pack_id, self.report.pack_version))),
                row("Pack hash", xml(&self.report.pack_hash)),
                row("Engine version", xml(&engine)),
                row("Scenario hash(es)", hashes),
                row("Classification", xml(&self.classification())),
            ]
            .concat(),
        )
    }

    /// The Pack-C *section* fragment — verdict banner, provenance line, metrics
    /// table, the regressions sub-table (when present), and the inline MODELLED /
    /// "not a certification" note — WITHOUT the document wrapper, the cover, or the
    /// top MODELLED banner. This is what a multi-pack composer
    /// ([`crate::pro_study::ProStudy`]) embeds so the cover and the MODELLED banner
    /// render exactly ONCE for the whole study.
    pub fn section_fragment_html(&self) -> String {
        format!(
            "<section style=\"margin:22px 0\">{verdict}{prov_line}{watch}{metrics}{regressions}{note}</section>",
            verdict = self.verdict_banner_html(),
            prov_line = self.provenance_line_html(),
            watch = self.watch_section_html(),
            metrics = self.metrics_table_html(),
            regressions = self.regressions_table_html(),
            note = self.modelled_note_html(),
        )
    }

    /// The self-contained, branded Pack-C HTML study document. No external fetches
    /// (offline/PDF-safe): brand tokens and the seal are inlined. The MODELLED banner
    /// and the "not a certification" disclaimer are always present.
    pub fn to_html(&self) -> String {
        // Every Kshana Pro document is an Ashforde document: the shared furniture of
        // crate::report_brand goes around the renderer's own page, which is unchanged.
        let body = self.to_html_unbranded();
        let id = crate::report_brand::content_id(&body);
        crate::report_brand::finish_html(
            &body,
            &crate::report_brand::DocMark::new("Scenario-subscription report", id),
        )
    }

    fn to_html_unbranded(&self) -> String {
        format!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>{title}</title></head>\
             <body style=\"background:{navy};color:{ivory};font-family:{body};\
             max-width:1000px;margin:auto;padding:24px 32px 48px\">\
             {cover}{banner}{summary}{section}{provenance}\
             <footer style=\"text-align:center;font-family:{body};font-size:12px;color:{bm};\
             border-top:1px solid {bm};margin-top:24px;padding-top:14px\">\
             {seal}<div style=\"margin-top:10px\">Ashforde O&Uuml; &middot; {classification} &middot; \
             MODELLED engineering study &mdash; not a certification; decision-support for your \
             own V&amp;V.</div></footer>\
             </body></html>",
            title = xml(&self.title),
            navy = brand::NAVY,
            ivory = brand::IVORY,
            body = brand::FONT_BODY,
            bm = brand::BRASS_MUTED,
            cover = brand::cover_html(&self.title, &self.subtitle()),
            banner = brand::modelled_banner_html(&self.banner_detail()),
            summary = self.exec_summary_html(),
            section = self.section_fragment_html(),
            provenance = self.provenance_html(),
            seal = brand::seal_img_html(56),
            classification = xml(&self.classification()),
        )
    }
}

/// Format a metric value compactly but deterministically: integers without a
/// fractional part render whole; everything else uses a fixed 4 decimals so the
/// HTML is byte-stable for equal inputs.
///
/// Non-finite inputs render as Rust's default (`"NaN"` / `"inf"`); callers are
/// expected to pass finite values (metrics come from valid JSON, so this holds).
fn fmt_num(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v:.4}")
    }
}

/// Minimal XML/HTML escaping for text interpolated into the report.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack_c::gate::{Direction, GateVerdict, Regression};
    use crate::pack_c::metrics::MetricSet;
    use std::collections::BTreeMap;

    fn metric_set(pairs: &[(&str, f64)]) -> MetricSet {
        MetricSet(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    /// A no-verdict (informational) report with a couple of metrics.
    fn informational_report() -> CiReport {
        CiReport {
            pack_id: "resilience-baseline".to_string(),
            pack_version: "1.2.0".to_string(),
            pack_hash: "abcdef0123456789deadbeef".to_string(),
            engine_version: Some("0.20.0".to_string()),
            scenario_hashes: vec!["hash-a".to_string(), "hash-b".to_string()],
            generated_utc: Some("2026-01-01T00:00:00Z".to_string()),
            customer: Some("ACME".to_string()),
            label: crate::pack_c::run::CI_LABEL.to_string(),
            metrics: metric_set(&[
                ("araim::fom.raim_availability", 0.95),
                ("araim::fom.mean_hpl_m", 22.5),
            ]),
            verdict: None,
            watch: None,
        }
    }

    /// A passing-verdict report (empty regressions).
    fn pass_report() -> CiReport {
        let mut r = informational_report();
        r.verdict = Some(GateVerdict {
            passed: true,
            regressions: Vec::new(),
            improvements: vec!["araim::fom.raim_availability".to_string()],
            informational: Vec::new(),
        });
        r
    }

    /// A failing-verdict report carrying one regression on a known metric.
    fn regressed_report() -> CiReport {
        let mut r = informational_report();
        r.metrics = metric_set(&[("araim::fom.raim_availability", 0.70)]);
        r.verdict = Some(GateVerdict {
            passed: false,
            regressions: vec![Regression {
                key: "araim::fom.raim_availability".to_string(),
                baseline: 0.90,
                current: 0.70,
                rel_change: -0.2222,
                threshold: 0.05,
                direction: Direction::HigherIsBetter,
            }],
            improvements: Vec::new(),
            informational: Vec::new(),
        });
        r
    }

    #[test]
    fn no_verdict_html_carries_title_modelled_version_and_short_hash() {
        let report = informational_report();
        let html = PackCReport::from_ci("Pack C — CI Study", &report).to_html();
        assert!(html.contains("Pack C &mdash; CI Study") || html.contains("Pack C — CI Study"));
        assert!(html.contains("MODELLED"));
        assert!(html.contains("not a certification"));
        // The pack version.
        assert!(html.contains("1.2.0"));
        // The short (first-12-char) pack hash, and NOT the full hash bleeding into
        // the provenance line.
        assert!(html.contains("abcdef012345"));
        // Informational framing present (no baseline).
        assert!(html.contains("Informational run"));
    }

    #[test]
    fn pass_verdict_html_says_pass() {
        let html = PackCReport::from_ci("Pack C", &pass_report()).to_html();
        assert!(html.contains("PASS"));
        // Honesty furniture survives a passing verdict.
        assert!(html.contains("not a certification"));
        assert!(html.contains("MODELLED"));
    }

    #[test]
    fn regression_html_renders_count_key_and_values() {
        let html = PackCReport::from_ci("Pack C", &regressed_report()).to_html();
        // The verdict word + count.
        assert!(html.contains("REGRESSION"));
        // The regressed key and its baseline + current values are rendered.
        assert!(html.contains("araim::fom.raim_availability"));
        assert!(html.contains("0.9000")); // baseline
        assert!(html.contains("0.7000")); // current
                                          // The signed Δ percent appears.
        assert!(html.contains("-22.2%"));
    }

    #[test]
    fn to_html_is_byte_identical_across_runs() {
        let report = regressed_report();
        let pc = PackCReport::from_ci("Determinism", &report);
        assert_eq!(pc.to_html(), pc.to_html());
    }

    #[test]
    fn section_fragment_has_metrics_table_and_modelled_note() {
        let report = informational_report();
        let fragment = PackCReport::from_ci("Pack C", &report).section_fragment_html();
        // The metrics table header + a metric key.
        assert!(fragment.contains("Figures of Merit"));
        assert!(fragment.contains("araim::fom.raim_availability"));
        // The inline MODELLED / not-a-certification note travels with the fragment.
        assert!(fragment.contains("MODELLED"));
        assert!(fragment.contains("not a certification"));
    }

    /// Cover + provenance compose like the rest of the study line, and the honesty
    /// furniture survives custom copy.
    #[test]
    fn cover_and_provenance_compose() {
        let html = PackCReport::from_ci("Pack C", &pass_report())
            .with_cover(StudyCover {
                subtitle: Some("Subscription Engagement".into()),
                executive_summary: Some("A bespoke executive summary line.".into()),
            })
            .with_provenance(StudyProvenance {
                study_id: Some("KP-2026-PACKC".into()),
                classification: Some("Commercial-in-Confidence".into()),
                ..Default::default()
            })
            .to_html();
        assert!(html.contains("Subscription Engagement"));
        assert!(html.contains("A bespoke executive summary line."));
        assert!(html.contains("KP-2026-PACKC"));
        assert!(html.contains("Commercial-in-Confidence"));
        assert!(html.contains("not a certification"));
        assert!(html.contains("MODELLED"));
    }
}
