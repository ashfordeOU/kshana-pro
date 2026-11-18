// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! GNSS degradation timeline: what happens to the GNSS signals during a hybrid
//! eLoran/GNSS test, and its translation into each simulator's timed commands.
//!
//! The scenario section (wired into [`crate::scenario::Scenario`] separately; parse it with
//! [`GnssDegradation::from_toml_table`] or [`GnssDegradation::from_scenario_toml`]):
//!
//! ```toml
//! [gnss_degradation]
//! description = "GPS L1 jamming then spoofing"
//!
//! [[gnss_degradation.events]]
//! type = "cn0_offset"          # temporary C/N0 offset of a group (until_s optional)
//! at_s = 10.0
//! until_s = 40.0
//! constellation = "gps"        # omitted = all constellations
//! signal = "L1CA"              # omitted = all signals
//! offset_db = -6.0
//!
//! [[gnss_degradation.events]]
//! type = "power_step"          # permanent, cumulative power step
//! at_s = 20.0
//! constellation = "galileo"
//! step_db = -3.0
//!
//! [[gnss_degradation.events]]
//! type = "sat_mask"            # satellites switched off (until_s optional)
//! at_s = 30.0
//! until_s = 90.0
//! constellation = "gps"
//! prns = [5, 12]
//!
//! [[gnss_degradation.events]]
//! type = "outage"              # whole group off for duration_s
//! at_s = 100.0
//! duration_s = 30.0
//! constellation = "beidou"
//!
//! [[gnss_degradation.events]]
//! type = "jamming"
//! at_s = 150.0
//! duration_s = 60.0
//! id = "j1"
//! kind = "wideband"            # cw | wideband | chirp | pulsed
//! center_hz = 1575420000.0
//! bandwidth_hz = 2000000.0
//! js_db = 40.0                 # jammer-to-signal ratio
//!
//! [[gnss_degradation.events]]
//! type = "spoofing"            # start/stop markers (the spoofer itself is configured
//! at_s = 250.0                 # in the simulator's scenario)
//! until_s = 400.0
//! label = "meaconing"
//! ```
//!
//! [`GnssDegradation::actions`] compiles the events into absolute, time-sorted
//! [`TimedAction`]s (group power offsets are re-sent as absolute totals at each change);
//! [`translate`] renders them for a [`CommandProfile`]. Overlapping groups (e.g. "all" and
//! "gps") are sent as separate commands; whether a simulator adds or replaces such offsets
//! depends on its command semantics and must be checked with the vendor's manual.

use super::profile::{
    ops, CommandProfile, CommandSchedule, Constellation, Fields, ScheduleMarker, ScheduledCommand,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Jammer waveform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JamKind {
    Cw,
    #[default]
    Wideband,
    Chirp,
    Pulsed,
}

impl JamKind {
    pub fn key(self) -> &'static str {
        match self {
            JamKind::Cw => "cw",
            JamKind::Wideband => "wideband",
            JamKind::Chirp => "chirp",
            JamKind::Pulsed => "pulsed",
        }
    }
}

fn d_jam_id() -> String {
    "jammer".into()
}

/// One degradation event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DegradationEvent {
    Cn0Offset {
        at_s: f64,
        #[serde(default)]
        until_s: Option<f64>,
        #[serde(default)]
        constellation: Option<Constellation>,
        #[serde(default)]
        signal: Option<String>,
        offset_db: f64,
    },
    PowerStep {
        at_s: f64,
        #[serde(default)]
        constellation: Option<Constellation>,
        #[serde(default)]
        signal: Option<String>,
        step_db: f64,
    },
    SatMask {
        at_s: f64,
        #[serde(default)]
        until_s: Option<f64>,
        constellation: Constellation,
        prns: Vec<u32>,
    },
    Outage {
        at_s: f64,
        duration_s: f64,
        #[serde(default)]
        constellation: Option<Constellation>,
        #[serde(default)]
        signal: Option<String>,
    },
    Jamming {
        at_s: f64,
        duration_s: f64,
        #[serde(default = "d_jam_id")]
        id: String,
        #[serde(default)]
        kind: JamKind,
        center_hz: f64,
        #[serde(default)]
        bandwidth_hz: f64,
        js_db: f64,
    },
    Spoofing {
        at_s: f64,
        #[serde(default)]
        until_s: Option<f64>,
        #[serde(default)]
        label: String,
    },
}

