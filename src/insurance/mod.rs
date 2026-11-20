// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Insurance and finance analytics (research 2026-11-20 gaps, epic E28)
//!
//! Calculations an underwriter, broker, risk manager or claims handler can run on their own
//! data, with every assumption stated and every input hashed:
//!
//! * [`trigger`]: a parametric PNT-outage trigger evaluated locally on the user's own station
//!   C/N0 series (B2801, local part): outage episodes per site, the k-of-n site rule, an outage
//!   index, modelled payouts under the user's (or a synthetic) cover schedule, and a backtest
//!   against archived events with their documented basis.
//! * [`basis`]: basis risk of such a trigger against the insureds' own loss records, and the
//!   regional accumulation of a portfolio under historical event footprints and deterministic
//!   scenarios (B2802).
//! * [`bi`]: business-interruption and loss quantification (B2803): timing outage BI for
//!   telecom and finance (SVN-23 class), farm yield, port delay, grounding severity and fleet
//!   cascade, each with its assumptions and a one-at-a-time sensitivity table.
//! * [`feed`]: underwriting variables per vessel or aircraft from tracks and interference
//!   events, as a schema'd feed, and a claims triage aid (B2805).
//!
//! Wording: every output is evidence, an indicator or a calculation for the user's own
//! decision. It is not insurance or financial advice, an underwriting or claims decision, a
//! rating, a price or a guarantee of any payout or loss figure. Cover schedules and wordings in
//! the examples are synthetic and labelled so. Evidence: **MODELLED** throughout; nothing here
//! is fitted to claims or loss data.

pub mod basis;
pub mod bi;
pub mod feed;
pub mod trigger;

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

pub use crate::briefings::{
    esc, html_head, iso, read_input, resolve, sig9, utc, write_json, write_text, InputRef,
};

/// `kshana-pro insurance trigger`.
pub const FEATURE_TRIGGER: &str = "pro-parametric-trigger";
/// `kshana-pro insurance basis`.
pub const FEATURE_BASIS: &str = "pro-basis-risk";
/// `kshana-pro insurance accumulation` (portfolio scale).
pub const FEATURE_ACCUMULATION: &str = "pro-accumulation";
/// `kshana-pro insurance bi`.
pub const FEATURE_BI: &str = "pro-bi-quantification";
/// `kshana-pro insurance feed`.
pub const FEATURE_FEED: &str = "pro-underwriting-feed";
/// `kshana-pro insurance triage`.
pub const FEATURE_TRIAGE: &str = "pro-claims-triage";

/// Every licence feature of this module.
pub const FEATURES: &[&str] = &[
    FEATURE_TRIGGER,
    FEATURE_BASIS,
    FEATURE_ACCUMULATION,
    FEATURE_BI,
    FEATURE_FEED,
    FEATURE_TRIAGE,
];

/// What every output says about itself.
pub const STATEMENT: &str = "Evidence, indicators and calculations for the user's own decision, \
computed from the inputs listed under stated assumptions. Not insurance or financial advice, not an \
underwriting, pricing or claims decision, not a rating and not a guarantee of any payout or loss \
figure. Cover terms and wordings are the user's own or synthetic examples labelled as such.";

/// SHA-256 (hex) of the compact JSON of `v`: the reproducibility fingerprint of a result.
/// Struct fields serialise in declaration order and maps are ordered, so equal inputs give an
/// equal fingerprint on every machine.
pub fn fingerprint<T: Serialize>(v: &T) -> String {
    let b = serde_json::to_vec(v).unwrap_or_default();
    hex::encode(Sha256::digest(&b))
}

/// Read a TOML request and remember its directory.
pub fn read_request<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<(T, String, Option<PathBuf>), String> {
    let (r, src) = crate::briefings::read_request(path)?;
    Ok((r, src, path.parent().map(Path::to_path_buf)))
}

/// Write `result` (JSON), the HTML page and a copy of the request into `out`; the CLI signs the
/// directory afterwards.
pub fn write_pack<T: Serialize>(
    out: &Path,
    result_file: &str,
    result: &T,
    html: &str,
    request_src: &str,
) -> Result<(), String> {
    write_json(out, result_file, result)?;
    write_text(out, "report.html", html.as_bytes())?;
    write_text(out, "request.toml", request_src.as_bytes())
}

/// Pearson correlation; `None` below three pairs or with no spread.
pub fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    let n = x.len().min(y.len());
    if n < 3 {
        return None;
    }
    let (mx, my) = (
        x[..n].iter().sum::<f64>() / n as f64,
        y[..n].iter().sum::<f64>() / n as f64,
    );
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (a, b) = (x[i] - mx, y[i] - my);
        sxy += a * b;
        sxx += a * a;
        syy += b * b;
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
}

/// Linear-interpolated quantile `q` in [0, 1] of `v` (sorted inside); `None` when empty.
pub fn quantile(v: &[f64], q: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let h = q.clamp(0.0, 1.0) * (s.len() - 1) as f64;
    let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
    Some(s[lo] + (s[hi] - s[lo]) * (h - lo as f64))
}

/// SplitMix64: the small, seeded generator the bootstrap uses, so a result is reproducible.
pub struct SplitMix(pub u64);

impl SplitMix {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform index below `n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Great-circle distance, km.
pub fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    kshana::interference_map::grid::haversine_m(lat1, lon1, lat2, lon2) / 1000.0
}

/// An HTML table from a header and rows (cells escaped).
pub fn html_table(head: &[&str], rows: &[Vec<String>]) -> String {
    let mut s = String::from("<table><tr>");
    for h in head {
        s.push_str(&format!("<th>{}</th>", esc(h)));
    }
    s.push_str("</tr>");
    for r in rows {
        s.push_str("<tr>");
        for c in r {
            s.push_str(&format!("<td>{}</td>", esc(c)));
        }
        s.push_str("</tr>");
    }
    s.push_str("</table>");
    s
}

/// The common page frame: title, label, statement, then `body`.
pub fn page(title: &str, label: &str, body: &str) -> String {
    format!(
        "{}<h1>{}</h1><p class=\"l\">{}</p><p class=\"l\">{}</p>{body}</body></html>",
        html_head(title),
        esc(title),
        esc(label),
        esc(STATEMENT)
    )
}

/// A number for a table cell.
pub fn num(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    if r == r.trunc() && r.abs() < 1e15 {
        format!("{}", r as i64)
    } else {
        format!("{r:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pearson_and_quantile_work_by_hand() {
        assert!((pearson(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap() - 1.0).abs() < 1e-12);
        assert!((pearson(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]).unwrap() + 1.0).abs() < 1e-12);
        assert_eq!(pearson(&[1.0, 2.0], &[1.0, 2.0]), None);
        assert_eq!(pearson(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]), None);
        assert_eq!(quantile(&[3.0, 1.0, 2.0], 0.5), Some(2.0));
        assert_eq!(quantile(&[0.0, 10.0], 0.25), Some(2.5));
        assert_eq!(quantile(&[], 0.5), None);
    }

    #[test]
    fn splitmix_is_deterministic() {
        let (mut a, mut b) = (SplitMix(7), SplitMix(7));
        for _ in 0..5 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert!(SplitMix(1).below(10) < 10);
    }
}
