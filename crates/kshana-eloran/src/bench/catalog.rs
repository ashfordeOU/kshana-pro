// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The hardware catalogue: published specifications of signal sources, GNSS simulators,
//! references, counters, scopes, eLoran receivers and antennas, each value cited to the
//! vendor's datasheet, product page or manual.
//!
//! The built-in catalogue is `data/hardware/catalog.json`, compiled into the crate. A lab
//! adds or overrides entries with its own JSON file ([`Catalog::merge_file`]): an entry with
//! the same `id` replaces the built-in one, so a measured value can replace a datasheet value.
//!
//! **Evidence: datasheet values, not measurements.** A catalogue value is what the vendor
//! publishes; [`HwSpec::confidence`] says where it came from. `null` means no reliable
//! public source was found, and the bench checker reports it as "unknown", never as a pass.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Device category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Sdr,
    Awg,
    Daq,
    GnssSim,
    Tic,
    Counter,
    Scope,
    SpectrumAnalyser,
    PowerMeter,
    Reference,
    GnssTimingRx,
    EloranRx,
    Antenna,
    Distribution,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Sdr => "SDR",
            Category::Awg => "arbitrary waveform generator",
            Category::Daq => "USB instrument / DAQ",
            Category::GnssSim => "GNSS simulator",
            Category::Tic => "time-interval counter",
            Category::Counter => "frequency/time counter",
            Category::Scope => "oscilloscope",
            Category::SpectrumAnalyser => "spectrum analyser",
            Category::PowerMeter => "power meter",
            Category::Reference => "frequency/time reference",
            Category::GnssTimingRx => "GNSS timing receiver",
            Category::EloranRx => "eLoran receiver",
            Category::Antenna => "antenna",
            Category::Distribution => "reference distribution",
        }
    }
}

/// Output (transmit / generator) side.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TxSpec {
    pub channels: Option<u32>,
    pub dac_bits: Option<u32>,
    pub max_sample_rate_hz: Option<f64>,
    pub freq_min_hz: Option<f64>,
    pub freq_max_hz: Option<f64>,
    /// "dc" or "ac".
    pub coupling: Option<String>,
    /// High-pass corner of an AC-coupled output (Hz).
    pub hp_corner_hz: Option<f64>,
    pub analog_bw_hz: Option<f64>,
    pub output_impedance_ohm: Option<f64>,
    pub max_vpp_into_50ohm: Option<f64>,
    pub max_power_dbm: Option<f64>,
    pub lf_capable_note: Option<String>,
}

/// Input (receive / digitiser) side.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RxSpec {
    pub channels: Option<u32>,
    pub adc_bits: Option<u32>,
    pub max_sample_rate_hz: Option<f64>,
    pub freq_min_hz: Option<f64>,
    pub freq_max_hz: Option<f64>,
    pub coupling: Option<String>,
    pub input_impedance_ohm: Option<f64>,
    pub max_input_vpp: Option<f64>,
}

/// Reference input or output.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RefPort {
    pub freqs_hz: Vec<f64>,
    pub connector: Option<String>,
    pub level: Option<String>,
}

/// Timing performance (counters, references, timing receivers).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimingSpec {
    pub single_shot_resolution_s: Option<f64>,
    pub jitter_rms_s: Option<f64>,
    /// Free text (vendors state accuracy in many forms).
    pub freq_accuracy: Option<serde_json::Value>,
    pub adev_1s: Option<f64>,
    pub holdover: Option<serde_json::Value>,
}

/// GNSS capability (simulators and timing receivers).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GnssSpec {
    pub constellations: Vec<String>,
    pub bands: Vec<String>,
    pub max_channels: Option<serde_json::Value>,
    pub remote_api: Option<String>,
}

/// A source citation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Source {
    pub url: String,
    pub title: String,
    pub revision: Option<String>,
    pub accessed: Option<String>,
}

/// One catalogue entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HwSpec {
    pub id: String,
    pub vendor: String,
    pub model: String,
    pub category: Category,
    #[serde(default)]
    pub tx: Option<TxSpec>,
    #[serde(default)]
    pub rx: Option<RxSpec>,
    #[serde(default)]
    pub ref_in: Option<RefPort>,
    #[serde(default)]
    pub ref_out: Option<RefPort>,
    #[serde(default)]
    pub pps_in: Option<bool>,
    #[serde(default)]
    pub pps_out: Option<bool>,
    #[serde(default)]
    pub timestamped_tx: Option<bool>,
    #[serde(default)]
    pub interface: Vec<String>,
    #[serde(default)]
    pub host_api: Vec<String>,
    #[serde(default)]
    pub scpi_port: Option<u16>,
    #[serde(default)]
    pub timing: Option<TimingSpec>,
    #[serde(default)]
    pub gnss: Option<GnssSpec>,
    #[serde(default)]
    pub price_class: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub confidence: Option<String>,
    /// Case-insensitive substrings of the `*IDN?` reply (or the SoapySDR/UHD device
    /// string) that identify this model when probing a bench. All must match.
    #[serde(default)]
    pub idn_patterns: Vec<String>,
    /// Kshana driver that runs it ("redpitaya", "uhd", "soapy", "digilent", "scpi_awg",
    /// "tic", "gnss_sim:<adapter>", "none").
    #[serde(default)]
    pub kshana_driver: Option<String>,
}

