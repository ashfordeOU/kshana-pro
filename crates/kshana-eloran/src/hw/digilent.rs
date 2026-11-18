// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! [`DigilentDevice`]: Digilent Analog Discovery 2 / 3 through the WaveForms SDK `dwf` C API
//! (`dwf.h`), behind the `digilent` feature (Unix).
//!
//! The WaveForms runtime is the user's own installation, opened at run time with `dlopen`
//! (`$KSHANA_DWF_LIB`, `libdwf.so`, `libdwf.so.3`, the macOS framework path), so the crate
//! has no link dependency.
//!
//! * **Transmit, streaming** ([`Device`]): AnalogOut *play* mode (`funcPlay`): the node
//!   frequency is the sample rate, the buffer is prefilled with `FDwfAnalogOutNodeDataSet`,
//!   the channel started with `FDwfAnalogOutConfigure`, then fed with
//!   `FDwfAnalogOutNodePlayStatus` / `FDwfAnalogOutNodePlayData`; lost or corrupted samples
//!   count as underruns. Sustained play rates are limited by USB (well below the 100 MS/s
//!   DAC rate); the achievable rate needs to be established on the actual host.
//! * **Transmit, segment** ([`SegmentPlayer`]): AnalogOut *custom* mode (`funcCustom`): the
//!   segment is the custom table, the node frequency is sample rate / points (one table
//!   per period), repeated forever (periodic) or once. The table size is device- and
//!   configuration-dependent (`FDwfAnalogOutNodeDataInfo`, checked).
//! * **Receive** — AnalogIn *record* mode (`acqmodeRecord`, record length 0 = unlimited),
//!   `FDwfAnalogInStatusRecord` / `FDwfAnalogInStatusData`; lost or corrupted samples count
//!   as overflows.
//! * **Start** — [`StartTime::NextPps`] selects the external trigger input 1 (`trigsrcExternal1`)
//!   as the start trigger: wire the reference 1PPS to trigger input T1. `Now` starts on the
//!   PC command.
//! * **Clock** — the Analog Discovery 2 has no external 10 MHz reference input;
//!   [`RefSource::External`] is refused (NEEDS CONFIRMATION for the Analog Discovery 3's
//!   clocking options).
//!
//! Levels: ±1.0 full scale maps to ±`full_scale_v` volts on the output (AWG range ±5 V)
//! and an input range of ±`in_full_scale_v` volts.
//!
//! **Output policy: conducted use only — cables or antenna simulators, never over the
//! air.** Printed when the device is opened.
//!
//! Evidence: **UNTESTED ON HARDWARE** — compile-tested only.

use super::awg::{AwgSegment, SegmentPlayer};
use super::dl::DynLib;
use super::{announce_output_policy, Device, DeviceConfig, DeviceStatus, RefSource, StartTime};
use std::ffi::{c_char, c_double, c_int, c_uchar, CStr};
use std::time::{Duration, Instant};

type Hdwf = c_int;
type Bool = c_int;

const NODE_CARRIER: c_int = 0;
const FUNC_CUSTOM: c_uchar = 30;
const FUNC_PLAY: c_uchar = 31;
const TRIGSRC_NONE: c_uchar = 0;
const TRIGSRC_EXTERNAL1: c_uchar = 11;
const ACQMODE_RECORD: c_int = 3;

