// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Maritime, aviation and timing evidence commands of programme W1119 (items C3, C5 to C8),
//! hung under the existing commands:
//!
//! * `kshana-pro forensics maritime <request.toml> ...` and `forensics maritime-synth`
//!   (C3, gate `pro-incident-forensics`);
//! * `kshana-pro forensics fdm <flight.csv> ...` and `forensics fdm-synth` (C5, same gate);
//! * `kshana-pro scorecard osnma <log.json> ...` and `scorecard osnma-synth` (C6, gate
//!   `pro-receiver-scorecard`);
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

pub const FDM_USAGE: &str = "usage: kshana-pro forensics fdm <flight.csv> --flight <id> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        GNSS jamming and spoofing events in a flight-data export, unified events and an EASA SIB 2022-02R4-oriented summary, signed (Pro)\n\
    kshana-pro forensics fdm-synth <clean|jamming|spoofing|holding|circle-spoof> <file.csv>\n\
        Write a synthetic flight to try it (free)";

/// `forensics fdm`.
pub fn fdm(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(FDM_USAGE);
    };
    let flight = super::flag(args, "--flight").unwrap_or_else(|| "flight".into());
    let r = producing(args, crate::forensics::KIND).and_then(|(out, seed, signer, utc)| {
        let csv = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        let r = crate::fdm::analyse(&flight, &csv, &crate::fdm::Params::default())?;
        crate::fdm::write_dir(Path::new(&out), &r, &csv)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((r, out))
    });
    match r {
        Ok((r, out)) => {
            eprintln!(
                "[fdm] {} jamming and {} spoofing event(s); signed in {out}",
                r.sib.jamming_events, r.sib.spoofing_events
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `forensics fdm-synth`.
pub fn fdm_synth(args: &[String]) -> ExitCode {
    use crate::fdm::synth::{make, Flight};
    let (Some(c), Some(file)) = (args.get(1), args.get(2)) else {
        return fail(FDM_USAGE);
    };
    let f = match c.as_str() {
        "clean" => Flight::Clean,
        "jamming" => Flight::Jamming,
        "spoofing" => Flight::Spoofing,
        "holding" => Flight::Holding,
        "circle-spoof" => Flight::CircleSpoof,
        _ => return fail(FDM_USAGE),
    };
    match std::fs::write(file, make(f)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(format!("{file}: {e}")),
    }
}

pub const OSNMA_USAGE: &str = "usage: kshana-pro scorecard osnma <receiver-log.json> --out <dir> --signer <name> --key-file <file> [--bars <bars.toml>] [--utc <iso>] [--license <token>] [--dev]\n\
        Score an OSNMA receiver's log against pre-registered bars (TTFAF, forged navigation data, replay, key/tree renewal, revocation); signed scorecard (Pro)\n\
    kshana-pro scorecard osnma-synth <good|faulty> <file.json>\n\
        Write a synthetic receiver log to try it (free)";

/// `scorecard osnma`.
pub fn osnma(args: &[String]) -> ExitCode {
    use crate::osnma_conformance as o;
    let Some(file) = file_arg(args) else {
        return fail(OSNMA_USAGE);
    };
    let r = producing(args, crate::scorecards::KIND).and_then(|(out, seed, signer, utc)| {
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let log: o::ReceiverLog =
            serde_json::from_slice(&bytes).map_err(|e| format!("{file}: {e}"))?;
        let bars = match super::flag(args, "--bars") {
            Some(b) => {
                let t = std::fs::read_to_string(&b).map_err(|e| format!("{b}: {e}"))?;
                toml::from_str::<o::Bars>(&t).map_err(|e| format!("{b}: {e}"))?
            }
            None => o::Bars::default(),
        };
        let c = o::score(&log, &bytes, &bars)?;
        o::write_dir(Path::new(&out), &c, &bytes)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((c, out))
    });
    match r {
        Ok((c, out)) => {
            eprintln!(
                "[osnma] {} met, {} not met, {} not assessed; signed scorecard in {out}",
                c.met, c.not_met, c.not_assessed
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `scorecard osnma-synth`.
pub fn osnma_synth(args: &[String]) -> ExitCode {
    let (Some(c), Some(file)) = (args.get(1), args.get(2)) else {
        return fail(OSNMA_USAGE);
    };
    let good = match c.as_str() {
        "good" => true,
        "faulty" => false,
        _ => return fail(OSNMA_USAGE),
    };
    let log = crate::osnma_conformance::synth::log(good);
    match std::fs::write(file, serde_json::to_string_pretty(&log).unwrap_or_default()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(format!("{file}: {e}")),
    }
}
