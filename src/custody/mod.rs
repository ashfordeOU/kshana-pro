// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Sealed incident capture and chain of custody (research 2026-11-20 gaps, B0604)
//!
//! When interference is suspected, the I/Q around the moment matters, and so does being able
//! to show later that nobody changed it. This module:
//!
//! 1. **freezes a ring buffer** around a trigger: the recording is streamed through a ring of
//!    `pre_s` seconds; at the trigger (a stated sample, or a block-power rise of the stated
//!    decibels over the running baseline) the ring is frozen and `post_s` more seconds are
//!    taken. The sample bytes are copied as they are, never re-encoded;
//! 2. **seals a SigMF capture**: the metadata carries, in the `kshana_custody` extension
//!    namespace, the calibration ID, the source file's SHA-256 and sample offset, the trigger,
//!    the SHA-256 of the capture data, the sealing time, the sealer's public key and an Ed25519
//!    signature over the canonical metadata without the signature;
//! 3. writes a **custody form** bound to the capture hash (who collected it, where, why, when),
//!    signed by the collector;
//! 4. keeps a **handover log**: each transfer is an entry chained to the previous one (the first
//!    to the custody form) by SHA-256 and signed by the party handing over.
//!
//! [`verify`] fails on any changed byte of the data, the metadata, the form or the log, and on a
//! broken or re-ordered chain. [`export`] gives the whole chain as one JSON document.
//!
//! Each piece carries its own signature, so the log can grow without re-signing the capture.
//! A hardware-anchored seal is not built here (it would sit behind `unfiled-methods`).
//! The capture is analysis input only: nothing here locates an emitter or transmits.

use std::path::Path;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::license::{key_id, parse_key_hex, public_key_for, KEY_LEN};

/// Pro feature of the capture.
pub const FEATURE: &str = "pro-custody-capture";
/// SigMF extension namespace.
pub const NS: &str = "kshana_custody";
/// Format of the custody form.
pub const FORM_FORMAT: &str = "kshana-pro-custody-form/1";
/// Format of the exported chain.
pub const CHAIN_FORMAT: &str = "kshana-pro-custody-chain/1";
/// Files of a custody directory.
pub const META: &str = "capture.sigmf-meta";
pub const DATA: &str = "capture.sigmf-data";
pub const FORM: &str = "custody.json";
pub const LOG: &str = "handover.json";

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// The trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Trigger {
    /// A stated sample index of the source recording.
    Sample { sample: u64 },
    /// The first block whose power rises `threshold_db` over the mean of the previous
    /// `baseline_blocks` blocks.
    Power {
        threshold_db: f64,
        #[serde(default = "d_block")]
        block_samples: usize,
        #[serde(default = "d_baseline")]
        baseline_blocks: usize,
    },
}

fn d_block() -> usize {
    1024
}
fn d_baseline() -> usize {
    16
}

/// The capture request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// Source SigMF metadata and data.
    pub meta: String,
    pub data: String,
    #[serde(default = "d_fs")]
    pub full_scale: f64,
    pub trigger: Trigger,
    pub pre_s: f64,
    pub post_s: f64,
    /// Calibration record of the receiver chain (the user's identifier).
    pub calibration_id: String,
    pub collected_by: String,
    #[serde(default)]
    pub organisation: Option<String>,
    /// Where the capture was taken, as the collector states it.
    pub location: String,
    pub reason: String,
}

fn d_fs() -> f64 {
    1.0
}

/// A ring buffer of whole samples (raw bytes).
pub struct Ring {
    buf: Vec<u8>,
    bps: usize,
    cap: usize,
    head: usize,
    len: usize,
}

