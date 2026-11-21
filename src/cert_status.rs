// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Product certificates: status against a revocation list, and yearly revalidation
//!
//! The Pro part of the validated-product register (research 2026-11-20 gaps, B2505). The public
//! register belongs to the open side and the revocation list is issued by the hosted service;
//! neither is built here. This module:
//!
//! * writes a **certificate** as a signed pack: `certificate.json`
//!   (`kshana-pro-cert-product/1`) with an ID, the subject product, the method and its version,
//!   the issue date and the revalidation-due date (twelve months after issue unless stated);
//! * reports a certificate's **status** ([`status`]): its pack must verify; then it is checked
//!   against a **signed revocation list** supplied as a file (`kshana-pro-cert-revocations/1`,
//!   `payload_hex.signature_hex` like the licence revocation list, Ed25519 by a key the caller
//!   trusts). The status is `valid`, `revoked` (with reason and date), `revalidation-overdue`,
//!   `unknown` (no list, a list that does not verify, or a list past its `next_update`) or
//!   `invalid` (the certificate pack does not verify). A stale list never reports `valid`;
//!   a revocation it carries still counts;
//! * builds the **yearly revalidation calendar** ([`schedule`]) of a set of certificates: due
//!   dates, overdue and due-soon flags, as CSV and iCalendar.
//!
//! No list issuer is built here: lists are only read and verified. **MODELLED**: a status of
//! the records supplied; whether a product is fit for use is for the certificate's issuer to
//! say.

use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};
use crate::license::{key_id, KEY_LEN};

/// Pro feature.
pub const FEATURE: &str = "pro-cert-revalidation";
/// Formats.
pub const CERT_FORMAT: &str = "kshana-pro-cert-product/1";
pub const LIST_FORMAT: &str = "kshana-pro-cert-revocations/1";
pub const STATUS_FORMAT: &str = "kshana-pro-cert-status/1";
pub const CALENDAR_FORMAT: &str = "kshana-pro-cert-calendar/1";
/// The certificate file inside its pack.
pub const CERT_FILE: &str = "certificate.json";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED";

/// A product certificate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Certificate {
    pub format: String,
    pub id: String,
    pub subject_product: String,
    pub method_id: String,
    pub method_version: String,
    pub issued_utc: String,
    pub revalidation_due_utc: String,
    pub issuer: String,
    #[serde(default)]
    pub scope: String,
}

/// One revoked certificate on a list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revoked {
    pub id: String,
    pub reason: String,
    pub revoked_utc: String,
}

/// A revocation list (the signed payload).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RevocationList {
    pub format: String,
    pub issuer: String,
    /// Grows with every list published.
    pub sequence: u64,
    pub issued_utc: String,
    /// After this time the list is stale and a certificate absent from it is `unknown`.
    pub next_update_utc: String,
    #[serde(default)]
    pub revoked: Vec<Revoked>,
    /// ID of the key that signed the list ([`key_id`]).
    pub kid: String,
}

