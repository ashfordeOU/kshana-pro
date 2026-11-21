// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro cert <make|status|schedule>`: product certificates, their status against a
//! signed revocation list, and the yearly revalidation calendar (docs/CERT-STATUS.md).

use std::path::Path;
use std::process::ExitCode;

use crate::cert_status as cs;

use super::forensics_cmd::keys;
use super::grid_cmd::{gate, signing};
use super::{flag, has_flag, utc_iso8601_now};

pub const CERT_USAGE: &str = "usage: kshana-pro cert <command>\n\
    make --id <id> --product <name> --method <id> --method-version <v> --issued <date> [--due <date>] [--scope <text>] --out <dir> --signer <name> --key-file <file> [--license <token>] [--dev]\n\
        Write and sign a product certificate pack; revalidation due twelve months after issue unless --due (Pro)\n\
    status <certificate-dir> [--list <revocations file> --trust <issuer key hex>[,...]] [--cert-trust <key hex>[,...]] [--at <utc>] [--json]\n\
        valid, revoked, revalidation-overdue, unknown (no list, or list stale) or invalid; exits 1 unless valid (no licence needed)\n\
    schedule <certificate-dir>[,...] --out <dir> --signer <name> --key-file <file> [--list <file> --trust <hex>] [--cert-trust <hex>] [--at <utc>] [--warn-days <n>] [--license <token>] [--dev]\n\
        Yearly revalidation calendar (CSV, iCalendar, page), signed (Pro)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn need(args: &[String], name: &str) -> Result<String, String> {
    flag(args, name).ok_or_else(|| format!("{name} is needed"))
}

fn list_text(args: &[String]) -> Result<Option<String>, String> {
    flag(args, "--list")
        .map(|p| std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}")))
        .transpose()
}

/// `kshana-pro cert ...`.
pub fn run(args: &[String]) -> ExitCode {
    let r: Result<(String, bool), String> = match args.first().map(String::as_str) {
        Some("make") => (|| {
            gate(args, cs::FEATURE)?;
            let s = signing(args, "cert make")?;
            let c = cs::certificate(
                &need(args, "--id")?,
                &need(args, "--product")?,
                &need(args, "--method")?,
                &need(args, "--method-version")?,
                &need(args, "--issued")?,
                flag(args, "--due").as_deref(),
                &s.signer,
                &flag(args, "--scope").unwrap_or_default(),
            )?;
            cs::write_certificate(Path::new(&s.out), &c)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok((
                format!(
                    "[cert] certificate {} for {} signed in {}; revalidation due {}",
                    c.id, c.subject_product, s.out, c.revalidation_due_utc
                ),
                true,
            ))
        })(),
        Some("status") => (|| {
            let dir = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(CERT_USAGE)?;
            let at = flag(args, "--at").unwrap_or_else(utc_iso8601_now);
            let st = cs::status(
                Path::new(dir),
                list_text(args)?.as_deref(),
                &keys(args, "--cert-trust")?,
                &keys(args, "--trust")?,
                &at,
            )?;
            let ok = st.state == "valid";
            if has_flag(args, "--json") {
                println!("{}", serde_json::to_string_pretty(&st).expect("serialises"));
            }
            Ok((format!("[cert] {}: {}", st.state, st.reason), ok))
        })(),
        Some("schedule") => (|| {
            gate(args, cs::FEATURE)?;
            let s = signing(args, "cert schedule")?;
            let arg = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(CERT_USAGE)?;
            let dirs: Vec<&Path> = arg
                .split(',')
                .filter(|d| !d.trim().is_empty())
                .map(Path::new)
                .collect();
            let warn: f64 = flag(args, "--warn-days")
                .map(|v| v.parse().map_err(|_| "--warn-days: not a number"))
                .transpose()?
                .unwrap_or(60.0);
            let at = flag(args, "--at").unwrap_or_else(|| s.utc.clone());
            let c = cs::schedule(
                &dirs,
                list_text(args)?.as_deref(),
                &keys(args, "--cert-trust")?,
                &keys(args, "--trust")?,
                &at,
                warn,
            )?;
            let out = Path::new(&s.out);
            crate::transport::write_pack(out, &c, &cs::calendar_html(&c), "certificates.txt", arg)?;
            std::fs::write(out.join("calendar.csv"), cs::calendar_csv(&c))
                .map_err(|e| e.to_string())?;
            std::fs::write(out.join("calendar.ics"), cs::calendar_ics(&c))
                .map_err(|e| e.to_string())?;
            crate::signed_dossier::sign(out, &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok((
                format!("[cert] {}\n[cert] signed calendar in {}", c.verdict, s.out),
                true,
            ))
        })(),
        _ => return fail(CERT_USAGE),
    };
    match r {
        Ok((m, ok)) => {
            eprintln!("{m}");
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => fail(e),
    }
}
