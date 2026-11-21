// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Area console (roadmap 0.42 F1, with the F2 rule)
//!
//! Fuse Kshana Stations, receivers and customer-supplied third-party feeds into one **area
//! trust picture** per monitored area, with an event timeline, and link events that several
//! areas see at the same time (wide-area events).
//!
//! * **Stations and receivers** report a state per epoch (`nominal`, `degraded`, `untrusted`,
//!   the open engine's per-station rule). Per area they go through the open engine's
//!   pre-registered area rule ([`kshana::area_trust::timeline_with`], version 1, 2 or 3's
//!   aggregation), unchanged: Kshana Pro adds the fusion around it, not a second area rule.
//! * **Third-party feeds** (customer-supplied detections, e.g. from a commercial RF-monitoring
//!   service or a national network) are intervals with a kind and a confidence. A feed alone can
//!   raise an area to at most `DEGRADED` and the event is marked **uncorroborated**; it is
//!   **corroborated** when the area's own network is degraded or untrusted at the same time.
//! * **Events** are the maximal runs of seconds in which the fused state is `DEGRADED` or
//!   `UNTRUSTED`, with the sources that took part, the basis (network, feed, both), and the state
//!   changes inside. Events of different areas that overlap (within [`LINK_TOLERANCE_S`]) form a
//!   wide-area event.
//!
//! **F2: no emitter positions.** The console shows detections and areas, **never emitter
//! positions**. A feed's emitter estimate (`lat_deg`, `lon_deg` columns) is read only to be
//! discarded; the picture counts how many were withheld and carries no coordinate of any
//! emitter. This is a hard switch ([`EMITTER_POSITIONS`]): it has no "on" value, no
//! configuration key and no environment variable, so turning it on takes a code change, which
//! the roadmap allows only after the open locator successor (B3c) meets its pre-registered gate
//! on the blind JammerTest 2027 real-data run. B3c has met only its wideband synthetic gate.
//!
//! **Tenancy.** Each area belongs to a tenant ([`Picture::for_tenant`] filters a picture to one).
//! A console with more than one tenant needs an Enterprise licence (the hook of the roadmap's
//! multi-tenant Enterprise operation: [`multi_tenant_allowed`]); the service serves each tenant
//! its own console (`<data-dir>/area/<tenant>/console.toml`).
//!
//! MODELLED where the inputs are: the console fuses what the sources report; it adds no
//! detection of its own and cannot be better than its sources.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::area_trust::{self, AreaRule, AreaState, StationState};
use serde::{Deserialize, Serialize};

/// The licence feature of the console (CLI and service).
pub const FEATURE: &str = "pro-area-console";
/// The scenario kind: a console file in, the picture out.
pub const KIND: &str = "pro-area-picture";
/// The console file's format tag.
pub const FORMAT: &str = "kshana-pro-area-console/1";
/// The picture's format tag.
pub const PICTURE_FORMAT: &str = "kshana-pro-area-picture/1";
/// Events of different areas closer than this are linked into one wide-area event, s.
pub const LINK_TOLERANCE_S: i64 = 60;

/// The F2 switch for emitter positions in the console. It has one value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PositionSwitch {
    /// Emitter positions are never shown, stored or exported by the console.
    Off,
}

/// F2: emitter positions stay off until the B3c locator meets its pre-registered gate on the
/// blind JammerTest 2027 real-data run (roadmap 0.42 F2 and V1).
pub const EMITTER_POSITIONS: PositionSwitch = PositionSwitch::Off;

/// Why positions are off (written into every picture and page).
pub const POSITIONS_REASON: &str = "Emitter positions are off: the console shows detections and \
areas only. They stay off until the locator successor (B3c) meets its pre-registered gate on the \
blind JammerTest 2027 real-data run; it has met only its wideband synthetic gate.";

/// The honesty label of every picture.
pub const LABEL: &str =
    "MODELLED: the picture fuses what the sources report, through the open engine's \
pre-registered area rule; it adds no detection of its own. Feed-only events are uncorroborated.";

/// A source's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// A Kshana Station.
    Station,
    /// A receiver whose trust is scored (receiver-trust).
    Receiver,
    /// A customer-supplied third-party detection feed.
    Feed,
}

/// One monitored area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaDef {
    pub id: String,
    pub name: String,
    #[serde(default = "default_tenant")]
    pub tenant: String,
    /// The open area rule: `v1`, `v2` (default) or `v3`.
    #[serde(default = "default_rule")]
    pub rule: AreaRule,
}