fn add_months(utc: &str, n: i64) -> String {
    let y: i64 = utc[..4].parse().unwrap_or(1970);
    let m: i64 = utc[5..7].parse().unwrap_or(1);
    let t = y * 12 + (m - 1) + n;
    let (ny, nm) = (t / 12, t % 12 + 1);
    let d: u32 = utc[8..10].parse().unwrap_or(1);
    let dim = match nm {
        2 if ny % 4 == 0 && (ny % 100 != 0 || ny % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    format!("{ny:04}-{nm:02}-{:02}{}", d.min(dim), &utc[10..])
}

fn utc_of(s: &str, what: &str) -> Result<f64, String> {
    let s = s.trim();
    let full = if s.len() == 10 {
        format!("{s}T00:00:00Z")
    } else {
        s.to_string()
    };
    parse_utc(&full).ok_or_else(|| format!("{what}: bad UTC time '{s}'"))
}

/// A certificate, checked. `due` defaults to twelve months after `issued`.
// One argument per certificate field, all plain strings, as the command line gives them.
#[allow(clippy::too_many_arguments)]
pub fn certificate(
    id: &str,
    product: &str,
    method_id: &str,
    method_version: &str,
    issued: &str,
    due: Option<&str>,
    issuer: &str,
    scope: &str,
) -> Result<Certificate, String> {
    for (v, n) in [
        (id, "id"),
        (product, "subject product"),
        (method_id, "method"),
        (method_version, "method version"),
        (issuer, "issuer"),
    ] {
        if v.trim().is_empty() {
            return Err(format!("certificate: {n} is empty"));
        }
    }
    let issued_t = utc_of(issued, "issued")?;
    let issued_utc = iso(issued_t);
    let due_utc = match due {
        Some(d) => iso(utc_of(d, "revalidation due")?),
        None => add_months(&issued_utc, 12),
    };
    if utc_of(&due_utc, "revalidation due")? <= issued_t {
        return Err("certificate: revalidation due must be after issue".into());
    }
    Ok(Certificate {
        format: CERT_FORMAT.into(),
        id: id.into(),
        subject_product: product.into(),
        method_id: method_id.into(),
        method_version: method_version.into(),
        issued_utc,
        revalidation_due_utc: due_utc,
        issuer: issuer.into(),
        scope: scope.into(),
    })
}

/// Write a certificate pack (sign it afterwards).
pub fn write_certificate(out: &Path, c: &Certificate) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(
        out.join(CERT_FILE),
        serde_json::to_string_pretty(c).expect("serialises"),
    )
    .map_err(|e| format!("{}: {e}", out.display()))
}

/// Read and check a signed revocation list against the trusted issuer keys.
pub fn verify_list(text: &str, trust: &[[u8; KEY_LEN]]) -> Result<RevocationList, String> {
    let (payload, sig) =
        crate::license::split_signed(text).map_err(|e| format!("revocation list: {e}"))?;
    let list: RevocationList = serde_json::from_slice(&payload)
        .map_err(|e| format!("revocation list: not a certificate revocation list: {e}"))?;
    if list.format != LIST_FORMAT {
        return Err(format!(
            "revocation list: format '{}', expected {LIST_FORMAT}",
            list.format
        ));
    }
    if trust.is_empty() {
        return Err("revocation list: no trusted issuer key (--trust)".into());
    }
    let sig = Signature::from_bytes(&sig);
    let ok = trust.iter().any(|pk| {
        key_id(pk).eq_ignore_ascii_case(&list.kid)
            && VerifyingKey::from_bytes(pk)
                .map(|k| k.verify_strict(&payload, &sig).is_ok())
                .unwrap_or(false)
    });
    if !ok {
        return Err(format!(
            "revocation list: signature by key {} does not verify under a trusted key",
            list.kid
        ));
    }
    utc_of(&list.issued_utc, "revocation list issued")?;
    utc_of(&list.next_update_utc, "revocation list next update")?;
    Ok(list)
}

/// What the status says about the list used.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListInfo {
    pub issuer: String,
    pub sequence: u64,
    pub issued_utc: String,
    pub next_update_utc: String,
    pub stale: bool,
}

/// The status of one certificate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub format: String,
    pub evidence: String,
    pub checked_utc: String,
    pub certificate: Option<Certificate>,
    /// `valid`, `revoked`, `revalidation-overdue`, `unknown` or `invalid`.
    pub state: String,
    pub reason: String,
    pub certificate_signature: String,
    pub list: Option<ListInfo>,
    pub revocation: Option<Revoked>,
}

