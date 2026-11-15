// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Results-only service API (roadmap 0.42 X6; docs/PROTECTION.md section 7)
//!
//! A generic gateway for methods whose working must never leave the server: inputs in, a
//! signed verdict out. This crate holds the mechanism only; which kinds run through it, and
//! how they compute, is registered by a server-side extension ([`register_kind`]) and is not
//! in this repository. The hosted Pro service for trade-secret methods (C3,
//! [`crate::serve::hosted`]) is built on it.
//!
//! What a caller gets for one query, and nothing else:
//!
//! * the **verdict**, one word from the closed set the kind declares
//!   ([`ResultsOnlyKind::verdicts`]); a kind that tries to answer anything outside that set
//!   is refused, so no number can be smuggled out through the verdict;
//! * the **evidence class** ([`EvidenceClass`]: measured, modelled, reported or mixed);
//! * the SHA-256 of the canonical input, a query ID, the time, the key ID, a per-licensee
//!   **watermark** ([`crate::watermark`], C4), and an Ed25519 **signature** over all of it
//!   through a [`LicenceSigner`] (a seed or AWS KMS).
//!
//! Intermediates (the full result the method computed) stay inside [`Gateway::query`]; they
//! are dropped after reduction and never logged or stored.
//!
//! Abuse controls, per tenant:
//!
//! * a **query budget** and a **compute budget** per period (a calendar month by default),
//!   persisted in the data directory so a restart does not reset them;
//! * **probing alerts**: queries whose inputs differ only in small numeric changes (same
//!   shape after rounding every number to two significant figures, different exact input)
//!   are counted in a sliding window; at `probe_alert_after` an alert is raised (access log
//!   line and the alert sink), at `probe_block_after` the tenant is refused until the window
//!   has passed. This is the pattern of someone sweeping parameters around a point to map
//!   the method's decision boundary;
//! * an **access log**: one hash-chained JSON line per query (time, tenant, kind, query ID,
//!   input hash, verdict or refusal). It holds no input values, so no customer data.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use kshana::api::KshanaError;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::license::signer::{sign_document, LicenceSigner, SharedSigner};
use crate::watermark::{self, ResultMark};

/// The envelope format.
pub const FORMAT: &str = "kshana-pro-results-only/1";

/// What kind of evidence a verdict rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceClass {
    Measured,
    Modelled,
    Reported,
    Mixed,
}

/// The reduced answer of a kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub verdict: String,
    pub evidence_class: EvidenceClass,
}

/// A method served results-only. Implemented by server-side extensions.
pub trait ResultsOnlyKind: Send + Sync {
    /// The kind name (carries the `pro-` prefix and is listed in the tier manifest).
    fn kind(&self) -> &str;
    /// The closed set of verdict words this kind may answer (each 1 to 32 of a-z, 0-9, -).
    fn verdicts(&self) -> &[&str];
    /// Run the method. The full result never leaves the gateway.
    fn run(&self, input: &Value) -> Result<Value, KshanaError>;
    /// Reduce a full result to its verdict.
    fn reduce(&self, full: &Value) -> Result<Verdict, KshanaError>;
}

static KINDS: OnceLock<RwLock<Vec<Arc<dyn ResultsOnlyKind>>>> = OnceLock::new();

fn kinds() -> &'static RwLock<Vec<Arc<dyn ResultsOnlyKind>>> {
    KINDS.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register a results-only kind for this process (a server-side extension does this at
/// start). A second registration of the same kind name replaces the first.
pub fn register_kind(k: Arc<dyn ResultsOnlyKind>) {
    let mut v = kinds().write().unwrap_or_else(|p| p.into_inner());
    v.retain(|x| x.kind() != k.kind());
    v.push(k);
}

/// The registered kinds' names.
pub fn registered_kinds() -> Vec<String> {
    kinds()
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .map(|k| k.kind().to_string())
        .collect()
}

fn find_kind(name: &str) -> Option<Arc<dyn ResultsOnlyKind>> {
    kinds()
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find(|k| k.kind() == name)
        .cloned()
}

/// Budgets and probing limits.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Queries per tenant per period (0: no query limit).
    pub queries_per_period: u32,
    /// Compute milliseconds per tenant per period (0: no compute limit).
    pub compute_ms_per_period: u64,
    /// Sliding window for probing detection, seconds.
    pub probe_window_secs: i64,
    /// Near-identical queries in the window that raise an alert.
    pub probe_alert_after: u32,
    /// Near-identical queries in the window after which the tenant is refused.
    pub probe_block_after: u32,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        GatewayConfig {
            queries_per_period: 1000,
            compute_ms_per_period: 3_600_000,
            probe_window_secs: 3600,
            probe_alert_after: 10,
            probe_block_after: 25,
        }
    }
}

