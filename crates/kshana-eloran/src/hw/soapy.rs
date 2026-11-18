// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! [`SoapyDevice`]: SoapySDR 0.8 transmit and receive, behind the `soapy` feature (Unix).
//!
//! The SoapySDR library and its device modules are the user's own installation, opened at
//! run time with `dlopen` (`libSoapySDR.so.0.8`, `libSoapySDR.so`, `libSoapySDR.dylib`), so
//! the crate has no build or link dependency. Calls follow the SoapySDR 0.8 C API
//! (`SoapySDR/Device.h`); another ABI version is refused.
//!
//! **Output policy: conducted use only — cables or antenna simulators, never over the
//! air.** The policy is printed when a device is opened.
//!
//! LF output: on DC-coupled LF daughterboards (e.g. LFTX/LFRX, BasicTX/RX) the real eLoran
//! signal is sent on I with Q = 0 at a centre frequency of 0 Hz, in stream format `CF32`
//! or `CS16`. Receive takes the I component as the real LF signal.
//!
//! PPS start: [`StartTime::NextPps`] sets the device time to 0 on the next PPS edge
//! (`setHardwareTime(0, "PPS")`), waits for it to apply, then starts the stream at the next
//! whole second of device time, at least 100 ms ahead (time given to `activateStream` and
//! to the first `writeStream`, as drivers differ in which they honour).
//!
//! Compile-tested only in this crate's CI; it needs real hardware to be verified.

use super::{announce_output_policy, Device, DeviceConfig, DeviceStatus, PpsSource, RefSource, StartTime};
use std::ffi::{c_char, c_int, c_long, c_void, CStr, CString};

type Dev = *mut c_void;
type Stream = *mut c_void;

const TX: c_int = 0;
const RX: c_int = 1;
const FLAG_END_BURST: c_int = 1 << 1;
const FLAG_HAS_TIME: c_int = 1 << 2;
const ERR_TIMEOUT: c_int = -1;
const ERR_OVERFLOW: c_int = -4;
const ERR_TIME: c_int = -6;
const ERR_UNDERFLOW: c_int = -7;

/// Stream sample format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoapyFormat {
    /// Complex float32 (I, Q).
    Cf32,
    /// Complex int16 (I, Q), full scale ±32767.
    Cs16,
}

impl SoapyFormat {
    fn name(self) -> &'static str {
        match self {
            SoapyFormat::Cf32 => "CF32",
            SoapyFormat::Cs16 => "CS16",
        }
    }
    fn bytes(self) -> usize {
        match self {
            SoapyFormat::Cf32 => 8,
            SoapyFormat::Cs16 => 4,
        }
    }
}

struct Lib(*mut c_void);

impl Lib {
    fn open() -> Result<Lib, String> {
        for name in ["libSoapySDR.so.0.8", "libSoapySDR.so", "libSoapySDR.dylib"] {
            let c = CString::new(name).map_err(|e| e.to_string())?;
            // SAFETY: dlopen with a valid C string; a null result is handled.
            let h = unsafe { libc::dlopen(c.as_ptr(), libc::RTLD_NOW) };
            if !h.is_null() {
                return Ok(Lib(h));
            }
        }
        Err("SoapySDR library not found (libSoapySDR.so.0.8)".into())
    }
    fn sym<T: Copy>(&self, name: &str) -> Result<T, String> {
        let c = CString::new(name).map_err(|e| e.to_string())?;
        // SAFETY: dlsym on an open handle; the caller names the C signature of the symbol.
        let p = unsafe { libc::dlsym(self.0, c.as_ptr()) };
        if p.is_null() {
            return Err(format!("SoapySDR symbol {name} missing"));
        }
        // SAFETY: `T` is an `extern "C" fn` pointer type, the same size as a data pointer.
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
    }
}

impl Drop for Lib {
    fn drop(&mut self) {
        // SAFETY: the handle came from dlopen and is closed once.
        unsafe { libc::dlclose(self.0) };
    }
}

