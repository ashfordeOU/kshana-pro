// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! [`UhdDevice`]: Ettus Research USRP transmit and receive through the UHD 4.x **C API**
//! (`uhd.h`: `uhd/usrp/usrp.h`, `uhd/types/metadata.h`, `uhd/types/tune_request.h`), behind
//! the `uhd` feature (Unix).
//!
//! The UHD library is the user's own installation, opened at run time with `dlopen`
//! (`$KSHANA_UHD_LIB`, then `libuhd.so.4.x.y` for the 4.x releases, `libuhd.so`,
//! `libuhd.dylib`), so the crate has no build or link dependency. Function signatures are
//! those of the UHD 4.x C API headers.
//!
//! **Output policy: conducted use only — cables or antenna simulators, never over the
//! air.** The policy is printed when a device is opened.
//!
//! LF output: on the DC-coupled LFTX/LFRX daughterboards the real eLoran signal is sent on
//! I with Q = 0 at a centre frequency of 0 Hz (CPU format `fc32`, over-the-wire `sc16`);
//! receive takes I as the real LF signal.
//!
//! Clocking: `clock_source`/`time_source` "external" or "internal" per [`RefSource`] /
//! [`super::PpsSource`]; reference lock is read from the motherboard sensor `ref_locked`.
//! [`StartTime::NextPps`] calls `uhd_usrp_set_time_unknown_pps(0, 0)` (UHD waits for a PPS
//! edge and sets the time to 0 on the following one), then starts the stream at the next
//! whole second of device time at least 100 ms ahead: timed burst (`has_time_spec`,
//! `start_of_burst`) for transmit, a timed `START_CONTINUOUS` stream command for receive.
//! Underflows, sequence and time errors are read from the transmit async messages; receive
//! overflows and late commands from the receive metadata.
//!
//! Evidence: **UNTESTED ON HARDWARE** — compile-tested only; needs a USRP with LFTX/LFRX
//! (and a 10 MHz / 1PPS source) to verify output, PPS start, lock reporting and capture.

use super::dl::DynLib;
use super::{
    announce_output_policy, Device, DeviceConfig, DeviceStatus, PpsSource, RefSource, StartTime,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};

type Usrp = *mut c_void;
type TxStreamer = *mut c_void;
type RxStreamer = *mut c_void;
type TxMd = *mut c_void;
type RxMd = *mut c_void;
type AsyncMd = *mut c_void;
type Sensor = *mut c_void;
/// `uhd_error` (enum); `UHD_ERROR_NONE` = 0.
type UhdError = c_int;

/// `uhd_stream_args_t`.
#[repr(C)]
struct StreamArgs {
    cpu_format: *const c_char,
    otw_format: *const c_char,
    args: *const c_char,
    channel_list: *const usize,
    n_channels: c_int,
}

/// `uhd_stream_cmd_t`.
#[repr(C)]
struct StreamCmd {
    stream_mode: c_int,
    num_samps: usize,
    stream_now: bool,
    time_spec_full_secs: i64,
    time_spec_frac_secs: f64,
}

/// `uhd_tune_request_t`.
#[repr(C)]
struct TuneRequest {
    target_freq: f64,
    rf_freq_policy: c_int,
    rf_freq: f64,
    dsp_freq_policy: c_int,
    dsp_freq: f64,
    args: *const c_char,
}

/// `uhd_tune_result_t`.
#[repr(C)]
#[derive(Default)]
struct TuneResult {
    clipped_rf_freq: f64,
    target_rf_freq: f64,
    actual_rf_freq: f64,
    target_dsp_freq: f64,
    actual_dsp_freq: f64,
}

// uhd_stream_mode_t
const STREAM_MODE_START_CONTINUOUS: c_int = b'a' as c_int;
const STREAM_MODE_STOP_CONTINUOUS: c_int = b'o' as c_int;
// uhd_tune_request_policy_t
const TUNE_POLICY_AUTO: c_int = b'A' as c_int;
// uhd_async_metadata_event_code_t
const ASYNC_UNDERFLOW: c_int = 0x2;
const ASYNC_SEQ_ERROR: c_int = 0x4;
const ASYNC_TIME_ERROR: c_int = 0x8;
const ASYNC_UNDERFLOW_IN_PACKET: c_int = 0x10;
const ASYNC_SEQ_ERROR_IN_BURST: c_int = 0x20;
// uhd_rx_metadata_error_code_t
const RX_ERR_NONE: c_int = 0x0;
const RX_ERR_TIMEOUT: c_int = 0x1;
const RX_ERR_LATE: c_int = 0x2;
const RX_ERR_OVERFLOW: c_int = 0x8;