fn default_tenant() -> String {
    "default".into()
}

fn default_rule() -> AreaRule {
    AreaRule::V2
}

/// One source of the console.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDef {
    pub id: String,
    pub kind: SourceKind,
    pub area: String,
    /// The source's file, relative to the console file.
    pub file: String,
    /// Who supplies a feed (required for a feed; e.g. "customer-supplied national network").
    #[serde(default)]
    pub provider: Option<String>,
    /// A feed's detections below this confidence are left out (default 0.5).
    #[serde(default)]
    pub min_confidence: Option<f64>,
}

/// A console file (`kshana-pro-area-console/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleSpec {
    pub format: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "area")]
    pub areas: Vec<AreaDef>,
    #[serde(rename = "source")]
    pub sources: Vec<SourceDef>,
}

fn ok_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

impl ConsoleSpec {
    pub fn parse(text: &str) -> Result<ConsoleSpec, String> {
        let s: ConsoleSpec = toml::from_str(text).map_err(|e| format!("console file: {e}"))?;
        s.validate()?;
        Ok(s)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT {
            return Err(format!("format must be {FORMAT:?}"));
        }
        if self.areas.is_empty() || self.areas.len() > 256 {
            return Err("between 1 and 256 areas".into());
        }
        if self.sources.len() > 4096 {
            return Err("at most 4096 sources".into());
        }
        for (i, a) in self.areas.iter().enumerate() {
            if !ok_id(&a.id) || !ok_id(&a.tenant) {
                return Err(format!(
                    "area {}: id and tenant are 1 to 64 letters, digits, '-', '_' or '.'",
                    i + 1
                ));
            }
            if self.areas[..i].iter().any(|b| b.id == a.id) {
                return Err(format!("area {} is listed twice", a.id));
            }
        }
        for (i, s) in self.sources.iter().enumerate() {
            if !ok_id(&s.id) {
                return Err(format!(
                    "source {}: id is 1 to 64 letters, digits, '-', '_' or '.'",
                    i + 1
                ));
            }
            if self.sources[..i].iter().any(|b| b.id == s.id) {
                return Err(format!("source {} is listed twice", s.id));
            }
            if !self.areas.iter().any(|a| a.id == s.area) {
                return Err(format!("source {}: no area {:?}", s.id, s.area));
            }
            if s.file.is_empty() {
                return Err(format!("source {}: file is empty", s.id));
            }
            match s.kind {
                SourceKind::Feed => {
                    if s.provider.as_deref().is_none_or(|p| p.trim().is_empty()) {
                        return Err(format!("feed {}: provider is required", s.id));
                    }
                    if let Some(c) = s.min_confidence {
                        if !(0.0..=1.0).contains(&c) {
                            return Err(format!("feed {}: min_confidence is in [0, 1]", s.id));
                        }
                    }
                }
                _ => {
                    if s.provider.is_some() || s.min_confidence.is_some() {
                        return Err(format!(
                            "source {}: provider and min_confidence are for feeds only",
                            s.id
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// The distinct tenants, sorted.
    pub fn tenants(&self) -> Vec<String> {
        let mut t: Vec<String> = self.areas.iter().map(|a| a.tenant.clone()).collect();
        t.sort();
        t.dedup();
        t
    }
}

/// Whether `ent` may run a console with more than one tenant (Enterprise only).
pub fn multi_tenant_allowed(ent: &crate::license::Entitlement) -> bool {
    ent.tier >= crate::license::Tier::Enterprise
}

/// One feed detection (the emitter estimate, if the feed had one, is already discarded).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    pub source: String,
    pub start_s: i64,
    pub end_s: i64,
    pub kind: String,
    pub confidence: f64,
}

/// A loaded source.
#[derive(Clone, Debug)]
pub enum Loaded {
    States(Vec<(f64, StationState)>),
    Detections(Vec<Detection>),
}

/// The inputs of a console, loaded.
#[derive(Clone, Debug)]
pub struct Inputs {
    pub spec: ConsoleSpec,
    pub data: Vec<Loaded>,
    /// Emitter estimates read from feeds and discarded (F2).
    pub positions_withheld: usize,
    /// SHA-256 over the console file and every source file, in order.
    pub sha256: String,
}

/// Parse a station or receiver state file: `t_s,state[,note]`. `t_s` is Unix seconds on UTC
/// for every station, receiver and feed (corroboration aligns them second by second); a source
/// on GPS seconds or another epoch must be converted first, or it is misaligned by 18 s or
/// more.
pub fn parse_states(id: &str, text: &str) -> Result<Vec<(f64, StationState)>, String> {
    let mut out: Vec<(f64, StationState)> = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("t_s") {
            continue;
        }
        let mut f = line.split(',').map(str::trim);
        let t: f64 = f
            .next()
            .and_then(|v| v.parse().ok())
            .filter(|v: &f64| v.is_finite())
            .ok_or(format!("{id} line {}: t_s is not a number", ln + 1))?;
        let st = match f.next() {
            Some("nominal") => StationState::Nominal,
            Some("degraded") => StationState::Degraded,
            Some("untrusted") => StationState::Untrusted,
            other => {
                return Err(format!(
                    "{id} line {}: state {:?} (nominal, degraded, untrusted)",
                    ln + 1,
                    other.unwrap_or("")
                ))
            }
        };
        if out.last().is_some_and(|p| p.0 > t) {
            return Err(format!("{id} line {}: times must not decrease", ln + 1));
        }
        out.push((t, st));
    }
    Ok(out)
}

/// Parse a feed file: `t_start_s,t_end_s,kind,confidence[,lat_deg,lon_deg]`, times in Unix
/// seconds on UTC like the station files. Returns the
/// detections at or above `min_conf` and the number of emitter estimates discarded (F2).
pub fn parse_feed(id: &str, text: &str, min_conf: f64) -> Result<(Vec<Detection>, usize), String> {
    let mut out = Vec::new();
    let mut withheld = 0;
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("t_start_s") {
            continue;
        }
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() != 4 && f.len() != 6 {
            return Err(format!("{id} line {}: 4 or 6 fields", ln + 1));
        }
        let num = |s: &str, what: &str| -> Result<f64, String> {
            s.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or(format!("{id} line {}: {what} is not a number", ln + 1))
        };
        let (a, b) = (num(f[0], "t_start_s")?, num(f[1], "t_end_s")?);
        if b < a {
            return Err(format!("{id} line {}: t_end_s before t_start_s", ln + 1));
        }
        let kind = f[2];
        if !matches!(kind, "jamming" | "spoofing" | "interference") {
            return Err(format!(
                "{id} line {}: kind {kind:?} (jamming, spoofing, interference)",
                ln + 1
            ));
        }
        let c = num(f[3], "confidence")?;
        if !(0.0..=1.0).contains(&c) {
            return Err(format!("{id} line {}: confidence is in [0, 1]", ln + 1));
        }
        if f.len() == 6 && !(f[4].is_empty() && f[5].is_empty()) {
            // F2: an emitter estimate is validated as a field and then dropped; nothing of it
            // is kept.
            num(f[4], "lat_deg")?;
            num(f[5], "lon_deg")?;
            withheld += 1;
        }
        if c >= min_conf {
            out.push(Detection {
                source: id.into(),
                start_s: a.floor() as i64,
                end_s: b.floor() as i64,
                kind: kind.into(),
                confidence: c,
            });
        }
    }
    Ok((out, withheld))
}

impl Inputs {
    /// Load a console file and its sources (paths relative to the file's directory).
    pub fn load(path: &Path) -> Result<Inputs, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Inputs::load_text(&text, &base)
    }

    pub fn load_text(text: &str, base: &Path) -> Result<Inputs, String> {
        use sha2::{Digest, Sha256};
        let spec = ConsoleSpec::parse(text)?;
        let mut h = Sha256::new();
        h.update(text.as_bytes());
        let mut data = Vec::with_capacity(spec.sources.len());
        let mut withheld = 0;
        for s in &spec.sources {
            if Path::new(&s.file).is_absolute() || s.file.split(['/', '\\']).any(|c| c == "..") {
                return Err(format!(
                    "source {}: file must be a relative path inside the console's directory",
                    s.id
                ));
            }
            let p = base.join(&s.file);
            let t = std::fs::read_to_string(&p)
                .map_err(|e| format!("source {}: {}: {e}", s.id, p.display()))?;
            h.update(t.as_bytes());
            data.push(match s.kind {
                SourceKind::Feed => {
                    let (d, w) = parse_feed(&s.id, &t, s.min_confidence.unwrap_or(0.5))?;
                    withheld += w;
                    Loaded::Detections(d)
                }
                _ => Loaded::States(parse_states(&s.id, &t)?),
            });
        }
        Ok(Inputs {
            spec,
            data,
            positions_withheld: withheld,
            sha256: hex::encode(h.finalize()),
        })
    }
}

// ------------------------------------------------------------------ the picture

/// A fused state (the open area state, plus whether a feed raised it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Fused {
    Unknown,
    Trusted,
    Degraded,
    Untrusted,
}

impl Fused {
    fn of(a: AreaState) -> Fused {
        match a {
            AreaState::Unknown => Fused::Unknown,
            AreaState::Trusted => Fused::Trusted,
            AreaState::Degraded => Fused::Degraded,
            AreaState::Untrusted => Fused::Untrusted,
        }
    }
    fn rank(self) -> u8 {
        match self {
            Fused::Unknown => 0,
            Fused::Trusted => 1,
            Fused::Degraded => 2,
            Fused::Untrusted => 3,
        }
    }
    pub fn is_event(self) -> bool {
        matches!(self, Fused::Degraded | Fused::Untrusted)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Fused::Unknown => "UNKNOWN",
            Fused::Trusted => "TRUSTED",
            Fused::Degraded => "DEGRADED",
            Fused::Untrusted => "UNTRUSTED",
        }
    }
}

/// One second of an area's fused timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Second {
    pub g: i64,
    /// The open area rule's published state.
    pub network: Fused,
    /// Feed detections active this second.
    pub feeds: usize,
    pub fused: Fused,
}

