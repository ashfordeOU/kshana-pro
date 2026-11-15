// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Synthetic attack IQ files, with guardrails (roadmap 0.42 F7, owner gate K6)
//!
//! Generates synthetic interference recordings (complex baseband, 16-bit interleaved I/Q)
//! for testing receivers and detectors in a shielded laboratory, over a conducted (cabled)
//! set-up or on an authorised test range. Owner decision K6 (15 November 2026): build it
//! with guardrails; counsel's export approval of 7 November stands as stated.
//! docs/ATTACK-IQ.md is the guardrail record. In code:
//!
//! 1. **Pro only, never in the open crate.** This module lives only in this private crate.
//! 2. **Licence-gated, vetted buyers only.** A verified, unexpired, non-trial licence that
//!    grants the `pro-attack-iq` feature (Enterprise tier, or the feature named on a Pro
//!    licence by the vendor) **and** carries `vetted_buyer: true`, which only the vendor sets
//!    after vetting the buyer. The CLI takes the token from `--license` only and ignores
//!    `--dev`.
//! 3. **Lab or test-range attestation before generation.** A signed-off attestation
//!    ([`Attestation`]) naming the licensee, the facility, the environment (shielded lab,
//!    conducted set-up or authorised test range), the authorisation reference, the purpose,
//!    a validity window that covers today and an accepted statement. Its SHA-256 goes into
//!    the file's metadata and the log.
//! 4. **Files only.** The output must be a new regular file; device nodes, pipes, sockets,
//!    standard output, `/dev`, `/proc`, `/sys` and URL-like or SDR-driver-like targets are
//!    refused. This module has no radio, device, network or process code at all (a test
//!    checks its source), and writes through a temporary file and a rename.
//! 5. **Watermarked and logged per licensee.** Every file carries the licence's mark in the
//!    sample least significant bits ([`crate::watermark::embed_lsb_i16`]) and in its metadata
//!    sidecar, and every generation appends a hash-chained line to the licensee's
//!    generation log (licensee ID, token ID, attestation hash, output hash, parameters).
//! 6. **Bounded and synthetic.** At most [`MAX_DURATION_S`] seconds and [`MAX_SAMPLES`]
//!    samples per file. Waveforms are generic jammer classes and a delayed replay of a
//!    synthetic pseudo-random spreading code; no real GNSS ranging code or navigation
//!    message is produced.
//!
//! **Limit.** Anyone holding a file can overwrite its least significant bits; the mark is
//! for tracing honest leaks, not a lock. The real controls are vetting, the licence and
//! the attestation.

use std::io::Write;
use std::path::{Path, PathBuf};

use kshana::api::KshanaError;
use rand::{Rng, SeedableRng};
use sha2::{Digest, Sha256};

use crate::license::Licence;
use crate::watermark::{self, ResultMark};

/// The licence feature that unlocks generation.
pub const FEATURE: &str = "pro-attack-iq";
/// The attestation format.
pub const ATTESTATION_FORMAT: &str = "kshana-pro-attack-iq-attestation/1";
/// The metadata sidecar format.
pub const META_FORMAT: &str = "kshana-pro-attack-iq/1";
/// The sidecar suffix appended to the output path.
pub const META_SUFFIX: &str = ".attack-iq.json";
/// The longest file, seconds.
pub const MAX_DURATION_S: f64 = 60.0;
/// The most complex samples in one file (about 400 MB of 16-bit I/Q).
pub const MAX_SAMPLES: usize = 100_000_000;
/// Environments an attestation may name.
pub const ENVIRONMENTS: &[&str] = &["shielded-lab", "conducted", "authorised-test-range"];
/// The statement the attestation must accept, word for word.
pub const STATEMENT: &str = "These files will be used only in a shielded laboratory, over a conducted (cabled) set-up, or on a test range whose authority has authorised the test; they will not be radiated outside such an environment, and will not be passed to anyone outside the licensee.";
/// The constant content the IQ mark is bound to, so the mark can be traced from the
/// samples alone.
const IQ_MARK_CONTENT: &[u8] = b"kshana-pro-attack-iq/v1";

fn bad(m: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(m.into())
}

/// The lab or test-range attestation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attestation {
    pub format: String,
    /// Must equal the licence's licensee.
    pub licensee: String,
    pub facility: String,
    /// One of [`ENVIRONMENTS`].
    pub environment: String,
    /// The test authorisation, range booking or lab accreditation reference.
    pub authorisation_ref: String,
    pub purpose: String,
    /// `YYYY-MM-DD`, inclusive.
    pub valid_from: String,
    pub valid_to: String,
    /// The responsible person who signs it off.
    pub signed_by: String,
    /// Must be [`STATEMENT`], word for word.
    pub statement: String,
}