/// The UHD C functions used.
struct Api {
    usrp_free: unsafe extern "C" fn(*mut Usrp) -> UhdError,
    last_error: unsafe extern "C" fn(Usrp, *mut c_char, usize) -> UhdError,
    set_tx_rate: unsafe extern "C" fn(Usrp, f64, usize) -> UhdError,
    set_rx_rate: unsafe extern "C" fn(Usrp, f64, usize) -> UhdError,
    set_tx_gain: unsafe extern "C" fn(Usrp, f64, usize, *const c_char) -> UhdError,
    set_rx_gain: unsafe extern "C" fn(Usrp, f64, usize, *const c_char) -> UhdError,
    set_tx_freq:
        unsafe extern "C" fn(Usrp, *mut TuneRequest, usize, *mut TuneResult) -> UhdError,
    set_rx_freq:
        unsafe extern "C" fn(Usrp, *mut TuneRequest, usize, *mut TuneResult) -> UhdError,
    set_clock_source: unsafe extern "C" fn(Usrp, *const c_char, usize) -> UhdError,
    set_time_source: unsafe extern "C" fn(Usrp, *const c_char, usize) -> UhdError,
    set_time_unknown_pps: unsafe extern "C" fn(Usrp, i64, f64) -> UhdError,
    get_time_now: unsafe extern "C" fn(Usrp, usize, *mut i64, *mut f64) -> UhdError,
    get_mboard_sensor: unsafe extern "C" fn(Usrp, *const c_char, usize, *mut Sensor) -> UhdError,
    sensor_make: unsafe extern "C" fn(*mut Sensor) -> UhdError,
    sensor_free: unsafe extern "C" fn(*mut Sensor) -> UhdError,
    sensor_to_bool: unsafe extern "C" fn(Sensor, *mut bool) -> UhdError,
    tx_streamer_make: unsafe extern "C" fn(*mut TxStreamer) -> UhdError,
    tx_streamer_free: unsafe extern "C" fn(*mut TxStreamer) -> UhdError,
    get_tx_stream: unsafe extern "C" fn(Usrp, *mut StreamArgs, TxStreamer) -> UhdError,
    tx_send: unsafe extern "C" fn(
        TxStreamer,
        *const *const c_void,
        usize,
        *mut TxMd,
        f64,
        *mut usize,
    ) -> UhdError,
    tx_recv_async:
        unsafe extern "C" fn(TxStreamer, *mut AsyncMd, f64, *mut bool) -> UhdError,
    tx_md_make: unsafe extern "C" fn(*mut TxMd, bool, i64, f64, bool, bool) -> UhdError,
    tx_md_free: unsafe extern "C" fn(*mut TxMd) -> UhdError,
    async_md_make: unsafe extern "C" fn(*mut AsyncMd) -> UhdError,
    async_md_free: unsafe extern "C" fn(*mut AsyncMd) -> UhdError,
    async_md_event: unsafe extern "C" fn(AsyncMd, *mut c_int) -> UhdError,
    rx_streamer_make: unsafe extern "C" fn(*mut RxStreamer) -> UhdError,
    rx_streamer_free: unsafe extern "C" fn(*mut RxStreamer) -> UhdError,
    get_rx_stream: unsafe extern "C" fn(Usrp, *mut StreamArgs, RxStreamer) -> UhdError,
    rx_issue: unsafe extern "C" fn(RxStreamer, *const StreamCmd) -> UhdError,
    rx_recv: unsafe extern "C" fn(
        RxStreamer,
        *mut *mut c_void,
        usize,
        *mut RxMd,
        f64,
        bool,
        *mut usize,
    ) -> UhdError,
    rx_md_make: unsafe extern "C" fn(*mut RxMd) -> UhdError,
    rx_md_free: unsafe extern "C" fn(*mut RxMd) -> UhdError,
    rx_md_error: unsafe extern "C" fn(RxMd, *mut c_int) -> UhdError,
}