/// The status of the certificate pack `dir` at `now_utc`. The pack is verified against
/// `cert_trust` (any key when empty); the list, when given, against `list_trust`.
pub fn status(
    dir: &Path,
    list_text: Option<&str>,
    cert_trust: &[[u8; KEY_LEN]],
    list_trust: &[[u8; KEY_LEN]],
    now_utc: &str,
) -> Result<Status, String> {
    let now = utc_of(now_utc, "--at")?;
    let mut st = Status {
        format: STATUS_FORMAT.into(),
        evidence: EVIDENCE.into(),
        checked_utc: iso(now),
        certificate: None,
        state: "invalid".into(),
        reason: String::new(),
        certificate_signature: String::new(),
        list: None,
        revocation: None,
    };
    match crate::signed_dossier::verify(dir, cert_trust) {
        Ok(v) if v.problems.is_empty() => {
            st.certificate_signature = format!(
                "verified (signer {}, key {}{})",
                v.signer,
                v.key_id,
                if cert_trust.is_empty() {
                    ", no trusted key given"
                } else {
                    ", trusted"
                }
            );
        }
        Ok(v) => {
            st.certificate_signature = format!("does not verify: {}", v.problems.join("; "));
            st.reason = "the certificate pack does not verify".into();
            return Ok(st);
        }
        Err(e) => {
            st.certificate_signature = format!("does not verify: {e}");
            st.reason = "the certificate pack does not verify".into();
            return Ok(st);
        }
    }
    let p = dir.join(CERT_FILE);
    let c: Certificate =
        serde_json::from_slice(&std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?)
            .map_err(|e| format!("{CERT_FILE}: {e}"))?;
    if c.format != CERT_FORMAT {
        return Err(format!(
            "{CERT_FILE}: format '{}', expected {CERT_FORMAT}",
            c.format
        ));
    }
    let due = utc_of(&c.revalidation_due_utc, "revalidation due")?;
    let overdue = now > due;
    let list = match list_text {
        None => Err("no revocation list supplied".to_string()),
        Some(t) => verify_list(t, list_trust),
    };
    let (state, reason) = match &list {
        Ok(l) => {
            let next = utc_of(&l.next_update_utc, "next update")?;
            let stale = now > next;
            st.list = Some(ListInfo {
                issuer: l.issuer.clone(),
                sequence: l.sequence,
                issued_utc: l.issued_utc.clone(),
                next_update_utc: l.next_update_utc.clone(),
                stale,
            });
            let rev = l.revoked.iter().find(|r| {
                r.id == c.id
                    && utc_of(&r.revoked_utc, "revoked")
                        .map(|t| t <= now)
                        .unwrap_or(true)
            });
            if let Some(r) = rev {
                st.revocation = Some(r.clone());
                (
                    "revoked",
                    format!("revoked on {}: {}", r.revoked_utc, r.reason),
                )
            } else if overdue {
                (
                    "revalidation-overdue",
                    format!("revalidation was due on {}", c.revalidation_due_utc),
                )
            } else if stale {
                (
                    "unknown",
                    format!(
                        "the revocation list (sequence {}) is past its next update ({})",
                        l.sequence, l.next_update_utc
                    ),
                )
            } else {
                (
                    "valid",
                    format!(
                        "not on revocation list sequence {}; revalidation due {}",
                        l.sequence, c.revalidation_due_utc
                    ),
                )
            }
        }
        Err(e) if overdue => (
            "revalidation-overdue",
            format!(
                "revalidation was due on {}; revocation not checked: {e}",
                c.revalidation_due_utc
            ),
        ),
        Err(e) => ("unknown", e.clone()),
    };
    st.state = state.into();
    st.reason = reason;
    st.certificate = Some(c);
    Ok(st)
}

/// One calendar row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Due {
    pub id: String,
    pub subject_product: String,
    pub method: String,
    pub issued_utc: String,
    pub revalidation_due_utc: String,
    pub days_left: f64,
    /// `OVERDUE`, `DUE SOON`, `SCHEDULED` or `REVOKED` (no revalidation).
    pub flag: String,
    pub status: String,
}

/// The revalidation calendar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub format: String,
    pub evidence: String,
    pub checked_utc: String,
    pub warn_days: f64,
    pub items: Vec<Due>,
    pub verdict: String,
}

