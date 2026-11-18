// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Bench readiness: does a set of hardware (an inventory of catalogue entries in roles)
//! meet what the simulator and its acceptance suite need? Every check cites the catalogue
//! value it used and says Pass, Warn, Fail or Unknown (no published value).
//!
//! Inventory (TOML):
//!
//! ```toml
//! name = "Lab bench A"
//!
//! [requirements]          # optional; defaults shown by `BenchRequirements::default()`
//! tx_channels = 1
//! need_gnss = true
//!
//! [[device]]
//! catalog = "redpitaya-stemlab-125-14"
//! role = "eloran_source"
//! host = "192.168.1.50"   # optional: where the probe and the generated profiles find it
//!
//! [[device]]
//! catalog = "tapr-ticc"
//! role = "tic"
//! serial = "/dev/ttyACM0"
//! ```

use super::catalog::{Catalog, Category, HwSpec};
use serde::{Deserialize, Serialize};

/// What a device does on the bench.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchRole {
    /// Generates the eLoran signal (100 kHz pulses) into the receiver input or antenna simulator.
    EloranSource,
    /// GNSS RF (a GNSS simulator, or an SDR running a GNSS signal generator).
    GnssSource,
    /// 10 MHz and 1PPS reference for everything.
    Reference,
    /// Time-interval counter (PPS-to-PPS, reference point to GNSS PPS).
    Tic,
    /// Frequency counter (reference-lock test).
    Counter,
    /// Spectrum analyser (dynamic range, spurious, isolation).
    Spectrum,
    /// Power meter (linearity).
    Power,
    /// Oscilloscope or digitiser (waveform capture, loopback calibration).
    Digitiser,
    /// The eLoran receiver under test.
    EloranRx,
    Antenna,
    /// Reference distribution amplifier.
    Distribution,
}

impl BenchRole {
    pub fn label(&self) -> &'static str {
        match self {
            BenchRole::EloranSource => "eLoran signal source",
            BenchRole::GnssSource => "GNSS signal source",
            BenchRole::Reference => "10 MHz / 1PPS reference",
            BenchRole::Tic => "time-interval counter",
            BenchRole::Counter => "frequency counter",
            BenchRole::Spectrum => "spectrum analyser",
            BenchRole::Power => "power meter",
            BenchRole::Digitiser => "oscilloscope / digitiser",
            BenchRole::EloranRx => "eLoran receiver (device under test)",
            BenchRole::Antenna => "antenna / antenna simulator",
            BenchRole::Distribution => "reference distribution",
        }
    }
}

/// One inventory line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InventoryItem {
    /// Catalogue id.
    pub catalog: String,
    pub role: BenchRole,
    /// Label on the bench (defaults to the catalogue id).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub serial: Option<String>,
    /// Device arguments for SoapySDR/UHD (e.g. "type=b200,serial=…").
    #[serde(default)]
    pub args: Option<String>,
}

impl InventoryItem {
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.catalog.clone())
    }
}

/// What the bench must do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BenchRequirements {
    /// eLoran output channels (1 = one antenna-simulator input; 2 = crossed H-field loops).
    pub tx_channels: u32,
    /// Lowest acceptable output sample rate (Hz). The simulator's continuous-time synthesis
    /// is specified at ≥ 1 MS/s; 400 kS/s is the floor (images of the 110 kHz band edge
    /// must stay above the band after reconstruction).
    pub min_sample_rate_hz: f64,
    pub warn_sample_rate_hz: f64,
    /// DAC resolution: quantisation SNR ≈ 6.02·N + 1.76 dB over the Nyquist band.
    pub min_dac_bits: u32,
    pub warn_dac_bits: u32,
    /// GNSS on the same bench (combined eLoran/GNSS tests).
    pub need_gnss: bool,
    pub gnss_bands: Vec<String>,
    /// eLoran-to-GNSS (and to UTC) time alignment: needs a shared 10 MHz and 1PPS.
    pub need_sync: bool,
    /// Time-interval counter single-shot resolution needed (s).
    pub tic_resolution_s: f64,
    /// Lowest digitiser sample rate for waveform checks (Hz).
    pub min_digitiser_rate_hz: f64,
    /// Peak voltage the receiver input or antenna simulator expects (Vpp into 50 Ω),
    /// used for the attenuator plan. `None` = unknown (plan says "set by the DUT manual").
    pub dut_input_vpp: Option<f64>,
}