/// The SoapySDR 0.8 C functions used (signatures from `SoapySDR/Device.h`).
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
    setup: extern "C" fn(Dev, c_int, *const c_char, *const usize, usize, *const c_void) -> Stream,
    close: extern "C" fn(Dev, Stream) -> c_int,
    activate: extern "C" fn(Dev, Stream, c_int, i64, usize) -> c_int,
    deactivate: extern "C" fn(Dev, Stream, c_int, i64) -> c_int,
    write: extern "C" fn(Dev, Stream, *const *const c_void, usize, *mut c_int, i64, c_long) -> c_int,
    read: extern "C" fn(Dev, Stream, *const *mut c_void, usize, *mut c_int, *mut i64, c_long) -> c_int,
    read_status: extern "C" fn(Dev, Stream, *mut usize, *mut c_int, *mut i64, c_long) -> c_int,
}

/// A SoapySDR device (TX and/or RX).
pub struct SoapyDevice {
    api: Api,
    dev: Dev,
    args: String,
    format: SoapyFormat,
    cfg: Option<DeviceConfig>,
    tx: Stream,
    rx: Stream,
    /// Device time of the first transmit sample, consumed by the first write.
    tx_time: Option<i64>,
    status: DeviceStatus,
    // Keep the library loaded for as long as the function pointers live (dropped last).
    _lib: Lib,
}

// SAFETY: a SoapySDR device handle may be used from any thread as long as calls are not
// concurrent; `SoapyDevice` is only ever used through `&mut self` (moved into one pump
// thread), so calls are serialised.
unsafe impl Send for SoapyDevice {}

fn cstr(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|e| e.to_string())
}

impl SoapyDevice {
    /// Open the device named by SoapySDR key=value `args` (e.g. `driver=uhd,type=b200`).
    /// Prints the output policy.
    pub fn open(args: &str, format: SoapyFormat) -> Result<SoapyDevice, String> {
        announce_output_policy(&format!("soapy:{args}"));
        let lib = Lib::open()?;
        let abi: extern "C" fn() -> *const c_char = lib.sym("SoapySDR_getABIVersion")?;
        // SAFETY: returns a static NUL-terminated string.
        let v = unsafe { CStr::from_ptr(abi()) }.to_string_lossy().into_owned();
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
            read: lib.sym("SoapySDRDevice_readStream")?,
            read_status: lib.sym("SoapySDRDevice_readStreamStatus")?,
        };
        let make: extern "C" fn(*const c_char) -> Dev = lib.sym("SoapySDRDevice_makeStrArgs")?;
        let a = cstr(args)?;
        let dev = make(a.as_ptr());
        let d = SoapyDevice {
            api,
            dev,
            args: args.into(),
            format,
            cfg: None,
            tx: std::ptr::null_mut(),
            rx: std::ptr::null_mut(),
            tx_time: None,
            status: DeviceStatus::default(),
            _lib: lib,
        };
        if dev.is_null() {
            return Err(d.err("make"));
        }
        Ok(d)
    }

    fn err(&self, what: &str) -> String {
        // SAFETY: lastError returns a thread-local NUL-terminated string.
        let e = unsafe { CStr::from_ptr((self.api.last_error)()) }.to_string_lossy().into_owned();
        format!("soapy:{} {what}: {e}", self.args)
    }

    fn check(&self, r: c_int, what: &str) -> Result<(), String> {
        if r != 0 {
            Err(self.err(what))
        } else {
            Ok(())
        }
    }

    /// Resolve a start time to a device time (ns), arming the PPS when asked.
    fn start_ns(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        match at {
            StartTime::Now => Ok(None),
            StartTime::AtDeviceTimeNs(t) => Ok(Some(t)),
            StartTime::NextPps => {
                let pps = cstr("PPS")?;
                self.check((self.api.set_hw_time)(self.dev, 0, pps.as_ptr()), "setHardwareTime(PPS)")?;
                // wait for the PPS edge that applies it
                std::thread::sleep(std::time::Duration::from_millis(1200));
                let now = (self.api.get_hw_time)(self.dev, std::ptr::null());
                let next = (now + 100_000_000).div_euclid(1_000_000_000) * 1_000_000_000 + 1_000_000_000;
                self.status.pps_aligned = true;
                Ok(Some(next))
            }
        }
    }

    fn setup(&self, dir: c_int, n: usize) -> Result<Stream, String> {
        let fmt = cstr(self.format.name())?;
        let ch: Vec<usize> = (0..n).collect();
        let s = (self.api.setup)(self.dev, dir, fmt.as_ptr(), ch.as_ptr(), n, std::ptr::null());
        if s.is_null() {
            return Err(self.err("setupStream"));
        }
        Ok(s)
    }

    /// Poll the transmit stream status without waiting (underflows, late bursts).
    fn poll_tx_status(&mut self) {
        loop {
            let (mut mask, mut flags, mut t) = (0usize, 0 as c_int, 0i64);
            let r = (self.api.read_status)(self.dev, self.tx, &mut mask, &mut flags, &mut t, 0);
            match r {
                ERR_UNDERFLOW => self.status.underruns += 1,
                ERR_TIME => self.status.late += 1,
                _ => break,
            }
        }
    }
}

