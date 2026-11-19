// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! First plug-in conformance: the moment a signal source is connected, check it against
//! what its datasheet predicts, and keep tamper-evident evidence of what it actually emitted.
//!
//! 1. The device plays the calibration pulse train into its own loopback at several levels
//!    ([`super::spectwin::measure_level`]); the same is done on its spec twin.
//! 2. Each measured level is compared with the twin's prediction within stated tolerances
//!    (gain, waveform SNR, zero-crossing spread, ECD); every deviation is turned into a
//!    diagnosis a technician can act on (load setting, attenuator, clipping, noise floor,
//!    reference lock, coupling).
//! 3. The captures are quantised to 16 bits, hashed, and bound with the measured fidelity and
//!    the device identity into one record ([`WaveformEvidence`]). Anyone holding the evidence
//!    directory can recompute every number from the stored captures
//!    ([`verify_evidence`]); signing the directory (kshana-pro `dossier sign`) makes it
//!    attributable.
//!
//! Evidence: the method is **MODELLED** (internal tests on twins with injected faults); the
//! tolerances are engineering defaults, configurable, not taken from a standard. Nothing here
//! has run on physical hardware yet.

use super::catalog::HwSpec;
use super::spectwin::{analyse_capture, measure_level, LevelCapture, LevelResult, SpecTwin};
use crate::hw::Device;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Format tag of the evidence record.
pub const FORMAT: &str = "kshana-eloran-firstplug/1";

/// Tolerances of the comparison (measured against predicted).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tolerances {
    /// |gain error| (dB) above which the level check warns / fails.
    pub gain_warn_db: f64,
    pub gain_fail_db: f64,
    /// SNR shortfall against the twin (dB) above which the check warns / fails. The twin
    /// models the converter only, so a real bench is expected to sit below it.
    pub snr_warn_db: f64,
    pub snr_fail_db: f64,
    /// Lowest acceptable SNR at the top level (dB).
    pub snr_floor_db: f64,
    /// Zero-crossing spread allowed above the prediction (ns).
    pub spread_fail_ns: f64,
    /// |ECD − predicted| (µs) above which the check warns / fails.
    pub ecd_warn_us: f64,
    pub ecd_fail_us: f64,
}

impl Default for Tolerances {
    fn default() -> Self {
        Tolerances {
            gain_warn_db: 1.0,
            gain_fail_db: 3.0,
            snr_warn_db: 25.0,
            snr_fail_db: 45.0,
            snr_floor_db: 20.0,
            spread_fail_ns: 2.0,
            ecd_warn_us: 0.1,
            ecd_fail_us: 0.3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

/// One comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub predicted: f64,
    pub measured: f64,
    pub verdict: Verdict,
    pub detail: String,
}

/// One level: prediction, measurement, checks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelCheck {
    pub level_dbfs: f64,
    pub predicted: LevelResult,
    pub measured: LevelResult,
    pub gain_db: f64,
    pub checks: Vec<Check>,
}

/// Hash and fidelity of one stored capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureRecord {
    pub level_dbfs: f64,
    pub file: String,
    pub samples: usize,
    /// SHA-256 of the file (16-bit little-endian, full scale = 32767).
    pub sha256: String,
    pub amp: f64,
    pub pps_s: Vec<f64>,
    pub measured: LevelResult,
    pub gain_db: f64,
}

/// What the device emitted, bound to who emitted it and how well.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveformEvidence {
    pub format: String,
    /// Commanded signal: the calibration pulse train (one LORAN-C pulse per ms, phase 0, ECD 0).
    pub signal: String,
    /// SHA-256 of the commanded top-level train (f64 little-endian), so the stimulus is fixed.
    pub commanded_sha256: String,
    pub fs_hz: f64,
    pub pulses: usize,
    pub device_name: String,
    pub catalog_id: String,
    /// `*IDN?` or driver identity, when known.
    pub identity: Option<String>,
    pub captures: Vec<CaptureRecord>,
    /// SHA-256 over the canonical JSON of every field above.
    pub binding_sha256: String,
}

/// The first plug-in report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstPlugReport {
    pub verdict: Verdict,
    pub levels: Vec<LevelCheck>,
    pub diagnoses: Vec<String>,
    pub tolerances: Tolerances,
    pub evidence: WaveformEvidence,
    pub evidence_label: String,
}

