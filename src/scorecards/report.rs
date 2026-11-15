// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The receiver resilience test report (roadmap 0.42, E3): a directory with the frozen
//! scorecard JSON, a readable HTML report, the campaign file as run and the mapping templates,
//! signed as a whole with the dossier signature ([`crate::signed_dossier`]). Building needs the
//! `pro-receiver-scorecard` kind; verifying needs nothing (`kshana-pro dossier verify`).

use std::path::Path;

use kshana::api::KshanaError;

use super::{Scorecard, DHS_TEMPLATE, P1952_TEMPLATE};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn opt(v: Option<f64>, unit: &str) -> String {
    v.map(|x| format!("{x} {unit}"))
        .unwrap_or_else(|| "–".into())
}

fn optn(v: Option<usize>) -> String {
    v.map(|x| x.to_string())
        .unwrap_or_else(|| "not assessed".into())
}

/// A one-line SVG badge.
pub fn summary_svg(s: &Scorecard) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"520\" height=\"72\" font-family=\"system-ui,sans-serif\"><rect width=\"520\" height=\"72\" fill=\"#0f1216\"/><text x=\"16\" y=\"30\" fill=\"#8cc4f5\" font-size=\"15\" font-weight=\"bold\">Receiver scorecard · {} {}</text><text x=\"16\" y=\"54\" fill=\"#9aa3af\" font-size=\"12\">{} cases · highest level supported by the evidence: {} · test evidence</text></svg>",
        esc(&s.receiver.maker),
        esc(&s.receiver.model),
        s.totals.cases,
        s.levels.highest_supported
    )
}

/// One row per event.
pub fn cases_csv(s: &Scorecard) -> String {
    let mut out = String::from(
        "case,event,kind,onset_s,end_s,monitor_outcome,monitor_latency_s,receiver_flag_latency_s,fix_epochs,window_epochs,max_error_m,max_time_error_s,misleading_unwarned,misleading_unmonitored,recovered,recovery_s\n",
    );
    let f = |v: Option<f64>| v.map(|x| x.to_string()).unwrap_or_default();
    let n = |v: Option<usize>| v.map(|x| x.to_string()).unwrap_or_default();
    for c in &s.cases {
        for e in &c.events {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                c.id,
                e.label.replace(',', ";"),
                e.kind,
                e.onset_s,
                e.end_s,
                e.monitor_outcome,
                f(e.monitor_latency_s),
                f(e.receiver_flag_latency_s),
                e.window_fix_epochs,
                e.window_epochs,
                f(e.max_error_m),
                f(e.max_time_error_s),
                n(e.misleading_unwarned),
                n(e.misleading_unmonitored),
                e.recovered.map(|b| b.to_string()).unwrap_or_default(),
                f(e.recovery_s)
            ));
        }
    }
    out
}