/// Library names tried after `$KSHANA_UHD_LIB` (UHD 4.x sonames carry the full version).
pub const UHD_LIBS: &[&str] = &[
    "libuhd.so.4.9.0",
    "libuhd.so.4.8.0",
    "libuhd.so.4.7.0",
    "libuhd.so.4.6.0",
    "libuhd.so.4.5.0",
    "libuhd.so.4.4.0",
    "libuhd.so.4.3.0",
    "libuhd.so.4.2.0",
    "libuhd.so.4.1.0",
    "libuhd.so.4.0.0",
    "libuhd.so.4",
    "libuhd.so",
    "libuhd.dylib",
];

/// A USRP opened through the UHD C API.
pub struct UhdDevice {
    api: Api,
    usrp: Usrp,
    args: String,
    cfg: Option<DeviceConfig>,
    tx: TxStreamer,
    rx: RxStreamer,
    tx_ready: bool,
    rx_ready: bool,
    tx_md: TxMd,
    rx_md: RxMd,
    async_md: AsyncMd,
    /// Device time (s) of the first transmit sample, consumed by the first send.
    tx_time: Option<(i64, f64)>,
    first_send: bool,
    status: DeviceStatus,
    _lib: DynLib,
}

// SAFETY: a UHD handle may be used from any thread as long as calls are not concurrent;
// `UhdDevice` is used only through `&mut self` (moved into one pump thread).
unsafe impl Send for UhdDevice {}

fn cstr(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|e| e.to_string())
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        // SAFETY: the field type is the C signature of this UHD 4.x C API function.
        unsafe { $lib.sym($name)? }
    };
}

impl UhdDevice {
    /// Open the USRP named by UHD device `args` (e.g. `type=x300,addr=192.168.40.2`).
    /// Prints the output policy.
    pub fn open(args: &str) -> Result<UhdDevice, String> {
        announce_output_policy(&format!("uhd:{args}"));
        let lib = DynLib::open(UHD_LIBS, Some("KSHANA_UHD_LIB"))
            .map_err(|e| format!("UHD: {e}"))?;
        let api = Api {
            usrp_free: sym!(lib, "uhd_usrp_free"),
            last_error: sym!(lib, "uhd_usrp_last_error"),
            set_tx_rate: sym!(lib, "uhd_usrp_set_tx_rate"),
            set_rx_rate: sym!(lib, "uhd_usrp_set_rx_rate"),
            set_tx_gain: sym!(lib, "uhd_usrp_set_tx_gain"),
            set_rx_gain: sym!(lib, "uhd_usrp_set_rx_gain"),
            set_tx_freq: sym!(lib, "uhd_usrp_set_tx_freq"),
            set_rx_freq: sym!(lib, "uhd_usrp_set_rx_freq"),
            set_clock_source: sym!(lib, "uhd_usrp_set_clock_source"),
            set_time_source: sym!(lib, "uhd_usrp_set_time_source"),
            set_time_unknown_pps: sym!(lib, "uhd_usrp_set_time_unknown_pps"),
            get_time_now: sym!(lib, "uhd_usrp_get_time_now"),
            get_mboard_sensor: sym!(lib, "uhd_usrp_get_mboard_sensor"),
            sensor_make: sym!(lib, "uhd_sensor_value_make"),
            sensor_free: sym!(lib, "uhd_sensor_value_free"),
            sensor_to_bool: sym!(lib, "uhd_sensor_value_to_bool"),
            tx_streamer_make: sym!(lib, "uhd_tx_streamer_make"),
            tx_streamer_free: sym!(lib, "uhd_tx_streamer_free"),
            get_tx_stream: sym!(lib, "uhd_usrp_get_tx_stream"),
            tx_send: sym!(lib, "uhd_tx_streamer_send"),
            tx_recv_async: sym!(lib, "uhd_tx_streamer_recv_async_msg"),
            tx_md_make: sym!(lib, "uhd_tx_metadata_make"),
            tx_md_free: sym!(lib, "uhd_tx_metadata_free"),
            async_md_make: sym!(lib, "uhd_async_metadata_make"),
            async_md_free: sym!(lib, "uhd_async_metadata_free"),
            async_md_event: sym!(lib, "uhd_async_metadata_event_code"),
            rx_streamer_make: sym!(lib, "uhd_rx_streamer_make"),
            rx_streamer_free: sym!(lib, "uhd_rx_streamer_free"),
            get_rx_stream: sym!(lib, "uhd_usrp_get_rx_stream"),
            rx_issue: sym!(lib, "uhd_rx_streamer_issue_stream_cmd"),
            rx_recv: sym!(lib, "uhd_rx_streamer_recv"),
            rx_md_make: sym!(lib, "uhd_rx_metadata_make"),
            rx_md_free: sym!(lib, "uhd_rx_metadata_free"),
            rx_md_error: sym!(lib, "uhd_rx_metadata_error_code"),
        };
        let make: unsafe extern "C" fn(*mut Usrp, *const c_char) -> UhdError =
            sym!(lib, "uhd_usrp_make");
        let a = cstr(args)?;
        let mut usrp: Usrp = std::ptr::null_mut();
        // SAFETY: valid out-pointer and C string.
        let r = unsafe { make(&mut usrp, a.as_ptr()) };
        if r != 0 || usrp.is_null() {
            return Err(format!("uhd:{args}: uhd_usrp_make failed ({r})"));
        }
        let mut d = UhdDevice {
            api,
            usrp,
            args: args.into(),
            cfg: None,
            tx: std::ptr::null_mut(),
            rx: std::ptr::null_mut(),
            tx_ready: false,
            rx_ready: false,
            tx_md: std::ptr::null_mut(),
            rx_md: std::ptr::null_mut(),
            async_md: std::ptr::null_mut(),
            tx_time: None,
            first_send: false,
            status: DeviceStatus::default(),
            _lib: lib,
        };
        // SAFETY: valid out-pointers for the handle constructors.
        unsafe {
            d.check((d.api.tx_streamer_make)(&mut d.tx), "tx_streamer_make")?;
            d.check((d.api.rx_streamer_make)(&mut d.rx), "rx_streamer_make")?;
            d.check((d.api.rx_md_make)(&mut d.rx_md), "rx_metadata_make")?;
            d.check((d.api.async_md_make)(&mut d.async_md), "async_metadata_make")?;
        }
        Ok(d)
    }

