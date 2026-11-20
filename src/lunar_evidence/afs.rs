// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! AFS receiver verification: the Pro evidence view on top of the open AFS reference generator
//! (`kshana::lunar_afs`, pinned to LSIS V1.0 of 29 January 2025). The open engine generates the
//! signal; this module scores what a receiver under test reported against it:
//!
//! * **Replica codes** (`codes`, `prn,channel,hex`): the receiver's dumped primary codes. AFS-Q
//!   (`q`) is compared chip by chip with `afs_q_primary(prn)` (LSIS-222, generated from the
//!   IS-GPS-800 construction, no cache needed). AFS-I (`i`) is checked to be a Gold code of the
//!   LSIS-221 pair (`g2_delay_of`); its PRN-to-delay assignment is checked against the LSIS
//!   Annex 3 file only when the verified local cache is present (`kshana lunanet-afs-check`
//!   fetch), otherwise it is reported as not checked.
//! * **Frames** (`frames`, `time_s,node,toi,bits,hex`): decoded subframe data with its 24-bit CRC
//!   appended (the last `bits` bits of `hex`). The CRC-24 of LSIS-FID0-469 is recomputed with the
//!   open `crc24_bits`; the time of interval (TOI, LSIS-720: 100 frames per 20 min, so one per
//!   12 s) must advance by the elapsed frame count modulo 100. The frame error rate carries a
//!   Wilson 95 % interval and is judged against `max_frame_error_rate` when one is given (the
//!   value is the verifier's, not a figure from the standard).
//! * **PRN collisions** (`assignments`, `node,i_prn,q_prn,secondary,tertiary_prn,tertiary_phase`):
//!   every PRN in 1 to 210 and no two nodes sharing an AFS-I PRN, an AFS-Q PRN or a tertiary
//!   code and phase. LSIS V1.0 Table 11 is an interim assignment ({LSIS-TBD-2001}).
//!
//! MODELLED: checked on synthetic dumps with injected chip errors, CRC errors, a TOI slip and a
//! duplicated PRN. It is receiver test evidence; it does not show that a receiver meets LSIS.

use std::collections::BTreeMap;

use kshana::lunar_afs::codes::{
    afs_q_primary, g2_delay_of, LsisCodes, I_PRIMARY_LEN, MAX_PRN, Q_PRIMARY_LEN,
};
use kshana::lunar_afs::frame::crc24_bits;
use kshana::lunar_afs::lsis::hex_bits_last;
use serde::{Deserialize, Serialize};

use super::{csv_rows, num, upper_verdict, wilson95, Loaded};

/// Frame period: 100 frames per 20 min block (LSIS-720).
pub const FRAME_PERIOD_S: f64 = 12.0;

