// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro testkit <...>`: the test-as-code kit ([`crate::testkit`]). `compile`,
//! `from-library` and `packs export` need a licence that unlocks `pro-test-kit`; `validate`,
//! `packs` and `show` need none.

use std::path::Path;
use std::process::ExitCode;

use crate::license;
use crate::testkit::{self, compile, packs, spec::Spec};

use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix};

pub const USAGE: &str = "usage: kshana-pro testkit <command>\n\
    validate <scenario> [--adapter <name>|all] [--profile <profile.toml>] [--json]     What each simulator adapter can and cannot express (free)\n\
    compile <scenario> --adapter <skydel|spirent|rs-smbv|labsat|gps-sdr-sim> --out <dir> [--profile <profile.toml>]   Native scenario files and COMPILE.json (Pro)\n\
    from-library <threat-library id> --out <scenario.toml>   A threat-library scenario as a neutral scenario (Pro)\n\
    packs [list] | packs export <pack> --out <dir>           Scenario packs (list free; export Pro)\n\
    show <pack/id>                                          One pack scenario as TOML (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

const WITH_VALUE: [&str; 4] = ["--adapter", "--out", "--license", "--profile"];

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
        Some("from-library") => {
            gate(args, testkit::FEATURE)?;
            let id = pos.first().ok_or(USAGE)?;
            let s = testkit::library::spec(id)?;
            Ok(emit(flag(args, "--out"), &s.to_toml()))
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
        _ => Err(USAGE.into()),
    }
}
