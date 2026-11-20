// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro grid <pmu|drift|scada|prc002|event>` and `kshana-pro sfn <assess|systems>`
//! (backlog E31: power-grid and broadcast SFN timing evidence; docs/GRID-SFN-TIMING.md).
//! Every analysis needs `pro-grid-timing` (grid) or `pro-sfn-timing` (sfn); `sfn systems`
//! is free. Signed packs verify with `kshana-pro dossier verify <dir> --trust <key>` (free).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::license::{self, Entitlement, Product};
use crate::timing::{grid, sfn};

use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

pub const GRID_USAGE: &str = "usage: kshana-pro grid <command>\n\
    pmu <request.toml> [--json <out>] [--license <token>] [--dev]   PMU time error as phase-angle and TVE evidence, joined with the PMU's own C37.118 sync flags; per event window, the angle-difference error of every PMU pair (Pro)\n\
    drift <request.toml> [--json <out>] [--csv <trace.csv>] [--license <token>] [--dev]   Gradual-spoof detection on a recorded time-error series or a same-bus PMU angle pair: level CUSUM, rate and step tests, warning before the budget is used up (Pro)\n\
    scada <request.toml> [--json <out>] [--license <token>] [--dev]   SCADA time-stamp integrity per device: backwards, duplicates, clock ahead, clock steps and drift, resolution (Pro)\n\
    prc002 <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Recording devices' measured time error against the ±2 ms PRC-002 criterion with monitoring gaps and holdover resilience, signed (Pro)\n\
    event <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Grid event forensics: was timing implicated? Per device error, drift before the event, sync flags, SCADA findings, corrected sequence of events, signed (Pro)";

pub const SFN_USAGE: &str = "usage: kshana-pro sfn <command>\n\
    assess <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   DAB/DVB-T SFN timing: recorded logs (UTC-level, transmitter and pair-margin events) and UTC-level scenarios (svn23, GNSS loss, ramp) against the guard interval, signed (Pro)\n\
    systems                                                         The guard intervals known (no licence needed)";

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

pub(super) fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn base_of(path: &str) -> PathBuf {
    Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn write_or_print(out: Option<String>, text: &str) -> Result<(), String> {
    match out {
        Some(o) => std::fs::write(&o, text).map_err(|e| format!("{o}: {e}")),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

fn parse<T: serde::de::DeserializeOwned>(path: &str) -> Result<(T, String), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let v = toml::from_str(&src).map_err(|e| format!("{path}: {e}"))?;
    Ok((v, src))
}

pub(super) struct Signing {
    pub(super) out: String,
    pub(super) signer: String,
    pub(super) seed: [u8; 32],
    pub(super) utc: String,
}

pub(super) fn signing(args: &[String], cmd: &str) -> Result<Signing, String> {
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return Err(format!("{cmd} needs --out, --signer and --key-file"));
    };
    Ok(Signing {
        out,
        signer,
        seed: super::forensics_cmd::read_seed(&kf)?,
        utc: flag(args, "--utc").unwrap_or_else(utc_iso8601_now),
    })
}

fn sign(s: &Signing) -> Result<(), String> {
    crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// `kshana-pro grid ...`.
pub fn run_grid(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(GRID_USAGE);
    };
    if !matches!(sub, Some("pmu" | "drift" | "scada" | "prc002" | "event")) {
        return fail(GRID_USAGE);
    }
    if let Err(e) = gate(args, grid::FEATURE) {
        return fail(e);
    }
    let base = base_of(&path);
    let res: Result<String, String> = (|| match sub {
        Some("pmu") => {
            let (req, _): (grid::PmuRequest, _) = parse(&path)?;
            let r = grid::assess_pmus(&req, &base)?;
            write_or_print(flag(args, "--json"), &grid::rounded_json(&r))?;
            for p in &r.pmus {
                eprintln!(
                    "  {:<24} {:<28} max |TE| {}",
                    p.name,
                    p.status,
                    p.max_abs_te_s
                        .map(crate::timing::fleet::human_time)
                        .unwrap_or_else(|| "-".into())
                );
            }
            Ok(format!("[grid pmu] {}", r.verdict))
        }
        Some("drift") => {
            let (req, _): (grid::DriftRequest, _) = parse(&path)?;
            let r = grid::run_drift(&req, &base)?;
            write_or_print(flag(args, "--json"), &grid::rounded_json(&r))?;
            if let Some(c) = flag(args, "--csv") {
                std::fs::write(&c, grid::drift_csv(&r)).map_err(|e| format!("{c}: {e}"))?;
            }
            Ok(format!("[grid drift] {}", r.verdict))
        }
        Some("scada") => {
            let (spec, _): (grid::ScadaSpec, _) = parse(&path)?;
            let r = grid::check_scada(&spec, &base)?;
            write_or_print(flag(args, "--json"), &grid::rounded_json(&r))?;
            Ok(format!("[grid scada] {}", r.verdict))
        }
        Some("prc002") => {
            let s = signing(args, "grid prc002")?;
            let (req, src): (grid::Prc002Request, _) = parse(&path)?;
            let r = grid::assess_prc002(&req, &base, &s.utc)?;
            grid::write_pack(
                Path::new(&s.out),
                &r,
                &grid::prc002_html(&r),
                "prc002.toml",
                &src,
            )?;
            sign(&s)?;
            Ok(format!("[grid prc002] {}; signed in {}", r.verdict, s.out))
        }
        Some("event") => {
            let s = signing(args, "grid event")?;
            let (req, src): (grid::EventRequest, _) = parse(&path)?;
            let r = grid::assess_event(&req, &base, &s.utc)?;
            grid::write_pack(
                Path::new(&s.out),
                &r,
                &grid::event_html(&r),
                "event.toml",
                &src,
            )?;
            sign(&s)?;
            Ok(format!("[grid event] {}; signed in {}", r.verdict, s.out))
        }
        _ => Err(GRID_USAGE.into()),
    })();
    match res {
        Ok(v) => {
            eprintln!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `kshana-pro sfn ...`.
pub fn run_sfn(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("systems") => {
            for (n, g, b) in sfn::systems() {
                println!("{n:<16} guard {:>9.3} µs  {b}", g * 1e6);
            }
            ExitCode::SUCCESS
        }
        Some("assess") => {
            let Some(path) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
                return fail(SFN_USAGE);
            };
            if let Err(e) = gate(args, sfn::FEATURE) {
                return fail(e);
            }
            let res: Result<String, String> = (|| {
                let s = signing(args, "sfn assess")?;
                let src = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                let req = sfn::SfnRequest::parse(&src)?;
                let r = sfn::assess(&req, &base_of(&path), &s.utc)?;
                sfn::write_dir(Path::new(&s.out), &r, &src)?;
                sign(&s)?;
                Ok(format!("[sfn] {}; signed in {}", r.verdict, s.out))
            })();
            match res {
                Ok(v) => {
                    eprintln!("{v}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(SFN_USAGE),
    }
}
