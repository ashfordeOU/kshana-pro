// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro space-rx <...>`: space-segment GNSS receiver evidence ([`crate::space_rx`]).
//! `exposure` needs `pro-space-rx-exposure`; `score` and `db add` need `pro-space-rx-qual`;
//! `detect`, `vectors`, `tolerance` and `rate` need `pro-space-rx-see`; `inject`, `db select`,
//! `db schema`, `sample` and `verify` need none.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::license;
use crate::space_rx::{self, exposure, obc, qual, see, see_rate};

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::entitlement;
use super::{flag, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro space-rx <command>\n\
    exposure <scenario.toml> --out <dir> [--signer <name> --key-file <file>] [--utc <iso>]\n\
        Ground-interference envelope seen by the spacecraft receiver: passes, revolutions, test points (Pro)\n\
    score --campaign <campaign.toml> --mission <mission.toml> --out <dir> --signer <name> --key-file <file>\n\
        Receiver scorecard and qualification process template, signed (Pro)\n\
    db add --db <db.json> --scorecard <dir> [--trust <key hex>[,...]]   Add a scorecard to the selection database (Pro)\n\
    db select --db <db.json> [--min-tid <krad>] [--min-js <dB>]          Receivers whose test-backed figures reach the levels (free)\n\
    db schema                                                            The database JSON Schema (free)\n\
    detect --trace <trace.csv> [--params <params.toml>] --out <dir> --signer <name> --key-file <file> [--truth <truth.json>]\n\
        Radiation-fault signatures in a receiver output trace, signed (Pro)\n\
    inject --trace <clean.csv> --events <inject.toml> --out <dir>         Fixture with injected signatures and ground truth (free)\n\
    vectors --trace <trace.csv> --out <dir>                              Receiver-to-OBC corrupt-data vectors and replay stream (Pro)\n\
    tolerance --vectors <vectors.json> --log <obc-log.csv> --out <dir> --signer <name> --key-file <file>\n\
        Score an OBC's accept/flag/reject log against the vectors, signed; exits 1 on a vector not met (Pro)\n\
    rate <model.toml> [--out <dir>]                                      SEE rate forecast (Pro)\n\
    sample <campaign|mission|exposure|trace|inject|rate> [--out <file>]  SYNTHETIC sample inputs (free)\n\
    verify <dir> --trust <key hex>[,...]                                 Check a signed output directory (free)";

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

fn need(args: &[String], name: &str) -> Result<String, String> {
    flag(args, name).ok_or_else(|| format!("needs {name}"))
}

fn out_dir(args: &[String]) -> Result<PathBuf, String> {
    let out = PathBuf::from(need(args, "--out")?);
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

fn json(v: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
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

fn sign(dir: &Path, args: &[String]) -> Result<(), String> {
    let (seed, signer, utc) = signing(args)?;
    crate::signed_dossier::sign(dir, &seed, &signer, &utc).map_err(|e| e.to_string())?;
    Ok(())
}

fn first_positional(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

pub fn run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_inner(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("exposure") => {
            gate(args, space_rx::FEATURE_EXPOSURE)?;
            let p = first_positional(args).ok_or(USAGE)?;
            let s = exposure::Scenario::parse(&read(p)?)?;
            let base = Path::new(p).parent().unwrap_or(Path::new("."));
            let doc = exposure::run(&s, base)?;
            let signed = flag(args, "--signer").is_some();
            if signed {
                signing(args)?;
            }
            let out = out_dir(args)?;
            write(&out, "result.json", &json(&doc))?;
            write(&out, "passes.csv", &exposure::passes_csv(&doc))?;
            let md = exposure::summary_md(&doc);
            write(&out, "summary.md", &md)?;
            if signed {
                sign(&out, args)?;
            }
            print!("{md}");
            Ok(ExitCode::SUCCESS)
        }
        Some("score") => {
            gate(args, space_rx::FEATURE_QUAL)?;
            let csrc = read(&need(args, "--campaign")?)?;
            let msrc = read(&need(args, "--mission")?)?;
            let c = qual::Campaign::parse(&csrc)?;
            let m = qual::Mission::parse(&msrc)?;
            signing(args)?;
            let sc = qual::score(&c, &m);
            let out = out_dir(args)?;
            write(&out, "scorecard.json", &json(&sc))?;
            let md = qual::scorecard_md(&sc);
            write(&out, "scorecard.md", &md)?;
            write(&out, "template.md", &qual::template_md(&c, &sc))?;
            write(&out, "campaign.toml", &csrc)?;
            write(&out, "mission.toml", &msrc)?;
            sign(&out, args)?;
            print!("{md}");
            Ok(ExitCode::SUCCESS)
        }
        Some("db") => match args.get(1).map(String::as_str) {
            Some("schema") => {
                println!("{}", json(&qual::db_schema()));
                Ok(ExitCode::SUCCESS)
            }
            Some("add") => {
                gate(args, space_rx::FEATURE_QUAL)?;
                let db_path = need(args, "--db")?;
                let dir = PathBuf::from(need(args, "--scorecard")?);
                let trust = keys(args, "--trust")?;
                if !trust.is_empty() {
                    match crate::signed_dossier::verify(&dir, &trust) {
                        Ok(r) if r.passed() => {}
                        _ => return Err(format!("{}: signature not trusted", dir.display())),
                    }
                }
                let sc: qual::Scorecard = serde_json::from_str(&read(
                    dir.join("scorecard.json").to_str().unwrap_or_default(),
                )?)
                .map_err(|e| format!("scorecard.json: {e}"))?;
                let mut db = if Path::new(&db_path).exists() {
                    qual::Db::parse(&read(&db_path)?)?
                } else {
                    qual::Db::new()
                };
                db.add(&sc);
                std::fs::write(&db_path, json(&db)).map_err(|e| format!("{db_path}: {e}"))?;
                println!(
                    "{} entries in {db_path} (added {} {} campaign {})",
                    db.entries.len(),
                    sc.receiver.vendor,
                    sc.receiver.model,
                    sc.campaign_id
                );
                Ok(ExitCode::SUCCESS)
            }
            Some("select") => {
                let db = qual::Db::parse(&read(&need(args, "--db")?)?)?;
                let num = |n: &str| -> Result<Option<f64>, String> {
                    flag(args, n)
                        .map(|v| {
                            v.parse::<f64>()
                                .map_err(|_| format!("{n} {v}: not a number"))
                        })
                        .transpose()
                };
                let sel = db.select(num("--min-tid")?, num("--min-js")?);
                for e in &sel {
                    println!(
                        "{} {} {} ({}; {}): TID {} krad, J/S held {} dB, LET tested {}",
                        e.receiver.vendor,
                        e.receiver.model,
                        e.receiver.firmware,
                        e.receiver.class,
                        e.data_class,
                        e.specs
                            .tid_functional_krad
                            .map_or("-".into(), |v| v.to_string()),
                        e.specs.js_held_db.map_or("-".into(), |v| v.to_string()),
                        e.specs.let_max_tested.map_or("-".into(), |v| v.to_string()),
                    );
                }
                println!("{} of {} entries", sel.len(), db.entries.len());
                Ok(ExitCode::SUCCESS)
            }
            _ => {
                println!("{USAGE}");
                Ok(ExitCode::SUCCESS)
            }
        },
        Some("detect") => {
            gate(args, space_rx::FEATURE_SEE)?;
            let tp = need(args, "--trace")?;
            let text = read(&tp)?;
            let trace = see::parse_trace(&text)?;
            let params = match flag(args, "--params") {
                Some(f) => toml::from_str(&read(&f)?).map_err(|e| format!("{f}: {e}"))?,
                None => see::Params::default(),
            };
            signing(args)?;
            let events = see::detect(&trace, &params);
            let mut doc = see::result(
                &trace,
                &params,
                &events,
                &space_rx::sha256_hex(text.as_bytes()),
            );
            let mut missed_any = false;
            if let Some(tf) = flag(args, "--truth") {
                let truth: serde_json::Value =
                    serde_json::from_str(&read(&tf)?).map_err(|e| format!("{tf}: {e}"))?;
                let (found, missed, unexpected) = see::match_truth(&truth, &events);
                missed_any = !missed.is_empty();
                doc["truth"] = serde_json::json!({
                    "found": found, "missed": missed, "unexpected": unexpected,
                });
            }
            let out = out_dir(args)?;
            write(&out, "see.json", &json(&doc))?;
            std::fs::copy(&tp, out.join("trace.csv")).map_err(|e| format!("{tp}: {e}"))?;
            sign(&out, args)?;
            println!(
                "{} epochs: {} events (SEU {}, SEFI {}, SEL {}, other {}) -> {}",
                trace.len(),
                events.len(),
                doc["counts"]["SEU"],
                doc["counts"]["SEFI"],
                doc["counts"]["SEL"],
                doc["counts"]["other"],
                out.display()
            );
            for e in &events {
                println!(
                    "  {:>6} {:<22} {:<9} {}",
                    e.epoch, e.kind, e.class, e.detail
                );
            }
            if !doc["truth"].is_null() {
                println!("  ground truth: {}", doc["truth"]);
            }
            Ok(if missed_any {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Some("inject") => {
            let trace = see::parse_trace(&read(&need(args, "--trace")?)?)?;
            let f = see::InjectFile::parse(&read(&need(args, "--events")?)?)?;
            let (t, truth) = see::inject(&trace, &f)?;
            let out = PathBuf::from(need(args, "--out")?);
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            write(&out, "trace.csv", &see::write_trace(&t))?;
            write(&out, "truth.json", &json(&truth))?;
            println!(
                "SYNTHETIC fixture: {} epochs, {} injected signatures -> {}",
                t.len(),
                truth["truth"].as_array().map_or(0, |v| v.len()),
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("vectors") => {
            gate(args, space_rx::FEATURE_SEE)?;
            let tp = need(args, "--trace")?;
            let text = read(&tp)?;
            let trace = see::parse_trace(&text)?;
            let v = obc::build(&trace)?;
            let limits = obc::Limits::default();
            let refs = obc::reference_actions(&v, &limits);
            let bad: Vec<&String> = v
                .iter()
                .filter(|x| !x.allowed.contains(&refs[&x.id]))
                .map(|x| &x.id)
                .collect();
            if !bad.is_empty() {
                return Err(format!("reference parser disagrees on {bad:?}"));
            }
            let out = PathBuf::from(need(args, "--out")?);
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            write(
                &out,
                "vectors.json",
                &json(&obc::vectors_doc(
                    &v,
                    &space_rx::sha256_hex(text.as_bytes()),
                    &limits,
                )),
            )?;
            std::fs::write(out.join("stream.nmea"), obc::stream_bytes(&v))
                .map_err(|e| format!("{}: {e}", out.display()))?;
            let mut csv = String::from("id,action\n");
            for (id, a) in &refs {
                csv.push_str(&format!("{id},{a}\n"));
            }
            write(&out, "reference-actions.csv", &csv)?;
            println!(
                "{} vectors ({} corrupt) -> {}",
                v.len(),
                v.iter().filter(|x| x.class != "nominal").count(),
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("tolerance") => {
            gate(args, space_rx::FEATURE_SEE)?;
            let vdoc: serde_json::Value = serde_json::from_str(&read(&need(args, "--vectors")?)?)
                .map_err(|e| format!("vectors: {e}"))?;
            let v: Vec<obc::Vector> = serde_json::from_value(vdoc["vectors"].clone())
                .map_err(|e| format!("vectors: {e}"))?;
            let lsrc = read(&need(args, "--log")?)?;
            let log = obc::parse_log(&lsrc)?;
            signing(args)?;
            let sc = obc::score(&v, &log);
            let out = out_dir(args)?;
            write(&out, "tolerance.json", &json(&sc))?;
            write(&out, "obc-log.csv", &lsrc)?;
            sign(&out, args)?;
            println!(
                "met {}, not met {}, not assessed {} -> {}",
                sc["met"],
                sc["not_met"],
                sc["not_assessed"],
                out.display()
            );
            for c in sc["classes"].as_array().into_iter().flatten() {
                println!(
                    "  {:<22} {}",
                    c["class"].as_str().unwrap_or(""),
                    c["verdict"].as_str().unwrap_or("")
                );
            }
            Ok(if sc["not_met"].as_u64().unwrap_or(0) > 0 {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Some("rate") => {
            gate(args, space_rx::FEATURE_SEE)?;
            let m = see_rate::Model::parse(&read(first_positional(args).ok_or(USAGE)?)?)?;
            let doc = see_rate::run(&m);
            let md = see_rate::markdown(&doc);
            if let Some(o) = flag(args, "--out") {
                let out = PathBuf::from(o);
                std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
                write(&out, "see-rate.json", &json(&doc))?;
                write(&out, "see-rate.md", &md)?;
            }
            print!("{md}");
            Ok(ExitCode::SUCCESS)
        }
        Some("sample") => {
            let what = first_positional(args).ok_or(USAGE)?;
            let text = match what.as_str() {
                "campaign" => qual::SAMPLE_CAMPAIGN.to_string(),
                "mission" => qual::SAMPLE_MISSION.to_string(),
                "exposure" => exposure::SAMPLE.to_string(),
                "trace" => see::write_trace(&see::synth_trace(600, 1.0, 550.0, 97.6, 7)),
                "inject" => see::SAMPLE_INJECT.to_string(),
                "rate" => see_rate::SAMPLE.to_string(),
                other => return Err(format!("unknown sample '{other}'")),
            };
            match flag(args, "--out") {
                Some(f) => {
                    std::fs::write(&f, text).map_err(|e| format!("{f}: {e}"))?;
                    eprintln!("[space-rx] wrote SYNTHETIC sample {f}");
                }
                None => print!("{text}"),
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("verify") => Ok(super::scorecard_cmd::verify_dir(args)),
        _ => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
    }
}
