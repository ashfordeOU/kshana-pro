// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Always-on watch walls (roadmap 0.42 U28, Enterprise)
//!
//! A wall is the one screen an operations room leaves on: the tenant's Station fleet (live,
//! stale, retired), its area trust picture and its timing fleet when they are configured,
//! and its jobs by state. `GET /wall/state` is the JSON (the Desktop Pro wall view and any
//! dashboard read it); `GET /wall` is a self-contained page with no script that reloads
//! itself every 30 seconds, for a browser in kiosk mode. Each part reads what the service
//! already serves; nothing is computed for the wall alone. Enterprise feature
//! `pro-watch-wall`.

use std::path::Path;

use serde_json::{json, Value};

use super::auth::{Principal, ALL_TENANTS};
use super::worker::JobService;

/// The Enterprise feature.
pub const FEATURE: &str = "pro-watch-wall";
/// The format tag of `GET /wall/state`.
pub const FORMAT: &str = "kshana-pro-watch-wall/1";

fn tenant_dir_name(t: &str) -> &str {
    if t == ALL_TENANTS {
        "_all"
    } else {
        t
    }
}

/// The wall of `who`'s tenant at `now`.
pub fn state(
    j: &JobService,
    ent: &crate::license::Entitlement,
    stations_claim: u32,
    who: &Principal,
    now: i64,
) -> Value {
    let all = super::fleet::all(&j.data_dir);
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    let stations: Vec<Value> = all
        .iter()
        .filter(|s| who.sees(&s.tenant))
        .map(|s| {
            let st = super::fleet::state(s, now);
            *counts.entry(st).or_default() += 1;
            json!({"id": s.id, "name": s.name, "tenant": s.tenant, "state": st,
                   "last_seen_unix": s.last_seen_unix, "health": s.health})
        })
        .collect();
    let t = tenant_dir_name(&who.tenant);
    let area = area_part(&j.data_dir, t);
    let timing = timing_part(&j.data_dir, t);
    let mut jobs = std::collections::BTreeMap::<&str, usize>::new();
    for s in j.store.list().iter().filter(|s| who.sees(&s.tenant)) {
        *jobs.entry(s.state.as_str()).or_default() += 1;
    }
    let alarm = counts.get("stale").copied().unwrap_or(0) > 0
        || area["worst"].as_str().is_some_and(|w| w == "UNTRUSTED")
        || timing["counts"]["act"].as_u64().unwrap_or(0) > 0
        || timing["counts"]["breached"].as_u64().unwrap_or(0) > 0;
    json!({
        "format": FORMAT,
        "tenant": who.tenant,
        "at_unix": now,
        "alarm": alarm,
        "fleet": {"limit": super::fleet::limit_for(ent, stations_claim, now), "counts": counts, "stations": stations},
        "area": area,
        "timing": timing,
        "jobs": jobs,
    })
}

fn area_part(data_dir: &Path, t: &str) -> Value {
    let file = data_dir.join("area").join(t).join("console.toml");
    if !file.is_file() {
        return json!({"configured": false});
    }
    match crate::area::Inputs::load(&file) {
        Ok(i) => {
            let p = crate::area::picture(&i);
            let rank = |s: &str| match s {
                "UNTRUSTED" => 3,
                "DEGRADED" => 2,
                "TRUSTED" => 1,
                _ => 0,
            };
            let areas: Vec<Value> = p
                .areas
                .iter()
                .map(|a| {
                    let cur = serde_json::to_value(a.current).unwrap_or_default();
                    json!({"id": a.id, "name": a.name, "current": cur, "events": a.events.len()})
                })
                .collect();
            let worst = areas
                .iter()
                .filter_map(|a| a["current"].as_str())
                .max_by_key(|s| rank(s))
                .unwrap_or("UNKNOWN")
                .to_string();
            json!({"configured": true, "title": p.title, "worst": worst, "areas": areas,
                   "wide_area": p.wide_area.len(), "emitter_positions": p.positions.emitter_positions})
        }
        Err(e) => json!({"configured": true, "error": e}),
    }
}

