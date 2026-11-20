// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Signed observation provenance (E25 B2501)
//!
//! A feeder (a station, a receiver gateway, a partner's collector) that sends a batch of
//! observations as a `kshana.obs/1` file can sign it. The signature block is a sidecar file
//! next to the batch, `<batch>.prov.json`, so the batch itself is unchanged: every reader of
//! `kshana.obs/1` (the open `kshana obs check` included) still reads it, signed or not.
//!
//! The block ([`FORMAT`]) states who fed the batch (feeder id and Ed25519 public key), when
//! the feeder signed it (the feeder's own clock, as stated), where the antenna was (from the
//! batch header, or declared by the feeder when the header has no position), what the batch
//! holds (SHA-256 and length of its exact bytes, station, record counts, first and last epoch
//! time) and where it sits in the feeder's sequence (a sequence number and the SHA-256 of the
//! feeder's previous batch). The Ed25519 signature covers the canonical JSON of all of that.
//!
//! Verification needs the batch, its sidecar and a feeder keyring ([`Keyring`]) the network
//! operator keeps: which key belongs to which feeder, from when and until when. It reports:
//!
//! * `signed`: the batch is unchanged, the signature holds, the key is the feeder's and was
//!   valid at the stated signing time, and the position matches the header;
//! * `unsigned`: no sidecar; the batch is still readable and checked as `kshana.obs/1`, but
//!   nothing says who fed it (refused only when the caller requires signatures);
//! * `rejected`: anything else, with the reasons (a changed byte, a wrong key, a key outside
//!   its validity, a position that differs from the header, a broken sequence).
//!
//! What a signature proves: the holder of the feeder key produced these bytes. It does not
//! prove the observations are true, and the signing time is the feeder's clock, not an
//! independent time-stamp (for that, see [`crate::longterm`]). Signing is a Pro feature
//! (`pro-obs-provenance`); verifying needs no licence.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::license::{key_id, parse_key_hex, public_key_for, KEY_LEN};

/// The feature that unlocks signing.
pub const KIND: &str = "pro-obs-provenance";
/// The signature block's format tag.
pub const FORMAT: &str = "kshana-obs-provenance/1";
/// The feeder keyring's format tag.
pub const KEYRING_FORMAT: &str = "kshana-feeder-keyring/1";
/// The sidecar's suffix: `<batch>` + this.
pub const SIDECAR_SUFFIX: &str = ".prov.json";
/// How far (metres, horizontally and vertically) a stated position may differ from the
/// header's before it counts as a different position. Positions are copied, not
/// re-measured, so any difference beyond rounding means the two disagree.
pub const POSITION_TOLERANCE_M: f64 = 0.01;

/// Where the antenna was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_m: Option<f64>,
    /// `header` (copied from the batch's station) or `declared` (stated by the feeder; the
    /// header has no position).
    pub source: String,
}

/// The signed statement: everything in the sidecar except the signature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statement {
    pub format: String,
    pub feeder_id: String,
    pub public_key: String,
    pub key_id: String,
    /// The feeder's clock when it signed, ISO 8601 UTC, as the feeder states it.
    pub signed_utc: String,
    pub station_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    pub batch_sha256: String,
    pub batch_bytes: u64,
    pub epochs: usize,
    pub alarms: usize,
    pub corrections: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_t_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_t_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_label: Option<String>,
    /// The feeder's batch counter (0 for its first batch).
    pub seq: u64,
    /// SHA-256 of the feeder's previous batch (absent for `seq` 0 or a feeder that does not
    /// chain its batches).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_batch_sha256: Option<String>,
}

/// The sidecar file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    #[serde(flatten)]
    pub statement: Statement,
    pub signature: String,
}

/// One feeder key and when it may sign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeederKey {
    pub feeder_id: String,
    pub public_key: String,
    /// First instant the key may sign (ISO 8601 UTC).
    pub valid_from: String,
    /// The key may not sign at or after this instant (rotation or retirement).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    /// Revoked (lost or compromised) from this instant: batches the feeder says it signed at
    /// or after it are refused, and so is every batch when the revocation states no time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_since: Option<String>,
}

/// The network operator's list of feeder keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyring {
    pub format: String,
    #[serde(default)]
    pub feeders: Vec<FeederKey>,
}

impl Default for Keyring {
    fn default() -> Self {
        Keyring {
            format: KEYRING_FORMAT.into(),
            feeders: Vec::new(),
        }
    }
}

