// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Low-cost GNSS path: the hybrid scene's GNSS half streamed to an SDR
//!
//! [`crate::eloran_gnss`] writes the GNSS half of a hybrid eLoran/GNSS scene to a file. This
//! module streams the same scene — the open engine's GNSS IQ layer (`kshana::iq::scene`), GPS
//! L1 C/A and Galileo E1-B on the eLoran scenario's trajectory and time base — chunk by chunk
//! from [`Scene::into_stream`] into a [`ComplexSink`]:
//!
//! * [`SigmfComplexSink`]: `ci16_le` SigMF, optionally **paced** to real time (the sink
//!   sleeps until each block's wall-clock instant, and counts late blocks as underruns), for
//!   dry runs, replay through any SDR tool, and tests;
//! * `SoapyComplexTx` (feature `eloran-soapy`, Unix): a minimal complex transmit path to a
//!   SoapySDR 0.8 device opened at run time with `dlopen` (no link dependency), the same
//!   approach as the eLoran crate's real-only `hw::soapy::SoapyDevice`, here with complex
//!   `CF32`/`CS16` samples at an RF centre frequency (1575.42 MHz). Compile-tested only; it
//!   needs a real device to be verified.
//!
//! ## Common start with the eLoran stream
//!
//! Sample 0 of both scenes is the scenario's `start_utc` ([`StartPlan`]: the TAI instant, the
//! GPS week and time of week). For a PPS-aligned start of both outputs, give both devices the
//! same 10 MHz and 1PPS, set both device clocks at the same PPS edge, and start both streams at
//! the same device time ([`StartPlan::shared_device_start`] gives a
//! `StartTime::AtDeviceTimeNs`); `StartTime::NextPps` arms each device on its own next edge,
//! which is common only if both are armed within the same second.
//!
//! ## Degradation
//!
//! A [`DegradationTimeline`] (C/N0 drops, ramps and outages per satellite over time) is applied
//! through the scene's channel hook ([`kshana::iq::scene::SceneChannel`]) by
//! [`DegradationChannel`]: path amplitudes are scaled by `10^{Δ/20}`, an outage returns no path
//! (no signal). [`DegradedTruth`] applies the same timeline to the truth records (`cn0_dbhz`
//! offset; `visible = false` in an outage), so the truth states what was generated. The scene
//! holds a channel value between geometry knots (1 kHz), so an event edge is resolved to 1 ms.
//!
//! ## Policy
//!
//! **Conducted use only.** GNSS L-band output goes into a cable, attenuator, or shielded
//! enclosure feeding the receiver under test, never to a radiating antenna: transmitting GNSS
//! signals over the air is unlawful in most jurisdictions and endangers navigation and timing
//! users. [`GNSS_OUTPUT_POLICY`] is printed when a stream starts and written into the SigMF
//! metadata.
//!
//! MODELLED: the signal is the open engine's synthetic scene (see `kshana::iq::scene`).

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kshana::gps_lnav::LnavConventions;
use kshana::iq::io::format::SampleFormat;
use kshana::iq::io::IqWriter;
use kshana::iq::scene::{
    NavData, NoiseConfig, ReceiverClock, SatGeometry, SatView, Scene, SceneChannel, SceneConfig,
    SceneSatellite, TruthRecord, TruthSink,
};
use kshana::iq::{Cf64, ChannelSnapshot, IqError, IqSink, IqSource, SampleSpec};
use kshana::rinex::parse_nav;
use kshana_eloran::hw::StartTime;
use kshana_eloran::time::AbsTime;
use kshana_eloran::timebase::GnssTime;
use kshana_eloran::Scenario;
use serde::{Deserialize, Serialize};

use crate::eloran_gnss::{
    gps_week_tow, select_ephemerides, to_gnss_trajectory, GnssOptions, L1_HZ,
};

/// The conducted-output policy of the GNSS path.
pub const GNSS_OUTPUT_POLICY: &str = "OUTPUT POLICY: conducted use only. Connect this GNSS \
L-band output to cables, attenuators or a shielded enclosure feeding the receiver under test, \
never to a radiating antenna. Over-the-air GNSS transmission is not permitted.";

/// Print the policy for `sink` on standard error.
pub fn announce_policy(sink: &str) {
    eprintln!("[kshana-pro eloran-gnss] {sink}: {GNSS_OUTPUT_POLICY}");
}

// ------------------------------------------------------------------ common start

/// The start instant shared by the eLoran and GNSS streams.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartPlan {
    /// The scenario's start (UTC, ISO 8601): sample 0 of both scenes.
    pub start_utc: String,
    /// TAI picoseconds since 1958 (`kshana_eloran::time::AbsTime`).
    pub start_tai_ps: i128,
    pub gps_week: i64,
    pub start_tow_s: f64,
    /// When the devices start (shared by both streams).
    pub device_start: StartTime,
}