/// A change of fused state inside an event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub g: i64,
    pub state: Fused,
}

/// One event of an area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub area: String,
    pub tenant: String,
    pub start_g: i64,
    pub end_g: i64,
    pub duration_s: i64,
    pub worst: Fused,
    /// `network`, `feed` or `network+feed`.
    pub basis: String,
    /// A feed detection and the area's own network agreed at some second.
    pub corroborated: bool,
    /// Stations and receivers that reported degraded or untrusted during the event.
    pub sources: Vec<String>,
    /// Feed detections overlapping the event (no emitter position: F2).
    pub detections: Vec<Detection>,
    pub changes: Vec<Change>,
    /// The wide-area event this belongs to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wide_area: Option<String>,
}

/// Events of several areas at the same time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WideArea {
    pub id: String,
    pub start_g: i64,
    pub end_g: i64,
    pub areas: Vec<String>,
    pub tenants: Vec<String>,
    pub events: Vec<String>,
}

/// One area's picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaPicture {
    pub id: String,
    pub name: String,
    pub tenant: String,
    pub rule: AreaRule,
    /// Sources by kind: station, receiver, feed.
    pub sources: BTreeMap<String, usize>,
    pub first_g: Option<i64>,
    pub last_g: Option<i64>,
    /// The fused state at the last second.
    pub current: Fused,
    /// Seconds per fused state.
    pub seconds: BTreeMap<String, usize>,
    pub events: Vec<Event>,
    /// The fused timeline, run-length encoded: `(first second, state, length)`.
    pub timeline: Vec<(i64, Fused, i64)>,
}