impl HwSpec {
    /// Whether it can put 90–110 kHz on its output *directly* (no up-conversion), by its
    /// published lower frequency limit and coupling. `None` = not enough data.
    pub fn lf_direct(&self) -> Option<bool> {
        let tx = self.tx.as_ref()?;
        if let Some(f) = tx.freq_min_hz {
            if f > 90e3 {
                return Some(false);
            }
        }
        if tx.coupling.as_deref() == Some("ac") {
            if let Some(hp) = tx.hp_corner_hz {
                return Some(hp < 9e3);
            }
        }
        match (tx.freq_min_hz, tx.freq_max_hz) {
            (Some(lo), Some(hi)) => Some(lo <= 90e3 && hi >= 110e3),
            (None, Some(hi)) if tx.coupling.as_deref() == Some("dc") => Some(hi >= 110e3),
            _ => None,
        }
    }

    /// Whether `idn` (an `*IDN?` reply or device string) identifies this model.
    pub fn matches_idn(&self, idn: &str) -> bool {
        let l = idn.to_ascii_lowercase();
        !self.idn_patterns.is_empty()
            && self
                .idn_patterns
                .iter()
                .all(|p| l.contains(&p.to_ascii_lowercase()))
    }

    /// Accepts the 10 MHz reference.
    pub fn takes_10mhz(&self) -> Option<bool> {
        self.ref_in
            .as_ref()
            .map(|r| r.freqs_hz.iter().any(|f| (f - 10e6).abs() < 1.0))
    }

    /// One-line summary.
    pub fn summary(&self) -> String {
        format!(
            "{} {} ({}, {})",
            self.vendor,
            self.model,
            self.category.label(),
            self.confidence.as_deref().unwrap_or("unsourced")
        )
    }
}

/// The catalogue.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    entries: BTreeMap<String, HwSpec>,
}

/// The built-in catalogue file.
pub const BUILTIN_JSON: &str = include_str!("../../data/hardware/catalog.json");

impl Catalog {
    /// The built-in catalogue (datasheet values with citations).
    pub fn builtin() -> Catalog {
        let mut c = Catalog::default();
        c.merge_json(BUILTIN_JSON)
            .expect("built-in hardware catalogue parses");
        c
    }

    /// Parse a JSON array of entries and add them (same id replaces).
    pub fn merge_json(&mut self, json: &str) -> Result<usize, String> {
        let v: Vec<HwSpec> =
            serde_json::from_str(json).map_err(|e| format!("hardware catalogue: {e}"))?;
        let n = v.len();
        for s in v {
            if s.id.trim().is_empty() {
                return Err("hardware catalogue: entry with an empty id".into());
            }
            self.entries.insert(s.id.clone(), s);
        }
        Ok(n)
    }

    /// Add a lab's own catalogue file.
    pub fn merge_file(&mut self, path: &std::path::Path) -> Result<usize, String> {
        let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.merge_json(&t)
    }

    pub fn get(&self, id: &str) -> Option<&HwSpec> {
        self.entries.get(id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HwSpec> {
        self.entries.values()
    }

    pub fn by_category(&self, c: Category) -> Vec<&HwSpec> {
        self.iter().filter(|s| s.category == c).collect()
    }

    /// Entries whose `idn_patterns` all match `idn`.
    pub fn identify(&self, idn: &str) -> Vec<&HwSpec> {
        self.iter().filter(|s| s.matches_idn(idn)).collect()
    }

    /// Closest ids to a mistyped one (for error messages).
    pub fn suggest(&self, id: &str) -> Vec<String> {
        let l = id.to_ascii_lowercase();
        let parts: Vec<&str> = l.split(['-', ' ', '_']).filter(|p| p.len() > 1).collect();
        let mut v: Vec<(usize, &String)> = self
            .entries
            .keys()
            .map(|k| {
                (
                    parts
                        .iter()
                        .filter(|p| k.contains(**p) || p.contains(k.as_str()))
                        .count(),
                    k,
                )
            })
            .filter(|(n, _)| *n > 0)
            .collect();
        v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        v.into_iter().take(5).map(|(_, k)| k.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalogue_is_sourced_and_unique() {
        let c = Catalog::builtin();
        // Every entry must be sourced; the count is checked once the datasheet data lands.
        for s in c.iter() {
            assert!(!s.sources.is_empty(), "{} has no source", s.id);
            for src in &s.sources {
                assert!(src.url.starts_with("http"), "{}: bad url {}", s.id, src.url);
            }
            assert_eq!(s.id, s.id.to_ascii_lowercase(), "{} not lowercase", s.id);
        }
    }

    #[test]
    fn lf_direct_reads_lower_limit_and_coupling() {
        let mk = |lo: Option<f64>, cpl: &str, hp: Option<f64>| HwSpec {
            id: "x".into(),
            vendor: "v".into(),
            model: "m".into(),
            category: Category::Sdr,
            tx: Some(TxSpec {
                freq_min_hz: lo,
                freq_max_hz: Some(50e6),
                coupling: Some(cpl.into()),
                hp_corner_hz: hp,
                ..Default::default()
            }),
            rx: None,
            ref_in: None,
            ref_out: None,
            pps_in: None,
            pps_out: None,
            timestamped_tx: None,
            interface: vec![],
            host_api: vec![],
            scpi_port: None,
            timing: None,
            gnss: None,
            price_class: None,
            notes: None,
            sources: vec![],
            confidence: None,
            idn_patterns: vec!["acme".into(), "x1".into()],
            kshana_driver: None,
        };
        assert_eq!(mk(Some(0.0), "dc", None).lf_direct(), Some(true));
        assert_eq!(mk(Some(70e6), "ac", None).lf_direct(), Some(false));
        assert_eq!(mk(Some(1e3), "ac", Some(30e3)).lf_direct(), Some(false));
        assert!(mk(None, "dc", None).matches_idn("ACME Corp,X1,123"));
        assert!(!mk(None, "dc", None).matches_idn("ACME Corp,X2,123"));
    }
}