impl StartPlan {
    /// The plan of scenario `sc`, starting the devices at `at`.
    pub fn from_scenario(sc: &Scenario, at: StartTime) -> Result<StartPlan, String> {
        let t = AbsTime::from_utc_iso(&sc.scene.start_utc)?;
        let (gps_week, start_tow_s) = gps_week_tow(t);
        Ok(StartPlan {
            start_utc: sc.scene.start_utc.clone(),
            start_tai_ps: t.0,
            gps_week,
            start_tow_s,
            device_start: at,
        })
    }

    /// A common device start: the first whole second of device time at least `lead_s` after
    /// `device_now_ns` (both devices' clocks set at the same PPS edge).
    pub fn shared_device_start(device_now_ns: i64, lead_s: f64) -> StartTime {
        let t = device_now_ns + (lead_s * 1e9) as i64;
        StartTime::AtDeviceTimeNs((t.div_euclid(1_000_000_000) + 1) * 1_000_000_000)
    }
}

// ------------------------------------------------------------------ sinks

/// Counters of a complex sink.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SinkStatus {
    pub samples: u64,
    /// Pacing fell behind (file) or the device reported an underflow.
    pub underruns: u64,
    pub late: u64,
    pub clipped: u64,
    pub start_time_ns: Option<i64>,
    pub pps_aligned: bool,
    pub message: String,
}

/// A sink of complex baseband samples (scene units: noise power 1 per sample when normalised).
pub trait ComplexSink: Send {
    fn name(&self) -> String;
    /// Whether writes are paced by a real clock.
    fn realtime(&self) -> bool;
    /// Start the stream; returns the device start time (ns) when known.
    fn start(&mut self, plan: &StartPlan, fs_hz: f64) -> Result<Option<i64>, String>;
    /// Write one block (blocks until accepted).
    fn write(&mut self, block: &[Cf64]) -> Result<(), String>;
    /// Flush and end the stream.
    fn stop(&mut self) -> Result<(), String>;
    fn status(&self) -> SinkStatus;
}

/// `ci16_le` SigMF file sink, optionally paced to real time.
pub struct SigmfComplexSink {
    base: PathBuf,
    /// Integer counts per scene unit (2000: noise σ ≈ 1414 counts, room for strong signals).
    pub counts_per_unit: f64,
    pub pace: bool,
    /// Extra global metadata fields.
    pub extra_meta: serde_json::Map<String, serde_json::Value>,
    writer: Option<IqWriter<BufWriter<std::fs::File>>>,
    fs_hz: f64,
    t0: Option<Instant>,
    plan: Option<StartPlan>,
    status: SinkStatus,
}

impl SigmfComplexSink {
    /// A sink writing `<base>.sigmf-data` and `<base>.sigmf-meta`.
    pub fn new(base: &Path, pace: bool) -> SigmfComplexSink {
        SigmfComplexSink {
            base: base.to_path_buf(),
            counts_per_unit: 2000.0,
            pace,
            extra_meta: Default::default(),
            writer: None,
            fs_hz: 0.0,
            t0: None,
            plan: None,
            status: SinkStatus::default(),
        }
    }
    pub fn data_path(&self) -> PathBuf {
        suffix(&self.base, "sigmf-data")
    }
    pub fn meta_path(&self) -> PathBuf {
        suffix(&self.base, "sigmf-meta")
    }
}

fn suffix(base: &Path, s: &str) -> PathBuf {
    let mut o = base.as_os_str().to_os_string();
    o.push(".");
    o.push(s);
    PathBuf::from(o)
}

