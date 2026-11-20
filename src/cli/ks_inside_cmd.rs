// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro ks-inside <methods|call|meter>` (backlog E39 B3901, docs/KS-INSIDE.md).

use std::path::Path;
use std::process::ExitCode;

use crate::ks_inside::{Config, KsInside, METHODS};
use crate::license::meter;

use super::{flag, has_flag, now_unix};

const USAGE: &str = "usage: kshana-pro ks-inside <command>\n\
    methods [--json]                                     The SDK's methods and the feature each needs (free)\n\
    call <method> <input-file> --token-file <f> --unit <id> --meter-log <f> [--revocations <f>]\n\
                                                         One metered call, as the C ABI makes it (OEM licence: product sdk, grant pro-ks-inside)\n\
    meter verify <log>                                   Check the meter log's hash chain (free)\n\
    meter summary <log> [--json]                         Per-month calls and active units, for the per-unit report (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

pub fn run(args: &[String]) -> ExitCode {
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("methods"), _) => {
            if has_flag(args, "--json") {
                let v: Vec<_> = METHODS
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "name": m.name,
                            "feature": m.feature,
                            "input": m.input,
                            "summary": m.summary,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({ "abi_version": crate::ks_inside::ABI_VERSION, "methods": v })
                );
            } else {
                println!("KS-Inside ABI {}:", crate::ks_inside::ABI_VERSION);
                for m in METHODS {
                    println!(
                        "  {:<26} [{}] {}",
                        m.name,
                        m.feature.unwrap_or("scenario's pro- kind"),
                        m.summary
                    );
                }
            }
            ExitCode::SUCCESS
        }
        (Some("call"), _) => call(args),
        (Some("meter"), Some("verify")) => match args.get(2) {
            None => fail(USAGE),
            Some(p) => match meter::verify(Path::new(p)) {
                Ok(n) => {
                    println!("meter log chain holds: {n} line(s)");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            },
        },
        (Some("meter"), Some("summary")) => match args.get(2) {
            None => fail(USAGE),
            Some(p) => match meter::summarise(Path::new(p)) {
                Ok(s) if has_flag(args, "--json") => {
                    println!("{}", serde_json::to_string_pretty(&s).expect("serialises"));
                    ExitCode::SUCCESS
                }
                Ok(s) => {
                    println!("{} line(s), chain head {}", s.lines, s.head);
                    for m in &s.months {
                        println!(
                            "  {}  active units {:>6}  ok {:>8}  refused {:>6}  error {:>6}",
                            m.month, m.active_units, m.ok, m.refused, m.error
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            },
        },
        _ => fail(USAGE),
    }
}

fn call(args: &[String]) -> ExitCode {
    let r = (|| -> Result<String, String> {
        let method = args.get(1).filter(|a| !a.starts_with("--")).ok_or(USAGE)?;
        let input = args.get(2).filter(|a| !a.starts_with("--")).ok_or(USAGE)?;
        let need = |n: &str| flag(args, n).ok_or_else(|| format!("call needs {n}"));
        let cfg = Config {
            token: read(&need("--token-file")?)?.trim().to_string(),
            unit_id: need("--unit")?,
            meter_log: need("--meter-log")?.into(),
            revocations: match flag(args, "--revocations") {
                Some(p) => vec![read(&p)?.trim().to_string()],
                None => Vec::new(),
            },
        };
        let now = now_unix();
        let k = KsInside::open(&cfg, now).map_err(|e| e.to_string())?;
        k.call(method, &read(input)?, now)
            .map_err(|e| e.to_string())
    })();
    match r {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
