// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The timing fleet page (roadmap 0.42 T2): one self-contained HTML page with every clock,
//! earliest warning first, its source and evidence class, the time to breach of every limit,
//! the predicted 95 % bound against the limits, and the clock's evidence history. The Station
//! console serves it (`GET /timing/fleet`) and `kshana-pro timing console` writes it.

use super::evidence::Entry;
use super::fleet::{human_duration, human_time, ClockAssessment, FleetAssessment};
use super::model::{NoiseSource, Z95};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn opt(s: &Option<String>) -> String {
    esc(s.as_deref().unwrap_or("—"))
}

/// The predicted 95 % bound `|m| + 1.96 s` against the limits, on a log time-error axis.
fn chart(c: &ClockAssessment, horizon_s: f64) -> String {
    let (w, h, l, r, t, b) = (360.0, 140.0, 44.0, 8.0, 8.0, 22.0);
    let n = 80;
    let pts: Vec<(f64, f64)> = (1..=n)
        .map(|k| {
            let u = horizon_s * k as f64 / n as f64;
            let p = c.holdover.predict(u);
            (u, p.mean_s.abs() + Z95 * p.sigma_s)
        })
        .collect();
    let lim_max = c.breaches.iter().map(|b| b.value_s).fold(0.0f64, f64::max);
    let ymax = pts.iter().map(|p| p.1).fold(lim_max, f64::max).max(1e-12) * 2.0;
    let ymin = pts
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min)
        .min(lim_max)
        .max(1e-13)
        / 2.0;
    let (ly0, ly1) = (ymin.log10(), ymax.log10());
    let px = |u: f64| l + (w - l - r) * u / horizon_s;
    let py = |v: f64| t + (h - t - b) * (1.0 - (v.max(ymin).log10() - ly0) / (ly1 - ly0));
    let path: String = pts
        .iter()
        .enumerate()
        .map(|(i, (u, v))| {
            format!(
                "{}{:.1},{:.1}",
                if i == 0 { "M" } else { "L" },
                px(*u),
                py(*v)
            )
        })
        .collect();
    let mut s = format!(
        "<svg viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"Predicted 95 % holdover bound of {} against its limits\" class=\"chart\">",
        esc(&c.id)
    );
    s.push_str(&format!(
        "<line x1=\"{l}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"axis\"/>",
        h - b,
        w - r,
        h - b
    ));
    for bq in &c.breaches {
        let y = py(bq.value_s);
        s.push_str(&format!(
            "<line x1=\"{l}\" y1=\"{y:.1}\" x2=\"{}\" y2=\"{y:.1}\" class=\"limit\"/><text x=\"{}\" y=\"{:.1}\" class=\"lt\" text-anchor=\"end\">{}</text>",
            w - r,
            w - r - 2.0,
            y - 3.0,
            esc(&human_time(bq.value_s))
        ));
    }
    s.push_str(&format!("<path d=\"{path}\" class=\"bound\"/>"));
    s.push_str(&format!(
        "<text x=\"{l}\" y=\"{}\" class=\"lt\">0</text><text x=\"{}\" y=\"{}\" class=\"lt\" text-anchor=\"end\">{}</text>",
        h - 6.0,
        w - r,
        h - 6.0,
        esc(&human_duration(Some(horizon_s)))
    ));
    s.push_str(&format!(
        "<text x=\"2\" y=\"{:.1}\" class=\"lt\">{}</text><text x=\"2\" y=\"{:.1}\" class=\"lt\">{}</text>",
        t + 8.0,
        esc(&human_time(ymax)),
        h - b,
        esc(&human_time(ymin))
    ));
    s.push_str("</svg>");
    s
}

