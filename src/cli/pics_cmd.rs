// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro pics <list|show|score|export|verify|mapping>` (W1119 C1). `score` needs a
//! licence that unlocks `pro-receiver-scorecard`; the rest need none.

use std::path::Path;
use std::process::ExitCode;

use crate::license;
use crate::pics::{self, export, log, score, Mapping, Suite};

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::{entitlement, verify_dir};
use super::{flag, has_flag, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro pics <command>\n\
    list [--json]                                   The PICS scenarios of the built-in suite\n\
    show <scenario> [--json]                        One scenario: parameters, expected behaviour, bars, mapped clauses\n\
    mapping [--json]                                The clause mapping table\n\
    score <log>... --out <dir> --signer <name> --key-file <file> [--input json|nmea|timing] [--scenario <id>] [--t0 <s>] [--clock-format <fmt>] [--clock-channel <c>] [--suite <suite.toml>] [--utc <iso>] [--license <token>] [--dev]\n\
        Score device logs against the pre-registered bars; write the signed scorecard (Pro)\n\
    export <scenario> --profile <skydel|spirent|labsat|rs-smbv> [--out <file.json>]   The scenario as simulator commands\n\
    sample <scenario> --behaviour <good|bad> [--out <file.json>]   A SYNTHETIC device log, for trying the runner\n\
    verify <dir> --trust <signer public key hex>[,...]   Check a scorecard's signature, files and bars, offline (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn positional(args: &[String]) -> Vec<String> {
    // Arguments after the subcommand that are not flags or flag values.
    const WITH_VALUE: [&str; 14] = [
        "--out",
        "--signer",
        "--key-file",
        "--input",
        "--scenario",
        "--t0",
        "--clock-format",
        "--clock-channel",
        "--suite",
        "--utc",
        "--license",
        "--profile",
        "--behaviour",
        "--trust",
    ];
    let mut out = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if WITH_VALUE.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if !a.starts_with("--") {
            out.push(a.clone());
        }
        i += 1;
    }
    out
}