fn q16(x: &[f64]) -> (Vec<u8>, Vec<f64>) {
    let mut bytes = Vec::with_capacity(x.len() * 2);
    let mut back = Vec::with_capacity(x.len());
    for &v in x {
        let c = (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&c.to_le_bytes());
        back.push(c as f64 / 32767.0);
    }
    (bytes, back)
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn commanded_sha(fs_hz: f64, pulses: usize, amp: f64) -> String {
    let pps: Vec<f64> = (0..pulses).map(|k| 1e-3 + k as f64 * 1e-3).collect();
    let n = ((1e-3 + pulses as f64 * 1e-3 + 1e-3) * fs_hz) as usize;
    let x = crate::hw::calib::calibration_pulse_train(fs_hz, n, &pps, 0.0, amp);
    let mut b = Vec::with_capacity(x.len() * 8);
    for v in x {
        b.extend_from_slice(&v.to_le_bytes());
    }
    sha(&b)
}

fn binding(e: &WaveformEvidence) -> String {
    let mut c = e.clone();
    c.binding_sha256 = String::new();
    sha(serde_json::to_string(&c).unwrap_or_default().as_bytes())
}

fn check(name: &str, p: f64, m: f64, v: Verdict, detail: String) -> Check {
    Check {
        name: name.into(),
        predicted: p,
        measured: m,
        verdict: v,
        detail,
    }
}

/// Compare one measured level with its prediction.
pub fn compare(pred: &LevelResult, meas: &LevelCapture, tol: &Tolerances) -> LevelCheck {
    let m = &meas.result;
    let mut checks = vec![];
    let g = meas.gain_db;
    let v = if !g.is_finite() || g.abs() > tol.gain_fail_db {
        Verdict::Fail
    } else if g.abs() > tol.gain_warn_db {
        Verdict::Warn
    } else {
        Verdict::Pass
    };
    checks.push(check(
        "gain",
        0.0,
        g,
        v,
        format!("output level {g:+.2} dB against the commanded level"),
    ));
    let short = pred.snr_db - m.snr_db;
    let v = if short > tol.snr_fail_db || (pred.level_dbfs >= -1.0 && m.snr_db < tol.snr_floor_db) {
        Verdict::Fail
    } else if short > tol.snr_warn_db {
        Verdict::Warn
    } else {
        Verdict::Pass
    };
    checks.push(check(
        "snr",
        pred.snr_db,
        m.snr_db,
        v,
        format!(
            "waveform SNR {:.1} dB, twin {:.1} dB",
            m.snr_db, pred.snr_db
        ),
    ));
    let excess = m.delay_std_ns - pred.delay_std_ns;
    let v = if excess > tol.spread_fail_ns {
        Verdict::Fail
    } else {
        Verdict::Pass
    };
    checks.push(check(
        "zero_crossing_spread",
        pred.delay_std_ns,
        m.delay_std_ns,
        v,
        format!(
            "pulse-to-pulse zero-crossing spread {:.3} ns",
            m.delay_std_ns
        ),
    ));
    let de = (m.ecd_us - pred.ecd_us).abs();
    let v = if !de.is_finite() || de > tol.ecd_fail_us {
        Verdict::Fail
    } else if de > tol.ecd_warn_us {
        Verdict::Warn
    } else {
        Verdict::Pass
    };
    checks.push(check(
        "ecd",
        pred.ecd_us,
        m.ecd_us,
        v,
        format!(
            "envelope-to-cycle discrepancy {:.3} µs, twin {:.3} µs",
            m.ecd_us, pred.ecd_us
        ),
    ));
    LevelCheck {
        level_dbfs: pred.level_dbfs,
        predicted: pred.clone(),
        measured: m.clone(),
        gain_db: g,
        checks,
    }
}

/// Turn the pattern of deviations into actions.
pub fn diagnose(levels: &[LevelCheck]) -> Vec<String> {
    let mut d = vec![];
    let gains: Vec<f64> = levels
        .iter()
        .map(|l| l.gain_db)
        .filter(|g| g.is_finite())
        .collect();
    if gains.len() < levels.len() || gains.iter().all(|g| *g < -30.0) {
        d.push("No usable signal at the input: check that the output is enabled, the right channel is cabled, and the loopback cable is connected.".into());
        return d;
    }
    let mean_g = gains.iter().sum::<f64>() / gains.len() as f64;
    let spread_g = gains.iter().cloned().fold(f64::MIN, f64::max)
        - gains.iter().cloned().fold(f64::MAX, f64::min);
    if (mean_g - 6.02).abs() < 1.5 && spread_g < 1.5 {
        d.push(format!("Level is {mean_g:+.1} dB high at every level: the output drives a high-impedance input while the device is set for a 50 Ω load. Terminate the line in 50 Ω or set the output load to high-Z."));
    } else if (mean_g + 6.02).abs() < 1.5 && spread_g < 1.5 {
        d.push(format!("Level is {mean_g:+.1} dB low at every level: a 50 Ω termination is present while the device is set for high-Z (or a second 50 Ω load is in parallel). Remove the extra load or set the output load to 50 Ω."));
    } else if mean_g < -10.0 && spread_g < 3.0 {
        d.push(format!("Level is {mean_g:+.1} dB at every level: an attenuator or a wrong input range is in the loopback path. Account for it in the calibration or remove it for the first-plug test."));
    }
    // Compression: top level loses gain relative to the others.
    if levels.len() >= 2 {
        let top = levels
            .iter()
            .max_by(|a, b| a.level_dbfs.partial_cmp(&b.level_dbfs).unwrap())
            .unwrap();
        let rest: Vec<f64> = levels
            .iter()
            .filter(|l| l.level_dbfs < top.level_dbfs)
            .map(|l| l.gain_db)
            .collect();
        if !rest.is_empty() {
            let r = rest.iter().sum::<f64>() / rest.len() as f64;
            if r - top.gain_db > 1.0 {
                d.push(format!("Gain drops by {:.1} dB at the top level ({} dBFS): clipping or compression. Lower the output level or raise the input range.", r - top.gain_db, top.level_dbfs));
            }
        }
    }
    let snr_fail: Vec<&LevelCheck> = levels
        .iter()
        .filter(|l| {
            l.checks
                .iter()
                .any(|c| c.name == "snr" && c.verdict != Verdict::Pass)
        })
        .collect();
    if !snr_fail.is_empty() {
        // A fixed noise floor: SNR falls dB-for-dB with the level.
        let mut pts: Vec<(f64, f64)> = levels
            .iter()
            .map(|l| (l.level_dbfs, l.measured.snr_db))
            .collect();
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let slope = if pts.len() >= 2 {
            (pts[pts.len() - 1].1 - pts[0].1) / (pts[pts.len() - 1].0 - pts[0].0)
        } else {
            0.0
        };
        if slope > 0.7 {
            d.push(format!("SNR falls about {slope:.1} dB per dB of level: an additive noise floor well above the converter's (ground loop, unshielded cable, noisy supply or input range too wide). Use short shielded coax, one ground point, and the input range closest to the signal."));
        } else {
            d.push("SNR is low at every level, independent of level: distortion rather than noise (filter, coupling, nonlinearity or interference at a fixed ratio). Check the coupling setting and the output filter.".into());
        }
    }
    if levels.iter().any(|l| {
        l.checks
            .iter()
            .any(|c| c.name == "zero_crossing_spread" && c.verdict == Verdict::Fail)
    }) {
        d.push("Zero crossings wander pulse to pulse: the sample clock is not locked. Check the 10 MHz reference cable and level, and that the device is set to the external reference.".into());
    }
    if levels.iter().any(|l| {
        l.checks
            .iter()
            .any(|c| c.name == "ecd" && c.verdict != Verdict::Pass)
    }) {
        d.push("The pulse envelope is shifted against the carrier (ECD): an AC-coupled path, a filter with group-delay ripple near 100 kHz, or a transformer in the loopback. Use a DC-coupled path and check the output coupling setting.".into());
    }
    d
}

/// Run the first plug-in check on `dev` (whose output is looped back to its own input),
/// against the spec twin of `spec`.
pub fn run(
    dev: &mut dyn Device,
    spec: &HwSpec,
    identity: Option<String>,
    fs_hz: f64,
    levels_dbfs: &[f64],
    pulses: usize,
    tol: &Tolerances,
) -> Result<(FirstPlugReport, Vec<(String, Vec<u8>)>), String> {
    let external = spec.takes_10mhz() == Some(true);
    let vfs = spec
        .tx
        .as_ref()
        .and_then(|t| t.max_vpp_into_50ohm)
        .map(|v| v / 2.0);
    let mut levels = vec![];
    let mut caps = vec![];
    let mut files = vec![];
    for &lvl in levels_dbfs {
        let mut tw = SpecTwin::from_spec(spec, fs_hz)?;
        let pred = measure_level(&mut tw, fs_hz, lvl, pulses, external, vfs)?.result;
        let raw = match measure_level(dev, fs_hz, lvl, pulses, external, vfs) {
            Ok(m) => m,
            Err(e) => {
                // No pulse found: a dead or disconnected path. Record a zero capture.
                let n = ((2e-3 + pulses as f64 * 1e-3) * fs_hz) as usize;
                let _ = e;
                LevelCapture {
                    result: LevelResult {
                        level_dbfs: lvl,
                        vpeak_50ohm: None,
                        delay_bias_ns: f64::NAN,
                        delay_std_ns: f64::NAN,
                        ecd_us: f64::NAN,
                        snr_db: f64::NEG_INFINITY,
                    },
                    gain_db: f64::NEG_INFINITY,
                    amp: 10f64.powf(lvl / 20.0),
                    pps_s: (0..pulses).map(|k| 1e-3 + k as f64 * 1e-3).collect(),
                    capture: vec![0.0; n],
                }
            }
        };
        // Quantise, store, and re-measure from the stored capture so verification can repeat it.
        let (bytes, back) = q16(&raw.capture);
        let meas = if raw.gain_db.is_finite() {
            analyse_capture(back, fs_hz, raw.pps_s.clone(), raw.amp, lvl, vfs)
                .unwrap_or(raw.clone())
        } else {
            raw.clone()
        };
        let file = format!("capture_{:+.0}dBFS.ri16", lvl)
            .replace('+', "p")
            .replace('-', "m");
        caps.push(CaptureRecord {
            level_dbfs: lvl,
            file: file.clone(),
            samples: bytes.len() / 2,
            sha256: sha(&bytes),
            amp: meas.amp,
            pps_s: meas.pps_s.clone(),
            measured: meas.result.clone(),
            gain_db: meas.gain_db,
        });
        files.push((file, bytes));
        levels.push(compare(&pred, &meas, tol));
    }
    let top = levels_dbfs.iter().cloned().fold(f64::MIN, f64::max);
    let mut ev = WaveformEvidence {
        format: FORMAT.into(),
        signal: "calibration pulse train v1: one LORAN-C pulse per 1 ms, phase 0, ECD 0, first PPS at 1 ms".into(),
        commanded_sha256: commanded_sha(fs_hz, pulses, 10f64.powf(top / 20.0)),
        fs_hz,
        pulses,
        device_name: dev.name(),
        catalog_id: spec.id.clone(),
        identity,
        captures: caps,
        binding_sha256: String::new(),
    };
    ev.binding_sha256 = binding(&ev);
    let verdict = levels
        .iter()
        .flat_map(|l| l.checks.iter().map(|c| c.verdict))
        .max()
        .unwrap_or(Verdict::Fail);
    let diagnoses = diagnose(&levels);
    Ok((
        FirstPlugReport {
            verdict,
            levels,
            diagnoses,
            tolerances: tol.clone(),
            evidence: ev,
            evidence_label: "MODELLED method; measured values are from this device's own loopback"
                .into(),
        },
        files,
    ))
}

/// Write the report and the captures into `dir` (then sign it with `dossier sign`).
pub fn write_evidence(
    dir: &Path,
    report: &FirstPlugReport,
    files: &[(String, Vec<u8>)],
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes).map_err(|e| format!("{name}: {e}"))?;
    }
    let j = serde_json::to_string_pretty(report).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("firstplug.json"), j).map_err(|e| e.to_string())
}