/// What the picture says about emitter positions (F2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Positions {
    pub emitter_positions: String,
    pub withheld: usize,
    pub reason: String,
}

/// The area trust picture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Picture {
    pub kind: String,
    pub format: String,
    pub title: String,
    pub inputs_sha256: String,
    pub areas: Vec<AreaPicture>,
    pub wide_area: Vec<WideArea>,
    pub positions: Positions,
    pub label: String,
}

fn area_seconds(inputs: &Inputs, area: &AreaDef) -> (Vec<Second>, Vec<usize>, Vec<usize>) {
    let mut stations = Vec::new();
    let mut st_idx = Vec::new();
    let mut feed_idx = Vec::new();
    for (i, s) in inputs.spec.sources.iter().enumerate() {
        if s.area != area.id {
            continue;
        }
        match &inputs.data[i] {
            Loaded::States(e) => {
                stations.push(area_trust::Station {
                    id: s.id.clone(),
                    epochs: e.clone(),
                });
                st_idx.push(i);
            }
            Loaded::Detections(_) => feed_idx.push(i),
        }
    }
    let net = area_trust::timeline_with(&stations, area.rule);
    let mut dets: Vec<&Detection> = Vec::new();
    for &i in &feed_idx {
        if let Loaded::Detections(d) = &inputs.data[i] {
            dets.extend(d.iter());
        }
    }
    let lo = net
        .first()
        .map(|s| s.g)
        .into_iter()
        .chain(dets.iter().map(|d| d.start_s))
        .min();
    let hi = net
        .last()
        .map(|s| s.g)
        .into_iter()
        .chain(dets.iter().map(|d| d.end_s))
        .max();
    let (Some(lo), Some(hi)) = (lo, hi) else {
        return (Vec::new(), st_idx, feed_idx);
    };
    let net_lo = net.first().map(|s| s.g).unwrap_or(i64::MAX);
    let mut out = Vec::with_capacity((hi - lo + 1).max(0) as usize);
    for g in lo..=hi {
        let network = if g >= net_lo && ((g - net_lo) as usize) < net.len() {
            Fused::of(net[(g - net_lo) as usize].state)
        } else {
            Fused::Unknown
        };
        let feeds = dets
            .iter()
            .filter(|d| d.start_s <= g && g <= d.end_s)
            .count();
        let fused = match network {
            Fused::Untrusted | Fused::Degraded => network,
            _ if feeds > 0 => Fused::Degraded,
            other => other,
        };
        out.push(Second {
            g,
            network,
            feeds,
            fused,
        });
    }
    (out, st_idx, feed_idx)
}