impl Ring {
    /// A ring of `cap` samples of `bps` bytes.
    pub fn new(cap: usize, bps: usize) -> Ring {
        Ring {
            buf: vec![0; cap * bps],
            bps,
            cap,
            head: 0,
            len: 0,
        }
    }
    /// Push one sample.
    pub fn push(&mut self, s: &[u8]) {
        if self.cap == 0 {
            return;
        }
        let i = (self.head + self.len) % self.cap;
        self.buf[i * self.bps..(i + 1) * self.bps].copy_from_slice(s);
        if self.len < self.cap {
            self.len += 1;
        } else {
            self.head = (self.head + 1) % self.cap;
        }
    }
    /// The samples held, oldest first.
    pub fn freeze(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len * self.bps);
        for k in 0..self.len {
            let i = (self.head + k) % self.cap;
            out.extend_from_slice(&self.buf[i * self.bps..(i + 1) * self.bps]);
        }
        out
    }
}

/// The trigger sample of a recording.
pub fn find_trigger(
    data: &[u8],
    dt: kshana::sigmf::DataType,
    full_scale: f64,
    t: &Trigger,
) -> Result<u64, String> {
    let bps = dt.bytes_per_sample();
    let n = (data.len() / bps) as u64;
    match t {
        Trigger::Sample { sample } => {
            if *sample >= n {
                return Err(format!(
                    "trigger sample {sample} is beyond the recording ({n} samples)"
                ));
            }
            Ok(*sample)
        }
        Trigger::Power {
            threshold_db,
            block_samples,
            baseline_blocks,
        } => {
            if *block_samples == 0 || *baseline_blocks == 0 {
                return Err("block_samples and baseline_blocks must be positive".into());
            }
            let mut hist: std::collections::VecDeque<f64> = std::collections::VecDeque::new();
            for (b, chunk) in data.chunks_exact(block_samples * bps).enumerate() {
                let x = kshana::sigmf::decode(chunk, dt, full_scale);
                let p = x.iter().map(|c| c.re * c.re + c.im * c.im).sum::<f64>() / x.len() as f64;
                if hist.len() == *baseline_blocks {
                    let base = hist.iter().sum::<f64>() / hist.len() as f64;
                    if base > 0.0 && 10.0 * (p / base).log10() >= *threshold_db {
                        return Ok((b * block_samples) as u64);
                    }
                    hist.pop_front();
                }
                hist.push_back(p);
            }
            Err("no block reached the power threshold".into())
        }
    }
}

/// Canonical bytes of a JSON value with one key removed from an object at `path`.
fn canonical_without(v: &Value, obj: &str, key: &str) -> Vec<u8> {
    let mut c = v.clone();
    if let Some(o) = c.get_mut(obj).and_then(Value::as_object_mut) {
        o.remove(key);
    } else if obj.is_empty() {
        if let Some(o) = c.as_object_mut() {
            o.remove(key);
        }
    }
    serde_json::to_vec(&c).unwrap_or_default()
}

fn sign(seed: &[u8; KEY_LEN], msg: &[u8]) -> String {
    hex::encode(SigningKey::from_bytes(seed).sign(msg).to_bytes())
}

fn check_sig(pk_hex: &str, sig_hex: &str, msg: &[u8]) -> bool {
    let Ok(pk) = parse_key_hex(pk_hex) else {
        return false;
    };
    let sig: Option<[u8; 64]> = hex::decode(sig_hex).ok().and_then(|b| b.try_into().ok());
    match (VerifyingKey::from_bytes(&pk), sig) {
        (Ok(k), Some(s)) => k.verify_strict(msg, &Signature::from_bytes(&s)).is_ok(),
        _ => false,
    }
}

/// The custody form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Form {
    pub format: String,
    pub title: String,
    /// SHA-256 of the capture data.
    pub capture_id: String,
    /// SHA-256 of the capture metadata file.
    pub meta_sha256: String,
    pub calibration_id: String,
    pub collected_by: String,
    pub organisation: Option<String>,
    pub location: String,
    pub reason: String,
    pub collected_utc: String,
    pub trigger_sample: u64,
    pub samples: u64,
    pub public_key: String,
    pub key_id: String,
    pub signature: String,
}

/// One handover.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Handover {
    pub seq: u64,
    pub utc: String,
    pub from: String,
    pub to: String,
    pub purpose: String,
    pub capture_id: String,
    pub prev_hash: String,
    pub hash: String,
    pub public_key: String,
    pub signature: String,
}

