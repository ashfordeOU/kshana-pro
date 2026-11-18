// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Run the acceptance suite on a bench described by an inventory: the eLoran source is
//! opened by its driver (or its spec twin with `rehearse`), the instruments by their
//! profiles (`<profiles_dir>/<label>.toml`, written by [`super::kit::write_kit`]), and every
//! test the bench can do runs; the rest are NOT RUN with the reason.

use super::catalog::Catalog;
use super::check::{BenchRole, Inventory, InventoryItem};
use super::open::open_item;
use crate::accept::suite::{
    default_suite, run_suite, Bench, DeviceStimulus, SuiteReport, TestCase,
};
use crate::hw::{DeviceConfig, PpsSource, RefSource};
use crate::instr::profile::{InstrumentProfile, Role};
use crate::instr::roles::{FrequencyCounter, PowerMeter, SpectrumAnalyser, TimeIntervalCounter};
use crate::instr::ticc::Ticc;
use crate::instr::{FrequencySource, PowerSource, SpectrumSource, TicSource};
use std::path::{Path, PathBuf};

/// Options of [`run_bench`].
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Output sample rate (Hz).
    pub fs_hz: f64,
    /// Seconds each stimulus runs while instruments measure.
    pub stimulus_s: f64,
    /// Use spec twins instead of hardware: the source and the TIC / counter are emulated
    /// from the catalogue; spectrum and power instruments are absent.
    pub rehearse: bool,
    /// Directory of instrument profiles.
    pub profiles_dir: Option<PathBuf>,
    /// Test ids to run (all when `None`).
    pub select: Option<Vec<String>>,
    pub evidence_dir: Option<PathBuf>,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            fs_hz: 2e6,
            stimulus_s: 5.0,
            rehearse: false,
            profiles_dir: None,
            select: None,
            evidence_dir: None,
        }
    }
}

/// A time-interval counter emulated from the catalogue: readings are a constant offset
/// plus white Gaussian noise whose RMS combines the counter's single-shot resolution and
/// the published PPS jitter of the reference and the source (root-sum-square).
pub struct TwinTic {
    pub label: String,
    pub offset_ns: f64,
    pub rms_ns: f64,
    rng: rand_chacha::ChaCha8Rng,
}