fn emit(out: Option<String>, text: &str) -> ExitCode {
    match out {
        Some(p) => match std::fs::write(&p, text) {
            Ok(()) => {
                eprintln!("[pics] wrote {p}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(format!("{p}: {e}")),
        },
        None => {
            println!("{text}");
            ExitCode::SUCCESS
        }
    }
}

fn read_logs(args: &[String], files: &[String]) -> Result<Vec<log::DeviceLog>, String> {
    let input = flag(args, "--input").unwrap_or_else(|| "json".into());
    let t0: f64 = flag(args, "--t0")
        .map(|v| v.parse().map_err(|e| format!("--t0: {e}")))
        .transpose()?
        .unwrap_or(0.0);
    let mut logs = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
        let l = match input.as_str() {
            "json" => log::DeviceLog::parse(&text)?,
            "nmea" | "timing" => {
                let sc = flag(args, "--scenario")
                    .ok_or("--input nmea or timing needs --scenario <id>")?;
                if input == "nmea" {
                    log::from_nmea(&text, &sc, t0)?
                } else {
                    let inp = kshana::clock_log::ClockLogInput {
                        format: flag(args, "--clock-format").unwrap_or_else(|| "timelab".into()),
                        text: Some(text),
                        channel: flag(args, "--clock-channel"),
                        ..Default::default()
                    };
                    log::from_timing(&inp, &sc, t0)?
                }
            }
            other => return Err(format!("unknown --input '{other}' (json, nmea or timing)")),
        };
        logs.push(l);
    }
    Ok(logs)
}

pub fn run(args: &[String]) -> ExitCode {
    let suite = Suite::builtin();
    let mapping = Mapping::builtin();
    match args.first().map(String::as_str) {
        Some("list") => {
            if has_flag(args, "--json") {
                return emit(
                    None,
                    &serde_json::to_string_pretty(&suite).expect("suite serialises"),
                );
            }
            println!(
                "{} {} — {} scenarios (bars pre-registered, lock-checked)",
                suite.suite.id,
                suite.suite.version,
                suite.scenarios.len()
            );
            for s in &suite.scenarios {
                println!(
                    "  {:<16} {:<9} {:<13} {}",
                    s.id, s.class, s.technique, s.title
                );
            }
            println!("{}", pics::LABEL);
            ExitCode::SUCCESS
        }
        Some("show") => {
            let Some(id) = positional(args).into_iter().next() else {
                return fail(USAGE);
            };
            let Some(sc) = suite.scenario(&id) else {
                return fail(format!("unknown scenario '{id}'"));
            };
            let rows = mapping.rows_for(&id);
            if has_flag(args, "--json") {
                let v = serde_json::json!({ "scenario": sc, "bars_sha256": sc.bars.sha256(), "mapped_to": rows });
                return emit(None, &serde_json::to_string_pretty(&v).unwrap());
            }
            println!(
                "{} — {}\n  class {} / technique {}; domains {}",
                sc.id,
                sc.title,
                sc.class,
                sc.technique,
                sc.domains.join(", ")
            );
            println!("  expected: {}", sc.expected);
            for (k, v) in &sc.params {
                println!("  param {k} = {v}");
            }
            println!(
                "  bars (sha256 {}): {}",
                sc.bars.sha256(),
                serde_json::to_string(&sc.bars).unwrap()
            );
            for r in rows {
                println!("  mapped: {} — {} — {}", r.standard, r.clause, r.topic);
            }
            ExitCode::SUCCESS
        }
        Some("mapping") => {
            if has_flag(args, "--json") {
                return emit(None, &serde_json::to_string_pretty(&mapping).unwrap());
            }
            for r in &mapping.rows {
                println!(
                    "{:<20} {:<45} {}",
                    r.standard,
                    r.clause,
                    r.scenarios.join(" ")
                );
            }
            ExitCode::SUCCESS
        }
        Some("export") => {
            let Some(id) = positional(args).into_iter().next() else {
                return fail(USAGE);
            };
            let Some(sc) = suite.scenario(&id) else {
                return fail(format!("unknown scenario '{id}'"));
            };
            let Some(p) = flag(args, "--profile") else {
                return fail("export needs --profile");
            };
            match export::export(&suite, sc, &p) {
                Ok(x) => emit(
                    flag(args, "--out"),
                    &serde_json::to_string_pretty(&x).unwrap(),
                ),
                Err(e) => fail(e),
            }
        }
        Some("sample") => {
            let Some(id) = positional(args).into_iter().next() else {
                return fail(USAGE);
            };
            let Some(sc) = suite.scenario(&id) else {
                return fail(format!("unknown scenario '{id}'"));
            };
            let b = match flag(args, "--behaviour").as_deref() {
                Some("good") => log::Behaviour::Good,
                Some("bad") => log::Behaviour::Bad,
                _ => return fail("sample needs --behaviour good or bad"),
            };
            emit(
                flag(args, "--out"),
                &log::synth(&suite, sc, b, 3.0).to_json(),
            )
        }
        Some("score") => {
            let now = now_unix();
            let ent = match entitlement(args, now) {
                Ok(e) => e,
                Err(e) => return fail(e),
            };
            if let Err(e) = license::check(&ent, pics::FEATURE, now) {
                return fail(e);
            }
            let files = positional(args);
            if files.is_empty() {
                return fail(USAGE);
            }
            let (Some(out), Some(signer), Some(kf)) = (
                flag(args, "--out"),
                flag(args, "--signer"),
                flag(args, "--key-file"),
            ) else {
                return fail("score needs --out, --signer and --key-file");
            };
            let seed = match read_seed(&kf) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let suite_src = match flag(args, "--suite") {
                Some(p) => match std::fs::read_to_string(&p) {
                    Ok(s) => s,
                    Err(e) => return fail(format!("{p}: {e}")),
                },
                None => pics::SUITE_V1.to_string(),
            };
            let logs = match read_logs(args, &files) {
                Ok(l) => l,
                Err(e) => return fail(e),
            };
            let s = match score::score(&suite_src, pics::LOCK_V1, &mapping, &logs) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let out = Path::new(&out);
            if let Err(e) = score::write_dir(out, &s, &suite_src, &logs) {
                return fail(e);
            }
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            if let Err(e) = crate::signed_dossier::sign(out, &seed, &signer, &utc) {
                return fail(e);
            }
            eprintln!(
                "[pics] {} cases: {} pass, {} fail, {} incomplete; highest DHS RPCF level supported: {}; signed scorecard in {}",
                s.totals.cases, s.totals.pass, s.totals.fail, s.totals.incomplete, s.levels.highest_supported, out.display()
            );
            ExitCode::SUCCESS
        }
        Some("verify") => {
            let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            if let Err(e) = keys(args, "--trust") {
                return fail(e);
            }
            let sig = verify_dir(args);
            let bars = std::fs::read_to_string(Path::new(dir).join("scorecard.json"))
                .map_err(|e| e.to_string())
                .and_then(|t| {
                    serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string())
                })
                .map(|v| score::check_bars_json(&v));
            match bars {
                Ok(p) if p.is_empty() => {
                    println!("bars: every case matches the pre-registered lock");
                    sig
                }
                Ok(p) => {
                    for x in p {
                        println!("  bars problem: {x}");
                    }
                    ExitCode::FAILURE
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(USAGE),
    }
}
