// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro lunar-evidence <command>`: lunar and cislunar PNT evidence
//! (`crate::lunar_evidence`, research backlog E24). Every result is a signed directory.
//!
//! Licence: `ura`, `time-consistency`, `afs-rx` need `pro-lunar-service-verification`;
//! `clock`, `station-delay`, `radiometric` need `pro-lunar-clock-evidence`; `fdir`, `config`,
//! `datum`, `tipover` need `pro-lunar-surface-evidence` (all Pro). `check` and `synth` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::aviation::write_result;
use crate::lunar_evidence::{self as le, summary};

use super::evidence_cmd::{producing, sign};

pub const USAGE: &str = "usage: kshana-pro lunar-evidence <command> <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
    ura                URA/SISA overbounding of a lunar navigation service's broadcast accuracy, per node and pooled ([ura])\n\
    time-consistency   Cross-satellite time-consistency monitor: steps and ramps against the ensemble ([time_consistency])\n\
    afs-rx             AFS receiver verification: replica codes, frame CRC and TOI, PRN collisions, against the open LSIS V1.0 generator ([afs_rx])\n\
    clock              Lunar clock characterisation: ADEV with 95 % intervals against a clock class or mask ([clock])\n\
    station-delay      Ground-station delay calibration with a GUM budget and En against the previous calibration ([station_delay])\n\
    radiometric        Radiometric anomaly forensics and per-pass track-quality score ([radiometric])\n\
    fdir               Landing-sensor FDIR results against a pre-registered, hash-locked plan ([fdir])\n\
    config             Flight configuration as loaded against the reviewed configuration ([config])\n\
    datum              Map-to-frame datum tie with the mean-Earth/principal-axis check ([datum])\n\
    tipover            MODELLED tip-over and design-slope exceedance probability over the landing dispersion ([tipover])\n\
  kshana-pro lunar-evidence check <dir> --trust <signer public key hex>[,...]   Check a signed result directory, offline (free)\n\
  kshana-pro lunar-evidence synth <dir>   Write synthetic inputs with known faults and a request for every command (free)\n\
  MODELLED engineering evidence; never a certification, qualification or approval.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn emit<T: serde::Serialize>(
    dir: &Path,
    cmd: &str,
    r: &T,
    headline: String,
    l: &le::Loaded,
) -> Result<String, String> {
    write_result(
        dir,
        &format!("{cmd}.json"),
        r,
        &summary(cmd, &headline),
        &[],
        &l.inputs,
    )?;
    Ok(headline)
}

fn produce(args: &[String], gate: &str) -> Result<String, String> {
    let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return Err(USAGE.into());
    };
    let (out, seed, signer, utc) = producing(args, gate)?;
    let mut l = le::load(Path::new(file))?;
    let dir = Path::new(&out);
    let cmd = args[0].as_str();
    let h = match cmd {
        "ura" => {
            let r = le::service::ura(&mut l)?;
            let all = r
                .groups
                .last()
                .map(|g| g.verdict.clone())
                .unwrap_or_default();
            let bad: Vec<&str> = r
                .groups
                .iter()
                .filter(|g| g.verdict == "NOT OVERBOUNDED" && g.node != "ALL")
                .map(|g| g.node.as_str())
                .collect();
            let h = format!(
                "pooled {all}; not overbounded: {}",
                if bad.is_empty() {
                    "none".into()
                } else {
                    bad.join(", ")
                }
            );
            emit(dir, cmd, &r, h, &l)?
        }
        "time-consistency" => {
            let r = le::service::time_consistency(&mut l)?;
            let h = r.verdict.clone();
            emit(dir, cmd, &r, h, &l)?
        }
        "afs-rx" => {
            let r = le::afs::afs_rx(&mut l)?;
            let h = format!(
                "{}: {} code(s), {} frame stream(s), {} collision(s)",
                r.verdict,
                r.codes.len(),
                r.frames.len(),
                r.collisions.len()
            );
            emit(dir, cmd, &r, h, &l)?
        }
        "clock" => {
            let r = le::clocks::clock(&mut l)?;
            let h = format!("{} against {}", r.verdict, r.against);
            emit(dir, cmd, &r, h, &l)?
        }
        "station-delay" => {
            let r = le::clocks::station_delay(&mut l)?;
            let h = format!(
                "{}: {} ns, U = {} ns (k = {}); {}",
                r.station, r.delay_ns, r.expanded_ns, r.k, r.verdict
            );
            emit(dir, cmd, &r, h, &l)?
        }
        "radiometric" => {
            let r = le::clocks::radiometric(&mut l)?;
            let h = r
                .passes
                .iter()
                .map(|p| format!("{} {} ({})", p.pass, p.grade, p.score))
                .collect::<Vec<_>>()
                .join(", ");
            emit(dir, cmd, &r, h, &l)?
        }
        "fdir" => {
            let r = le::surface::fdir(&mut l)?;
            let h = format!("{}: {}", r.plan, r.verdict);
            emit(dir, cmd, &r, h, &l)?
        }
        "config" => {
            let r = le::surface::config(&mut l)?;
            let h = format!(
                "{}: {} mismatched, {} missing, {} unexpected",
                r.verdict,
                r.mismatched.len(),
                r.missing.len(),
                r.unexpected.len()
            );
            emit(dir, cmd, &r, h, &l)?
        }
        "datum" => {
            let r = le::surface::datum(&mut l)?;
            let h = format!(
                "{}: RMS {} m{}",
                r.verdict,
                r.rms_residual_m,
                r.frame_finding
                    .as_deref()
                    .map(|f| format!("; {f}"))
                    .unwrap_or_default()
            );
            emit(dir, cmd, &r, h, &l)?
        }
        _ => {
            let r = le::surface::tipover(&mut l)?;
            let h = format!(
                "P(tip) {} at the stated dispersion (allowable slope {} deg)",
                r.cases[0].p_tip, r.allowable_slope_deg
            );
            emit(dir, "tipover", &r, h, &l)?
        }
    };
    sign(&out, &seed, &signer, &utc)?;
    Ok(format!("[lunar-evidence] {h}; signed in {out}"))
}

/// `kshana-pro lunar-evidence ...`.
pub fn run(args: &[String]) -> ExitCode {
    let gate = match args.first().map(String::as_str) {
        Some("ura" | "time-consistency" | "afs-rx") => le::FEATURE_SERVICE,
        Some("clock" | "station-delay" | "radiometric") => le::FEATURE_CLOCK,
        Some("fdir" | "config" | "datum" | "tipover") => le::FEATURE_SURFACE,
        Some("check") => return super::scorecard_cmd::verify_dir(args),
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            return match le::synth::write_all(Path::new(dir)) {
                Ok(()) => {
                    println!("wrote synthetic lunar evidence inputs and requests under {dir}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            };
        }
        _ => return fail(USAGE),
    };
    match produce(args, gate) {
        Ok(msg) => {
            eprintln!("{msg}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