impl TwinTic {
    pub fn new(label: &str, offset_ns: f64, rms_ns: f64, seed: u64) -> TwinTic {
        use rand::SeedableRng;
        TwinTic {
            label: label.into(),
            offset_ns,
            rms_ns,
            rng: rand_chacha::ChaCha8Rng::seed_from_u64(seed),
        }
    }
    fn gauss(&mut self) -> f64 {
        use rand::Rng;
        let (u1, u2): (f64, f64) = (self.rng.gen::<f64>().max(1e-300), self.rng.gen());
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

impl TicSource for TwinTic {
    fn id(&self) -> String {
        format!("twin TIC {}", self.label)
    }
    fn arm_pps(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn read_n(&mut self, n: usize) -> Result<Vec<f64>, String> {
        Ok((0..n)
            .map(|_| self.offset_ns + self.rms_ns * self.gauss())
            .collect())
    }
}

/// A frequency counter emulated from the reference's published 1 s Allan deviation.
pub struct TwinCounter {
    pub adev_1s: f64,
    tic: TwinTic,
}

impl FrequencySource for TwinCounter {
    fn id(&self) -> String {
        "twin frequency counter".into()
    }
    fn read_hz(&mut self, gate_s: f64) -> Result<f64, String> {
        let y = self.adev_1s / gate_s.max(1e-3).sqrt() * self.tic.gauss();
        Ok(10e6 * (1.0 + y))
    }
}

fn jitter_s(cat: &Catalog, items: &[&InventoryItem]) -> Option<f64> {
    items
        .iter()
        .filter_map(|d| cat.get(&d.catalog))
        .filter_map(|s| s.timing.as_ref().and_then(|t| t.jitter_rms_s))
        .next()
}

/// What was opened, for the report.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Opened {
    pub source: Option<String>,
    pub instruments: Vec<(String, String)>,
    pub skipped: Vec<(String, String)>,
}

fn profile_for(dir: &Option<PathBuf>, item: &InventoryItem) -> Result<InstrumentProfile, String> {
    let dir = dir
        .as_ref()
        .ok_or("instrument profiles directory not given (write one with `bench kit`)")?;
    let p = dir.join(format!("{}.toml", item.label()));
    let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    InstrumentProfile::from_toml(&t)
}

/// Run the acceptance suite (or `cases`) on the bench.
pub fn run_bench(
    inv: &Inventory,
    cat: &Catalog,
    cases: Option<Vec<TestCase>>,
    opt: &RunOptions,
) -> Result<(SuiteReport, Opened), String> {
    let mut opened = Opened::default();
    let src = inv
        .in_role(BenchRole::EloranSource)
        .into_iter()
        .next()
        .ok_or("the inventory has no eloran_source")?;
    let spec = cat.get(&src.catalog);
    let channels = spec
        .and_then(|s| s.tx.as_ref())
        .and_then(|t| t.channels)
        .unwrap_or(1)
        .min(inv.requirements.tx_channels.max(1)) as usize;
    let (addr, mut dev) = open_item(src, cat, opt.fs_hz, opt.rehearse)?;
    let mut cfg = DeviceConfig::tx(opt.fs_hz, channels);
    let have_ref = !inv.in_role(BenchRole::Reference).is_empty();
    if have_ref && spec.and_then(|s| s.takes_10mhz()) == Some(true) {
        cfg.clock_source = RefSource::External;
    }
    if have_ref && spec.and_then(|s| s.pps_in) == Some(true) {
        cfg.time_source = PpsSource::External;
    }
    dev.configure(&cfg)?;
    opened.source = Some(addr);
    let mut stim = DeviceStimulus::new(dev, opt.fs_hz, channels, opt.stimulus_s);

    let mut tic: Option<Box<dyn TicSource>> = None;
    let mut sync_tic: Option<Box<dyn TicSource>> = None;
    let mut sa: Option<Box<dyn SpectrumSource>> = None;
    let mut pm: Option<Box<dyn PowerSource>> = None;
    let mut fc: Option<Box<dyn FrequencySource>> = None;
    if opt.rehearse {
        let refs = inv.in_role(BenchRole::Reference);
        let srcs = inv.in_role(BenchRole::EloranSource);
        let tics = inv.in_role(BenchRole::Tic);
        if let Some(t) = tics.first() {
            let res = cat
                .get(&t.catalog)
                .and_then(|s| s.timing.as_ref())
                .and_then(|x| x.single_shot_resolution_s.or(x.jitter_rms_s));
            let parts = [res, jitter_s(cat, &refs), jitter_s(cat, &srcs)];
            let rms = parts.iter().flatten().map(|x| x * x).sum::<f64>().sqrt() * 1e9;
            let note = format!(
                "twin TIC rms {rms:.3} ns from {} published value(s)",
                parts.iter().flatten().count()
            );
            tic = Some(Box::new(TwinTic::new(&t.label(), 0.0, rms, 1)));
            if !inv.in_role(BenchRole::GnssSource).is_empty() {
                let g = jitter_s(cat, &inv.in_role(BenchRole::GnssSource)).unwrap_or(0.0) * 1e9;
                sync_tic = Some(Box::new(TwinTic::new(
                    &t.label(),
                    0.0,
                    (rms * rms + g * g).sqrt(),
                    2,
                )));
            }
            opened.instruments.push((t.label(), note));
        }
        if let Some(c) = inv.in_role(BenchRole::Counter).first() {
            let adev = refs
                .iter()
                .filter_map(|d| cat.get(&d.catalog))
                .filter_map(|s| s.timing.as_ref().and_then(|t| t.adev_1s))
                .next();
            match adev {
                Some(a) => {
                    fc = Some(Box::new(TwinCounter {
                        adev_1s: a,
                        tic: TwinTic::new("fc", 0.0, 0.0, 3),
                    }));
                    opened.instruments.push((
                        c.label(),
                        format!("twin counter, reference ADEV(1 s) {a:e}"),
                    ));
                }
                None => opened.skipped.push((
                    c.label(),
                    "reference ADEV(1 s) not published: counter not rehearsed".into(),
                )),
            }
        }
        for item in inv
            .devices
            .iter()
            .filter(|d| matches!(d.role, BenchRole::Spectrum | BenchRole::Power))
        {
            opened.skipped.push((item.label(), "spectrum and power instruments are not emulated: their tests run on the real bench".into()));
        }
    } else {
        for item in &inv.devices {
            if !matches!(
                item.role,
                BenchRole::Tic | BenchRole::Counter | BenchRole::Spectrum | BenchRole::Power
            ) {
                continue;
            }
            let p = match profile_for(&opt.profiles_dir, item) {
                Ok(p) => p,
                Err(e) => {
                    opened.skipped.push((item.label(), e));
                    continue;
                }
            };
            let r: Result<(), String> = (|| {
                match (item.role, p.role) {
                    (BenchRole::Tic, Role::Ticc) => {
                        let t: Box<dyn TicSource> = Box::new(Ticc::open(&p, None)?);
                        if tic.is_none() {
                            tic = Some(t)
                        } else {
                            sync_tic = Some(t)
                        }
                    }
                    (BenchRole::Tic, _) => {
                        let t: Box<dyn TicSource> = Box::new(TimeIntervalCounter::open(&p, None)?);
                        if tic.is_none() {
                            tic = Some(t)
                        } else {
                            sync_tic = Some(t)
                        }
                    }
                    (BenchRole::Spectrum, _) => {
                        sa = Some(Box::new(SpectrumAnalyser::open(&p, None)?))
                    }
                    (BenchRole::Power, _) => pm = Some(Box::new(PowerMeter::open(&p, None)?)),
                    (BenchRole::Counter, _) => {
                        fc = Some(Box::new(FrequencyCounter::open(&p, None)?))
                    }
                    _ => {}
                }
                Ok(())
            })();
            match r {
                Ok(()) => opened
                    .instruments
                    .push((item.label(), format!("{:?}", p.role))),
                Err(e) => opened.skipped.push((item.label(), e)),
            }
        }
    }
    let cases = cases.unwrap_or_else(default_suite);
    let mut bench = Bench {
        stimulus: Some(&mut stim),
        power: pm.as_deref_mut().map(|p| p as &mut dyn PowerSource),
        spectrum: sa.as_deref_mut().map(|p| p as &mut dyn SpectrumSource),
        tic: tic.as_deref_mut().map(|p| p as &mut dyn TicSource),
        sync_tic: sync_tic.as_deref_mut().map(|p| p as &mut dyn TicSource),
        freq: fc.as_deref_mut().map(|p| p as &mut dyn FrequencySource),
        evidence_dir: opt.evidence_dir.clone(),
        evidence_refs: vec![format!(
            "device: {}",
            opened.source.clone().unwrap_or_default()
        )],
    };
    let rep = run_suite(&cases, opt.select.as_deref(), &mut bench);
    drop(bench);
    stim.into_device()?;
    Ok((rep, opened))
}

/// Load the inventory and the catalogue (plus an optional lab catalogue).
pub fn load(inventory: &Path, lab_catalog: Option<&Path>) -> Result<(Inventory, Catalog), String> {
    let inv = Inventory::load(inventory)?;
    let mut cat = Catalog::builtin();
    if let Some(p) = lab_catalog {
        cat.merge_file(p)?;
    }
    Ok((inv, cat))
}
