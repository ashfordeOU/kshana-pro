// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro mbse <command>`: the command-line front end. The binary resolves the
//! licence first (exactly as for the other Pro commands) and then calls [`run`].

use std::path::Path;

use super::impact;
use super::pack;
use super::reqif;
use super::requirements;
use super::sysml;
use super::trace::{import_requirements, vcrm, Trace};
use kshana::api::KshanaError;

/// Usage text for `kshana-pro mbse`.
pub const USAGE: &str = "kshana-pro mbse — requirements, traceability and verification over open Kshana engine outputs\n\
\n\
  kshana-pro mbse import <reqs.csv|.reqif|.sysml> [--to json|csv|reqif|sysml] [--title <t>] [--utc <iso>] [--out <file>]\n\
  kshana-pro mbse trace <trace.toml> [--out <file>]                 Requirement-to-run links with each run's stamp and verdict (JSON)\n\
  kshana-pro mbse vcrm <trace.toml> [--format md|csv|json] [--out <file>] [--fail-on-open]\n\
  kshana-pro mbse sysml <trace.toml> [--out <file>]                 SysML v2 textual model of the trace\n\
  kshana-pro mbse sysml --check <model.sysml>                       Check a file against the supported SysML v2 subset\n\
  kshana-pro mbse impact --old <trace.toml> --new <trace.toml> [--format md|json]\n\
  kshana-pro mbse impact <trace.toml> --scenario-old <a.toml> --scenario-new <b.toml> [--format md|json]\n\
  kshana-pro mbse pack <trace.toml> --out <dir> [--utc <iso>]       Evidence pack: evidence-pack.html + evidence-pack.json\n\
\n\
Every command takes [--license <token>] or, in a debug build, [--dev]. See docs/MBSE.md.";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// The first positional argument after the command (skipping flags and their values).
fn positional(args: &[String]) -> Option<String> {
    let takes_value = [
        "--to",
        "--title",
        "--utc",
        "--out",
        "--format",
        "--old",
        "--new",
        "--scenario-old",
        "--scenario-new",
        "--check",
        "--license",
    ];
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if takes_value.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        return Some(a.clone());
    }
    None
}

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

fn emit(args: &[String], content: &str) -> Result<(), KshanaError> {
    match flag(args, "--out") {
        Some(out) => {
            std::fs::write(&out, content)
                .map_err(|e| err(format!("could not write {out}: {e}")))?;
            eprintln!("[wrote {out}]");
        }
        None => print!("{content}"),
    }
    Ok(())
}

fn need_trace(args: &[String]) -> Result<Trace, KshanaError> {
    let p = positional(args)
        .ok_or_else(|| err(format!("this command needs a trace file\n\n{USAGE}")))?;
    Trace::load(Path::new(&p))
}

