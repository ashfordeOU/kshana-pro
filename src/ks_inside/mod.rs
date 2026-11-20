// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # KS-Inside: the embedded OEM SDK (backlog E39 B3901; docs/KS-INSIDE.md)
//!
//! A small, stable facade over a minimal set of Pro functions for receiver, simulator and
//! timing-appliance makers who build Kshana into their own product. Two layers:
//!
//! * this Rust facade ([`KsInside`]): open with a licence token, a unit ID (the device's
//!   serial) and a meter-log path, then [`KsInside::call`] a method by name with a text
//!   input; the answer is JSON text;
//! * a C ABI over it ([`ffi`], header `sdk/ks-inside/include/ks_inside.h`), version
//!   [`ABI_VERSION`].
//!
//! **Licence gate.** Opening needs a token that (1) verifies under the build's keys and
//! revocation lists, for the `sdk` product, named explicitly; (2) carries the explicit grant
//! `pro-ks-inside`; (3) is at the Enterprise tier the tier manifest gives that feature.
//! Each call is checked again (expiry, and the Pro kind or feature the method needs), so a
//! licence that expires while the device runs stops at the next call.
//!
//! **Metering.** Every call to a known method, allowed or refused, appends a line to the
//! meter log ([`crate::license::meter`]) before the answer is returned; if the line cannot
//! be written the call fails and the answer is dropped. The per-unit report for royalties
//! is `kshana-pro ks-inside meter summary <log>`.
//!
//! **Methods** ([`METHODS`]): deliberately few, all pure computations with no file or
//! network access:
//!
//! | Method                     | Needs                      | Input                            |
//! |----------------------------|----------------------------|----------------------------------|
//! | `scenario.run`             | the scenario's `pro-` kind | a Pro scenario as TOML text      |
//! | `timing.drift`             | `pro-grid-timing`          | JSON: time-error series          |
//! | `timing.timebomb-analyse`  | `pro-timing-timebomb`      | JSON: vector and device capture  |
//!
//! Results keep the labels of the function they come from (MODELLED, COMPUTED); the SDK
//! adds no claim of its own. No binary of this SDK is published by this repository.

pub mod ffi;

use std::path::PathBuf;

use kshana::api::KshanaError;
use serde::Deserialize;

use crate::license::activation::{self, State};
use crate::license::meter::{self, Event, MeterLog, Outcome};
use crate::license::{self, revocation, Entitlement, Keyring, Licence, Product, VerifyOptions};

/// The C ABI version. Raised only on an incompatible change of the header.
pub const ABI_VERSION: u32 = 1;

/// The licence feature (and explicit grant) the SDK needs.
pub const FEATURE: &str = "pro-ks-inside";

/// The longest input a call takes (bytes).
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// One SDK method.
#[derive(Debug, Clone, Copy)]
pub struct Method {
    pub name: &'static str,
    /// The Pro feature it needs; `None`: the `pro-` kind of the scenario it is given.
    pub feature: Option<&'static str>,
    pub input: &'static str,
    pub summary: &'static str,
}

/// Every method of this ABI version.
pub const METHODS: &[Method] = &[
    Method {
        name: "scenario.run",
        feature: None,
        input: "a Pro scenario (TOML text, kind = \"pro-...\")",
        summary: "Run a Pro scenario kind through the licence gate; the kind's own JSON result",
    },
    Method {
        name: "timing.drift",
        feature: Some(crate::timing::grid::FEATURE),
        input: "JSON {\"series\": [[unix_s, time_error_s], ...], \"detector\": {...}, \"budget_s\": n, \"name\": s}",
        summary: "Gradual-drift, step and level-shift detection on a time-error series (MODELLED)",
    },
    Method {
        name: "timing.timebomb-analyse",
        feature: Some(crate::timing::timebomb::FEATURE),
        input: "JSON {\"vector\": <vector spec>, \"capture_format\": \"nmea|csv|ubx\", \"capture\": text | \"capture_hex\": hex, \"start_index\": k, \"replay_start_lab\": s}",
        summary: "Faults in a device's time output during a future-epoch replay (COMPUTED)",
    },
];

/// Why a call or an open failed. [`CallError::code`] is the C ABI's status code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// A bad argument (null, not UTF-8, empty unit ID, input too long).
    Argument(String),
    /// The licence does not open the SDK.
    Licence(String),
    /// The licence does not allow this call (metered as refused).
    Refused(String),
    /// No such method.
    Method(String),
    /// The input was not valid for the method, or the method failed (metered as error).
    Input(String),
    /// The meter log could not be written; no answer is returned.
    Meter(String),
}

