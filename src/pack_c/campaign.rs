// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Campaign figures** — named numbers read out of an open-engine `kind = "campaign"`
//! result, so a watch can name "the time error at the end of the holdover phase"
//! instead of an index into a 458-sample array.
//!
//! Every figure is read from the campaign's own result document; nothing is recomputed
//! from the member scenarios. The names, all under `campaign.`:
//!
//! | Figure | Unit | What it is |
//! |---|---|---|
//! | `phase.<p>.t0_s`, `.t1_s`, `.duration_s` | s | the phase window on the mission timeline |
//! | `phase.<p>.end.<channel>` | the channel's | the channel's value at the last grid time the phase owns |
//! | `phase.<p>.carried.<channel>` | the channel's | the value carried into the phase from the end of the previous one |
//! | `first_alarm_s` | s | the first grid time at which the alarm channel is 1 (absent when it never alarms) |
//! | `alarm_share` | 1 | the share of grid times with a value at which the alarm is 1 |
//! | `event.e<k>.t_s` | s | the time of the k-th timeline event, in time order, from 1 |
//! | `handoff.h<k>.value` | the handoff's | the k-th number handed between phases, from 1 |
//! | `mc.<m>.<stat>` | the metric's | a Monte Carlo metric's `mean`, `std`, `n`, `p05`, `p50`, `p95`, `ci95_low`, `ci95_high` |
//! | `sweep.<m>.min`, `.max` | the metric's | the smallest and largest node value of a sweep metric |
//!
//! `<p>` and `<m>` are the phase and metric names with every character other than a
//! letter, a digit or `_` replaced by `_` (the criterion grammar has no `-`), so the
//! phase `integrity-alarm` is `campaign.phase.integrity_alarm`.
//!
//! **Which grid time is the end of a phase.** The engine assigns grid time `t` to the
//! phase whose window `[t0, t1)` holds it, the last phase also owning its own end, and a
//! grid time takes the latest sample of the owning phase's run at or before it. The end
//! value of a phase is therefore the value at the last grid time `t < t1` (or `t <= t1`
//! for the last phase). When that value is null the figure is absent, never guessed.
//!
//! A sweep crossing — the first value of the first axis at which a metric falls below
//! (or rises above) a threshold the watch states — is computed per watch by
//! [`sweep_crossing`].

use crate::pack_c::watch::Crossing;
use serde_json::{json, Map, Value};

/// One named campaign figure.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedFigure {
    /// The dotted name under `campaign.` (for example `phase.holdover.end.time_error_ns`).
    pub name: String,
    /// The value, read from the campaign result.
    pub value: f64,
    /// Its unit.
    pub unit: String,
    /// What it is, in words.
    pub note: String,
}

/// A phase or metric name made safe for the criterion grammar.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

const T_EPS: f64 = 1e-9;

