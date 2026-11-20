// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro avionics <...>`: the avionics test library ([`crate::avionics`]). `generate`
//! and `run` need `pro-avionics-tests`; `recovery` needs `pro-avionics-recovery`; `evidence`
//! and `kit` need `pro-avionics-kit` (Enterprise); `list`, `show`, `validate`, `sample` and
//! `verify` need none.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::avionics::procedure::{self, DutLog, ParamsFile, Procedure};
use crate::avionics::{self, kit, library, recovery, sbas_seq};
use crate::license;
use crate::testkit::score::ScenarioVerdict;

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro avionics <command>\n\
    list [--json]                                            Built-in procedures (free)\n\
    show <id>                                                One procedure as TOML (free)\n\
    validate <procedure.toml|id>                             Parse and check a procedure (free)\n\
    generate <procedure.toml|id|sequence.toml> --out <dir>   SBAS frame stream (frames.csv) and expected behaviour (stream.json) (Pro)\n\
    run <procedure.toml|id> --out <dir> --signer <name> --key-file <file> [--params <params.toml>] [--log <dut-log.json>] [--utc <iso>]\n\
        Score the equipment log against the procedure; signed verdict directory; exits 1 on a check not met (Pro)\n\
    sample <procedure.toml|id> --behaviour good|bad [--out <log.json>]   A SYNTHETIC equipment log, to try the scoring (free)\n\
    recovery --fleet <fleet.toml> --verdicts <dir>[,...] --out <dir> --signer <name> --key-file <file>\n\
        [--target <s> --target-source <text>] [--trust <key hex>[,...]]\n\
        Recovery-time scorecard per avionics type from test-kit verdicts (Pro)\n\
    evidence --trace <trace.toml> --runs <dir>[,...] --out <dir> --signer <name> --key-file <file> --trust <key hex>[,...]\n\
        Requirement-to-run traceability and test records over signed procedure verdicts (Enterprise)\n\
    kit --out <dir> --signer <name> --key-file <file>        Tool-qualification support kit: verification cases run now, configuration index, drafts (Enterprise)\n\
    verify <dir> --trust <key hex>[,...]                     Check a signed output directory (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

const WITH_VALUE: [&str; 15] = [
    "--out",
    "--signer",
    "--key-file",
    "--params",
    "--log",
    "--utc",
    "--behaviour",
    "--fleet",
    "--verdicts",
    "--target",
    "--target-source",
    "--trust",
    "--trace",
    "--runs",
    "--license",
];

fn positional(args: &[String]) -> Vec<String> {
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

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn signing(args: &[String]) -> Result<([u8; license::KEY_LEN], String, String), String> {
    let (Some(signer), Some(kf)) = (flag(args, "--signer"), flag(args, "--key-file")) else {
        return Err("needs --signer and --key-file".into());
    };
    Ok((
        read_seed(&kf)?,
        signer,
        flag(args, "--utc").unwrap_or_else(utc_iso8601_now),
    ))
}

/// A procedure from a file or the library.
fn load_procedure(name: &str) -> Result<Procedure, String> {
    if Path::new(name).is_file() {
        return Procedure::load(Path::new(name));
    }
    library::get(name)
}

fn out_dir(args: &[String]) -> Result<PathBuf, String> {
    let out = PathBuf::from(flag(args, "--out").ok_or("needs --out <dir>")?);
    if out.join(crate::signed_dossier::SIGNATURE_FILE).exists() {
        return Err(format!(
            "{} already holds signed output; it is never overwritten",
            out.display()
        ));
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(out)
}

fn write(dir: &Path, name: &str, text: &str) -> Result<(), String> {
    std::fs::write(dir.join(name), text).map_err(|e| format!("{}/{name}: {e}", dir.display()))
}

fn sign(dir: &Path, args: &[String]) -> Result<(), String> {
    let (seed, signer, utc) = signing(args)?;
    crate::signed_dossier::sign(dir, &seed, &signer, &utc).map_err(|e| e.to_string())?;
    Ok(())
}

fn dirs(args: &[String], name: &str) -> Result<Vec<PathBuf>, String> {
    let v = flag(args, name).ok_or_else(|| format!("needs {name} <dir>[,...]"))?;
    Ok(v.split(',')
        .filter(|s| !s.trim().is_empty())
        .map(|s| PathBuf::from(s.trim()))
        .collect())
}

pub fn run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(c) => c,
        Err(e) => fail(e),
    }
}

fn run_inner(args: &[String]) -> Result<ExitCode, String> {
    let pos = positional(args);
    match args.first().map(String::as_str) {
        Some("list") => {
            let ps: Vec<Procedure> = library::PROCEDURES
                .iter()
                .map(|(id, _)| library::get(id))
                .collect::<Result<_, _>>()?;
            if has_flag(args, "--json") {
                let rows: Vec<serde_json::Value> = ps
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "id": p.id, "version": p.version, "kind": p.kind,
                            "title": p.title, "modelled_on": p.modelled_on,
                            "parameters": p.parameters.iter().map(|x| &x.name).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&rows).unwrap());
            } else {
                println!("Avionics test procedures ({})", avionics::LABEL);
                for p in &ps {
                    println!(
                        "  {:<32} {:<13} {}",
                        p.id,
                        serde_json::to_value(p.kind).unwrap().as_str().unwrap(),
                        p.title
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("show") => {
            let id = pos.first().ok_or(USAGE)?;
            print!(
                "{}",
                library::find(id).ok_or(format!("unknown procedure '{id}'"))?
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("validate") => {
            let p = load_procedure(pos.first().ok_or(USAGE)?)?;
            println!("{} v{}: procedure valid ({:?})", p.id, p.version, p.kind);
            for x in &p.parameters {
                println!(
                    "  parameter {:<20} {:<4} {}",
                    x.name,
                    x.unit,
                    match x.public_value {
                        Some(v) => format!("public value {v} ({})", x.public_source),
                        None => format!("applicant supplies ({})", x.source),
                    }
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("generate") => {
            gate(args, avionics::FEATURE)?;
            let name = pos.first().ok_or(USAGE)?;
            let seq = match load_procedure(name) {
                Ok(p) => p
                    .sequence
                    .ok_or_else(|| format!("{name}: not an SBAS-sequence procedure"))?,
                Err(e) => {
                    let src = std::fs::read_to_string(name).map_err(|_| e)?;
                    sbas_seq::SequenceSpec::parse_file(&src)?
                }
            };
            let stream = sbas_seq::generate(&seq)?;
            let out = PathBuf::from(flag(args, "--out").ok_or("needs --out <dir>")?);
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            write(&out, "frames.csv", &stream.frames_csv())?;
            write(
                &out,
                "stream.json",
                &serde_json::to_string_pretty(&stream).unwrap(),
            )?;
            eprintln!(
                "[avionics] {} frames, {} expectations, frames hash {} -> {}",
                stream.frames.len(),
                stream.expectations.len(),
                stream.frames_sha256,
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("run") => {
            gate(args, avionics::FEATURE)?;
            let p = load_procedure(pos.first().ok_or(USAGE)?)?;
            signing(args)?;
            let params_file = flag(args, "--params")
                .map(|f| {
                    std::fs::read_to_string(&f)
                        .map_err(|e| format!("{f}: {e}"))
                        .and_then(|s| ParamsFile::parse(&s))
                })
                .transpose()?;
            let params = p.resolve_params(params_file.as_ref());
            let log_bytes = flag(args, "--log")
                .map(|f| std::fs::read(&f).map_err(|e| format!("{f}: {e}")))
                .transpose()?;
            let log = log_bytes
                .as_ref()
                .map(|b| DutLog::parse(&String::from_utf8_lossy(b)))
                .transpose()?;
            let v = procedure::score(&p, &params, log.as_ref(), log_bytes.as_deref())?;
            let out = out_dir(args)?;
            write(
                &out,
                "verdict.json",
                &serde_json::to_string_pretty(&v).unwrap(),
            )?;
            write(&out, "procedure.toml", &p.to_toml())?;
            if let Some(pf) = &params_file {
                write(
                    &out,
                    "params.toml",
                    &toml::to_string(pf).unwrap_or_default(),
                )?;
            }
            if let Some(b) = &log_bytes {
                std::fs::write(out.join("dut-log.json"), b)
                    .map_err(|e| format!("{}: {e}", out.display()))?;
            }
            if let Some(seq) = &p.sequence {
                write(&out, "frames.csv", &sbas_seq::generate(seq)?.frames_csv())?;
            }
            sign(&out, args)?;
            println!(
                "{} v{}: {} (met {}, not met {}, not assessed {}) -> {}",
                v.procedure,
                v.procedure_version,
                v.verdict,
                v.met,
                v.not_met,
                v.not_assessed,
                out.display()
            );
            for c in v.checks.iter().filter(|c| c.kind != "reference") {
                println!("  {:<44} {:<13} {}", c.id, c.verdict.as_str(), c.detail);
            }
            Ok(if v.not_met > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Some("sample") => {
            let p = load_procedure(pos.first().ok_or(USAGE)?)?;
            let good = match flag(args, "--behaviour").as_deref() {
                Some("good") => true,
                Some("bad") => false,
                _ => return Err("needs --behaviour good|bad".into()),
            };
            let log = procedure::synth_log(&p, good)?;
            match flag(args, "--out") {
                Some(f) => {
                    std::fs::write(&f, log.to_json()).map_err(|e| format!("{f}: {e}"))?;
                    eprintln!("[avionics] wrote SYNTHETIC log {f}");
                }
                None => println!("{}", log.to_json()),
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("recovery") => {
            gate(args, avionics::FEATURE_RECOVERY)?;
            let fleet_path = flag(args, "--fleet").ok_or("needs --fleet <fleet.toml>")?;
            let fleet = recovery::Fleet::parse(
                &std::fs::read_to_string(&fleet_path).map_err(|e| format!("{fleet_path}: {e}"))?,
            )?;
            signing(args)?;
            let trust = keys(args, "--trust")?;
            let mut verdicts = Vec::new();
            let mut skipped = Vec::new();
            for d in kit::find_verdicts(&dirs(args, "--verdicts")?)? {
                let text = std::fs::read_to_string(d.join("verdict.json"))
                    .map_err(|e| format!("{}: {e}", d.display()))?;
                let Ok(v) = serde_json::from_str::<ScenarioVerdict>(&text) else {
                    skipped.push(format!("{}: not a test-kit verdict", d.display()));
                    continue;
                };
                if !trust.is_empty() {
                    match crate::signed_dossier::verify(&d, &trust) {
                        Ok(r) if r.passed() => {}
                        _ => {
                            skipped.push(format!("{}: signature not trusted", d.display()));
                            continue;
                        }
                    }
                }
                verdicts.push((d.display().to_string(), v));
            }
            let target = flag(args, "--target")
                .map(|t| {
                    t.parse::<f64>()
                        .map_err(|_| format!("--target {t}: not a number"))
                })
                .transpose()?;
            let source =
                flag(args, "--target-source").unwrap_or_else(|| "operator-supplied target".into());
            let sc = recovery::scorecard(&fleet, &verdicts, target, &source)?;
            let out = out_dir(args)?;
            write(
                &out,
                "scorecard.json",
                &serde_json::to_string_pretty(&sc).unwrap(),
            )?;
            let mut md = recovery::markdown(&sc);
            if !skipped.is_empty() {
                md.push_str(&format!(
                    "\nInputs left out:\n\n- {}\n",
                    skipped.join("\n- ")
                ));
            }
            write(&out, "scorecard.md", &md)?;
            sign(&out, args)?;
            print!("{md}");
            Ok(ExitCode::SUCCESS)
        }
        Some("evidence") => {
            gate(args, avionics::FEATURE_KIT)?;
            let tp = flag(args, "--trace").ok_or("needs --trace <trace.toml>")?;
            let trace = kit::Trace::parse(
                &std::fs::read_to_string(&tp).map_err(|e| format!("{tp}: {e}"))?,
            )?;
            signing(args)?;
            let trust = keys(args, "--trust")?;
            if trust.is_empty() {
                return Err(
                    "needs --trust <key hex>: only runs signed by a trusted key count".into(),
                );
            }
            let runs = kit::find_verdicts(&dirs(args, "--runs")?)?;
            let pack = kit::evidence_pack(&trace, &runs, &trust)?;
            let out = out_dir(args)?;
            kit::write_evidence_pack(&out, &trace, &pack)?;
            sign(&out, args)?;
            println!(
                "{} runs, {} trace rows, requirements with gaps: {} -> {}",
                pack.runs.len(),
                pack.matrix.len(),
                if pack.requirements_with_gaps.is_empty() {
                    "none".into()
                } else {
                    pack.requirements_with_gaps.join(", ")
                },
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("kit") => {
            gate(args, avionics::FEATURE_KIT)?;
            signing(args)?;
            let out = out_dir(args)?;
            let r = kit::write_kit(&out)?;
            sign(&out, args)?;
            println!(
                "verification cases: {} passed, {} failed -> {}",
                r.passed,
                r.failed,
                out.display()
            );
            for c in r.cases.iter().filter(|c| c.result != "pass") {
                println!(
                    "  FAILED {}: expected {}; got {}",
                    c.id, c.expected, c.actual
                );
            }
            Ok(if kit::all_passed(&r) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Some("verify") => Ok(super::scorecard_cmd::verify_dir(args)),
        _ => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
    }
}
