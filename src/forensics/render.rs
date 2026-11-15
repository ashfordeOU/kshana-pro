// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The faces of a forensic report: HTML, PDF, the timeline as CSV and SVG, and the affected
//! area and tracks as GeoJSON. Every face is drawn from [`ForensicReport`]; none reads the
//! clock, so all but the PDF (whose trailer `/ID` is random inside the PDF writer) are
//! byte-identical on a re-run.

use std::fmt::Write as _;

use serde_json::{json, Value};

use super::detect::{Finding, ForensicReport};
use crate::report_brand as brand;
use crate::report_pdf::{rgb, sans, sans_bold, serif, serif_bold, wrap, Layout};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The timeline as CSV: `utc,kind,class,finding,text`.
pub fn timeline_csv(r: &ForensicReport) -> String {
    let mut s = String::from("utc,kind,class,finding,text\n");
    for e in &r.timeline {
        let _ = writeln!(
            s,
            "{},{},{},{},{}",
            e.utc,
            e.kind,
            e.class,
            e.finding.as_deref().unwrap_or(""),
            csv_field(&e.text)
        );
    }
    s
}

/// The findings as CSV.
pub fn findings_csv(r: &ForensicReport) -> String {
    let mut s = String::from(
        "id,category,source,data_class,onset_earliest_utc,onset_latest_utc,end_utc,duration_s,confidence,supported,indicators\n",
    );
    for f in &r.findings {
        let _ = writeln!(
            s,
            "{},{},{},{},{},{},{},{},{},{},{}",
            f.id,
            f.category,
            csv_field(&f.source),
            f.data_class,
            f.onset.earliest_utc,
            f.onset.latest_utc,
            f.end.earliest_utc,
            f.duration_s,
            f.confidence.level,
            f.supported,
            csv_field(&f.indicators.join(" "))
        );
    }
    s
}

fn color(cat: &str) -> &'static str {
    match cat {
        "jamming" => "#b5562d",
        "spoofing" => "#6b3fa0",
        "jamming-and-spoofing" => "#8a2f4f",
        _ => "#7a7a7a",
    }
}

/// The findings on a time axis across the incident window (one row per source).
pub fn timeline_svg(r: &ForensicReport) -> String {
    let p = super::inputs::parse_utc;
    let (Some(w0), Some(w1)) = (p(&r.incident.start_utc), p(&r.incident.end_utc)) else {
        return String::new();
    };
    let mut sources: Vec<&str> = r.findings.iter().map(|f| f.source.as_str()).collect();
    sources.dedup();
    let mut uniq: Vec<&str> = Vec::new();
    for s in sources {
        if !uniq.contains(&s) {
            uniq.push(s);
        }
    }
    let (w, ml, mr, row, top) = (760.0, 170.0, 20.0, 26.0, 34.0);
    let h = top + row * uniq.len().max(1) as f64 + 40.0;
    let x = |t: f64| ml + (t - w0) / (w1 - w0) * (w - ml - mr);
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" \
font-family=\"sans-serif\" font-size=\"11\"><rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\
<text x=\"{ml}\" y=\"18\" fill=\"{}\">Findings in the incident window (MODELLED; colours: jamming-like, spoofing-like, both, not attributed)</text>",
        brand::PLATE,
        brand::INK_2
    );
    for (i, src) in uniq.iter().enumerate() {
        let y = top + row * i as f64;
        let _ = write!(
            s,
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"{}\">{}</text>\
<line x1=\"{ml}\" x2=\"{:.1}\" y1=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\"/>",
            ml - 8.0,
            y + 14.0,
            brand::INK,
            esc(src),
            w - mr,
            y + 10.0,
            y + 10.0,
            brand::LINE
        );
        for f in r.findings.iter().filter(|f| f.source == *src) {
            let (a, b) = (
                p(&f.onset.latest_utc).unwrap_or(w0).clamp(w0, w1),
                p(&f.end.earliest_utc).unwrap_or(w1).clamp(w0, w1),
            );
            let _ = write!(
                s,
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"14\" fill=\"{}\"><title>{} {}</title></rect>\
<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#ffffff\" font-size=\"9\">{}</text>",
                x(a),
                y + 3.0,
                (x(b) - x(a)).max(2.0),
                color(f.category),
                f.id,
                esc(f.category),
                x(a) + 2.0,
                y + 13.5,
                f.id
            );
        }
    }
    let yb = h - 22.0;
    let _ = write!(
        s,
        "<text x=\"{ml}\" y=\"{yb}\" fill=\"{0}\">{1}</text><text x=\"{2}\" y=\"{yb}\" text-anchor=\"end\" fill=\"{0}\">{3}</text></svg>",
        brand::INK_2,
        esc(&r.incident.start_utc),
        w - mr,
        esc(&r.incident.end_utc)
    );
    s
}