impl Keyring {
    /// Read and check a keyring file.
    pub fn from_file(path: &Path) -> Result<Keyring, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let k: Keyring =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        k.check()?;
        Ok(k)
    }

    /// Every entry well formed: the format, a feeder id, a 32-byte key, parseable times.
    pub fn check(&self) -> Result<(), String> {
        if self.format != KEYRING_FORMAT {
            return Err(format!("keyring: unknown format '{}'", self.format));
        }
        for (i, f) in self.feeders.iter().enumerate() {
            if f.feeder_id.trim().is_empty() {
                return Err(format!("keyring entry {i}: no feeder_id"));
            }
            parse_key_hex(&f.public_key).map_err(|e| format!("keyring entry {i}: {e}"))?;
            parse_time(&f.valid_from).map_err(|e| format!("keyring entry {i}: {e}"))?;
            for t in [&f.valid_until, &f.revoked_since].into_iter().flatten() {
                parse_time(t).map_err(|e| format!("keyring entry {i}: {e}"))?;
            }
        }
        Ok(())
    }

    /// Add a key (refusing an exact duplicate).
    pub fn add(&mut self, key: FeederKey) -> Result<(), String> {
        if self
            .feeders
            .iter()
            .any(|f| f.feeder_id == key.feeder_id && f.public_key == key.public_key)
        {
            return Err(format!(
                "feeder {} already has this key in the keyring",
                key.feeder_id
            ));
        }
        self.feeders.push(key);
        self.check()
    }
}

/// Unix seconds from ISO 8601 UTC (or Unix seconds).
pub(crate) fn parse_time(s: &str) -> Result<i64, String> {
    crate::forensics::inputs::parse_utc(s)
        .filter(|t| t.is_finite())
        .map(|t| t.floor() as i64)
        .ok_or_else(|| format!("'{s}' is not an ISO 8601 UTC time"))
}

/// The sidecar path of a batch.
pub fn sidecar_path(batch: &Path) -> PathBuf {
    let mut s = batch.as_os_str().to_os_string();
    s.push(SIDECAR_SUFFIX);
    PathBuf::from(s)
}

fn canonical(st: &Statement) -> Result<Vec<u8>, String> {
    // Declaration-order struct fields, no white space: canonical for a given statement.
    let mut out = FORMAT.as_bytes().to_vec();
    out.push(b'\n');
    out.extend(serde_json::to_vec(st).map_err(|e| format!("statement: {e}"))?);
    Ok(out)
}

/// What a batch holds, from `kshana::obs::check`.
struct BatchFacts {
    station_id: String,
    header_location: Option<(f64, f64, Option<f64>)>,
    epochs: usize,
    alarms: usize,
    corrections: usize,
    first_t_s: Option<f64>,
    last_t_s: Option<f64>,
    start_label: Option<String>,
}

fn facts(bytes: &[u8]) -> Result<BatchFacts, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "the batch is not UTF-8 text".to_string())?;
    let (records, report) = kshana::obs::check(text)?;
    let mut header_location = None;
    let mut start_label = None;
    let (mut first, mut last) = (None, None);
    for r in &records {
        match r {
            kshana::obs::Record::Header(h) => {
                if let (Some(lat), Some(lon)) = (h.station.lat_deg, h.station.lon_deg) {
                    header_location = Some((lat, lon, h.station.height_m));
                }
                start_label = h.start_label.clone();
            }
            kshana::obs::Record::Epoch(e) => {
                first.get_or_insert(e.epoch.t_s);
                last = Some(e.epoch.t_s);
            }
            _ => {}
        }
    }
    Ok(BatchFacts {
        station_id: report.station,
        header_location,
        epochs: report.epochs,
        alarms: report.alarms,
        corrections: report.corrections,
        first_t_s: first,
        last_t_s: last,
        start_label,
    })
}

/// How to sign a batch.
#[derive(Debug, Clone, Default)]
pub struct SignOptions {
    pub feeder_id: String,
    /// The feeder's clock, ISO 8601 UTC.
    pub signed_utc: String,
    pub seq: u64,
    /// The feeder's previous batch bytes (to chain this one to it).
    pub prev_batch_sha256: Option<String>,
    /// A position to declare when the header has none (lat, lon, height).
    pub declared_location: Option<(f64, f64, Option<f64>)>,
}

