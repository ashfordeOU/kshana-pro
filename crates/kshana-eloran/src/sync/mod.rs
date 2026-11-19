// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! GNSS-simulator synchronisation (stream S14).
//!
//! # Timing model
//!
//! A hybrid eLoran/GNSS test needs both simulators on one time base:
//!
//! 1. **Common 10 MHz and 1PPS.** Both simulators lock to the same 10 MHz and take their
//!    device time from the same 1PPS (distribution amplifier, matched cables). Without a
//!    common reference the two outputs drift apart; [`SyncTimingModel::plan`] warns.
//! 2. **Start-epoch handshake.** The scenario's `start_utc` (a whole second) is the UTC label
//!    of the PPS edge on which both start. The GNSS simulator is given that epoch (UTC, or
//!    GPST calendar time / GPS week + TOW, as it prefers — [`StartEpoch`]), loaded with the
//!    receiver trajectory ([`export`]), and **armed for the next PPS** (or an external
//!    trigger) before the edge; the eLoran device is started with
//!    [`crate::hw::StartTime::NextPps`] for the same edge ([`arm_gnss`]).
//! 3. **Measured offset correction.** The residual `t_eLoran − t_GNSS` at the reference
//!    planes, measured once (e.g. both outputs into a scope or TIC against the PPS) and
//!    stored in the calibration file ([`crate::hw::calib::GnssOffsetCal`]), is removed on the
//!    eLoran side by adding it to the channel delays compensated by
//!    [`crate::hw::calib::CalibratedSink::from_calibration`] (`extra_delay_ns`).
//! 4. **Budget.** [`sync_budget`] combines the remaining uncertainties and checks the
//!    expanded uncertainty against the target (default 10 ns).
//!
//! # Adapters
//!
//! [`GnssSimAdapter`] is the remote-control abstraction. Implementations: the generic
//! line-based TCP/SCPI adapter [`scpi::ScpiTcpAdapter`] with configurable command
//! templates; trajectory exports for simulators that import files
//! ([`export::NmeaTrajectoryExport`], [`export::CsvTrajectoryExport`]); and the
//! [`bridge::SoftwareGnssBridge`] description of the data the open engine's IQ scene needs.

pub mod bridge;
pub mod certify;
pub mod degrade;
pub mod edo;
pub mod export;
pub mod gpssdrsim;
pub mod labsat;
pub mod measure;
pub mod motion;
pub mod profile;
pub mod refdist;
pub mod rs_smbv;
pub mod scpi;
pub mod skydel;
pub mod spirent;

use crate::hw::calib::{combine, BudgetLine, CalibrationFile, UncertaintyComponent};
use crate::hw::StartTime;
use crate::time::{days_from_civil, tai_minus_utc_s};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A UTC calendar instant (no leap-second second; arithmetic is on the civil day grid).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UtcStamp {
    /// Days since 1970-01-01.
    pub days: i64,
    /// Nanoseconds into the day.
    pub ns_of_day: i64,
}

const NS_PER_DAY: i64 = 86_400_000_000_000;

/// Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl UtcStamp {
    /// Parse `YYYY-MM-DDTHH:MM:SS[.f]Z`.
    pub fn parse(s: &str) -> Result<UtcStamp, String> {
        let t = s.trim().trim_end_matches('Z');
        let (date, time) = t
            .split_once('T')
            .ok_or_else(|| format!("expected YYYY-MM-DDTHH:MM:SSZ, got {s}"))?;
        let d: Vec<&str> = date.split('-').collect();
        let tm: Vec<&str> = time.split(':').collect();
        if d.len() != 3 || tm.len() != 3 {
            return Err(format!("expected YYYY-MM-DDTHH:MM:SSZ, got {s}"));
        }
        let p = |x: &str| x.parse::<i64>().map_err(|e| format!("{x}: {e}"));
        let (y, mo, da, h, mi) = (p(d[0])?, p(d[1])?, p(d[2])?, p(tm[0])?, p(tm[1])?);
        let (whole, frac) = tm[2].split_once('.').unwrap_or((tm[2], ""));
        let sec = p(whole)?;
        let mut frac_ns = 0i64;
        if !frac.is_empty() {
            let digits: String = frac.chars().take(9).collect();
            frac_ns = p(&digits)? * 10i64.pow(9 - digits.len() as u32);
        }
        if !(1..=12).contains(&mo) || !(1..=31).contains(&da) || h > 23 || mi > 59 || sec > 59 {
            return Err(format!("date or time out of range: {s}"));
        }
        Ok(UtcStamp {
            days: days_from_civil(y, mo, da),
            ns_of_day: ((h * 60 + mi) * 60 + sec) * 1_000_000_000 + frac_ns,
        })
    }
    /// Add `dt` seconds (rounded to the nanosecond).
    pub fn plus(self, dt: f64) -> UtcStamp {
        let total = self.ns_of_day as i128 + (dt * 1e9).round() as i128;
        let d = total.div_euclid(NS_PER_DAY as i128);
        UtcStamp {
            days: self.days + d as i64,
            ns_of_day: total.rem_euclid(NS_PER_DAY as i128) as i64,
        }
    }
    pub fn ymd(self) -> (i64, i64, i64) {
        civil_from_days(self.days)
    }
    /// (hour, minute, second, nanosecond).
    pub fn hms(self) -> (i64, i64, i64, i64) {
        let s = self.ns_of_day / 1_000_000_000;
        (
            s / 3600,
            (s / 60) % 60,
            s % 60,
            self.ns_of_day % 1_000_000_000,
        )
    }
    /// `YYYY-MM-DDTHH:MM:SS.fffffffffZ`.
    pub fn iso(self) -> String {
        let (y, m, d) = self.ymd();
        let (h, mi, s, ns) = self.hms();
        format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ns:09}Z")
    }
    /// TAI − UTC (s) on this date.
    pub fn leap_s(self) -> f64 {
        let (y, m, d) = self.ymd();
        tai_minus_utc_s(y, m, d)
    }
    /// GPS time (s since 1980-01-06T00:00:00 UTC, the GPS epoch): UTC + (TAI−UTC) − 19 s.
    pub fn gps_seconds(self) -> f64 {
        let d = self.days - days_from_civil(1980, 1, 6);
        d as f64 * 86_400.0 + self.ns_of_day as f64 * 1e-9 + self.leap_s() - 19.0
    }
    /// (GPS week, time of week in s).
    pub fn gps_week_tow(self) -> (i64, f64) {
        let g = self.gps_seconds();
        let w = (g / 604_800.0).floor();
        (w as i64, g - w * 604_800.0)
    }
}

/// Time scale in which a simulator expects its start epoch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochScale {
    #[default]
    Utc,
    /// GPST calendar time (UTC + leap seconds since 1980, i.e. + 18 s from 2017).
    Gpst,
}

/// Scenario start epoch handed to a GNSS simulator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartEpoch {
    /// UTC of the start PPS edge.
    pub utc: UtcStamp,
    pub scale: EpochScale,
}

impl StartEpoch {
    pub fn from_utc_iso(s: &str, scale: EpochScale) -> Result<StartEpoch, String> {
        Ok(StartEpoch {
            utc: UtcStamp::parse(s)?,
            scale,
        })
    }
    /// GPST calendar stamp of the same instant.
    pub fn gpst_calendar(&self) -> UtcStamp {
        self.utc.plus(self.utc.leap_s() - 19.0)
    }
    /// The stamp in the scale the simulator wants.
    pub fn in_scale(&self) -> UtcStamp {
        match self.scale {
            EpochScale::Utc => self.utc,
            EpochScale::Gpst => self.gpst_calendar(),
        }
    }
    /// Placeholder values for command templates: `epoch_iso`, `date` (YYYY-MM-DD),
    /// `time` (HH:MM:SS), `year`, `month`, `day`, `hour`, `minute`, `second`, `utc_iso`,
    /// `gpst_iso`, `gps_week`, `gps_tow`, `leap_s` (GPS − UTC, s).
    pub fn fields(&self) -> BTreeMap<String, String> {
        let s = self.in_scale();
        let (y, m, d) = s.ymd();
        let (h, mi, se, _) = s.hms();
        let (w, tow) = self.utc.gps_week_tow();
        let mut f = BTreeMap::new();
        f.insert("epoch_iso".into(), s.iso());
        f.insert("date".into(), format!("{y:04}-{m:02}-{d:02}"));
        f.insert("time".into(), format!("{h:02}:{mi:02}:{se:02}"));
        f.insert("year".into(), format!("{y:04}"));
        f.insert("month".into(), format!("{m:02}"));
        f.insert("day".into(), format!("{d:02}"));
        f.insert("hour".into(), format!("{h:02}"));
        f.insert("minute".into(), format!("{mi:02}"));
        f.insert("second".into(), format!("{se:02}"));
        f.insert("utc_iso".into(), self.utc.iso());
        f.insert("gpst_iso".into(), self.gpst_calendar().iso());
        f.insert("gps_week".into(), w.to_string());
        f.insert("gps_tow".into(), format!("{tow:.3}"));
        f.insert("leap_s".into(), format!("{}", self.utc.leap_s() - 19.0));
        f
    }
}

