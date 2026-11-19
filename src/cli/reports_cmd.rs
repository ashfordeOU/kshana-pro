// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro reports <command>`: interference reports from crews, controllers and
//! mariners to verified, signed evidence (`crate::ireports`). Also reachable as
//! `kshana-pro forensics reports <command>`.
//!
//! Licence: `normalise`, `validate` and `episodes` need `pro-interference-reports` (Pro);
//! `verify` and `reset-check` need `pro-report-verification` (Pro); `ledger` needs
//! `pro-exposure-ledger` (Enterprise). `check`, `params`, `regimes` and `synth` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::ireports::{self, episodes, ledger, normalise, validate, verify, SourceFormat};
use crate::license;

use super::evidence_cmd::{producing, sign};
use super::forensics_cmd::keys;
use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro reports <command>\n\
    normalise <file>[,<file>...] --out <reports.json> [--format auto|dgca|eccairs|asrs|notam|shiplog|csv] [--regime <id>[,<id>...]] [--license <token>] [--dev]\n\
        Read pilot, crew, ATC and mariner reports (DGCA-style and EASA ECR-like forms, ASRS-like records, NOTAM text, ship logs, CSV) into one versioned schema (Pro)\n\
    validate <reports.json | report file> [--regime dgca|easa|asrs|maritime|generic[,...]] [--json <out>] [--strict] [--license <token>] [--dev]\n\
        Flag missing mandatory fields per regime (DGCA 10-minute rule, EU 376/2014 72-hour rule, ...); --strict exits 3 when any report is incomplete (Pro)\n\
    episodes <reports.json> [--adsb <csv>[,<csv>...]] [--params <params.toml>] --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Cluster reports and ADS-B NIC/NACp drops into episodes with explicit start and end rules; signed (Pro)\n\
    verify <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Cross-check reports against receiver logs, ADS-B and AIS: CORROBORATED / UNCORROBORATED / CONTRADICTED with reasons; signed (Pro)\n\
    reset-check <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Post-spoofing check: does the receiver still disagree with an independent reference? RESET RECOMMENDED is a recommendation only (MODELLED); signed (Pro)\n\
    ledger <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Per-aircraft or per-vessel exposure ledger from signed episodes, for maintenance planning; signed (Enterprise)\n\
    check <dir> --trust <signer public key hex>[,...]\n\
        Check a signed reports directory, offline (free)\n\
    params\n\
        Print the default episode parameters as TOML (free)\n\
    regimes\n\
        List the validation regimes (free)\n\
    synth <dir>\n\
        Write synthetic observation data and request files matching the sample reports in tests/data/interference-reports (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn list(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn normalise_cmd(args: &[String]) -> ExitCode {
    let Some(files) = file_arg(args) else {
        return fail(USAGE);
    };
    if let Err(e) = gate(args, ireports::FEATURE_REPORTS) {
        return fail(e);
    }
    let Some(out) = flag(args, "--out") else {
        return fail("reports normalise needs --out <reports.json>");
    };
    let fmt = match flag(args, "--format").as_deref() {
        None | Some("auto") => None,
        Some(f) => match SourceFormat::parse(f) {
            Some(f) => Some(f),
            None => return fail(format!("unknown --format '{f}'")),
        },
    };
    let mut reports = vec![];
    for f in list(files) {
        let text = match std::fs::read_to_string(&f) {
            Ok(t) => t,
            Err(e) => return fail(format!("{f}: {e}")),
        };
        match normalise::normalise(&text, &ireports::safe_name(&f), fmt) {
            Ok(r) => {
                eprintln!(
                    "[reports] {f}: {} report(s) read as {:?}",
                    r.len(),
                    r[0].source.format
                );
                reports.extend(r);
            }
            Err(e) => return fail(e),
        }
    }
    let mut doc = ireports::ReportsDoc::new(reports);
    if let Some(regs) = flag(args, "--regime") {
        for r in &doc.reports {
            for g in list(&regs) {
                match validate::validate(r, &g) {
                    Ok(v) => doc.validation.push(v),
                    Err(e) => return fail(e),
                }
            }
        }
    }
    if let Err(e) = std::fs::write(&out, ireports::json_text(&doc)) {
        return fail(format!("{out}: {e}"));
    }
    println!("wrote {out} ({} report(s))", doc.reports.len());
    ExitCode::SUCCESS
}

fn read_reports(file: &str) -> Result<ireports::ReportsDoc, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    if text.trim_start().starts_with('{') {
        ireports::ReportsDoc::from_json(&text).map_err(|e| format!("{file}: {e}"))
    } else {
        normalise::normalise(&text, &ireports::safe_name(file), None).map(ireports::ReportsDoc::new)
    }
}

fn validate_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    if let Err(e) = gate(args, ireports::FEATURE_REPORTS) {
        return fail(e);
    }
    let doc = match read_reports(file) {
        Ok(d) => d,
        Err(e) => return fail(e),
    };
    let regimes = flag(args, "--regime").map(|r| list(&r));
    let mut all = vec![];
    for r in &doc.reports {
        let regs = regimes
            .clone()
            .unwrap_or_else(|| vec![validate::default_regime(r).to_string()]);
        for g in regs {
            match validate::validate(r, &g) {
                Ok(v) => {
                    println!(
                        "{} [{}] {}{}",
                        v.report_id,
                        v.regime,
                        v.status.to_uppercase(),
                        v.report_delay_min
                            .map(|d| format!(" (reported after {d} min)"))
                            .unwrap_or_default()
                    );
                    for i in &v.issues {
                        println!("    {:?}: {} ({})", i.severity, i.rule, i.field);
                    }
                    all.push(v);
                }
                Err(e) => return fail(e),
            }
        }
    }
    if let Some(j) = flag(args, "--json") {
        if let Err(e) = std::fs::write(&j, ireports::json_text(&all)) {
            return fail(format!("{j}: {e}"));
        }
    }
    let incomplete = all.iter().filter(|v| !v.complete()).count();
    println!("{} of {} validation(s) incomplete", incomplete, all.len());
    if incomplete > 0 && has_flag(args, "--strict") {
        return ExitCode::from(3);
    }
    ExitCode::SUCCESS
}