/// The HTML test report.
pub fn html(s: &Scorecard) -> String {
    let mut h = String::new();
    h.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    h.push_str(&format!(
        "<title>Receiver resilience test report: {} {}</title>",
        esc(&s.receiver.maker),
        esc(&s.receiver.model)
    ));
    h.push_str("<style>body{font-family:system-ui,sans-serif;max-width:980px;margin:2rem auto;padding:0 16px;color:#1d2330;background:#fff}h1{font-size:1.5rem}h2{font-size:1.15rem;margin-top:2rem;border-bottom:1px solid #d5dae3}table{border-collapse:collapse;width:100%;font-size:.86rem;margin:.6rem 0}th,td{border:1px solid #d5dae3;padding:4px 6px;text-align:left;vertical-align:top}th{background:#f2f4f8}.label{background:#fff7e0;border:1px solid #e8d9a8;padding:.6rem;font-size:.86rem}.met{color:#146c2e}.not{color:#a12020}.na{color:#6b7280}code{font-size:.82rem}</style></head><body>");
    h.push_str(&format!(
        "<h1>Receiver resilience test report</h1><p><b>{} {}</b>{}{} · campaign <code>{}</code>{}{}</p>",
        esc(&s.receiver.maker),
        esc(&s.receiver.model),
        s.receiver
            .firmware
            .as_deref()
            .map(|f| format!(", firmware {}", esc(f)))
            .unwrap_or_default(),
        s.receiver
            .antenna
            .as_deref()
            .map(|a| format!(", antenna {}", esc(a)))
            .unwrap_or_default(),
        esc(&s.campaign.id),
        s.campaign
            .facility
            .as_deref()
            .map(|f| format!(" · {}", esc(f)))
            .unwrap_or_default(),
        s.campaign
            .operator
            .as_deref()
            .map(|o| format!(" · run by {}", esc(o)))
            .unwrap_or_default(),
    ));
    h.push_str(&format!("<p class=\"label\">{}</p>", esc(s.label)));
    // Levels.
    h.push_str(&format!(
        "<h2>Resilience levels (test evidence)</h2><p>{}</p><p><b>Highest level supported by this campaign's evidence: {}</b></p><table><tr><th>Level</th><th>Summary</th><th>Evidence</th><th>Cases met / not met / not assessed</th></tr>",
        esc(&s.levels.wording),
        if s.levels.highest_supported == 0 {
            "none".to_string()
        } else {
            s.levels.highest_supported.to_string()
        }
    ));
    for l in &s.levels.levels {
        let cls = match l.evidence.as_str() {
            "supported" => "met",
            "not supported" => "not",
            _ => "na",
        };
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"{cls}\">{}</td><td>{} / {} / {}</td></tr>",
            l.level,
            esc(&l.summary),
            l.evidence,
            l.cases_met,
            l.cases_not_met,
            l.cases_not_assessed
        ));
    }
    h.push_str(&format!(
        "</table><p><small>{}</small></p>",
        esc(&s.levels.framework)
    ));
    // Cases.
    h.push_str("<h2>Cases</h2><table><tr><th>Case</th><th>Source</th><th>Truth</th><th>Fix availability under threat</th><th>Max error</th><th>Max time error</th><th>Misleading without warning (receiver / with monitor)</th><th>Longest recovery</th><th>L1 · L2 · L3 · L4</th></tr>");
    for c in &s.cases {
        let lv = |v: super::levels::Verdict| {
            let cls = match v {
                super::levels::Verdict::Met => "met",
                super::levels::Verdict::NotMet => "not",
                super::levels::Verdict::NotAssessed => "na",
            };
            format!("<span class=\"{cls}\">{}</span>", v.as_str())
        };
        h.push_str(&format!(
            "<tr><td><b>{}</b><br>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{} · {} · {} · {}</td></tr>",
            esc(&c.id),
            esc(&c.title),
            esc(&c.source),
            esc(&c.truth),
            c.fix_availability
                .map(|a| format!("{:.1} %", a * 100.0))
                .unwrap_or_else(|| "–".into()),
            opt(c.max_error_m, "m"),
            opt(c.max_time_error_s, "s"),
            optn(c.misleading.unwarned_epochs),
            optn(c.misleading.unmonitored_epochs),
            opt(c.recovery_s, "s"),
            lv(c.levels.l1),
            lv(c.levels.l2),
            lv(c.levels.l3),
            lv(c.levels.l4)
        ));
    }
    h.push_str("</table>");
    h.push_str("<h2>Events</h2><table><tr><th>Case</th><th>Event</th><th>Window (s)</th><th>Open monitor on the log</th><th>Receiver's own flag</th><th>Fix epochs</th><th>Max / p95 error</th><th>Recovered</th></tr>");
    for c in &s.cases {
        for e in &c.events {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{} ({})</td><td>{} to {}</td><td>{}{}{}</td><td>{}</td><td>{} of {}</td><td>{} / {}</td><td>{}</td></tr>",
                esc(&c.id),
                esc(&e.label),
                esc(&e.kind),
                e.onset_s,
                e.end_s,
                esc(&e.monitor_outcome),
                e.monitor_latency_s
                    .map(|l| format!(" after {l} s"))
                    .unwrap_or_default(),
                if e.monitor_first.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", esc(&e.monitor_first.join(", ")))
                },
                e.receiver_flag_latency_s
                    .map(|l| format!("after {l} s"))
                    .unwrap_or_else(|| "none".into()),
                e.window_fix_epochs,
                e.window_epochs,
                opt(e.max_error_m, "m"),
                opt(e.p95_error_m, "m"),
                match (e.recovered, e.recovery_s) {
                    (Some(true), Some(r)) => format!("yes, {r} s after the end"),
                    (Some(false), _) => "no".into(),
                    _ => "not judged".into(),
                }
            ));
        }
    }
    h.push_str("</table>");
    // P1952.
    h.push_str(&format!(
        "<h2>Mapped to the IEEE P1952 draft topics</h2><p>{}</p><p><small>{} {}</small></p><table><tr><th>Topic</th><th>Summary</th><th>Evidence in receiver-scorecard.json</th></tr>",
        esc(&s.p1952.wording),
        esc(&s.p1952.standard),
        esc(&s.p1952.status)
    ));
    for t in &s.p1952.topics {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
            esc(&t.topic),
            esc(&t.summary),
            esc(&t.evidence.join(", "))
        ));
    }
    h.push_str("</table>");
    // Bars and provenance.
    let b = &s.bars;
    h.push_str(&format!(
        "<h2>Bars stated before scoring</h2><table><tr><th>Bar</th><th>Value</th></tr>\
         <tr><td>Fixed</td><td>{}</td></tr><tr><td>Wrong output: position</td><td>&gt; {} m</td></tr>\
         <tr><td>Wrong output: time</td><td>&gt; {} s</td></tr><tr><td>Level 1: recovery within</td><td>{} s (held {} s)</td></tr>\
         <tr><td>Level 3: bound</td><td>{} m, {} s, availability ≥ {}</td></tr><tr><td>Level 4: bound</td><td>{} m, availability ≥ {}</td></tr>\
         <tr><td>Monitor alarm counts as a warning for level 2</td><td>{}</td></tr></table>\
         <p>Bars SHA-256 <code>{}</code> · campaign file SHA-256 <code>{}</code> · method <code>{}</code> · {} · {} · threat library {}</p>",
        esc(b.fixed_utc.as_deref().unwrap_or("not stated")),
        b.misleading_error_m,
        b.misleading_time_error_s,
        b.max_recovery_s,
        b.recovery_hold_s,
        b.bounded_error_m,
        b.bounded_time_error_s,
        b.bounded_min_availability,
        b.full_error_m,
        b.full_min_availability,
        b.monitor_counts_as_warning,
        s.bars_sha256,
        s.campaign_sha256,
        s.method,
        esc(&s.engine.open_engine),
        esc(&s.engine.pro),
        esc(&s.engine.threat_library)
    ));
    h.push_str("<h2>What this report does not show</h2><ul>");
    for l in &s.limits {
        h.push_str(&format!("<li>{}</li>", esc(l)));
    }
    h.push_str(
        "</ul><h2>Inputs</h2><table><tr><th>Case</th><th>Log SHA-256</th><th>Epochs</th></tr>",
    );
    for c in &s.cases {
        h.push_str(&format!(
            "<tr><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
            esc(&c.id),
            c.log_sha256,
            c.epochs
        ));
    }
    h.push_str("</table><p><small>Verify this directory offline with the free edition: <code>kshana-pro dossier verify &lt;dir&gt; --trust &lt;signer key&gt;</code>.</small></p></body></html>\n");
    h
}