    fn err(&self, what: &str, code: UhdError) -> String {
        let mut buf = vec![0 as c_char; 512];
        // SAFETY: buffer of the stated length; UHD NUL-terminates.
        let msg = unsafe {
            (self.api.last_error)(self.usrp, buf.as_mut_ptr(), buf.len());
            CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
        };
        format!("uhd:{} {what} ({code}): {msg}", self.args)
    }

    fn check(&self, r: UhdError, what: &str) -> Result<(), String> {
        if r != 0 {
            Err(self.err(what, r))
        } else {
            Ok(())
        }
    }

    /// Reference lock from the `ref_locked` motherboard sensor (None if not available).
    pub fn ref_locked(&self) -> Option<bool> {
        let name = cstr("ref_locked").ok()?;
        let mut s: Sensor = std::ptr::null_mut();
        let mut v = false;
        // SAFETY: sensor handle created, used and freed here.
        unsafe {
            if (self.api.sensor_make)(&mut s) != 0 {
                return None;
            }
            let ok = (self.api.get_mboard_sensor)(self.usrp, name.as_ptr(), 0, &mut s) == 0
                && (self.api.sensor_to_bool)(s, &mut v) == 0;
            (self.api.sensor_free)(&mut s);
            ok.then_some(v)
        }
    }

    /// Resolve a start time to a device time (full s, frac s), arming the PPS when asked.
    fn start_time(&mut self, at: StartTime) -> Result<Option<(i64, f64)>, String> {
        match at {
            StartTime::Now => Ok(None),
            StartTime::AtDeviceTimeNs(t) => Ok(Some((
                t.div_euclid(1_000_000_000),
                t.rem_euclid(1_000_000_000) as f64 * 1e-9,
            ))),
            StartTime::NextPps => {
                // SAFETY: plain call on a valid handle; UHD blocks until the PPS edge.
                let r = unsafe { (self.api.set_time_unknown_pps)(self.usrp, 0, 0.0) };
                self.check(r, "set_time_unknown_pps")?;
                let (mut full, mut frac) = (0i64, 0f64);
                // SAFETY: valid out-pointers.
                let r = unsafe { (self.api.get_time_now)(self.usrp, 0, &mut full, &mut frac) };
                self.check(r, "get_time_now")?;
                let next = if frac < 0.9 { full + 1 } else { full + 2 };
                self.status.pps_aligned = true;
                Ok(Some((next, 0.0)))
            }
        }
    }