impl DegradationEvent {
    pub fn at_s(&self) -> f64 {
        match self {
            DegradationEvent::Cn0Offset { at_s, .. }
            | DegradationEvent::PowerStep { at_s, .. }
            | DegradationEvent::SatMask { at_s, .. }
            | DegradationEvent::Outage { at_s, .. }
            | DegradationEvent::Jamming { at_s, .. }
            | DegradationEvent::Spoofing { at_s, .. } => *at_s,
        }
    }
}

/// The `[gnss_degradation]` scenario section.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GnssDegradation {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub events: Vec<DegradationEvent>,
}

/// A constellation/signal group (`None` = all).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Group {
    pub constellation: Option<Constellation>,
    pub signal: Option<String>,
}

impl Group {
    pub fn label(&self) -> String {
        format!(
            "{}/{}",
            self.constellation.map(|c| c.key()).unwrap_or("all"),
            self.signal.as_deref().unwrap_or("all")
        )
    }
    /// Whether this group covers constellation `c` and signal `s`.
    pub fn covers(&self, c: Constellation, s: &str) -> bool {
        self.constellation.is_none_or(|g| g == c) && self.signal.as_deref().is_none_or(|g| g == s)
    }
}

/// A primitive, absolute action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// Absolute offsets of a group after this instant (power steps and C/N0 offsets kept
    /// apart so a simulator with a C/N0 command can use it).
    GroupOffset {
        group: Group,
        power_db: f64,
        cn0_db: f64,
        power_changed: bool,
        cn0_changed: bool,
    },
    SatDisable {
        constellation: Constellation,
        prn: u32,
    },
    SatEnable {
        constellation: Constellation,
        prn: u32,
    },
    SignalOff {
        group: Group,
    },
    SignalOn {
        group: Group,
    },
    JammerStart {
        id: String,
        kind: JamKind,
        center_hz: f64,
        bandwidth_hz: f64,
        js_db: f64,
    },
    JammerStop {
        id: String,
    },
    SpoofStart {
        label: String,
    },
    SpoofStop {
        label: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedAction {
    pub t_s: f64,
    pub action: Action,
}

/// The degradation state at one instant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DegradationState {
    /// Group label → (power dB, C/N0 dB).
    pub offsets_db: BTreeMap<String, (f64, f64)>,
    pub masked: Vec<(Constellation, u32)>,
    pub outages: Vec<String>,
    /// Active jammers: (id, J/S dB).
    pub jammers: Vec<(String, f64)>,
    pub spoofing: Vec<String>,
}

fn finite_nonneg(name: &str, v: f64) -> Result<(), String> {
    if !v.is_finite() || v < 0.0 {
        return Err(format!("gnss_degradation: {name} must be finite and >= 0"));
    }
    Ok(())
}