impl Default for BenchRequirements {
    fn default() -> Self {
        BenchRequirements {
            tx_channels: 1,
            min_sample_rate_hz: 400e3,
            warn_sample_rate_hz: 1e6,
            min_dac_bits: 8,
            warn_dac_bits: 12,
            need_gnss: true,
            gnss_bands: vec!["L1".into()],
            need_sync: true,
            tic_resolution_s: 1e-9,
            min_digitiser_rate_hz: 1e6,
            dut_input_vpp: None,
        }
    }
}

/// A bench.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub requirements: BenchRequirements,
    #[serde(default, rename = "device")]
    pub devices: Vec<InventoryItem>,
}

impl Inventory {
    pub fn from_toml(s: &str) -> Result<Inventory, String> {
        toml::from_str(s).map_err(|e| format!("inventory: {e}"))
    }
    pub fn load(path: &std::path::Path) -> Result<Inventory, String> {
        let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Inventory::from_toml(&t)
    }
    pub fn in_role(&self, r: BenchRole) -> Vec<&InventoryItem> {
        self.devices.iter().filter(|d| d.role == r).collect()
    }
}

/// Outcome of one check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Warn,
    /// No published value: check by hand or measure.
    Unknown,
    Fail,
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub role: BenchRole,
    /// Inventory label (empty when the role is missing).
    pub device: String,
    pub check: String,
    pub status: Status,
    pub detail: String,
    /// The catalogue source the value came from.
    #[serde(default)]
    pub source: Option<String>,
}

/// Bench assessment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assessment {
    pub bench: String,
    pub findings: Vec<Finding>,
    /// No Fail, and every required role is filled.
    pub ready: bool,
    /// Acceptance-suite tests this bench can run (test kinds), and those it cannot, with why.
    pub runnable: Vec<String>,
    pub not_runnable: Vec<(String, String)>,
    pub counts: StatusCounts,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StatusCounts {
    pub pass: usize,
    pub warn: usize,
    pub unknown: usize,
    pub fail: usize,
}

impl Assessment {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Plain-text report.
    pub fn to_text(&self) -> String {
        let mut s = format!(
            "Bench: {}\nReady: {}  (pass {}, warn {}, unknown {}, fail {})\n\n",
            if self.bench.is_empty() {
                "(unnamed)"
            } else {
                &self.bench
            },
            if self.ready { "YES" } else { "NO" },
            self.counts.pass,
            self.counts.warn,
            self.counts.unknown,
            self.counts.fail
        );
        for f in &self.findings {
            let tag = match f.status {
                Status::Pass => "PASS",
                Status::Warn => "WARN",
                Status::Unknown => "UNKNOWN",
                Status::Fail => "FAIL",
            };
            s += &format!(
                "[{tag:7}] {} / {}: {}: {}\n",
                f.role.label(),
                if f.device.is_empty() { "-" } else { &f.device },
                f.check,
                f.detail
            );
        }
        s += "\nAcceptance tests runnable on this bench:\n";
        for r in &self.runnable {
            s += &format!("  + {r}\n");
        }
        for (t, why) in &self.not_runnable {
            s += &format!("  - {t}: {why}\n");
        }
        s
    }
}

struct Ctx<'a> {
    out: Vec<Finding>,
    cat: &'a Catalog,
}