/// The affected area (MODELLED) as GeoJSON: the box, the centroid and the affected observers.
pub fn area_geojson(r: &ForensicReport) -> String {
    let a = &r.area;
    let mut feats: Vec<Value> = Vec::new();
    if let Some(b) = a.bbox {
        feats.push(json!({
            "type": "Feature",
            "properties": { "role": "affected-area", "class": "modelled", "radius_m": a.radius_m, "note": a.note },
            "geometry": { "type": "Polygon", "coordinates": [[
                [b[2], b[0]], [b[3], b[0]], [b[3], b[1]], [b[2], b[1]], [b[2], b[0]] ]] }
        }));
    }
    if let Some(c) = a.centroid {
        feats.push(json!({
            "type": "Feature",
            "properties": { "role": "centroid", "class": "modelled" },
            "geometry": { "type": "Point", "coordinates": [c[1], c[0]] }
        }));
    }
    for p in &a.points {
        feats.push(json!({
            "type": "Feature",
            "properties": { "role": "affected-observer", "class": "modelled" },
            "geometry": { "type": "Point", "coordinates": [p[1], p[0]] }
        }));
    }
    if let Some(b) = r.incident.area {
        feats.push(json!({
            "type": "Feature",
            "properties": { "role": "incident-area", "class": "stated" },
            "geometry": { "type": "Polygon", "coordinates": [[
                [b.lon_min, b.lat_min], [b.lon_max, b.lat_min], [b.lon_max, b.lat_max],
                [b.lon_min, b.lat_max], [b.lon_min, b.lat_min] ]] }
        }));
    }
    let doc = json!({ "type": "FeatureCollection", "features": feats });
    serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
}

/// The subject's tracks as GeoJSON: one LineString per source and the flagged points.
pub fn tracks_geojson(r: &ForensicReport) -> String {
    let mut feats: Vec<Value> = Vec::new();
    for t in &r.tracks {
        let coords: Vec<Value> = t.points.iter().map(|p| json!([p.lon, p.lat])).collect();
        feats.push(json!({
            "type": "Feature",
            "properties": { "source": t.source, "class": t.class,
                "from": t.points.first().map(|p| p.utc.clone()), "to": t.points.last().map(|p| p.utc.clone()) },
            "geometry": { "type": "LineString", "coordinates": coords }
        }));
        let flagged: Vec<Value> = t
            .points
            .iter()
            .filter(|p| p.flagged)
            .map(|p| json!([p.lon, p.lat]))
            .collect();
        if !flagged.is_empty() {
            feats.push(json!({
                "type": "Feature",
                "properties": { "source": t.source, "class": t.class, "role": "flagged" },
                "geometry": { "type": "MultiPoint", "coordinates": flagged }
            }));
        }
    }
    let doc = json!({ "type": "FeatureCollection", "features": feats });
    serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
}

fn status_text(r: &ForensicReport) -> &'static str {
    match r.status {
        "final" => "Final (standard pack)",
        "final-with-checklist-failures" => "Final, with checklist failures (see the checklist)",
        "awaiting-expert-review" => {
            "Awaiting expert review: not claim-grade until a named reviewer signs"
        }
        "claim-grade-checklist-not-met" => {
            "Claim-grade checklist NOT met: this report is not claim-grade"
        }
        _ => "",
    }
}