/// Sign a batch's exact bytes; returns the sidecar content. The batch must be a valid
/// `kshana.obs/1` file. A declared position is refused when the header has its own.
pub fn sign_batch(
    batch: &[u8],
    seed: &[u8; KEY_LEN],
    opts: &SignOptions,
) -> Result<Provenance, String> {
    if opts.feeder_id.trim().is_empty() {
        return Err("a provenance block needs a feeder id".into());
    }
    parse_time(&opts.signed_utc)?;
    if let Some(p) = &opts.prev_batch_sha256 {
        if p.len() != 64 || hex::decode(p).is_err() {
            return Err("the previous batch hash must be 64 hex characters".into());
        }
    }
    let f = facts(batch)?;
    let location = match (f.header_location, opts.declared_location) {
        (Some(_), Some(_)) => return Err(
            "the batch header states the station position; a declared position would contradict it"
                .into(),
        ),
        (Some((lat, lon, h)), None) => Some(Location {
            lat_deg: lat,
            lon_deg: lon,
            height_m: h,
            source: "header".into(),
        }),
        (None, Some((lat, lon, h))) => {
            if !(lat.is_finite()
                && lon.is_finite()
                && (-90.0..=90.0).contains(&lat)
                && (-180.0..=180.0).contains(&lon))
            {
                return Err("the declared position is not a valid latitude and longitude".into());
            }
            Some(Location {
                lat_deg: lat,
                lon_deg: lon,
                height_m: h,
                source: "declared".into(),
            })
        }
        (None, None) => None,
    };
    let pk = public_key_for(seed);
    let statement = Statement {
        format: FORMAT.into(),
        feeder_id: opts.feeder_id.trim().into(),
        public_key: hex::encode(pk),
        key_id: key_id(&pk),
        signed_utc: opts.signed_utc.clone(),
        station_id: f.station_id,
        location,
        batch_sha256: hex::encode(Sha256::digest(batch)),
        batch_bytes: batch.len() as u64,
        epochs: f.epochs,
        alarms: f.alarms,
        corrections: f.corrections,
        first_t_s: f.first_t_s,
        last_t_s: f.last_t_s,
        start_label: f.start_label,
        seq: opts.seq,
        prev_batch_sha256: opts.prev_batch_sha256.clone(),
    };
    let sig = SigningKey::from_bytes(seed).sign(&canonical(&statement)?);
    Ok(Provenance {
        statement,
        signature: hex::encode(sig.to_bytes()),
    })
}

/// The outcome for one batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Signed,
    Unsigned,
    Rejected,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Signed => "signed",
            Status::Unsigned => "unsigned",
            Status::Rejected => "rejected",
        }
    }
}

/// What verifying one batch found.
#[derive(Debug, Clone, Serialize)]
pub struct BatchVerification {
    pub batch: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feeder_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub station_id: Option<String>,
    pub epochs: usize,
    pub batch_sha256: String,
    pub problems: Vec<String>,
}

fn close(a: f64, b: f64, metres_per_unit: f64) -> bool {
    ((a - b) * metres_per_unit).abs() <= POSITION_TOLERANCE_M
}