impl Ctx<'_> {
    fn add(
        &mut self,
        role: BenchRole,
        dev: &str,
        check: &str,
        st: Status,
        detail: String,
        spec: Option<&HwSpec>,
    ) {
        self.out.push(Finding {
            role,
            device: dev.to_string(),
            check: check.to_string(),
            status: st,
            detail,
            source: spec.and_then(|s| s.sources.first()).map(|s| s.url.clone()),
        });
    }
}

fn fmt_hz(f: f64) -> String {
    if f >= 1e9 {
        format!("{:.4} GHz", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.3} MHz", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.1} kHz", f / 1e3)
    } else {
        format!("{f:.0} Hz")
    }
}

/// Band centre frequencies (Hz) by name.
pub fn gnss_band_hz(band: &str) -> Option<f64> {
    Some(match band.to_ascii_uppercase().as_str() {
        "L1" | "E1" | "B1C" => 1_575.42e6,
        "L2" => 1_227.60e6,
        "L5" | "E5A" | "B2A" => 1_176.45e6,
        "E5B" | "B2B" => 1_207.14e6,
        "E6" => 1_278.75e6,
        "G1" => 1_602.0e6,
        "B1I" => 1_561.098e6,
        _ => return None,
    })
}

/// Assess `inv` against its requirements using `cat`.
pub fn assess(inv: &Inventory, cat: &Catalog) -> Assessment {
    let req = &inv.requirements;
    let mut cx = Ctx { out: vec![], cat };
    // Unknown catalogue ids first.
    for d in &inv.devices {
        if cx.cat.get(&d.catalog).is_none() {
            let sug = cx.cat.suggest(&d.catalog);
            cx.add(
                d.role,
                &d.label(),
                "catalogue entry",
                Status::Fail,
                format!(
                    "'{}' is not in the catalogue{}; add it with a lab catalogue file",
                    d.catalog,
                    if sug.is_empty() {
                        String::new()
                    } else {
                        format!(" (did you mean: {})", sug.join(", "))
                    }
                ),
                None,
            );
        }
    }
    let spec_of = |d: &InventoryItem| cat.get(&d.catalog);

    // --- eLoran source ---------------------------------------------------------------
    let srcs = inv.in_role(BenchRole::EloranSource);
    if srcs.is_empty() {
        cx.add(
            BenchRole::EloranSource,
            "",
            "present",
            Status::Fail,
            "no eLoran signal source in the inventory".into(),
            None,
        );
    }
    for d in &srcs {
        let Some(s) = spec_of(d) else { continue };
        check_eloran_source(&mut cx, req, d, s);
    }
    let tx_total: u32 = srcs
        .iter()
        .filter_map(|d| {
            spec_of(d)
                .and_then(|s| s.tx.as_ref())
                .and_then(|t| t.channels)
        })
        .sum();
    if !srcs.is_empty() {
        let st = if tx_total >= req.tx_channels {
            Status::Pass
        } else {
            Status::Fail
        };
        cx.add(
            BenchRole::EloranSource,
            "",
            "output channels",
            st,
            format!(
                "{tx_total} published output channel(s) across sources; {} needed",
                req.tx_channels
            ),
            None,
        );
    }

    // --- GNSS source -----------------------------------------------------------------
    if req.need_gnss {
        let g = inv.in_role(BenchRole::GnssSource);
        if g.is_empty() {
            cx.add(
                BenchRole::GnssSource,
                "",
                "present",
                Status::Fail,
                "GNSS is required (need_gnss) but no GNSS source is listed".into(),
                None,
            );
        }
        for d in &g {
            let Some(s) = spec_of(d) else { continue };
            check_gnss_source(&mut cx, req, d, s);
        }
    }

    // --- Reference -------------------------------------------------------------------
    let refs = inv.in_role(BenchRole::Reference);
    // Devices that want the 10 MHz.
    let consumers: Vec<&InventoryItem> = inv
        .devices
        .iter()
        .filter(|d| {
            !matches!(
                d.role,
                BenchRole::Reference | BenchRole::Distribution | BenchRole::Antenna
            )
        })
        .filter(|d| spec_of(d).and_then(|s| s.takes_10mhz()) == Some(true))
        .collect();
    if req.need_sync {
        if refs.is_empty() {
            cx.add(BenchRole::Reference, "", "present", Status::Fail, "time alignment (need_sync) needs a 10 MHz + 1PPS reference (GPSDO, rubidium or a timing receiver)".into(), None);
        }
        for d in &refs {
            let Some(s) = spec_of(d) else { continue };
            let has10 = s
                .ref_out
                .as_ref()
                .map(|r| r.freqs_hz.iter().any(|f| (f - 10e6).abs() < 1.0));
            let (st, det) = match has10 {
                Some(true) => (Status::Pass, "10 MHz output".to_string()),
                Some(false) => (Status::Fail, "no 10 MHz output published".to_string()),
                None => (Status::Unknown, "10 MHz output not stated".to_string()),
            };
            cx.add(
                BenchRole::Reference,
                &d.label(),
                "10 MHz out",
                st,
                det,
                Some(s),
            );
            let (st, det) = match s.pps_out {
                Some(true) => (Status::Pass, "1PPS output".into()),
                Some(false) => (
                    Status::Fail,
                    "no 1PPS output published (needed for the TIC start and device time)".into(),
                ),
                None => (Status::Unknown, "1PPS output not stated".into()),
            };
            cx.add(
                BenchRole::Reference,
                &d.label(),
                "1PPS out",
                st,
                det,
                Some(s),
            );
        }
        // Consumers that cannot take the reference.
        for d in inv
            .devices
            .iter()
            .filter(|d| matches!(d.role, BenchRole::EloranSource | BenchRole::GnssSource))
        {
            let Some(s) = spec_of(d) else { continue };
            let (st, det) = match s.takes_10mhz() {
                Some(true) => (Status::Pass, "accepts the external 10 MHz".to_string()),
                Some(false) => (Status::Fail, "does not accept 10 MHz: its clock cannot be locked to the reference".to_string()),
                None => (Status::Unknown, "reference input not stated: check the manual (some take 10 MHz only with an option)".to_string()),
            };
            cx.add(d.role, &d.label(), "locks to 10 MHz", st, det, Some(s));
            let (st, det) = match (s.pps_in, s.timestamped_tx) {
                (Some(true), _) => (Status::Pass, "1PPS input: output can start on a reference PPS edge".to_string()),
                (_, Some(true)) => (Status::Warn, "no PPS input published, but timed start is supported: align by loopback calibration".to_string()),
                (Some(false), _) => (Status::Warn, "no PPS input: absolute start time comes from loopback calibration against the reference".to_string()),
                (None, _) => (Status::Unknown, "PPS input not stated".to_string()),
            };
            cx.add(d.role, &d.label(), "starts on PPS", st, det, Some(s));
        }
        // Outputs vs consumers.
        let outs: usize = refs
            .iter()
            .filter_map(|d| spec_of(d))
            .map(|s| {
                s.ref_out
                    .as_ref()
                    .map(|r| {
                        r.freqs_hz
                            .iter()
                            .filter(|f| (**f - 10e6).abs() < 1.0)
                            .count()
                    })
                    .unwrap_or(0)
            })
            .sum::<usize>()
            + inv
                .in_role(BenchRole::Distribution)
                .iter()
                .map(|_| 6usize)
                .sum::<usize>();
        if !refs.is_empty() && consumers.len() > outs.max(1) {
            cx.add(
                BenchRole::Distribution,
                "",
                "reference fan-out",
                Status::Warn,
                format!(
                    "{} devices take 10 MHz but the reference publishes {} 10 MHz output(s): add a distribution amplifier or tee only with a buffer (never split a reference passively over long cables)",
                    consumers.len(),
                    outs
                ),
                None,
            );
        }
    }

    // --- TIC --------------------------------------------------------------------------
    let tics: Vec<&InventoryItem> = inv
        .devices
        .iter()
        .filter(|d| matches!(d.role, BenchRole::Tic))
        .collect();
    if req.need_sync && tics.is_empty() {
        cx.add(BenchRole::Tic, "", "present", Status::Warn, "no time-interval counter: PPS jitter, delay stability and eLoran-GNSS sync tests will be NOT RUN".into(), None);
    }
    for d in &tics {
        let Some(s) = spec_of(d) else { continue };
        let r = s.timing.as_ref().and_then(|t| t.single_shot_resolution_s);
        let (st, det) = match r {
            Some(r) if r <= req.tic_resolution_s => (Status::Pass, format!("single-shot resolution {:.0} ps (needed ≤ {:.0} ps)", r * 1e12, req.tic_resolution_s * 1e12)),
            Some(r) if r <= 10.0 * req.tic_resolution_s => (Status::Warn, format!("single-shot resolution {:.0} ps is coarser than {:.0} ps: average more readings", r * 1e12, req.tic_resolution_s * 1e12)),
            Some(r) => (Status::Fail, format!("single-shot resolution {:.1} ns is too coarse for {:.1} ns", r * 1e9, req.tic_resolution_s * 1e9)),
            None => (Status::Unknown, "single-shot resolution not published".into()),
        };
        cx.add(BenchRole::Tic, &d.label(), "resolution", st, det, Some(s));
    }

    // --- Digitiser -------------------------------------------------------------------
    let digs = inv.in_role(BenchRole::Digitiser);
    if digs.is_empty() {
        cx.add(BenchRole::Digitiser, "", "present", Status::Warn, "no scope or digitiser: loopback delay calibration needs one (or an SDR receive channel)".into(), None);
    }
    for d in &digs {
        let Some(s) = spec_of(d) else { continue };
        let fs = s.rx.as_ref().and_then(|r| r.max_sample_rate_hz);
        let (st, det) = match fs {
            Some(f) if f >= req.min_digitiser_rate_hz => (
                Status::Pass,
                format!("{} sampling", fmt_hz(f).replace("Hz", "S/s")),
            ),
            Some(f) => (
                Status::Fail,
                format!(
                    "{} sampling is below {}",
                    fmt_hz(f),
                    fmt_hz(req.min_digitiser_rate_hz)
                ),
            ),
            None => (Status::Unknown, "sample rate not published".into()),
        };
        cx.add(
            BenchRole::Digitiser,
            &d.label(),
            "sample rate",
            st,
            det,
            Some(s),
        );
        if let Some(c) = s.rx.as_ref().and_then(|r| r.coupling.clone()) {
            let st = if c == "dc" || s.category == Category::Scope {
                Status::Pass
            } else {
                Status::Warn
            };
            cx.add(
                BenchRole::Digitiser,
                &d.label(),
                "input coupling",
                st,
                format!("{c}-coupled input"),
                Some(s),
            );
        }
    }

    // --- Instruments that only gate tests ---------------------------------------------
    let has = |r: BenchRole| !inv.in_role(r).is_empty();
    let mut runnable = vec![
        "toa_resolution".to_string(),
        "ecd_resolution".to_string(),
        "asf_resolution".to_string(),
        "snr_range".to_string(),
        "skywave_delay_range".to_string(),
    ];
    let mut not = vec![];
    let src_ok = !srcs.is_empty();
    let mut gate = |name: &str, ok: bool, why: &str| {
        if ok {
            runnable.push(name.to_string());
        } else {
            not.push((name.to_string(), why.to_string()));
        }
    };
    gate(
        "power_linearity",
        src_ok && (has(BenchRole::Power) || has(BenchRole::Spectrum)),
        "needs a power meter (or a spectrum analyser in band-power mode)",
    );
    gate(
        "dynamic_range",
        src_ok && has(BenchRole::Spectrum),
        "needs a spectrum analyser",
    );
    gate(
        "spurious_harmonics",
        src_ok && has(BenchRole::Spectrum),
        "needs a spectrum analyser",
    );
    gate(
        "channel_isolation",
        src_ok && has(BenchRole::Spectrum) && tx_total >= 2,
        "needs a spectrum analyser and ≥ 2 output channels",
    );
    gate(
        "pps_jitter",
        src_ok && has(BenchRole::Tic) && !refs.is_empty(),
        "needs a TIC and a reference PPS",
    );
    gate(
        "delay_stability",
        src_ok && has(BenchRole::Tic) && !refs.is_empty(),
        "needs a TIC and a reference PPS",
    );
    gate(
        "reference_lock",
        src_ok && has(BenchRole::Counter),
        "needs a frequency counter",
    );
    gate(
        "eloran_gnss_sync",
        src_ok && has(BenchRole::Tic) && has(BenchRole::GnssSource) && !refs.is_empty(),
        "needs a TIC, a GNSS source and a shared reference",
    );
    gate(
        "loopback_delay_calibration",
        src_ok && has(BenchRole::Digitiser),
        "needs a scope/digitiser (or an SDR receive channel)",
    );

    let fails = cx.out.iter().filter(|f| f.status == Status::Fail).count();
    let mut counts = StatusCounts::default();
    for f in &cx.out {
        match f.status {
            Status::Pass => counts.pass += 1,
            Status::Warn => counts.warn += 1,
            Status::Unknown => counts.unknown += 1,
            Status::Fail => counts.fail += 1,
        }
    }
    Assessment {
        bench: inv.name.clone(),
        findings: cx.out,
        ready: fails == 0,
        runnable,
        not_runnable: not,
        counts,
    }
}

