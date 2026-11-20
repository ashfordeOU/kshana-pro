// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro bridge ...` (ship bridge consistency aid, dead reckoning, ECDIS EP tester,
//! drills) and `kshana-pro dependency ...` (PNT dependency audit, questionnaire, national
//! register). Producing a Pro result needs a licence that unlocks its feature; `dr`,
//! `ep-vectors`, `drills list|show`, `dependency templates|template` and every `verify` need
//! none.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::bridge::{self, consistency, dr, drills, ep};
use crate::dependency::{self, register, templates};
use crate::license;

use super::evidence_cmd::{producing, sign};
use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::{entitlement, verify_dir};
use super::{flag, has_flag, now_unix};

pub const BRIDGE_USAGE: &str = "usage: kshana-pro bridge <command>\n\
    check <log.nmea> [--config <bridge.toml>] [--json] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]\n\
        Cross-sensor consistency of a recorded bridge log (GNSS, GNSS compass, gyro, log, echo-sounder, radar; NMEA 0183 or IEC 61162-450): score per epoch and alerts naming the sensor (Pro)\n\
    dr [--config <dr.toml>] [--speed-kn <kn>] [--depth-m <m>] [--outage-s <s>] [--step-s <s>] [--limit-m <m>] [--monte-carlo <runs>] [--seed <n>] [--json]\n\
        Dead-reckoning error growth during a GNSS outage, 95 % radius, time to a limit, Monte Carlo check (free)\n\
    ep-vectors --out <dir>\n\
        Write the ECDIS estimated-position fallback test vectors (free)\n\
    ep-check <vector-dir> --observed <ecdis.csv> [--json] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]\n\
        Check what an ECDIS showed against a vector: loss indicated, EP within tolerance, manual position kept, frozen position flagged (Pro)\n\
    drills <list | show <id> | build <id> --out <dir> [--signer <name> --key-file <file>] | score <id> --actions <actions.csv> --out <dir> --signer <name> --key-file <file> [--trainee <n>] [--vessel <n>] [--instructor <n>] [--date <d>]> [--license <token>] [--dev]\n\
        GNSS-denied drill packs: stimulus, instructor log, rubric; scoring and a signed training record (list, show free; build, score Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a signed result directory, offline (no licence needed)";

pub const DEPENDENCY_USAGE: &str = "usage: kshana-pro dependency <command>\n\
    templates\n\
        The sector templates (free)\n\
    template <sector>\n\
        Print a starter inventory for a sector (free)\n\
    audit <inventory.toml|.json> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        PNT dependency graph and audit: GNSS denial, spoofing, single points of failure, template coverage; GraphML and JSON export; signed (Pro)\n\
    questionnaire <audit-dir> [--trust <hex>[,...]] [--json] [--license <token>] [--dev]\n\
        Answer the standard GNSS-reliance questions from a signed audit (Pro)\n\
    register <audit-dir>[,<audit-dir>...] --pseudonym-key-file <file> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--min-group <n>] [--title <t>] [--utc <iso>] [--license <token>] [--dev]\n\
        National register: pseudonymous per-operator status and per-sector aggregates with small groups withheld (Enterprise)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a signed audit or register, offline (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

fn positional(args: &[String], i: usize) -> Option<&String> {
    args.get(i).filter(|a| !a.starts_with("--"))
}

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

/// The output directory, signing seed, signer and time of a signed result.
type Signing = (String, [u8; 32], String, String);

/// `--out` with signing, when `--out` is given.
fn optional_signed(args: &[String], feature: &str) -> Result<Option<Signing>, String> {
    if flag(args, "--out").is_none() {
        gate(args, feature)?;
        return Ok(None);
    }
    producing(args, feature).map(Some)
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).expect("serialises") + "\n"
}