/// A tenant's allowance, overriding the gateway default (C3: per subscription).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Allowance {
    #[serde(default)]
    pub queries_per_period: Option<u32>,
    #[serde(default)]
    pub compute_ms_per_period: Option<u64>,
}

/// One query.
pub struct Query<'a> {
    /// The tenant (subscription) the query is charged to.
    pub tenant: &'a str,
    /// The licence holder, for the watermark.
    pub licensee: &'a str,
    pub kind: &'a str,
    pub input: &'a Value,
    /// Overrides the default budgets for this tenant.
    pub allowance: Option<&'a Allowance>,
}

/// Why a query was not answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    UnknownKind,
    QueryBudget {
        retry_after: i64,
    },
    ComputeBudget {
        retry_after: i64,
    },
    Probing {
        retry_after: i64,
    },
    BadInput(String),
    /// The kind failed or answered outside its verdict set (detail not shown to callers).
    MethodFailed,
    Internal(String),
}

impl Refusal {
    /// HTTP status for the refusal.
    pub fn status(&self) -> u16 {
        match self {
            Refusal::UnknownKind => 404,
            Refusal::QueryBudget { .. }
            | Refusal::ComputeBudget { .. }
            | Refusal::Probing { .. } => 429,
            Refusal::BadInput(_) => 422,
            Refusal::MethodFailed | Refusal::Internal(_) => 500,
        }
    }
    /// Text for the caller (names no method detail).
    pub fn message(&self) -> String {
        match self {
            Refusal::UnknownKind => "no such results-only kind".into(),
            Refusal::QueryBudget { .. } => "query allowance for this period used up".into(),
            Refusal::ComputeBudget { .. } => "compute allowance for this period used up".into(),
            Refusal::Probing { .. } => "too many near-identical queries; try again later".into(),
            Refusal::BadInput(m) => format!("invalid input: {m}"),
            Refusal::MethodFailed => "the method could not produce a verdict for this input".into(),
            Refusal::Internal(_) => "internal error".into(),
        }
    }
    /// Seconds after which a retry may succeed.
    pub fn retry_after(&self) -> Option<i64> {
        match self {
            Refusal::QueryBudget { retry_after }
            | Refusal::ComputeBudget { retry_after }
            | Refusal::Probing { retry_after } => Some(*retry_after),
            _ => None,
        }
    }
    fn code(&self) -> &'static str {
        match self {
            Refusal::UnknownKind => "unknown-kind",
            Refusal::QueryBudget { .. } => "query-budget",
            Refusal::ComputeBudget { .. } => "compute-budget",
            Refusal::Probing { .. } => "probing",
            Refusal::BadInput(_) => "bad-input",
            Refusal::MethodFailed => "method-failed",
            Refusal::Internal(_) => "internal",
        }
    }
}

/// The signed answer's payload.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    pub format: String,
    pub query_id: String,
    pub kind: String,
    pub verdict: String,
    pub evidence_class: EvidenceClass,
    pub input_sha256: String,
    pub issued_unix: i64,
    pub kid: String,
    pub watermark: ResultMark,
}

impl Envelope {
    /// The bytes the watermark is bound to: every field except the watermark.
    pub fn marked_content(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}|{:?}|{}|{}|{}",
            self.format,
            self.query_id,
            self.kind,
            self.verdict,
            self.evidence_class,
            self.input_sha256,
            self.issued_unix,
            self.kid
        )
        .into_bytes()
    }
}

/// A probing alert.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ProbeAlert {
    pub t: i64,
    pub tenant: String,
    pub kind: String,
    /// Near-identical queries in the window.
    pub count: u32,
    pub blocked: bool,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Usage {
    /// Period index (months since 1970-01 for the default monthly period).
    period: i64,
    queries: BTreeMap<String, u32>,
    compute_ms: BTreeMap<String, u64>,
}

#[derive(Default)]
struct State {
    usage: Usage,
    /// Per tenant: (time, shape hash, exact hash) of recent queries.
    recent: BTreeMap<String, VecDeque<(i64, String, String)>>,
    blocked_until: BTreeMap<String, i64>,
    log_prev: String,
}