/// Verify one batch against its sidecar (`None`: unsigned) and the keyring.
pub fn verify_batch(
    name: &str,
    batch: &[u8],
    prov: Option<&Provenance>,
    keyring: &Keyring,
) -> BatchVerification {
    let mut v = BatchVerification {
        batch: name.into(),
        status: Status::Rejected,
        feeder_id: None,
        key_id: None,
        seq: None,
        signed_utc: None,
        station_id: None,
        epochs: 0,
        batch_sha256: hex::encode(Sha256::digest(batch)),
        problems: Vec::new(),
    };
    let f = match facts(batch) {
        Ok(f) => f,
        Err(e) => {
            v.problems
                .push(format!("not a valid kshana.obs/1 batch: {e}"));
            return v;
        }
    };
    v.station_id = Some(f.station_id.clone());
    v.epochs = f.epochs;
    let Some(p) = prov else {
        v.status = Status::Unsigned;
        return v;
    };
    let st = &p.statement;
    v.feeder_id = Some(st.feeder_id.clone());
    v.seq = Some(st.seq);
    v.signed_utc = Some(st.signed_utc.clone());
    if st.format != FORMAT {
        v.problems
            .push(format!("unknown provenance format '{}'", st.format));
        return v;
    }
    let pk = match parse_key_hex(&st.public_key) {
        Ok(k) => k,
        Err(e) => {
            v.problems.push(format!("public key: {e}"));
            return v;
        }
    };
    v.key_id = Some(key_id(&pk));
    if key_id(&pk) != st.key_id {
        v.problems.push("key_id does not match public_key".into());
    }
    let sig: Option<[u8; 64]> = hex::decode(&p.signature)
        .ok()
        .and_then(|b| b.try_into().ok());
    let sig_ok = match (VerifyingKey::from_bytes(&pk), sig, canonical(st)) {
        (Ok(k), Some(s), Ok(msg)) => k.verify_strict(&msg, &Signature::from_bytes(&s)).is_ok(),
        _ => false,
    };
    if !sig_ok {
        v.problems.push("signature does not verify".into());
    }
    // The batch is the one signed.
    if st.batch_sha256 != v.batch_sha256 || st.batch_bytes != batch.len() as u64 {
        v.problems
            .push("the batch bytes differ from the signed batch (changed or truncated)".into());
    }
    if st.station_id != f.station_id
        || st.epochs != f.epochs
        || st.alarms != f.alarms
        || st.corrections != f.corrections
        || st.first_t_s != f.first_t_s
        || st.last_t_s != f.last_t_s
        || st.start_label != f.start_label
    {
        v.problems
            .push("the signed summary (station, counts, times) does not match the batch".into());
    }
    // Position: copied from the header, or declared only when the header has none.
    match (&st.location, f.header_location) {
        (Some(l), Some((lat, lon, h))) => {
            let m_per_deg = 111_320.0;
            let lon_scale = m_per_deg * lat.to_radians().cos().abs().max(1e-6);
            let h_ok = match (l.height_m, h) {
                (Some(a), Some(b)) => close(a, b, 1.0),
                (None, None) => true,
                _ => false,
            };
            if l.source != "header"
                || !close(l.lat_deg, lat, m_per_deg)
                || !close(l.lon_deg, lon, lon_scale)
                || !h_ok
            {
                v.problems
                    .push("the signed position differs from the batch header's".into());
            }
        }
        (Some(l), None) if l.source != "declared" => v.problems.push(
            "the signed position claims the header as source, but the header has none".into(),
        ),
        (None, Some(_)) => v
            .problems
            .push("the batch header has a position the provenance block does not state".into()),
        _ => {}
    }
    // The key belongs to this feeder and was valid when the feeder says it signed.
    let at = parse_time(&st.signed_utc);
    let candidates: Vec<&FeederKey> = keyring
        .feeders
        .iter()
        .filter(|k| {
            k.feeder_id == st.feeder_id
                && parse_key_hex(&k.public_key)
                    .map(|x| x == pk)
                    .unwrap_or(false)
        })
        .collect();
    if candidates.is_empty() {
        v.problems.push(format!(
            "key {} is not a key of feeder '{}' in the keyring",
            key_id(&pk),
            st.feeder_id
        ));
    } else {
        match at {
            Err(e) => v.problems.push(format!("signed_utc: {e}")),
            Ok(t) => {
                let reason = candidates
                    .iter()
                    .map(|k| key_window_problem(k, t))
                    .collect::<Vec<_>>();
                if reason.iter().all(Option::is_some) {
                    v.problems.push(reason[0].clone().unwrap_or_default());
                }
            }
        }
    }
    if v.problems.is_empty() {
        v.status = Status::Signed;
    }
    v
}

/// Why `k` could not sign at `t`, or `None` if it could.
fn key_window_problem(k: &FeederKey, t: i64) -> Option<String> {
    let from = parse_time(&k.valid_from).ok()?;
    if let Some(r) = &k.revoked_since {
        if parse_time(r).map(|r| t >= r).unwrap_or(true) {
            return Some(format!(
                "the feeder key was revoked from {r}, before the stated signing time"
            ));
        }
    }
    if t < from {
        return Some(format!(
            "the stated signing time is before the key's validity ({})",
            k.valid_from
        ));
    }
    if let Some(u) = &k.valid_until {
        if parse_time(u).map(|u| t >= u).unwrap_or(true) {
            return Some(format!(
                "the stated signing time is after the key's validity ended ({u})"
            ));
        }
    }
    None
}

