// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The programme evidence pack: one self-contained HTML page and one JSON file.
//!
//! The pack holds the verification cross-reference matrix, a traceability graph
//! (requirements to runs, coloured by verdict, dashed where the basis is MODELLED or
//! PARTNER), the reproducibility record of every run, the open items and a sign-off
//! table. The HTML loads nothing from the network: styles and the graph are inline, so
//! it opens offline. The JSON carries the same content, and `pack_sha256` is the
//! SHA-256 of that content, so the same trace always gives the same hash; the optional
//! issue stamp is kept outside it.

use serde::Serialize;

use super::evidence::sha256_hex;
use super::trace::{SignOff, Trace, Vcrm};
use crate::mbse::criterion::Verdict;

/// A node of the traceability graph.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
}

/// An edge of the traceability graph: `requirement` verified by `run`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GraphEdge {
    pub requirement: String,
    pub run: String,
    pub verdict: String,
    pub basis: String,
}

/// The pack body (everything the hash covers).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackBody {
    pub pack_schema: String,
    pub pack_schema_version: u32,
    pub generated_by: String,
    pub vcrm: Vcrm,
    pub graph_nodes: Vec<GraphNode>,
    pub graph_edges: Vec<GraphEdge>,
    pub sign_off: Vec<SignOff>,
}

/// The honesty label every evidence pack carries, in the open engine's `label` form. It
/// sits beside the hashed body, like the issue stamp; the body's matrix carries
/// [`super::trace::VCRM_LABEL`] inside the hash.
pub const PACK_LABEL: &str = "MODELLED - an evidence pack over open-engine simulation runs: the matrix, the requirement-to-run graph and the sign-off block rest on modelled runs, each with its own evidence basis. It is not a verification on hardware and not a certification.";

/// The evidence pack.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pack {
    /// The honesty label, [`PACK_LABEL`].
    pub label: &'static str,
    pub body: PackBody,
    pub pack_sha256: String,
    pub issued_utc: Option<String>,
}