impl Handover {
    fn digest(&self) -> String {
        sha(&serde_json::to_vec(&json!({
            "seq": self.seq, "utc": self.utc, "from": self.from, "to": self.to,
            "purpose": self.purpose, "capture_id": self.capture_id, "prev_hash": self.prev_hash,
        }))
        .unwrap_or_default())
    }
}

/// What a capture produced.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Captured {
    pub capture_id: String,
    pub trigger_sample: u64,
    pub first_source_sample: u64,
    pub samples: u64,
    pub pre_samples: u64,
    pub post_samples: u64,
}

/// Capture, seal and write the custody directory.
pub fn capture(
    q: &Request,
    base: &Path,
    out: &Path,
    seed: &[u8; KEY_LEN],
    utc: &str,
) -> Result<Captured, String> {
    for (n, v) in [("pre_s", q.pre_s), ("post_s", q.post_s)] {
        if v.is_nan() || v < 0.0 {
            return Err(format!("{n} must be zero or positive"));
        }
    }
    for (n, v) in [
        ("calibration_id", &q.calibration_id),
        ("collected_by", &q.collected_by),
        ("location", &q.location),
        ("reason", &q.reason),
    ] {
        if v.trim().is_empty() {
            return Err(format!("{n} must be stated"));
        }
    }
    let meta_text = crate::timing::grid::read_text(base, &q.meta)?;
    let data_path = if Path::new(&q.data).is_absolute() {
        q.data.clone().into()
    } else {
        base.join(&q.data)
    };
    let data = std::fs::read(&data_path).map_err(|e| format!("{}: {e}", data_path.display()))?;
    let meta = kshana::sigmf::parse_meta(&meta_text)?;
    let dt = meta.datatype()?;
    let fs = match meta.global.sample_rate {
        Some(r) if r > 0.0 => r,
        _ => return Err(format!("{}: no sample rate", q.meta)),
    };
    let bps = dt.bytes_per_sample();
    let start = meta.captures.first().map(|c| c.sample_start).unwrap_or(0) as usize * bps;
    let body = data.get(start..).ok_or("capture starts beyond the data")?;
    let trig = find_trigger(body, dt, q.full_scale, &q.trigger)?;
    let (pre_n, post_n) = (
        (q.pre_s * fs).round() as usize,
        (q.post_s * fs).round() as usize,
    );
    // Stream through the ring; freeze at the trigger; take the post-trigger samples.
    let mut ring = Ring::new(pre_n, bps);
    let mut post = Vec::with_capacity(post_n * bps);
    for (k, s) in body.chunks_exact(bps).enumerate() {
        let k = k as u64;
        if k < trig {
            ring.push(s);
        } else if (k - trig) < post_n as u64 {
            post.extend_from_slice(s);
        } else {
            break;
        }
    }
    let mut cap = ring.freeze();
    let pre_got = (cap.len() / bps) as u64;
    cap.extend_from_slice(&post);
    let n = (cap.len() / bps) as u64;
    let capture_id = sha(&cap);
    // SigMF metadata of the capture, sealed.
    let mut m = meta.clone();
    m.captures = vec![kshana::sigmf::Capture {
        sample_start: 0,
        frequency: meta.captures.first().and_then(|c| c.frequency),
        datetime: None,
    }];
    m.annotations = Vec::new();
    let pk = public_key_for(seed);
    let mut g = Map::new();
    let put = |g: &mut Map<String, Value>, k: &str, v: Value| {
        g.insert(format!("{NS}:{k}"), v);
    };
    put(&mut g, "calibration_id", json!(q.calibration_id));
    put(&mut g, "source_data_sha256", json!(sha(&data)));
    put(&mut g, "first_source_sample", json!(trig - pre_got));
    put(&mut g, "trigger_sample", json!(pre_got));
    put(
        &mut g,
        "trigger",
        serde_json::to_value(&q.trigger).map_err(|e| e.to_string())?,
    );
    put(&mut g, "pre_samples", json!(pre_got));
    put(&mut g, "post_samples", json!(n - pre_got));
    put(&mut g, "data_sha256", json!(capture_id));
    put(&mut g, "sealed_utc", json!(utc));
    put(&mut g, "public_key", json!(hex::encode(pk)));
    put(&mut g, "key_id", json!(key_id(&pk)));
    let text = kshana::sigmf::meta_to_json_with_extensions(&m, NS, "1.0.0", &g, &[])?;
    let mut v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let sig = sign(seed, &serde_json::to_vec(&v).map_err(|e| e.to_string())?);
    v["global"][format!("{NS}:signature")] = json!(sig);
    let meta_out = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
    // Custody form.
    let mut form = Form {
        format: FORM_FORMAT.into(),
        title: q.title.clone(),
        capture_id: capture_id.clone(),
        meta_sha256: sha(meta_out.as_bytes()),
        calibration_id: q.calibration_id.clone(),
        collected_by: q.collected_by.clone(),
        organisation: q.organisation.clone(),
        location: q.location.clone(),
        reason: q.reason.clone(),
        collected_utc: utc.into(),
        trigger_sample: pre_got,
        samples: n,
        public_key: hex::encode(pk),
        key_id: key_id(&pk),
        signature: String::new(),
    };
    let fv = serde_json::to_value(&form).map_err(|e| e.to_string())?;
    form.signature = sign(seed, &canonical_without(&fv, "", "signature"));
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out).map_err(io)?;
    std::fs::write(out.join(DATA), &cap).map_err(io)?;
    std::fs::write(out.join(META), &meta_out).map_err(io)?;
    std::fs::write(
        out.join(FORM),
        serde_json::to_string_pretty(&form).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(io)?;
    std::fs::write(out.join(LOG), "[]\n").map_err(io)?;
    Ok(Captured {
        capture_id,
        trigger_sample: pre_got,
        first_source_sample: trig - pre_got,
        samples: n,
        pre_samples: pre_got,
        post_samples: n - pre_got,
    })
}