impl Attestation {
    /// A template to fill in.
    pub fn template(licensee: &str) -> Attestation {
        Attestation {
            format: ATTESTATION_FORMAT.into(),
            licensee: licensee.into(),
            facility: "".into(),
            environment: "shielded-lab".into(),
            authorisation_ref: "".into(),
            purpose: "".into(),
            valid_from: "".into(),
            valid_to: "".into(),
            signed_by: "".into(),
            statement: STATEMENT.into(),
        }
    }

    /// Check the attestation for `licensee` on `today` (`YYYY-MM-DD`).
    pub fn check(&self, licensee: &str, today: &str) -> Result<(), KshanaError> {
        if self.format != ATTESTATION_FORMAT {
            return Err(bad(format!(
                "attestation format must be {ATTESTATION_FORMAT}"
            )));
        }
        if self.licensee.trim().to_lowercase() != licensee.trim().to_lowercase() {
            return Err(bad(
                "the attestation names another licensee than the licence",
            ));
        }
        for (name, v) in [
            ("facility", &self.facility),
            ("authorisation_ref", &self.authorisation_ref),
            ("purpose", &self.purpose),
            ("signed_by", &self.signed_by),
        ] {
            if v.trim().len() < 3 {
                return Err(bad(format!("the attestation needs {name}")));
            }
        }
        if !ENVIRONMENTS.contains(&self.environment.as_str()) {
            return Err(bad(format!(
                "environment must be one of {}",
                ENVIRONMENTS.join(", ")
            )));
        }
        let date = |s: &str| {
            let b = s.as_bytes();
            s.len() == 10
                && b[4] == b'-'
                && b[7] == b'-'
                && s.bytes().filter(u8::is_ascii_digit).count() == 8
        };
        if !date(&self.valid_from) || !date(&self.valid_to) {
            return Err(bad("valid_from and valid_to must be YYYY-MM-DD"));
        }
        if !(self.valid_from.as_str() <= today && today <= self.valid_to.as_str()) {
            return Err(bad(format!(
                "the attestation is valid {} to {}, not on {today}",
                self.valid_from, self.valid_to
            )));
        }
        if self.statement.trim() != STATEMENT {
            return Err(bad(
                "the attestation must carry the use statement word for word",
            ));
        }
        Ok(())
    }

    /// SHA-256 of the attestation's canonical JSON.
    pub fn sha256(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or_default();
        hex::encode(Sha256::digest(
            crate::serve::jobs::canonical_json(&v).as_bytes(),
        ))
    }
}

/// The licence checks of guardrail 2. `ent_allows` is whether the licence's entitlement
/// allows [`FEATURE`] (its tier or an explicit grant), as [`crate::license::check`] says.
pub fn check_licence(lic: &Licence, ent_allows: bool, now: i64) -> Result<(), KshanaError> {
    if lic.claims.trial {
        return Err(bad(
            "synthetic attack IQ is not available on a trial licence",
        ));
    }
    if lic.entitlement.is_expired(now) {
        return Err(bad("the licence has expired"));
    }
    if !ent_allows {
        return Err(KshanaError::Unsupported(format!(
            "synthetic attack IQ needs a licence granting '{FEATURE}' (ask the vendor)"
        )));
    }
    if !lic.claims.vetted_buyer {
        return Err(KshanaError::Unsupported(
            "synthetic attack IQ is sold to vetted buyers only; this licence is not marked vetted"
                .into(),
        ));
    }
    Ok(())
}

/// A waveform class.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Waveform {
    /// A continuous-wave tone at `offset_hz` from the centre.
    Cw { offset_hz: f64 },
    /// A linear sweep over ±`sweep_hz`/2 every `period_s` (chirp jammer).
    Chirp { sweep_hz: f64, period_s: f64 },
    /// Band-limited noise of `bandwidth_hz` (a moving-average filtered Gaussian).
    Noise { bandwidth_hz: f64 },
    /// A tone switched on for `duty` of every `period_s`.
    Pulsed {
        offset_hz: f64,
        period_s: f64,
        duty: f64,
    },
    /// A synthetic spread signal and its replay `delay_s` later with `gain_db` (meaconing
    /// class). The spreading code is pseudo-random from the seed, never a real GNSS code.
    Replay {
        chip_rate_hz: f64,
        delay_s: f64,
        gain_db: f64,
    },
}

/// What to generate.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Request {
    pub waveform: Waveform,
    pub sample_rate_hz: f64,
    pub duration_s: f64,
    /// Interference-to-noise power ratio, dB.
    pub inr_db: f64,
    pub seed: u64,
}