/// Receives probing alerts.
pub type AlertSink = Box<dyn Fn(&ProbeAlert) + Send + Sync>;

/// The gateway.
pub struct Gateway {
    pub config: GatewayConfig,
    signer: SharedSigner,
    watermark_key: [u8; 32],
    dir: Option<PathBuf>,
    state: Mutex<State>,
    alert_sink: Option<AlertSink>,
}

/// Months since January 1970 for Unix seconds (the budget period).
pub fn month_index(unix: i64) -> i64 {
    let days = unix.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y - 1970) * 12 + (m - 1)
}

/// Unix seconds at the start of month index `i`.
fn month_start(i: i64) -> i64 {
    let y = 1970 + i.div_euclid(12);
    let m = i.rem_euclid(12) + 1;
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) * 86_400
}

/// The canonical JSON of `v` with every number rounded to two significant figures: two
/// inputs with the same shape differ only by small numeric changes.
fn shape(v: &Value) -> Value {
    match v {
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(0.0);
            if x == 0.0 || !x.is_finite() {
                Value::from(0)
            } else {
                let mag = x.abs().log10().floor() as i32 - 1;
                let r = (x / 10f64.powi(mag)).round() * 10f64.powi(mag);
                Value::String(format!("{r:.3e}"))
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(shape).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), shape(v))).collect()),
        other => other.clone(),
    }
}

fn sha_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

impl Gateway {
    /// A gateway signing with `signer`, marking with `watermark_key`, keeping its budget
    /// file and access log in `dir` (none: in memory only, for tests).
    pub fn new(
        config: GatewayConfig,
        signer: SharedSigner,
        watermark_key: [u8; 32],
        dir: Option<&Path>,
    ) -> Result<Gateway, KshanaError> {
        let mut st = State::default();
        if let Some(d) = dir {
            std::fs::create_dir_all(d).map_err(|e| {
                KshanaError::InvalidInput(format!("could not create {}: {e}", d.display()))
            })?;
            if let Ok(t) = std::fs::read_to_string(d.join(USAGE_FILE)) {
                st.usage = serde_json::from_str(&t).unwrap_or_default();
            }
            if let Ok(t) = std::fs::read_to_string(d.join(LOG_FILE)) {
                if let Some(last) = t.lines().last() {
                    st.log_prev = sha_hex(last);
                }
            }
        }
        Ok(Gateway {
            config,
            signer,
            watermark_key,
            dir: dir.map(Path::to_path_buf),
            state: Mutex::new(st),
            alert_sink: None,
        })
    }

    /// Call `sink` for every probing alert (besides the access log line).
    pub fn with_alert_sink(mut self, sink: AlertSink) -> Gateway {
        self.alert_sink = Some(sink);
        self
    }

