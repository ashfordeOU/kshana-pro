// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Alert policy engine (roadmap 0.42 F5)
//!
//! Decide **who is told what, when, and through which output**: routes match alerts by kind,
//! severity, area and source; each route names recipients and the open engine's I5 outputs
//! (webhooks with their ServiceNow, PagerDuty, Opsgenie, Slack and Teams templates, syslog, CEF,
//! Kafka, SNMP traps, Splunk HEC, Elastic, STIX/TAXII, CAP, MQTT, NATS, Prometheus,
//! OpenTelemetry); **quiet hours** hold lower-severity alerts into a digest sent when they end;
//! **escalation** notifies further recipients when nobody acknowledged in time;
//! **de-duplication** folds repeats of one alert key inside a window into the first
//! notification, and never hides a worse one.
//!
//! The engine plans; it sends nothing. Delivery goes through the open I5 outputs, which stay
//! free, and the **free edition keeps raw alerts**: every alert is in the raw log unchanged
//! ([`raw_log`], `kshana-pro alerts raw`, no licence), and a plan accounts for every input
//! alert (notified, folded, held into a digest, or unrouted), so a policy can never lose one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The licence feature of `kshana-pro alerts route`.
pub const FEATURE: &str = "pro-alert-policy";
/// The policy file's format tag.
pub const POLICY_FORMAT: &str = "kshana-pro-alert-policy/1";
/// The plan's format tag.
pub const PLAN_FORMAT: &str = "kshana-pro-alert-plan/1";

/// The I5 output names a route may use (the open engine's connectors and their templates).
pub const I5_OUTPUTS: &[&str] = &[
    "webhook",
    "servicenow",
    "pagerduty",
    "opsgenie",
    "slack",
    "teams",
    "syslog",
    "cef",
    "kafka",
    "snmp-trap",
    "splunk-hec",
    "elastic",
    "stix-taxii",
    "cap",
    "mqtt",
    "nats",
    "prometheus",
    "opentelemetry",
];

/// An alert's severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    pub fn parse(s: &str) -> Option<Severity> {
        match s.trim() {
            "info" => Some(Severity::Info),
            "warning" => Some(Severity::Warning),
            "critical" => Some(Severity::Critical),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Critical => "critical",
        }
    }
}

/// One raw alert.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub t_unix: i64,
    pub id: String,
    pub source: String,
    pub kind: String,
    pub severity: Severity,
    pub area: String,
    pub text: String,
}

/// The raw alert table header.
pub const ALERTS_HEADER: &str = "t_unix,id,source,kind,severity,area,text";

/// Parse a raw alert table (`t_unix,id,source,kind,severity,area,text`; the text may hold commas).
pub fn parse_alerts(text: &str) -> Result<Vec<Alert>, String> {
    let mut out = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with("t_unix") {
            continue;
        }
        let f: Vec<&str> = line.splitn(7, ',').collect();
        if f.len() != 7 {
            return Err(format!("alerts line {}: 7 fields", ln + 1));
        }
        let t: i64 = f[0]
            .trim()
            .parse()
            .map_err(|_| format!("alerts line {}: t_unix is not an integer", ln + 1))?;
        let severity = Severity::parse(f[4]).ok_or(format!(
            "alerts line {}: severity (info, warning, critical)",
            ln + 1
        ))?;
        if f[1].trim().is_empty() || f[3].trim().is_empty() {
            return Err(format!("alerts line {}: id and kind are required", ln + 1));
        }
        out.push(Alert {
            t_unix: t,
            id: f[1].trim().into(),
            source: f[2].trim().into(),
            kind: f[3].trim().into(),
            severity,
            area: f[5].trim().into(),
            text: f[6].trim().into(),
        });
    }
    let mut ids: Vec<&str> = out.iter().map(|a| a.id.as_str()).collect();
    ids.sort();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err("alert ids must be unique".into());
    }
    Ok(out)
}

/// The raw log: every alert, unchanged, in time order (the free edition's alerts).
pub fn raw_log(alerts: &[Alert]) -> String {
    let mut v: Vec<&Alert> = alerts.iter().collect();
    v.sort_by(|a, b| (a.t_unix, &a.id).cmp(&(b.t_unix, &b.id)));
    let mut s = format!("{ALERTS_HEADER}\n");
    for a in v {
        s.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            a.t_unix,
            a.id,
            a.source,
            a.kind,
            a.severity.as_str(),
            a.area,
            a.text
        ));
    }
    s
}