impl GnssDegradation {
    /// Parse the section's table (the value of `gnss_degradation`).
    pub fn from_toml_table(t: &toml::Table) -> Result<GnssDegradation, String> {
        let d: GnssDegradation = toml::Value::Table(t.clone())
            .try_into()
            .map_err(|e| format!("gnss_degradation: {e}"))?;
        d.validate()?;
        Ok(d)
    }
    /// Parse the section from scenario TOML text; `None` when the section is absent.
    pub fn from_scenario_toml(src: &str) -> Result<Option<GnssDegradation>, String> {
        let v: toml::Table = toml::from_str(src).map_err(|e| format!("scenario: {e}"))?;
        match v.get("gnss_degradation") {
            None => Ok(None),
            Some(toml::Value::Table(t)) => GnssDegradation::from_toml_table(t).map(Some),
            Some(_) => Err("gnss_degradation must be a table".into()),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        for (i, e) in self.events.iter().enumerate() {
            let ctx = |m: &str| format!("gnss_degradation event {i}: {m}");
            finite_nonneg("at_s", e.at_s()).map_err(|m| ctx(&m))?;
            let until_ok = |u: &Option<f64>| match u {
                Some(u) if !u.is_finite() || *u <= e.at_s() => Err(ctx("until_s must be > at_s")),
                _ => Ok(()),
            };
            match e {
                DegradationEvent::Cn0Offset {
                    until_s, offset_db, ..
                } => {
                    until_ok(until_s)?;
                    if !offset_db.is_finite() {
                        return Err(ctx("offset_db must be finite"));
                    }
                }
                DegradationEvent::PowerStep { step_db, .. } => {
                    if !step_db.is_finite() {
                        return Err(ctx("step_db must be finite"));
                    }
                }
                DegradationEvent::SatMask { until_s, prns, .. } => {
                    until_ok(until_s)?;
                    if prns.is_empty() {
                        return Err(ctx("prns must not be empty"));
                    }
                }
                DegradationEvent::Outage { duration_s, .. } => {
                    if !duration_s.is_finite() || *duration_s <= 0.0 {
                        return Err(ctx("duration_s must be > 0"));
                    }
                }
                DegradationEvent::Jamming {
                    duration_s,
                    center_hz,
                    bandwidth_hz,
                    js_db,
                    ..
                } => {
                    if !duration_s.is_finite() || *duration_s <= 0.0 {
                        return Err(ctx("duration_s must be > 0"));
                    }
                    if !center_hz.is_finite() || *center_hz <= 0.0 {
                        return Err(ctx("center_hz must be > 0"));
                    }
                    finite_nonneg("bandwidth_hz", *bandwidth_hz).map_err(|m| ctx(&m))?;
                    if !js_db.is_finite() {
                        return Err(ctx("js_db must be finite"));
                    }
                }
                DegradationEvent::Spoofing { until_s, .. } => until_ok(until_s)?,
            }
        }
        Ok(())
    }

    /// Compile into time-sorted absolute actions. At equal times, "off/stop" actions come
    /// before "on/start" ones, and group offsets last.
    pub fn actions(&self) -> Vec<TimedAction> {
        let mut out: Vec<(f64, u8, Action)> = Vec::new();
        // group offsets: (time, group) -> (Δpower, Δcn0)
        let mut deltas: BTreeMap<Group, Vec<(f64, f64, f64)>> = BTreeMap::new();
        for e in &self.events {
            match e {
                DegradationEvent::Cn0Offset {
                    at_s,
                    until_s,
                    constellation,
                    signal,
                    offset_db,
                } => {
                    let g = Group {
                        constellation: *constellation,
                        signal: signal.clone(),
                    };
                    let v = deltas.entry(g).or_default();
                    v.push((*at_s, 0.0, *offset_db));
                    if let Some(u) = until_s {
                        v.push((*u, 0.0, -*offset_db));
                    }
                }
                DegradationEvent::PowerStep {
                    at_s,
                    constellation,
                    signal,
                    step_db,
                } => {
                    let g = Group {
                        constellation: *constellation,
                        signal: signal.clone(),
                    };
                    deltas.entry(g).or_default().push((*at_s, *step_db, 0.0));
                }
                DegradationEvent::SatMask {
                    at_s,
                    until_s,
                    constellation,
                    prns,
                } => {
                    for &prn in prns {
                        out.push((
                            *at_s,
                            1,
                            Action::SatDisable {
                                constellation: *constellation,
                                prn,
                            },
                        ));
                        if let Some(u) = until_s {
                            out.push((
                                *u,
                                0,
                                Action::SatEnable {
                                    constellation: *constellation,
                                    prn,
                                },
                            ));
                        }
                    }
                }
                DegradationEvent::Outage {
                    at_s,
                    duration_s,
                    constellation,
                    signal,
                } => {
                    let g = Group {
                        constellation: *constellation,
                        signal: signal.clone(),
                    };
                    out.push((*at_s, 1, Action::SignalOff { group: g.clone() }));
                    out.push((at_s + duration_s, 0, Action::SignalOn { group: g }));
                }
                DegradationEvent::Jamming {
                    at_s,
                    duration_s,
                    id,
                    kind,
                    center_hz,
                    bandwidth_hz,
                    js_db,
                } => {
                    out.push((
                        *at_s,
                        1,
                        Action::JammerStart {
                            id: id.clone(),
                            kind: *kind,
                            center_hz: *center_hz,
                            bandwidth_hz: *bandwidth_hz,
                            js_db: *js_db,
                        },
                    ));
                    out.push((at_s + duration_s, 0, Action::JammerStop { id: id.clone() }));
                }
                DegradationEvent::Spoofing {
                    at_s,
                    until_s,
                    label,
                } => {
                    out.push((
                        *at_s,
                        1,
                        Action::SpoofStart {
                            label: label.clone(),
                        },
                    ));
                    if let Some(u) = until_s {
                        out.push((
                            *u,
                            0,
                            Action::SpoofStop {
                                label: label.clone(),
                            },
                        ));
                    }
                }
            }
        }
        for (g, mut v) in deltas {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (mut p, mut c) = (0.0, 0.0);
            let mut i = 0;
            while i < v.len() {
                let t = v[i].0;
                let (p0, c0) = (p, c);
                while i < v.len() && v[i].0 == t {
                    p += v[i].1;
                    c += v[i].2;
                    i += 1;
                }
                let (pc, cc) = ((p - p0).abs() > 1e-12, (c - c0).abs() > 1e-12);
                if pc || cc {
                    out.push((
                        t,
                        2,
                        Action::GroupOffset {
                            group: g.clone(),
                            power_db: p,
                            cn0_db: c,
                            power_changed: pc,
                            cn0_changed: cc,
                        },
                    ));
                }
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        out.into_iter()
            .map(|(t_s, _, action)| TimedAction { t_s, action })
            .collect()
    }

    /// The state at scene time `t_s` (actions at exactly `t_s` applied).
    pub fn state_at(&self, t_s: f64) -> DegradationState {
        let mut offs: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        let mut masked = BTreeSet::new();
        let mut outages = BTreeSet::new();
        let mut jam: BTreeMap<String, f64> = BTreeMap::new();
        let mut spoof = BTreeSet::new();
        for a in self.actions().into_iter().take_while(|a| a.t_s <= t_s) {
            match a.action {
                Action::GroupOffset {
                    group,
                    power_db,
                    cn0_db,
                    ..
                } => {
                    offs.insert(group.label(), (power_db, cn0_db));
                }
                Action::SatDisable { constellation, prn } => {
                    masked.insert((constellation, prn));
                }
                Action::SatEnable { constellation, prn } => {
                    masked.remove(&(constellation, prn));
                }
                Action::SignalOff { group } => {
                    outages.insert(group.label());
                }
                Action::SignalOn { group } => {
                    outages.remove(&group.label());
                }
                Action::JammerStart { id, js_db, .. } => {
                    jam.insert(id, js_db);
                }
                Action::JammerStop { id } => {
                    jam.remove(&id);
                }
                Action::SpoofStart { label } => {
                    spoof.insert(label);
                }
                Action::SpoofStop { label } => {
                    spoof.remove(&label);
                }
            }
        }
        DegradationState {
            offsets_db: offs,
            masked: masked.into_iter().collect(),
            outages: outages.into_iter().collect(),
            jammers: jam.into_iter().collect(),
            spoofing: spoof.into_iter().collect(),
        }
    }

    /// Net power + C/N0 offset (dB) seen by constellation `c`, signal `s` at `t_s`: the sum
    /// over all groups covering it (all, constellation, constellation+signal).
    pub fn effective_offset_db(&self, c: Constellation, s: &str, t_s: f64) -> f64 {
        let mut last: BTreeMap<Group, f64> = BTreeMap::new();
        for a in self.actions().into_iter().take_while(|a| a.t_s <= t_s) {
            if let Action::GroupOffset {
                group,
                power_db,
                cn0_db,
                ..
            } = a.action
            {
                if group.covers(c, s) {
                    last.insert(group, power_db + cn0_db);
                }
            }
        }
        last.values().sum()
    }
}

fn group_fields(p: &CommandProfile, g: &Group) -> Fields {
    p.group_fields(g.constellation, g.signal.as_deref())
}

/// Render the degradation for `profile`. Actions the profile cannot express are listed in
/// [`CommandSchedule::unsupported`] instead of failing; spoofing intervals are always kept
/// as [`ScheduleMarker`]s.
pub fn translate(
    deg: &GnssDegradation,
    profile: &CommandProfile,
) -> Result<CommandSchedule, String> {
    deg.validate()?;
    let mut entries = Vec::new();
    let mut markers = Vec::new();
    let mut unsupported = Vec::new();
    let has_cn0 = profile.supports(ops::SET_CN0_OFFSET);
    for ta in deg.actions() {
        let t = ta.t_s;
        let mut cmds: Vec<(&str, Fields, String)> = Vec::new();
        match &ta.action {
            Action::GroupOffset {
                group,
                power_db,
                cn0_db,
                power_changed,
                cn0_changed,
            } => {
                let mk = |v: f64| {
                    let mut f = group_fields(profile, group);
                    f.insert("offset_db".into(), format!("{v:.2}"));
                    f
                };
                if has_cn0 {
                    if *power_changed {
                        cmds.push((
                            ops::SET_POWER_OFFSET,
                            mk(*power_db),
                            format!("power offset {} = {power_db:.2} dB", group.label()),
                        ));
                    }
                    if *cn0_changed {
                        cmds.push((
                            ops::SET_CN0_OFFSET,
                            mk(*cn0_db),
                            format!("C/N0 offset {} = {cn0_db:.2} dB", group.label()),
                        ));
                    }
                } else {
                    let v = power_db + cn0_db;
                    cmds.push((
                        ops::SET_POWER_OFFSET,
                        mk(v),
                        format!("power offset {} = {v:.2} dB", group.label()),
                    ));
                }
            }
            Action::SatDisable { constellation, prn }
            | Action::SatEnable { constellation, prn } => {
                let mut f = profile.group_fields(Some(*constellation), None);
                f.insert("prn".into(), prn.to_string());
                let (op, w) = if matches!(ta.action, Action::SatDisable { .. }) {
                    (ops::SAT_DISABLE, "off")
                } else {
                    (ops::SAT_ENABLE, "on")
                };
                cmds.push((op, f, format!("{} PRN {prn} {w}", constellation.key())));
            }
            Action::SignalOff { group } => cmds.push((
                ops::SIGNAL_OFF,
                group_fields(profile, group),
                format!("outage start {}", group.label()),
            )),
            Action::SignalOn { group } => cmds.push((
                ops::SIGNAL_ON,
                group_fields(profile, group),
                format!("outage end {}", group.label()),
            )),
            Action::JammerStart {
                id,
                kind,
                center_hz,
                bandwidth_hz,
                js_db,
            } => {
                let mut f = Fields::new();
                f.insert("jammer_id".into(), id.clone());
                f.insert("kind".into(), kind.key().into());
                f.insert("center_hz".into(), format!("{center_hz:.0}"));
                f.insert("center_mhz".into(), format!("{:.6}", center_hz / 1e6));
                f.insert("bandwidth_hz".into(), format!("{bandwidth_hz:.0}"));
                f.insert("js_db".into(), format!("{js_db:.2}"));
                f.insert(
                    "power_dbm".into(),
                    format!("{:.2}", profile.ref_signal_power_dbm + js_db),
                );
                cmds.push((
                    ops::JAMMER_START,
                    f,
                    format!("jammer {id} start ({}, J/S {js_db} dB)", kind.key()),
                ));
            }
            Action::JammerStop { id } => {
                let mut f = Fields::new();
                f.insert("jammer_id".into(), id.clone());
                cmds.push((ops::JAMMER_STOP, f, format!("jammer {id} stop")));
            }
            Action::SpoofStart { label } | Action::SpoofStop { label } => {
                let start = matches!(ta.action, Action::SpoofStart { .. });
                let w = format!("spoofing {label} {}", if start { "start" } else { "stop" });
                markers.push(ScheduleMarker {
                    t_s: t,
                    label: w.clone(),
                });
                let op = if start {
                    ops::SPOOF_START
                } else {
                    ops::SPOOF_STOP
                };
                if profile.supports(op) {
                    let mut f = Fields::new();
                    f.insert("label".into(), label.clone());
                    cmds.push((op, f, w));
                }
            }
        }
        for (op, f, what) in cmds {
            if !profile.supports(op) {
                unsupported.push(format!(
                    "t = {t} s: {what} (profile {} has no {op:?} command)",
                    profile.name
                ));
                continue;
            }
            let mut all = profile.time_fields(Some(t));
            all.extend(f);
            let lines = profile
                .templates(op)
                .iter()
                .map(|tpl| super::profile::fill_template(tpl, &all))
                .collect::<Result<Vec<_>, _>>()?;
            entries.push(ScheduledCommand {
                t_s: t,
                op: op.into(),
                lines,
                what,
            });
        }
    }
    Ok(CommandSchedule {
        profile: profile.name.clone(),
        mode: profile.schedule,
        entries,
        markers,
        unsupported,
        profile_confirmed: profile.confirmed,
    })
}

#[cfg(test)]
mod tests {
    use super::super::{labsat, rs_smbv, skydel, spirent};
    use super::*;

    const SRC: &str = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"

[gnss_degradation]
description = "test"

[[gnss_degradation.events]]
type = "cn0_offset"
at_s = 10.0
until_s = 40.0
constellation = "gps"
signal = "L1CA"
offset_db = -6.0

[[gnss_degradation.events]]
type = "power_step"
at_s = 20.0
constellation = "gps"
signal = "L1CA"
step_db = -3.0

[[gnss_degradation.events]]
type = "sat_mask"
at_s = 30.0
until_s = 90.0
constellation = "gps"
prns = [5, 12]

[[gnss_degradation.events]]
type = "outage"
at_s = 100.0
duration_s = 30.0
constellation = "beidou"

[[gnss_degradation.events]]
type = "jamming"
at_s = 150.0
duration_s = 60.0
id = "j1"
kind = "cw"
center_hz = 1575420000.0
js_db = 40.0

[[gnss_degradation.events]]
type = "spoofing"
at_s = 250.0
until_s = 400.0
label = "meaconing"
"#;

    fn deg() -> GnssDegradation {
        GnssDegradation::from_scenario_toml(SRC).unwrap().unwrap()
    }

    #[test]
    fn parses_section_and_compiles_timeline() {
        let d = deg();
        assert_eq!(d.events.len(), 6);
        assert!(GnssDegradation::from_scenario_toml("[scene]\nx=1")
            .unwrap()
            .is_none());
        let a = d.actions();
        let times: Vec<f64> = a.iter().map(|a| a.t_s).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]));
        // group offset at 10 (cn0 -6), 20 (power -3), 40 (cn0 back to 0)
        let offs: Vec<(f64, f64, f64)> = a
            .iter()
            .filter_map(|a| match &a.action {
                Action::GroupOffset {
                    power_db, cn0_db, ..
                } => Some((a.t_s, *power_db, *cn0_db)),
                _ => None,
            })
            .collect();
        assert_eq!(
            offs,
            vec![(10.0, 0.0, -6.0), (20.0, -3.0, -6.0), (40.0, -3.0, 0.0)]
        );
        let s = d.state_at(35.0);
        assert_eq!(
            s.masked,
            vec![(Constellation::Gps, 5), (Constellation::Gps, 12)]
        );
        assert_eq!(s.offsets_db["gps/L1CA"], (-3.0, -6.0));
        assert!(d.state_at(95.0).masked.is_empty());
        assert_eq!(d.state_at(110.0).outages, vec!["beidou/all".to_string()]);
        assert!(d.state_at(130.0).outages.is_empty());
        assert_eq!(d.state_at(200.0).jammers, vec![("j1".to_string(), 40.0)]);
        assert_eq!(d.state_at(300.0).spoofing, vec!["meaconing".to_string()]);
        assert!((d.effective_offset_db(Constellation::Gps, "L1CA", 25.0) + 9.0).abs() < 1e-12);
        assert_eq!(d.effective_offset_db(Constellation::Gps, "L5", 25.0), 0.0);
    }

