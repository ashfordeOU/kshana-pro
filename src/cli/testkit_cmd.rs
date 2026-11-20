// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro testkit <...>`: the test-as-code kit ([`crate::testkit`]). `compile`, `run`,
//! `from-incident`, `from-library`, `fidelity` and `packs export` need a licence that unlocks
//! `pro-test-kit`; `diff` (and `run --baseline`) need `pro-test-kit-regression`
//! (Enterprise); `validate`, `packs`, `show`, `sample` and `verify` need none.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::license;
use crate::pics::log::DeviceLog;
use crate::testkit::{self, compile, diff, fidelity, incident, packs, run, spec::Spec};

use super::forensics_cmd::{keys, read_seed};
use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro testkit <command>\n\
    validate <scenario> [--adapter <name>|all] [--profile <profile.toml>] [--json]     What each simulator adapter can and cannot express (free)\n\
    compile <scenario> --adapter <skydel|spirent|rs-smbv|labsat|gps-sdr-sim> --out <dir> [--profile <profile.toml>]   Native scenario files and COMPILE.json (Pro)\n\
    run <scenario|dir|pack:name>... --build <id> --out <dir> --signer <name> --key-file <file>\n\
        [--logs <dir> [--t0 <s>]] | [--adapter <name> [--profile <profile.toml>] --sim <host:port> --receiver <host:port> [--sim-trajectory <path>] [--no-pace] [--tail <s>]]\n\
        [--capture <capture.toml>] [--receiver-name <name>] [--baseline <build dir>] [--utc <iso>]\n\
        Score the receiver per scenario; one signed verdict per scenario per build (Pro; --baseline: Enterprise)\n\
    diff <build dir A> <build dir B> [--trust <key hex>[,...]] [--out <diff.json>] [--markdown <file>]\n\
        Regression diff between two builds; exits 1 on a regression (Enterprise)\n\
    from-incident --log <file> --incident <incident.toml> --out <scenario.toml> [--input nmea|json] [--observed <log.json>]\n\
        A field log and an incident description as a reproducible regression scenario (Pro)\n\
    from-library <threat-library id> --out <scenario.toml>   A threat-library scenario as a neutral scenario (Pro)\n\
    fidelity <capture.toml> --scenario <scenario> [--json]   Replay fidelity limits of a record-and-replay capture (Pro)\n\
    packs [list] | packs export <pack> --out <dir>           Scenario packs (list free; export Pro)\n\
    show <pack/id>                                          One pack scenario as TOML (free)\n\
    sample <scenario> --behaviour good|bad [--out <log.json>]   A SYNTHETIC receiver log, to try the runner (free)\n\
    verify <dir> --trust <key hex>[,...]                     Check a verdict or build directory's signature (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

