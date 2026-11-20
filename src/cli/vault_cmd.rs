// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro vault ...` (research 2026-11-20 gaps, B2504 first slice; docs/EVIDENCE-VAULT.md).
//! Every command that changes or reads the vault needs the Enterprise feature; `import` of an
//! export is free, so a customer can always take their evidence out and back.

use std::path::Path;
use std::process::ExitCode;

use crate::vault;

use super::flag;
use super::grid_cmd::gate;

pub const VAULT_USAGE: &str = "usage: kshana-pro vault <command> <vault-dir> ...\n\
    init <vault>                                                     Create an empty vault\n\
    ingest <vault> <signed-dir> [--tag <t>] [--retain-years <n>] [--trust <key-hex>]   Copy in a signed evidence directory after checking its signature\n\
    list <vault>                                                     The packs held\n\
    hold <vault> <id> --reason <text>                                Legal hold: blocks deletion\n\
    release <vault> <id> --reason <text>                             Release a legal hold\n\
    delete <vault> <id>                                              Delete a pack (refused under hold or before its retention date)\n\
    trend <vault> [--html <out>]                                     Packs per month per family\n\
    calendar <vault> <obligations.toml>                              Next due date and latest serving pack per declared obligation\n\
    submission <vault> --title <t> --packs <id,id,...> [--out <file.md>]   Submission skeleton citing pack ids\n\
    verify <vault>                                                   Check the hash-chained ledger\n\
    export <vault> <out-dir>                                         Full open export with a SHA-256 manifest\n\
    import <export-dir> <new-vault>                                  Import an export, checking every digest (no licence needed)\n\
    (all but import: Enterprise feature pro-evidence-vault; add [--license <token>] [--dev] [--utc <iso>])";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro vault ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let pos: Vec<&String> = args[1.min(args.len())..]
        .iter()
        .scan(false, |skip, a| {
            let r = if *skip {
                *skip = false;
                None
            } else if a.starts_with("--") {
                *skip = !matches!(a.as_str(), "--dev");
                None
            } else {
                Some(a)
            };
            Some(r)
        })
        .flatten()
        .collect();
    let Some(v) = pos.first().map(|s| Path::new(s.as_str())) else {
        return fail(VAULT_USAGE);
    };
    let utc = flag(args, "--utc").unwrap_or_else(super::utc_iso8601_now);
    if sub != "import" {
        if let Err(e) = gate(args, vault::FEATURE) {
            return fail(e);
        }
    }
    let arg2 = || {
        pos.get(1)
            .map(|s| s.as_str())
            .ok_or_else(|| VAULT_USAGE.to_string())
    };
    let r: Result<String, String> = (|| match sub {
        "init" => vault::init(v, &utc).map(|_| format!("[vault] created {}", v.display())),
        "ingest" => {
            let tags: Vec<String> = flag(args, "--tag").into_iter().collect();
            let years: u32 = flag(args, "--retain-years")
                .map(|y| y.parse().map_err(|_| "bad --retain-years".to_string()))
                .transpose()?
                .unwrap_or(7);
            let trust = match flag(args, "--trust") {
                Some(t) => vec![crate::license::parse_key_hex(&t).map_err(|e| e.to_string())?],
                None => Vec::new(),
            };
            let e = vault::ingest(v, Path::new(arg2()?), &tags, years, &trust, &utc)?;
            Ok(format!(
                "[vault] ingested {} ({})",
                e.id,
                e.format.unwrap_or_default()
            ))
        }
        "list" => {
            for e in vault::list(v)? {
                println!(
                    "{}  {:<40} signed {}  {}",
                    &e.id[..16],
                    e.format.unwrap_or_default(),
                    e.signed_utc,
                    e.legal_hold
                        .map(|r| format!("HOLD: {r}"))
                        .unwrap_or_default()
                );
            }
            Ok("[vault] listed".into())
        }
        "hold" | "release" => {
            let reason = flag(args, "--reason").ok_or("--reason is needed")?;
            if sub == "hold" {
                vault::hold(v, arg2()?, &reason, &utc)?;
            } else {
                vault::release(v, arg2()?, &reason, &utc)?;
            }
            Ok(format!("[vault] {sub} recorded"))
        }
        "delete" => vault::delete(v, arg2()?, &utc).map(|_| "[vault] deleted".into()),
        "trend" => {
            let t = vault::trend(v)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&t).map_err(|e| e.to_string())?
            );
            if let Some(h) = flag(args, "--html") {
                std::fs::write(&h, vault::trend_html(&t)).map_err(|e| format!("{h}: {e}"))?;
            }
            Ok(format!("[vault] {} month(s)", t.len()))
        }
        "calendar" => {
            let p = arg2()?;
            #[derive(serde::Deserialize)]
            struct O {
                obligations: Vec<vault::Obligation>,
            }
            let o: O =
                toml::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?)
                    .map_err(|e| format!("{p}: {e}"))?;
            let c = vault::calendar(v, &o.obligations, &utc)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?
            );
            Ok(format!("[vault] {} obligation(s)", c.len()))
        }
        "submission" => {
            let title = flag(args, "--title").ok_or("--title is needed")?;
            let ids: Vec<String> = flag(args, "--packs")
                .ok_or("--packs is needed")?
                .split(',')
                .map(str::to_string)
                .collect();
            let md = vault::submission(v, &title, &ids)?;
            match flag(args, "--out") {
                Some(o) => std::fs::write(&o, md).map_err(|e| format!("{o}: {e}"))?,
                None => print!("{md}"),
            }
            Ok("[vault] submission skeleton written".into())
        }
        "verify" => {
            let p = vault::check_ledger(v)?;
            if p.is_empty() {
                Ok("[vault] ledger verifies".into())
            } else {
                Err(p.join("; "))
            }
        }
        "export" => {
            let m = vault::export(v, Path::new(arg2()?), &utc)?;
            Ok(format!("[vault] exported {} file(s)", m.files.len()))
        }
        "import" => {
            let n = vault::import(v, Path::new(arg2()?))?;
            Ok(format!("[vault] imported {n} file(s)"))
        }
        _ => Err(VAULT_USAGE.into()),
    })();
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