    #[test]
    fn rejects_bad_events() {
        let mut t = toml::Table::new();
        let bad = r#"
[[events]]
type = "outage"
at_s = 1.0
duration_s = 0.0
"#;
        t.extend(toml::from_str::<toml::Table>(bad).unwrap());
        assert!(GnssDegradation::from_toml_table(&t).is_err());
        let bad = SRC.replace("until_s = 90.0", "until_s = 10.0");
        assert!(GnssDegradation::from_scenario_toml(&bad).is_err());
        let bad = SRC.replace("type = \"spoofing\"", "type = \"teleport\"");
        assert!(GnssDegradation::from_scenario_toml(&bad).is_err());
    }

    #[test]
    fn translates_for_skydel() {
        let s = translate(&deg(), &skydel::default_profile()).unwrap();
        assert!(!s.profile_confirmed);
        assert_eq!(s.mode, super::super::profile::ScheduleMode::HostTimed);
        let first = &s.entries[0];
        assert_eq!(first.t_s, 10.0);
        assert_eq!(first.op, ops::SET_POWER_OFFSET);
        assert!(
            first.lines[0].contains("\"System\":\"GPS\",\"Signal\":\"L1CA\",\"Offset\":-6.00"),
            "{:?}",
            first.lines
        );
        // all six kinds of event appear as commands or markers
        assert!(s.entries.iter().any(|e| e.op == ops::SAT_DISABLE));
        assert!(s.entries.iter().any(|e| e.op == ops::JAMMER_START));
        assert_eq!(s.markers.len(), 2);
        for e in &s.entries {
            for l in &e.lines {
                serde_json::from_str::<serde_json::Value>(l)
                    .unwrap_or_else(|err| panic!("{l}: {err}"));
            }
        }
    }

