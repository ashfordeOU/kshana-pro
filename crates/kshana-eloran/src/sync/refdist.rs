// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Timing-reference distribution model: where each device's 10 MHz and 1PPS come from
//! and how late they arrive.
//!
//! A [`RefDistConfig`] (TOML) lists sources (GPSDO, Rb, house master, external),
//! distribution amplifiers, devices (eLoran simulator, GNSS simulator, receivers,
//! counters) and the cables between them, each cable carrying `pps`, `10mhz` or `both`.
//! [`RefDistConfig::analyse`] walks every device's inputs upstream to their root source
//! and reports, per signal, the path, the delay relative to the master's time (sum of
//! source output, cable, amplifier and device input delays) and its standard uncertainty
//! (RSS), plus warnings for missing references, devices not tied to the master, and
//! devices whose 10 MHz and 1PPS come from different roots. [`RefDistReport::skew`] gives
//! the delay between two devices with the shared upstream path cancelled, and
//! [`RefDistReport::cable_table`] the cable-delay table.
//!
//! Cable delay: `τ = L / (v_f · c)`; `u² = (u_L / (v_f c))² + (L · u_vf / (v_f² c))²`, or a
//! measured delay and uncertainty when given.
//!
//! ```toml
//! master = "gpsdo"
//! reference_device = "eloran_sim"
//! [[sources]]
//! id = "gpsdo"
//! kind = "gpsdo"
//! [[amplifiers]]
//! id = "da1"
//! delay_ns = 2.5
//! u_ns = 0.2
//! [[devices]]
//! id = "eloran_sim"
//! kind = "simulator"
//! [[cables]]
//! id = "c1"
//! from = "gpsdo"
//! to = "da1"
//! signal = "both"
//! length_m = 1.0
//! velocity_factor = 0.66
//! length_u_m = 0.005
//! velocity_factor_u = 0.003
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Speed of light (m/s).
pub const C_M_PER_S: f64 = 299_792_458.0;

/// A reference signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RefSignal {
    #[serde(rename = "10mhz")]
    TenMhz,
    #[serde(rename = "pps")]
    Pps,
}

impl RefSignal {
    pub fn key(self) -> &'static str {
        match self {
            RefSignal::TenMhz => "10mhz",
            RefSignal::Pps => "pps",
        }
    }
}

/// Signals carried by a cable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CableSignal {
    #[serde(rename = "10mhz")]
    TenMhz,
    #[serde(rename = "pps")]
    #[default]
    Pps,
    /// Both (two matched cables, or a combined distribution).
    Both,
}

impl CableSignal {
    pub fn carries(self, s: RefSignal) -> bool {
        matches!(
            (self, s),
            (CableSignal::Both, _)
                | (CableSignal::Pps, RefSignal::Pps)
                | (CableSignal::TenMhz, RefSignal::TenMhz)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Gpsdo,
    Rubidium,
    Cesium,
    HouseMaster,
    External,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Simulator,
    GnssSimulator,
    Receiver,
    Counter,
    Other,
}

fn d_both() -> Vec<RefSignal> {
    vec![RefSignal::TenMhz, RefSignal::Pps]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub kind: SourceKind,
    /// Delay from the source's time scale (or its disciplining input) to its outputs (ns).
    #[serde(default)]
    pub output_delay_ns: f64,
    #[serde(default)]
    pub output_u_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Amplifier {
    pub id: String,
    /// Input-to-output delay (ns) and its standard uncertainty (incl. channel skew).
    #[serde(default)]
    pub delay_ns: f64,
    #[serde(default)]
    pub u_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub kind: DeviceKind,
    /// Signals the device needs (default both).
    #[serde(default = "d_both")]
    pub requires: Vec<RefSignal>,
    /// Internal delay from the reference input to the device's timing plane (ns).
    #[serde(default)]
    pub input_delay_ns: f64,
    #[serde(default)]
    pub input_u_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cable {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub signal: CableSignal,
    #[serde(default)]
    pub length_m: f64,
    #[serde(default = "d_vf")]
    pub velocity_factor: f64,
    #[serde(default)]
    pub length_u_m: f64,
    #[serde(default)]
    pub velocity_factor_u: f64,
    /// Measured delay (ns) overriding the length model.
    #[serde(default)]
    pub measured_delay_ns: Option<f64>,
    #[serde(default)]
    pub measured_u_ns: Option<f64>,
}

fn d_vf() -> f64 {
    0.66
}

impl Cable {
    /// (delay ns, standard uncertainty ns).
    pub fn delay(&self) -> (f64, f64) {
        if let Some(d) = self.measured_delay_ns {
            return (d, self.measured_u_ns.unwrap_or(0.0));
        }
        let v = self.velocity_factor * C_M_PER_S;
        let d = self.length_m / v * 1e9;
        let a = self.length_u_m / v * 1e9;
        let b = self.length_m * self.velocity_factor_u
            / (self.velocity_factor * self.velocity_factor * C_M_PER_S)
            * 1e9;
        (d, (a * a + b * b).sqrt())
    }
}

/// The distribution configuration (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefDistConfig {
    /// Id of the master source.
    pub master: String,
    /// Device the skews in the report are referred to.
    #[serde(default)]
    pub reference_device: Option<String>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub amplifiers: Vec<Amplifier>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub cables: Vec<Cable>,
}

/// One delay component along a path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathElement {
    /// "source:<id>", "cable:<id>", "amplifier:<id>", "device:<id>".
    pub element: String,
    pub delay_ns: f64,
    pub u_ns: f64,
}

/// A device's reference for one signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceReference {
    pub device: String,
    pub signal: RefSignal,
    /// Root source reached (`None` when the input is missing or dangling).
    pub root: Option<String>,
    /// From the root to the device.
    pub path: Vec<PathElement>,
    pub delay_ns: f64,
    pub u_ns: f64,
}

/// A cable-table row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CableRow {
    pub id: String,
    pub from: String,
    pub to: String,
    pub signal: CableSignal,
    pub length_m: f64,
    pub velocity_factor: f64,
    pub delay_ns: f64,
    pub u_ns: f64,
    pub measured: bool,
}