impl Request {
    fn check(&self) -> Result<usize, KshanaError> {
        if !(self.sample_rate_hz.is_finite() && (1e3..=100e6).contains(&self.sample_rate_hz)) {
            return Err(bad("sample rate must be 1 kHz to 100 MHz"));
        }
        if !(self.duration_s.is_finite()
            && self.duration_s > 0.0
            && self.duration_s <= MAX_DURATION_S)
        {
            return Err(bad(format!(
                "duration must be above 0 and at most {MAX_DURATION_S} s"
            )));
        }
        if !(self.inr_db.is_finite() && (-30.0..=60.0).contains(&self.inr_db)) {
            return Err(bad("inr_db must be -30 to 60 dB"));
        }
        let n = (self.sample_rate_hz * self.duration_s).round() as usize;
        if n > MAX_SAMPLES {
            return Err(bad(format!("at most {MAX_SAMPLES} samples per file")));
        }
        if n < watermark::LSB_SAMPLES {
            return Err(bad("too short to carry the watermark"));
        }
        let nyq = self.sample_rate_hz / 2.0;
        match &self.waveform {
            Waveform::Cw { offset_hz } | Waveform::Pulsed { offset_hz, .. } if offset_hz.abs() >= nyq => {
                Err(bad("offset must be inside the sampled band"))
            }
            Waveform::Chirp { sweep_hz, period_s } if *sweep_hz <= 0.0 || *sweep_hz >= 2.0 * nyq || *period_s <= 0.0 => {
                Err(bad("chirp sweep must be inside the band and the period positive"))
            }
            Waveform::Noise { bandwidth_hz } if *bandwidth_hz <= 0.0 || *bandwidth_hz > 2.0 * nyq => {
                Err(bad("noise bandwidth must be inside the band"))
            }
            Waveform::Pulsed { period_s, duty, .. } if *period_s <= 0.0 || !(0.0 < *duty && *duty <= 1.0) => {
                Err(bad("pulse period must be positive and duty in (0, 1]"))
            }
            Waveform::Replay { chip_rate_hz, delay_s, gain_db }
                if *chip_rate_hz <= 0.0 || *chip_rate_hz > nyq || *delay_s < 0.0 || *delay_s > self.duration_s || !gain_db.is_finite() =>
            {
                Err(bad("replay needs a chip rate inside the band, a delay within the file and a finite gain"))
            }
            _ => Ok(n),
        }
    }
}

/// Synthesise the samples (complex, unit noise power before scaling).
fn synthesise(r: &Request, n: usize) -> Vec<(f64, f64)> {
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(r.seed);
    let fs = r.sample_rate_hz;
    let amp = 10f64.powf(r.inr_db / 20.0);
    let tau = std::f64::consts::TAU;
    let mut gauss = move || -> f64 {
        // Box-Muller.
        let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
        let u2: f64 = rng.gen::<f64>();
        (-2.0 * u1.ln()).sqrt() * (tau * u2).cos()
    };
    let noise_sigma = std::f64::consts::FRAC_1_SQRT_2;
    let mut out: Vec<(f64, f64)> = (0..n)
        .map(|_| (noise_sigma * gauss(), noise_sigma * gauss()))
        .collect();
    match &r.waveform {
        Waveform::Cw { offset_hz } => {
            for (k, s) in out.iter_mut().enumerate() {
                let ph = tau * offset_hz * k as f64 / fs;
                s.0 += amp * ph.cos();
                s.1 += amp * ph.sin();
            }
        }
        Waveform::Chirp { sweep_hz, period_s } => {
            let rate = sweep_hz / period_s;
            for (k, s) in out.iter_mut().enumerate() {
                let t = (k as f64 / fs) % period_s;
                let ph = tau * (-sweep_hz / 2.0 * t + 0.5 * rate * t * t);
                s.0 += amp * ph.cos();
                s.1 += amp * ph.sin();
            }
        }
        Waveform::Noise { bandwidth_hz } => {
            let taps = ((fs / bandwidth_hz).round() as usize).max(1);
            let raw: Vec<(f64, f64)> = (0..n + taps).map(|_| (gauss(), gauss())).collect();
            let norm = (taps as f64).sqrt() * std::f64::consts::SQRT_2;
            let (mut ai, mut aq) = (0.0, 0.0);
            for (k, s) in out.iter_mut().enumerate() {
                if k == 0 {
                    for x in &raw[..taps] {
                        ai += x.0;
                        aq += x.1;
                    }
                } else {
                    ai += raw[k + taps - 1].0 - raw[k - 1].0;
                    aq += raw[k + taps - 1].1 - raw[k - 1].1;
                }
                s.0 += amp * ai / norm;
                s.1 += amp * aq / norm;
            }
        }
        Waveform::Pulsed {
            offset_hz,
            period_s,
            duty,
        } => {
            for (k, s) in out.iter_mut().enumerate() {
                let t = k as f64 / fs;
                if (t % period_s) / period_s < *duty {
                    let ph = tau * offset_hz * t;
                    s.0 += amp * ph.cos();
                    s.1 += amp * ph.sin();
                }
            }
        }
        Waveform::Replay {
            chip_rate_hz,
            delay_s,
            gain_db,
        } => {
            // A 1023-chip pseudo-random code from the seed (not a GNSS code).
            let mut crng = rand_chacha::ChaCha20Rng::seed_from_u64(r.seed ^ 0x5EED_C0DE);
            let code: Vec<f64> = (0..1023)
                .map(|_| if crng.gen::<bool>() { 1.0 } else { -1.0 })
                .collect();
            let chip = |t: f64| code[((t * chip_rate_hz) as usize) % code.len()];
            let d = (delay_s * fs).round() as usize;
            let g = 10f64.powf(gain_db / 20.0);
            for (k, s) in out.iter_mut().enumerate() {
                let t = k as f64 / fs;
                s.0 += amp * chip(t);
                if k >= d {
                    s.0 += amp * g * chip((k - d) as f64 / fs);
                }
            }
        }
    }
    out
}