/// `[afs_rx]` section.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AfsReq {
    #[serde(default)]
    pub receiver: String,
    #[serde(default)]
    pub codes: Option<String>,
    #[serde(default)]
    pub frames: Option<String>,
    #[serde(default)]
    pub assignments: Option<String>,
    #[serde(default)]
    pub max_frame_error_rate: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CodeCheck {
    pub prn: u16,
    pub channel: String,
    pub chips: usize,
    pub mismatched_chips: Option<usize>,
    pub first_mismatch: Option<usize>,
    pub g2_delay: Option<usize>,
    pub verdict: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FrameCheck {
    pub node: String,
    pub frames: usize,
    pub crc_failures: usize,
    pub toi_slips: usize,
    pub frame_error_rate: f64,
    pub wilson95: Option<(f64, f64)>,
    pub verdict: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AfsReport {
    pub schema: String,
    pub standard: String,
    pub receiver: String,
    pub codes: Vec<CodeCheck>,
    pub frames: Vec<FrameCheck>,
    pub collisions: Vec<String>,
    pub verdict: String,
    pub evidence: String,
}

/// Bits to hexadecimal, right-aligned (the leading pad bits are zero), as LSIS Annex 3 prints.
pub fn bits_to_hex(bits: &[u8]) -> String {
    let pad = (4 - bits.len() % 4) % 4;
    let mut v: Vec<u8> = vec![0; pad];
    v.extend(bits.iter().map(|b| b & 1));
    v.chunks(4)
        .map(|c| {
            let n = c.iter().fold(0u32, |a, b| (a << 1) | *b as u32);
            char::from_digit(n, 16).unwrap().to_ascii_uppercase()
        })
        .collect()
}

fn check_code(
    prn: u16,
    ch: &str,
    hex: &str,
    lsis: Option<&LsisCodes>,
) -> Result<CodeCheck, String> {
    let mismatch = |got: &[u8], want: &[u8]| {
        let bad: Vec<usize> = (0..want.len()).filter(|&i| got[i] != want[i]).collect();
        (bad.len(), bad.first().copied())
    };
    match ch {
        "q" => {
            let got = hex_bits_last(hex, Q_PRIMARY_LEN);
            let want = afs_q_primary(prn)?;
            if got.len() != Q_PRIMARY_LEN {
                return Err(format!(
                    "PRN {prn} q: {} chips, need {Q_PRIMARY_LEN}",
                    got.len()
                ));
            }
            let (n, first) = mismatch(&got, &want);
            Ok(CodeCheck {
                prn,
                channel: "q".into(),
                chips: got.len(),
                mismatched_chips: Some(n),
                first_mismatch: first,
                g2_delay: None,
                verdict: if n == 0 { "MATCH" } else { "MISMATCH" }.into(),
            })
        }
        "i" => {
            let got = hex_bits_last(hex, I_PRIMARY_LEN);
            if got.len() != I_PRIMARY_LEN {
                return Err(format!(
                    "PRN {prn} i: {} chips, need {I_PRIMARY_LEN}",
                    got.len()
                ));
            }
            let d = g2_delay_of(&got);
            let (mm, first, verdict) = match (d, lsis) {
                (None, _) => (None, None, "NOT A GOLD CODE OF THE AFS-I PAIR".to_string()),
                (Some(_), Some(c)) => {
                    let (n, f) = mismatch(&got, &c.afs_i_primary(prn)?);
                    (
                        Some(n),
                        f,
                        if n == 0 { "MATCH" } else { "WRONG PRN" }.into(),
                    )
                }
                (Some(_), None) => (
                    None,
                    None,
                    "GOLD CODE OK; PRN assignment not checked (LSIS cache absent)".into(),
                ),
            };
            Ok(CodeCheck {
                prn,
                channel: "i".into(),
                chips: got.len(),
                mismatched_chips: mm,
                first_mismatch: first,
                g2_delay: d,
                verdict,
            })
        }
        other => Err(format!("channel '{other}' must be i or q")),
    }
}

/// AFS receiver verification.
pub fn afs_rx(l: &mut Loaded) -> Result<AfsReport, String> {
    let r = l
        .req
        .afs_rx
        .clone()
        .ok_or("the request has no [afs_rx] section")?;
    if r.codes.is_none() && r.frames.is_none() && r.assignments.is_none() {
        return Err("[afs_rx] needs codes, frames or assignments".into());
    }
    let prn_of = |s: &str| -> Result<u16, String> {
        let p: u16 = s.parse().map_err(|_| format!("PRN '{s}'"))?;
        if (1..=MAX_PRN).contains(&p) {
            Ok(p)
        } else {
            Err(format!("PRN {p} outside 1 to {MAX_PRN}"))
        }
    };
    let mut codes = Vec::new();
    if let Some(f) = &r.codes {
        let text = l.read(f, "replica-codes")?;
        let lsis = LsisCodes::load().ok();
        for row in csv_rows(&text, &["prn", "channel", "hex"])? {
            codes.push(check_code(
                prn_of(&row[0])?,
                &row[1].to_ascii_lowercase(),
                &row[2],
                lsis.as_ref(),
            )?);
        }
    }
    let mut frames = Vec::new();
    if let Some(f) = &r.frames {
        let text = l.read(f, "decoded-frames")?;
        let mut by: BTreeMap<String, Vec<(f64, u32, bool)>> = BTreeMap::new();
        for row in csv_rows(&text, &["time_s", "node", "toi", "bits", "hex"])? {
            let n = num(&row[3], "bits")? as usize;
            if n <= 24 {
                return Err("bits must exceed the 24 CRC bits".into());
            }
            let b = hex_bits_last(&row[4], n);
            let ok = b.len() == n && crc24_bits(&b[..n - 24])[..] == b[n - 24..];
            let toi: u32 = row[2].parse().map_err(|_| format!("toi '{}'", row[2]))?;
            by.entry(row[1].clone())
                .or_default()
                .push((num(&row[0], "time_s")?, toi, ok));
        }
        for (node, mut v) in by {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            let crc = v.iter().filter(|x| !x.2).count();
            let slips = v
                .windows(2)
                .filter(|w| {
                    let steps = ((w[1].0 - w[0].0) / FRAME_PERIOD_S).round() as u32;
                    w[0].2 && w[1].2 && (w[0].1 + steps) % 100 != w[1].1
                })
                .count();
            let w = wilson95(crc, v.len());
            let verdict = match (r.max_frame_error_rate, w) {
                (Some(m), Some((lo, hi))) => upper_verdict(lo, hi, m, "MEETS", "EXCEEDS"),
                _ => "REPORTED (no frame error rate requirement given)".into(),
            };
            frames.push(FrameCheck {
                node,
                frames: v.len(),
                crc_failures: crc,
                toi_slips: slips,
                frame_error_rate: crc as f64 / v.len() as f64,
                wilson95: w,
                verdict: if slips > 0 {
                    format!("{verdict}; TOI SLIPS")
                } else {
                    verdict
                },
            });
        }
    }
    let mut collisions = Vec::new();
    if let Some(f) = &r.assignments {
        let text = l.read(f, "prn-assignments")?;
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for row in csv_rows(
            &text,
            &[
                "node",
                "i_prn",
                "q_prn",
                "secondary",
                "tertiary_prn",
                "tertiary_phase",
            ],
        )? {
            let node = &row[0];
            let (i, q, t) = (prn_of(&row[1])?, prn_of(&row[2])?, prn_of(&row[4])?);
            if num(&row[3], "secondary")? > 3.0 {
                collisions.push(format!(
                    "node {node}: secondary code S{} does not exist",
                    row[3]
                ));
            }
            for key in [
                format!("AFS-I PRN {i}"),
                format!("AFS-Q PRN {q}"),
                format!("tertiary PRN {t} phase {}", row[5]),
            ] {
                if let Some(other) = seen.insert(key.clone(), node.clone()) {
                    collisions.push(format!("{key} used by nodes {other} and {node}"));
                }
            }
        }
    }
    let bad = codes
        .iter()
        .any(|c| c.verdict != "MATCH" && !c.verdict.starts_with("GOLD"))
        || frames
            .iter()
            .any(|f| f.verdict.contains("EXCEEDS") || f.toi_slips > 0)
        || !collisions.is_empty();
    Ok(AfsReport {
        schema: "kshana-pro/lunar-afs-receiver/v1".into(),
        standard: kshana::lunar_afs::LSIS_VERSION.into(),
        receiver: r.receiver,
        codes,
        frames,
        collisions,
        verdict: if bad { "FINDINGS" } else { "NO FINDINGS" }.into(),
        evidence: super::EVIDENCE.into(),
    })
}