/// Build the pack for a trace.
pub fn build(trace: &Trace, issued_utc: Option<String>) -> Pack {
    let v = super::trace::vcrm(trace);
    let mut nodes: Vec<GraphNode> = v
        .rows
        .iter()
        .map(|r| GraphNode {
            id: r.requirement.clone(),
            kind: "requirement".into(),
            label: r.requirement.clone(),
        })
        .collect();
    nodes.extend(v.runs.iter().map(|r| GraphNode {
        id: r.id.clone(),
        kind: "run".into(),
        label: format!(
            "{} ({})",
            r.id,
            r.kind.clone().unwrap_or_else(|| "?".into())
        ),
    }));
    let edges = v
        .rows
        .iter()
        .flat_map(|r| {
            r.cells.iter().map(move |c| GraphEdge {
                requirement: r.requirement.clone(),
                run: c.run.clone(),
                verdict: c.verdict.as_str().into(),
                basis: c.basis.label.clone(),
            })
        })
        .collect();
    let sign_off = if trace.sign_off.is_empty() {
        ["Prepared by", "Checked by", "Approved by"]
            .iter()
            .map(|r| SignOff {
                role: r.to_string(),
                name: String::new(),
            })
            .collect()
    } else {
        trace.sign_off.clone()
    };
    let body = PackBody {
        pack_schema: "kshana-pro-mbse-evidence-pack".into(),
        pack_schema_version: 1,
        generated_by: format!("Kshana Pro {} (MBSE)", crate::version()),
        vcrm: v,
        graph_nodes: nodes,
        graph_edges: edges,
        sign_off,
    };
    let canonical = serde_json::to_string(&body).expect("pack body serialises");
    Pack {
        label: PACK_LABEL,
        pack_sha256: sha256_hex(canonical.as_bytes()),
        body,
        issued_utc,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn verdict_class(v: &str) -> &'static str {
    match v {
        "met" => "met",
        "not met" => "fail",
        _ => "nv",
    }
}

fn graph_svg(p: &PackBody) -> String {
    let reqs: Vec<&GraphNode> = p
        .graph_nodes
        .iter()
        .filter(|n| n.kind == "requirement")
        .collect();
    let runs: Vec<&GraphNode> = p.graph_nodes.iter().filter(|n| n.kind == "run").collect();
    let row = 34.0;
    let n = reqs.len().max(runs.len()).max(1) as f64;
    let h = 40.0 + n * row;
    let (w, lx, rx, bw) = (760.0, 20.0, 480.0, 260.0);
    let y_of = |i: usize, count: usize| {
        let span = n * row;
        let step = span / count.max(1) as f64;
        30.0 + step * i as f64 + step / 2.0
    };
    let mut s = format!(
        "<svg class=\"graph\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"Traceability graph: requirements on the left, runs on the right, one line per verification link\" xmlns=\"http://www.w3.org/2000/svg\">"
    );
    s += &format!("<text x=\"{lx}\" y=\"18\" class=\"g-h\">Requirements</text><text x=\"{rx}\" y=\"18\" class=\"g-h\">Kshana runs</text>");
    for e in &p.graph_edges {
        let (Some(ri), Some(ui)) = (
            reqs.iter().position(|n| n.id == e.requirement),
            runs.iter().position(|n| n.id == e.run),
        ) else {
            continue;
        };
        let (y1, y2) = (y_of(ri, reqs.len()), y_of(ui, runs.len()));
        let x1 = lx + bw;
        let dash = if e.basis == "VALIDATED" {
            ""
        } else {
            " stroke-dasharray=\"6 4\""
        };
        s += &format!(
            "<path d=\"M{x1} {y1} C{} {y1}, {} {y2}, {rx} {y2}\" class=\"edge {}\"{dash}><title>{} ← {}: {} ({} basis)</title></path>",
            x1 + 90.0,
            rx - 90.0,
            verdict_class(&e.verdict),
            esc(&e.requirement),
            esc(&e.run),
            esc(&e.verdict),
            esc(&e.basis)
        );
    }
    for (i, r) in reqs.iter().enumerate() {
        let y = y_of(i, reqs.len());
        s += &format!(
            "<rect x=\"{lx}\" y=\"{}\" width=\"{bw}\" height=\"24\" rx=\"4\" class=\"node\"/><text x=\"{}\" y=\"{}\" class=\"n-t\">{}</text>",
            y - 12.0,
            lx + 10.0,
            y + 4.0,
            esc(&r.label)
        );
    }
    for (i, r) in runs.iter().enumerate() {
        let y = y_of(i, runs.len());
        s += &format!(
            "<rect x=\"{rx}\" y=\"{}\" width=\"{bw}\" height=\"24\" rx=\"4\" class=\"node run\"/><text x=\"{}\" y=\"{}\" class=\"n-t\">{}</text>",
            y - 12.0,
            rx + 10.0,
            y + 4.0,
            esc(&r.label)
        );
    }
    s += "</svg>";
    s
}

impl Pack {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("pack serialises")
    }

    pub fn to_html(&self) -> String {
        // Every Kshana Pro document is an Ashforde document: the shared furniture of
        // crate::report_brand goes around the renderer's own page, which is unchanged.
        let body = self.to_html_unbranded();
        let id = crate::report_brand::content_id(&body);
        crate::report_brand::finish_html(
            &body,
            &crate::report_brand::DocMark::new("Model-based systems engineering evidence pack", id),
        )
    }

    fn to_html_unbranded(&self) -> String {
        let b = &self.body;
        let v = &b.vcrm;
        let mut h = String::new();
        h += "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">";
        h += &format!("<title>{} evidence pack</title>", esc(&v.title));
        h += "<style>
:root{--bg:#f7f5f0;--fg:#1d1b17;--muted:#6b645a;--line:#d9d3c7;--card:#fffdf8;--met:#1f7a52;--fail:#b3261e;--nv:#8a6d00;--mod:#6a3fb5;--node:#efe9dd}
@media (prefers-color-scheme:dark){:root{--bg:#12110e;--fg:#ece6da;--muted:#a39b8d;--line:#39342b;--card:#1b1915;--met:#4fc38a;--fail:#f2837a;--nv:#e2c15a;--mod:#b99af0;--node:#26231d}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,Segoe UI,sans-serif}
main{max-width:1100px;margin:0 auto;padding:24px 16px 64px}h1{font-size:26px;margin:0 0 4px}h2{font-size:18px;margin:36px 0 10px;border-bottom:1px solid var(--line);padding-bottom:6px}
.meta{color:var(--muted);font-size:13px}.tiles{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;margin:18px 0}
.tile{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:10px 12px}.tile b{display:block;font-size:24px}
.scroll{overflow-x:auto;border:1px solid var(--line);border-radius:8px;background:var(--card)}table{border-collapse:collapse;width:100%;font-size:13px}
th,td{text-align:left;vertical-align:top;padding:7px 9px;border-bottom:1px solid var(--line)}th{font-weight:600;background:var(--node)}
code{font:12px/1.4 ui-monospace,Menlo,Consolas,monospace;word-break:break-word}.chip{display:inline-block;padding:1px 7px;border-radius:10px;font-size:12px;font-weight:600;border:1px solid currentColor;white-space:nowrap}
.met{color:var(--met)}.fail{color:var(--fail)}.nv{color:var(--nv)}.mod{color:var(--mod)}.val{color:var(--met)}
.graph{width:100%;height:auto;background:var(--card);border:1px solid var(--line);border-radius:8px}.g-h{font-size:12px;font-weight:600;fill:var(--muted)}
.node{fill:var(--node);stroke:var(--line)}.n-t{font-size:12px;fill:var(--fg)}.edge{fill:none;stroke-width:2}.edge.met{stroke:var(--met)}.edge.fail{stroke:var(--fail)}.edge.nv{stroke:var(--nv)}
ul.items{padding-left:18px}ul.items li{margin:4px 0}.sig td{height:42px}.small{font-size:12px;color:var(--muted)}
</style></head><body><main>";
        h += &format!("<h1>{}</h1>", esc(&v.title));
        h += &format!(
            "<div class=\"meta\">Programme evidence pack · {} · pack SHA-256 <code>{}</code>{}</div>",
            esc(&b.generated_by),
            self.pack_sha256,
            self.issued_utc
                .as_ref()
                .map(|u| format!(" · issued {}", esc(u)))
                .unwrap_or_default()
        );
        if let Some(p) = &v.programme {
            h += &format!("<p>{}</p>", esc(p));
        }
        h += "<p class=\"small\">Every verdict below was computed from a result document the open Kshana engine wrote. Evidence is the run's reproducibility stamp (scenario hash, seed, engine version) plus each field path and value read. A verdict on a MODELLED or PARTNER basis is flagged: the capability behind that figure is not externally validated in the open verification matrix. The verification cross-reference matrix (VCRM) lists one line per requirement and run.</p>";
        let sm = &v.summary;
        h += "<div class=\"tiles\">";
        for (label, n, cls) in [
            ("Requirements", sm.requirements, ""),
            ("Met", sm.met, "met"),
            (
                "Met on a MODELLED or PARTNER basis",
                sm.met_on_modelled_basis,
                "mod",
            ),
            ("Not met", sm.not_met, "fail"),
            ("Not verified", sm.not_verified, "nv"),
            ("Open items", sm.open_items, ""),
        ] {
            h += &format!("<div class=\"tile\"><b class=\"{cls}\">{n}</b>{label}</div>");
        }
        h += "</div>";

        h += "<h2>Verification cross-reference matrix</h2><div class=\"scroll\"><table><thead><tr><th>Requirement</th><th>Method</th><th>Acceptance criterion</th><th>Run</th><th>Verdict</th><th>Basis</th><th>Evidence</th></tr></thead><tbody>";
        for r in &v.rows {
            let rowspan = r.cells.len().max(1);
            let head = format!(
                "<td rowspan=\"{rowspan}\"><b>{}</b><div class=\"small\">{}</div><div>overall: <span class=\"chip {}\">{}</span></div></td><td rowspan=\"{rowspan}\">{}</td><td rowspan=\"{rowspan}\"><code>{}</code></td>",
                esc(&r.requirement),
                esc(&r.text),
                verdict_class(r.verdict.as_str()),
                r.verdict.as_str(),
                r.method.as_str(),
                esc(r.criterion.as_deref().unwrap_or("none"))
            );
            if r.cells.is_empty() {
                h += &format!("<tr>{head}<td>—</td><td><span class=\"chip nv\">not verified</span></td><td>—</td><td>no run traced</td></tr>");
            }
            for (i, c) in r.cells.iter().enumerate() {
                let ev = if c.evidence.observations.is_empty() {
                    esc(c.reason.as_deref().unwrap_or(""))
                } else {
                    c.evidence
                        .observations
                        .iter()
                        .map(|o| {
                            format!(
                                "<div><code>{}</code> = <code>{}</code>{}<br><span class=\"small\">{}{}</span></div>",
                                esc(&o.path),
                                esc(&o.value.to_string()),
                                o.unit.as_ref().map(|u| format!(" {}", esc(u))).unwrap_or_default(),
                                esc(&o.detail),
                                o.unit_source
                                    .as_ref()
                                    .map(|s| format!("; unit from {}", esc(s)))
                                    .unwrap_or_default()
                            )
                        })
                        .collect::<String>()
                        + &c.reason
                            .as_ref()
                            .filter(|_| c.verdict == Verdict::NotVerified)
                            .map(|r| format!("<div class=\"small\">{}</div>", esc(r)))
                            .unwrap_or_default()
                };
                let stamp = format!(
                    "<div class=\"small\">hash <code>{}</code> · seed {} · engine {}</div>",
                    esc(&c
                        .evidence
                        .scenario_hash
                        .clone()
                        .unwrap_or_else(|| "not stamped".into())),
                    c.evidence
                        .seed
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "—".into()),
                    esc(&c
                        .evidence
                        .engine_version
                        .clone()
                        .unwrap_or_else(|| "—".into()))
                );
                let basis_cls = if c.basis.modelled_basis { "mod" } else { "val" };
                let caps = c
                    .basis
                    .contributions
                    .iter()
                    .map(|b| {
                        format!(
                            "{}{}",
                            b.label,
                            b.capability
                                .as_ref()
                                .map(|x| format!(": {}", esc(x)))
                                .unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("<br>");
                h += &format!(
                    "<tr>{}<td>{}{stamp}</td><td><span class=\"chip {}\">{}</span></td><td><span class=\"chip {basis_cls}\">{}{}</span><div class=\"small\">{caps}</div></td><td>{ev}</td></tr>",
                    if i == 0 { head.clone() } else { String::new() },
                    esc(&c.run),
                    verdict_class(c.verdict.as_str()),
                    c.verdict.as_str(),
                    c.basis.label,
                    if c.basis.modelled_basis { " (flagged)" } else { "" }
                );
            }
        }
        h += "</tbody></table></div>";

        h += "<h2>Traceability graph</h2><p class=\"small\">Solid line: VALIDATED basis. Dashed line: MODELLED or PARTNER basis. Green met, red not met, amber not verified.</p>";
        h += &graph_svg(b);

        h += "<h2>Reproducibility record</h2><div class=\"scroll\"><table><thead><tr><th>Run</th><th>Kind</th><th>Scenario</th><th>Stamp</th><th>Files (SHA-256)</th><th>Integrity</th></tr></thead><tbody>";
        for r in &v.runs {
            let st = &r.stamp;
            h += &format!(
                "<tr><td><b>{}</b><div class=\"small\">{}</div></td><td>{}<div class=\"small\">{}</div></td><td>{}<div class=\"small\">{}</div></td><td>scenario_hash <code>{}</code><div class=\"small\">{}</div>seed {} <span class=\"small\">({})</span><br>engine {} <span class=\"small\">({})</span><br>schema {} · platform {}</td><td>result <code>{}</code><br><code>{}</code>{}</td><td>{}</td></tr>",
                esc(&r.id),
                esc(r.description.as_deref().unwrap_or("")),
                esc(r.kind.as_deref().unwrap_or("?")),
                esc(&r.kind_source),
                esc(r.scenario_file.as_ref().map(|f| f.path.as_str()).unwrap_or("not given")),
                r.scenario_file.as_ref().map(|f| format!("<code>{}</code>", f.sha256)).unwrap_or_default(),
                esc(st.scenario_hash.as_deref().unwrap_or("not stamped")),
                esc(&st.scenario_hash_source),
                st.seed.map(|s| s.to_string()).unwrap_or_else(|| "—".into()),
                esc(&st.seed_source),
                esc(st.engine_version.as_deref().unwrap_or("—")),
                esc(&st.engine_version_source),
                esc(st.schema_version.as_deref().unwrap_or("—")),
                esc(st.platform.as_deref().unwrap_or("—")),
                esc(&r.result_file.path),
                r.result_file.sha256,
                r.report_file
                    .as_ref()
                    .map(|f| format!("<br>report <code>{}</code><br><code>{}</code>", esc(&f.path), f.sha256))
                    .unwrap_or_default(),
                if r.integrity_problems.is_empty() {
                    let checked = [
                        st.scenario_sha256_recorded.as_ref().map(|_| "scenario hash matches the report"),
                        st.result_sha256_recorded.as_ref().map(|_| "result hash matches the report"),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("; ");
                    format!(
                        "<span class=\"chip met\">intact</span><div class=\"small\">{}</div>",
                        if checked.is_empty() { "no report hash to check against".to_string() } else { checked }
                    )
                } else {
                    format!(
                        "<span class=\"chip fail\">problem</span><div class=\"small\">{}</div>",
                        esc(&r.integrity_problems.join("; "))
                    )
                }
            );
        }
        h += "</tbody></table></div>";

        h += &format!("<h2>Open items ({})</h2>", v.open_items.len());
        if v.open_items.is_empty() {
            h += "<p>None.</p>";
        } else {
            h += "<ul class=\"items\">";
            for o in &v.open_items {
                h += &format!(
                    "<li><b>{}</b>{}{}: {}</li>",
                    esc(&o.category),
                    o.requirement
                        .as_ref()
                        .map(|r| format!(" · {}", esc(r)))
                        .unwrap_or_default(),
                    o.run
                        .as_ref()
                        .map(|r| format!(" · {}", esc(r)))
                        .unwrap_or_default(),
                    esc(&o.text)
                );
            }
            h += "</ul>";
        }

        h += "<h2>Sign-off</h2><div class=\"scroll\"><table class=\"sig\"><thead><tr><th>Role</th><th>Name</th><th>Signature</th><th>Date</th></tr></thead><tbody>";
        for s in &b.sign_off {
            h += &format!(
                "<tr><td>{}</td><td>{}</td><td></td><td></td></tr>",
                esc(&s.role),
                esc(&s.name)
            );
        }
        h += "</tbody></table></div>";

        h += "<h2>Inputs</h2><div class=\"scroll\"><table><thead><tr><th>File</th><th>SHA-256</th></tr></thead><tbody>";
        for f in &v.inputs {
            h += &format!(
                "<tr><td>{}</td><td><code>{}</code></td></tr>",
                esc(&f.path),
                f.sha256
            );
        }
        h += "</tbody></table></div>";
        h += "</main></body></html>\n";
        h
    }
}