/// The dwf functions used (signatures from `dwf.h`).
struct Api {
    last_error: unsafe extern "C" fn(*mut c_char) -> Bool,
    close: unsafe extern "C" fn(Hdwf) -> Bool,
    auto_configure: unsafe extern "C" fn(Hdwf, c_int) -> Bool,
    out_enable: unsafe extern "C" fn(Hdwf, c_int, c_int, Bool) -> Bool,
    out_function: unsafe extern "C" fn(Hdwf, c_int, c_int, c_uchar) -> Bool,
    out_frequency: unsafe extern "C" fn(Hdwf, c_int, c_int, c_double) -> Bool,
    out_amplitude: unsafe extern "C" fn(Hdwf, c_int, c_int, c_double) -> Bool,
    out_offset: unsafe extern "C" fn(Hdwf, c_int, c_int, c_double) -> Bool,
    out_data_set: unsafe extern "C" fn(Hdwf, c_int, c_int, *const c_double, c_int) -> Bool,
    out_data_info: unsafe extern "C" fn(Hdwf, c_int, c_int, *mut c_int, *mut c_int) -> Bool,
    out_play_status:
        unsafe extern "C" fn(Hdwf, c_int, c_int, *mut c_int, *mut c_int, *mut c_int) -> Bool,
    out_play_data: unsafe extern "C" fn(Hdwf, c_int, c_int, *const c_double, c_int) -> Bool,
    out_trigger_source: unsafe extern "C" fn(Hdwf, c_int, c_uchar) -> Bool,
    out_run_set: unsafe extern "C" fn(Hdwf, c_int, c_double) -> Bool,
    out_repeat_set: unsafe extern "C" fn(Hdwf, c_int, c_int) -> Bool,
    out_configure: unsafe extern "C" fn(Hdwf, c_int, Bool) -> Bool,
    in_enable: unsafe extern "C" fn(Hdwf, c_int, Bool) -> Bool,
    in_range: unsafe extern "C" fn(Hdwf, c_int, c_double) -> Bool,
    in_mode: unsafe extern "C" fn(Hdwf, c_int) -> Bool,
    in_frequency: unsafe extern "C" fn(Hdwf, c_double) -> Bool,
    in_record_length: unsafe extern "C" fn(Hdwf, c_double) -> Bool,
    in_trigger_source: unsafe extern "C" fn(Hdwf, c_uchar) -> Bool,
    in_configure: unsafe extern "C" fn(Hdwf, Bool, Bool) -> Bool,
    in_status: unsafe extern "C" fn(Hdwf, Bool, *mut c_uchar) -> Bool,
    in_status_record: unsafe extern "C" fn(Hdwf, *mut c_int, *mut c_int, *mut c_int) -> Bool,
    in_status_data: unsafe extern "C" fn(Hdwf, c_int, *mut c_double, c_int) -> Bool,
}

/// Library names tried after `$KSHANA_DWF_LIB`.
pub const DWF_LIBS: &[&str] = &[
    "libdwf.so",
    "libdwf.so.3",
    "/Library/Frameworks/dwf.framework/dwf",
];

/// An Analog Discovery 2/3.
pub struct DigilentDevice {
    api: Api,
    hdwf: Hdwf,
    index: i32,
    /// Output volts for ±1.0.
    pub full_scale_v: f64,
    /// Input volts for ±1.0.
    pub in_full_scale_v: f64,
    /// Samples prefilled before the play-mode start.
    pub prefill: usize,
    cfg: Option<DeviceConfig>,
    pending: Vec<Vec<f64>>,
    started: bool,
    at: StartTime,
    status: DeviceStatus,
    _lib: DynLib,
}

// SAFETY: an HDWF handle is an integer; calls are serialised through `&mut self`.
unsafe impl Send for DigilentDevice {}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        // SAFETY: the field type is the C signature of this dwf.h function.
        unsafe { $lib.sym($name)? }
    };
}

impl DigilentDevice {
    /// Open device `index` (−1 = first available). Prints the output policy.
    pub fn open(index: i32) -> Result<DigilentDevice, String> {
        announce_output_policy(&format!("digilent:{index}"));
        let lib = DynLib::open(DWF_LIBS, Some("KSHANA_DWF_LIB"))
            .map_err(|e| format!("WaveForms SDK: {e}"))?;
        let api = Api {
            last_error: sym!(lib, "FDwfGetLastErrorMsg"),
            close: sym!(lib, "FDwfDeviceClose"),
            auto_configure: sym!(lib, "FDwfDeviceAutoConfigureSet"),
            out_enable: sym!(lib, "FDwfAnalogOutNodeEnableSet"),
            out_function: sym!(lib, "FDwfAnalogOutNodeFunctionSet"),
            out_frequency: sym!(lib, "FDwfAnalogOutNodeFrequencySet"),
            out_amplitude: sym!(lib, "FDwfAnalogOutNodeAmplitudeSet"),
            out_offset: sym!(lib, "FDwfAnalogOutNodeOffsetSet"),
            out_data_set: sym!(lib, "FDwfAnalogOutNodeDataSet"),
            out_data_info: sym!(lib, "FDwfAnalogOutNodeDataInfo"),
            out_play_status: sym!(lib, "FDwfAnalogOutNodePlayStatus"),
            out_play_data: sym!(lib, "FDwfAnalogOutNodePlayData"),
            out_trigger_source: sym!(lib, "FDwfAnalogOutTriggerSourceSet"),
            out_run_set: sym!(lib, "FDwfAnalogOutRunSet"),
            out_repeat_set: sym!(lib, "FDwfAnalogOutRepeatSet"),
            out_configure: sym!(lib, "FDwfAnalogOutConfigure"),
            in_enable: sym!(lib, "FDwfAnalogInChannelEnableSet"),
            in_range: sym!(lib, "FDwfAnalogInChannelRangeSet"),
            in_mode: sym!(lib, "FDwfAnalogInAcquisitionModeSet"),
            in_frequency: sym!(lib, "FDwfAnalogInFrequencySet"),
            in_record_length: sym!(lib, "FDwfAnalogInRecordLengthSet"),
            in_trigger_source: sym!(lib, "FDwfAnalogInTriggerSourceSet"),
            in_configure: sym!(lib, "FDwfAnalogInConfigure"),
            in_status: sym!(lib, "FDwfAnalogInStatus"),
            in_status_record: sym!(lib, "FDwfAnalogInStatusRecord"),
            in_status_data: sym!(lib, "FDwfAnalogInStatusData"),
        };
        let open: unsafe extern "C" fn(c_int, *mut Hdwf) -> Bool = sym!(lib, "FDwfDeviceOpen");
        let mut h: Hdwf = 0;
        // SAFETY: valid out-pointer.
        let ok = unsafe { open(index, &mut h) };
        let d = DigilentDevice {
            api,
            hdwf: h,
            index,
            full_scale_v: 1.0,
            in_full_scale_v: 2.5,
            prefill: 4096,
            cfg: None,
            pending: vec![],
            started: false,
            at: StartTime::Now,
            status: DeviceStatus::default(),
            _lib: lib,
        };
        if ok == 0 || h == 0 {
            return Err(d.err("FDwfDeviceOpen"));
        }
        // apply settings only on Configure calls
        // SAFETY: valid handle.
        unsafe { (d.api.auto_configure)(h, 0) };
        Ok(d)
    }