fn finding_html(f: &Finding) -> String {
    let mag: Vec<String> = f
        .magnitude
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let pos = f
        .position_at_onset
        .as_ref()
        .map(|p| {
            format!(
                "{:.5}, {:.5} ({}; {})",
                p.lat,
                p.lon,
                p.class.to_uppercase(),
                esc(&p.note)
            )
        })
        .unwrap_or_else(|| "no position".into());
    let ev: String = f
        .evidence
        .iter()
        .map(|e| {
            format!(
                "<li><b>{}</b> {}{}</li>",
                e.class.to_uppercase(),
                esc(&e.statement),
                e.input_sha256
                    .as_ref()
                    .map(|h| format!(" <code>{}</code>", &h[..16]))
                    .unwrap_or_default()
            )
        })
        .collect();
    let reasons: String = f
        .confidence
        .reasons
        .iter()
        .map(|r| format!("<li>{}</li>", esc(r)))
        .collect();
    format!(
        "<section class=\"finding\" id=\"{id}\"><h3>{id} · {cat} <span class=\"tag\">{dc} data · MODELLED finding</span></h3>\
<p>{stmt}</p><table>\
<tr><th>Onset</th><td>between {o0} and {o1} (± {ou} s; {ob})</td></tr>\
<tr><th>End</th><td>between {e0} and {e1} ({eb})</td></tr>\
<tr><th>Indicators</th><td>{ind}</td></tr>\
<tr><th>Magnitude</th><td>{mag}</td></tr>\
<tr><th>Position at onset</th><td>{pos}</td></tr>\
<tr><th>Confidence</th><td><b>{lvl}</b> ({pts} of 4 points){sup}<ul>{reasons}</ul></td></tr>\
<tr><th>Corroborated by</th><td>{corr}</td></tr>\
<tr><th>Evidence</th><td><ul>{ev}</ul></td></tr></table></section>",
        id = f.id,
        cat = esc(f.category),
        dc = f.data_class.to_uppercase(),
        stmt = esc(&f.statement),
        o0 = f.onset.earliest_utc,
        o1 = f.onset.latest_utc,
        ou = f.onset.uncertainty_s,
        ob = esc(&f.onset.basis),
        e0 = f.end.earliest_utc,
        e1 = f.end.latest_utc,
        eb = esc(&f.end.basis),
        ind = esc(&f.indicators.join(", ")),
        mag = esc(&mag.join("; ")),
        lvl = f.confidence.level,
        pts = f.confidence.points,
        sup = if f.supported { " · meets the claim-grade bar" } else { " · indicative" },
        corr = if f.corroborated_by.is_empty() { "none".into() } else { f.corroborated_by.join(", ") },
    )
}

