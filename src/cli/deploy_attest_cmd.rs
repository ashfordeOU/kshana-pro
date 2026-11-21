// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro deploy-attest <baseline|check|synth>`: signed deployment baselines and
//! configuration-drift reports for ground software (docs/DEPLOY-ATTESTATION.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::deploy_attest;

use super::flag;
use super::forensics_cmd::keys;
use super::grid_cmd::{gate, signing};

pub const DEPLOY_USAGE: &str = "usage: kshana-pro deploy-attest <command>\n\
    baseline <release-dir|hashes.sha256> --deployment <name> --release <id> --out <dir> --signer <name> --key-file <file> [--exclude <glob>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Signed baseline manifest of a released deployment (SHA-256 of every file) (Pro)\n\
    check <request.toml> --trust <key hex>[,...] --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Host hash snapshots against the verified baseline: modified, missing, added and accepted files, drift timeline, signed report (Pro)\n\
    synth <dir>   Write a SYNTHETIC release, host snapshots and a request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro deploy-attest ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(DEPLOY_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("synth") => (|| {
            let d = Path::new(&arg);
            let (rel, snaps, req) = deploy_attest::synth();
            for (p, t) in &rel {
                let f = d.join("release").join(p);
                std::fs::create_dir_all(f.parent().expect("has a parent"))
                    .map_err(|e| e.to_string())?;
                std::fs::write(f, t).map_err(|e| e.to_string())?;
            }
            for (n, t) in &snaps {
                std::fs::write(d.join(n), t).map_err(|e| e.to_string())?;
            }
            std::fs::write(d.join("request.toml"), req).map_err(|e| e.to_string())?;
            Ok(format!(
                "[deploy-attest] SYNTHETIC release, snapshots and request in {arg}: run baseline on {arg}/release with --exclude 'logs/*' --out {arg}/baseline-pack, then check request.toml"
            ))
        })(),
        Some("baseline") => (|| {
            gate(args, deploy_attest::FEATURE)?;
            let s = signing(args, "deploy-attest baseline")?;
            let dep = flag(args, "--deployment").ok_or("baseline needs --deployment")?;
            let rel = flag(args, "--release").ok_or("baseline needs --release")?;
            let ex: Vec<String> = flag(args, "--exclude")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(String::from)
                .collect();
            let p = Path::new(&arg);
            let b = if p.is_dir() {
                deploy_attest::baseline_of_dir(p, &dep, &rel, &ex, &s.utc)?
            } else {
                let t = std::fs::read_to_string(p).map_err(|e| format!("{arg}: {e}"))?;
                let files =
                    deploy_attest::parse_hash_list(&t).map_err(|e| format!("{arg}: {e}"))?;
                deploy_attest::baseline_of_list(
                    files,
                    &dep,
                    &rel,
                    &format!("hash list {arg}"),
                    &ex,
                    &s.utc,
                )?
            };
            deploy_attest::write_baseline(Path::new(&s.out), &b)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[deploy-attest] signed baseline of {} {} ({} files) in {}",
                b.deployment,
                b.release,
                b.files.len(),
                s.out
            ))
        })(),
        Some("check") => (|| {
            gate(args, deploy_attest::FEATURE)?;
            let s = signing(args, "deploy-attest check")?;
            let trust = keys(args, "--trust")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: deploy_attest::Request =
                toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let (rep, inputs) = deploy_attest::check(&q, &base, &trust)?;
            deploy_attest::write(Path::new(&s.out), &rep, &src, &inputs)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[deploy-attest] {}\n[deploy-attest] signed report in {}",
                rep.verdict, s.out
            ))
        })(),
        _ => return fail(DEPLOY_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