    fn err(&self, what: &str) -> String {
        let mut buf = vec![0 as c_char; 512];
        // SAFETY: 512-byte buffer as required by FDwfGetLastErrorMsg.
        let msg = unsafe {
            (self.api.last_error)(buf.as_mut_ptr());
            CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
        };
        format!("digilent:{} {what}: {}", self.index, msg.trim())
    }

    fn ok(&self, r: Bool, what: &str) -> Result<(), String> {
        if r == 0 {
            Err(self.err(what))
        } else {
            Ok(())
        }
    }

    fn trig(at: StartTime) -> c_uchar {
        match at {
            StartTime::Now => TRIGSRC_NONE,
            _ => TRIGSRC_EXTERNAL1,
        }
    }

    fn setup_out(&self, ch: c_int, func: c_uchar, freq: f64, at: StartTime) -> Result<(), String> {
        let h = self.hdwf;
        let a = &self.api;
        // SAFETY: valid handle; scalar arguments.
        unsafe {
            self.ok((a.out_enable)(h, ch, NODE_CARRIER, 1), "AnalogOutNodeEnableSet")?;
            self.ok((a.out_function)(h, ch, NODE_CARRIER, func), "AnalogOutNodeFunctionSet")?;
            self.ok((a.out_frequency)(h, ch, NODE_CARRIER, freq), "AnalogOutNodeFrequencySet")?;
            self.ok(
                (a.out_amplitude)(h, ch, NODE_CARRIER, self.full_scale_v),
                "AnalogOutNodeAmplitudeSet",
            )?;
            self.ok((a.out_offset)(h, ch, NODE_CARRIER, 0.0), "AnalogOutNodeOffsetSet")?;
            self.ok((a.out_trigger_source)(h, ch, Self::trig(at)), "AnalogOutTriggerSourceSet")?;
        }
        Ok(())
    }