    /// The access log path, when the gateway has a directory.
    pub fn access_log(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(LOG_FILE))
    }

    /// Answer one query: a signed envelope (`payload_hex.signature_hex`) or a refusal.
    pub fn query(&self, q: &Query<'_>, now: i64) -> Result<String, Refusal> {
        let input_canon = crate::serve::jobs::canonical_json(q.input);
        let input_sha = sha_hex(&input_canon);
        let query_id = {
            use rand::RngCore;
            let mut b = [0u8; 12];
            rand::rngs::OsRng.fill_bytes(&mut b);
            format!("rq_{}", hex::encode(b))
        };
        let r = self.query_inner(q, now, &input_sha, &input_canon, &query_id);
        let (verdict, status) = match &r {
            Ok((env, _)) => (env.verdict.clone(), "answered".to_string()),
            Err(f) => ("-".to_string(), format!("refused:{}", f.code())),
        };
        self.log(serde_json::json!({
            "t": now, "tenant": q.tenant, "kind": q.kind, "query_id": query_id,
            "input_sha256": input_sha, "verdict": verdict, "status": status,
        }))
        .map_err(Refusal::Internal)?;
        r.map(|(_, text)| text)
    }

    fn query_inner(
        &self,
        q: &Query<'_>,
        now: i64,
        input_sha: &str,
        input_canon: &str,
        query_id: &str,
    ) -> Result<(Envelope, String), Refusal> {
        let kind = find_kind(q.kind).ok_or(Refusal::UnknownKind)?;
        if input_canon.len() > 1 << 20 {
            return Err(Refusal::BadInput("input larger than 1 MiB".into()));
        }
        let q_limit = q
            .allowance
            .and_then(|a| a.queries_per_period)
            .unwrap_or(self.config.queries_per_period);
        let c_limit = q
            .allowance
            .and_then(|a| a.compute_ms_per_period)
            .unwrap_or(self.config.compute_ms_per_period);
        // Budgets and probing, under the lock; the method runs outside it.
        {
            let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let period = month_index(now);
            if st.usage.period != period {
                st.usage = Usage {
                    period,
                    ..Default::default()
                };
            }
            let next = month_start(period + 1) - now;
            if let Some(until) = st.blocked_until.get(q.tenant).copied() {
                if until > now {
                    return Err(Refusal::Probing {
                        retry_after: until - now,
                    });
                }
                st.blocked_until.remove(q.tenant);
            }
            let used_q = st.usage.queries.get(q.tenant).copied().unwrap_or(0);
            if q_limit > 0 && used_q >= q_limit {
                return Err(Refusal::QueryBudget { retry_after: next });
            }
            let used_c = st.usage.compute_ms.get(q.tenant).copied().unwrap_or(0);
            if c_limit > 0 && used_c >= c_limit {
                return Err(Refusal::ComputeBudget { retry_after: next });
            }
            // Probing: same shape, different exact input, within the window.
            let sh = sha_hex(&crate::serve::jobs::canonical_json(&shape(q.input)));
            let win = self.config.probe_window_secs;
            let recent = st.recent.entry(q.tenant.to_string()).or_default();
            while recent.front().is_some_and(|(t, _, _)| *t <= now - win) {
                recent.pop_front();
            }
            recent.push_back((now, sh.clone(), input_sha.to_string()));
            if recent.len() > 10_000 {
                recent.pop_front();
            }
            let mut exact: Vec<&str> = recent
                .iter()
                .filter(|(_, s, _)| *s == sh)
                .map(|(_, _, e)| e.as_str())
                .collect();
            exact.sort_unstable();
            exact.dedup();
            let near = exact.len() as u32;
            if near >= self.config.probe_alert_after && self.config.probe_alert_after > 0 {
                let blocked =
                    self.config.probe_block_after > 0 && near >= self.config.probe_block_after;
                let alert = ProbeAlert {
                    t: now,
                    tenant: q.tenant.into(),
                    kind: q.kind.into(),
                    count: near,
                    blocked,
                };
                if blocked {
                    st.blocked_until.insert(q.tenant.to_string(), now + win);
                    st.recent.remove(q.tenant);
                }
                drop(st);
                self.log(serde_json::json!({
                    "t": now, "tenant": q.tenant, "kind": q.kind, "alert": "probing",
                    "count": near, "blocked": blocked,
                }))
                .map_err(Refusal::Internal)?;
                if let Some(s) = &self.alert_sink {
                    s(&alert);
                }
                if blocked {
                    return Err(Refusal::Probing { retry_after: win });
                }
            }
        }
        let started = std::time::Instant::now();
        let full = kind.run(q.input);
        let ms = started.elapsed().as_millis() as u64;
        {
            let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
            *st.usage.queries.entry(q.tenant.to_string()).or_default() += 1;
            *st.usage.compute_ms.entry(q.tenant.to_string()).or_default() += ms.max(1);
            if let Some(d) = &self.dir {
                let text = serde_json::to_vec(&st.usage).unwrap_or_default();
                crate::serve::jobs::write_atomic(&d.join(USAGE_FILE), &text)
                    .map_err(|e| Refusal::Internal(e.to_string()))?;
            }
        }
        let full = full.map_err(|e| match e {
            KshanaError::InvalidInput(m) => Refusal::BadInput(first_line(&m)),
            _ => Refusal::MethodFailed,
        })?;
        let v = kind.reduce(&full).map_err(|_| Refusal::MethodFailed)?;
        drop(full); // intermediates end here
        if !valid_word(&v.verdict) || !kind.verdicts().contains(&v.verdict.as_str()) {
            return Err(Refusal::MethodFailed);
        }
        let kid = self
            .signer
            .kid()
            .map_err(|e| Refusal::Internal(e.to_string()))?;
        let mut env = Envelope {
            format: FORMAT.into(),
            query_id: query_id.into(),
            kind: q.kind.into(),
            verdict: v.verdict,
            evidence_class: v.evidence_class,
            input_sha256: input_sha.into(),
            issued_unix: now,
            kid,
            watermark: ResultMark {
                format: String::new(),
                nonce: String::new(),
                mark: String::new(),
            },
        };
        env.watermark = watermark::mark(&self.watermark_key, q.licensee, &env.marked_content());
        let payload = serde_json::to_vec(&env).map_err(|e| Refusal::Internal(e.to_string()))?;
        let text = sign_document(self.signer.as_ref(), &payload)
            .map_err(|e| Refusal::Internal(e.to_string()))?;
        Ok((env, text))
    }

    fn log(&self, mut line: Value) -> Result<(), String> {
        let Some(d) = &self.dir else { return Ok(()) };
        let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
        line["prev"] = Value::String(st.log_prev.clone());
        let text = line.to_string();
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(d.join(LOG_FILE))
            .map_err(|e| e.to_string())?;
        writeln!(f, "{text}").map_err(|e| e.to_string())?;
        st.log_prev = sha_hex(&text);
        Ok(())
    }
}