/// Build the picture from loaded inputs.
pub fn picture(inputs: &Inputs) -> Picture {
    let mut areas = Vec::new();
    for a in &inputs.spec.areas {
        let (secs, st_idx, feed_idx) = area_seconds(inputs, a);
        let mut sources = BTreeMap::new();
        for s in inputs.spec.sources.iter().filter(|s| s.area == a.id) {
            let k = match s.kind {
                SourceKind::Station => "station",
                SourceKind::Receiver => "receiver",
                SourceKind::Feed => "feed",
            };
            *sources.entry(k.to_string()).or_insert(0) += 1;
        }
        let mut seconds = BTreeMap::new();
        for s in &secs {
            *seconds.entry(s.fused.as_str().to_string()).or_insert(0) += 1;
        }
        let mut timeline: Vec<(i64, Fused, i64)> = Vec::new();
        for s in &secs {
            match timeline.last_mut() {
                Some(l) if l.1 == s.fused && l.0 + l.2 == s.g => l.2 += 1,
                _ => timeline.push((s.g, s.fused, 1)),
            }
        }
        // Events.
        let mut events = Vec::new();
        let mut k = 0;
        while k < secs.len() {
            if !secs[k].fused.is_event() {
                k += 1;
                continue;
            }
            let start = k;
            while k < secs.len() && secs[k].fused.is_event() {
                k += 1;
            }
            let run = &secs[start..k];
            let (g0, g1) = (run[0].g, run[run.len() - 1].g);
            let worst = run
                .iter()
                .map(|s| s.fused)
                .max_by_key(|f| f.rank())
                .unwrap_or(Fused::Degraded);
            let net_part = run.iter().any(|s| s.network.is_event());
            let feed_part = run.iter().any(|s| s.feeds > 0);
            let corroborated = run.iter().any(|s| s.network.is_event() && s.feeds > 0);
            let basis = match (net_part, feed_part) {
                (true, true) => "network+feed",
                (true, false) => "network",
                _ => "feed",
            };
            let mut srcs = Vec::new();
            for &i in &st_idx {
                if let Loaded::States(e) = &inputs.data[i] {
                    let hit = e.iter().any(|(t, st)| {
                        let g = t.floor() as i64;
                        g >= g0 && g <= g1 && *st >= StationState::Degraded
                    });
                    if hit {
                        srcs.push(inputs.spec.sources[i].id.clone());
                    }
                }
            }
            let mut detections = Vec::new();
            for &i in &feed_idx {
                if let Loaded::Detections(d) = &inputs.data[i] {
                    detections.extend(
                        d.iter()
                            .filter(|d| d.start_s <= g1 && d.end_s >= g0)
                            .cloned(),
                    );
                }
            }
            let mut changes = Vec::new();
            for s in run {
                if changes.last().is_none_or(|c: &Change| c.state != s.fused) {
                    changes.push(Change {
                        g: s.g,
                        state: s.fused,
                    });
                }
            }
            events.push(Event {
                id: format!("{}-{}", a.id, events.len() + 1),
                area: a.id.clone(),
                tenant: a.tenant.clone(),
                start_g: g0,
                end_g: g1,
                duration_s: g1 - g0 + 1,
                worst,
                basis: basis.into(),
                corroborated,
                sources: srcs,
                detections,
                changes,
                wide_area: None,
            });
        }
        areas.push(AreaPicture {
            id: a.id.clone(),
            name: a.name.clone(),
            tenant: a.tenant.clone(),
            rule: a.rule,
            sources,
            first_g: secs.first().map(|s| s.g),
            last_g: secs.last().map(|s| s.g),
            current: secs.last().map(|s| s.fused).unwrap_or(Fused::Unknown),
            seconds,
            events,
            timeline,
        });
    }
    // Wide-area linking: connected components of overlapping events of different areas.
    let mut all: Vec<(usize, usize)> = Vec::new();
    for (ai, a) in areas.iter().enumerate() {
        for ei in 0..a.events.len() {
            all.push((ai, ei));
        }
    }
    let ev = |p: (usize, usize)| &areas[p.0].events[p.1];
    let n = all.len();
    let mut comp: Vec<usize> = (0..n).collect();
    fn find(c: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while c[r] != r {
            r = c[r];
        }
        let mut j = i;
        while c[j] != r {
            let next = c[j];
            c[j] = r;
            j = next;
        }
        r
    }
    for i in 0..n {
        for j in i + 1..n {
            let (x, y) = (ev(all[i]), ev(all[j]));
            if x.area != y.area
                && x.start_g <= y.end_g + LINK_TOLERANCE_S
                && y.start_g <= x.end_g + LINK_TOLERANCE_S
            {
                let (a, b) = (find(&mut comp, i), find(&mut comp, j));
                if a != b {
                    comp[a.max(b)] = a.min(b);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut comp, i);
        groups.entry(r).or_default().push(i);
    }
    let mut wide = Vec::new();
    let mut assign: Vec<((usize, usize), String)> = Vec::new();
    let mut ordered: Vec<Vec<usize>> = groups
        .into_values()
        .filter(|g| {
            let mut ar: Vec<&str> = g.iter().map(|&i| ev(all[i]).area.as_str()).collect();
            ar.sort();
            ar.dedup();
            ar.len() >= 2
        })
        .collect();
    ordered.sort_by_key(|g| g.iter().map(|&i| ev(all[i]).start_g).min().unwrap_or(0));
    for g in ordered {
        let id = format!("wide-{}", wide.len() + 1);
        let mut ars: Vec<String> = g.iter().map(|&i| ev(all[i]).area.clone()).collect();
        ars.sort();
        ars.dedup();
        let mut ten: Vec<String> = g.iter().map(|&i| ev(all[i]).tenant.clone()).collect();
        ten.sort();
        ten.dedup();
        let mut evs: Vec<String> = g.iter().map(|&i| ev(all[i]).id.clone()).collect();
        evs.sort();
        wide.push(WideArea {
            id: id.clone(),
            start_g: g.iter().map(|&i| ev(all[i]).start_g).min().unwrap_or(0),
            end_g: g.iter().map(|&i| ev(all[i]).end_g).max().unwrap_or(0),
            areas: ars,
            tenants: ten,
            events: evs,
        });
        for &i in &g {
            assign.push((all[i], id.clone()));
        }
    }
    for ((ai, ei), id) in assign {
        areas[ai].events[ei].wide_area = Some(id);
    }
    Picture {
        kind: KIND.into(),
        format: PICTURE_FORMAT.into(),
        title: inputs
            .spec
            .title
            .clone()
            .unwrap_or_else(|| "Area trust picture".into()),
        inputs_sha256: inputs.sha256.clone(),
        areas,
        wide_area: wide,
        positions: Positions {
            emitter_positions: match EMITTER_POSITIONS {
                PositionSwitch::Off => "off".into(),
            },
            withheld: inputs.positions_withheld,
            reason: POSITIONS_REASON.into(),
        },
        label: LABEL.into(),
    }
}

impl Picture {
    /// The picture of one tenant: its areas, their events, and the wide-area events they take
    /// part in (listing only that tenant's areas and events).
    pub fn for_tenant(&self, tenant: &str) -> Picture {
        let mut p = self.clone();
        p.areas.retain(|a| a.tenant == tenant);
        let mine: Vec<String> = p
            .areas
            .iter()
            .flat_map(|a| a.events.iter().map(|e| e.id.clone()))
            .collect();
        p.wide_area = self
            .wide_area
            .iter()
            .filter_map(|w| {
                let evs: Vec<String> = w
                    .events
                    .iter()
                    .filter(|e| mine.contains(e))
                    .cloned()
                    .collect();
                if evs.is_empty() {
                    return None;
                }
                let mut w = w.clone();
                w.areas.retain(|a| p.areas.iter().any(|x| &x.id == a));
                w.tenants = vec![tenant.to_string()];
                w.events = evs;
                Some(w)
            })
            .collect();
        p
    }

    /// Every event of the picture, earliest first.
    pub fn events(&self) -> Vec<&Event> {
        let mut v: Vec<&Event> = self.areas.iter().flat_map(|a| a.events.iter()).collect();
        v.sort_by(|a, b| (a.start_g, &a.id).cmp(&(b.start_g, &b.id)));
        v
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the picture is plain data") + "\n"
    }
}

// ------------------------------------------------------------------ the page

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn colour(f: Fused) -> &'static str {
    match f {
        Fused::Unknown => "#6b7280",
        Fused::Trusted => "#2f9e6e",
        Fused::Degraded => "#d39b2a",
        Fused::Untrusted => "#d0453a",
    }
}

fn bar(a: &AreaPicture) -> String {
    let (Some(lo), Some(hi)) = (a.first_g, a.last_g) else {
        return String::new();
    };
    let span = (hi - lo + 1).max(1) as f64;
    let mut s = format!(
        "<svg viewBox=\"0 0 600 18\" class=\"bar\" role=\"img\" aria-label=\"Fused state of {} over time\">",
        esc(&a.name)
    );
    for (g, f, len) in &a.timeline {
        s.push_str(&format!(
            "<rect x=\"{:.2}\" y=\"0\" width=\"{:.2}\" height=\"18\" fill=\"{}\"/>",
            600.0 * (g - lo) as f64 / span,
            (600.0 * *len as f64 / span).max(0.3),
            colour(*f)
        ));
    }
    s.push_str("</svg>");
    s
}

/// The console page: one self-contained HTML page (no script, no external resource).
pub fn page(p: &Picture, note: &str) -> String {
    let mut h = String::new();
    h.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    h.push_str(&format!("<title>{}</title>", esc(&p.title)));
    h.push_str("<style>:root{--bg:#0f1216;--fg:#e5e7eb;--mut:#9aa3af;--card:#171b21;--line:#2a313b}@media (prefers-color-scheme: light){:root{--bg:#f7f7f5;--fg:#111827;--mut:#4b5563;--card:#fff;--line:#e5e7eb}}body{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif;margin:0;padding:16px}main{max-width:1100px;margin:0 auto}h1{font-size:20px;margin:0 0 4px}.mut{color:var(--mut)}.banner{border:1px solid var(--line);border-left:4px solid #8cc4f5;padding:8px 12px;margin:12px 0;background:var(--card)}.card{background:var(--card);border:1px solid var(--line);border-radius:6px;padding:12px;margin:12px 0}.bar{width:100%;height:18px;display:block}table{border-collapse:collapse;width:100%;font-size:13px}th,td{text-align:left;padding:4px 6px;border-bottom:1px solid var(--line);vertical-align:top}.st{display:inline-block;padding:1px 6px;border-radius:3px;color:#fff;font-weight:600;font-size:12px}.wrap{overflow-x:auto}</style></head><body><main>");
    h.push_str(&format!(
        "<h1>{}</h1><div class=\"mut\">{} areas · {} events · {} wide-area · inputs {} · {}</div>",
        esc(&p.title),
        p.areas.len(),
        p.events().len(),
        p.wide_area.len(),
        &p.inputs_sha256[..12.min(p.inputs_sha256.len())],
        esc(note)
    ));
    h.push_str(&format!(
        "<div class=\"banner\"><strong>Emitter positions: {}.</strong> {} ({} emitter estimates from feeds withheld.)</div>",
        esc(&p.positions.emitter_positions),
        esc(&p.positions.reason),
        p.positions.withheld
    ));
    let st = |f: Fused| {
        format!(
            "<span class=\"st\" style=\"background:{}\">{}</span>",
            colour(f),
            f.as_str()
        )
    };
    if !p.wide_area.is_empty() {
        h.push_str("<div class=\"card\"><h2>Wide-area events</h2><div class=\"wrap\"><table><tr><th>Event</th><th>From (s)</th><th>To (s)</th><th>Areas</th><th>Area events</th></tr>");
        for w in &p.wide_area {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&w.id),
                w.start_g,
                w.end_g,
                esc(&w.areas.join(", ")),
                esc(&w.events.join(", "))
            ));
        }
        h.push_str("</table></div></div>");
    }
    for a in &p.areas {
        let srcs: Vec<String> = a.sources.iter().map(|(k, n)| format!("{n} {k}")).collect();
        h.push_str(&format!(
            "<div class=\"card\"><h2>{} {}</h2><div class=\"mut\">{} · tenant {} · rule {:?} · {}</div>{}",
            esc(&a.name),
            st(a.current),
            esc(&a.id),
            esc(&a.tenant),
            a.rule,
            esc(&srcs.join(", ")),
            bar(a)
        ));
        if a.events.is_empty() {
            h.push_str("<p class=\"mut\">No events.</p>");
        } else {
            h.push_str("<div class=\"wrap\"><table><tr><th>Event</th><th>From (s)</th><th>Duration</th><th>Worst</th><th>Basis</th><th>Sources</th><th>Detections (no positions)</th><th>Timeline</th></tr>");
            for e in &a.events {
                let dets: Vec<String> = e
                    .detections
                    .iter()
                    .map(|d| format!("{} {} ({:.2})", d.source, d.kind, d.confidence))
                    .collect();
                let ch: Vec<String> = e
                    .changes
                    .iter()
                    .map(|c| format!("{} {}", c.g, c.state.as_str()))
                    .collect();
                h.push_str(&format!(
                    "<tr><td>{}{}</td><td>{}</td><td>{} s</td><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    esc(&e.id),
                    e.wide_area.as_deref().map(|w| format!(" <span class=\"mut\">({})</span>", esc(w))).unwrap_or_default(),
                    e.start_g,
                    e.duration_s,
                    st(e.worst),
                    esc(&e.basis),
                    if e.corroborated { "" } else if e.basis == "feed" { " (uncorroborated)" } else { "" },
                    esc(&e.sources.join(", ")),
                    esc(&dets.join("; ")),
                    esc(&ch.join(" → "))
                ));
            }
            h.push_str("</table></div>");
        }
        h.push_str("</div>");
    }
    h.push_str(&format!(
        "<p class=\"mut\">{}</p></main></body></html>\n",
        esc(&p.label)
    ));
    h
}