/// Quantise to interleaved i16 with headroom.
fn quantise(x: &[(f64, f64)]) -> Vec<i16> {
    let peak = x
        .iter()
        .fold(0f64, |m, (i, q)| m.max(i.abs()).max(q.abs()))
        .max(1e-12);
    let scale = 30_000.0 / peak;
    x.iter()
        .flat_map(|(i, q)| [(i * scale).round() as i16, (q * scale).round() as i16])
        .collect()
}

/// Guardrail 4: the output target must be a new regular file.
pub fn check_output_path(out: &Path) -> Result<(), KshanaError> {
    let s = out.to_string_lossy();
    let low = s.to_ascii_lowercase();
    if s.trim().is_empty() || s == "-" {
        return Err(bad(
            "output must be a file path (standard output is refused)",
        ));
    }
    if low.contains("://") {
        return Err(bad(
            "output must be a file path, not a URL or device address",
        ));
    }
    for drv in [
        "uhd:", "soapy:", "hackrf", "bladerf", "limesdr", "plutosdr", "usrp", "rtl_tcp", "tx:",
    ] {
        if low.contains(drv) {
            return Err(bad(
                "output looks like a radio device or driver; files only",
            ));
        }
    }
    let abs = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(out)
    };
    for p in ["/dev/", "/proc/", "/sys/", "\\\\.\\"] {
        if abs.to_string_lossy().starts_with(p) {
            return Err(bad(
                "output inside a device or kernel tree is refused; files only",
            ));
        }
    }
    if std::fs::symlink_metadata(&abs).is_ok() {
        return Err(bad(format!(
            "{} already exists; attack IQ is written only to a new file",
            abs.display()
        )));
    }
    let parent = abs.parent().ok_or_else(|| bad("output has no folder"))?;
    let pm = std::fs::metadata(parent)
        .map_err(|e| bad(format!("output folder {}: {e}", parent.display())))?;
    if !pm.is_dir() {
        return Err(bad("output folder is not a folder"));
    }
    Ok(())
}

/// What a generation produced.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Meta {
    pub format: String,
    pub notice: String,
    pub sample_format: String,
    pub samples: usize,
    pub request: Request,
    pub licensee_id: String,
    pub token_id: String,
    pub attestation_sha256: String,
    pub environment: String,
    pub generated_unix: i64,
    pub watermark: ResultMark,
    /// SHA-256 of the IQ file.
    pub data_sha256: String,
}

/// The notice written into every sidecar.
pub const NOTICE: &str = "SYNTHETIC TEST SIGNAL. For use only in a shielded laboratory, over a conducted set-up or on an authorised test range, under the attestation named here. Radiating it elsewhere is unlawful in most jurisdictions. Watermarked and logged to the licensee.";

/// The per-token watermark key: the vendor recomputes it from the issuance register.
fn token_mark_key(token: &str) -> [u8; 32] {
    let mut k = [0u8; 32];
    k.copy_from_slice(&Sha256::digest(
        format!("kshana-pro-attack-iq/mark-key/v1|{}", token.trim()).as_bytes(),
    ));
    k
}