    #[test]
    fn translates_for_spirent_time_tagged() {
        let s = translate(&deg(), &spirent::default_profile()).unwrap();
        assert_eq!(s.mode, super::super::profile::ScheduleMode::TimeTagged);
        let l = s.all_lines();
        assert!(l.iter().any(|x| x.starts_with("00:00:10.00,")), "{l:?}");
        assert!(l.iter().any(|x| x.starts_with("00:02:30.00,")), "{l:?}");
    }

    #[test]
    fn translates_for_smbv_with_scpi_names() {
        let s = translate(&deg(), &rs_smbv::default_profile()).unwrap();
        let l = s.all_lines();
        assert!(
            l.iter().all(|x| x.starts_with(':') || x.starts_with('*')),
            "{l:?}"
        );
        assert!(
            l.iter().any(|x| x.contains("GPS") && x.contains("-3.00")),
            "{l:?}"
        );
        // C/N0 command absent: power offset carries the combined -9 dB at t = 20 s
        assert!(s
            .entries
            .iter()
            .any(|e| e.t_s == 20.0 && e.lines.iter().any(|x| x.contains("-9.00"))));
    }

    #[test]
    fn translates_for_labsat_mostly_unsupported() {
        let s = translate(&deg(), &labsat::default_profile()).unwrap();
        assert!(s.entries.is_empty());
        assert!(s.unsupported.len() >= 8, "{:?}", s.unsupported);
        assert_eq!(s.markers.len(), 2);
    }

    #[test]
    fn gps_sdr_sim_reports_offline_limitations() {
        let n = super::super::gpssdrsim::degradation_support(&deg());
        assert!(n.iter().any(|x| x.contains("jammer")), "{n:?}");
    }
}