/// An acknowledgement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ack {
    pub t_unix: i64,
    pub alert_id: String,
    pub by: String,
}

/// Parse acknowledgements (`t_unix,alert_id,by`).
pub fn parse_acks(text: &str) -> Result<Vec<Ack>, String> {
    let mut out = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with("t_unix") {
            continue;
        }
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() != 3 {
            return Err(format!("acks line {}: 3 fields", ln + 1));
        }
        out.push(Ack {
            t_unix: f[0]
                .parse()
                .map_err(|_| format!("acks line {}: t_unix is not an integer", ln + 1))?,
            alert_id: f[1].into(),
            by: f[2].into(),
        });
    }
    Ok(out)
}

/// A recipient and its default outputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub id: String,
    pub outputs: Vec<String>,
}

/// Quiet hours (local time `HH:MM`, wrapping midnight when `end < start`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuietHours {
    pub start: String,
    pub end: String,
    /// Alerts of this severity or worse are sent during quiet hours (default: none are).
    #[serde(default)]
    pub allow: Option<Severity>,
}

/// One escalation step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escalation {
    /// Seconds after the first notification without an acknowledgement.
    pub after_s: i64,
    pub notify: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
}

/// One route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub name: String,
    /// Alert kinds this route takes (empty: any).
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default = "default_sev")]
    pub min_severity: Severity,
    /// Areas (empty: any).
    #[serde(default)]
    pub areas: Vec<String>,
    /// Sources (empty: any).
    #[serde(default)]
    pub sources: Vec<String>,
    pub notify: Vec<String>,
    /// Outputs for this route (empty: each recipient's defaults).
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub quiet_hours: Option<QuietHours>,
    #[serde(default)]
    pub escalate: Vec<Escalation>,
    /// Fields that make the de-duplication key: any of `kind`, `area`, `source` (default all three).
    #[serde(default)]
    pub dedup_key: Vec<String>,
    /// Stop at this route when it matches (default true).
    #[serde(default = "yes")]
    pub stop: bool,
}

fn default_sev() -> Severity {
    Severity::Info
}

fn yes() -> bool {
    true
}

/// A policy file (`kshana-pro-alert-policy/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub format: String,
    /// Offset of local time from UTC for quiet hours, minutes.
    #[serde(default)]
    pub timezone_offset_min: i64,
    /// De-duplication window, s.
    #[serde(default = "default_window")]
    pub dedup_window_s: i64,
    #[serde(rename = "recipient")]
    pub recipients: Vec<Recipient>,
    #[serde(rename = "route")]
    pub routes: Vec<Route>,
}

fn default_window() -> i64 {
    300
}