    fn feed(&mut self) -> Result<(), String> {
        let (h, a) = (self.hdwf, &self.api);
        let t0 = Instant::now();
        while self.pending.first().map(|p| !p.is_empty()).unwrap_or(false) {
            let mut free_min = c_int::MAX;
            for c in 0..self.pending.len() {
                let (mut free, mut lost, mut corrupt) = (0, 0, 0);
                // SAFETY: valid out-pointers.
                let r = unsafe {
                    (a.out_play_status)(h, c as c_int, NODE_CARRIER, &mut free, &mut lost, &mut corrupt)
                };
                if r == 0 {
                    return Err(self.err("AnalogOutNodePlayStatus"));
                }
                if lost > 0 || corrupt > 0 {
                    self.status.underruns += 1;
                }
                free_min = free_min.min(free);
            }
            let k = (free_min.max(0) as usize).min(self.pending[0].len());
            if k == 0 {
                if t0.elapsed() > Duration::from_secs(5) {
                    return Err("digilent: play buffer not draining (device stopped?)".into());
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            for c in 0..self.pending.len() {
                let d: Vec<f64> = self.pending[c].drain(..k).collect();
                // SAFETY: `d` holds `k` doubles.
                let r = unsafe {
                    (a.out_play_data)(h, c as c_int, NODE_CARRIER, d.as_ptr(), k as c_int)
                };
                if r == 0 {
                    return Err(self.err("AnalogOutNodePlayData"));
                }
            }
        }
        Ok(())
    }

    fn begin_play(&mut self) -> Result<(), String> {
        let n = self.pending.first().map(|p| p.len()).unwrap_or(0).min(self.prefill);
        for c in 0..self.pending.len() {
            let d: Vec<f64> = self.pending[c].drain(..n).collect();
            // SAFETY: `d` holds `n` doubles.
            let r = unsafe {
                (self.api.out_data_set)(self.hdwf, c as c_int, NODE_CARRIER, d.as_ptr(), n as c_int)
            };
            self.ok(r, "AnalogOutNodeDataSet")?;
        }
        for c in 0..self.pending.len() {
            // SAFETY: valid handle.
            let r = unsafe { (self.api.out_configure)(self.hdwf, c as c_int, 1) };
            self.ok(r, "AnalogOutConfigure")?;
        }
        self.started = true;
        Ok(())
    }
}

impl Drop for DigilentDevice {
    fn drop(&mut self) {
        // SAFETY: closes the handle once.
        unsafe { (self.api.close)(self.hdwf) };
    }
}

impl SegmentPlayer for DigilentDevice {
    fn play_segment(&mut self, seg: &AwgSegment, at: StartTime) -> Result<(), String> {
        let n = seg.report.samples as usize;
        for c in 0..seg.codes.len() {
            let ch = c as c_int;
            let (mut lo, mut hi) = (0, 0);
            // SAFETY: valid out-pointers.
            let r = unsafe {
                (self.api.out_data_info)(self.hdwf, ch, NODE_CARRIER, &mut lo, &mut hi)
            };
            self.ok(r, "AnalogOutNodeDataInfo")?;
            if n < lo.max(1) as usize || n > hi as usize {
                return Err(format!(
                    "segment of {n} points outside the custom table limits {lo}..={hi}"
                ));
            }
            self.setup_out(ch, FUNC_CUSTOM, seg.report.sample_rate_hz / n as f64, at)?;
            let d = seg.normalised(c);
            let periodic = seg.report.periodic;
            // SAFETY: `d` holds `n` doubles; scalar arguments otherwise.
            unsafe {
                self.ok(
                    (self.api.out_data_set)(self.hdwf, ch, NODE_CARRIER, d.as_ptr(), n as c_int),
                    "AnalogOutNodeDataSet",
                )?;
                let run = if periodic { 0.0 } else { seg.report.duration_s };
                self.ok((self.api.out_run_set)(self.hdwf, ch, run), "AnalogOutRunSet")?;
                self.ok(
                    (self.api.out_repeat_set)(self.hdwf, ch, if periodic { 0 } else { 1 }),
                    "AnalogOutRepeatSet",
                )?;
                self.ok((self.api.out_configure)(self.hdwf, ch, 1), "AnalogOutConfigure")?;
            }
        }
        self.status.tx_streaming = true;
        self.status.pps_aligned = at == StartTime::NextPps;
        Ok(())
    }
    fn stop_playback(&mut self) -> Result<(), String> {
        let n = self.cfg.as_ref().map(|c| c.tx_channels).unwrap_or(2).max(1);
        for c in 0..n {
            // SAFETY: valid handle.
            unsafe {
                (self.api.out_configure)(self.hdwf, c as c_int, 0);
                (self.api.out_enable)(self.hdwf, c as c_int, NODE_CARRIER, 0);
            }
        }
        self.status.tx_streaming = false;
        Ok(())
    }
}

impl Device for DigilentDevice {
    fn name(&self) -> String {
        format!("digilent:{}", self.index)
    }
    fn realtime(&self) -> bool {
        true
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        cfg.validate()?;
        if cfg.tx_channels > 2 || cfg.rx_channels > 2 {
            return Err("the Analog Discovery has two AWG and two scope channels".into());
        }
        if cfg.clock_source == RefSource::External {
            return Err("no external 10 MHz reference input on this device".into());
        }
        self.cfg = Some(cfg.clone());
        Ok(())
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.cfg.as_ref().map(|c| c.sample_rate_hz)
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let cfg = self.cfg.clone().ok_or("device not configured")?;
        if cfg.tx_channels == 0 {
            return Err("no tx channels configured".into());
        }
        for c in 0..cfg.tx_channels {
            self.setup_out(c as c_int, FUNC_PLAY, cfg.sample_rate_hz, at)?;
        }
        self.pending = vec![Vec::new(); cfg.tx_channels];
        self.started = false;
        self.at = at;
        self.status.tx_streaming = true;
        self.status.pps_aligned = at == StartTime::NextPps;
        Ok(None)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        if !self.status.tx_streaming || block.len() != self.pending.len() {
            return Err("transmit stream not started or channel count differs".into());
        }
        for (p, x) in self.pending.iter_mut().zip(block) {
            p.extend(x.iter().map(|v| v.clamp(-1.0, 1.0)));
        }
        self.status.samples_tx += block.first().map(|c| c.len()).unwrap_or(0) as u64;
        if !self.started {
            if self.pending[0].len() < self.prefill {
                return Ok(());
            }
            self.begin_play()?;
        }
        self.feed()
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        if self.status.tx_streaming {
            if !self.started && !self.pending.is_empty() && !self.pending[0].is_empty() {
                self.begin_play()?;
            }
            if self.started {
                self.feed()?;
            }
        }
        self.status.tx_streaming = false;
        let _ = self.at;
        Ok(())
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let cfg = self.cfg.clone().ok_or("device not configured")?;
        if cfg.rx_channels == 0 {
            return Err("no rx channels configured".into());
        }
        let (h, a) = (self.hdwf, &self.api);
        // SAFETY: valid handle; scalar arguments.
        unsafe {
            for c in 0..cfg.rx_channels {
                self.ok((a.in_enable)(h, c as c_int, 1), "AnalogInChannelEnableSet")?;
                self.ok(
                    (a.in_range)(h, c as c_int, 2.0 * self.in_full_scale_v),
                    "AnalogInChannelRangeSet",
                )?;
            }
            self.ok((a.in_mode)(h, ACQMODE_RECORD), "AnalogInAcquisitionModeSet")?;
            self.ok((a.in_frequency)(h, cfg.sample_rate_hz), "AnalogInFrequencySet")?;
            self.ok((a.in_record_length)(h, 0.0), "AnalogInRecordLengthSet")?;
            self.ok((a.in_trigger_source)(h, Self::trig(at)), "AnalogInTriggerSourceSet")?;
            self.ok((a.in_configure)(h, 0, 1), "AnalogInConfigure")?;
        }
        self.status.rx_streaming = true;
        self.status.pps_aligned = at == StartTime::NextPps;
        Ok(None)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        if !self.status.rx_streaming {
            return Err("receive stream not started".into());
        }
        let nch = self.cfg.as_ref().map(|c| c.rx_channels).unwrap_or(1);
        let mut out = vec![Vec::with_capacity(n); nch];
        let t0 = Instant::now();
        while out[0].len() < n {
            let mut st: c_uchar = 0;
            let (mut avail, mut lost, mut corrupt) = (0, 0, 0);
            // SAFETY: valid out-pointers.
            unsafe {
                self.ok((self.api.in_status)(self.hdwf, 1, &mut st), "AnalogInStatus")?;
                self.ok(
                    (self.api.in_status_record)(self.hdwf, &mut avail, &mut lost, &mut corrupt),
                    "AnalogInStatusRecord",
                )?;
            }
            if lost > 0 || corrupt > 0 {
                self.status.overflows += 1;
            }
            if avail <= 0 {
                if !out[0].is_empty() || t0.elapsed() > Duration::from_secs(5) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            let k = avail as usize;
            for (c, o) in out.iter_mut().enumerate() {
                let mut buf = vec![0.0f64; k];
                // SAFETY: `buf` holds `k` doubles.
                let r = unsafe {
                    (self.api.in_status_data)(self.hdwf, c as c_int, buf.as_mut_ptr(), k as c_int)
                };
                self.ok(r, "AnalogInStatusData")?;
                o.extend(buf.iter().map(|v| v / self.in_full_scale_v));
            }
        }
        // samples beyond `n` are kept for the next call would need buffering; return all
        self.status.samples_rx += out[0].len() as u64;
        Ok(out)
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        // SAFETY: valid handle.
        unsafe { (self.api.in_configure)(self.hdwf, 0, 0) };
        self.status.rx_streaming = false;
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        self.status.clone()
    }
}