fn check_eloran_source(cx: &mut Ctx, req: &BenchRequirements, d: &InventoryItem, s: &HwSpec) {
    let role = BenchRole::EloranSource;
    let dev = d.label();
    let Some(tx) = s.tx.as_ref() else {
        cx.add(
            role,
            &dev,
            "has an output",
            Status::Fail,
            format!("{} has no published signal output", s.model),
            Some(s),
        );
        return;
    };
    if !matches!(s.category, Category::Sdr | Category::Awg | Category::Daq) {
        cx.add(
            role,
            &dev,
            "pulse-shaping source",
            Status::Warn,
            format!(
                "a {} is not a waveform source: check that its output can carry arbitrary eLoran pulses, not only a sine or square ({})",
                s.category.label(),
                tx.lf_capable_note.as_deref().unwrap_or("see the catalogue notes")
            ),
            Some(s),
        );
    }
    // Band: the decisive check.
    let (st, det) = match s.lf_direct() {
        Some(true) => (
            Status::Pass,
            format!(
                "covers 90–110 kHz directly ({}-coupled, from {})",
                tx.coupling.as_deref().unwrap_or("?"),
                tx.freq_min_hz.map(fmt_hz).unwrap_or_else(|| "DC".into())
            ),
        ),
        Some(false) => (
            Status::Fail,
            format!(
                "cannot output 90–110 kHz directly: lowest frequency {}{}{}",
                tx.freq_min_hz.map(fmt_hz).unwrap_or_else(|| "?".into()),
                tx.coupling
                    .as_deref()
                    .map(|c| format!(", {c}-coupled"))
                    .unwrap_or_default(),
                tx.lf_capable_note
                    .as_deref()
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default()
            ),
        ),
        None => (
            Status::Unknown,
            "lowest output frequency / coupling not published: check before buying".into(),
        ),
    };
    cx.add(role, &dev, "90–110 kHz band", st, det, Some(s));
    // Sample rate.
    let (st, det) = match tx.max_sample_rate_hz {
        Some(f) if f >= req.warn_sample_rate_hz => (
            Status::Pass,
            format!("{} max output rate", fmt_hz(f).replace("Hz", "S/s")),
        ),
        Some(f) if f >= req.min_sample_rate_hz => (
            Status::Warn,
            format!(
                "{} output rate is below the recommended {}",
                fmt_hz(f).replace("Hz", "S/s"),
                fmt_hz(req.warn_sample_rate_hz).replace("Hz", "S/s")
            ),
        ),
        Some(f) => (
            Status::Fail,
            format!(
                "{} output rate is below the {} floor",
                fmt_hz(f).replace("Hz", "S/s"),
                fmt_hz(req.min_sample_rate_hz).replace("Hz", "S/s")
            ),
        ),
        None => (Status::Unknown, "output sample rate not published".into()),
    };
    cx.add(role, &dev, "sample rate", st, det, Some(s));
    // DAC bits.
    let (st, det) = match tx.dac_bits {
        Some(b) if b >= req.warn_dac_bits => (
            Status::Pass,
            format!(
                "{b}-bit DAC (≈{:.0} dB quantisation SNR)",
                6.02 * b as f64 + 1.76
            ),
        ),
        Some(b) if b >= req.min_dac_bits => (
            Status::Warn,
            format!(
                "{b}-bit DAC (≈{:.0} dB): low-SNR and interference-ratio tests lose range",
                6.02 * b as f64 + 1.76
            ),
        ),
        Some(b) => (
            Status::Fail,
            format!("{b}-bit DAC is below {} bits", req.min_dac_bits),
        ),
        None => (Status::Unknown, "DAC resolution not published".into()),
    };
    cx.add(role, &dev, "DAC resolution", st, det, Some(s));
    // Driver.
    let (st, det) = match s.kshana_driver.as_deref() {
        Some("none") | None => (Status::Warn, "no Kshana streaming driver: use file export (SigMF) and the vendor's playback, or an AWG upload".to_string()),
        Some(drv) => (Status::Pass, format!("Kshana driver: {drv}")),
    };
    cx.add(role, &dev, "driver", st, det, Some(s));
}