// ------------------------------------------------------------------ scenario kind

/// A `pro-area-picture` scenario: the path of a console file.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaPictureScenario {
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Path of the console file (relative to the working directory).
    pub console: String,
}

impl AreaPictureScenario {
    pub const KIND: &'static str = KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Area console: fuse Stations, receivers and customer-supplied third-party feeds into an area trust picture with event timelines and wide-area events; detections and areas only, never emitter positions.",
            required_fields: &["console"],
            optional_fields: &["title"],
        }
    }
}

impl Scenario for AreaPictureScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let inputs = Inputs::load(Path::new(&self.console)).map_err(KshanaError::InvalidInput)?;
        let p = picture(&inputs);
        let summary = format!(
            "Kshana Pro area console | {} areas, {} events, {} wide-area | emitter positions off ({} withheld)",
            p.areas.len(),
            p.events().len(),
            p.wide_area.len(),
            p.positions.withheld
        );
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"72\" font-family=\"system-ui,sans-serif\"><rect width=\"480\" height=\"72\" fill=\"#0f1216\"/><text x=\"16\" y=\"30\" fill=\"#8cc4f5\" font-size=\"15\" font-weight=\"bold\">Area console · {} areas</text><text x=\"16\" y=\"54\" fill=\"#9aa3af\" font-size=\"12\">{} events · {} wide-area · positions off</text></svg>",
            p.areas.len(),
            p.events().len(),
            p.wide_area.len()
        );
        let mut csv = String::from("event,area,tenant,start_g,end_g,duration_s,worst,basis,corroborated,sources,wide_area\n");
        for e in p.events() {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                e.id,
                e.area,
                e.tenant,
                e.start_g,
                e.end_g,
                e.duration_s,
                e.worst.as_str(),
                e.basis,
                e.corroborated,
                e.sources.join(" "),
                e.wide_area.clone().unwrap_or_default()
            ));
        }
        Ok(RunOutput {
            json: serde_json::to_string(&p).expect("the picture is plain data"),
            svg,
            summary,
            csv: Some(csv),
        })
    }
}

impl ExternalPack for AreaPictureScenario {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(KIND));
    }
}