pub fn bridge(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("check") => {
            let Some(log) = positional(args, 1) else {
                return fail(BRIDGE_USAGE);
            };
            let r = optional_signed(args, bridge::FEATURE_CONSISTENCY).and_then(|signing| {
                let cfg = match flag(args, "--config") {
                    Some(p) => consistency::Config::parse(&read(&p)?)?,
                    None => consistency::Config::default(),
                };
                let bytes = std::fs::read(log).map_err(|e| format!("{log}: {e}"))?;
                let rep = consistency::run(&bytes, &cfg)?;
                if let Some((out, seed, signer, utc)) = &signing {
                    consistency::write_dir(Path::new(out), &rep)?;
                    sign(out, seed, signer, utc)?;
                }
                Ok((rep, signing.map(|s| s.0)))
            });
            match r {
                Ok((rep, out)) => {
                    match out {
                        Some(out) => eprintln!(
                            "[bridge] {} epochs, {} alerts, {} GNSS outages; signed in {out}",
                            rep.epochs,
                            rep.alerts.len(),
                            rep.outages.len()
                        ),
                        None if has_flag(args, "--json") => print!("{}", json(&rep)),
                        None => print!("{}", consistency::summary_text(&rep)),
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("dr") => {
            let r = (|| -> Result<dr::DrReport, String> {
                let mut m = match flag(args, "--config") {
                    Some(p) => toml::from_str::<dr::DrModel>(&read(&p)?)
                        .map_err(|e| format!("invalid DR model: {e}"))?,
                    None => dr::DrModel::default(),
                };
                let num = |n: &str| -> Result<Option<f64>, String> {
                    flag(args, n)
                        .map(|v| v.parse::<f64>().map_err(|_| format!("{n}: not a number")))
                        .transpose()
                };
                if let Some(v) = num("--speed-kn")? {
                    m.speed_kn = v;
                }
                if let Some(v) = num("--depth-m")? {
                    m.depth_m = Some(v);
                }
                m.validate()?;
                let outage = num("--outage-s")?.unwrap_or(7200.0);
                let step = num("--step-s")?.unwrap_or(600.0);
                let runs = num("--monte-carlo")?.map(|v| v as usize);
                let seed = num("--seed")?.unwrap_or(1.0) as u64;
                Ok(m.report(
                    &dr::times(outage, step),
                    num("--limit-m")?,
                    runs.map(|n| (n, seed)),
                ))
            })();
            match r {
                Ok(rep) if has_flag(args, "--json") => {
                    print!("{}", json(&rep));
                    ExitCode::SUCCESS
                }
                Ok(rep) => {
                    println!(
                        "Dead-reckoning quality at {} kn, {} track. {}\n{}\n",
                        rep.model.speed_kn,
                        if rep.water_track { "water" } else { "bottom" },
                        rep.advisory,
                        rep.label
                    );
                    println!(
                        "{:>8} {:>10} {:>10} {:>10} {:>8}",
                        "t_s", "along_m", "cross_m", "r95_m", "r95_NM"
                    );
                    for r in &rep.table {
                        println!(
                            "{:>8.0} {:>10.1} {:>10.1} {:>10.1} {:>8.3}",
                            r.t_s,
                            r.along_m,
                            r.cross_m,
                            r.r95_m,
                            r.r95_m / 1852.0
                        );
                    }
                    if let Some(l) = rep.limit_m {
                        match rep.time_to_limit_s {
                            Some(t) => println!(
                                "95 % radius reaches {l:.0} m after {:.0} s ({:.2} h)",
                                t,
                                t / 3600.0
                            ),
                            None => println!("95 % radius stays below {l:.0} m for 72 h"),
                        }
                    }
                    if let Some(mc) = &rep.monte_carlo {
                        println!(
                            "Monte Carlo at {:.0} s ({} runs, seed {}): along {:.1} m, cross {:.1} m, r95 {:.1} m",
                            mc.t_s, mc.runs, mc.seed, mc.along_m, mc.cross_m, mc.r95_m
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("ep-vectors") => {
            let Some(out) = flag(args, "--out") else {
                return fail("ep-vectors needs --out <dir>");
            };
            match ep::write_all(Path::new(&out)) {
                Ok(n) => {
                    eprintln!("[bridge] wrote {n} ECDIS EP vectors to {out}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("ep-check") => {
            let Some(vdir) = positional(args, 1) else {
                return fail(BRIDGE_USAGE);
            };
            let r = optional_signed(args, bridge::FEATURE_EP_TEST).and_then(|signing| {
                let obs_path = flag(args, "--observed").ok_or("ep-check needs --observed <csv>")?;
                let v: ep::Vector = serde_json::from_str(&read(
                    &Path::new(vdir).join("vector.json").to_string_lossy(),
                )?)
                .map_err(|e| format!("vector.json: {e}"))?;
                let obs = ep::parse_observed(&read(&obs_path)?)?;
                let res = ep::check(&v, &obs);
                if let Some((out, seed, signer, utc)) = &signing {
                    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;
                    std::fs::write(Path::new(out).join("ep-check.json"), json(&res))
                        .map_err(|e| e.to_string())?;
                    std::fs::write(Path::new(out).join("vector.json"), json(&v))
                        .map_err(|e| e.to_string())?;
                    std::fs::copy(&obs_path, Path::new(out).join("observed.csv"))
                        .map_err(|e| format!("{obs_path}: {e}"))?;
                    sign(out, seed, signer, utc)?;
                }
                Ok(res)
            });
            match r {
                Ok(res) => {
                    if has_flag(args, "--json") {
                        print!("{}", json(&res));
                    } else {
                        println!(
                            "{}: {}",
                            res.vector,
                            if res.passed { "PASS" } else { "FAIL" }
                        );
                        for c in &res.criteria {
                            println!("  {:<26} {:?}: {}", c.id, c.outcome, c.detail);
                        }
                    }
                    if res.passed {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(e) => fail(e),
            }
        }
        Some("drills") => drills_cmd(&args[1..]),
        Some("verify") => verify_dir(args),
        _ => fail(BRIDGE_USAGE),
    }
}

fn drills_cmd(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("list") => {
            for (d, _) in drills::all() {
                println!(
                    "{:<28} {} ({} rubric items{})",
                    d.id,
                    d.title,
                    d.rubric.len(),
                    if d.has_stimulus { "" } else { ", checklist" }
                );
            }
            ExitCode::SUCCESS
        }
        Some("show") => match positional(args, 1).map(|id| drills::find(id)) {
            Some(Ok((d, _))) => {
                print!("{}", json(&d));
                ExitCode::SUCCESS
            }
            Some(Err(e)) => fail(e),
            None => fail(BRIDGE_USAGE),
        },
        Some("build") => {
            let Some(id) = positional(args, 1) else {
                return fail(BRIDGE_USAGE);
            };
            let r = (|| -> Result<String, String> {
                let out = flag(args, "--out").ok_or("drills build needs --out <dir>")?;
                let signing = if flag(args, "--signer").is_some() {
                    Some(producing(args, bridge::FEATURE_DRILLS)?)
                } else {
                    gate(args, bridge::FEATURE_DRILLS)?;
                    None
                };
                let b = drills::build(id)?;
                drills::write_dir(Path::new(&out), &b)?;
                if let Some((o, seed, signer, utc)) = signing {
                    sign(&o, &seed, &signer, &utc)?;
                }
                Ok(out)
            })();
            match r {
                Ok(out) => {
                    eprintln!("[bridge] drill {id} written to {out}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("score") => {
            let Some(id) = positional(args, 1) else {
                return fail(BRIDGE_USAGE);
            };
            let r = producing(args, bridge::FEATURE_DRILLS).and_then(|(out, seed, signer, utc)| {
                let (d, _) = drills::find(id)?;
                let ap = flag(args, "--actions").ok_or("drills score needs --actions <csv>")?;
                let actions = drills::parse_actions(&read(&ap)?)?;
                let session = drills::Session {
                    trainee: flag(args, "--trainee").unwrap_or_default(),
                    vessel_or_centre: flag(args, "--vessel").unwrap_or_default(),
                    instructor: flag(args, "--instructor").unwrap_or_default(),
                    date: flag(args, "--date").unwrap_or_else(|| utc.clone()),
                };
                let s = drills::score(&d, &actions, session);
                let o = Path::new(&out);
                std::fs::create_dir_all(o).map_err(|e| format!("{out}: {e}"))?;
                let w = |n: &str, t: String| {
                    std::fs::write(o.join(n), t).map_err(|e| format!("{n}: {e}"))
                };
                w("score.json", json(&s))?;
                w("score.txt", drills::score_text(&s))?;
                w("rubric.json", json(&d))?;
                w("actions.csv", read(&ap)?)?;
                sign(&out, &seed, &signer, &utc)?;
                Ok((s, out))
            });
            match r {
                Ok((s, out)) => {
                    eprintln!(
                        "[bridge] {}: {:.1} % ({}); signed training record in {out}",
                        s.drill,
                        s.percent,
                        if s.passed { "pass" } else { "not yet" }
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(BRIDGE_USAGE),
    }
}

pub fn dependency(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("templates") => {
            for t in templates::TEMPLATES {
                println!("{:<16} {} ({} items)", t.id, t.title, t.items.len());
            }
            ExitCode::SUCCESS
        }
        Some("template") => match positional(args, 1).map(|s| templates::starter(s)) {
            Some(Ok(s)) => {
                print!("{s}");
                ExitCode::SUCCESS
            }
            Some(Err(e)) => fail(e),
            None => fail(DEPENDENCY_USAGE),
        },
        Some("audit") => {
            let Some(file) = positional(args, 1) else {
                return fail(DEPENDENCY_USAGE);
            };
            let r =
                producing(args, dependency::FEATURE_AUDIT).and_then(|(out, seed, signer, utc)| {
                    let src = read(file)?;
                    let inv = dependency::Inventory::parse(&src)?;
                    let a = dependency::audit(&inv, &src)?;
                    dependency::write_dir(Path::new(&out), &a, &inv, &src)?;
                    sign(&out, &seed, &signer, &utc)?;
                    Ok((a, out))
                });
            match r {
                Ok((a, out)) => {
                    eprintln!(
                        "[dependency] {}: status {}, {} findings ({} high); signed audit in {out}",
                        a.asset,
                        a.status.level,
                        a.findings.len(),
                        a.findings.iter().filter(|f| f.severity == "high").count()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("questionnaire") => {
            let Some(dir) = positional(args, 1) else {
                return fail(DEPENDENCY_USAGE);
            };
            let r = gate(args, dependency::FEATURE_AUDIT).and_then(|()| {
                let trust = keys(args, "--trust")?;
                let (a, sha, _) = register::read_audit(Path::new(dir), &trust)?;
                Ok((a, sha))
            });
            match r {
                Ok((a, sha)) => {
                    let qs = dependency::questionnaire(&a);
                    if has_flag(args, "--json") {
                        print!(
                            "{}",
                            json(&serde_json::json!({
                                "asset": a.asset, "operator": a.operator, "audit_sha256": sha,
                                "label": "MODELLED", "answers": qs
                            }))
                        );
                    } else {
                        println!(
                            "GNSS reliance questionnaire: {} ({}). Answers drawn from the signed audit {}.\n",
                            a.asset, a.operator, sha
                        );
                        for q in qs {
                            println!("{} {}\n   {} [{}]\n", q.id, q.question, q.answer, q.basis);
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("register") => {
            let Some(list) = positional(args, 1) else {
                return fail(DEPENDENCY_USAGE);
            };
            let r = producing(args, dependency::FEATURE_REGISTER).and_then(
                |(out, seed, signer, utc)| {
                    let trust = keys(args, "--trust")?;
                    let kf = flag(args, "--pseudonym-key-file")
                        .ok_or("register needs --pseudonym-key-file <file>")?;
                    let key = read_seed(&kf)?;
                    let min_group = flag(args, "--min-group")
                        .map(|v| {
                            v.parse::<usize>()
                                .map_err(|_| "--min-group: not a number".to_string())
                        })
                        .transpose()?
                        .unwrap_or(3);
                    let dirs: Vec<PathBuf> = list
                        .split(',')
                        .filter(|s| !s.trim().is_empty())
                        .map(PathBuf::from)
                        .collect();
                    let title =
                        flag(args, "--title").unwrap_or_else(|| "PNT dependency register".into());
                    let reg = register::build(&title, &dirs, &trust, &key, min_group)?;
                    register::write_dir(Path::new(&out), &reg)?;
                    sign(&out, &seed, &signer, &utc)?;
                    Ok((reg, out))
                },
            );
            match r {
                Ok((reg, out)) => {
                    eprintln!(
                        "[dependency] register: {} operators in {} sectors ({} withheld), {} audits rejected; signed in {out}",
                        reg.entries.len(),
                        reg.sectors.len(),
                        reg.sectors.iter().filter(|s| s.suppressed).count(),
                        reg.rejected.len()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        Some("verify") => verify_dir(args),
        _ => fail(DEPENDENCY_USAGE),
    }
}