impl CallError {
    pub fn code(&self) -> i32 {
        match self {
            CallError::Argument(_) => ffi::KS_ERR_ARGUMENT,
            CallError::Licence(_) => ffi::KS_ERR_LICENCE,
            CallError::Refused(_) => ffi::KS_ERR_REFUSED,
            CallError::Method(_) => ffi::KS_ERR_METHOD,
            CallError::Input(_) => ffi::KS_ERR_INPUT,
            CallError::Meter(_) => ffi::KS_ERR_METER,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            CallError::Argument(m)
            | CallError::Licence(m)
            | CallError::Refused(m)
            | CallError::Method(m)
            | CallError::Input(m)
            | CallError::Meter(m) => m,
        }
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// What [`KsInside::open`] needs.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// The OEM licence token.
    pub token: String,
    /// The unit (device serial); stored in the meter log only as a salted hash.
    pub unit_id: String,
    pub meter_log: PathBuf,
    /// Signed revocation lists shipped with the OEM's firmware (may be empty).
    pub revocations: Vec<String>,
}

/// An open SDK instance.
#[derive(Debug)]
pub struct KsInside {
    licence: Licence,
    entitlement: Entitlement,
    unit: String,
    meter: MeterLog,
}

fn this_machine() -> Option<String> {
    license::machine::this_machine_hash()
}

impl KsInside {
    /// Open with the keys compiled into this build (debug builds without keys: the demo key).
    pub fn open(cfg: &Config, now: i64) -> Result<KsInside, CallError> {
        let ring = crate::cli::licence_keyring().map_err(|e| CallError::Licence(e.to_string()))?;
        KsInside::open_with(&ring, cfg, now)
    }

    /// Open against an explicit keyring.
    pub fn open_with(ring: &Keyring, cfg: &Config, now: i64) -> Result<KsInside, CallError> {
        let unit = cfg.unit_id.trim();
        if unit.is_empty() || unit.len() > 128 || unit.chars().any(char::is_control) {
            return Err(CallError::Argument(
                "the unit ID is 1 to 128 printable characters".into(),
            ));
        }
        let lists = cfg
            .revocations
            .iter()
            .map(|t| revocation::verify(ring, t))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CallError::Licence(e.to_string()))?;
        let opts = VerifyOptions {
            now_unix: now,
            product: Some(Product::Sdk),
            machine: Some(&this_machine),
            allow_expired: false,
        };
        let st = activation::evaluate(ring, Some(&cfg.token), &lists, &opts);
        let licence = match (&st.state, st.licence) {
            (State::Active | State::Expiring { .. }, Some(l)) => l,
            (s, _) => {
                return Err(CallError::Licence(format!(
                    "the licence does not open KS-Inside ({}): {}",
                    s.as_str(),
                    st.warning.unwrap_or_default()
                )))
            }
        };
        if !licence.claims.products.iter().any(|p| p == "sdk") {
            return Err(CallError::Licence(
                "the licence must name the 'sdk' product".into(),
            ));
        }
        if !licence.claims.features.iter().any(|f| f == FEATURE) {
            return Err(CallError::Licence(format!(
                "the licence carries no '{FEATURE}' grant (an OEM agreement adds it)"
            )));
        }
        license::check(&st.entitlement, FEATURE, now)
            .map_err(|e| CallError::Licence(e.to_string()))?;
        // The explicit grant does not lift the tier: the SDK needs both.
        if st.entitlement.tier < license::feature_min_tier(FEATURE) {
            return Err(CallError::Licence(format!(
                "KS-Inside needs the '{}' tier; the licence is '{}'",
                license::feature_min_tier(FEATURE).as_str(),
                st.entitlement.tier.as_str()
            )));
        }
        let meter = MeterLog::open(&cfg.meter_log).map_err(|e| CallError::Meter(e.to_string()))?;
        Ok(KsInside {
            unit: meter::unit_hash(&licence.token_id, unit),
            entitlement: st.entitlement,
            licence,
            meter,
        })
    }

    /// The instance's description (not metered): ABI version, licence ID, tier, expiry,
    /// unit hash and the methods.
    pub fn info(&self) -> serde_json::Value {
        serde_json::json!({
            "format": "kshana-pro-ks-inside-info/1",
            "abi_version": ABI_VERSION,
            "build": env!("CARGO_PKG_VERSION"),
            "licence": self.licence.token_id,
            "tier": self.entitlement.tier.as_str(),
            "expires_unix": self.entitlement.expires_unix,
            "unit": self.unit,
            "meter_log": self.meter.path().display().to_string(),
            "methods": METHODS.iter().map(|m| serde_json::json!({
                "name": m.name,
                "feature": m.feature.unwrap_or("(the scenario's pro- kind)"),
                "input": m.input,
                "summary": m.summary,
            })).collect::<Vec<_>>(),
        })
    }

    fn record(
        &self,
        method: &str,
        feature: &str,
        outcome: Outcome,
        input: &[u8],
        now: i64,
    ) -> Result<(), CallError> {
        self.meter
            .record(
                &Event {
                    licence: &self.licence.token_id,
                    unit_hash: &self.unit,
                    method,
                    feature,
                    outcome,
                    input,
                },
                now,
            )
            .map(|_| ())
            .map_err(|e| CallError::Meter(e.to_string()))
    }