/// Trajectory file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajFormat {
    /// NMEA 0183 GGA + RMC sentences.
    Nmea,
    /// Generic CSV `time_s,utc,lat_deg,lon_deg,height_m,heading_deg`.
    Csv,
}

/// A trajectory file written for a simulator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrajectoryFile {
    pub path: PathBuf,
    pub format: TrajFormat,
    pub rate_hz: f64,
    pub samples: usize,
}

/// How the GNSS simulator starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmMode {
    Immediate,
    /// Start on the next 1PPS edge of the common reference.
    NextPps,
    /// Start on an external trigger input.
    ExternalTrigger,
}

/// GNSS simulator state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnssSimState {
    Idle,
    Armed,
    Running,
    Stopped,
    Error,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssSimStatus {
    pub state: GnssSimState,
    /// Raw status text from the simulator.
    pub raw: String,
}

/// Remote control of a GNSS simulator.
pub trait GnssSimAdapter {
    /// Connect; returns the simulator's identification.
    fn connect(&mut self) -> Result<String, String>;
    /// Set the scenario start epoch.
    fn set_start_epoch(&mut self, epoch: &StartEpoch) -> Result<(), String>;
    /// Load a receiver trajectory file (the path as seen by the simulator).
    fn load_trajectory(&mut self, traj: &TrajectoryFile) -> Result<(), String>;
    /// Arm for start.
    fn arm(&mut self, mode: ArmMode) -> Result<(), String>;
    fn status(&mut self) -> Result<GnssSimStatus, String>;
    fn stop(&mut self) -> Result<(), String>;
}

/// The eLoran-vs-GNSS timing model of a test set-up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncTimingModel {
    /// Both simulators locked to one 10 MHz.
    pub common_10mhz: bool,
    /// Both simulators take device time from one 1PPS.
    pub common_pps: bool,
    /// Measured `t_eLoran − t_GNSS` (ns) and its standard uncertainty.
    pub eloran_minus_gnss_ns: f64,
    pub eloran_minus_gnss_u_ns: f64,
    /// Start of the GNSS simulator: next PPS (default) or external trigger.
    pub gnss_arm: ArmMode,
    pub epoch_scale: EpochScale,
}

impl SyncTimingModel {
    /// Model from a calibration file (offset 0 if the file has none).
    pub fn from_calibration(
        cal: &CalibrationFile,
        common_10mhz: bool,
        common_pps: bool,
    ) -> SyncTimingModel {
        let (o, u) = cal
            .gnss_offset
            .as_ref()
            .map(|g| (g.eloran_minus_gnss_ns, g.u_ns))
            .unwrap_or((0.0, 0.0));
        SyncTimingModel {
            common_10mhz,
            common_pps,
            eloran_minus_gnss_ns: o,
            eloran_minus_gnss_u_ns: u,
            gnss_arm: ArmMode::NextPps,
            epoch_scale: EpochScale::Utc,
        }
    }

    /// Plan a synchronised start at the scenario's `start_utc`.
    pub fn plan(&self, start_utc: &str) -> Result<SyncPlan, String> {
        let epoch = StartEpoch::from_utc_iso(start_utc, self.epoch_scale)?;
        let mut warnings = Vec::new();
        if epoch.utc.ns_of_day % 1_000_000_000 != 0 {
            warnings
                .push("start_utc is not a whole second: it cannot coincide with a PPS edge".into());
        }
        if !self.common_10mhz {
            warnings.push("no common 10 MHz: the eLoran and GNSS outputs will drift apart".into());
        }
        if !self.common_pps {
            warnings.push("no common 1PPS: the start instants are not tied together".into());
        }
        if self.gnss_arm == ArmMode::Immediate {
            warnings.push("GNSS simulator armed for immediate start: not PPS aligned".into());
        }
        Ok(SyncPlan {
            epoch,
            eloran_start: StartTime::NextPps,
            gnss_arm: self.gnss_arm,
            eloran_extra_delay_ns: self.eloran_minus_gnss_ns,
            warnings,
        })
    }
}