/// The report as one self-contained HTML page.
pub fn report_html(r: &ForensicReport) -> String {
    let inc = &r.incident;
    let mut h = String::new();
    let _ = write!(
        h,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>Forensic report {id}</title><style>\
body{{background:{paper};color:{ink};font-family:{body};max-width:60rem;margin:0 auto;padding:1.5rem;line-height:1.45}}\
h1,h2,h3{{font-family:{head};color:{navy}}} h2{{border-bottom:1px solid {brass};padding-bottom:.2rem}}\
table{{border-collapse:collapse;width:100%;margin:.5rem 0}} th,td{{border:1px solid {line};padding:.3rem .5rem;text-align:left;vertical-align:top}}\
th{{background:{parch};width:12rem}} code{{font-family:{mono};font-size:.85em;word-break:break-all}}\
.banner{{background:{seal};padding:.6rem .8rem;font-weight:600}} .tag{{font-size:.7em;color:{ink2};font-weight:400}}\
.finding{{background:{plate};border:1px solid {line};padding:.2rem .8rem;margin:.8rem 0}}\
.sample{{color:#a33;font-weight:700}} svg{{max-width:100%;height:auto}}\
</style></head><body>",
        id = esc(&inc.id),
        paper = brand::PAPER,
        ink = brand::INK,
        ink2 = brand::INK_2,
        body = brand::FONT_BODY,
        head = brand::FONT_HEAD,
        mono = brand::FONT_MONO,
        navy = brand::NAVY_INK,
        brass = brand::BRASS,
        line = brand::LINE,
        parch = brand::PARCHMENT,
        seal = brand::BRASS_SEAL,
        plate = brand::PLATE,
    );
    if brand::sample() {
        let _ = write!(h, "<p class=\"sample\">{}</p>", esc(brand::SAMPLE_TEXT));
    }
    let _ = write!(
        h,
        "<p>{seal}</p><h1>Incident forensic report: {title}</h1>\
<p><b>{variant}</b> · case <code>{id}</code> · status: <b>{status}</b> · generated {gen}</p>\
<p class=\"banner\">{label}</p><p>{valid}</p>",
        seal = brand::seal_img_html(56),
        title = esc(&inc.title),
        variant = r.variant.title(),
        id = esc(&inc.id),
        status = status_text(r),
        gen = esc(&r.generated_utc),
        label = esc(r.label),
        valid = esc(r.validation_status),
    );
    // 1. What happened.
    let _ = write!(
        h,
        "<h2>1. What happened</h2><p><b>{}</b></p><table>\
<tr><th>Jamming-like</th><td>{} {}</td></tr><tr><th>Spoofing-like</th><td>{} {}</td></tr></table>",
        esc(&r.verdict.sentence),
        r.verdict.jamming.status,
        r.verdict
            .jamming
            .confidence
            .map(|c| format!("({c} confidence)"))
            .unwrap_or_default(),
        r.verdict.spoofing.status,
        r.verdict
            .spoofing
            .confidence
            .map(|c| format!("({c} confidence)"))
            .unwrap_or_default(),
    );
    let _ = write!(
        h,
        "<table><tr><th>Subject</th><td>{} {}</td></tr><tr><th>Window</th><td>{} to {}</td></tr>\
<tr><th>Area</th><td>{}</td></tr><tr><th>Reported (as given)</th><td>{}</td></tr><tr><th>Requested by</th><td>{}</td></tr></table>",
        inc.subject.as_str(),
        esc(inc.subject_id.as_deref().unwrap_or("")),
        esc(&inc.start_utc),
        esc(&inc.end_utc),
        inc.area
            .map(|a| format!("{}..{} N, {}..{} E", a.lat_min, a.lat_max, a.lon_min, a.lon_max))
            .unwrap_or_else(|| "not stated (every position in the data)".into()),
        esc(inc.reported.as_deref().unwrap_or("none")),
        esc(inc.requested_by.as_deref().unwrap_or("not stated")),
    );
    let _ = write!(h, "<h2>2. Findings</h2>");
    if r.findings.is_empty() {
        h.push_str("<p>No finding: no monitor or detector fired in the window at the method's thresholds.</p>");
    }
    for f in &r.findings {
        h.push_str(&finding_html(f));
    }
    // 3. When and where.
    let _ = write!(h, "<h2>3. When and where</h2>{}", timeline_svg(r));
    h.push_str("<table><tr><th>UTC</th><th>Event</th></tr>");
    for e in &r.timeline {
        let _ = write!(
            h,
            "<tr><td>{}</td><td>{} <span class=\"tag\">{}</span>{}</td></tr>",
            e.utc,
            esc(&e.text),
            e.class.to_uppercase(),
            e.finding
                .as_ref()
                .map(|f| format!(" ({f})"))
                .unwrap_or_default()
        );
    }
    h.push_str("</table>");
    let a = &r.area;
    let _ = write!(
        h,
        "<h3>Affected area (MODELLED)</h3><p>{}</p><table><tr><th>Affected observers</th><td>{}</td></tr>\
<tr><th>Unaffected observers inside</th><td>{}</td></tr><tr><th>Centroid</th><td>{}</td></tr>\
<tr><th>Radius</th><td>{}</td></tr></table><p>Map layers: <code>area.geojson</code>, <code>tracks.geojson</code>.</p>",
        esc(&a.note),
        a.affected_observers,
        a.unaffected_observers_inside,
        a.centroid.map(|c| format!("{:.5}, {:.5}", c[0], c[1])).unwrap_or_else(|| "none".into()),
        a.radius_m.map(|r| format!("{r:.0} m")).unwrap_or_else(|| "none".into()),
    );
    if !r.area_activity.is_empty() {
        h.push_str("<h3>AIS and ADS-B in the window and area (REPORTED data)</h3><table><tr><th>Input</th><th>Observed</th><th>Flagged</th><th>By detector</th></tr>");
        for x in &r.area_activity {
            let by: Vec<String> = x.flagged.iter().map(|(k, v)| format!("{k} {v}")).collect();
            let _ = write!(
                h,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}{}</td></tr>",
                esc(&x.label),
                x.observed,
                x.flagged_any,
                esc(&by.join("; ")),
                x.unassessed
                    .map(|u| format!("; unassessed {u}"))
                    .unwrap_or_default()
            );
        }
        h.push_str("</table>");
    }
    if !r.receivers.is_empty() {
        h.push_str("<h3>Receiver logs (MEASURED data)</h3><table><tr><th>Log</th><th>Epochs</th><th>Monitors</th><th>Engine verdict</th></tr>");
        for x in &r.receivers {
            let mons: Vec<String> = x
                .monitors_run
                .iter()
                .map(|m| {
                    serde_json::to_value(m)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                })
                .collect();
            let _ = write!(
                h,
                "<tr><td>{} ({}, {})</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&x.label),
                x.role,
                x.platform,
                x.epochs,
                esc(&mons.join(", ")),
                esc(&x.engine_verdict)
            );
        }
        h.push_str("</table>");
    }
    if !r.statements.is_empty() {
        h.push_str("<h3>Statements (REPORTED, recorded as given)</h3><table>");
        for s in &r.statements {
            let _ = write!(
                h,
                "<tr><th>{} · {}</th><td>{}</td></tr>",
                esc(&s.source),
                esc(&s.utc),
                esc(&s.text)
            );
        }
        h.push_str("</table>");
    }
    // 4. How confident.
    let _ = write!(
        h,
        "<h2>4. How confident</h2><p>Each finding scores up to 4 points: sustained (at least {:.0} s); two or more \
independent indicators of the attributed kind; the discriminator (a broad loss across satellites for jamming; a \
position, time or authentication indicator for spoofing); corroboration by another source within {:.0} s. \
4 points is <b>high</b> (only possible with corroboration), 2 or 3 <b>medium</b>, less <b>low</b>. Onset and end \
times are bounded by the last clean and the first alarmed epoch or report. These are stated rules, not probabilities.</p>\
<ul>{}</ul>",
        r.method.strong_episode_s,
        r.method.corroboration_window_s,
        r.limits.iter().map(|l| format!("<li>{}</li>", esc(l))).collect::<String>()
    );
    // 5. What it rests on.
    h.push_str("<h2>5. What it rests on</h2><table><tr><th>Input</th><th>Class</th><th>SHA-256</th><th>Coverage</th></tr>");
    for i in &r.inputs {
        let _ = write!(
            h,
            "<tr><td>{} ({}, <code>inputs/{}</code>, {} bytes)</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
            esc(&i.label),
            i.role.as_str(),
            esc(&i.file),
            i.bytes,
            i.data_class.to_uppercase(),
            i.sha256,
            match (&i.first_utc, &i.last_utc, i.window_coverage) {
                (Some(a), Some(b), Some(c)) => format!("{a} to {b}; {:.0} % of the window", c * 100.0),
                _ => "no absolute time".into(),
            }
        );
    }
    h.push_str("</table><h3>Evidence checklist</h3><table><tr><th>Item</th><th>Required for</th><th>Result</th><th>Detail</th></tr>");
    for c in &r.checklist {
        let _ = write!(
            h,
            "<tr><td>{} {}</td><td>{}</td><td><b>{}</b></td><td>{}</td></tr>",
            c.id,
            esc(c.item),
            c.required_for,
            c.result,
            esc(&c.detail)
        );
    }
    h.push_str("</table>");
    if r.expert_review.required {
        let e = &r.expert_review;
        let blank =
            |v: &Option<String>| esc(v.as_deref().unwrap_or("(to be completed by the reviewer)"));
        let _ = write!(
            h,
            "<h3>Expert review</h3><p>Status: <b>{}</b>. {}</p><table>\
<tr><th>Reviewer</th><td>{}</td></tr><tr><th>Qualification</th><td>{}</td></tr><tr><th>Organisation</th><td>{}</td></tr>\
<tr><th>Independence</th><td>{}</td></tr><tr><th>Decision</th><td>{}</td></tr><tr><th>Statement</th><td>{}</td></tr>\
<tr><th>Date</th><td>{}</td></tr></table>",
            e.status,
            esc(&e.how_to_complete),
            blank(&e.reviewer_name),
            blank(&e.qualification),
            blank(&e.organisation),
            blank(&e.independence_statement),
            blank(&e.decision),
            blank(&e.statement),
            blank(&e.reviewed_utc),
        );
    }
    // 6. How to reproduce it.
    let _ = write!(
        h,
        "<h2>6. How to reproduce it</h2><table><tr><th>Kshana Pro</th><td>{}</td></tr><tr><th>Open engine</th><td>{}</td></tr>\
<tr><th>Method</th><td>{} (parameters SHA-256 <code>{}</code>)</td></tr><tr><th>Recipe</th><td><code>{}</code></td></tr>\
<tr><th>Command</th><td><code>{}</code></td></tr></table><p>{}</p>\
<p>Verify this bundle offline, with no licence: <code>kshana-pro forensics verify &lt;bundle&gt; --trust &lt;issuer public key&gt;</code> \
(or <code>kshana-pro dossier verify</code>, or <code>scripts/verify-dossier-signature.py</code>); each receiver's evidence pack under \
<code>evidence/</code> also verifies with the open engine's <code>kshana evidence verify</code>.</p></body></html>",
        esc(&r.engine.kshana_pro),
        esc(&r.engine.open_engine),
        r.engine.method_id,
        r.engine.method_params_sha256,
        r.reproduce.recipe,
        esc(&r.reproduce.command),
        esc(&r.reproduce.note),
    );
    h.push('\n');
    h
}

/// The report as a branded PDF (ASCII text; the standard-14 fonts).
pub fn report_pdf(r: &ForensicReport) -> Result<Vec<u8>, String> {
    let ascii = |s: &str| -> String {
        s.chars()
            .map(|c| match c {
                '\u{2013}' | '\u{2014}' => '-',
                '\u{00b1}' => '+',
                c if c.is_ascii() => c,
                _ => '?',
            })
            .collect()
    };
    let (w, lh) = (170.0_f32, 4.6_f32);
    let mut l = Layout::new();
    l.cover_lockup();
    for line in wrap(
        &ascii(&format!("Incident forensic report: {}", r.incident.title)),
        w,
        20.0,
    ) {
        l.ensure(10.0);
        l.text_at(20.0, &line, serif_bold(), 20.0, rgb(brand::INK));
        l.advance(9.0);
    }
    l.paragraph(
        &ascii(&format!(
            "{} - case {} - status: {} - generated {}",
            r.variant.title(),
            r.incident.id,
            status_text(r),
            r.generated_utc
        )),
        serif(),
        11.0,
        5.5,
        rgb(brand::INK),
    );
    l.advance(3.0);
    l.paragraph(&ascii(r.label), sans_bold(), 9.0, lh, rgb(brand::INK));
    l.paragraph(
        &ascii(r.validation_status),
        sans(),
        9.0,
        lh,
        rgb(brand::INK_2),
    );
    l.heading("1. What happened");
    l.paragraph(
        &ascii(&r.verdict.sentence),
        serif_bold(),
        10.5,
        5.2,
        rgb(brand::INK),
    );
    l.heading("2. Findings");
    if r.findings.is_empty() {
        l.paragraph(
            "No finding: no monitor or detector fired in the window.",
            serif(),
            10.0,
            5.0,
            rgb(brand::INK),
        );
    }
    for f in &r.findings {
        l.paragraph(
            &ascii(&format!(
                "{} - {} ({} data, MODELLED finding)",
                f.id,
                f.category,
                f.data_class.to_uppercase()
            )),
            sans_bold(),
            10.0,
            5.0,
            rgb(brand::NAVY_INK),
        );
        l.paragraph(&ascii(&f.statement), serif(), 9.5, lh, rgb(brand::INK));
        l.paragraph(
            &ascii(&format!(
                "Onset {} to {} ({}); end {} to {}. Confidence {} ({}/4): {}. Corroborated by: {}.",
                f.onset.earliest_utc,
                f.onset.latest_utc,
                f.onset.basis,
                f.end.earliest_utc,
                f.end.latest_utc,
                f.confidence.level,
                f.confidence.points,
                f.confidence.reasons.join("; "),
                if f.corroborated_by.is_empty() {
                    "none".into()
                } else {
                    f.corroborated_by.join(", ")
                }
            )),
            sans(),
            8.5,
            4.2,
            rgb(brand::INK_2),
        );
        for e in &f.evidence {
            l.paragraph(
                &ascii(&format!(
                    "  {} - {}{}",
                    e.class.to_uppercase(),
                    e.statement,
                    e.input_sha256
                        .as_ref()
                        .map(|h| format!(" (sha256 {})", &h[..16]))
                        .unwrap_or_default()
                )),
                sans(),
                8.0,
                4.0,
                rgb(brand::INK_2),
            );
        }
        l.advance(2.0);
    }
    l.heading("3. When and where");
    for e in &r.timeline {
        l.paragraph(
            &ascii(&format!(
                "{}  {} [{}]",
                e.utc,
                e.text,
                e.class.to_uppercase()
            )),
            sans(),
            8.5,
            4.2,
            rgb(brand::INK),
        );
    }
    l.paragraph(&ascii(&r.area.note), sans(), 8.5, 4.2, rgb(brand::INK_2));
    if let (Some(c), Some(rad)) = (r.area.centroid, r.area.radius_m) {
        l.paragraph(
            &format!(
                "Affected observers {}; centroid {:.5}, {:.5}; radius {:.0} m; unaffected observers inside {}.",
                r.area.affected_observers, c[0], c[1], rad, r.area.unaffected_observers_inside
            ),
            sans(),
            8.5,
            4.2,
            rgb(brand::INK),
        );
    }
    l.heading("4. How confident");
    for x in &r.limits {
        l.paragraph(&ascii(&format!("- {x}")), sans(), 8.5, 4.2, rgb(brand::INK));
    }
    l.heading("5. What it rests on");
    for i in &r.inputs {
        l.paragraph(
            &ascii(&format!(
                "{} ({}, {}, {} bytes) sha256 {}",
                i.label,
                i.role.as_str(),
                i.data_class.to_uppercase(),
                i.bytes,
                i.sha256
            )),
            sans(),
            8.0,
            4.0,
            rgb(brand::INK),
        );
    }
    for c in &r.checklist {
        l.paragraph(
            &ascii(&format!(
                "{} [{}] {} ({}): {}",
                c.id, c.result, c.item, c.required_for, c.detail
            )),
            sans(),
            8.0,
            4.0,
            rgb(brand::INK_2),
        );
    }
    if r.expert_review.required {
        l.paragraph(
            &ascii(&format!(
                "Expert review: {}. Reviewer, qualification, organisation, independence, decision, statement and date are \
                 left blank here and completed only by a named reviewer, signed against this report's hash.",
                r.expert_review.status
            )),
            sans_bold(),
            8.5,
            4.2,
            rgb(brand::INK),
        );
    }
    l.heading("6. How to reproduce it");
    l.paragraph(
        &ascii(&format!(
            "Kshana Pro {}; open engine {}; method {} (parameters sha256 {}). Recipe: {}. Command: {}. {}",
            r.engine.kshana_pro,
            r.engine.open_engine,
            r.engine.method_id,
            r.engine.method_params_sha256,
            r.reproduce.recipe,
            r.reproduce.command,
            r.reproduce.note
        )),
        sans(),
        8.5,
        4.2,
        rgb(brand::INK),
    );
    let mark = crate::brand_pdf::PdfMark::new("Incident forensic report", r.incident.id.clone());
    let pages = l.finish(
        &ascii(&format!(
            "Incident forensic report {} - {}",
            r.incident.id,
            r.variant.as_str()
        )),
        &mark,
    );
    crate::brand_pdf::save(&ascii(&format!("Forensic report {}", r.incident.id)), pages)
}