/// Check each feeder's sequence over a set of verified batches: numbers rising without a
/// gap or repeat, and each batch naming its predecessor's hash where it chains. Returns the
/// problems (empty when every sequence holds). Unsigned and rejected batches are ignored.
pub fn check_sequences(
    batches: &[BatchVerification],
    provs: &[Option<&Provenance>],
) -> Vec<String> {
    // Per feeder: (sequence, batch hash, predecessor hash, batch name).
    type Link<'a> = (u64, &'a str, Option<&'a str>, &'a str);
    let mut by_feeder: BTreeMap<&str, Vec<Link>> = BTreeMap::new();
    for (b, p) in batches.iter().zip(provs) {
        if b.status != Status::Signed {
            continue;
        }
        let Some(p) = p else { continue };
        by_feeder.entry(&p.statement.feeder_id).or_default().push((
            p.statement.seq,
            b.batch_sha256.as_str(),
            p.statement.prev_batch_sha256.as_deref(),
            b.batch.as_str(),
        ));
    }
    let mut problems = Vec::new();
    for (feeder, mut seqs) in by_feeder {
        seqs.sort_by_key(|s| s.0);
        for w in seqs.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.0 == b.0 {
                problems.push(format!(
                    "feeder '{feeder}': sequence {} appears twice ({} and {})",
                    a.0, a.3, b.3
                ));
            } else if b.0 != a.0 + 1 {
                problems.push(format!(
                    "feeder '{feeder}': sequence jumps from {} to {} (missing batches)",
                    a.0, b.0
                ));
            } else if let Some(prev) = b.2 {
                if prev != a.1 {
                    problems.push(format!(
                        "feeder '{feeder}': batch {} does not name batch {} as its predecessor",
                        b.3, a.3
                    ));
                }
            }
        }
    }
    problems
}