/// A synchronised-start plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncPlan {
    pub epoch: StartEpoch,
    /// How to start the eLoran device.
    pub eloran_start: StartTime,
    pub gnss_arm: ArmMode,
    /// Add to the compensated channel delays (`CalibratedSink::from_calibration`'s
    /// `extra_delay_ns`): advances the eLoran output by the measured offset.
    pub eloran_extra_delay_ns: f64,
    pub warnings: Vec<String>,
}

/// Steps performed by [`arm_gnss`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HandshakeLog {
    pub identity: String,
    pub status: GnssSimStatus,
    pub steps: Vec<String>,
}

/// Connect, set the epoch, load the trajectory and arm the GNSS simulator per `plan`;
/// fails unless the simulator then reports Armed (or Running for an immediate start).
/// Start the eLoran device with `plan.eloran_start` afterwards, before the PPS edge.
pub fn arm_gnss(
    adapter: &mut dyn GnssSimAdapter,
    plan: &SyncPlan,
    traj: Option<&TrajectoryFile>,
) -> Result<HandshakeLog, String> {
    let mut steps = Vec::new();
    let identity = adapter.connect()?;
    steps.push(format!("connected: {identity}"));
    adapter.set_start_epoch(&plan.epoch)?;
    steps.push(format!("epoch {}", plan.epoch.in_scale().iso()));
    if let Some(t) = traj {
        adapter.load_trajectory(t)?;
        steps.push(format!("trajectory {}", t.path.display()));
    }
    adapter.arm(plan.gnss_arm)?;
    steps.push(format!("armed {:?}", plan.gnss_arm));
    let status = adapter.status()?;
    let ok = match plan.gnss_arm {
        ArmMode::Immediate => matches!(status.state, GnssSimState::Running | GnssSimState::Armed),
        _ => status.state == GnssSimState::Armed,
    };
    if !ok {
        return Err(format!(
            "GNSS simulator not armed after handshake: {:?} ({})",
            status.state, status.raw
        ));
    }
    Ok(HandshakeLog {
        identity,
        status,
        steps,
    })
}

// ---------------------------------------------------------------------------------------
// Sync accuracy budget
// ---------------------------------------------------------------------------------------

/// Inputs of the eLoran-vs-GNSS timing budget (standard uncertainties, ns).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncBudgetInputs {
    /// 10 MHz/1PPS distribution (amplifier skew, cable mismatch).
    pub reference_distribution_ns: f64,
    /// Trigger/start latency uncertainty of the start mechanism (residual after PPS start).
    pub trigger_latency_ns: f64,
    /// eLoran channel delay calibration.
    pub channel_delay_cal_ns: f64,
    /// GNSS simulator output delay calibration.
    pub gnss_sim_delay_cal_ns: f64,
    /// Further inputs.
    #[serde(default)]
    pub extra: Vec<UncertaintyComponent>,
    /// Coverage factor (default 2).
    pub k: f64,
    /// Target for the expanded uncertainty (ns, default 10).
    pub target_ns: f64,
}

impl Default for SyncBudgetInputs {
    fn default() -> Self {
        SyncBudgetInputs {
            reference_distribution_ns: 0.0,
            trigger_latency_ns: 0.0,
            channel_delay_cal_ns: 0.0,
            gnss_sim_delay_cal_ns: 0.0,
            extra: vec![],
            k: 2.0,
            target_ns: 10.0,
        }
    }
}

/// The budget result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncBudgetReport {
    pub lines: Vec<BudgetLine>,
    pub combined_u_ns: f64,
    pub k: f64,
    pub expanded_u_ns: f64,
    pub target_ns: f64,
    /// Expanded uncertainty ≤ target.
    pub pass: bool,
    pub margin_ns: f64,
    /// Name of the largest contributor.
    pub dominant: String,
}

