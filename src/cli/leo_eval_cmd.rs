// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro leo-eval ...` (research 2026-11-20 gaps, epic E21: B2101, B2107, B2108;
//! docs/LEO-EVALUATION.md).

use std::process::ExitCode;

use crate::leo_eval::{self, claims, od, sims};

use super::flag;
use super::grid_cmd::gate;

pub const LEO_EVAL_USAGE: &str = "usage: kshana-pro leo-eval <command>\n\
    sim-hash <procedure.toml>                                        SHA-256 to register before the runs (no licence needed)\n\
    sim-score <procedure.toml> <runs.csv> --registered <sha256> [--json <out>]   Multi-simulator scorecard in input order, never ranked (Pro)\n\
    sisre <diffs.csv> --altitude-km <h> [--mask-deg <m>] [--json <out>]   SISRE of broadcast minus precise orbit and clock with numeric weights; overbound of the broadcast sigma (Pro)\n\
    dcb <dcb.csv> [--k <k>] [--json <out>]                           DCB day-to-day stability and jumps (Pro)\n\
    deps <register.toml> [--json <out>]                              PPP dependency register rated (Pro)\n\
    claims-template [--out <dir>]                                    Claim templates for Celeste, CentiSpace, Geespace: placeholders only (no licence needed)\n\
    claims-check <claims.json>                                       Refuse values without source URL, date and quote (no licence needed)\n\
    claims-compare <claims.json> <measured.csv> [--json <out>]       Claimed against measured (`claim,value`) (Pro)\n\
    (Pro commands take [--license <token>] [--dev])";

fn fail(m: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {m}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

fn emit<T: serde::Serialize>(args: &[String], v: &T) -> Result<(), String> {
    let t = crate::transport::rounded_json(v);
    match flag(args, "--json") {
        Some(o) => std::fs::write(&o, t).map_err(|e| format!("{o}: {e}")),
        None => {
            println!("{t}");
            Ok(())
        }
    }
}

fn num(args: &[String], n: &str, d: Option<f64>) -> Result<f64, String> {
    match flag(args, n) {
        Some(v) => v.parse().map_err(|_| format!("{n} must be a number")),
        None => d.ok_or_else(|| format!("{n} is needed")),
    }
}

/// `kshana-pro leo-eval ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let pos: Vec<&String> = args
        .iter()
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .collect();
    let free = matches!(sub, "sim-hash" | "claims-template" | "claims-check");
    if !free {
        if let Err(e) = gate(args, leo_eval::FEATURE) {
            return fail(e);
        }
    }
    let r: Result<String, String> = (|| match (sub, pos.as_slice()) {
        ("sim-hash", [p]) => {
            println!("{}", sims::procedure_hash(&read(p)?));
            Ok("[leo-eval] register this hash before taking runs".into())
        }
        ("sim-score", [p, r]) => {
            let reg = flag(args, "--registered").ok_or("--registered <sha256> is needed")?;
            let s = sims::score(&read(p)?, &reg, &read(r)?)?;
            emit(args, &s)?;
            Ok(format!(
                "[leo-eval] {} simulator(s), {} step(s)",
                s.simulators.len(),
                s.steps.len()
            ))
        }
        ("sisre", [d]) => {
            let w = od::weights(
                num(args, "--altitude-km", None)?,
                num(args, "--mask-deg", Some(5.0))?,
            )?;
            let s = od::sisre(&read(d)?, &w)?;
            emit(args, &serde_json::json!({"weights": w, "satellites": s}))?;
            Ok(format!("[leo-eval] SISRE for {} satellite(s)", s.len()))
        }
        ("dcb", [d]) => {
            let v = od::dcb(&read(d)?, num(args, "--k", Some(3.0))?)?;
            emit(args, &v)?;
            Ok(format!(
                "[leo-eval] {} satellite(s), {} jump(s)",
                v.len(),
                v.iter().map(|x| x.jumps.len()).sum::<usize>()
            ))
        }
        ("deps", [d]) => {
            #[derive(serde::Deserialize)]
            struct R {
                dependency: Vec<od::Dependency>,
            }
            let r: R = toml::from_str(&read(d)?).map_err(|e| format!("{d}: {e}"))?;
            let v = od::deps(&r.dependency)?;
            emit(args, &v)?;
            Ok(format!(
                "[leo-eval] {} critical dependency(ies)",
                v.iter().filter(|x| x.rating == "critical").count()
            ))
        }
        ("claims-template", []) => {
            let out = flag(args, "--out");
            for d in claims::templates() {
                let t = serde_json::to_string_pretty(&d).map_err(|e| e.to_string())? + "\n";
                match &out {
                    Some(o) => {
                        std::fs::create_dir_all(o).map_err(|e| e.to_string())?;
                        let f = format!("{o}/{}.json", d.constellation.to_lowercase());
                        std::fs::write(&f, t).map_err(|e| format!("{f}: {e}"))?;
                    }
                    None => print!("{t}"),
                }
            }
            Ok("[leo-eval] templates written: fill each claim from a public source".into())
        }
        ("claims-check", [f]) => {
            let d: claims::Dataset =
                serde_json::from_str(&read(f)?).map_err(|e| format!("{f}: {e}"))?;
            let n = claims::check(&d)?;
            Ok(format!(
                "[leo-eval] {}: {} claim(s), {n} placeholder(s)",
                d.constellation,
                d.claims.len()
            ))
        }
        ("claims-compare", [f, m]) => {
            let d: claims::Dataset =
                serde_json::from_str(&read(f)?).map_err(|e| format!("{f}: {e}"))?;
            let t = crate::transport::Table::parse(&read(m)?)?;
            let (cc, cv) = (t.need("claim")?, t.need("value")?);
            let mut meas = Vec::new();
            for i in 0..t.rows.len() {
                if let Some(v) = t.num(i, Some(cv))? {
                    meas.push((t.text(i, Some(cc)).to_string(), v));
                }
            }
            let c = claims::compare(&d, &meas)?;
            emit(args, &c)?;
            Ok(format!(
                "[leo-eval] {} of {} compared claim(s) reproduced",
                c.iter().filter(|x| x.reproduced).count(),
                c.len()
            ))
        }
        _ => Err(LEO_EVAL_USAGE.into()),
    })();
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