/// Read a sidecar.
pub fn read_sidecar(path: &Path) -> Result<Provenance, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The batches to verify: a file, or every `*.jsonl` file in a directory (sorted).
pub fn batch_files(target: &Path) -> Result<Vec<PathBuf>, String> {
    if target.is_dir() {
        let mut out: Vec<PathBuf> = std::fs::read_dir(target)
            .map_err(|e| format!("{}: {e}", target.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        out.sort();
        Ok(out)
    } else {
        Ok(vec![target.to_path_buf()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; KEY_LEN] = [11; KEY_LEN];

    fn batch(lat: Option<f64>, n: usize) -> Vec<u8> {
        let station = kshana::obs::Station {
            id: "ST-01".into(),
            lat_deg: lat,
            lon_deg: lat.map(|_| 24.75),
            height_m: lat.map(|_| 31.2),
            ..Default::default()
        };
        let tl = kshana::receiver_trust::Timeline {
            epochs: (0..n)
                .map(|i| kshana::receiver_trust::LogEpoch {
                    t_s: i as f64,
                    agc: Some(4000.0 + i as f64),
                    ..Default::default()
                })
                .collect(),
            start_label: Some("2026-11-20T08:00:00Z".into()),
            observables: vec!["agc".into()],
            skipped_records: 0,
        };
        kshana::obs::to_jsonl(&kshana::obs::from_timeline(&tl, &station, Some("ubx")))
            .unwrap()
            .into_bytes()
    }

    fn ring() -> Keyring {
        Keyring {
            format: KEYRING_FORMAT.into(),
            feeders: vec![FeederKey {
                feeder_id: "feeder-a".into(),
                public_key: hex::encode(public_key_for(&SEED)),
                valid_from: "2026-01-01T00:00:00Z".into(),
                valid_until: Some("2027-01-01T00:00:00Z".into()),
                revoked_since: None,
            }],
        }
    }

    fn opts(seq: u64) -> SignOptions {
        SignOptions {
            feeder_id: "feeder-a".into(),
            signed_utc: "2026-11-20T08:01:00Z".into(),
            seq,
            ..Default::default()
        }
    }

    #[test]
    fn signed_batch_verifies_and_every_tamper_is_rejected() {
        let b = batch(Some(59.43), 5);
        let p = sign_batch(&b, &SEED, &opts(0)).unwrap();
        assert_eq!(p.statement.location.as_ref().unwrap().source, "header");
        let v = verify_batch("b", &b, Some(&p), &ring());
        assert_eq!(v.status, Status::Signed, "{:?}", v.problems);
        // One changed byte in the batch.
        let mut t = b.clone();
        let i = t.iter().position(|&c| c == b'4').unwrap();
        t[i] = b'5';
        assert_eq!(
            verify_batch("b", &t, Some(&p), &ring()).status,
            Status::Rejected
        );
        // A truncated batch (last epoch dropped) is still valid obs/1 but not the signed one.
        let cut = batch(Some(59.43), 4);
        assert_eq!(
            verify_batch("b", &cut, Some(&p), &ring()).status,
            Status::Rejected
        );
        // An edited statement (the feeder's clock) breaks the signature.
        let mut q = p.clone();
        q.statement.signed_utc = "2026-11-20T09:00:00Z".into();
        assert!(verify_batch("b", &b, Some(&q), &ring())
            .problems
            .iter()
            .any(|x| x.contains("signature")));
        // Another feeder's key.
        let other = sign_batch(&b, &[12; KEY_LEN], &opts(0)).unwrap();
        assert_eq!(
            verify_batch("b", &b, Some(&other), &ring()).status,
            Status::Rejected
        );
    }

    #[test]
    fn unsigned_batches_stay_readable() {
        let b = batch(None, 3);
        let v = verify_batch("b", &b, None, &ring());
        assert_eq!(v.status, Status::Unsigned);
        assert_eq!(v.epochs, 3);
        assert!(v.problems.is_empty());
    }

    #[test]
    fn location_rules() {
        // Header has a position: a declared one is refused.
        let b = batch(Some(59.43), 2);
        let mut o = opts(0);
        o.declared_location = Some((59.0, 24.0, None));
        assert!(sign_batch(&b, &SEED, &o).is_err());
        // No header position: the declared one is signed and accepted.
        let b = batch(None, 2);
        let p = sign_batch(&b, &SEED, &o).unwrap();
        assert_eq!(p.statement.location.as_ref().unwrap().source, "declared");
        assert_eq!(
            verify_batch("b", &b, Some(&p), &ring()).status,
            Status::Signed
        );
    }

    #[test]
    fn key_windows_and_revocation() {
        let b = batch(Some(59.43), 2);
        let mut o = opts(0);
        o.signed_utc = "2027-02-01T00:00:00Z".into();
        let p = sign_batch(&b, &SEED, &o).unwrap();
        let v = verify_batch("b", &b, Some(&p), &ring());
        assert!(v.problems.iter().any(|x| x.contains("validity ended")));
        let mut r = ring();
        r.feeders[0].revoked_since = Some("2026-11-01T00:00:00Z".into());
        let p = sign_batch(&b, &SEED, &opts(0)).unwrap();
        let v = verify_batch("b", &b, Some(&p), &r);
        assert!(v.problems.iter().any(|x| x.contains("revoked")));
        // Revoked after the stated signing time: still accepted.
        r.feeders[0].revoked_since = Some("2026-12-01T00:00:00Z".into());
        assert_eq!(verify_batch("b", &b, Some(&p), &r).status, Status::Signed);
    }

    #[test]
    fn sequences_detect_gaps_repeats_and_broken_links() {
        let b0 = batch(Some(59.43), 2);
        let b1 = batch(Some(59.43), 3);
        let b2 = batch(Some(59.43), 4);
        let h0 = hex::encode(Sha256::digest(&b0));
        let h1 = hex::encode(Sha256::digest(&b1));
        let p0 = sign_batch(&b0, &SEED, &opts(0)).unwrap();
        let mut o1 = opts(1);
        o1.prev_batch_sha256 = Some(h0.clone());
        let p1 = sign_batch(&b1, &SEED, &o1).unwrap();
        let mut o2 = opts(2);
        o2.prev_batch_sha256 = Some(h1);
        let p2 = sign_batch(&b2, &SEED, &o2).unwrap();
        let r = ring();
        let run = |set: &[(&[u8], &Provenance)]| {
            let vs: Vec<_> = set
                .iter()
                .enumerate()
                .map(|(i, (b, p))| verify_batch(&format!("b{i}"), b, Some(p), &r))
                .collect();
            let ps: Vec<_> = set.iter().map(|(_, p)| Some(*p)).collect();
            check_sequences(&vs, &ps)
        };
        assert!(run(&[(&b0, &p0), (&b1, &p1), (&b2, &p2)]).is_empty());
        assert!(run(&[(&b0, &p0), (&b2, &p2)])[0].contains("jumps"));
        assert!(run(&[(&b0, &p0), (&b0, &p0)])[0].contains("twice"));
        // b2 claims b1 as predecessor; put b0 in b1's place with sequence 1.
        let mut o = opts(1);
        o.prev_batch_sha256 = Some(h0);
        let p0b = sign_batch(&b0, &SEED, &o).unwrap();
        assert!(run(&[(&b0, &p0), (&b0, &p0b), (&b2, &p2)])
            .iter()
            .any(|x| x.contains("predecessor")));
    }
}