impl ComplexSink for SigmfComplexSink {
    fn name(&self) -> String {
        format!("sigmf:{}", self.base.display())
    }
    fn realtime(&self) -> bool {
        self.pace
    }
    fn start(&mut self, plan: &StartPlan, fs_hz: f64) -> Result<Option<i64>, String> {
        let path = self.data_path();
        let f = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.writer = Some(
            IqWriter::new(BufWriter::new(f), SampleFormat::CI16_LE)
                .with_scale(1.0 / self.counts_per_unit),
        );
        self.fs_hz = fs_hz;
        self.plan = Some(plan.clone());
        if self.pace && plan.device_start == StartTime::NextPps {
            // the system clock's next whole second stands in for the PPS edge
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            std::thread::sleep(Duration::from_nanos(
                1_000_000_000 - now.subsec_nanos() as u64,
            ));
            self.status.pps_aligned = true;
        }
        self.t0 = Some(Instant::now());
        self.status.start_time_ns = match plan.device_start {
            StartTime::AtDeviceTimeNs(t) => Some(t),
            _ => None,
        };
        Ok(self.status.start_time_ns)
    }
    fn write(&mut self, block: &[Cf64]) -> Result<(), String> {
        let w = self.writer.as_mut().ok_or("sink not started")?;
        w.write(block).map_err(|e| e.to_string())?;
        self.status.samples += block.len() as u64;
        self.status.clipped = w.clipped();
        if self.pace {
            if let Some(t0) = self.t0 {
                let due = Duration::from_secs_f64(self.status.samples as f64 / self.fs_hz);
                let el = t0.elapsed();
                if el < due {
                    std::thread::sleep(due - el);
                } else if el - due > Duration::from_secs_f64(block.len() as f64 / self.fs_hz) {
                    self.status.underruns += 1;
                }
            }
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), String> {
        let Some(mut w) = self.writer.take() else {
            return Ok(());
        };
        w.finish().map_err(|e| e.to_string())?;
        let plan = self.plan.clone();
        let mut global = serde_json::Map::new();
        global.insert("core:datatype".into(), "ci16_le".into());
        global.insert("core:sample_rate".into(), self.fs_hz.into());
        global.insert("core:version".into(), "1.0.0".into());
        global.insert(
            "core:description".into(),
            "Kshana Pro hybrid scene, GNSS half, streamed (GPS L1 C/A, Galileo E1-B)".into(),
        );
        global.insert("kshana:output_policy".into(), GNSS_OUTPUT_POLICY.into());
        global.insert("kshana:counts_per_unit".into(), self.counts_per_unit.into());
        if let Some(p) = &plan {
            global.insert("kshana:start_utc".into(), p.start_utc.clone().into());
            global.insert("kshana:gps_week".into(), p.gps_week.into());
            global.insert("kshana:start_tow_s".into(), p.start_tow_s.into());
            global.insert(
                "kshana:device_start".into(),
                serde_json::to_value(p.device_start).unwrap_or_default(),
            );
        }
        global.insert("kshana:paced".into(), self.pace.into());
        global.insert("kshana:underruns".into(), self.status.underruns.into());
        for (k, v) in &self.extra_meta {
            global.insert(k.clone(), v.clone());
        }
        let meta = serde_json::json!({
            "global": global,
            "captures": [{ "core:sample_start": 0, "core:frequency": L1_HZ }],
            "annotations": []
        });
        let mp = self.meta_path();
        std::fs::write(&mp, serde_json::to_string_pretty(&meta).unwrap())
            .map_err(|e| format!("{}: {e}", mp.display()))
    }
    fn status(&self) -> SinkStatus {
        self.status.clone()
    }
}

// ------------------------------------------------------------------ degradation

/// What an event does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    /// C/N0 lower by `db` over the event.
    Cn0Drop { db: f64 },
    /// C/N0 offset linear from `from_db` at the start to `to_db` at the end.
    Ramp { from_db: f64, to_db: f64 },
    /// No signal over the event.
    Outage,
}

/// One event of a timeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DegradationEvent {
    /// Satellite ids (GPS PRN; Galileo `1000 + PRN`); `None` for every satellite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sats: Option<Vec<u32>>,
    /// Scene time (s): `[start_s, end_s)`.
    pub start_s: f64,
    pub end_s: f64,
    #[serde(flatten)]
    pub effect: Effect,
}

/// GNSS degradation over time (TOML: `[[event]]` tables).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DegradationTimeline {
    #[serde(default, rename = "event")]
    pub events: Vec<DegradationEvent>,
}

impl DegradationTimeline {
    pub fn from_toml(s: &str) -> Result<DegradationTimeline, String> {
        let t: DegradationTimeline = toml::from_str(s).map_err(|e| e.to_string())?;
        t.validate()?;
        Ok(t)
    }
    pub fn validate(&self) -> Result<(), String> {
        for (i, e) in self.events.iter().enumerate() {
            if !(e.start_s.is_finite() && e.end_s.is_finite() && e.end_s > e.start_s) {
                return Err(format!("event {i}: end_s must be after start_s"));
            }
            let ok = match e.effect {
                Effect::Cn0Drop { db } => db.is_finite(),
                Effect::Ramp { from_db, to_db } => from_db.is_finite() && to_db.is_finite(),
                Effect::Outage => true,
            };
            if !ok {
                return Err(format!("event {i}: values must be finite"));
            }
        }
        Ok(())
    }
    /// The C/N0 offset (dB) of satellite `sat` at `t_s`; `None` in an outage.
    pub fn offset_db(&self, sat: u32, t_s: f64) -> Option<f64> {
        let mut total = 0.0;
        for e in &self.events {
            if t_s < e.start_s || t_s >= e.end_s {
                continue;
            }
            if e.sats.as_ref().is_some_and(|s| !s.contains(&sat)) {
                continue;
            }
            match e.effect {
                Effect::Outage => return None,
                Effect::Cn0Drop { db } => total -= db,
                Effect::Ramp { from_db, to_db } => {
                    let u = (t_s - e.start_s) / (e.end_s - e.start_s);
                    total += from_db + u * (to_db - from_db);
                }
            }
        }
        Some(total)
    }
}