    fn stream_args(&self, n: usize, f: impl FnOnce(&mut StreamArgs) -> UhdError) -> UhdError {
        let cpu = CString::new("fc32").expect("static");
        let otw = CString::new("sc16").expect("static");
        let args = CString::new("").expect("static");
        let ch: Vec<usize> = (0..n).collect();
        let mut sa = StreamArgs {
            cpu_format: cpu.as_ptr(),
            otw_format: otw.as_ptr(),
            args: args.as_ptr(),
            channel_list: ch.as_ptr(),
            n_channels: n as c_int,
        };
        f(&mut sa)
    }

    fn poll_async(&mut self) {
        loop {
            let mut valid = false;
            // SAFETY: handles valid; zero timeout.
            let r = unsafe { (self.api.tx_recv_async)(self.tx, &mut self.async_md, 0.0, &mut valid) };
            if r != 0 || !valid {
                break;
            }
            let mut code: c_int = 0;
            // SAFETY: valid metadata handle and out-pointer.
            unsafe { (self.api.async_md_event)(self.async_md, &mut code) };
            if code & (ASYNC_UNDERFLOW | ASYNC_UNDERFLOW_IN_PACKET) != 0 {
                self.status.underruns += 1;
            }
            if code & ASYNC_TIME_ERROR != 0 {
                self.status.late += 1;
            }
            if code & (ASYNC_SEQ_ERROR | ASYNC_SEQ_ERROR_IN_BURST) != 0 {
                self.status.message = "sequence error on transmit".into();
            }
        }
    }

    fn send(&mut self, bufs: &[Vec<f32>], sob: bool, eob: bool) -> Result<usize, String> {
        let n = bufs.first().map(|b| b.len() / 2).unwrap_or(0);
        let mut done = 0;
        let mut first = true;
        loop {
            let (has_t, full, frac) = match (first, self.tx_time.take()) {
                (true, Some((f, x))) => (true, f, x),
                _ => (false, 0, 0.0),
            };
            if !self.tx_md.is_null() {
                // SAFETY: frees the previous metadata handle.
                unsafe { (self.api.tx_md_free)(&mut self.tx_md) };
            }
            let s = sob && first;
            let e = eob;
            // SAFETY: valid out-pointer.
            let r = unsafe { (self.api.tx_md_make)(&mut self.tx_md, has_t, full, frac, s, e) };
            self.check(r, "tx_metadata_make")?;
            let ptrs: Vec<*const c_void> = bufs
                .iter()
                .map(|b| b[done * 2..].as_ptr() as *const c_void)
                .collect();
            let mut sent = 0usize;
            // SAFETY: one buffer per channel holding at least `n − done` complex samples.
            let r = unsafe {
                (self.api.tx_send)(self.tx, ptrs.as_ptr(), n - done, &mut self.tx_md, 1.0, &mut sent)
            };
            self.check(r, "tx_streamer_send")?;
            done += sent;
            first = false;
            if done >= n {
                break;
            }
            if sent == 0 {
                self.status.underruns += 1; // timeout: device not accepting data
            }
        }
        Ok(done)
    }
}

impl Drop for UhdDevice {
    fn drop(&mut self) {
        // SAFETY: each handle is freed once; null handles are skipped.
        unsafe {
            if !self.tx_md.is_null() {
                (self.api.tx_md_free)(&mut self.tx_md);
            }
            if !self.rx_md.is_null() {
                (self.api.rx_md_free)(&mut self.rx_md);
            }
            if !self.async_md.is_null() {
                (self.api.async_md_free)(&mut self.async_md);
            }
            if !self.tx.is_null() {
                (self.api.tx_streamer_free)(&mut self.tx);
            }
            if !self.rx.is_null() {
                (self.api.rx_streamer_free)(&mut self.rx);
            }
            (self.api.usrp_free)(&mut self.usrp);
        }
    }
}