/// Skew between two devices (`a − b`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skew {
    pub a: String,
    pub b: String,
    pub signal: RefSignal,
    pub delay_ns: f64,
    /// Uncertainty of the non-shared path elements only.
    pub u_ns: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefDistReport {
    pub master: String,
    pub references: Vec<DeviceReference>,
    /// Skews of every device against `reference_device`.
    pub skews: Vec<Skew>,
    pub warnings: Vec<String>,
}

impl RefDistReport {
    pub fn get(&self, device: &str, s: RefSignal) -> Option<&DeviceReference> {
        self.references
            .iter()
            .find(|r| r.device == device && r.signal == s)
    }
    /// Delay of `a` minus `b` for signal `s`, shared upstream elements cancelled; `None`
    /// when either is missing or they have different roots.
    pub fn skew(&self, a: &str, b: &str, s: RefSignal) -> Option<Skew> {
        let (ra, rb) = (self.get(a, s)?, self.get(b, s)?);
        if ra.root.is_none() || ra.root != rb.root {
            return None;
        }
        let common = ra
            .path
            .iter()
            .zip(&rb.path)
            .take_while(|(x, y)| x.element == y.element)
            .count();
        let rest = |r: &DeviceReference| -> (f64, f64) {
            r.path[common..].iter().fold((0.0, 0.0), |(d, u), e| {
                (d + e.delay_ns, u + e.u_ns * e.u_ns)
            })
        };
        let (da, ua) = rest(ra);
        let (db, ub) = rest(rb);
        Some(Skew {
            a: a.into(),
            b: b.into(),
            signal: s,
            delay_ns: da - db,
            u_ns: (ua + ub).sqrt(),
        })
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
}

#[derive(Clone, Copy)]
enum NodeRef<'a> {
    Source(&'a Source),
    Amp(&'a Amplifier),
    // kept for a complete node map (devices are leaves; their data is not read)
    #[allow(dead_code)]
    Device(&'a Device),
}

impl RefDistConfig {
    pub fn from_toml(src: &str) -> Result<RefDistConfig, String> {
        let c: RefDistConfig = toml::from_str(src).map_err(|e| format!("refdist: {e}"))?;
        c.validate()?;
        Ok(c)
    }
    fn nodes(&self) -> BTreeMap<&str, NodeRef<'_>> {
        let mut m = BTreeMap::new();
        for s in &self.sources {
            m.insert(s.id.as_str(), NodeRef::Source(s));
        }
        for a in &self.amplifiers {
            m.insert(a.id.as_str(), NodeRef::Amp(a));
        }
        for d in &self.devices {
            m.insert(d.id.as_str(), NodeRef::Device(d));
        }
        m
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        let all = self
            .sources
            .iter()
            .map(|s| &s.id)
            .chain(self.amplifiers.iter().map(|a| &a.id))
            .chain(self.devices.iter().map(|d| &d.id));
        for id in all {
            if !ids.insert(id.as_str()) {
                return Err(format!("refdist: duplicate node id {id:?}"));
            }
        }
        if !self.sources.iter().any(|s| s.id == self.master) {
            return Err(format!("refdist: master {:?} is not a source", self.master));
        }
        let mut cids = BTreeSet::new();
        for c in &self.cables {
            if !cids.insert(c.id.as_str()) {
                return Err(format!("refdist: duplicate cable id {:?}", c.id));
            }
            for n in [&c.from, &c.to] {
                if !ids.contains(n.as_str()) {
                    return Err(format!(
                        "refdist: cable {} refers to unknown node {n:?}",
                        c.id
                    ));
                }
            }
            if self.devices.iter().any(|d| d.id == c.from) {
                return Err(format!(
                    "refdist: cable {} starts at device {:?} (devices do not redistribute)",
                    c.id, c.from
                ));
            }
            if c.measured_delay_ns.is_none()
                && (!(c.velocity_factor > 0.0 && c.velocity_factor <= 1.0) || c.length_m < 0.0)
            {
                return Err(format!(
                    "refdist: cable {} needs length_m >= 0 and 0 < velocity_factor <= 1",
                    c.id
                ));
            }
        }
        for s in [RefSignal::TenMhz, RefSignal::Pps] {
            for id in &ids {
                let n = self
                    .cables
                    .iter()
                    .filter(|c| c.to == *id && c.signal.carries(s))
                    .count();
                if n > 1 {
                    return Err(format!(
                        "refdist: node {id:?} has {n} {} inputs (one expected)",
                        s.key()
                    ));
                }
            }
        }
        if let Some(r) = &self.reference_device {
            if !self.devices.iter().any(|d| &d.id == r) {
                return Err(format!("refdist: reference_device {r:?} is not a device"));
            }
        }
        Ok(())
    }

    /// Trace node `id` for signal `s` back to its root: path from root to `id` (excluding
    /// the end device's own input delay) and the root id.
    fn trace(&self, id: &str, s: RefSignal) -> Result<(Vec<PathElement>, Option<String>), String> {
        let nodes = self.nodes();
        let mut rev = Vec::new();
        let mut seen = BTreeSet::new();
        let mut cur = id.to_string();
        loop {
            if !seen.insert(cur.clone()) {
                return Err(format!("refdist: {} loop through {cur:?}", s.key()));
            }
            let node = nodes[cur.as_str()];
            if cur != id {
                match node {
                    NodeRef::Amp(a) => rev.push(PathElement {
                        element: format!("amplifier:{}", a.id),
                        delay_ns: a.delay_ns,
                        u_ns: a.u_ns,
                    }),
                    NodeRef::Source(src) => rev.push(PathElement {
                        element: format!("source:{}", src.id),
                        delay_ns: src.output_delay_ns,
                        u_ns: src.output_u_ns,
                    }),
                    NodeRef::Device(_) => unreachable!("validated: devices have no outputs"),
                }
            }
            let input = self
                .cables
                .iter()
                .find(|c| c.to == cur && c.signal.carries(s));
            match input {
                Some(c) => {
                    let (d, u) = c.delay();
                    rev.push(PathElement {
                        element: format!("cable:{}", c.id),
                        delay_ns: d,
                        u_ns: u,
                    });
                    cur = c.from.clone();
                }
                None => {
                    rev.reverse();
                    let root = match node {
                        NodeRef::Source(_) if cur != id => Some(cur),
                        _ => None,
                    };
                    return Ok((rev, root));
                }
            }
        }
    }

    /// Compute every device's references and the warnings.
    pub fn analyse(&self) -> Result<RefDistReport, String> {
        self.validate()?;
        let mut refs = Vec::new();
        let mut warnings = Vec::new();
        for d in &self.devices {
            let mut roots: BTreeMap<RefSignal, Option<String>> = BTreeMap::new();
            for &s in &d.requires {
                let (mut path, root) = self.trace(&d.id, s)?;
                path.push(PathElement {
                    element: format!("device:{}", d.id),
                    delay_ns: d.input_delay_ns,
                    u_ns: d.input_u_ns,
                });
                match &root {
                    None if path.len() == 1 => warnings.push(format!(
                        "device {} has no {} reference input",
                        d.id,
                        s.key()
                    )),
                    None => warnings.push(format!(
                        "device {} {} input does not reach a source (dangling amplifier)",
                        d.id,
                        s.key()
                    )),
                    Some(r) if *r != self.master => warnings.push(format!(
                        "device {} {} is referenced to {r}, not the master {}: no shared reference",
                        d.id,
                        s.key(),
                        self.master
                    )),
                    _ => {}
                }
                roots.insert(s, root.clone());
                let delay = path.iter().map(|e| e.delay_ns).sum();
                let u = path.iter().map(|e| e.u_ns * e.u_ns).sum::<f64>().sqrt();
                refs.push(DeviceReference {
                    device: d.id.clone(),
                    signal: s,
                    root,
                    path,
                    delay_ns: delay,
                    u_ns: u,
                });
            }
            if let (Some(Some(a)), Some(Some(b))) =
                (roots.get(&RefSignal::TenMhz), roots.get(&RefSignal::Pps))
            {
                if a != b {
                    warnings.push(format!(
                        "device {}: 10 MHz from {a} but 1PPS from {b}",
                        d.id
                    ));
                }
            }
        }
        // simulators must share one root
        let sims: Vec<&DeviceReference> = refs
            .iter()
            .filter(|r| {
                self.devices.iter().any(|d| {
                    d.id == r.device
                        && matches!(d.kind, DeviceKind::Simulator | DeviceKind::GnssSimulator)
                })
            })
            .collect();
        for s in [RefSignal::TenMhz, RefSignal::Pps] {
            let rs: BTreeSet<Option<&String>> = sims
                .iter()
                .filter(|r| r.signal == s)
                .map(|r| r.root.as_ref())
                .collect();
            if rs.len() > 1 {
                warnings.push(format!(
                    "eLoran and GNSS simulators do not share one {} reference",
                    s.key()
                ));
            }
        }
        let used: BTreeSet<&str> = self
            .cables
            .iter()
            .flat_map(|c| [c.from.as_str(), c.to.as_str()])
            .collect();
        for s in &self.sources {
            if !used.contains(s.id.as_str()) {
                warnings.push(format!("source {} is not connected", s.id));
            }
        }
        let mut rep = RefDistReport {
            master: self.master.clone(),
            references: refs,
            skews: vec![],
            warnings,
        };
        if let Some(rd) = &self.reference_device {
            let mut sk = Vec::new();
            for r in &rep.references {
                if r.device != *rd {
                    if let Some(s) = rep.skew(&r.device, rd, r.signal) {
                        sk.push(s);
                    }
                }
            }
            rep.skews = sk;
        }
        Ok(rep)
    }

    /// The cable-delay table.
    pub fn cable_table(&self) -> Vec<CableRow> {
        self.cables
            .iter()
            .map(|c| {
                let (d, u) = c.delay();
                CableRow {
                    id: c.id.clone(),
                    from: c.from.clone(),
                    to: c.to.clone(),
                    signal: c.signal,
                    length_m: c.length_m,
                    velocity_factor: c.velocity_factor,
                    delay_ns: d,
                    u_ns: u,
                    measured: c.measured_delay_ns.is_some(),
                }
            })
            .collect()
    }

    /// The cable-delay table as CSV.
    pub fn cable_table_csv(&self) -> String {
        let mut s =
            String::from("id,from,to,signal,length_m,velocity_factor,delay_ns,u_ns,measured\n");
        for r in self.cable_table() {
            s.push_str(&format!(
                "{},{},{},{},{:.3},{:.4},{:.4},{:.4},{}\n",
                r.id,
                r.from,
                r.to,
                match r.signal {
                    CableSignal::Both => "both",
                    CableSignal::Pps => "pps",
                    CableSignal::TenMhz => "10mhz",
                },
                r.length_m,
                r.velocity_factor,
                r.delay_ns,
                r.u_ns,
                r.measured
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = r#"
master = "gpsdo"
reference_device = "eloran_sim"

[[sources]]
id = "gpsdo"
kind = "gpsdo"
output_delay_ns = 0.0
output_u_ns = 0.0

[[sources]]
id = "rb"
kind = "rubidium"

[[amplifiers]]
id = "da1"
delay_ns = 2.5
u_ns = 0.2

[[devices]]
id = "eloran_sim"
kind = "simulator"
input_delay_ns = 1.0
input_u_ns = 0.1

[[devices]]
id = "gnss_sim"
kind = "gnss_simulator"

[[devices]]
id = "tic"
kind = "counter"
requires = ["10mhz"]

[[devices]]
id = "rx"
kind = "receiver"
requires = ["pps"]

[[cables]]
id = "c0"
from = "gpsdo"
to = "da1"
signal = "both"
length_m = 1.0
velocity_factor = 0.66
length_u_m = 0.01
velocity_factor_u = 0.005

[[cables]]
id = "c1"
from = "da1"
to = "eloran_sim"
signal = "both"
length_m = 2.0
velocity_factor = 0.66

[[cables]]
id = "c2"
from = "da1"
to = "gnss_sim"
signal = "pps"
measured_delay_ns = 15.0
measured_u_ns = 0.05

[[cables]]
id = "c3"
from = "rb"
to = "gnss_sim"
signal = "10mhz"
length_m = 1.0

[[cables]]
id = "c4"
from = "da1"
to = "tic"
signal = "10mhz"
length_m = 0.5
velocity_factor = 0.7
"#;

    #[test]
    fn cable_delay_model() {
        let c = Cable {
            id: "x".into(),
            from: "a".into(),
            to: "b".into(),
            signal: CableSignal::Pps,
            length_m: 10.0,
            velocity_factor: 0.66,
            length_u_m: 0.02,
            velocity_factor_u: 0.01,
            measured_delay_ns: None,
            measured_u_ns: None,
        };
        let (d, u) = c.delay();
        assert!((d - 50.5398).abs() < 1e-3, "{d}");
        let a = 0.02 / (0.66 * C_M_PER_S) * 1e9;
        let b = d * 0.01 / 0.66;
        assert!((u - (a * a + b * b).sqrt()).abs() < 1e-9);
    }

    #[test]
    fn delays_paths_and_warnings() {
        let cfg = RefDistConfig::from_toml(CFG).unwrap();
        let r = cfg.analyse().unwrap();
        let e = r.get("eloran_sim", RefSignal::Pps).unwrap();
        let c0 = 1.0 / (0.66 * C_M_PER_S) * 1e9;
        assert_eq!(e.root.as_deref(), Some("gpsdo"));
        let names: Vec<&str> = e.path.iter().map(|p| p.element.as_str()).collect();
        assert_eq!(
            names,
            [
                "source:gpsdo",
                "cable:c0",
                "amplifier:da1",
                "cable:c1",
                "device:eloran_sim"
            ]
        );
        assert!((e.delay_ns - (3.0 * c0 + 2.5 + 1.0)).abs() < 1e-9);
        let g = r.get("gnss_sim", RefSignal::Pps).unwrap();
        assert!((g.delay_ns - (c0 + 2.5 + 15.0)).abs() < 1e-9);
        // skew with the shared c0 and da1 cancelled
        let sk = r.skew("gnss_sim", "eloran_sim", RefSignal::Pps).unwrap();
        assert!((sk.delay_ns - (15.0 - 2.0 * c0 - 1.0)).abs() < 1e-9);
        assert!((sk.u_ns - (0.05f64.powi(2) + 0.1f64.powi(2)).sqrt()).abs() < 1e-12);
        assert!(r
            .skews
            .iter()
            .any(|s| s.a == "gnss_sim" && s.signal == RefSignal::Pps));
        let w = r.warnings.join("\n");
        assert!(w.contains("gnss_sim 10mhz is referenced to rb"), "{w}");
        assert!(w.contains("10 MHz from rb but 1PPS from gpsdo"), "{w}");
        assert!(w.contains("do not share one 10mhz"), "{w}");
        assert!(w.contains("rx has no pps reference input"), "{w}");
        assert!(!w.contains("share one pps"), "{w}");
        // no skew between different roots
        assert!(r
            .skew("gnss_sim", "eloran_sim", RefSignal::TenMhz)
            .is_none());
        let csv = cfg.cable_table_csv();
        assert_eq!(csv.lines().count(), 6);
        assert!(
            csv.contains("c2,da1,gnss_sim,pps,0.000,0.6600,15.0000,0.0500,true"),
            "{csv}"
        );
        assert!(serde_json::from_str::<serde_json::Value>(&r.to_json()).is_ok());
    }

    #[test]
    fn rejects_invalid_graphs() {
        assert!(
            RefDistConfig::from_toml(&CFG.replace("master = \"gpsdo\"", "master = \"da1\""))
                .is_err()
        );
        assert!(
            RefDistConfig::from_toml(&CFG.replace("to = \"tic\"", "to = \"nowhere\"")).is_err()
        );
        // two PPS inputs into gnss_sim
        assert!(RefDistConfig::from_toml(&CFG.replace(
            "signal = \"10mhz\"\nlength_m = 1.0",
            "signal = \"pps\"\nlength_m = 1.0"
        ))
        .is_err());
        // loop between two amplifiers
        let lp = r#"
master = "m"
[[sources]]
id = "m"
kind = "house_master"
[[amplifiers]]
id = "a"
[[amplifiers]]
id = "b"
[[devices]]
id = "d"
kind = "other"
requires = ["pps"]
[[cables]]
id = "x"
from = "a"
to = "b"
[[cables]]
id = "y"
from = "b"
to = "a"
[[cables]]
id = "z"
from = "b"
to = "d"
"#;
        let c = RefDistConfig::from_toml(lp).unwrap();
        assert!(c.analyse().unwrap_err().contains("loop"));
    }
}