/// Scene channel applying a timeline over an optional inner channel.
pub struct DegradationChannel {
    pub timeline: DegradationTimeline,
    inner: Option<Box<dyn SceneChannel>>,
}

impl DegradationChannel {
    pub fn new(timeline: DegradationTimeline, inner: Option<Box<dyn SceneChannel>>) -> Self {
        DegradationChannel { timeline, inner }
    }
}

impl SceneChannel for DegradationChannel {
    fn snapshot(&mut self, sat_id: u32, t_s: f64, view: &SatView) -> ChannelSnapshot {
        let mut snap = match self.inner.as_mut() {
            Some(c) => c.snapshot(sat_id, t_s, view),
            None => ChannelSnapshot {
                t_s,
                paths: vec![kshana::iq::scene::direct_path()],
            },
        };
        match self.timeline.offset_db(sat_id, t_s) {
            None => snap.paths.clear(),
            Some(db) => {
                let g = 10f64.powf(db / 20.0);
                for p in &mut snap.paths {
                    p.amplitude *= g;
                }
            }
        }
        snap
    }
}

/// Truth sink applying a timeline to `cn0_dbhz` and `visible`.
pub struct DegradedTruth<'a> {
    pub timeline: DegradationTimeline,
    inner: &'a mut dyn TruthSink,
}

impl<'a> DegradedTruth<'a> {
    pub fn new(timeline: DegradationTimeline, inner: &'a mut dyn TruthSink) -> Self {
        DegradedTruth { timeline, inner }
    }
}

impl TruthSink for DegradedTruth<'_> {
    fn record(&mut self, r: &TruthRecord) -> Result<(), IqError> {
        let mut r = *r;
        match self.timeline.offset_db(r.sat_id, r.t_s) {
            None => {
                r.visible = false;
                r.cn0_dbhz = f64::NEG_INFINITY;
            }
            Some(db) => r.cn0_dbhz += db,
        }
        self.inner.record(&r)
    }
    fn finish(&mut self) -> Result<(), IqError> {
        self.inner.finish()
    }
}

// ------------------------------------------------------------------ the stream

/// Options of a stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SdrOptions {
    /// Scene options (sample rate, systems, mask, C/N0, duration, threads).
    pub gnss: GnssOptions,
    /// Add thermal noise (normalised to unit power per sample).
    pub noise: bool,
    /// Only these satellite ids (GPS PRN; Galileo `1000 + PRN`); `None` = every usable one.
    pub only: Option<Vec<u32>>,
    pub chunk_samples: usize,
    pub start: StartTime,
    pub timeline: Option<DegradationTimeline>,
    /// Truth interval (s).
    pub truth_interval_s: f64,
}

impl Default for SdrOptions {
    fn default() -> Self {
        SdrOptions {
            gnss: GnssOptions::default(),
            noise: true,
            only: None,
            chunk_samples: 1 << 16,
            start: StartTime::NextPps,
            timeline: None,
            truth_interval_s: 0.1,
        }
    }
}

/// What a stream did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamReport {
    pub plan: StartPlan,
    pub satellites: Vec<String>,
    pub samples: u64,
    pub truth_records: u64,
    pub sink: SinkStatus,
    /// Scene seconds per wall-clock second.
    pub realtime_factor: f64,
    pub policy: String,
}