fn read_log(dir: &Path) -> Result<Vec<Handover>, String> {
    let p = dir.join(LOG);
    let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{LOG}: {e}"))
}

/// Append a handover signed by the party handing over.
pub fn handover(
    dir: &Path,
    from: &str,
    to: &str,
    purpose: &str,
    seed: &[u8; KEY_LEN],
    utc: &str,
) -> Result<Handover, String> {
    if from.trim().is_empty() || to.trim().is_empty() || purpose.trim().is_empty() {
        return Err("from, to and purpose must be stated".into());
    }
    let v = verify(dir, &[])?;
    if !v.passed() {
        return Err(format!(
            "the custody directory does not verify; no handover is added: {}",
            v.problems.join("; ")
        ));
    }
    let mut log = read_log(dir)?;
    let form_bytes = std::fs::read(dir.join(FORM)).map_err(|e| e.to_string())?;
    let prev = log
        .last()
        .map(|l| l.hash.clone())
        .unwrap_or_else(|| sha(&form_bytes));
    let pk = public_key_for(seed);
    let mut h = Handover {
        seq: log.len() as u64 + 1,
        utc: utc.into(),
        from: from.into(),
        to: to.into(),
        purpose: purpose.into(),
        capture_id: v.capture_id.clone(),
        prev_hash: prev,
        hash: String::new(),
        public_key: hex::encode(pk),
        signature: String::new(),
    };
    h.hash = h.digest();
    h.signature = sign(seed, h.hash.as_bytes());
    log.push(h.clone());
    std::fs::write(
        dir.join(LOG),
        serde_json::to_string_pretty(&log).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(h)
}

/// Verification result.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Verification {
    pub capture_id: String,
    pub sealer_key_id: String,
    pub collector_key_id: String,
    pub handovers: usize,
    /// Whether the sealing key is among the trusted keys (`None`: none given).
    pub trusted: Option<bool>,
    pub problems: Vec<String>,
}

