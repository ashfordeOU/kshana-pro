// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro pntdp <run|vectors|check>` (W1119 C2). `run` needs a licence that unlocks
//! `pro-multi-source-trust`; `vectors` and `check` need none.

use std::path::Path;
use std::process::ExitCode;

use crate::license;
use crate::pntdp::{self, vectors, Config};

use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro pntdp <command>\n\
    run <inputs.csv> [--config <config.toml>] [--out <outputs.csv>] [--json] [--license <token>] [--dev]\n\
        Multi-source PNT data processing (MSC.1/Circ.1575): selected solution, integrity level and flags per epoch (Pro)\n\
    vectors --out <dir>     Write the test vectors (inputs plus expected integrity flags)\n\
    check <vector-dir>      Run one vector directory and compare with its expected.csv";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("run") => {
            let now = now_unix();
            let ent = match entitlement(args, now) {
                Ok(e) => e,
                Err(e) => return fail(e),
            };
            if let Err(e) = license::check(&ent, pntdp::FEATURE, now) {
                return fail(e);
            }
            let Some(inp) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let cfg_src = match flag(args, "--config") {
                Some(p) => match read(&p) {
                    Ok(s) => s,
                    Err(e) => return fail(e),
                },
                None => vectors::CONFIG_FULL.to_string(),
            };
            let result = Config::parse(&cfg_src).and_then(|cfg| {
                let text = read(inp)?;
                let samples = pntdp::parse_inputs(&cfg, &text)?;
                pntdp::run(&cfg, &samples)
            });
            let out = match result {
                Ok(o) => o,
                Err(e) => return fail(e),
            };
            let text = if has_flag(args, "--json") {
                serde_json::to_string_pretty(&serde_json::json!({
                    "method": pntdp::METHOD,
                    "label": pntdp::LABEL,
                    "epochs": out,
                }))
                .expect("outputs serialise")
            } else {
                pntdp::outputs_csv(&out)
            };
            let worst = out.iter().map(|o| o.integrity).min();
            match flag(args, "--out") {
                Some(p) => {
                    if let Err(e) = std::fs::write(&p, text) {
                        return fail(format!("{p}: {e}"));
                    }
                    eprintln!(
                        "[pntdp] {} epochs; lowest integrity {}; wrote {p}",
                        out.len(),
                        worst.map(|i| i.as_str()).unwrap_or("-")
                    );
                }
                None => print!("{text}"),
            }
            ExitCode::SUCCESS
        }
        Some("vectors") => {
            let Some(out) = flag(args, "--out") else {
                return fail("vectors needs --out <dir>");
            };
            match vectors::write_all(Path::new(&out)) {
                Ok(()) => {
                    eprintln!("[pntdp] wrote {} vectors to {out}", vectors::all().len());
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("check") => {
            let Some(dir) = args.get(1).filter(|a| !a.starts_with("--")) else {
                return fail(USAGE);
            };
            let d = Path::new(dir);
            let r = (|| -> Result<Vec<String>, String> {
                let cfg = Config::parse(&read(&d.join("config.toml").to_string_lossy())?)?;
                let samples =
                    pntdp::parse_inputs(&cfg, &read(&d.join("inputs.csv").to_string_lossy())?)?;
                let exp =
                    vectors::parse_expected(&read(&d.join("expected.csv").to_string_lossy())?)?;
                Ok(vectors::compare(&pntdp::run(&cfg, &samples)?, &exp))
            })();
            match r {
                Ok(p) if p.is_empty() => {
                    println!("{dir}: every asserted epoch matches");
                    ExitCode::SUCCESS
                }
                Ok(p) => {
                    for x in p.iter().take(20) {
                        println!("  mismatch: {x}");
                    }
                    fail(format!("{dir}: {} mismatches", p.len()))
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(USAGE),
    }
}