/// Re-check an evidence directory: file hashes, the binding hash, and every measured number
/// recomputed from the stored captures. Returns the list of problems (empty = intact).
pub fn verify_evidence(dir: &Path) -> Result<Vec<String>, String> {
    let t = std::fs::read_to_string(dir.join("firstplug.json"))
        .map_err(|e| format!("firstplug.json: {e}"))?;
    let r: FirstPlugReport =
        serde_json::from_str(&t).map_err(|e| format!("firstplug.json: {e}"))?;
    let ev = &r.evidence;
    let mut bad = vec![];
    if ev.format != FORMAT {
        bad.push(format!("unknown format {}", ev.format));
    }
    if binding(ev) != ev.binding_sha256 {
        bad.push("binding hash does not match the record (a field was changed)".into());
    }
    let vfs = r.levels.first().and_then(|l| {
        l.predicted
            .vpeak_50ohm
            .map(|v| v / 10f64.powf(l.level_dbfs / 20.0))
    });
    for c in &ev.captures {
        let bytes = match std::fs::read(dir.join(&c.file)) {
            Ok(b) => b,
            Err(e) => {
                bad.push(format!("{}: {e}", c.file));
                continue;
            }
        };
        if sha(&bytes) != c.sha256 {
            bad.push(format!("{}: SHA-256 differs from the record", c.file));
            continue;
        }
        if !c.gain_db.is_finite() {
            continue;
        }
        let x: Vec<f64> = bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f64 / 32767.0)
            .collect();
        match analyse_capture(x, ev.fs_hz, c.pps_s.clone(), c.amp, c.level_dbfs, vfs) {
            Ok(m) => {
                let close = |a: f64, b: f64| {
                    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
                        || (a.is_nan() && b.is_nan())
                };
                if !close(m.result.delay_bias_ns, c.measured.delay_bias_ns)
                    || !close(m.result.snr_db, c.measured.snr_db)
                    || !close(m.gain_db, c.gain_db)
                    || !close(m.result.ecd_us, c.measured.ecd_us)
                {
                    bad.push(format!(
                        "{}: the recorded measurement does not follow from the stored capture",
                        c.file
                    ));
                }
            }
            Err(e) => bad.push(format!("{}: {e}", c.file)),
        }
    }
    Ok(bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::catalog::{Catalog, TxSpec};
    use crate::hw::{DeviceConfig, DeviceStatus, StartTime};

    fn spec() -> HwSpec {
        let mut c = Catalog::default();
        c.merge_json(r#"[{"id":"lf","vendor":"V","model":"LF","category":"awg","sources":[{"url":"https://e.org","title":"t"}]}]"#).unwrap();
        let mut s = c.get("lf").unwrap().clone();
        s.tx = Some(TxSpec {
            channels: Some(1),
            dac_bits: Some(14),
            max_sample_rate_hz: Some(125e6),
            freq_min_hz: Some(0.0),
            freq_max_hz: Some(50e6),
            coupling: Some("dc".into()),
            max_vpp_into_50ohm: Some(2.0),
            ..Default::default()
        });
        s
    }

    /// A "real" device: the twin with a fault injected between output and input.
    struct Faulty {
        inner: SpecTwin,
        gain: f64,
        noise: f64,
        clip: f64,
        dead: bool,
        k: u64,
    }
    impl Device for Faulty {
        fn name(&self) -> String {
            "faulty bench".into()
        }
        fn realtime(&self) -> bool {
            false
        }
        fn configure(&mut self, c: &DeviceConfig) -> Result<(), String> {
            self.inner.configure(c)
        }
        fn sample_rate_hz(&self) -> Option<f64> {
            self.inner.sample_rate_hz()
        }
        fn start_tx(&mut self, a: StartTime) -> Result<Option<i64>, String> {
            self.inner.start_tx(a)
        }
        fn write_tx(&mut self, b: &[Vec<f64>]) -> Result<(), String> {
            self.inner.write_tx(b)
        }
        fn stop_tx(&mut self) -> Result<(), String> {
            self.inner.stop_tx()
        }
        fn start_rx(&mut self, a: StartTime) -> Result<Option<i64>, String> {
            self.inner.start_rx(a)
        }
        fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
            let mut b = self.inner.read_rx(n)?;
            for ch in b.iter_mut() {
                for v in ch.iter_mut() {
                    self.k = self
                        .k
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let u = ((self.k >> 11) as f64 / (1u64 << 53) as f64) - 0.5;
                    *v = if self.dead {
                        0.0
                    } else {
                        (*v * self.gain + self.noise * u * 3.46).clamp(-self.clip, self.clip)
                    };
                }
            }
            Ok(b)
        }
        fn stop_rx(&mut self) -> Result<(), String> {
            self.inner.stop_rx()
        }
        fn status(&self) -> DeviceStatus {
            self.inner.status()
        }
    }

    fn bench(gain: f64, noise: f64, clip: f64, dead: bool) -> Faulty {
        Faulty {
            inner: SpecTwin::from_spec(&spec(), 2e6).unwrap(),
            gain,
            noise,
            clip,
            dead,
            k: 7,
        }
    }

    const LV: [f64; 3] = [0.0, -20.0, -40.0];

    #[test]
    fn a_healthy_device_conforms_and_its_evidence_verifies() {
        let mut d = bench(1.0, 0.0, 10.0, false);
        let (r, files) = run(
            &mut d,
            &spec(),
            Some("V,LF,SN1".into()),
            2e6,
            &LV,
            6,
            &Tolerances::default(),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::Pass, "{:#?}", r.levels);
        assert!(r.diagnoses.is_empty(), "{:?}", r.diagnoses);
        let dir = std::env::temp_dir().join(format!("kshana-firstplug-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_evidence(&dir, &r, &files).unwrap();
        let v0 = verify_evidence(&dir).unwrap();
        assert!(v0.is_empty(), "{v0:?}");
        // Tamper with one capture byte: detected.
        let f = dir.join(&r.evidence.captures[1].file);
        let mut b = std::fs::read(&f).unwrap();
        b[5000] ^= 1;
        std::fs::write(&f, &b).unwrap();
        let bad = verify_evidence(&dir).unwrap();
        assert!(bad.iter().any(|x| x.contains("SHA-256")), "{bad:?}");
        // Edit a recorded number in the JSON: detected by the binding hash.
        write_evidence(&dir, &r, &files).unwrap();
        let mut rr = r.clone();
        rr.evidence.captures[0].measured.snr_db += 3.0;
        std::fs::write(
            dir.join("firstplug.json"),
            serde_json::to_string(&rr).unwrap(),
        )
        .unwrap();
        let bad = verify_evidence(&dir).unwrap();
        assert!(bad.iter().any(|x| x.contains("binding")), "{bad:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_mismatch_is_diagnosed_as_plus_6_db() {
        let mut d = bench(2.0, 0.0, 10.0, false);
        let (r, _) = run(
            &mut d,
            &spec(),
            None,
            2e6,
            &[-6.0, -20.0, -40.0],
            6,
            &Tolerances::default(),
        )
        .unwrap();
        assert_eq!(r.verdict, Verdict::Fail);
        assert!(
            r.diagnoses.iter().any(|x| x.contains("high-impedance")),
            "{:?}",
            r.diagnoses
        );
        let mut d = bench(0.5, 0.0, 10.0, false);
        let (r, _) = run(&mut d, &spec(), None, 2e6, &LV, 6, &Tolerances::default()).unwrap();
        assert!(
            r.diagnoses.iter().any(|x| x.contains("50 Ω termination")),
            "{:?}",
            r.diagnoses
        );
    }

    #[test]
    fn noise_floor_clipping_and_dead_path_are_told_apart() {
        let mut d = bench(1.0, 2e-3, 10.0, false);
        let (r, _) = run(&mut d, &spec(), None, 2e6, &LV, 6, &Tolerances::default()).unwrap();
        assert!(
            r.diagnoses.iter().any(|x| x.contains("noise floor")),
            "{:?} {:#?}",
            r.diagnoses,
            r.levels
                .iter()
                .map(|l| l.measured.snr_db)
                .collect::<Vec<_>>()
        );
        let mut d = bench(1.0, 0.0, 0.5, false);
        let (r, _) = run(&mut d, &spec(), None, 2e6, &LV, 6, &Tolerances::default()).unwrap();
        assert!(
            r.diagnoses.iter().any(|x| x.contains("clipping")),
            "{:?}",
            r.diagnoses
        );
        let mut d = bench(1.0, 0.0, 10.0, true);
        let (r, _) = run(&mut d, &spec(), None, 2e6, &LV, 6, &Tolerances::default()).unwrap();
        assert_eq!(r.verdict, Verdict::Fail);
        assert!(
            r.diagnoses[0].contains("No usable signal"),
            "{:?}",
            r.diagnoses
        );
    }
}