/// Build the GNSS scene of `sc` (same construction as [`crate::eloran_gnss::generate`]).
pub fn build_scene(
    sc: &Scenario,
    nav_text: &str,
    opts: &SdrOptions,
) -> Result<(Scene, Vec<String>, StartPlan), String> {
    let plan = StartPlan::from_scenario(sc, opts.start)?;
    let g = &opts.gnss;
    let duration = g.duration_s.unwrap_or(sc.scene.params.duration_s);
    let ephs = parse_nav(nav_text)?;
    let residual_s = sc
        .gnss_time_residual_ns
        .get(&GnssTime::Gpst)
        .copied()
        .unwrap_or(0.0)
        * 1e-9;
    let spec = SampleSpec {
        fs_hz: g.fs_hz,
        center_hz: L1_HZ,
        if_hz: 0.0,
    };
    let mut cfg = SceneConfig::new(spec, duration);
    cfg.start_tow_s = plan.start_tow_s;
    cfg.receiver = to_gnss_trajectory(&sc.receiver);
    cfg.clock = ReceiverClock {
        bias_s: -residual_s,
        drift_s_per_s: 0.0,
    };
    cfg.seed = sc.scene.params.seed;
    cfg.elevation_mask_deg = g.elevation_mask_deg;
    cfg.truth_interval_s = opts.truth_interval_s;
    cfg.threads = g.threads.max(1);
    cfg.chunk_samples = opts.chunk_samples.max(1);
    cfg.noise = NoiseConfig {
        enabled: opts.noise,
        ..NoiseConfig::default()
    };
    let mut scene = Scene::new(cfg).map_err(|e| e.to_string())?;
    let want = |id: u32| opts.only.as_ref().is_none_or(|o| o.contains(&id));
    let (week, tow) = (plan.gps_week, plan.start_tow_s);
    let mut names = Vec::new();
    if g.gps {
        for e in select_ephemerides(&ephs, 'G', week, tow) {
            if !want(e.prn as u32) {
                continue;
            }
            let s = SceneSatellite::gps_l1ca_broadcast(&e, g.cn0_dbhz, LnavConventions::default())
                .map_err(|x| x.to_string())?;
            names.push(format!("G{:02} L1C/A", e.prn));
            scene.add_satellite(s);
        }
    }
    if g.galileo {
        for e in select_ephemerides(&ephs, 'E', week, tow) {
            if !want(1000 + e.prn as u32) {
                continue;
            }
            let code =
                kshana::iq::signals::galileo::e1b(e.prn as u16).map_err(|x| x.to_string())?;
            names.push(format!("E{:02} E1-B", e.prn));
            scene.add_satellite(SceneSatellite {
                id: 1000 + e.prn as u32,
                code: Box::new(code),
                geometry: SatGeometry::Broadcast(Box::new(e)),
                cn0_dbhz: g.cn0_dbhz,
                nav: NavData::Seeded {
                    seed: sc.scene.params.seed ^ e.prn as u64,
                },
            });
        }
    }
    if names.is_empty() {
        return Err(format!(
            "no usable ephemeris for GPS week {week} TOW {tow:.0} s (±2 h) in the navigation file"
        ));
    }
    if let Some(t) = &opts.timeline {
        t.validate()?;
        scene.set_channel(Box::new(DegradationChannel::new(t.clone(), None)));
    }
    Ok((scene, names, plan))
}

