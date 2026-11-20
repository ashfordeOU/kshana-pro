// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro aviation <command>`: aviation operational analytics (`crate::aviation`,
//! research backlog E11). Every result is a signed directory and decision support only.
//!
//! Licence: `approach` needs `pro-approach-availability` (Pro), `residual`
//! `pro-residual-effects` (Pro), `nuisance` `pro-nuisance-alerts` (Pro), `areas`
//! `pro-unpublished-areas` (Enterprise). `check` and `synth` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::aviation::{self, approach, nuisance, residual, unpublished};

use super::evidence_cmd::{producing, sign};

pub const USAGE: &str = "usage: kshana-pro aviation <command>\n\
    approach <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Per-airport GNSS approach availability loss, go-around exposure, capacity impact (modelled) and oceanic RNP support from ADS-B; signed (Pro)\n\
    residual <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Residual effects after interference across consecutive flights (clock, degradation after leaving the area, TAWS alerts) and a clean-or-reset recommendation; signed (Pro)\n\
    nuisance <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Nuisance-alert and alert-fatigue statistics per route and phase (TAWS, clock, bridge GNSS/BNWAS), workload per crew-hour, training extract; signed (Pro)\n\
    areas <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Interference areas in fleet data with no covering NOTAM, and short bursts, above the publication minimum at 0.5° cells; alerts for operations; signed (Enterprise)\n\
    check <dir> --trust <signer public key hex>[,...]\n\
        Check a signed result directory, offline (free)\n\
    synth <dir>\n\
        Write synthetic inputs and request files for every command (free)\n\
  Decision support only: never an operational approval or an airworthiness determination. MODELLED.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

fn approach_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, aviation::FEATURE_APPROACH).and_then(|(out, seed, signer, utc)| {
        let (r, inp) = approach::run(Path::new(file))?;
        aviation::write_result(
            Path::new(&out),
            approach::RESULT_FILE,
            &r,
            &approach::summary(&r),
            &[],
            &inp,
        )?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((r, out))
    });
    match r {
        Ok((r, out)) => {
            print!("{}", approach::summary(&r));
            eprintln!("[aviation] signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn residual_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, aviation::FEATURE_RESIDUAL).and_then(|(out, seed, signer, utc)| {
        let (r, inp) = residual::run(Path::new(file))?;
        aviation::write_result(
            Path::new(&out),
            residual::RESULT_FILE,
            &r,
            &residual::summary(&r),
            &[],
            &inp,
        )?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((r, out))
    });
    match r {
        Ok((r, out)) => {
            print!("{}", residual::summary(&r));
            eprintln!("[aviation] signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn nuisance_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, aviation::FEATURE_NUISANCE).and_then(|(out, seed, signer, utc)| {
        let (r, inp) = nuisance::run(Path::new(file))?;
        aviation::write_result(
            Path::new(&out),
            nuisance::RESULT_FILE,
            &r,
            &nuisance::summary(&r),
            &[("training-extract.csv", nuisance::training_csv(&r))],
            &inp,
        )?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((r, out))
    });
    match r {
        Ok((r, out)) => {
            print!("{}", nuisance::summary(&r));
            eprintln!("[aviation] signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn areas_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, aviation::FEATURE_AREAS).and_then(|(out, seed, signer, utc)| {
        let (r, inp) = unpublished::run(Path::new(file))?;
        aviation::write_result(
            Path::new(&out),
            unpublished::RESULT_FILE,
            &r,
            &unpublished::summary(&r),
            &[
                ("alerts.csv", unpublished::alerts_table(&r)),
                ("areas.geojson", unpublished::geojson(&r)),
            ],
            &inp,
        )?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((r, out))
    });
    match r {
        Ok((r, out)) => {
            print!("{}", unpublished::summary(&r));
            eprintln!("[aviation] signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `kshana-pro aviation ...`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("approach") => approach_cmd(args),
        Some("residual") => residual_cmd(args),
        Some("nuisance") => nuisance_cmd(args),
        Some("areas") => areas_cmd(args),
        Some("check") => super::scorecard_cmd::verify_dir(args),
        Some("synth") => {
            let Some(dir) = args.get(1) else {
                return fail(USAGE);
            };
            match aviation::synth::write_all(Path::new(dir)) {
                Ok(()) => {
                    println!("wrote approach/, residual/, nuisance/ and areas/ under {dir}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        _ => fail(USAGE),
    }
}