const WITH_VALUE: [&str; 25] = [
    "--adapter",
    "--out",
    "--build",
    "--signer",
    "--key-file",
    "--logs",
    "--t0",
    "--sim",
    "--receiver",
    "--sim-trajectory",
    "--tail",
    "--capture",
    "--receiver-name",
    "--baseline",
    "--utc",
    "--trust",
    "--markdown",
    "--log",
    "--incident",
    "--input",
    "--observed",
    "--scenario",
    "--behaviour",
    "--license",
    "--profile",
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

/// The `--profile <file.toml>` override (a lab-confirmed command profile), if given.
fn custom_profile(
    args: &[String],
) -> Result<Option<kshana_eloran::sync::profile::CommandProfile>, String> {
    flag(args, "--profile")
        .map(|p| kshana_eloran::sync::profile::CommandProfile::load(Path::new(&p)))
        .transpose()
}

/// A scenario from a file or a pack (`<pack>/<id>` or a bare pack id).
fn load_spec(name: &str) -> Result<Spec, String> {
    if Path::new(name).is_file() {
        return Spec::load(Path::new(name));
    }
    match packs::find(name) {
        Some((_, src)) => Spec::parse(src),
        None => Err(format!("{name}: not a file or a pack scenario")),
    }
}

fn emit(out: Option<String>, text: &str) -> ExitCode {
    match out {
        Some(p) => match std::fs::write(&p, text) {
            Ok(()) => {
                eprintln!("[testkit] wrote {p}");
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

pub fn run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(c) => c,
        Err(e) => fail(e),
    }
}

fn run_inner(args: &[String]) -> Result<ExitCode, String> {
    let pos = positional(args);
    match args.first().map(String::as_str) {
        Some("validate") => {
            let spec = load_spec(pos.first().ok_or(USAGE)?)?;
            let which = flag(args, "--adapter").unwrap_or_else(|| "all".into());
            let names: Vec<&str> = if which == "all" {
                compile::ADAPTERS.to_vec()
            } else {
                vec![which.as_str()]
            };
            let cp = custom_profile(args)?;
            let vs = names
                .iter()
                .map(|a| compile::validate_with(&spec, a, cp.as_ref()))
                .collect::<Result<Vec<_>, _>>()?;
            if has_flag(args, "--json") {
                return Ok(emit(None, &serde_json::to_string_pretty(&vs).unwrap()));
            }
            println!(
                "{} v{} — scenario valid ({})",
                spec.id,
                spec.version,
                testkit::LABEL_SHORT
            );
            for v in &vs {
                println!(
                    "  {:<12} {:<22} {}",
                    v.adapter,
                    if v.expressible {
                        "expressible"
                    } else {
                        "NOT fully expressible"
                    },
                    serde_json::to_string(&v.counts).unwrap()
                );
                for f in v
                    .findings
                    .iter()
                    .filter(|f| f.support != compile::Support::Native)
                {
                    println!(
                        "      {:<13} {:<18} {}",
                        serde_json::to_value(f.support).unwrap().as_str().unwrap(),
                        f.feature,
                        f.detail
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("compile") => {
            gate(args, testkit::FEATURE)?;
            let spec = load_spec(pos.first().ok_or(USAGE)?)?;
            let (Some(a), Some(out)) = (flag(args, "--adapter"), flag(args, "--out")) else {
                return Err("compile needs --adapter and --out".into());
            };
            let c = compile::compile_with(&spec, &a, custom_profile(args)?.as_ref())?;
            compile::write(&c, Path::new(&out))?;
            eprintln!(
                "[testkit] {} for {}: {} files in {}; {}",
                c.scenario,
                c.adapter,
                c.file_names.len(),
                out,
                if c.validation.expressible {
                    "fully expressible".to_string()
                } else {
                    format!(
                        "NOT fully expressible: see COMPILE.json ({} unsupported)",
                        c.validation.counts.get("unsupported").copied().unwrap_or(0)
                    )
                }
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("run") => {
            gate(args, testkit::FEATURE)?;
            if flag(args, "--baseline").is_some() {
                gate(args, testkit::FEATURE_REGRESSION)?;
            }
            if pos.is_empty() {
                return Err(USAGE.into());
            }
            let set = run::load_set(&pos)?;
            let (Some(build), Some(out)) = (flag(args, "--build"), flag(args, "--out")) else {
                return Err("run needs --build and --out".into());
            };
            let (seed, signer, utc) = signing(args)?;
            let capture = match flag(args, "--capture") {
                Some(p) => Some(fidelity::Capture::parse(
                    &std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?,
                )?),
                None => None,
            };
            let cfg = run::RunConfig {
                build: build.clone(),
                receiver: flag(args, "--receiver-name")
                    .unwrap_or_else(|| "receiver under test".into()),
                capture,
            };
            let out = PathBuf::from(out);
            let summary = if let Some(dir) = flag(args, "--logs") {
                let t0 = flag(args, "--t0")
                    .map(|v| v.parse::<f64>().map_err(|e| format!("--t0: {e}")))
                    .transpose()?
                    .unwrap_or(0.0);
                run::run_offline(
                    &set,
                    &run::Source::Logs {
                        dir: dir.into(),
                        t0_s: t0,
                    },
                    &cfg,
                    &out,
                    &seed,
                    &signer,
                    &utc,
                )?
            } else if let Some(a) = flag(args, "--adapter") {
                if a == "gps-sdr-sim" {
                    return Err("gps-sdr-sim writes an I/Q file and has no live control: compile it, replay the file conducted, record the receiver, and run with --logs".into());
                }
                let profile = match custom_profile(args)? {
                    Some(p) => p,
                    None => compile::adapter_profile(&a)?,
                };
                let (Some(sim), Some(rx)) = (flag(args, "--sim"), flag(args, "--receiver")) else {
                    return Err(
                        "a live run needs --sim <host:port> and --receiver <host:port>".into(),
                    );
                };
                let live = run::Live {
                    profile,
                    sim_addr: sim,
                    receiver_addr: rx,
                    sim_trajectory_path: flag(args, "--sim-trajectory"),
                    real_time: !has_flag(args, "--no-pace"),
                    tail_s: flag(args, "--tail")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(10.0),
                };
                let bd = out.join(&build);
                if bd.join("summary.json").exists() {
                    return Err(format!("{} already holds a build", bd.display()));
                }
                let mut vs = Vec::new();
                for spec in &set {
                    let l = run::run_live(spec, &live)?;
                    let v = run::verdict_for(spec, Some(&l), &cfg)?;
                    run::write_verdict(
                        &bd.join(&spec.id),
                        spec,
                        &v,
                        Some(&l),
                        &seed,
                        &signer,
                        &utc,
                    )?;
                    vs.push(v);
                }
                run::finish_build(&bd, &cfg, &vs, &seed, &signer, &utc)?
            } else {
                return Err("run needs --logs <dir> or --adapter with --sim and --receiver".into());
            };
            eprintln!(
                "[testkit] build {}: {} pass, {} fail, {} incomplete, {} not run; signed verdicts in {}",
                summary.build, summary.pass, summary.fail, summary.incomplete, summary.not_run, out.join(&build).display()
            );
            if let Some(b) = flag(args, "--baseline") {
                let d = diff::diff(Path::new(&b), &out.join(&build), &[])?;
                std::fs::write(
                    out.join(&build).with_extension("diff.json"),
                    serde_json::to_string_pretty(&d).unwrap(),
                )
                .map_err(|e| e.to_string())?;
                eprint!("{}", diff::markdown(&d));
                if d.regressions > 0 {
                    return Ok(ExitCode::FAILURE);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("diff") => {
            gate(args, testkit::FEATURE_REGRESSION)?;
            let (Some(a), Some(b)) = (pos.first(), pos.get(1)) else {
                return Err(USAGE.into());
            };
            let trust = keys(args, "--trust")?;
            let d = diff::diff(Path::new(a), Path::new(b), &trust)?;
            if let Some(m) = flag(args, "--markdown") {
                std::fs::write(&m, diff::markdown(&d)).map_err(|e| format!("{m}: {e}"))?;
            }
            let code = emit(
                flag(args, "--out"),
                &serde_json::to_string_pretty(&d).unwrap(),
            );
            eprintln!(
                "[testkit] {} → {}: {} regression(s), {} fixed",
                d.from_build, d.to_build, d.regressions, d.fixed
            );
            if d.regressions > 0 || !d.signature_problems.is_empty() {
                return Ok(ExitCode::FAILURE);
            }
            Ok(code)
        }
        Some("from-incident") => {
            gate(args, testkit::FEATURE)?;
            let (Some(lp), Some(ip), Some(out)) = (
                flag(args, "--log"),
                flag(args, "--incident"),
                flag(args, "--out"),
            ) else {
                return Err("from-incident needs --log, --incident and --out".into());
            };
            let inc = incident::Incident::parse(
                &std::fs::read_to_string(&ip).map_err(|e| format!("{ip}: {e}"))?,
            )?;
            let text = std::fs::read_to_string(&lp).map_err(|e| format!("{lp}: {e}"))?;
            let l = match flag(args, "--input").as_deref().unwrap_or("nmea") {
                "nmea" => crate::pics::log::from_nmea(&text, &inc.id, 0.0)?,
                "json" => DeviceLog::parse(&text)?,
                o => return Err(format!("unknown --input '{o}' (nmea or json)")),
            };
            let c = incident::convert(&inc, &l)?;
            std::fs::write(&out, c.spec.to_toml()).map_err(|e| format!("{out}: {e}"))?;
            let obs = flag(args, "--observed").unwrap_or_else(|| {
                Path::new(&out)
                    .with_file_name(format!("{}.json", inc.id))
                    .display()
                    .to_string()
            });
            std::fs::write(&obs, c.observed.to_json()).map_err(|e| format!("{obs}: {e}"))?;
            eprintln!("[testkit] scenario {} written to {out}; the field log on the scenario clock in {obs} (run it with --logs <its dir>)", inc.id);
            Ok(ExitCode::SUCCESS)
        }
        Some("from-library") => {
            gate(args, testkit::FEATURE)?;
            let id = pos.first().ok_or(USAGE)?;
            let s = testkit::library::spec(id)?;
            Ok(emit(flag(args, "--out"), &s.to_toml()))
        }
        Some("fidelity") => {
            gate(args, testkit::FEATURE)?;
            let cp = pos.first().ok_or(USAGE)?;
            let cap = fidelity::Capture::parse(
                &std::fs::read_to_string(cp).map_err(|e| format!("{cp}: {e}"))?,
            )?;
            let spec = load_spec(&flag(args, "--scenario").ok_or("fidelity needs --scenario")?)?
                .resolve()?;
            let f = fidelity::assess(&cap, &spec);
            if has_flag(args, "--json") {
                return Ok(emit(None, &serde_json::to_string_pretty(&f).unwrap()));
            }
            println!("{}", f.statement);
            for l in &f.limits {
                println!(
                    "  {:<15} {:<13} score {:.2}  {}",
                    l.id,
                    serde_json::to_value(l.severity).unwrap().as_str().unwrap(),
                    l.score,
                    l.detail
                );
            }
            println!("{}", f.label);
            Ok(ExitCode::SUCCESS)
        }
        Some("packs") => match pos.first().map(String::as_str) {
            None | Some("list") => {
                for (p, d, items) in packs::PACKS {
                    println!("{p} — {d}");
                    for (id, src) in *items {
                        let t = Spec::parse(src)
                            .map(|s| s.title)
                            .unwrap_or_else(|e| format!("INVALID: {e}"));
                        println!("    {id:<38} {t}");
                    }
                }
                println!("{}", testkit::LABEL_SHORT);
                Ok(ExitCode::SUCCESS)
            }
            Some("export") => {
                gate(args, testkit::FEATURE)?;
                let p = pos.get(1).ok_or("packs export needs a pack name")?;
                let out = flag(args, "--out").ok_or("packs export needs --out")?;
                std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;
                for (id, src) in packs::pack(p)? {
                    std::fs::write(Path::new(&out).join(format!("{id}.toml")), src)
                        .map_err(|e| format!("{out}: {e}"))?;
                }
                eprintln!("[testkit] pack {p} written to {out}");
                Ok(ExitCode::SUCCESS)
            }
            Some(o) => Err(format!("unknown packs command '{o}'")),
        },
        Some("show") => {
            let n = pos.first().ok_or(USAGE)?;
            let (_, src) = packs::find(n).ok_or_else(|| format!("unknown pack scenario '{n}'"))?;
            print!("{src}");
            Ok(ExitCode::SUCCESS)
        }
        Some("sample") => {
            let spec = load_spec(pos.first().ok_or(USAGE)?)?.resolve()?;
            let good = match flag(args, "--behaviour").as_deref() {
                Some("good") => true,
                Some("bad") => false,
                _ => return Err("sample needs --behaviour good or bad".into()),
            };
            Ok(emit(
                flag(args, "--out"),
                &run::synth_log(&spec, good).to_json(),
            ))
        }
        Some("verify") => {
            let dir = pos.first().ok_or(USAGE)?;
            let trust = keys(args, "--trust")?;
            let v =
                crate::signed_dossier::verify(Path::new(dir), &trust).map_err(|e| e.to_string())?;
            println!(
                "signer: {} ({}), {} files, signature {}, key {}",
                v.signer,
                v.key_id,
                v.files,
                if v.signature_ok { "valid" } else { "INVALID" },
                match v.trusted {
                    Some(true) => "trusted",
                    Some(false) => "NOT trusted",
                    None => "not pinned (give --trust)",
                }
            );
            for p in &v.problems {
                println!("  problem: {p}");
            }
            Ok(if v.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        _ => Err(USAGE.into()),
    }
}