/// The satellites of the scene above the elevation mask at its start, as `(id, elevation
/// degrees)` sorted by decreasing elevation (ids: GPS PRN, Galileo `1000 + PRN`), to aim a
/// degradation timeline.
pub fn satellites_in_view(
    sc: &Scenario,
    nav_text: &str,
    opts: &SdrOptions,
) -> Result<Vec<(u32, f64)>, String> {
    let plan = StartPlan::from_scenario(sc, opts.start)?;
    let ephs = parse_nav(nav_text)?;
    let rx = to_gnss_trajectory(&sc.receiver).position(0.0);
    let station = kshana::frames::ecef_to_geodetic(rx);
    let mut out = Vec::new();
    for (sys, on, base) in [('G', opts.gnss.gps, 0u32), ('E', opts.gnss.galileo, 1000)] {
        if !on {
            continue;
        }
        for e in select_ephemerides(&ephs, sys, plan.gps_week, plan.start_tow_s) {
            let sat = e.sv_position_ecef(plan.start_tow_s - 0.075);
            let el = kshana::frames::look_angles(station, sat)
                .el_rad
                .to_degrees();
            if el >= opts.gnss.elevation_mask_deg {
                out.push((base + e.prn as u32, el));
            }
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(out)
}

/// Stream the GNSS half of `sc` into `sink`, writing truth to `truth` (with the timeline's
/// C/N0 offsets applied). Prints the output policy.
pub fn stream(
    sc: &Scenario,
    nav_text: &str,
    opts: &SdrOptions,
    sink: &mut dyn ComplexSink,
    truth: &mut dyn TruthSink,
) -> Result<StreamReport, String> {
    let (scene, satellites, plan) = build_scene(sc, nav_text, opts)?;
    announce_policy(&sink.name());
    let fs = scene.config().spec.fs_hz;
    let chunk = scene.config().chunk_samples;
    let mut timeline_truth;
    let truth: &mut dyn TruthSink = match &opts.timeline {
        Some(t) => {
            timeline_truth = DegradedTruth::new(t.clone(), truth);
            &mut timeline_truth
        }
        None => truth,
    };
    let mut src = scene.into_stream();
    let mut buf = vec![Cf64::default(); chunk];
    sink.start(&plan, fs)?;
    let t0 = Instant::now();
    let (mut samples, mut records) = (0u64, 0u64);
    let res: Result<(), String> = (|| {
        loop {
            let n = src.read(&mut buf).map_err(|e| e.to_string())?;
            for r in src.take_truth() {
                truth.record(&r).map_err(|e| e.to_string())?;
                records += 1;
            }
            if n == 0 {
                break;
            }
            sink.write(&buf[..n])?;
            samples += n as u64;
        }
        truth.finish().map_err(|e| e.to_string())
    })();
    let stop = sink.stop();
    res?;
    stop?;
    let wall = t0.elapsed().as_secs_f64().max(1e-9);
    Ok(StreamReport {
        plan,
        satellites,
        samples,
        truth_records: records,
        sink: sink.status(),
        realtime_factor: samples as f64 / fs / wall,
        policy: GNSS_OUTPUT_POLICY.into(),
    })
}

// ------------------------------------------------------------------ SoapySDR complex TX

#[cfg(all(feature = "eloran-soapy", unix))]
pub use soapy_tx::{SoapyComplexFormat, SoapyComplexTx};

#[cfg(all(feature = "eloran-soapy", unix))]
mod soapy_tx {
    //! Minimal SoapySDR 0.8 complex transmit (one channel), opened with `dlopen`.
    //! Compile-tested only; needs a real device to be verified.
    use super::{announce_policy, ComplexSink, SinkStatus, StartPlan};
    use kshana::iq::Cf64;
    use kshana_eloran::hw::StartTime;
    use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};

    #[cfg_attr(target_os = "linux", link(name = "dl"))]
    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }
    const RTLD_NOW: c_int = 2;
    const TX: c_int = 0;
    const FLAG_END_BURST: c_int = 1 << 1;
    const FLAG_HAS_TIME: c_int = 1 << 2;
    const ERR_TIMEOUT: c_int = -1;
    const ERR_TIME: c_int = -6;
    const ERR_UNDERFLOW: c_int = -7;

    type Dev = *mut c_void;
    type Stream = *mut c_void;

    /// Complex stream format.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SoapyComplexFormat {
        Cf32,
        /// Complex int16; `full_scale` scene units map to 32767.
        Cs16,
    }

    struct Lib(*mut c_void);
    impl Lib {
        fn open() -> Result<Lib, String> {
            for name in ["libSoapySDR.so.0.8", "libSoapySDR.so", "libSoapySDR.dylib"] {
                let c = CString::new(name).map_err(|e| e.to_string())?;
                // SAFETY: valid C string; a null handle is handled.
                let h = unsafe { dlopen(c.as_ptr(), RTLD_NOW) };
                if !h.is_null() {
                    return Ok(Lib(h));
                }
            }
            Err("SoapySDR library not found (libSoapySDR.so.0.8)".into())
        }
        fn sym<T: Copy>(&self, name: &str) -> Result<T, String> {
            let c = CString::new(name).map_err(|e| e.to_string())?;
            // SAFETY: dlsym on an open handle; the caller names the C signature.
            let p = unsafe { dlsym(self.0, c.as_ptr()) };
            if p.is_null() {
                return Err(format!("SoapySDR symbol {name} missing"));
            }
            // SAFETY: `T` is an `extern "C" fn` pointer, the size of a data pointer.
            Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
        }
    }
    impl Drop for Lib {
        fn drop(&mut self) {
            // SAFETY: handle from dlopen, closed once.
            unsafe { dlclose(self.0) };
        }
    }

    struct Api {
        unmake: extern "C" fn(Dev) -> c_int,
        last_error: extern "C" fn() -> *const c_char,
        set_rate: extern "C" fn(Dev, c_int, usize, f64) -> c_int,
        set_freq: extern "C" fn(Dev, c_int, usize, f64, *const c_void) -> c_int,
        set_gain: extern "C" fn(Dev, c_int, usize, f64) -> c_int,
        set_clock_source: extern "C" fn(Dev, *const c_char) -> c_int,
        set_time_source: extern "C" fn(Dev, *const c_char) -> c_int,
        get_hw_time: extern "C" fn(Dev, *const c_char) -> i64,
        set_hw_time: extern "C" fn(Dev, i64, *const c_char) -> c_int,
        setup:
            extern "C" fn(Dev, c_int, *const c_char, *const usize, usize, *const c_void) -> Stream,
        close: extern "C" fn(Dev, Stream) -> c_int,
        activate: extern "C" fn(Dev, Stream, c_int, i64, usize) -> c_int,
        deactivate: extern "C" fn(Dev, Stream, c_int, i64) -> c_int,
        write: extern "C" fn(
            Dev,
            Stream,
            *const *const c_void,
            usize,
            *mut c_int,
            i64,
            c_long,
        ) -> c_int,
        read_status: extern "C" fn(Dev, Stream, *mut usize, *mut c_int, *mut i64, c_long) -> c_int,
    }

    /// A SoapySDR device transmitting complex baseband on one channel at an RF centre.
    pub struct SoapyComplexTx {
        api: Api,
        dev: Dev,
        args: String,
        format: SoapyComplexFormat,
        /// Scene units mapped to device full scale.
        pub full_scale: f64,
        center_hz: f64,
        gain_db: f64,
        external_ref: bool,
        tx: Stream,
        tx_time: Option<i64>,
        status: SinkStatus,
        _lib: Lib,
    }

    // SAFETY: the handle is only used through `&mut self`, so calls are serialised.
    unsafe impl Send for SoapyComplexTx {}

    impl SoapyComplexTx {
        /// Open `args` (e.g. `driver=uhd`) for complex TX at `center_hz` with `gain_db`;
        /// `external_ref` selects the external 10 MHz and 1PPS. Prints the policy.
        pub fn open(
            args: &str,
            format: SoapyComplexFormat,
            center_hz: f64,
            gain_db: f64,
            external_ref: bool,
        ) -> Result<SoapyComplexTx, String> {
            announce_policy(&format!("soapy:{args}"));
            let lib = Lib::open()?;
            let abi: extern "C" fn() -> *const c_char = lib.sym("SoapySDR_getABIVersion")?;
            // SAFETY: static NUL-terminated string.
            let v = unsafe { CStr::from_ptr(abi()) }
                .to_string_lossy()
                .into_owned();
            if !v.starts_with("0.8") {
                return Err(format!("SoapySDR ABI {v} not supported (0.8 required)"));
            }
            let api = Api {
                unmake: lib.sym("SoapySDRDevice_unmake")?,
                last_error: lib.sym("SoapySDRDevice_lastError")?,
                set_rate: lib.sym("SoapySDRDevice_setSampleRate")?,
                set_freq: lib.sym("SoapySDRDevice_setFrequency")?,
                set_gain: lib.sym("SoapySDRDevice_setGain")?,
                set_clock_source: lib.sym("SoapySDRDevice_setClockSource")?,
                set_time_source: lib.sym("SoapySDRDevice_setTimeSource")?,
                get_hw_time: lib.sym("SoapySDRDevice_getHardwareTime")?,
                set_hw_time: lib.sym("SoapySDRDevice_setHardwareTime")?,
                setup: lib.sym("SoapySDRDevice_setupStream")?,
                close: lib.sym("SoapySDRDevice_closeStream")?,
                activate: lib.sym("SoapySDRDevice_activateStream")?,
                deactivate: lib.sym("SoapySDRDevice_deactivateStream")?,
                write: lib.sym("SoapySDRDevice_writeStream")?,
                read_status: lib.sym("SoapySDRDevice_readStreamStatus")?,
            };
            let make: extern "C" fn(*const c_char) -> Dev =
                lib.sym("SoapySDRDevice_makeStrArgs")?;
            let a = CString::new(args).map_err(|e| e.to_string())?;
            let dev = make(a.as_ptr());
            let d = SoapyComplexTx {
                api,
                dev,
                args: args.into(),
                format,
                full_scale: 8.0,
                center_hz,
                gain_db,
                external_ref,
                tx: std::ptr::null_mut(),
                tx_time: None,
                status: SinkStatus::default(),
                _lib: lib,
            };
            if dev.is_null() {
                return Err(d.err("make"));
            }
            Ok(d)
        }

        fn err(&self, what: &str) -> String {
            // SAFETY: thread-local NUL-terminated string.
            let e = unsafe { CStr::from_ptr((self.api.last_error)()) }
                .to_string_lossy()
                .into_owned();
            format!("soapy:{} {what}: {e}", self.args)
        }
        fn check(&self, r: c_int, what: &str) -> Result<(), String> {
            if r != 0 {
                Err(self.err(what))
            } else {
                Ok(())
            }
        }
        fn poll(&mut self) {
            loop {
                let (mut m, mut f, mut t) = (0usize, 0 as c_int, 0i64);
                match (self.api.read_status)(self.dev, self.tx, &mut m, &mut f, &mut t, 0) {
                    ERR_UNDERFLOW => self.status.underruns += 1,
                    ERR_TIME => self.status.late += 1,
                    _ => break,
                }
            }
        }
    }

    impl ComplexSink for SoapyComplexTx {
        fn name(&self) -> String {
            format!("soapy:{}", self.args)
        }
        fn realtime(&self) -> bool {
            true
        }
        fn start(&mut self, plan: &StartPlan, fs_hz: f64) -> Result<Option<i64>, String> {
            let src = CString::new(if self.external_ref {
                "external"
            } else {
                "internal"
            })
            .map_err(|e| e.to_string())?;
            self.check(
                (self.api.set_clock_source)(self.dev, src.as_ptr()),
                "setClockSource",
            )?;
            self.check(
                (self.api.set_time_source)(self.dev, src.as_ptr()),
                "setTimeSource",
            )?;
            self.check((self.api.set_rate)(self.dev, TX, 0, fs_hz), "setSampleRate")?;
            self.check(
                (self.api.set_freq)(self.dev, TX, 0, self.center_hz, std::ptr::null()),
                "setFrequency",
            )?;
            self.check(
                (self.api.set_gain)(self.dev, TX, 0, self.gain_db),
                "setGain",
            )?;
            let fmt = CString::new(match self.format {
                SoapyComplexFormat::Cf32 => "CF32",
                SoapyComplexFormat::Cs16 => "CS16",
            })
            .map_err(|e| e.to_string())?;
            let ch = [0usize];
            self.tx =
                (self.api.setup)(self.dev, TX, fmt.as_ptr(), ch.as_ptr(), 1, std::ptr::null());
            if self.tx.is_null() {
                return Err(self.err("setupStream"));
            }
            let t = match plan.device_start {
                StartTime::Now => None,
                StartTime::AtDeviceTimeNs(t) => Some(t),
                StartTime::NextPps => {
                    let pps = CString::new("PPS").map_err(|e| e.to_string())?;
                    self.check(
                        (self.api.set_hw_time)(self.dev, 0, pps.as_ptr()),
                        "setHardwareTime(PPS)",
                    )?;
                    std::thread::sleep(std::time::Duration::from_millis(1200));
                    let now = (self.api.get_hw_time)(self.dev, std::ptr::null());
                    self.status.pps_aligned = true;
                    Some(
                        (now + 100_000_000).div_euclid(1_000_000_000) * 1_000_000_000
                            + 1_000_000_000,
                    )
                }
            };
            let (flags, tn) = t.map(|t| (FLAG_HAS_TIME, t)).unwrap_or((0, 0));
            self.check(
                (self.api.activate)(self.dev, self.tx, flags, tn, 0),
                "activateStream(TX)",
            )?;
            self.tx_time = t;
            self.status.start_time_ns = t;
            Ok(t)
        }
        fn write(&mut self, block: &[Cf64]) -> Result<(), String> {
            if self.tx.is_null() {
                return Err("transmit stream not started".into());
            }
            let bytes = match self.format {
                SoapyComplexFormat::Cf32 => 8,
                SoapyComplexFormat::Cs16 => 4,
            };
            let k = 1.0 / self.full_scale;
            let mut buf = Vec::with_capacity(block.len() * bytes);
            for s in block {
                match self.format {
                    SoapyComplexFormat::Cf32 => {
                        buf.extend_from_slice(&((s.re * k) as f32).to_ne_bytes());
                        buf.extend_from_slice(&((s.im * k) as f32).to_ne_bytes());
                    }
                    SoapyComplexFormat::Cs16 => {
                        for v in [s.re, s.im] {
                            let x = v * k * 32767.0;
                            if x.abs() > 32767.0 {
                                self.status.clipped += 1;
                            }
                            buf.extend_from_slice(
                                &(x.round().clamp(-32767.0, 32767.0) as i16).to_ne_bytes(),
                            );
                        }
                    }
                }
            }
            let n = block.len();
            let (mut done, mut timeouts) = (0usize, 0);
            while done < n {
                let p = [buf[done * bytes..].as_ptr() as *const c_void];
                let (mut flags, tn) = match self.tx_time.take() {
                    Some(t) => (FLAG_HAS_TIME, t),
                    None => (0, 0),
                };
                let r = (self.api.write)(
                    self.dev,
                    self.tx,
                    p.as_ptr(),
                    n - done,
                    &mut flags,
                    tn,
                    1_000_000,
                );
                if r > 0 {
                    done += r as usize;
                } else if r == ERR_TIMEOUT && timeouts < 5 {
                    timeouts += 1;
                } else if r == ERR_UNDERFLOW {
                    self.status.underruns += 1;
                } else if r < 0 {
                    return Err(self.err(&format!("writeStream ({r})")));
                }
            }
            self.status.samples += n as u64;
            self.poll();
            Ok(())
        }
        fn stop(&mut self) -> Result<(), String> {
            if self.tx.is_null() {
                return Ok(());
            }
            let zero = [0u8; 8];
            let p = [zero.as_ptr() as *const c_void];
            let mut flags = FLAG_END_BURST;
            (self.api.write)(self.dev, self.tx, p.as_ptr(), 0, &mut flags, 0, 100_000);
            self.poll();
            (self.api.deactivate)(self.dev, self.tx, 0, 0);
            (self.api.close)(self.dev, self.tx);
            self.tx = std::ptr::null_mut();
            Ok(())
        }
        fn status(&self) -> SinkStatus {
            self.status.clone()
        }
    }

    impl Drop for SoapyComplexTx {
        fn drop(&mut self) {
            let _ = self.stop();
            if !self.dev.is_null() {
                (self.api.unmake)(self.dev);
            }
        }
    }
}
