// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Maritime, aviation and timing evidence commands of programme W1119 (items C3, C5 to C8),
//! hung under the existing commands:
//!
//! * `kshana-pro forensics maritime <request.toml> ...` and `forensics maritime-synth`
//!   (C3, gate `pro-incident-forensics`);
//!
//! Producing a signed result needs the licence feature of its gate; verifying a signed
//! directory needs none (`kshana-pro dossier verify`).

use std::path::Path;
use std::process::ExitCode;

use super::evidence_cmd::{producing, sign};

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

pub const MARITIME_USAGE: &str = "usage: kshana-pro forensics maritime <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Signed maritime incident evidence pack (actor vs victim) from the vessel's NMEA, AIS of the vessel and neighbours, and Station events (Pro)\n\
    kshana-pro forensics maritime-synth <clean|area-jamming|area-spoofing|local-spoofing|ais-manipulation|ship-side-fault> <dir>\n\
        Write a synthetic scenario (own.nmea, ais.csv, request.toml) to try the pack (free)";

/// `forensics maritime`.
pub fn maritime(args: &[String]) -> ExitCode {
    use crate::forensics::maritime as m;
    let Some(file) = file_arg(args) else {
        return fail(MARITIME_USAGE);
    };
    let r = producing(args, crate::forensics::KIND).and_then(|(out, seed, signer, utc)| {
        let req = m::MaritimeRequest::from_file(Path::new(file))?;
        let inp = req.load()?;
        let pack = m::analyse(&inp)?;
        m::write_pack(Path::new(&out), &pack, &inp)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((pack, out))
    });
    match r {
        Ok((p, out)) => {
            eprintln!(
                "[maritime] {} episode(s); conclusion {:?}; signed evidence pack in {out}",
                p.episodes.len(),
                p.verdict.conclusion
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `forensics maritime-synth`.
pub fn maritime_synth(args: &[String]) -> ExitCode {
    use crate::forensics::maritime::synth::{make, Case};
    let (Some(c), Some(dir)) = (args.get(1), args.get(2)) else {
        return fail(MARITIME_USAGE);
    };
    let case = match c.as_str() {
        "clean" => Case::Clean,
        "area-jamming" => Case::AreaJamming,
        "area-spoofing" => Case::AreaSpoofing,
        "local-spoofing" => Case::LocalSpoofing,
        "ais-manipulation" => Case::AisManipulation,
        "ship-side-fault" => Case::ShipSideFault,
        _ => return fail(MARITIME_USAGE),
    };
    let s = make(case);
    let d = Path::new(dir);
    let req = format!(
        "title = \"Synthetic {c}\"\nvessel_id = \"{}\"\nnmea = \"own.nmea\"\nais = \"ais.csv\"\n",
        s.vessel_id
    );
    let w = || -> std::io::Result<()> {
        std::fs::create_dir_all(d)?;
        std::fs::write(d.join("own.nmea"), &s.nmea)?;
        std::fs::write(d.join("ais.csv"), &s.ais_csv)?;
        std::fs::write(d.join("request.toml"), req)
    };
    match w() {
        Ok(()) => {
            eprintln!("[maritime] synthetic {c} scenario in {dir}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(format!("{dir}: {e}")),
    }
}