/// Generate one file. Every guardrail is checked first; nothing is written on refusal.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    lic: &Licence,
    token: &str,
    ent_allows: bool,
    attestation: &Attestation,
    req: &Request,
    out: &Path,
    log_dir: &Path,
    now: i64,
) -> Result<Meta, KshanaError> {
    check_licence(lic, ent_allows, now)?;
    attestation.check(&lic.claims.licensee, &utc_date(now))?;
    let n = req.check()?;
    check_output_path(out)?;
    let meta_path = PathBuf::from(format!("{}{META_SUFFIX}", out.display()));
    if meta_path.exists() {
        return Err(bad(format!("{} already exists", meta_path.display())));
    }
    std::fs::create_dir_all(log_dir)
        .map_err(|e| bad(format!("log folder {}: {e}", log_dir.display())))?;

    let mut samples = quantise(&synthesise(req, n));
    let mut nonce = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut nonce);
    let mark = watermark::mark_with_nonce(
        &token_mark_key(token),
        &lic.claims.licensee,
        &hex::encode(nonce),
        IQ_MARK_CONTENT,
    );
    if !watermark::embed_lsb_i16(&mut samples, &mark) {
        return Err(bad("too short to carry the watermark"));
    }
    let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let meta = Meta {
        format: META_FORMAT.into(),
        notice: NOTICE.into(),
        sample_format: "ci16_le (interleaved I/Q, 16-bit signed, little-endian)".into(),
        samples: n,
        request: req.clone(),
        licensee_id: crate::propack::format::licensee_id(&lic.claims.licensee),
        token_id: lic.token_id.clone(),
        attestation_sha256: attestation.sha256(),
        environment: attestation.environment.clone(),
        generated_unix: now,
        watermark: mark,
        data_sha256: hex::encode(Sha256::digest(&bytes)),
    };
    // Log first: a generation that cannot be logged is not made.
    append_log(log_dir, &meta)?;
    write_new(out, &bytes)?;
    let json = serde_json::to_vec_pretty(&meta).map_err(|e| bad(e.to_string()))?;
    write_new(&meta_path, &json)?;
    Ok(meta)
}

/// Write `bytes` to a new file through a temporary file in the same folder.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), KshanaError> {
    let tmp = PathBuf::from(format!("{}.partial", path.display()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| bad(format!("{}: {e}", tmp.display())))?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| bad(format!("{}: {e}", tmp.display())))?;
    drop(f);
    if path.exists() {
        let _ = std::fs::remove_file(&tmp);
        return Err(bad(format!("{} appeared while writing", path.display())));
    }
    std::fs::rename(&tmp, path).map_err(|e| bad(format!("{}: {e}", path.display())))
}

/// The generation log file in `log_dir`.
pub fn log_path(log_dir: &Path) -> PathBuf {
    log_dir.join("attack-iq-generations.log")
}

fn append_log(log_dir: &Path, m: &Meta) -> Result<(), KshanaError> {
    let p = log_path(log_dir);
    let prev = std::fs::read_to_string(&p)
        .ok()
        .and_then(|t| {
            t.lines()
                .last()
                .map(|l| hex::encode(Sha256::digest(l.as_bytes())))
        })
        .unwrap_or_default();
    let line = serde_json::json!({
        "t": m.generated_unix,
        "licensee_id": m.licensee_id,
        "token_id": m.token_id,
        "attestation_sha256": m.attestation_sha256,
        "environment": m.environment,
        "data_sha256": m.data_sha256,
        "samples": m.samples,
        "request": m.request,
        "mark": m.watermark.mark,
        "prev": prev,
    })
    .to_string();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)
        .map_err(|e| bad(format!("{}: {e}", p.display())))?;
    writeln!(f, "{line}").map_err(|e| bad(format!("{}: {e}", p.display())))
}

/// Vendor side: the licensee whose token marked these samples, from `(licensee, token)`
/// pairs out of the issuance register.
pub fn trace<'a>(samples: &[i16], candidates: &'a [(String, String)]) -> Option<&'a str> {
    let m = watermark::extract_lsb_i16(samples)?;
    candidates
        .iter()
        .find(|(who, tok)| watermark::verify(&token_mark_key(tok), who, IQ_MARK_CONTENT, &m))
        .map(|(who, _)| who.as_str())
}

/// Read a ci16_le file into samples.
pub fn read_ci16(path: &Path) -> Result<Vec<i16>, KshanaError> {
    let b = std::fs::read(path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    Ok(b.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect())
}

fn utc_date(unix: i64) -> String {
    crate::serve::jobs::utc_stamp(unix)[..10].to_string()
}