    /// Call `method` with `input` at `now`. The answer is JSON text.
    pub fn call(&self, method: &str, input: &str, now: i64) -> Result<String, CallError> {
        let m = METHODS.iter().find(|m| m.name == method).ok_or_else(|| {
            CallError::Method(format!("no method '{method}' in ABI {ABI_VERSION}"))
        })?;
        if input.len() > MAX_INPUT_BYTES {
            return Err(CallError::Argument(format!(
                "input longer than {MAX_INPUT_BYTES} bytes"
            )));
        }
        let feature = match m.feature {
            Some(f) => f.to_string(),
            None => match crate::registry::peek_kind(input) {
                Some(k) if k.starts_with(crate::registry::PRO_KIND_PREFIX) => k,
                _ => {
                    self.record(m.name, "-", Outcome::Error, input.as_bytes(), now)?;
                    return Err(CallError::Input(
                        "scenario.run takes a Pro scenario (kind = \"pro-...\"); open kinds run in the open engine".into(),
                    ));
                }
            },
        };
        let gate = license::check(&self.entitlement, FEATURE, now)
            .and_then(|_| license::check(&self.entitlement, &feature, now));
        if let Err(e) = gate {
            self.record(m.name, &feature, Outcome::Refused, input.as_bytes(), now)?;
            return Err(CallError::Refused(e.to_string()));
        }
        let result = run_method(m.name, input, &self.entitlement, now);
        let outcome = if result.is_ok() {
            Outcome::Ok
        } else {
            Outcome::Error
        };
        self.record(m.name, &feature, outcome, input.as_bytes(), now)?;
        result.map_err(CallError::Input)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DriftInput {
    series: Vec<[f64; 2]>,
    #[serde(default)]
    detector: crate::timing::grid::DriftParams,
    #[serde(default)]
    budget_s: Option<f64>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimebombInput {
    vector: crate::timing::timebomb::VectorSpec,
    capture_format: String,
    #[serde(default)]
    capture: Option<String>,
    #[serde(default)]
    capture_hex: Option<String>,
    #[serde(default)]
    start_index: usize,
    #[serde(default)]
    replay_start_lab: Option<f64>,
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<String, String> {
    serde_json::to_string(v).map_err(|e| e.to_string())
}

fn run_method(name: &str, input: &str, ent: &Entitlement, now: i64) -> Result<String, String> {
    match name {
        "scenario.run" => crate::registry::run_gated(input, ent, now)
            .map(|o| o.json)
            .map_err(|e: KshanaError| e.to_string()),
        "timing.drift" => {
            use crate::timing::grid;
            let d: DriftInput =
                serde_json::from_str(input).map_err(|e| format!("timing.drift input: {e}"))?;
            d.detector.check()?;
            if d.series.len() < 2 {
                return Err("timing.drift needs at least two samples".into());
            }
            if d.series.iter().flatten().any(|x| !x.is_finite())
                || d.series.windows(2).any(|w| w[1][0] < w[0][0])
            {
                return Err("timing.drift: samples must be finite and in time order".into());
            }
            if d.budget_s.is_some_and(|b| !(b.is_finite() && b > 0.0)) {
                return Err("timing.drift: budget_s must be > 0".into());
            }
            let series: Vec<(f64, f64)> = d.series.iter().map(|p| (p[0], p[1])).collect();
            let mut r = grid::detect_drift(&series, &d.detector, d.budget_s);
            r.name = d.name;
            r.input = "clock".into();
            to_json(&r)
        }
        "timing.timebomb-analyse" => {
            use crate::timing::timebomb as tb;
            let t: TimebombInput = serde_json::from_str(input)
                .map_err(|e| format!("timing.timebomb-analyse input: {e}"))?;
            if t.vector.pre_s < 1
                || t.vector.post_s < 1
                || t.vector.pre_s + t.vector.post_s > 86_400
            {
                return Err(
                    "the vector's pre_s and post_s must be >= 1 s and together at most one day"
                        .into(),
                );
            }
            let bytes = match (t.capture, t.capture_hex) {
                (Some(s), None) => s.into_bytes(),
                (None, Some(h)) => hex::decode(h.trim()).map_err(|_| "capture_hex is not hex")?,
                _ => return Err("give exactly one of capture and capture_hex".into()),
            };
            let (recs, bad) = tb::read_capture(&t.capture_format, &bytes)?;
            let a = tb::analyse_with(
                &t.vector,
                &recs,
                &t.capture_format,
                bad,
                tb::Pairing {
                    start_k: t.start_index,
                    replay_start_lab: t.replay_start_lab,
                },
            );
            to_json(&a)
        }
        other => Err(format!("no method '{other}'")),
    }
}