fn episodes_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, ireports::FEATURE_REPORTS).and_then(|(out, seed, signer, utc)| {
        let rb = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let doc = ireports::ReportsDoc::from_json(&String::from_utf8_lossy(&rb))
            .map_err(|e| format!("{file}: {e}"))?;
        let params = match flag(args, "--params") {
            Some(p) => {
                let t = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
                toml::from_str::<episodes::Params>(&t).map_err(|e| format!("{p}: {e}"))?
            }
            None => episodes::Params::default(),
        };
        let mut inputs = vec![ireports::InputFile {
            role: "reports".into(),
            file: ireports::safe_name(file),
            sha256: ireports::sha256_hex(&rb),
        }];
        let mut copies = vec![(file.clone(), rb)];
        let mut adsb = vec![];
        for a in flag(args, "--adsb").map(|v| list(&v)).unwrap_or_default() {
            let b = std::fs::read(&a).map_err(|e| format!("{a}: {e}"))?;
            inputs.push(ireports::InputFile {
                role: "adsb".into(),
                file: ireports::safe_name(&a),
                sha256: ireports::sha256_hex(&b),
            });
            adsb.push((a.clone(), String::from_utf8_lossy(&b).to_string()));
            copies.push((a, b));
        }
        let d = episodes::build(&doc.reports, &adsb, &params, inputs)?;
        episodes::write_dir(Path::new(&out), &d, &copies)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((d, out))
    });
    match r {
        Ok((d, out)) => {
            for e in &d.episodes {
                println!(
                    "{} {} to {} ({} min, {}, {} report(s), {} ADS-B aircraft)",
                    e.id,
                    e.start_utc,
                    e.end_utc,
                    e.duration_min,
                    e.kind,
                    e.reports.len(),
                    e.adsb_aircraft.len()
                );
            }
            eprintln!(
                "[reports] {} episode(s), {} member(s) below threshold, {} report(s) not clustered; signed in {out}",
                d.episodes.len(),
                d.below_threshold.len(),
                d.not_clustered.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn verify_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, ireports::FEATURE_VERIFICATION).and_then(|(out, seed, signer, utc)| {
        let (d, copies) = verify::run(Path::new(file))?;
        verify::write_dir(Path::new(&out), &d, &copies)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((d, out))
    });
    match r {
        Ok((d, out)) => {
            for rec in &d.records {
                println!(
                    "{} {}{}{}",
                    rec.report_id,
                    rec.label.as_str(),
                    match rec.kind_agreement {
                        Some(false) => " (kind differs)",
                        _ => "",
                    },
                    rec.observed_layer
                        .as_deref()
                        .map(|l| format!(" (layer: {l})"))
                        .unwrap_or_default()
                );
            }
            eprintln!("[reports] {} record(s); signed in {out}", d.records.len());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn reset_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, ireports::FEATURE_VERIFICATION).and_then(|(out, seed, signer, utc)| {
        let (c, copies) = ledger::run_reset(Path::new(file))?;
        ledger::write_reset(Path::new(&out), &c, &copies)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((c, out))
    });
    match r {
        Ok((c, out)) => {
            println!(
                "{}: {} (MODELLED, a recommendation only)",
                c.subject, c.outcome
            );
            for r in &c.reasons {
                println!("    {r}");
            }
            eprintln!("[reports] signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn ledger_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let trust = match keys(args, "--trust") {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    let r = producing(args, ireports::FEATURE_LEDGER).and_then(|(out, seed, signer, utc)| {
        let (l, copies) = ledger::run_ledger(Path::new(file), &trust)?;
        ledger::write_ledger(Path::new(&out), &l, &copies)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((l, out))
    });
    match r {
        Ok((l, out)) => {
            for e in &l.entries {
                println!(
                    "{} ({}): {} min exposure, {} spoofing event(s), {} since reset, flag {}",
                    e.subject_id,
                    e.platform.as_str(),
                    e.exposure_minutes,
                    e.spoofing_events,
                    e.spoofing_events_since_reset,
                    e.flag
                );
            }
            for r in &l.refused {
                println!("refused: {r}");
            }
            eprintln!("[reports] {} subject(s); signed in {out}", l.entries.len());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `args`: the arguments after `reports`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("normalise") | Some("normalize") => normalise_cmd(args),
        Some("validate") => validate_cmd(args),
        Some("episodes") => episodes_cmd(args),
        Some("verify") => verify_cmd(args),
        Some("reset-check") => reset_cmd(args),
        Some("ledger") => ledger_cmd(args),
        Some("check") => super::scorecard_cmd::verify_dir(args),
        Some("params") => {
            print!("{}", episodes::Params::default_toml());
            ExitCode::SUCCESS
        }
        Some("regimes") => {
            for r in validate::REGIMES {
                println!("{r}");
            }
            ExitCode::SUCCESS
        }
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            match ireports::synth::write_all(Path::new(dir)) {
                Ok(files) => {
                    for f in files {
                        println!("wrote {f}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(USAGE),
    }
}