/// Combine the sync uncertainty budget (RSS) and check `k·u_c ≤ target`.
pub fn sync_budget(inp: &SyncBudgetInputs) -> SyncBudgetReport {
    let mut comps = vec![
        UncertaintyComponent::normal(
            "reference distribution (10 MHz/1PPS)",
            inp.reference_distribution_ns,
        ),
        UncertaintyComponent::normal("trigger/start latency", inp.trigger_latency_ns),
        UncertaintyComponent::normal("eLoran channel delay calibration", inp.channel_delay_cal_ns),
        UncertaintyComponent::normal(
            "GNSS simulator delay calibration",
            inp.gnss_sim_delay_cal_ns,
        ),
    ];
    comps.extend(inp.extra.iter().cloned());
    let lines: Vec<BudgetLine> = comps.iter().map(BudgetLine::type_b).collect();
    let (uc, _) = combine(&lines);
    let k = if inp.k > 0.0 { inp.k } else { 2.0 };
    let target = if inp.target_ns > 0.0 {
        inp.target_ns
    } else {
        10.0
    };
    let dominant = lines
        .iter()
        .max_by(|a, b| {
            a.contribution_ns
                .partial_cmp(&b.contribution_ns)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|l| l.name.clone())
        .unwrap_or_default();
    SyncBudgetReport {
        lines,
        combined_u_ns: uc,
        k,
        expanded_u_ns: k * uc,
        target_ns: target,
        pass: k * uc <= target,
        margin_ns: target - k * uc,
        dominant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw::calib::Distribution;

    #[test]
    fn utc_stamp_parse_shift_and_gps_time() {
        let u = UtcStamp::parse("2026-11-18T23:59:59.95Z").unwrap();
        assert_eq!(u.plus(0.06).iso(), "2026-11-19T00:00:00.010000000Z");
        assert_eq!(u.plus(-86400.0).iso(), "2026-11-17T23:59:59.950000000Z");
        // GPS epoch is week 0, tow 0
        let g = UtcStamp::parse("1980-01-06T00:00:00Z").unwrap();
        assert_eq!(g.gps_week_tow(), (0, 0.0));
        // 2017-01-01T00:00:00 UTC = GPS week 1930, TOW 18 s (leap seconds 18)
        let t = UtcStamp::parse("2017-01-01T00:00:00Z").unwrap();
        let (w, tow) = t.gps_week_tow();
        assert_eq!(w, 1930);
        assert!((tow - 18.0).abs() < 1e-9);
    }

    #[test]
    fn epoch_fields_in_both_scales() {
        let e = StartEpoch::from_utc_iso("2026-11-18T00:00:00Z", EpochScale::Gpst).unwrap();
        let f = e.fields();
        assert_eq!(f["epoch_iso"], "2026-11-18T00:00:18.000000000Z");
        assert_eq!(f["utc_iso"], "2026-11-18T00:00:00.000000000Z");
        assert_eq!(f["leap_s"], "18");
        assert_eq!(f["date"], "2026-11-18");
        assert_eq!(f["time"], "00:00:18");
    }

    #[test]
    fn plan_applies_measured_offset_and_warns() {
        let m = SyncTimingModel {
            common_10mhz: true,
            common_pps: false,
            eloran_minus_gnss_ns: 4.2,
            eloran_minus_gnss_u_ns: 0.8,
            gnss_arm: ArmMode::NextPps,
            epoch_scale: EpochScale::Utc,
        };
        let p = m.plan("2026-11-18T00:00:00Z").unwrap();
        assert_eq!(p.eloran_start, StartTime::NextPps);
        assert_eq!(p.eloran_extra_delay_ns, 4.2);
        assert_eq!(p.warnings.len(), 1);
        assert!(m.plan("2026-11-18T00:00:00.5Z").unwrap().warnings.len() == 2);
    }

    #[test]
    fn budget_combines_and_checks_target() {
        let r = sync_budget(&SyncBudgetInputs {
            reference_distribution_ns: 1.0,
            trigger_latency_ns: 2.0,
            channel_delay_cal_ns: 2.0,
            gnss_sim_delay_cal_ns: 4.0,
            ..Default::default()
        });
        assert!((r.combined_u_ns - 5.0).abs() < 1e-12);
        assert!((r.expanded_u_ns - 10.0).abs() < 1e-12);
        assert!(r.pass);
        assert_eq!(r.dominant, "GNSS simulator delay calibration");
        let mut i = SyncBudgetInputs {
            gnss_sim_delay_cal_ns: 4.0,
            ..Default::default()
        };
        i.extra.push(UncertaintyComponent {
            name: "cable temp".into(),
            value_ns: 6.0,
            distribution: Distribution::Rectangular,
            sensitivity: 1.0,
        });
        let r = sync_budget(&i);
        assert!((r.combined_u_ns - (16.0f64 + 12.0).sqrt()).abs() < 1e-12);
        assert!(!r.pass);
    }
}
