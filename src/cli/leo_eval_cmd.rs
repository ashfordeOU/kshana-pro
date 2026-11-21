// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro leo-eval ...` (research 2026-11-20 gaps, epic E21: B2101, B2107, B2108;
//! docs/LEO-EVALUATION.md).

use std::process::ExitCode;

use crate::leo_eval::{self, claims, od, sims};

use super::flag;
use super::grid_cmd::gate;

pub const LEO_EVAL_USAGE: &str = "usage: kshana-pro leo-eval <command>\n\
    sim-procedure <id>                                               A published procedure, frozen at 1.0 (no licence needed)\n\
    sim-hash <procedure.toml>                                        Method hash to register before the runs (no licence needed)\n\
    sim-score <procedure.toml> <runs.csv> [--registered <sha256>] [--json <out>]   Multi-simulator scorecard in input order, never ranked (Pro)\n\
    sim-equivalence <procedure.toml> <runs.csv> [--registered <sha256>] [--json <out>]   Simulator equivalence within the stated tolerances, self-test verdict vocabulary (Pro)\n\
    doppler --altitude-km <h> --carrier-hz <f>                       Doppler and Doppler-rate bounds for a procedure's stress steps (Pro)\n\
    sisre <diffs.csv> --altitude-km <h> [--mask-deg <m>] [--json <out>]   SISRE of broadcast minus precise orbit and clock with numeric weights; overbound of the broadcast sigma (Pro)\n\
    traim <study.toml> [--json <out>]                                T-RAIM fault-injection study on the open snapshot RAIM: false alarms, detection, exclusion, recovery by redundancy (Pro)\n\
    dcb <dcb.csv> [--k <k>] [--json <out>]                           DCB day-to-day stability and jumps (Pro)\n\
    deps <register.toml> [--json <out>]                              PPP dependency register rated (Pro)\n\
    dd-template <claims.json> [--measured <claim,value csv>] [--out <file.md>]   Investor due-diligence report template; every verdict cites its source (Pro)\n\
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
    let free = matches!(sub, "sim-hash" | "sim-procedure" | "claims-check");
    if !free {
        if let Err(e) = gate(args, leo_eval::FEATURE) {
            return fail(e);
        }
    }
    let r: Result<String, String> = (|| match (sub, pos.as_slice()) {
        ("sim-procedure", [id]) => {
            let t = sims::PUBLISHED
                .iter()
                .find(|x| x.0 == id.as_str())
                .ok_or_else(|| {
                    format!(
                        "unknown procedure (published: {})",
                        sims::PUBLISHED
                            .iter()
                            .map(|x| x.0)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            print!("{}", t.1);
            Ok("[leo-eval] published procedure, frozen at 1.0: copy it, add your bars and tolerances, register its hash".into())
        }
        ("sim-hash", [p]) => {
            println!("{}", sims::procedure_hash(&read(p)?)?);
            Ok("[leo-eval] register this hash before taking runs".into())
        }
        ("sim-score", [p, r]) => {
            let reg = flag(args, "--registered");
            let s = sims::score(&read(p)?, reg.as_deref(), &read(r)?)?;
            emit(args, &s)?;
            Ok(format!(
                "[leo-eval] {} simulator(s), {} step(s)",
                s.simulators.len(),
                s.steps.len()
            ))
        }
        ("sim-equivalence", [p, r]) => {
            let reg = flag(args, "--registered");
            let e = sims::equivalence(&read(p)?, reg.as_deref(), &read(r)?)?;
            emit(args, &e)?;
            Ok(format!(
                "[leo-eval] {} step comparison(s): {}",
                e.steps.len(),
                e.steps
                    .iter()
                    .map(|x| format!(
                        "{} {} vs {}: {}",
                        x.step, x.simulator_a, x.simulator_b, x.verdict
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        }
        ("doppler", []) => {
            let (d, r) = sims::doppler_bounds(
                num(args, "--altitude-km", None)?,
                num(args, "--carrier-hz", None)?,
            )?;
            println!("{{\"max_doppler_hz\": {d:.1}, \"max_doppler_rate_hz_s\": {r:.2}}}");
            Ok("[leo-eval] circular orbit, Earth rotation neglected".into())
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
        ("traim", [d]) => {
            let q: crate::leo_eval::traim::Study =
                toml::from_str(&read(d)?).map_err(|e| format!("{d}: {e}"))?;
            let r = crate::leo_eval::traim::run(&q)?;
            emit(args, &r)?;
            Ok(format!(
                "[leo-eval] T-RAIM study over {} redundancy case(s)",
                r.rows.len()
            ))
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
        ("dd-template", [f]) => {
            let d: claims::Dataset =
                serde_json::from_str(&read(f)?).map_err(|e| format!("{f}: {e}"))?;
            let mut meas = Vec::new();
            let mut src = "none supplied".to_string();
            if let Some(m) = flag(args, "--measured") {
                let t = crate::transport::Table::parse(&read(&m)?)?;
                let (cc, cv) = (t.need("claim")?, t.need("value")?);
                for i in 0..t.rows.len() {
                    if let Some(v) = t.num(i, Some(cv))? {
                        meas.push((t.text(i, Some(cc)).to_string(), v));
                    }
                }
                src = m;
            }
            let md = claims::dd_report(&d, &meas, &src)?;
            match flag(args, "--out") {
                Some(o) => std::fs::write(&o, md).map_err(|e| format!("{o}: {e}"))?,
                None => print!("{md}"),
            }
            Ok("[leo-eval] due-diligence template written".into())
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
