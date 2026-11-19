// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro timing <assess|console|metrics|history|scorecard|compliance|audit>` (roadmap 0.42 T1-T3).
//! `assess` needs `pro-timing-holdover`; `console`, `metrics` and `history` need
//! `pro-timing-fleet`; `audit` needs `pro-timing-audit`. Verifying an audit report needs no
//! licence: `kshana-pro dossier verify <dir> --trust <key>`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::license::{self, Entitlement, Product};
use crate::timing::{self, audit, console, evidence, fleet, metrics};

use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro timing <command>\n\
    assess <fleet.toml> [--evidence <dir>] [--utc <iso>] [--json <out.json>] [--license <token>] [--dev]   Holdover and time to breach of every clock; append each clock's evidence entry (Pro)\n\
    console <fleet.toml> [--evidence <dir>] --out <page.html> [--license <token>] [--dev]   The fleet page with each clock's evidence history (Pro)\n\
    metrics <fleet.toml> [--out <file.prom>] [--license <token>] [--dev]   The fleet in the Prometheus text format, for the Grafana dashboard (Pro)\n\
    history verify <dir>                                                     Check every clock's evidence chain (no licence needed)\n\
    scorecard <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Timing scorecard under threat library spoofing scenarios, signed (Pro, T4); verify with `kshana-pro dossier verify` (free)\n\
    compliance list | compliance <mifid2-rts25|dora-rts-2024-1774|eu-cer|india-ist-draft> <inputs.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Signed annual timing evidence pack, requirement -> evidence -> verdict (Pro); verify with `kshana-pro dossier verify` (free)\n\
    audit <fleet.toml> --profile <rts25-traceability|nist-ir-8323-profile> --out <dir> --signer <name> --key-file <file> [--evidence <dir>] [--operator <operator.toml>] [--utc <iso>] [--license <token>] [--dev]   Signed audit report mapped to the profile (Pro); verify with `kshana-pro dossier verify` (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn entitlement(args: &[String], now: i64) -> Result<Entitlement, String> {
    if has_flag(args, "--dev") && cfg!(debug_assertions) {
        return Ok(Entitlement {
            licensee: "(dev)".into(),
            tier: license::Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
            arksa: false,
        });
    }
    match flag(args, "--license") {
        Some(t) => license_cmd::verify_token_for(&t, now, Product::Cli)
            .map_err(|e| format!("license invalid: {e}")),
        None => Ok(license_cmd::stored_or_evaluation(now, Product::Cli)),
    }
}

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

/// Read and validate a fleet file; source paths become relative to its directory.
pub fn load_fleet(path: &str) -> Result<fleet::FleetSpec, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let mut spec = fleet::FleetSpec::parse(&text).map_err(|e| e.to_string())?;
    let base = Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    spec.resolve_paths(&base);
    Ok(spec)
}

fn histories(
    dir: Option<&str>,
    a: &fleet::FleetAssessment,
) -> Result<Vec<(String, Vec<evidence::Entry>)>, String> {
    let Some(d) = dir else {
        return Ok(a
            .clocks
            .iter()
            .map(|c| (c.id.clone(), Vec::new()))
            .collect());
    };
    a.clocks
        .iter()
        .map(|c| {
            evidence::read(Path::new(d), &c.id)
                .map(|h| (c.id.clone(), h))
                .map_err(|e| e.to_string())
        })
        .collect()
}

pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    let file = args.get(1).filter(|a| !a.starts_with("--")).cloned();
    match (sub, file) {
        (Some("history"), Some(v)) if v == "verify" => {
            let Some(dir) = args.get(2) else {
                return fail(USAGE);
            };
            let rd = match std::fs::read_dir(dir) {
                Ok(r) => r,
                Err(e) => return fail(format!("{dir}: {e}")),
            };
            let mut names: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|n| n.strip_suffix(".jsonl").map(String::from))
                .collect();
            names.sort();
            let mut bad = false;
            for n in &names {
                match evidence::read(Path::new(dir), n) {
                    Ok(h) => {
                        let p = evidence::verify_chain(&h);
                        if p.is_empty() {
                            println!("{n}: {} entries, chain intact", h.len());
                        } else {
                            bad = true;
                            println!("{n}: BROKEN: {}", p.join("; "));
                        }
                    }
                    Err(e) => {
                        bad = true;
                        println!("{n}: {e}");
                    }
                }
            }
            if bad {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        (Some("scorecard"), Some(path)) => scorecard_cmd(args, &path),
        (Some("compliance"), Some(fw)) => compliance_cmd(args, &fw),
        (Some(cmd @ ("assess" | "console" | "metrics" | "audit")), Some(path)) => {
            let feature = match cmd {
                "assess" => timing::KIND,
                "audit" => timing::AUDIT_FEATURE,
                _ => timing::FLEET_FEATURE,
            };
            if let Err(e) = gate(args, feature) {
                return fail(e);
            }
            let spec = match load_fleet(&path) {
                Ok(s) => s,
                Err(e) => return fail(e),
            };
            let a = match fleet::assess(&spec) {
                Ok(a) => a,
                Err(e) => return fail(e),
            };
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            let ev = flag(args, "--evidence");
            match cmd {
                "assess" => {
                    if let Some(d) = &ev {
                        if let Err(e) = evidence::append(Path::new(d), &a, &utc) {
                            return fail(e);
                        }
                        eprintln!("[appended {} evidence entries to {d}]", a.clocks.len());
                    }
                    let json = a.to_json();
                    match flag(args, "--json") {
                        Some(o) => {
                            if let Err(e) = std::fs::write(&o, json + "\n") {
                                return fail(format!("{o}: {e}"));
                            }
                        }
                        None => println!("{json}"),
                    }
                    for id in &a.order {
                        if let Some(c) = a.clock(id) {
                            eprintln!(
                                "{:<12} {:<8} earliest warning {}",
                                c.id,
                                c.status,
                                fleet::human_duration(c.worst_warn_s())
                            );
                        }
                    }
                    ExitCode::SUCCESS
                }
                "console" => {
                    let Some(out) = flag(args, "--out") else {
                        return fail("console needs --out <page.html>");
                    };
                    let h = match histories(ev.as_deref(), &a) {
                        Ok(h) => h,
                        Err(e) => return fail(e),
                    };
                    let page = console::page(&a, &h, &format!("generated {utc}"));
                    match std::fs::write(&out, page) {
                        Ok(()) => {
                            eprintln!("[wrote {out}]");
                            ExitCode::SUCCESS
                        }
                        Err(e) => fail(format!("{out}: {e}")),
                    }
                }
                "metrics" => {
                    let text = metrics::exposition(&a);
                    match flag(args, "--out") {
                        Some(o) => match std::fs::write(&o, text) {
                            Ok(()) => ExitCode::SUCCESS,
                            Err(e) => fail(format!("{o}: {e}")),
                        },
                        None => {
                            print!("{text}");
                            ExitCode::SUCCESS
                        }
                    }
                }
                _ => audit_cmd(args, &spec, &a, ev.as_deref(), &utc),
            }
        }
        _ => fail(USAGE),
    }
}

