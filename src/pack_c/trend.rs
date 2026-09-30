// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Trend report** — every watched figure over a verified run history, as a
//! self-contained HTML page with one inline SVG sparkline per figure, and as JSON.
//!
//! Each sparkline has one point per run in the history. A point from a run in which the
//! figure regressed or failed its requirement is drawn in red; requirement limits are
//! drawn as dashed horizontal lines; a change of engine version between two runs is
//! marked by a dotted vertical line labelled with the new version. Below the charts, one
//! row per run: its time stamp, engine version, verdict, the watches that failed, and the
//! first characters of its chain hash. A failed run's row is red.
//!
//! The page is built only from a history that [`crate::pack_c::history::verify`]
//! accepted; it states the number of records and the last hash, so a reader can check
//! it against the file. It reads no clock and no randomness, so equal histories give
//! byte-identical pages.

use crate::pack_c::history::ChainedRecord;
use serde_json::{json, Value};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fmt(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if (1e-3..1e6).contains(&a) {
        let s = format!("{v:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    } else {
        format!("{v:.4e}")
    }
}

/// Watch identities in first-seen order across the history.
fn figure_ids(records: &[ChainedRecord]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for r in records {
        for id in r.record.figures.keys() {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    ids
}

fn failed_status(p: &crate::pack_c::history::HistoryPoint) -> bool {
    matches!(p.status.as_str(), "regressed" | "not-found")
        || p.requirement
            .as_deref()
            .map(|r| r != "met")
            .unwrap_or(false)
}

/// The trend as JSON: the runs, and every watched figure with one point per run.
pub fn render_json(records: &[ChainedRecord], title: &str) -> String {
    let runs: Vec<Value> = records
        .iter()
        .map(|c| {
            json!({
                "index": c.record.index,
                "time_utc": c.record.time_utc,
                "engine_version": c.record.engine_version,
                "pack_hash": c.record.pack_hash,
                "verdict": c.record.verdict,
                "failures": c.record.failures,
                "hash": c.hash,
            })
        })
        .collect();
    let figures: Vec<Value> = figure_ids(records)
        .iter()
        .map(|id| {
            let points: Vec<Value> = records
                .iter()
                .map(|c| match c.record.figures.get(id) {
                    Some(p) => json!({
                        "index": c.record.index,
                        "value": p.value,
                        "status": p.status,
                        "requirement": p.requirement,
                        "failed": failed_status(p),
                    }),
                    None => {
                        json!({"index": c.record.index, "value": null, "status": "not watched"})
                    }
                })
                .collect();
            let last = records.iter().rev().find_map(|c| c.record.figures.get(id));
            json!({
                "id": id,
                "unit": last.and_then(|p| p.unit.clone()),
                "evidence": last.and_then(|p| p.evidence.clone()),
                "limits": last.map(|p| p.limits.clone()).unwrap_or_default(),
                "points": points,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "title": title,
        "label": TREND_LABEL,
        "records": records.len(),
        "last_hash": records.last().map(|c| c.hash.clone()),
        "runs": runs,
        "figures": figures,
    }))
    .unwrap_or_default()
}

/// The honesty label every campaign-watch trend document carries, in the open engine's
/// `label` form.
pub const TREND_LABEL: &str = "MODELLED - the trend of watched figures across a verified campaign-watch history: every point is simulation output of the open engine for one CI run, with its own evidence label; it is not a measurement series.";

const W: f64 = 340.0;
const H: f64 = 130.0;
const PAD_L: f64 = 12.0;
const PAD_R: f64 = 12.0;
const PAD_T: f64 = 22.0;
const PAD_B: f64 = 18.0;

fn sparkline(records: &[ChainedRecord], id: &str) -> String {
    let n = records.len();
    let pts: Vec<(usize, Option<f64>, bool)> = records
        .iter()
        .enumerate()
        .map(|(i, c)| match c.record.figures.get(id) {
            Some(p) => (i, p.value.filter(|v| v.is_finite()), failed_status(p)),
            None => (i, None, false),
        })
        .collect();
    let limits = records
        .iter()
        .rev()
        .find_map(|c| c.record.figures.get(id))
        .map(|p| p.limits.clone())
        .unwrap_or_default();
    let mut ys: Vec<f64> = pts.iter().filter_map(|p| p.1).collect();
    ys.extend(limits.iter().map(|l| l.value));
    if ys.is_empty() {
        return "<p class=\"muted\">No value in any run.</p>".into();
    }
    let (mut lo, mut hi) = ys
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    if hi - lo < 1e-12 * hi.abs().max(1.0) {
        let pad = if hi == 0.0 { 1.0 } else { hi.abs() * 0.05 };
        lo -= pad;
        hi += pad;
    } else {
        let pad = (hi - lo) * 0.12;
        lo -= pad;
        hi += pad;
    }
    let x = |i: usize| {
        if n <= 1 {
            W / 2.0
        } else {
            PAD_L + (W - PAD_L - PAD_R) * i as f64 / (n - 1) as f64
        }
    };
    let y = |v: f64| PAD_T + (H - PAD_T - PAD_B) * (hi - v) / (hi - lo);
    let mut svg = format!(
        "<svg class=\"spark\" viewBox=\"0 0 {W} {H}\" role=\"img\" aria-label=\"{} over {n} runs\">",
        esc(id)
    );
    // Engine-version boundaries.
    for i in 1..n {
        let (a, b) = (
            &records[i - 1].record.engine_version,
            &records[i].record.engine_version,
        );
        if a != b {
            let xb = (x(i - 1) + x(i)) / 2.0;
            svg.push_str(&format!(
                "<line class=\"engine\" x1=\"{xb:.1}\" y1=\"{PAD_T}\" x2=\"{xb:.1}\" y2=\"{:.1}\"/>\
                 <text class=\"engine-label\" x=\"{:.1}\" y=\"12\">engine {}</text>",
                H - PAD_B,
                xb + 3.0,
                esc(b.as_deref().unwrap_or("n/a"))
            ));
        }
    }
    // Requirement limits.
    for l in &limits {
        let yl = y(l.value);
        svg.push_str(&format!(
            "<line class=\"limit\" x1=\"{PAD_L}\" y1=\"{yl:.1}\" x2=\"{:.1}\" y2=\"{yl:.1}\"/>\
             <text class=\"limit-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">requirement {} {}</text>",
            W - PAD_R,
            W - PAD_R,
            yl - 4.0,
            esc(&l.op),
            fmt(l.value)
        ));
    }
    // The line, broken where a run has no value.
    let mut seg: Vec<String> = Vec::new();
    let flush = |seg: &mut Vec<String>, svg: &mut String| {
        if seg.len() > 1 {
            svg.push_str(&format!(
                "<polyline class=\"line\" points=\"{}\"/>",
                seg.join(" ")
            ));
        }
        seg.clear();
    };
    for (i, v, _) in &pts {
        match v {
            Some(v) => seg.push(format!("{:.1},{:.1}", x(*i), y(*v))),
            None => flush(&mut seg, &mut svg),
        }
    }
    flush(&mut seg, &mut svg);
    for (i, v, bad) in &pts {
        if let Some(v) = v {
            svg.push_str(&format!(
                "<circle class=\"{}\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"{}\"><title>run {}: {}</title></circle>",
                if *bad { "pt bad" } else { "pt" },
                x(*i),
                y(*v),
                if *bad { "5" } else { "3.5" },
                records[*i].record.index,
                fmt(*v)
            ));
        }
    }
    svg.push_str(&format!(
        "<text class=\"axis\" x=\"{PAD_L}\" y=\"{:.1}\">{}</text><text class=\"axis\" x=\"{PAD_L}\" y=\"{:.1}\">{}</text>",
        PAD_T - 6.0,
        fmt(hi),
        H - 4.0,
        fmt(lo)
    ));
    svg.push_str("</svg>");
    svg
}

/// The trend as a self-contained HTML page. `title` names the pack; `description` (the
/// pack's own description) is printed under it.
pub fn render_html(records: &[ChainedRecord], title: &str, description: Option<&str>) -> String {
    let mut cards = String::new();
    for id in figure_ids(records) {
        let last = records.iter().rev().find_map(|c| c.record.figures.get(&id));
        let unit = last.and_then(|p| p.unit.clone()).unwrap_or_default();
        let evidence = last
            .and_then(|p| p.evidence.clone())
            .unwrap_or_else(|| "not stated".into());
        let latest = last
            .and_then(|p| p.value)
            .map(|v| {
                if unit == "1" {
                    fmt(v)
                } else {
                    format!("{} {}", fmt(v), esc(&unit))
                }
            })
            .unwrap_or_else(|| "not found".into());
        let n_points = records
            .iter()
            .filter(|c| c.record.figures.get(&id).and_then(|p| p.value).is_some())
            .count();
        cards.push_str(&format!(
            "<section class=\"card\"><div class=\"card-head\"><h2>{}</h2><span class=\"tag\">{}</span></div>\
             <p class=\"meta\">latest {} &middot; {} point(s) over {} run(s){}</p>{}</section>",
            esc(&id),
            esc(&evidence),
            latest,
            n_points,
            records.len(),
            if unit.is_empty() || unit == "1" { String::new() } else { format!(" &middot; unit {}", esc(&unit)) },
            sparkline(records, &id)
        ));
    }
    let mut rows = String::new();
    for c in records {
        let r = &c.record;
        rows.push_str(&format!(
            "<tr class=\"{}\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"why\">{}</td><td><code>{}</code></td></tr>",
            if r.verdict == "fail" { "fail" } else { "" },
            r.index,
            esc(r.time_utc.as_deref().unwrap_or("")),
            esc(r.engine_version.as_deref().unwrap_or("n/a")),
            esc(&r.verdict),
            esc(&r.failures.join(", ")),
            esc(&c.hash[..12.min(c.hash.len())]),
        ));
    }
    let last_hash = records.last().map(|c| c.hash.clone()).unwrap_or_default();
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{title} trend</title><style>\
:root{{--bg:#f7f6f2;--panel:#ffffff;--ink:#1c2230;--muted:#5c6475;--line:#d9d6cc;--accent:#1f5f8b;--bad:#b3402f;--limit:#8a6d1c;--engine:#6b5aa6}}\
@media (prefers-color-scheme: dark){{:root:not([data-theme=\"light\"]){{--bg:#10141c;--panel:#171d28;--ink:#e7e9ee;--muted:#9aa3b5;--line:#2a3242;--accent:#6fb3e0;--bad:#ef7a66;--limit:#d8b75a;--engine:#a998e8}}}}\
:root[data-theme=\"dark\"]{{--bg:#10141c;--panel:#171d28;--ink:#e7e9ee;--muted:#9aa3b5;--line:#2a3242;--accent:#6fb3e0;--bad:#ef7a66;--limit:#d8b75a;--engine:#a998e8}}\
*{{box-sizing:border-box}}body{{margin:0;background:var(--bg);color:var(--ink);font:15px/1.5 system-ui,-apple-system,Segoe UI,sans-serif}}\
main{{max-width:1040px;margin:0 auto;padding:24px 16px 48px}}h1{{font-size:24px;margin:0 0 4px}}\
.lede{{color:var(--muted);margin:0 0 6px}}.chain{{font-size:13px;color:var(--muted);margin:0 0 20px;overflow-wrap:anywhere}}\
.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(300px,100%),1fr));gap:14px}}\
.card{{min-width:0;background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:12px 14px}}\
.card-head{{display:flex;justify-content:space-between;align-items:baseline;gap:8px}}\
.card h2{{font-size:15px;margin:0;word-break:break-word}}.tag{{font-size:11px;letter-spacing:.06em;border:1px solid var(--line);border-radius:4px;padding:1px 6px;color:var(--muted)}}\
.meta{{font-size:13px;color:var(--muted);margin:4px 0 6px}}.muted{{color:var(--muted)}}\
.spark{{width:100%;height:auto;display:block}}.line{{fill:none;stroke:var(--accent);stroke-width:2;vector-effect:non-scaling-stroke}}\
.pt{{fill:var(--accent)}}.pt.bad{{fill:var(--bad)}}.limit{{stroke:var(--limit);stroke-width:1.5;stroke-dasharray:6 4;vector-effect:non-scaling-stroke}}\
.limit-label{{fill:var(--limit);font-size:11px}}.engine{{stroke:var(--engine);stroke-width:1.5;stroke-dasharray:2 3;vector-effect:non-scaling-stroke}}\
.engine-label{{fill:var(--engine);font-size:11px}}.axis{{fill:var(--muted);font-size:10px}}\
.runs{{margin-top:26px;overflow-x:auto;scrollbar-color:var(--line) transparent}}table{{border-collapse:collapse;width:100%;font-size:13px}}\
th,td{{text-align:left;padding:6px 10px 6px 0;border-top:1px solid var(--line);white-space:nowrap;vertical-align:top}}td.why{{white-space:normal;min-width:220px}}\
tr.fail td{{color:var(--bad);font-weight:600}}.legend{{font-size:13px;color:var(--muted);margin:18px 0 0}}\
.honesty{{font-size:13px;color:var(--muted);border-top:1px solid var(--line);margin-top:26px;padding-top:12px}}\
</style></head><body><main><h1>{title}</h1>{desc}\
<p class=\"chain\">History verified: {n} record(s), hash chain intact, last hash <code>{last}</code>.</p>\
<div class=\"grid\">{cards}</div>\
<p class=\"legend\">Red point: the figure regressed or failed its requirement in that run. Dashed line: a requirement limit. Dotted line: the engine version changed between two runs.</p>\
<section class=\"runs\"><h2>Runs</h2><table><thead><tr><th>Run</th><th>Time (Coordinated Universal Time, UTC)</th><th>Engine</th><th>Verdict</th><th>Failed watches</th><th>Chain hash</th></tr></thead><tbody>{rows}</tbody></table></section>\
<p class=\"honesty\">Every figure is the output of an open-engine scenario run and carries that run's evidence label (VALIDATED, MODELLED, PARTNER or MIXED). A watch proves that a figure stayed stable or moved; it does not prove that a MODELLED figure is true. Decision support for your own verification and validation, not a certification.</p>\
</main></body></html>",
        title = esc(title),
        desc = description
            .map(|d| format!("<p class=\"lede\">{}</p>", esc(d)))
            .unwrap_or_default(),
        n = records.len(),
        last = esc(&last_hash),
        cards = cards,
        rows = rows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack_c::history::{chain, HistoryPoint, HistoryRecord};
    use std::collections::BTreeMap;

    fn rec(engine: &str, v: f64, status: &str, verdict: &str) -> HistoryRecord {
        let mut figures = BTreeMap::new();
        figures.insert(
            "w".to_string(),
            HistoryPoint {
                value: Some(v),
                unit: Some("%".into()),
                status: status.into(),
                requirement: Some("met".into()),
                limits: vec![crate::pack_c::watch::Limit {
                    op: ">=".into(),
                    value: 99.0,
                }],
                evidence: Some("MODELLED".into()),
            },
        );
        HistoryRecord {
            index: 0,
            time_utc: None,
            engine_version: Some(engine.into()),
            pack_id: "p".into(),
            pack_version: "1".into(),
            pack_hash: "h".into(),
            label: crate::pack_c::run::CI_LABEL.to_string(),
            metrics_digest: "d".into(),
            verdict: verdict.into(),
            failures: if verdict == "fail" {
                vec!["w".into()]
            } else {
                vec![]
            },
            figures,
            result_sha256: BTreeMap::new(),
            prev_hash: String::new(),
        }
    }

    #[test]
    fn one_point_per_run_red_regression_and_engine_boundary() {
        let mut recs = Vec::new();
        for r in [
            rec("0.28.0", 100.0, "no-baseline", "pass"),
            rec("0.28.0", 99.5, "regressed", "fail"),
            rec("0.29.0", 100.0, "improved", "pass"),
        ] {
            let (c, _) = chain(&recs, r);
            recs.push(c);
        }
        let html = render_html(&recs, "p 1", Some("demo"));
        assert_eq!(html.matches("<circle").count(), 3);
        assert_eq!(html.matches("class=\"pt bad\"").count(), 1);
        assert!(html.contains("engine 0.29.0"));
        assert!(html.contains("requirement &gt;= 99"));
        assert_eq!(html.matches("<tr class=\"fail\">").count(), 1);
        let j: serde_json::Value = serde_json::from_str(&render_json(&recs, "p 1")).unwrap();
        assert_eq!(j["figures"][0]["points"].as_array().unwrap().len(), 3);
        assert_eq!(render_html(&recs, "p 1", Some("demo")), html);
    }
}