fn f(v: &Value) -> Option<f64> {
    v.as_f64().filter(|x| x.is_finite())
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Every named figure of a campaign result (empty for any other kind).
pub fn figures(doc: &Value) -> Vec<NamedFigure> {
    if doc.get("kind").and_then(Value::as_str) != Some("campaign") {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Some(tl) = doc.get("timeline") {
        timeline_figures(tl, &mut out);
    }
    if let Some(mc) = doc.get("monte_carlo") {
        mc_figures(mc, &mut out);
    }
    if let Some(sw) = doc.get("sweep") {
        sweep_figures(sw, &mut out);
    }
    out
}

fn timeline_figures(tl: &Value, out: &mut Vec<NamedFigure>) {
    let t_s: Vec<f64> = tl
        .get("t_s")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_default();
    let phases = tl
        .get("phases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let channels = tl
        .get("channels")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let n_ph = phases.len();
    for (pi, ph) in phases.iter().enumerate() {
        let name = sanitize(&s(ph, "name"));
        let (Some(t0), Some(t1)) = (ph.get("t0_s").and_then(f), ph.get("t1_s").and_then(f)) else {
            continue;
        };
        let last = pi + 1 == n_ph;
        let base = format!("phase.{name}");
        out.push(NamedFigure {
            name: format!("{base}.t0_s"),
            value: t0,
            unit: "s".into(),
            note: format!("start of phase '{}' on the mission timeline", s(ph, "name")),
        });
        out.push(NamedFigure {
            name: format!("{base}.t1_s"),
            value: t1,
            unit: "s".into(),
            note: format!(
                "end of phase '{}' on the mission timeline ({})",
                s(ph, "name"),
                s(ph, "ended_by")
            ),
        });
        out.push(NamedFigure {
            name: format!("{base}.duration_s"),
            value: t1 - t0,
            unit: "s".into(),
            note: format!("length of phase '{}'", s(ph, "name")),
        });
        // The last grid index the phase owns.
        let owned = t_s
            .iter()
            .enumerate()
            .rev()
            .find(|(_, &t)| t >= t0 - T_EPS && (t < t1 - T_EPS || (last && t <= t1 + T_EPS)));
        if let Some((k, &tk)) = owned {
            for (ch, c) in &channels {
                let unit = c
                    .get("unit")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let v = c
                    .get("values")
                    .and_then(Value::as_array)
                    .and_then(|a| a.get(k))
                    .and_then(f);
                if let Some(v) = v {
                    out.push(NamedFigure {
                        name: format!("{base}.end.{}", sanitize(ch)),
                        value: v,
                        unit,
                        note: format!(
                            "channel '{ch}' at t = {tk} s, the last grid time of phase '{}'",
                            s(ph, "name")
                        ),
                    });
                }
            }
        }
        if let Some(carried) = ph.get("carried").and_then(Value::as_object) {
            for (ch, v) in carried {
                if let Some(v) = f(v) {
                    let unit = channels
                        .get(ch)
                        .and_then(|c| c.get("unit"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    out.push(NamedFigure {
                        name: format!("{base}.carried.{}", sanitize(ch)),
                        value: v,
                        unit,
                        note: format!(
                            "'{ch}' carried into phase '{}' from the end of the previous phase",
                            s(ph, "name")
                        ),
                    });
                }
            }
        }
    }
    if let Some(alarm) = channels
        .get("alarm")
        .and_then(|c| c.get("values"))
        .and_then(Value::as_array)
    {
        let present: Vec<(usize, f64)> = alarm
            .iter()
            .enumerate()
            .filter_map(|(i, v)| f(v).map(|x| (i, x)))
            .collect();
        if !present.is_empty() {
            let on = present.iter().filter(|(_, v)| *v > 0.5).count();
            out.push(NamedFigure {
                name: "alarm_share".into(),
                value: on as f64 / present.len() as f64,
                unit: "1".into(),
                note: format!(
                    "share of the {} grid times with an alarm value at which the alarm is up",
                    present.len()
                ),
            });
            if let Some((i, _)) = present.iter().find(|(_, v)| *v > 0.5) {
                if let Some(t) = t_s.get(*i).copied().filter(|t| t.is_finite()) {
                    out.push(NamedFigure {
                        name: "first_alarm_s".into(),
                        value: t,
                        unit: "s".into(),
                        note: "first grid time at which the alarm channel is 1".into(),
                    });
                }
            }
        }
    }
    if let Some(events) = tl.get("events").and_then(Value::as_array) {
        for (k, e) in events.iter().enumerate() {
            if let Some(t) = e.get("t_s").and_then(f) {
                out.push(NamedFigure {
                    name: format!("event.e{}.t_s", k + 1),
                    value: t,
                    unit: "s".into(),
                    note: format!("event in phase '{}': {}", s(e, "phase"), s(e, "label")),
                });
            }
        }
    }
    if let Some(handoffs) = tl.get("handoffs").and_then(Value::as_array) {
        for (k, h) in handoffs.iter().enumerate() {
            if let Some(v) = h.get("value").and_then(f) {
                out.push(NamedFigure {
                    name: format!("handoff.h{}.value", k + 1),
                    value: v,
                    unit: s(h, "unit"),
                    note: format!("number handed on between phases: {}", compact(h)),
                });
            }
        }
    }
}

fn compact(v: &Value) -> String {
    match v.as_object() {
        Some(m) => m
            .iter()
            .filter(|(k, _)| k.as_str() != "value")
            .map(|(k, x)| match x.as_str() {
                Some(t) => format!("{k} {t}"),
                None => format!("{k} {x}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
        None => v.to_string(),
    }
}

const MC_STATS: [&str; 8] = [
    "mean",
    "std",
    "n",
    "p05",
    "p50",
    "p95",
    "ci95_low",
    "ci95_high",
];

fn mc_figures(mc: &Value, out: &mut Vec<NamedFigure>) {
    let Some(metrics) = mc.get("metrics").and_then(Value::as_object) else {
        return;
    };
    for (m, stats) in metrics {
        let unit = s(stats, "unit");
        for st in MC_STATS {
            if let Some(v) = stats.get(st).and_then(f) {
                out.push(NamedFigure {
                    name: format!("mc.{}.{st}", sanitize(m)),
                    value: v,
                    unit: if st == "n" {
                        "count".into()
                    } else {
                        unit.clone()
                    },
                    note: format!("Monte Carlo metric '{m}' ({}): {st}", s(stats, "path")),
                });
            }
        }
    }
}

/// A sweep node's value of `metric`: the number, or the `mean` of an ensemble node.
fn node_value(node: &Value, metric: &str) -> Option<f64> {
    let v = node.get("metrics")?.get(metric)?;
    f(v).or_else(|| v.get("mean").and_then(f))
}

fn sweep_figures(sw: &Value, out: &mut Vec<NamedFigure>) {
    let Some(metrics) = sw.get("metrics").and_then(Value::as_object) else {
        return;
    };
    let nodes = sw
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (m, spec) in metrics {
        let vals: Vec<f64> = nodes.iter().filter_map(|n| node_value(n, m)).collect();
        if vals.is_empty() {
            continue;
        }
        let unit = s(spec, "unit");
        let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        for (st, v) in [("min", min), ("max", max)] {
            out.push(NamedFigure {
                name: format!("sweep.{}.{st}", sanitize(m)),
                value: v,
                unit: unit.clone(),
                note: format!("{st} of sweep metric '{m}' over {} nodes", vals.len()),
            });
        }
    }
}

/// The first value of the sweep's first axis at which `metric` crosses the watch's
/// threshold, in node order, with the axis unit. Nodes whose other axis coordinates are
/// not at their first value are skipped, so on a grid of several axes the crossing is
/// read along the first axis at the first value of every other axis.
pub fn sweep_crossing(
    doc: &Value,
    metric: &str,
    crossing: &Crossing,
) -> Result<(f64, String), String> {
    let sw = doc
        .get("sweep")
        .ok_or("the result has no sweep block, so a crossing cannot be read")?;
    let order: Vec<String> = sw
        .get("axis_order")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let first = order.first().ok_or("the sweep states no axes")?;
    let axes = sw
        .get("axes")
        .and_then(Value::as_object)
        .ok_or("the sweep states no axes")?;
    let unit = axes.get(first).map(|a| s(a, "unit")).unwrap_or_default();
    let first_values: Vec<(String, f64)> = order[1..]
        .iter()
        .filter_map(|ax| {
            axes.get(ax)
                .and_then(|a| a.get("values"))
                .and_then(Value::as_array)
                .and_then(|v| v.first())
                .and_then(f)
                .map(|v| (ax.clone(), v))
        })
        .collect();
    if sw.get("metrics").and_then(|m| m.get(metric)).is_none() {
        return Err(format!("the sweep has no metric '{metric}'"));
    }
    let nodes = sw
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("the sweep has no nodes")?;
    for node in nodes {
        let coords = node.get("coords");
        let on_line = first_values.iter().all(|(ax, v0)| {
            coords
                .and_then(|c| c.get(ax))
                .and_then(f)
                .map(|v| (v - v0).abs() <= T_EPS * v0.abs().max(1.0))
                .unwrap_or(false)
        });
        if !on_line {
            continue;
        }
        let Some(v) = node_value(node, metric) else {
            continue;
        };
        let crossed = match (crossing.below, crossing.above) {
            (Some(b), _) => v < b,
            (_, Some(a)) => v > a,
            _ => false,
        };
        if crossed {
            let x = coords
                .and_then(|c| c.get(first))
                .and_then(f)
                .ok_or("a sweep node has no coordinate on the first axis")?;
            return Ok((x, unit));
        }
    }
    Err(format!(
        "sweep metric '{metric}' never crosses the threshold along axis '{first}'"
    ))
}

/// A copy of `doc` with the campaign figures injected under a top-level `campaign`
/// object, each with a `units` entry (provenance `computed`: the overlay read it from the
/// campaign's own computed timeline). A non-campaign document is returned unchanged.
pub fn augment(doc: &Value) -> Result<Value, String> {
    let figs = figures(doc);
    let mut out = doc.clone();
    if figs.is_empty() {
        return Ok(out);
    }
    if out.get("campaign").is_some() {
        return Err("the result already has a top-level 'campaign' field".into());
    }
    let mut root = Map::new();
    let mut units = Map::new();
    for fig in &figs {
        insert_dotted(&mut root, &fig.name, json!(fig.value));
        units.insert(
            format!("campaign.{}", fig.name),
            json!({"unit": fig.unit, "provenance": "computed", "note": fig.note}),
        );
    }
    let obj = out.as_object_mut().ok_or("the result is not an object")?;
    obj.insert("campaign".into(), Value::Object(root));
    let u = obj
        .entry("units")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(u) = u.as_object_mut() {
        u.extend(units);
    }
    Ok(out)
}

fn insert_dotted(root: &mut Map<String, Value>, dotted: &str, v: Value) {
    let segs: Vec<&str> = dotted.split('.').collect();
    let mut cur = root;
    for seg in &segs[..segs.len() - 1] {
        let next = cur
            .entry(seg.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !next.is_object() {
            *next = Value::Object(Map::new());
        }
        cur = next.as_object_mut().expect("object just ensured");
    }
    cur.insert(segs[segs.len() - 1].to_string(), v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> Value {
        json!({
            "kind": "campaign",
            "timeline": {
                "step_s": 10.0,
                "t_s": [0.0, 10.0, 20.0, 30.0, 40.0],
                "phases": [
                    {"name": "a-one", "t0_s": 0.0, "t1_s": 20.0, "ended_by": "duration_s", "carried": {}},
                    {"name": "b", "t0_s": 20.0, "t1_s": 40.0, "ended_by": "duration_s", "carried": {"x": 2.0}}
                ],
                "events": [{"t_s": 20.0, "phase": "b", "label": "alarm", "alarm": true}],
                "handoffs": [],
                "channels": {
                    "x": {"unit": "ns", "values": [0.0, 1.0, 2.0, 3.0, 4.0]},
                    "alarm": {"unit": "1", "values": [0.0, 0.0, 1.0, 1.0, null]}
                }
            },
            "units": {}
        })
    }

    #[test]
    fn end_of_phase_is_the_last_owned_grid_time() {
        let figs = figures(&chain());
        let get = |n: &str| figs.iter().find(|f| f.name == n).map(|f| f.value);
        // Phase a-one owns t = 0 and 10 (t = 20 belongs to b); b owns 20..=40.
        assert_eq!(get("phase.a_one.end.x"), Some(1.0));
        assert_eq!(get("phase.b.end.x"), Some(4.0));
        assert_eq!(get("phase.b.carried.x"), Some(2.0));
        assert_eq!(get("phase.a_one.duration_s"), Some(20.0));
        assert_eq!(get("first_alarm_s"), Some(20.0));
        assert_eq!(get("alarm_share"), Some(0.5));
        assert_eq!(get("event.e1.t_s"), Some(20.0));
        // The alarm is null at the last grid time: no end value for it in phase b.
        assert_eq!(get("phase.b.end.alarm"), None);
    }

    #[test]
    fn augment_injects_figures_with_units() {
        let a = augment(&chain()).unwrap();
        assert_eq!(a["campaign"]["phase"]["b"]["end"]["x"], json!(4.0));
        assert_eq!(a["units"]["campaign.phase.b.end.x"]["unit"], json!("ns"));
        let plain = json!({"kind": "clock", "fom": {"a": 1.0}});
        assert_eq!(augment(&plain).unwrap(), plain);
    }

    #[test]
    fn sweep_crossing_reads_the_first_axis() {
        let doc = json!({
            "kind": "campaign",
            "sweep": {
                "axis_order": ["p"],
                "axes": {"p": {"unit": "dBW", "values": [-40.0, -30.0, -20.0]}},
                "metrics": {"avail": {"unit": "1"}},
                "nodes": [
                    {"coords": {"p": -40.0}, "metrics": {"avail": 1.0}},
                    {"coords": {"p": -30.0}, "metrics": {"avail": 0.8}},
                    {"coords": {"p": -20.0}, "metrics": {"avail": 0.1}}
                ]
            }
        });
        let below = Crossing {
            below: Some(0.5),
            above: None,
        };
        assert_eq!(
            sweep_crossing(&doc, "avail", &below).unwrap(),
            (-20.0, "dBW".into())
        );
        let never = Crossing {
            below: Some(0.0),
            above: None,
        };
        assert!(sweep_crossing(&doc, "avail", &never).is_err());
        assert!(sweep_crossing(&doc, "missing", &below).is_err());
    }
}