/// The calendar of the certificate packs `dirs` at `now_utc`: every certificate's status, its
/// next revalidation, and the flag (`DUE SOON` within `warn_days`).
pub fn schedule(
    dirs: &[&Path],
    list_text: Option<&str>,
    cert_trust: &[[u8; KEY_LEN]],
    list_trust: &[[u8; KEY_LEN]],
    now_utc: &str,
    warn_days: f64,
) -> Result<Calendar, String> {
    if warn_days.is_nan() || warn_days < 0.0 {
        return Err("warn days must be zero or positive".into());
    }
    let now = utc_of(now_utc, "--at")?;
    let mut items = Vec::new();
    for d in dirs {
        let s = status(d, list_text, cert_trust, list_trust, now_utc)?;
        let Some(c) = &s.certificate else {
            return Err(format!(
                "{}: {} ({})",
                d.display(),
                s.reason,
                s.certificate_signature
            ));
        };
        let due = utc_of(&c.revalidation_due_utc, "revalidation due")?;
        let days_left = (due - now) / 86_400.0;
        let flag = if s.state == "revoked" {
            "REVOKED"
        } else if days_left < 0.0 {
            "OVERDUE"
        } else if days_left <= warn_days {
            "DUE SOON"
        } else {
            "SCHEDULED"
        };
        items.push(Due {
            id: c.id.clone(),
            subject_product: c.subject_product.clone(),
            method: format!("{} v{}", c.method_id, c.method_version),
            issued_utc: c.issued_utc.clone(),
            revalidation_due_utc: c.revalidation_due_utc.clone(),
            days_left,
            flag: flag.into(),
            status: s.state.clone(),
        });
    }
    items.sort_by(|a, b| {
        a.days_left
            .partial_cmp(&b.days_left)
            .expect("finite")
            .then(a.id.cmp(&b.id))
    });
    for w in items.windows(2) {
        if w[0].id == w[1].id {
            return Err(format!("certificate '{}' given twice", w[0].id));
        }
    }
    let n = |f: &str| items.iter().filter(|i| i.flag == f).count();
    let verdict = format!(
        "{} certificates: {} overdue, {} due within {warn_days} days, {} scheduled, {} revoked",
        items.len(),
        n("OVERDUE"),
        n("DUE SOON"),
        n("SCHEDULED"),
        n("REVOKED")
    );
    Ok(Calendar {
        format: CALENDAR_FORMAT.into(),
        evidence: EVIDENCE.into(),
        checked_utc: iso(now),
        warn_days,
        items,
        verdict,
    })
}

/// The calendar as CSV.
pub fn calendar_csv(c: &Calendar) -> String {
    let q = |s: &str| {
        if s.contains(',') || s.contains('"') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut s = String::from(
        "id,subject_product,method,issued_utc,revalidation_due_utc,days_left,flag,status\n",
    );
    for i in &c.items {
        s.push_str(&format!(
            "{},{},{},{},{},{:.1},{},{}\n",
            q(&i.id),
            q(&i.subject_product),
            q(&i.method),
            i.issued_utc,
            i.revalidation_due_utc,
            i.days_left,
            i.flag,
            i.status
        ));
    }
    s
}

/// The calendar as iCalendar (one all-day event per revalidation not revoked).
pub fn calendar_ics(c: &Calendar) -> String {
    let esc = |s: &str| {
        s.replace('\\', "\\\\")
            .replace(';', "\\;")
            .replace(',', "\\,")
    };
    let stamp: String = c
        .checked_utc
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    let mut s = String::from(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Kshana Pro//certificate revalidation//EN\r\n",
    );
    for i in c.items.iter().filter(|i| i.flag != "REVOKED") {
        let day: String = i.revalidation_due_utc[..10].replace('-', "");
        s.push_str(&format!(
            "BEGIN:VEVENT\r\nUID:{}-revalidation@kshana-pro\r\nDTSTAMP:{stamp}\r\nDTSTART;VALUE=DATE:{day}\r\nSUMMARY:{}\r\nDESCRIPTION:{}\r\nEND:VEVENT\r\n",
            esc(&i.id),
            esc(&format!("Revalidate {} ({})", i.subject_product, i.flag)),
            esc(&format!("Certificate {} under {}, issued {}", i.id, i.method, i.issued_utc)),
        ));
    }
    s.push_str("END:VCALENDAR\r\n");
    s
}

/// The calendar page.
pub fn calendar_html(c: &Calendar) -> String {
    let rows = c
        .items
        .iter()
        .map(|i| {
            vec![
                i.id.clone(),
                i.subject_product.clone(),
                i.method.clone(),
                i.revalidation_due_utc.clone(),
                format!("{:.0}", i.days_left),
                i.flag.clone(),
                i.status.clone(),
            ]
        })
        .collect();
    crate::transport::page(
        "Certificate revalidation calendar",
        &c.evidence,
        &c.verdict,
        &[format!("Checked at {}.", c.checked_utc)],
        &[(
            "Certificates".into(),
            vec![
                "id",
                "product",
                "method",
                "revalidation due",
                "days left",
                "flag",
                "status",
            ],
            rows,
        )],
    )
}