fn audit_cmd(
    args: &[String],
    spec: &fleet::FleetSpec,
    a: &fleet::FleetAssessment,
    ev: Option<&str>,
    utc: &str,
) -> ExitCode {
    let (Some(profile), Some(out), Some(signer), Some(kf)) = (
        flag(args, "--profile"),
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return fail("audit needs --profile, --out, --signer and --key-file");
    };
    let op = match flag(args, "--operator") {
        Some(p) => match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| toml::from_str::<audit::OperatorStatement>(&t).map_err(|e| e.to_string()))
        {
            Ok(o) => o,
            Err(e) => return fail(format!("{p}: {e}")),
        },
        None => audit::OperatorStatement::default(),
    };
    let seed = match std::fs::read_to_string(&kf)
        .map_err(|e| e.to_string())
        .and_then(|s| {
            let s = s.trim();
            let s = s
                .lines()
                .find_map(|l| l.trim().strip_prefix("private_key="))
                .unwrap_or(s);
            license::parse_key_hex(s.trim()).map_err(|e| e.to_string())
        }) {
        Ok(s) => s,
        Err(e) => return fail(format!("{kf}: {e}")),
    };
    let h = match histories(ev, a) {
        Ok(h) => h,
        Err(e) => return fail(e),
    };
    let report = match audit::build(spec, a, &h, &profile, &op, utc) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let dir = Path::new(&out);
    if let Err(e) = audit::write_dir(dir, &report, a, &h) {
        return fail(e);
    }
    match crate::signed_dossier::sign(dir, &seed, &signer, utc) {
        Ok(p) => {
            eprintln!(
                "[wrote and signed {out} ({}); public key {}]",
                p.display(),
                hex::encode(license::public_key_for(&seed))
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn scorecard_cmd(args: &[String], path: &str) -> ExitCode {
    use crate::timing::scorecard::{self, TimingScorecardSpec};
    if let Err(e) = gate(args, scorecard::KIND) {
        return fail(e);
    }
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return fail("scorecard needs --out, --signer and --key-file");
    };
    let seed = match super::forensics_cmd::read_seed(&kf) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let (spec, src) = match TimingScorecardSpec::from_file(Path::new(path)) {
        Ok(x) => x,
        Err(e) => return fail(e),
    };
    let s = match scorecard::score(&spec, &src) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let out = Path::new(&out);
    if let Err(e) = scorecard::write_dir(out, &s, &src) {
        return fail(e);
    }
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    if let Err(e) = crate::signed_dossier::sign(out, &seed, &signer, &utc) {
        return fail(e);
    }
    eprintln!(
        "[timing scorecard] {} grandmaster-scenario pairs; highest level supported: {}; signed in {}",
        s.results.len(),
        s.levels.highest_supported,
        out.display()
    );
    ExitCode::SUCCESS
}

/// `timing compliance list` and `timing compliance <framework> <inputs.toml> ...` (W1119 C4).
fn compliance_cmd(args: &[String], fw: &str) -> ExitCode {
    use crate::timing::compliance;
    if fw == "list" {
        for (id, title, source) in compliance::FRAMEWORKS {
            println!("{id:<20} {title}\n{:<20} {source}", "");
        }
        return ExitCode::SUCCESS;
    }
    if let Err(e) = gate(args, compliance::FEATURE) {
        return fail(e);
    }
    let Some(path) = args.get(2).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return fail("compliance needs --out, --signer and --key-file");
    };
    let seed = match super::forensics_cmd::read_seed(&kf) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return fail(format!("{path}: {e}")),
    };
    let inp = match compliance::Inputs::parse(&src) {
        Ok(i) => i,
        Err(e) => return fail(e),
    };
    let base = Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    let pack = match compliance::build(fw, &inp, &base, &utc) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    let out = Path::new(&out);
    if let Err(e) = compliance::write_dir(out, &pack, &src, &inp, &base) {
        return fail(e);
    }
    if let Err(e) = crate::signed_dossier::sign(out, &seed, &signer, &utc) {
        return fail(e);
    }
    let c = &pack.counts;
    eprintln!(
        "[timing compliance] {}: {} requirements: {} supported, {} not supported, {} insufficient, {} operator statements, {} to complete; signed pack in {}",
        pack.framework,
        pack.requirements.len(),
        c.supported,
        c.not_supported,
        c.insufficient,
        c.operator_statement,
        c.to_complete,
        out.display()
    );
    for p in &pack.evidence.problems {
        eprintln!("  input problem: {p}");
    }
    ExitCode::SUCCESS
}