/// Run an `mbse` command (`args[0]` is the command). Returns the process exit code.
pub fn run(args: &[String]) -> Result<u8, KshanaError> {
    match args.first().map(String::as_str) {
        Some("import") => {
            let p = positional(args).ok_or_else(|| err("import needs a requirements file"))?;
            let reqs = import_requirements(Path::new(&p))?;
            let title = flag(args, "--title").unwrap_or_else(|| {
                Path::new(&p)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Requirements".into())
            });
            let out = match flag(args, "--to").as_deref().unwrap_or("json") {
                "json" => {
                    serde_json::to_string_pretty(&reqs).expect("requirements serialise") + "\n"
                }
                "csv" => requirements::to_csv(&reqs),
                "reqif" => reqif::to_reqif(&title, &reqs, flag(args, "--utc").as_deref()),
                "sysml" => sysml::export_requirements(&title, &reqs),
                other => return Err(err(format!("--to {other}: use json, csv, reqif or sysml"))),
            };
            eprintln!("[imported {} requirements from {p}]", reqs.len());
            emit(args, &out)?;
            Ok(0)
        }
        Some("trace") => {
            let t = need_trace(args)?;
            let v = vcrm(&t);
            emit(
                args,
                &(serde_json::to_string_pretty(&v.links_json()).expect("json") + "\n"),
            )?;
            Ok(0)
        }
        Some("vcrm") => {
            let t = need_trace(args)?;
            let v = vcrm(&t);
            let out = match flag(args, "--format").as_deref().unwrap_or("md") {
                "md" | "markdown" => v.to_markdown(),
                "csv" => v.to_csv(),
                "json" => v.to_json() + "\n",
                other => return Err(err(format!("--format {other}: use md, csv or json"))),
            };
            emit(args, &out)?;
            eprintln!(
                "[vcrm] {} requirements: {} met ({} on a MODELLED or PARTNER basis), {} not met, {} not verified",
                v.summary.requirements,
                v.summary.met,
                v.summary.met_on_modelled_basis,
                v.summary.not_met,
                v.summary.not_verified
            );
            Ok(
                if has(args, "--fail-on-open") && v.summary.met != v.summary.requirements {
                    1
                } else {
                    0
                },
            )
        }
        Some("sysml") => {
            if let Some(f) = flag(args, "--check") {
                let src = std::fs::read_to_string(&f)
                    .map_err(|e| err(format!("could not read {f}: {e}")))?;
                let (reqs, links) = sysml::requirements_from_sysml(&src).or_else(|e| {
                    // A model with no requirement defs is still checkable.
                    sysml::check(&src)
                        .map(|_| (vec![], Default::default()))
                        .map_err(|_| e)
                })?;
                println!(
                    "{f}: valid in the supported SysML v2 subset ({} requirement definitions, {} verify links)",
                    reqs.len(),
                    links.values().map(Vec::len).sum::<usize>()
                );
                return Ok(0);
            }
            let t = need_trace(args)?;
            let v = vcrm(&t);
            let text = sysml::export(&t, &v);
            // The export must pass its own checker before it is written.
            sysml::check(&text)?;
            emit(args, &text)?;
            Ok(0)
        }
        Some("impact") => {
            let report = if let (Some(o), Some(n)) = (flag(args, "--old"), flag(args, "--new")) {
                impact::compare(&Trace::load(Path::new(&o))?, &Trace::load(Path::new(&n))?)
            } else if let (Some(o), Some(n)) =
                (flag(args, "--scenario-old"), flag(args, "--scenario-new"))
            {
                let t = need_trace(args)?;
                impact::compare_scenarios(&t, Path::new(&o), Path::new(&n))?
            } else {
                return Err(err(format!(
                    "impact needs --old and --new traces, or a trace with --scenario-old and --scenario-new\n\n{USAGE}"
                )));
            };
            let out = match flag(args, "--format").as_deref().unwrap_or("md") {
                "md" | "markdown" => report.to_markdown(),
                "json" => report.to_json() + "\n",
                other => return Err(err(format!("--format {other}: use md or json"))),
            };
            emit(args, &out)?;
            Ok(0)
        }
        Some("pack") => {
            let t = need_trace(args)?;
            let dir = flag(args, "--out").ok_or_else(|| err("pack needs --out <dir>"))?;
            std::fs::create_dir_all(&dir)
                .map_err(|e| err(format!("could not create {dir}: {e}")))?;
            let p = pack::build(&t, flag(args, "--utc"));
            let html = Path::new(&dir).join("evidence-pack.html");
            let json = Path::new(&dir).join("evidence-pack.json");
            std::fs::write(&html, p.to_html()).map_err(|e| err(e.to_string()))?;
            std::fs::write(&json, p.to_json() + "\n").map_err(|e| err(e.to_string()))?;
            eprintln!(
                "[wrote {} and {}; pack SHA-256 {}]",
                html.display(),
                json.display(),
                p.pack_sha256
            );
            Ok(0)
        }
        _ => {
            println!("{USAGE}");
            Ok(if args.is_empty() { 0 } else { 2 })
        }
    }
}