fn hhmm(s: &str) -> Option<i64> {
    let (h, m) = s.split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

impl Policy {
    pub fn parse(text: &str) -> Result<Policy, String> {
        let p: Policy = toml::from_str(text).map_err(|e| format!("policy: {e}"))?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != POLICY_FORMAT {
            return Err(format!("format must be {POLICY_FORMAT:?}"));
        }
        if !(-14 * 60..=14 * 60).contains(&self.timezone_offset_min) {
            return Err("timezone_offset_min is within +-840".into());
        }
        if !(0..=7 * 86_400).contains(&self.dedup_window_s) {
            return Err("dedup_window_s is 0 to 604800".into());
        }
        let out_ok = |o: &String| I5_OUTPUTS.contains(&o.as_str());
        for r in &self.recipients {
            if r.id.trim().is_empty() {
                return Err("a recipient has no id".into());
            }
            if let Some(o) = r.outputs.iter().find(|o| !out_ok(o)) {
                return Err(format!(
                    "recipient {}: output {o:?} is not an I5 output ({})",
                    r.id,
                    I5_OUTPUTS.join(", ")
                ));
            }
            if r.outputs.is_empty() {
                return Err(format!("recipient {}: at least one output", r.id));
            }
        }
        let known = |id: &String| self.recipients.iter().any(|r| &r.id == id);
        if self.routes.is_empty() {
            return Err("at least one route".into());
        }
        for r in &self.routes {
            if r.notify.is_empty() {
                return Err(format!(
                    "route {}: notify names at least one recipient",
                    r.name
                ));
            }
            if let Some(x) = r.notify.iter().find(|x| !known(x)) {
                return Err(format!("route {}: unknown recipient {x:?}", r.name));
            }
            if let Some(o) = r.outputs.iter().find(|o| !out_ok(o)) {
                return Err(format!(
                    "route {}: output {o:?} is not an I5 output",
                    r.name
                ));
            }
            if let Some(q) = &r.quiet_hours {
                if hhmm(&q.start).is_none() || hhmm(&q.end).is_none() {
                    return Err(format!("route {}: quiet hours are HH:MM", r.name));
                }
            }
            let mut last = 0;
            for e in &r.escalate {
                if e.after_s <= last {
                    return Err(format!(
                        "route {}: escalation steps have increasing after_s > 0",
                        r.name
                    ));
                }
                last = e.after_s;
                if e.notify.is_empty() {
                    return Err(format!(
                        "route {}: an escalation step notifies nobody",
                        r.name
                    ));
                }
                if let Some(x) = e.notify.iter().find(|x| !known(x)) {
                    return Err(format!("route {}: unknown recipient {x:?}", r.name));
                }
                if let Some(o) = e.outputs.iter().find(|o| !out_ok(o)) {
                    return Err(format!(
                        "route {}: output {o:?} is not an I5 output",
                        r.name
                    ));
                }
            }
            if let Some(k) = r
                .dedup_key
                .iter()
                .find(|k| !matches!(k.as_str(), "kind" | "area" | "source"))
            {
                return Err(format!(
                    "route {}: dedup_key field {k:?} (kind, area, source)",
                    r.name
                ));
            }
        }
        Ok(())
    }

    fn quiet(&self, q: &QuietHours, t: i64) -> bool {
        let local = (t + self.timezone_offset_min * 60).rem_euclid(86_400) / 60;
        let (s, e) = (hhmm(&q.start).unwrap_or(0), hhmm(&q.end).unwrap_or(0));
        if s == e {
            false
        } else if s < e {
            local >= s && local < e
        } else {
            local >= s || local < e
        }
    }

    /// The next end of quiet hours after `t` (unix seconds).
    fn quiet_end(&self, q: &QuietHours, t: i64) -> i64 {
        let e = hhmm(&q.end).unwrap_or(0) * 60;
        let off = self.timezone_offset_min * 60;
        let day = (t + off).div_euclid(86_400) * 86_400;
        let mut end = day + e - off;
        while end <= t {
            end += 86_400;
        }
        end
    }
}

/// One planned notification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub t_unix: i64,
    pub route: String,
    /// `new`, `worse`, `digest` (quiet hours ended) or `escalation-<n>`.
    pub reason: String,
    pub severity: Severity,
    /// `(recipient, output)` pairs.
    pub deliveries: Vec<(String, String)>,
    /// The alerts this notification carries (the first one names the key).
    pub alerts: Vec<String>,
    /// Repeats folded into this notification's key since the previous one.
    pub folded: usize,
}

/// The plan of a policy over a set of alerts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub format: String,
    pub raw_alerts: usize,
    pub notifications: Vec<Notification>,
    /// Alerts folded into an earlier notification of the same key (de-duplication).
    pub folded: Vec<String>,
    /// Alerts held by quiet hours (each is also in a digest, unless the plan ends first).
    pub held: Vec<String>,
    /// Alerts no route matched (still in the raw log).
    pub unrouted: Vec<String>,
    /// Alerts acknowledged, with who acknowledged.
    pub acknowledged: Vec<(String, String)>,
}

impl Plan {
    /// Every input alert is accounted for: notified, folded, held or unrouted.
    pub fn accounts_for(&self, alerts: &[Alert]) -> bool {
        alerts.iter().all(|a| {
            self.notifications.iter().any(|n| n.alerts.contains(&a.id))
                || self.folded.contains(&a.id)
                || self.held.contains(&a.id)
                || self.unrouted.contains(&a.id)
        })
    }

    pub fn notifications_csv(&self) -> String {
        let mut s = String::from("t_unix,route,reason,severity,recipient,output,alerts,folded\n");
        for n in &self.notifications {
            for (r, o) in &n.deliveries {
                s.push_str(&format!(
                    "{},{},{},{},{},{},{},{}\n",
                    n.t_unix,
                    n.route,
                    n.reason,
                    n.severity.as_str(),
                    r,
                    o,
                    n.alerts.join(" "),
                    n.folded
                ));
            }
        }
        s
    }
}

fn matches(r: &Route, a: &Alert) -> bool {
    a.severity >= r.min_severity
        && (r.kinds.is_empty() || r.kinds.contains(&a.kind))
        && (r.areas.is_empty() || r.areas.contains(&a.area))
        && (r.sources.is_empty() || r.sources.contains(&a.source))
}