fn check_gnss_source(cx: &mut Ctx, req: &BenchRequirements, d: &InventoryItem, s: &HwSpec) {
    let role = BenchRole::GnssSource;
    let dev = d.label();
    match s.category {
        Category::GnssSim => {
            let bands: Vec<String> = s.gnss.as_ref().map(|g| g.bands.clone()).unwrap_or_default();
            for b in &req.gnss_bands {
                let have = bands.iter().any(|x| {
                    x.eq_ignore_ascii_case(b)
                        || (gnss_band_hz(x).is_some() && gnss_band_hz(x) == gnss_band_hz(b))
                });
                let (st, det) = if bands.is_empty() {
                    (Status::Unknown, format!("band {b}: bands not published"))
                } else if have {
                    (Status::Pass, format!("band {b} supported"))
                } else {
                    (
                        Status::Fail,
                        format!("band {b} not in the published bands ({})", bands.join(", ")),
                    )
                };
                cx.add(role, &dev, "GNSS bands", st, det, Some(s));
            }
            let api = s.gnss.as_ref().and_then(|g| g.remote_api.clone());
            let st = if api.is_some() {
                Status::Pass
            } else {
                Status::Unknown
            };
            cx.add(
                role,
                &dev,
                "remote control",
                st,
                api.unwrap_or_else(|| "remote-control interface not published".into()),
                Some(s),
            );
        }
        _ => {
            // An SDR used as a GNSS signal generator.
            let hi = s.tx.as_ref().and_then(|t| t.freq_max_hz);
            for b in &req.gnss_bands {
                let Some(f) = gnss_band_hz(b) else { continue };
                let lo = s.tx.as_ref().and_then(|t| t.freq_min_hz).unwrap_or(0.0);
                let (st, det) = match hi {
                    Some(h) if h >= f && lo <= f => (Status::Warn, format!("can transmit {b} ({}) as an SDR GNSS generator: lab use only, conducted, and not a calibrated simulator", fmt_hz(f))),
                    Some(h) => (Status::Fail, format!("tops out at {}, below {b} ({})", fmt_hz(h), fmt_hz(f))),
                    None => (Status::Unknown, "RF range not published".into()),
                };
                cx.add(role, &dev, "GNSS band", st, det, Some(s));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAT: &str = r#"[
      {"id":"lf-gen","vendor":"A","model":"LF","category":"sdr",
       "tx":{"channels":2,"dac_bits":14,"max_sample_rate_hz":125e6,"freq_min_hz":0,"freq_max_hz":50e6,"coupling":"dc"},
       "rx":{"channels":2,"adc_bits":14,"max_sample_rate_hz":125e6,"coupling":"dc"},
       "ref_in":{"freqs_hz":[10e6]},"pps_in":true,"kshana_driver":"redpitaya",
       "sources":[{"url":"https://example.org/lf","title":"t"}]},
      {"id":"rf-sdr","vendor":"B","model":"RF","category":"sdr",
       "tx":{"channels":2,"dac_bits":12,"max_sample_rate_hz":61.44e6,"freq_min_hz":70e6,"freq_max_hz":6e9,"coupling":"ac"},
       "ref_in":{"freqs_hz":[10e6]},"pps_in":true,"kshana_driver":"uhd",
       "sources":[{"url":"https://example.org/rf","title":"t"}]},
      {"id":"gpsdo","vendor":"C","model":"G","category":"reference",
       "ref_out":{"freqs_hz":[10e6]},"pps_out":true,
       "sources":[{"url":"https://example.org/g","title":"t"}]},
      {"id":"ticc","vendor":"D","model":"T","category":"tic",
       "timing":{"single_shot_resolution_s":6e-11},
       "sources":[{"url":"https://example.org/t","title":"t"}]}
    ]"#;

    fn cat() -> Catalog {
        let mut c = Catalog::default();
        c.merge_json(CAT).unwrap();
        c
    }

    #[test]
    fn lf_bench_is_ready_and_rf_only_sdr_is_not() {
        let inv = Inventory::from_toml(
            r#"name = "A"
            [[device]]
            catalog = "lf-gen"
            role = "eloran_source"
            [[device]]
            catalog = "rf-sdr"
            role = "gnss_source"
            [[device]]
            catalog = "gpsdo"
            role = "reference"
            [[device]]
            catalog = "ticc"
            role = "tic"
            [[device]]
            catalog = "lf-gen"
            role = "digitiser"
            name = "lf-gen rx"
            "#,
        )
        .unwrap();
        let a = assess(&inv, &cat());
        assert!(a.ready, "{}", a.to_text());
        assert!(a.runnable.iter().any(|r| r == "eloran_gnss_sync"));
        assert!(a.not_runnable.iter().any(|(r, _)| r == "dynamic_range"));

        let bad = Inventory::from_toml(
            r#"[requirements]
            need_gnss = false
            [[device]]
            catalog = "rf-sdr"
            role = "eloran_source"
            [[device]]
            catalog = "gpsdo"
            role = "reference"
            "#,
        )
        .unwrap();
        let a = assess(&bad, &cat());
        assert!(!a.ready);
        let f = a
            .findings
            .iter()
            .find(|f| f.check == "90–110 kHz band")
            .unwrap();
        assert_eq!(f.status, Status::Fail);
        assert!(f.detail.contains("70.000 MHz"), "{}", f.detail);
        assert_eq!(f.source.as_deref(), Some("https://example.org/rf"));
    }

    #[test]
    fn unknown_ids_fail_with_suggestions() {
        let inv = Inventory::from_toml(
            r#"[[device]]
            catalog = "lf-generator"
            role = "eloran_source""#,
        )
        .unwrap();
        let a = assess(&inv, &cat());
        let f = &a.findings[0];
        assert_eq!(f.status, Status::Fail);
        assert!(f.detail.contains("lf-gen"), "{}", f.detail);
    }
}