impl Device for UhdDevice {
    fn name(&self) -> String {
        format!("uhd:{}", self.args)
    }
    fn realtime(&self) -> bool {
        true
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        cfg.validate()?;
        let clock = cstr(match cfg.clock_source {
            RefSource::Internal => "internal",
            RefSource::External => "external",
        })?;
        let time = cstr(match cfg.time_source {
            PpsSource::Internal => "internal",
            PpsSource::External => "external",
        })?;
        let empty = cstr("")?;
        // SAFETY: valid handle and C strings; tune request/result live across the calls.
        unsafe {
            self.check(
                (self.api.set_clock_source)(self.usrp, clock.as_ptr(), 0),
                "set_clock_source",
            )?;
            self.check(
                (self.api.set_time_source)(self.usrp, time.as_ptr(), 0),
                "set_time_source",
            )?;
            for c in 0..cfg.tx_channels {
                self.check(
                    (self.api.set_tx_rate)(self.usrp, cfg.sample_rate_hz, c),
                    "set_tx_rate",
                )?;
                let mut tr = TuneRequest {
                    target_freq: cfg.center_freq_hz,
                    rf_freq_policy: TUNE_POLICY_AUTO,
                    rf_freq: 0.0,
                    dsp_freq_policy: TUNE_POLICY_AUTO,
                    dsp_freq: 0.0,
                    args: empty.as_ptr(),
                };
                let mut res = TuneResult::default();
                self.check(
                    (self.api.set_tx_freq)(self.usrp, &mut tr, c, &mut res),
                    "set_tx_freq",
                )?;
                if let Some(g) = cfg.tx_gain_db.get(c) {
                    self.check(
                        (self.api.set_tx_gain)(self.usrp, *g, c, empty.as_ptr()),
                        "set_tx_gain",
                    )?;
                }
            }
            for c in 0..cfg.rx_channels {
                self.check(
                    (self.api.set_rx_rate)(self.usrp, cfg.sample_rate_hz, c),
                    "set_rx_rate",
                )?;
                let mut tr = TuneRequest {
                    target_freq: cfg.center_freq_hz,
                    rf_freq_policy: TUNE_POLICY_AUTO,
                    rf_freq: 0.0,
                    dsp_freq_policy: TUNE_POLICY_AUTO,
                    dsp_freq: 0.0,
                    args: empty.as_ptr(),
                };
                let mut res = TuneResult::default();
                self.check(
                    (self.api.set_rx_freq)(self.usrp, &mut tr, c, &mut res),
                    "set_rx_freq",
                )?;
                if let Some(g) = cfg.rx_gain_db.get(c) {
                    self.check(
                        (self.api.set_rx_gain)(self.usrp, *g, c, empty.as_ptr()),
                        "set_rx_gain",
                    )?;
                }
            }
        }
        self.cfg = Some(cfg.clone());
        self.status.message = format!("configured fs={}", cfg.sample_rate_hz);
        Ok(())
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.cfg.as_ref().map(|c| c.sample_rate_hz)
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let n = self.cfg.as_ref().ok_or("device not configured")?.tx_channels;
        if n == 0 {
            return Err("no tx channels configured".into());
        }
        if !self.tx_ready {
            let (usrp, tx, f) = (self.usrp, self.tx, self.api.get_tx_stream);
            // SAFETY: stream args valid for the duration of the call.
            let r = self.stream_args(n, |sa| unsafe { f(usrp, sa, tx) });
            self.check(r, "get_tx_stream")?;
            self.tx_ready = true;
        }
        let t = self.start_time(at)?;
        self.tx_time = t;
        self.first_send = true;
        let ns = t.map(|(f, x)| f * 1_000_000_000 + (x * 1e9).round() as i64);
        self.status.tx_streaming = true;
        self.status.start_time_ns = ns;
        Ok(ns)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        if !self.status.tx_streaming {
            return Err("transmit stream not started".into());
        }
        let n = block.first().map(|c| c.len()).unwrap_or(0);
        // fc32 interleaved I/Q, real signal on I, Q = 0
        let bufs: Vec<Vec<f32>> = block
            .iter()
            .map(|ch| ch.iter().flat_map(|&v| [v as f32, 0.0]).collect())
            .collect();
        let sob = self.first_send;
        self.first_send = false;
        self.send(&bufs, sob, false)?;
        self.status.samples_tx += n as u64;
        self.poll_async();
        Ok(())
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        if !self.status.tx_streaming {
            return Ok(());
        }
        let n = self.cfg.as_ref().map(|c| c.tx_channels).unwrap_or(1);
        let z: Vec<Vec<f32>> = vec![vec![0.0; 2]; n];
        let _ = self.send(&z, false, true);
        self.poll_async();
        self.status.tx_streaming = false;
        self.status.ref_locked = self.ref_locked();
        Ok(())
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let n = self.cfg.as_ref().ok_or("device not configured")?.rx_channels;
        if n == 0 {
            return Err("no rx channels configured".into());
        }
        if !self.rx_ready {
            let (usrp, rx, f) = (self.usrp, self.rx, self.api.get_rx_stream);
            // SAFETY: stream args valid for the duration of the call.
            let r = self.stream_args(n, |sa| unsafe { f(usrp, sa, rx) });
            self.check(r, "get_rx_stream")?;
            self.rx_ready = true;
        }
        let t = self.start_time(at)?;
        let cmd = StreamCmd {
            stream_mode: STREAM_MODE_START_CONTINUOUS,
            num_samps: 0,
            stream_now: t.is_none(),
            time_spec_full_secs: t.map(|x| x.0).unwrap_or(0),
            time_spec_frac_secs: t.map(|x| x.1).unwrap_or(0.0),
        };
        // SAFETY: valid streamer and command.
        let r = unsafe { (self.api.rx_issue)(self.rx, &cmd) };
        self.check(r, "issue_stream_cmd")?;
        let ns = t.map(|(f, x)| f * 1_000_000_000 + (x * 1e9).round() as i64);
        self.status.rx_streaming = true;
        self.status.start_time_ns = ns;
        Ok(ns)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        if !self.status.rx_streaming {
            return Err("receive stream not started".into());
        }
        let nch = self.cfg.as_ref().map(|c| c.rx_channels).unwrap_or(1);
        let mut bufs: Vec<Vec<f32>> = vec![vec![0.0; 2 * n]; nch];
        let mut ptrs: Vec<*mut c_void> = bufs
            .iter_mut()
            .map(|b| b.as_mut_ptr() as *mut c_void)
            .collect();
        let mut got = 0usize;
        // SAFETY: one buffer per channel with room for `n` complex samples.
        let r = unsafe {
            (self.api.rx_recv)(self.rx, ptrs.as_mut_ptr(), n, &mut self.rx_md, 3.0, false, &mut got)
        };
        self.check(r, "rx_streamer_recv")?;
        let mut code: c_int = RX_ERR_NONE;
        // SAFETY: valid metadata handle and out-pointer.
        unsafe { (self.api.rx_md_error)(self.rx_md, &mut code) };
        match code {
            RX_ERR_NONE | RX_ERR_TIMEOUT => {}
            RX_ERR_OVERFLOW => self.status.overflows += 1,
            RX_ERR_LATE => self.status.late += 1,
            c => return Err(format!("uhd:{} receive error code {c:#x}", self.args)),
        }
        self.status.samples_rx += got as u64;
        Ok(bufs
            .iter()
            .map(|b| (0..got).map(|i| b[2 * i] as f64).collect())
            .collect())
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        if self.status.rx_streaming {
            let cmd = StreamCmd {
                stream_mode: STREAM_MODE_STOP_CONTINUOUS,
                num_samps: 0,
                stream_now: true,
                time_spec_full_secs: 0,
                time_spec_frac_secs: 0.0,
            };
            // SAFETY: valid streamer and command.
            unsafe { (self.api.rx_issue)(self.rx, &cmd) };
        }
        self.status.rx_streaming = false;
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        let mut s = self.status.clone();
        if s.ref_locked.is_none() {
            if let Some(c) = &self.cfg {
                if c.clock_source == RefSource::External {
                    s.ref_locked = self.ref_locked();
                }
            }
        }
        s
    }
}