fn key(r: &Route, a: &Alert) -> String {
    let fields: Vec<&str> = if r.dedup_key.is_empty() {
        vec!["kind", "area", "source"]
    } else {
        r.dedup_key.iter().map(String::as_str).collect()
    };
    fields
        .iter()
        .map(|f| match *f {
            "kind" => a.kind.as_str(),
            "area" => a.area.as_str(),
            _ => a.source.as_str(),
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn deliveries(p: &Policy, notify: &[String], outputs: &[String]) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for r in notify {
        let outs: Vec<String> = if outputs.is_empty() {
            p.recipients
                .iter()
                .find(|x| &x.id == r)
                .map(|x| x.outputs.clone())
                .unwrap_or_default()
        } else {
            outputs.to_vec()
        };
        for o in outs {
            if !v.contains(&(r.clone(), o.clone())) {
                v.push((r.clone(), o));
            }
        }
    }
    v
}

struct Group {
    route: usize,
    first_notified: i64,
    open_until: i64,
    severity: Severity,
    alerts: Vec<String>,
    escalated: usize,
    folded_since: usize,
}

/// Plan the notifications of `alerts` under `policy` up to `until` (unix seconds; escalations
/// and digests later than it are not planned).
pub fn evaluate(policy: &Policy, alerts: &[Alert], acks: &[Ack], until: i64) -> Plan {
    let mut sorted: Vec<&Alert> = alerts.iter().collect();
    sorted.sort_by(|a, b| (a.t_unix, &a.id).cmp(&(b.t_unix, &b.id)));
    let mut plan = Plan {
        format: PLAN_FORMAT.into(),
        raw_alerts: alerts.len(),
        notifications: Vec::new(),
        folded: Vec::new(),
        held: Vec::new(),
        unrouted: Vec::new(),
        acknowledged: Vec::new(),
    };
    let ack_time = |ids: &[String]| -> Option<i64> {
        acks.iter()
            .filter(|k| ids.contains(&k.alert_id))
            .map(|k| k.t_unix)
            .min()
    };
    let mut groups: BTreeMap<(usize, String), Group> = BTreeMap::new();
    let mut closed: Vec<Group> = Vec::new();
    // Held per route: (alert id, severity), and the time the digest goes out.
    let mut digests: BTreeMap<usize, (i64, Vec<String>, Severity)> = BTreeMap::new();
    for a in sorted {
        let mut routed = false;
        for (ri, r) in policy.routes.iter().enumerate() {
            if !matches(r, a) {
                continue;
            }
            routed = true;
            let quiet = r
                .quiet_hours
                .as_ref()
                .filter(|q| policy.quiet(q, a.t_unix) && q.allow.is_none_or(|al| a.severity < al));
            if let Some(q) = quiet {
                let end = policy.quiet_end(q, a.t_unix);
                let d = digests.entry(ri).or_insert((end, Vec::new(), a.severity));
                d.1.push(a.id.clone());
                d.2 = d.2.max(a.severity);
                if !plan.held.contains(&a.id) {
                    plan.held.push(a.id.clone());
                }
            } else {
                let k = (ri, key(r, a));
                let open_sev = groups
                    .get(&k)
                    .filter(|g| a.t_unix < g.open_until)
                    .map(|g| g.severity);
                match open_sev {
                    Some(sev) if a.severity <= sev => {
                        let g = groups.get_mut(&k).expect("present");
                        g.alerts.push(a.id.clone());
                        g.folded_since += 1;
                        if !plan.folded.contains(&a.id) {
                            plan.folded.push(a.id.clone());
                        }
                    }
                    _ => {
                        let worse = open_sev.is_some();
                        let folded = groups.get(&k).map(|g| g.folded_since).unwrap_or(0);
                        plan.notifications.push(Notification {
                            t_unix: a.t_unix,
                            route: r.name.clone(),
                            reason: if worse { "worse" } else { "new" }.into(),
                            severity: a.severity,
                            deliveries: deliveries(policy, &r.notify, &r.outputs),
                            alerts: vec![a.id.clone()],
                            folded: if worse { 0 } else { folded },
                        });
                        let replaced = groups.insert(
                            k,
                            Group {
                                route: ri,
                                first_notified: a.t_unix,
                                open_until: a.t_unix + policy.dedup_window_s,
                                severity: a.severity,
                                alerts: vec![a.id.clone()],
                                escalated: 0,
                                folded_since: 0,
                            },
                        );
                        // A group whose window ended keeps its own escalations.
                        closed.extend(replaced);
                    }
                }
            }
            if r.stop {
                break;
            }
        }
        if !routed {
            plan.unrouted.push(a.id.clone());
        }
    }
    // Digests at the end of quiet hours.
    for (ri, (t, ids, sev)) in digests {
        if t > until {
            continue;
        }
        let r = &policy.routes[ri];
        plan.notifications.push(Notification {
            t_unix: t,
            route: r.name.clone(),
            reason: "digest".into(),
            severity: sev,
            deliveries: deliveries(policy, &r.notify, &r.outputs),
            alerts: ids.clone(),
            folded: 0,
        });
        groups.insert(
            (ri, format!("digest@{t}")),
            Group {
                route: ri,
                first_notified: t,
                open_until: t,
                severity: sev,
                alerts: ids,
                escalated: 0,
                folded_since: 0,
            },
        );
    }
    // Escalations: a group not acknowledged by first_notified + after_s.
    for g in closed.iter_mut().chain(groups.values_mut()) {
        let r = &policy.routes[g.route];
        let acked = ack_time(&g.alerts);
        for (n, e) in r.escalate.iter().enumerate() {
            let due = g.first_notified + e.after_s;
            if due > until || acked.is_some_and(|t| t <= due) {
                break;
            }
            g.escalated = n + 1;
            plan.notifications.push(Notification {
                t_unix: due,
                route: r.name.clone(),
                reason: format!("escalation-{}", n + 1),
                severity: g.severity,
                deliveries: deliveries(policy, &e.notify, &e.outputs),
                alerts: g.alerts.clone(),
                folded: 0,
            });
        }
    }
    for k in acks {
        if alerts.iter().any(|a| a.id == k.alert_id) {
            plan.acknowledged.push((k.alert_id.clone(), k.by.clone()));
        }
    }
    plan.notifications.sort_by(|a, b| {
        (a.t_unix, &a.route, &a.reason, &a.alerts).cmp(&(b.t_unix, &b.route, &b.reason, &b.alerts))
    });
    plan
}

// ------------------------------------------------------------------ alert sources

/// Alerts from an area picture: one per event, at its start (kind `area-degraded` or
/// `area-untrusted`; `critical` when untrusted, `warning` otherwise; a feed-only event is
/// `info` and says it is uncorroborated). No emitter position is carried (F2).
pub fn from_picture(p: &crate::area::Picture) -> Vec<Alert> {
    use crate::area::Fused;
    p.events()
        .into_iter()
        .map(|e| {
            let (kind, severity) = match (e.worst, e.basis.as_str()) {
                (_, "feed") => ("area-feed-detection", Severity::Info),
                (Fused::Untrusted, _) => ("area-untrusted", Severity::Critical),
                _ => ("area-degraded", Severity::Warning),
            };
            Alert {
                t_unix: e.start_g,
                id: e.id.clone(),
                source: "area-console".into(),
                kind: kind.into(),
                severity,
                area: e.area.clone(),
                text: format!(
                    "{} {} for {} s ({}{}{})",
                    e.area,
                    e.worst.as_str(),
                    e.duration_s,
                    e.basis,
                    if e.corroborated {
                        ", corroborated"
                    } else if e.basis == "feed" {
                        ", uncorroborated"
                    } else {
                        ""
                    },
                    e.wide_area
                        .as_deref()
                        .map(|w| format!(", {w}"))
                        .unwrap_or_default()
                ),
            }
        })
        .collect()
}

/// Alerts from a multi-source trust run: every switch away from a source (warning) and every
/// entry into holdover or no output (critical). `t0_unix` is added to the run's times.
pub fn from_trust(ev: &crate::multi_source::Evidence, site: &str, t0_unix: i64) -> Vec<Alert> {
    ev.switches
        .iter()
        // The first record is the run's start (from no output to the first choice).
        .filter(|s| !(s.index == 0 && s.from == "none"))
        .map(|s| {
            let (kind, severity) = match s.to.as_str() {
                "holdover" => ("timing-holdover", Severity::Critical),
                "none" => ("timing-no-output", Severity::Critical),
                _ if s.reason == "better source held" || s.reason == "a source qualifies" => {
                    ("timing-source-restored", Severity::Info)
                }
                _ => ("timing-source-switch", Severity::Warning),
            };
            Alert {
                t_unix: t0_unix + s.t_s.floor() as i64,
                id: format!("{site}-switch-{}", s.index),
                source: "multi-source-trust".into(),
                kind: kind.into(),
                severity,
                area: site.into(),
                text: format!("{} -> {}: {}", s.from, s.to, s.reason),
            }
        })
        .collect()
}