/// Write the report directory (new or empty): `receiver-scorecard.json`, `report.html`,
/// `campaign.toml`, `cases.csv`, `mapping/ieee-p1952-draft.json`,
/// `mapping/dhs-rpcf-levels.json` and `README.txt`. Signing is a separate step.
pub fn write_dir(out: &Path, s: &Scorecard, campaign_src: &str) -> Result<(), KshanaError> {
    let io = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", out.display()));
    if out.exists() && std::fs::read_dir(out).map_err(io)?.next().is_some() {
        return Err(KshanaError::InvalidInput(format!(
            "{}: not empty; a report goes into a new or empty directory",
            out.display()
        )));
    }
    std::fs::create_dir_all(out.join("mapping")).map_err(io)?;
    let json =
        serde_json::to_string_pretty(s).map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    std::fs::write(out.join("receiver-scorecard.json"), json + "\n").map_err(io)?;
    std::fs::write(out.join("report.html"), html(s)).map_err(io)?;
    std::fs::write(out.join("campaign.toml"), campaign_src).map_err(io)?;
    std::fs::write(out.join("cases.csv"), cases_csv(s)).map_err(io)?;
    std::fs::write(out.join("mapping/ieee-p1952-draft.json"), P1952_TEMPLATE).map_err(io)?;
    std::fs::write(out.join("mapping/dhs-rpcf-levels.json"), DHS_TEMPLATE).map_err(io)?;
    std::fs::write(
        out.join("README.txt"),
        format!(
            "Receiver resilience test report ({}), campaign {}.\n\n{}\n\n\
             receiver-scorecard.json   the scorecard (frozen format {})\n\
             report.html      the readable test report\n\
             campaign.toml    the campaign file as run (its SHA-256 is in the scorecard)\n\
             cases.csv        one row per event\n\
             mapping/         the level and topic templates the mapping used\n\
             signature.json   the signer's Ed25519 signature over every file\n\n\
             Verify offline, no licence needed: kshana-pro dossier verify <this directory> --trust <signer key>\n\
             Log files are not included; their SHA-256 are in the scorecard.\n",
            s.method,
            s.campaign.id,
            s.label,
            s.format
        ),
    )
    .map_err(io)?;
    Ok(())
}
