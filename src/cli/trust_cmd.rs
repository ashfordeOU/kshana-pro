// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro trust <run|verify>` (roadmap 0.42 F3). `run` needs `pro-multi-source-trust`;
//! `verify` needs no licence.

use std::path::Path;
use std::process::ExitCode;

use crate::multi_source::{self, Config, Evidence};

use super::scorecard_cmd::entitlement;
use super::{flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro trust <command>\n\
    run <config.toml> --input <sources.csv> --out <dir> [--license <token>] [--dev]\n\
        Choose epoch by epoch which time source to trust (GNSS, OSNMA, LEO, eLoran, PTP, holdover);\n\
        write trust-decisions.csv and the hash-chained trust-evidence.json (Pro)\n\
    verify <dir>\n\
        Check the evidence against the decisions table (no licence needed)";

/// The decisions table's file name.
pub const DECISIONS_FILE: &str = "trust-decisions.csv";
/// The evidence document's file name.
pub const EVIDENCE_FILE: &str = "trust-evidence.json";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("run") => run_cmd(args),
        Some("verify") => verify_cmd(args),
        _ => fail(USAGE),
    }
}

fn run_cmd(args: &[String]) -> ExitCode {
    let Some(cfg_path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let (Some(input), Some(out)) = (flag(args, "--input"), flag(args, "--out")) else {
        return fail(USAGE);
    };
    let now = now_unix();
    let ent = match entitlement(args, now) {
        Ok(e) => e,
        Err(e) => return fail(e),
    };
    if let Err(e) = crate::license::check(&ent, multi_source::KIND, now) {
        return fail(e);
    }
    let cfg: Config = match std::fs::read_to_string(cfg_path)
        .map_err(|e| format!("{cfg_path}: {e}"))
        .and_then(|t| toml::from_str(&t).map_err(|e| format!("{cfg_path}: {e}")))
    {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    if let Err(e) = cfg.validate() {
        return fail(format!("{cfg_path}: {e}"));
    }
    let text = match std::fs::read_to_string(&input) {
        Ok(t) => t,
        Err(e) => return fail(format!("{input}: {e}")),
    };
    let rows = match multi_source::parse_table(&cfg, &text) {
        Ok(r) => r,
        Err(e) => return fail(format!("{input}: {e}")),
    };
    let sha = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(text.as_bytes()))
    };
    let r = match multi_source::run_table(&cfg, &rows, &sha) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let dir = Path::new(&out);
    if let Err(e) = std::fs::create_dir_all(dir) {
        return fail(format!("{out}: {e}"));
    }
    let ev = serde_json::to_string_pretty(&r.evidence).expect("the evidence is plain data") + "\n";
    if let Err(e) = std::fs::write(dir.join(DECISIONS_FILE), &r.decisions_csv)
        .and_then(|_| std::fs::write(dir.join(EVIDENCE_FILE), ev))
    {
        return fail(format!("{out}: {e}"));
    }
    let c = &r.evidence.counts;
    println!(
        "{} epochs: {} trusted, {} holdover, {} none; {} switches; wrote {out}/{DECISIONS_FILE} and {out}/{EVIDENCE_FILE}",
        r.evidence.epochs,
        c.trusted,
        c.holdover,
        c.none,
        r.evidence.switches.len()
    );
    ExitCode::SUCCESS
}

fn verify_cmd(args: &[String]) -> ExitCode {
    let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let dir = Path::new(dir);
    let read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| format!("{f}: {e}"));
    let (ev, csv) = match (read(EVIDENCE_FILE), read(DECISIONS_FILE)) {
        (Ok(e), Ok(c)) => (e, c),
        (Err(e), _) | (_, Err(e)) => return fail(e),
    };
    let ev: Evidence = match serde_json::from_str(&ev) {
        Ok(e) => e,
        Err(e) => return fail(format!("{EVIDENCE_FILE}: {e}")),
    };
    let p = ev.verify(&csv);
    if p.is_empty() {
        println!(
            "verified: {} epochs, {} switch records, chain intact, decisions table matches ({})",
            ev.epochs,
            ev.switches.len(),
            ev.method
        );
        ExitCode::SUCCESS
    } else {
        for x in &p {
            println!("FAILED: {x}");
        }
        ExitCode::FAILURE
    }
}