/// The page. `history` gives each clock's evidence entries (oldest first); pass empty
/// vectors when there is no history yet.
pub fn page(a: &FleetAssessment, history: &[(String, Vec<Entry>)], served: &str) -> String {
    let mut rows = String::new();
    let mut cards = String::new();
    for id in &a.order {
        let Some(c) = a.clock(id) else { continue };
        let lims: String = c
            .breaches
            .iter()
            .map(|b| {
                format!(
                    "<div><span class=\"ln\">{}</span> warn {} · median {}</div>",
                    esc(&b.name),
                    esc(&human_duration(b.t_warn_s)),
                    esc(&human_duration(b.t_50_s))
                )
            })
            .collect();
        let ev = match c.noise_source {
            NoiseSource::Measured => "measured".to_string(),
            NoiseSource::Prior => format!("prior ({})", c.prior.as_deref().unwrap_or("stated")),
        };
        rows.push_str(&format!(
            "<tr><td><a href=\"#c-{id}\">{id}</a></td><td>{}</td><td>{} {}</td><td>{}</td><td><code>{}</code> · {}</td><td>{}</td><td>{}</td><td><span class=\"st st-{st}\">{st}</span></td></tr>",
            opt(&c.site),
            opt(&c.vendor),
            opt(&c.model),
            opt(&c.role),
            esc(&c.source_format),
            esc(&c.record),
            esc(&ev),
            lims,
            id = esc(&c.id),
            st = esc(&c.status),
        ));
        let hist = history
            .iter()
            .find(|(k, _)| k == &c.id)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[]);
        let mut hrows = String::new();
        for e in hist.iter().rev().take(12) {
            let warn = e
                .breaches
                .iter()
                .filter_map(|b| b.t_warn_s)
                .min_by(f64::total_cmp);
            hrows.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td><span class=\"st st-{s}\">{s}</span></td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
                e.seq,
                esc(&e.utc),
                esc(&human_duration(warn)),
                esc(&human_time(e.phase_s)),
                esc(&e.sha256[..12.min(e.sha256.len())]),
                s = esc(&e.status)
            ));
        }
        let htable = if hrows.is_empty() {
            "<p class=\"sub\">No evidence history yet.</p>".to_string()
        } else {
            format!(
                "<div class=\"wrap\"><table class=\"h\"><thead><tr><th>#</th><th>UTC</th><th>Status</th><th>Earliest warning</th><th>Offset</th><th>Entry</th></tr></thead><tbody>{hrows}</tbody></table></div>"
            )
        };
        let preds: String = c
            .predictions
            .iter()
            .map(|p| {
                format!(
                    "<li>{}: {} ± {}</li>",
                    esc(&human_duration(Some(p.u_s))),
                    esc(&human_time(p.mean_s)),
                    esc(&human_time(Z95 * p.sigma_s))
                )
            })
            .collect();
        cards.push_str(&format!(
            "<section class=\"card\" id=\"c-{id}\"><h2>{id} <span class=\"st st-{st}\">{st}</span></h2><p class=\"sub\">{} · {} {} · {} · source <code>{}</code> ({} samples at {} s, record <code>{}</code>)</p><div class=\"cols\"><div>{}</div><div><h3>Predicted holdover error (95 %)</h3><ul>{preds}</ul></div></div><h3>Evidence history</h3>{htable}</section>",
            opt(&c.site),
            opt(&c.vendor),
            opt(&c.model),
            opt(&c.role),
            esc(&c.source_format),
            c.n_used,
            c.tau0_s,
            esc(&c.record_sha256[..12]),
            chart(c, a.horizon_s),
            id = esc(&c.id),
            st = esc(&c.status),
        ));
    }
    let k = &a.counts;
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Timing fleet</title>
<style>
:root {{ color-scheme: light dark; --bg:#f6f7f9; --surface:#fff; --ink:#14171c; --muted:#5b6472; --line:#dde1e7;
 --ok:#17663a; --ok-bg:#dcf3e5; --watch:#6b5b00; --watch-bg:#fff4c2; --act:#9b1c1c; --act-bg:#fbe1e1; --accent:#0b57a8;
 --mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; --sans: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{ --bg:#0f1216; --surface:#171b21; --ink:#e7eaee; --muted:#9aa3af; --line:#2a3039;
 --ok:#8fdcaf; --ok-bg:#123824; --watch:#f1d66b; --watch-bg:#3a3210; --act:#f3a3a3; --act-bg:#441717; --accent:#8cc4f5; }} }}
:root[data-theme="dark"] {{ --bg:#0f1216; --surface:#171b21; --ink:#e7eaee; --muted:#9aa3af; --line:#2a3039;
 --ok:#8fdcaf; --ok-bg:#123824; --watch:#f1d66b; --watch-bg:#3a3210; --act:#f3a3a3; --act-bg:#441717; --accent:#8cc4f5; }}
* {{ box-sizing: border-box; }}
body {{ margin:0; background:var(--bg); color:var(--ink); font:15px/1.5 var(--sans); }}
main {{ max-width:1280px; margin:0 auto; padding:24px 16px 48px; }}
h1 {{ font-size:22px; margin:0 0 4px; }} h2 {{ font-size:18px; margin:0 0 4px; }} h3 {{ font-size:13px; text-transform:uppercase; letter-spacing:.04em; color:var(--muted); margin:16px 0 6px; }}
.sub {{ color:var(--muted); margin:0 0 16px; }}
.tiles {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(130px,1fr)); gap:12px; margin-bottom:20px; }}
.tile {{ background:var(--surface); border:1px solid var(--line); border-radius:8px; padding:12px 14px; }}
.tile b {{ display:block; font-size:24px; font-variant-numeric:tabular-nums; }} .tile span {{ color:var(--muted); font-size:13px; }}
.wrap {{ background:var(--surface); border:1px solid var(--line); border-radius:8px; overflow-x:auto; }}
table {{ width:100%; border-collapse:collapse; min-width:860px; }} table.h {{ min-width:560px; }}
th,td {{ text-align:left; padding:8px 12px; border-bottom:1px solid var(--line); vertical-align:top; font-size:14px; }}
th {{ font-size:12px; text-transform:uppercase; letter-spacing:.04em; color:var(--muted); font-weight:600; }}
code {{ font-family:var(--mono); font-size:13px; }} a {{ color:var(--accent); }}
.ln {{ color:var(--muted); }}
.st {{ display:inline-block; padding:1px 8px; border-radius:999px; font-size:12px; font-weight:600; }}
.st-ok {{ color:var(--ok); background:var(--ok-bg); }} .st-watch {{ color:var(--watch); background:var(--watch-bg); }}
.st-act,.st-breached {{ color:var(--act); background:var(--act-bg); }}
.card {{ background:var(--surface); border:1px solid var(--line); border-radius:8px; padding:16px; margin-top:16px; }}
.cols {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(300px,1fr)); gap:16px; }}
.chart {{ width:100%; max-width:480px; height:auto; }} .chart .axis {{ stroke:var(--line); }}
.chart .limit {{ stroke:var(--act); stroke-dasharray:4 3; }} .chart .bound {{ fill:none; stroke:var(--accent); stroke-width:2; }}
.chart .lt {{ fill:var(--muted); font-size:10px; font-family:var(--mono); }}
.note {{ color:var(--muted); font-size:13px; margin-top:24px; }}
</style></head>
<body><main>
<h1>{title}</h1>
<p class="sub">{fleet} · {n} clocks · window {win} · horizon {hor} · {served}</p>
<div class="tiles"><div class="tile"><b>{n}</b><span>clocks</span></div><div class="tile"><b>{act}</b><span>act (warning within 4 h)</span></div><div class="tile"><b>{watch}</b><span>watch</span></div><div class="tile"><b>{ok}</b><span>ok within horizon</span></div><div class="tile"><b>{br}</b><span>beyond a limit now</span></div></div>
<div class="wrap"><table><thead><tr><th>Clock</th><th>Site</th><th>Vendor and model</th><th>Role</th><th>Source</th><th>Noise</th><th>Time to breach</th><th>Status</th></tr></thead><tbody>
{rows}</tbody></table></div>
{cards}
<p class="note">{label}<br>"warn" is the first time the breach probability reaches 5 %, "median" the first time it reaches 50 %. Limits are values the operator set. A <em>prior</em> clock's record is a servo offset; its noise comes from the stated class or datasheet.</p>
</main></body></html>
"#,
        title = esc(&a.title),
        fleet = esc(&a.fleet),
        n = k.clocks,
        act = k.act,
        watch = k.watch,
        ok = k.ok,
        br = k.breached,
        win = esc(&human_duration(Some(a.window_s))),
        hor = esc(&human_duration(Some(a.horizon_s))),
        served = esc(served),
        label = esc(&a.label),
    )
}