impl Device for SoapyDevice {
    fn name(&self) -> String {
        format!("soapy:{}", self.args)
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
        self.check((self.api.set_clock_source)(self.dev, clock.as_ptr()), "setClockSource")?;
        let time = cstr(match cfg.time_source {
            PpsSource::Internal => "internal",
            PpsSource::External => "external",
        })?;
        self.check((self.api.set_time_source)(self.dev, time.as_ptr()), "setTimeSource")?;
        for (dir, n, gains) in [(TX, cfg.tx_channels, &cfg.tx_gain_db), (RX, cfg.rx_channels, &cfg.rx_gain_db)] {
            for c in 0..n {
                self.check((self.api.set_rate)(self.dev, dir, c, cfg.sample_rate_hz), "setSampleRate")?;
                self.check((self.api.set_freq)(self.dev, dir, c, cfg.center_freq_hz, std::ptr::null()), "setFrequency")?;
                if let Some(g) = gains.get(c) {
                    self.check((self.api.set_gain)(self.dev, dir, c, *g), "setGain")?;
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
        self.tx = self.setup(TX, n)?;
        let t = self.start_ns(at)?;
        let (flags, tn) = t.map(|t| (FLAG_HAS_TIME, t)).unwrap_or((0, 0));
        self.check((self.api.activate)(self.dev, self.tx, flags, tn, 0), "activateStream(TX)")?;
        self.tx_time = t;
        self.status.tx_streaming = true;
        self.status.start_time_ns = t;
        Ok(t)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        if self.tx.is_null() {
            return Err("transmit stream not started".into());
        }
        let n = block.first().map(|c| c.len()).unwrap_or(0);
        // Interleaved I/Q per channel, real signal on I, Q = 0.
        let bufs: Vec<Vec<u8>> = block
            .iter()
            .map(|ch| {
                let mut b = Vec::with_capacity(n * self.format.bytes());
                for &v in ch {
                    match self.format {
                        SoapyFormat::Cf32 => {
                            b.extend_from_slice(&(v as f32).to_ne_bytes());
                            b.extend_from_slice(&0f32.to_ne_bytes());
                        }
                        SoapyFormat::Cs16 => {
                            let x = (v * 32767.0).round().clamp(-32767.0, 32767.0) as i16;
                            b.extend_from_slice(&x.to_ne_bytes());
                            b.extend_from_slice(&0i16.to_ne_bytes());
                        }
                    }
                }
                b
            })
            .collect();
        let mut done = 0usize;
        let mut timeouts = 0;
        while done < n {
            let ptrs: Vec<*const c_void> = bufs.iter().map(|b| b[done * self.format.bytes()..].as_ptr() as *const c_void).collect();
            let (mut flags, tn) = match self.tx_time.take() {
                Some(t) => (FLAG_HAS_TIME, t),
                None => (0, 0),
            };
            let r = (self.api.write)(self.dev, self.tx, ptrs.as_ptr(), n - done, &mut flags, tn, 1_000_000);
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
        self.status.samples_tx += n as u64;
        self.poll_tx_status();
        Ok(())
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        if self.tx.is_null() {
            return Ok(());
        }
        // end of burst: zero-length write with END_BURST
        let zero = [0u8; 8];
        let ptrs: Vec<*const c_void> = (0..self.cfg.as_ref().map(|c| c.tx_channels).unwrap_or(1)).map(|_| zero.as_ptr() as *const c_void).collect();
        let mut flags = FLAG_END_BURST;
        (self.api.write)(self.dev, self.tx, ptrs.as_ptr(), 0, &mut flags, 0, 100_000);
        self.poll_tx_status();
        (self.api.deactivate)(self.dev, self.tx, 0, 0);
        (self.api.close)(self.dev, self.tx);
        self.tx = std::ptr::null_mut();
        self.status.tx_streaming = false;
        Ok(())
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let n = self.cfg.as_ref().ok_or("device not configured")?.rx_channels;
        if n == 0 {
            return Err("no rx channels configured".into());
        }
        self.rx = self.setup(RX, n)?;
        let t = self.start_ns(at)?;
        let (flags, tn) = t.map(|t| (FLAG_HAS_TIME, t)).unwrap_or((0, 0));
        self.check((self.api.activate)(self.dev, self.rx, flags, tn, 0), "activateStream(RX)")?;
        self.status.rx_streaming = true;
        self.status.start_time_ns = t;
        Ok(t)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        if self.rx.is_null() {
            return Err("receive stream not started".into());
        }
        let nch = self.cfg.as_ref().map(|c| c.rx_channels).unwrap_or(1);
        let mut bufs: Vec<Vec<u8>> = vec![vec![0u8; n * self.format.bytes()]; nch];
        let mut timeouts = 0;
        loop {
            let ptrs: Vec<*mut c_void> = bufs.iter_mut().map(|b| b.as_mut_ptr() as *mut c_void).collect();
            let (mut flags, mut t) = (0 as c_int, 0i64);
            let r = (self.api.read)(self.dev, self.rx, ptrs.as_ptr(), n, &mut flags, &mut t, 1_000_000);
            if r > 0 {
                let k = r as usize;
                self.status.samples_rx += k as u64;
                // take I as the real LF signal
                return Ok(bufs
                    .iter()
                    .map(|b| {
                        (0..k)
                            .map(|i| match self.format {
                                SoapyFormat::Cf32 => f32::from_ne_bytes([b[8 * i], b[8 * i + 1], b[8 * i + 2], b[8 * i + 3]]) as f64,
                                SoapyFormat::Cs16 => i16::from_ne_bytes([b[4 * i], b[4 * i + 1]]) as f64 / 32767.0,
                            })
                            .collect()
                    })
                    .collect());
            }
            match r {
                ERR_OVERFLOW => self.status.overflows += 1,
                ERR_TIMEOUT if timeouts < 5 => timeouts += 1,
                _ => return Err(self.err(&format!("readStream ({r})"))),
            }
        }
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        if !self.rx.is_null() {
            (self.api.deactivate)(self.dev, self.rx, 0, 0);
            (self.api.close)(self.dev, self.rx);
            self.rx = std::ptr::null_mut();
        }
        self.status.rx_streaming = false;
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        self.status.clone()
    }
}

impl Drop for SoapyDevice {
    fn drop(&mut self) {
        let _ = self.stop_tx();
        let _ = self.stop_rx();
        if !self.dev.is_null() {
            (self.api.unmake)(self.dev);
        }
    }
}