impl Verification {
    /// No problem found.
    pub fn passed(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Verify a custody directory.
pub fn verify(dir: &Path, trust: &[[u8; KEY_LEN]]) -> Result<Verification, String> {
    let rd = |n: &str| std::fs::read(dir.join(n)).map_err(|e| format!("{n}: {e}"));
    let (meta_b, data_b, form_b) = (rd(META)?, rd(DATA)?, rd(FORM)?);
    let mut v = Verification {
        capture_id: sha(&data_b),
        ..Default::default()
    };
    // Sealed metadata.
    let mv: Value = serde_json::from_slice(&meta_b).map_err(|e| format!("{META}: {e}"))?;
    let g = &mv["global"];
    let field = |k: &str| g[format!("{NS}:{k}")].as_str().unwrap_or("").to_string();
    if field("data_sha256") != v.capture_id {
        v.problems
            .push("capture data does not match the sealed hash".into());
    }
    let pk = field("public_key");
    v.sealer_key_id = field("key_id");
    if !check_sig(
        &pk,
        &field("signature"),
        &canonical_without(&mv, "global", &format!("{NS}:signature")),
    ) {
        v.problems.push("metadata seal does not verify".into());
    }
    if let Ok(k) = parse_key_hex(&pk) {
        if key_id(&k) != v.sealer_key_id {
            v.problems
                .push("sealer key_id does not match its public key".into());
        }
        if !trust.is_empty() {
            v.trusted = Some(trust.contains(&k));
            if v.trusted == Some(false) {
                v.problems.push(format!(
                    "sealed by key {}, which is not trusted",
                    v.sealer_key_id
                ));
            }
        }
    }
    // Custody form.
    let fv: Value = serde_json::from_slice(&form_b).map_err(|e| format!("{FORM}: {e}"))?;
    let form: Form = serde_json::from_value(fv.clone()).map_err(|e| format!("{FORM}: {e}"))?;
    v.collector_key_id = form.key_id.clone();
    if form.format != FORM_FORMAT {
        v.problems.push(format!("{FORM}: format '{}'", form.format));
    }
    if form.capture_id != v.capture_id {
        v.problems
            .push("custody form is bound to another capture".into());
    }
    if form.meta_sha256 != sha(&meta_b) {
        v.problems
            .push("custody form is bound to other metadata".into());
    }
    if !check_sig(
        &form.public_key,
        &form.signature,
        &canonical_without(&fv, "", "signature"),
    ) {
        v.problems
            .push("custody form signature does not verify".into());
    }
    // Handover chain.
    let log = read_log(dir)?;
    let mut prev = sha(&form_b);
    for (i, h) in log.iter().enumerate() {
        let n = i as u64 + 1;
        if h.seq != n {
            v.problems
                .push(format!("handover {n}: sequence {} out of order", h.seq));
        }
        if h.prev_hash != prev {
            v.problems.push(format!(
                "handover {n}: does not chain to the previous record"
            ));
        }
        if h.capture_id != v.capture_id {
            v.problems
                .push(format!("handover {n}: names another capture"));
        }
        if h.digest() != h.hash {
            v.problems
                .push(format!("handover {n}: changed after it was recorded"));
        }
        if !check_sig(&h.public_key, &h.signature, h.hash.as_bytes()) {
            v.problems
                .push(format!("handover {n}: signature does not verify"));
        }
        prev = h.hash.clone();
    }
    v.handovers = log.len();
    Ok(v)
}

/// The whole chain as one document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Chain {
    pub format: &'static str,
    pub form: Form,
    pub handovers: Vec<Handover>,
    pub verification: Verification,
}

/// Export the chain.
pub fn export(dir: &Path, trust: &[[u8; KEY_LEN]]) -> Result<Chain, String> {
    let form: Form =
        serde_json::from_slice(&std::fs::read(dir.join(FORM)).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{FORM}: {e}"))?;
    Ok(Chain {
        format: CHAIN_FORMAT,
        form,
        handovers: read_log(dir)?,
        verification: verify(dir, trust)?,
    })
}
