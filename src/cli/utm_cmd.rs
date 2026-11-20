// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro utm <command>`: UTM / U-space positioning-integrity services (`crate::utm`,
//! research backlog E18 B1804). Every result is a signed directory and decision support only.
//!
//! Licence: `sora`, `rid-check` and `attest` need `pro-utm-integrity` (Pro), `feed`
//! `pro-ussp-feed` (Enterprise). `check` and `synth` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::aviation::write_result;
use crate::utm;

use super::evidence_cmd::{producing, sign};

pub const USAGE: &str = "usage: kshana-pro utm <command>\n\
    sora <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        SORA-ready GNSS-interference input for one operation: route exposure per day map, days affected, dead-reckoning error over the worst degraded stretch, containment margin; fills sora-input.md; signed (Pro)\n\
    feed <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        USSP integrity feed: per map cell in an area, latest state, degraded days, advisory class and validity, as JSON Lines; signed (Enterprise)\n\
    rid-check <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Remote ID position authenticity from the broadcasts alone: speed consistency, implied speed and climb, time order, clones, operator distance; signed (Pro)\n\
    attest <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Per-flight positioning-integrity attestation from telemetry, the day map and Remote ID; signed (Pro)\n\
    check <dir> --trust <signer public key hex>[,...]\n\
        Check a signed result directory, offline (free)\n\
    synth <dir>\n\
        Write synthetic inputs and request files for every command (free)\n\
  Decision support only: never a flight or operational authorisation or a SORA approval. MODELLED.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn produce(args: &[String], gate: &str) -> Result<String, String> {
    let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return Err(USAGE.into());
    };
    let (out, seed, signer, utc) = producing(args, gate)?;
    let l = utm::load(Path::new(file))?;
    let dir = Path::new(&out);
    let headline = match args.first().map(String::as_str) {
        Some("sora") => {
            let r = utm::sora(&l.req, &l.maps)?;
            let h = format!(
                "{}: interference class {}, containment margin {} m",
                r.operation.id, r.interference_class, r.containment_margin_m
            );
            write_result(
                dir,
                "sora.json",
                &r,
                &utm::summary("sora", &h),
                &[("sora-input.md", utm::sora_markdown(&r))],
                &l.inputs,
            )?;
            h
        }
        Some("feed") => {
            let r = utm::feed(&l.req, &l.maps)?;
            let h = format!(
                "{} cells over {} days; {} persistent",
                r.records.len(),
                r.days.len(),
                r.records
                    .iter()
                    .filter(|x| x.advisory == "persistent")
                    .count()
            );
            write_result(
                dir,
                "feed.json",
                &r,
                &utm::summary("feed", &h),
                &[("feed.jsonl", utm::feed_jsonl(&r))],
                &l.inputs,
            )?;
            h
        }
        Some("rid-check") => {
            let text = l
                .rid
                .as_ref()
                .ok_or("rid-check needs rid = \"<file.csv>\" in the request")?;
            let (reports, bad) = utm::parse_rid(text)?;
            let r = utm::rid_check(&reports, bad, &l.req.rid_params);
            let h = r
                .identities
                .iter()
                .map(|i| format!("{} {}", i.uas_id, i.verdict))
                .collect::<Vec<_>>()
                .join(", ");
            write_result(
                dir,
                "rid-check.json",
                &r,
                &utm::summary("rid-check", &h),
                &[],
                &l.inputs,
            )?;
            h
        }
        _ => {
            let r = utm::attest(&l)?;
            let h = format!("{}: {}", r.operation, r.statement);
            write_result(
                dir,
                "attestation.json",
                &r,
                &utm::summary("attest", &h),
                &[],
                &l.inputs,
            )?;
            h
        }
    };
    sign(&out, &seed, &signer, &utc)?;
    Ok(format!("[utm] {headline}; signed in {out}"))
}

/// `kshana-pro utm ...`.
pub fn run(args: &[String]) -> ExitCode {
    let gate = match args.first().map(String::as_str) {
        Some("sora" | "rid-check" | "attest") => utm::FEATURE,
        Some("feed") => utm::FEATURE_FEED,
        Some("check") => return super::scorecard_cmd::verify_dir(args),
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            return match utm::synth::write_all(Path::new(dir)) {
                Ok(()) => {
                    println!("wrote synthetic maps, Remote ID, telemetry and requests under {dir}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            };
        }
        _ => return fail(USAGE),
    };
    match produce(args, gate) {
        Ok(msg) => {
            eprintln!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