fn timing_part(data_dir: &Path, t: &str) -> Value {
    let dir = data_dir.join("timing").join(t);
    let Ok(text) = std::fs::read_to_string(dir.join("fleet.toml")) else {
        return json!({"configured": false});
    };
    let mut spec = match crate::timing::fleet::FleetSpec::parse(&text) {
        Ok(s) => s,
        Err(e) => return json!({"configured": true, "error": e.to_string()}),
    };
    spec.resolve_paths(&dir);
    match crate::timing::fleet::assess(&spec) {
        Ok(a) => {
            let clocks: Vec<Value> = a
                .clocks
                .iter()
                .map(|c| json!({"id": c.id, "site": c.site, "status": c.status, "worst_warn_s": c.worst_warn_s()}))
                .collect();
            json!({"configured": true, "title": a.title, "counts": a.counts, "clocks": clocks})
        }
        Err(e) => json!({"configured": true, "error": e.to_string()}),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn tone(s: &str) -> &'static str {
    match s {
        "live" | "TRUSTED" | "ok" | "succeeded" => "#2f9e6e",
        "stale" | "DEGRADED" | "watch" | "registered" | "queued" | "running" => "#d39b2a",
        "UNTRUSTED" | "act" | "breached" | "failed" => "#d0453a",
        _ => "#6b7280",
    }
}

/// The wall page: self-contained, no script; it reloads itself every 30 s.
pub fn page(s: &Value) -> String {
    let mut h = String::from("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"30\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Kshana watch wall</title><style>:root{--bg:#0b0e12;--fg:#e5e7eb;--mut:#9aa3af;--card:#141920;--line:#262d36}body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.4 system-ui,sans-serif;padding:16px}h1{font-size:22px;margin:0 0 4px}h2{font-size:15px;color:var(--mut);margin:0 0 8px;text-transform:uppercase;letter-spacing:.06em}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:12px}.card{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:12px}.dot{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px}.row{display:flex;justify-content:space-between;padding:3px 0;border-bottom:1px solid var(--line)}.alarm{border-color:#d0453a}.mut{color:var(--mut)}</style></head><body>");
    h.push_str(&format!(
        "<h1>Watch wall · {}</h1><div class=\"mut\">updated at unix {} · reloads every 30 s · {}</div><div class=\"grid\" style=\"margin-top:12px\">",
        esc(s["tenant"].as_str().unwrap_or("")),
        s["at_unix"],
        if s["alarm"] == true { "ATTENTION" } else { "no alarm" }
    ));
    let row = |label: &str, state: &str| {
        format!(
            "<div class=\"row\"><span><span class=\"dot\" style=\"background:{}\"></span>{}</span><span class=\"mut\">{}</span></div>",
            tone(state),
            esc(label),
            esc(state)
        )
    };
    h.push_str("<div class=\"card\"><h2>Station fleet</h2>");
    for st in s["fleet"]["stations"].as_array().into_iter().flatten() {
        h.push_str(&row(
            st["name"].as_str().unwrap_or(""),
            st["state"].as_str().unwrap_or(""),
        ));
    }
    if s["fleet"]["stations"]
        .as_array()
        .is_none_or(|a| a.is_empty())
    {
        h.push_str("<div class=\"mut\">No Station registered.</div>");
    }
    h.push_str("</div><div class=\"card\"><h2>Area trust</h2>");
    if s["area"]["configured"] == true {
        for a in s["area"]["areas"].as_array().into_iter().flatten() {
            h.push_str(&row(
                a["name"].as_str().unwrap_or(""),
                a["current"].as_str().unwrap_or(""),
            ));
        }
        h.push_str("<div class=\"mut\">Detections and areas only; no emitter positions.</div>");
    } else {
        h.push_str("<div class=\"mut\">No area console configured.</div>");
    }
    h.push_str("</div><div class=\"card\"><h2>Timing fleet</h2>");
    if s["timing"]["configured"] == true {
        for c in s["timing"]["clocks"].as_array().into_iter().flatten() {
            h.push_str(&row(
                c["id"].as_str().unwrap_or(""),
                c["status"].as_str().unwrap_or(""),
            ));
        }
    } else {
        h.push_str("<div class=\"mut\">No timing fleet configured.</div>");
    }
    h.push_str("</div><div class=\"card\"><h2>Jobs</h2>");
    for (k, v) in s["jobs"].as_object().into_iter().flatten() {
        h.push_str(&format!(
            "<div class=\"row\"><span><span class=\"dot\" style=\"background:{}\"></span>{}</span><span class=\"mut\">{}</span></div>",
            tone(k),
            esc(k),
            v
        ));
    }
    h.push_str("</div></div></body></html>\n");
    h
}