const USAGE_FILE: &str = "results-only-usage.json";
const LOG_FILE: &str = "results-only-access.log";

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").chars().take(200).collect()
}

fn valid_word(w: &str) -> bool {
    (1..=32).contains(&w.len())
        && w.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Verify a signed envelope against trusted public keys; the caller (or anyone holding the
/// public key) can check the verdict is genuine. Tracing the watermark needs the vendor key.
pub fn verify_envelope(keys: &[[u8; 32]], text: &str) -> Result<Envelope, KshanaError> {
    let (payload, sig) = crate::license::split_signed(text)?;
    let env: Envelope = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("results-only envelope: {e}")))?;
    let sig = ed25519_dalek::Signature::from_bytes(&sig);
    let ok = keys.iter().any(|k| {
        crate::license::key_id(k).eq_ignore_ascii_case(&env.kid)
            && ed25519_dalek::VerifyingKey::from_bytes(k)
                .map(|vk| vk.verify_strict(&payload, &sig).is_ok())
                .unwrap_or(false)
    });
    if !ok {
        return Err(KshanaError::InvalidInput(
            "results-only envelope: signature does not verify under a trusted key".into(),
        ));
    }
    if env.format != FORMAT {
        return Err(KshanaError::InvalidInput(
            "results-only envelope: unknown format".into(),
        ));
    }
    Ok(env)
}

/// Verify the access log's hash chain; returns the number of lines.
pub fn verify_access_log(path: &Path) -> Result<usize, KshanaError> {
    let t = std::fs::read_to_string(path)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
    let mut prev = String::new();
    for (i, line) in t.lines().enumerate() {
        let v: Value = serde_json::from_str(line)
            .map_err(|e| KshanaError::InvalidInput(format!("line {}: {e}", i + 1)))?;
        if v["prev"].as_str() != Some(prev.as_str()) {
            return Err(KshanaError::InvalidInput(format!(
                "access log chain broken at line {}",
                i + 1
            )));
        }
        prev = sha_hex(line);
    }
    Ok(t.lines().count())
}

/// The signer's public key, for callers verifying envelopes.
pub fn public_key(signer: &dyn LicenceSigner) -> Result<[u8; 32], KshanaError> {
    signer.public_key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_and_shapes() {
        assert_eq!(month_index(0), 0);
        assert_eq!(month_start(0), 0);
        // 2026-11-15 is in month (2026-1970)*12 + 10.
        let t = 1_794_700_800; // 2026-11-15T00:00:00Z
        assert_eq!(month_index(t), 56 * 12 + 10);
        assert_eq!(month_start(56 * 12 + 10), 1_793_491_200); // 2026-11-01
        assert_eq!(month_index(month_start(56 * 12 + 11)), 56 * 12 + 11);
        let a = serde_json::json!({"x": 12.31, "y": [1000.4, "s"]});
        let b = serde_json::json!({"x": 12.29, "y": [1000.1, "s"]});
        let c = serde_json::json!({"x": 19.0, "y": [1000.1, "s"]});
        assert_eq!(shape(&a), shape(&b));
        assert_ne!(shape(&a), shape(&c));
        assert!(valid_word("pass") && valid_word("not-assessed"));
        assert!(!valid_word("0.123 dB") && !valid_word("") && !valid_word("PASS"));
    }
}
