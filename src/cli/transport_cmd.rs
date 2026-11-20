// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro road|ecall|auto|nr|rail ...` (research 2026-11-20 gaps, epic E30: road, rail,
//! automotive, eCall and 3GPP evidence; docs/ROAD-RAIL-AUTO.md). Every analysis needs its
//! Pro feature; listings, the suite and mapping, vectors, duty-cycle detection and the
//! battery model are free. Signed packs verify with `kshana-pro dossier verify <dir>
//! --trust <key>` (free).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::transport::{agnss, auto, ecall, nr, rail, telematics, tolerance, Table};

use super::grid_cmd::{gate, signing, Signing};

pub const ROAD_USAGE: &str = "usage: kshana-pro road <command>\n\
    check <request.toml> [--json <out>] [--html <out>] [--license <token>] [--dev]   GNSS anti-spoof and jamming indicators over a telematics or on-board-unit stream: episodes, fingerprint class, per-trip trust; pseudonymised (Pro)\n\
    obu-eval <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   On-board-unit fraud evaluation over test runs with recorded attack windows: detection, time to detect, false episodes, unreported distance; signed EETS evidence (Pro)\n\
    library [--json]                                                 The jammer fingerprint library (no licence needed)";

pub const ECALL_USAGE: &str = "usage: kshana-pro ecall <command>\n\
    suite [--json]                                                   The eCall GNSS tests of 2017/79 Annex VI with their limits, and two Kshana interference extensions (no licence needed)\n\
    mapping [--json]                                                 Every Annex VI requirement mapped to its tests, with the UN R144 and ERA-GLONASS notes (no licence needed)\n\
    score <results.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Score laboratory results against the Annex VI limits; signed pre-assessment (Pro)\n\
    location <calls.csv> [--title <t>] [--json <out>] [--license <token>] [--dev]   AML/E112 and PSAP location reliability against the declared confidence, with and without interference (Pro)\n\
    duty <log.csv> [--json]                                          Detect a duty-cycled GNSS receiver in a tracker log (no licence needed)\n\
    battery <params.toml> [--json]                                   IoT tracker battery life against fix interval (no licence needed)";

pub const AUTO_USAGE: &str = "usage: kshana-pro auto <command>\n\
    catalogue [--json]                                               SOTIF triggering-condition catalogue for GNSS (no licence needed)\n\
    pl <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   Protection-level validation per triggering condition with coverage (Pro)\n\
    v2x <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   V2X confidence-ellipse, time, detection and false-flag checks over a decoded CAM log (Pro)\n\
    inject <request.toml> --csv <out.csv> [--license <token>] [--dev]   Data-level fault injection on recorded position output; generates no signal (Pro)";

pub const NR_USAGE: &str = "usage: kshana-pro nr <command>\n\
    tdd <request.toml> [--json <out>] [--license <token>] [--dev]   5G TDD frame timing under GNSS loss: phase budget and guard-period interference onset from the holdover model (Pro)\n\
    prs <request.toml> [--json <out>] [--license <token>] [--dev]   PRS DL-TDOA accuracy against bandwidth, SNR, gNB synchronisation and geometry (Pro)\n\
    vectors [--out <file>]                                           GNSS integrity reference vectors (no licence needed)\n\
    check <vectors.json> <device.csv> [--json <out>] [--license <token>] [--dev]   Check a device's protection levels against the vectors (Pro)\n\
    tolerance <request.toml> [--json <out>] [--license <token>] [--dev]   Shared-risk test tolerances and test requirements on user-transcribed TS 37.571-1 values, with placeholders (Pro)\n\
    agnss-scaffold [--out <file>]                                    A-GNSS vector-set skeleton, every value a placeholder (no licence needed)\n\
    agnss-status <set.json> [--json <out>]                           Which values are still placeholders; refuses values without a source (no licence needed)\n\
    agnss-score <set.json> <results.csv> [--rule point|lower-bound] [--json <out>] [--license <token>] [--dev]   Score device trials against the transcribed limits and tolerances (Pro)";

pub const RAIL_USAGE: &str = "usage: kshana-pro rail <command>\n\
    thr <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   Position hazard rate including jamming and spoofing against the apportioned THR (Pro)\n\
    balise <request.toml> [--json <out>] [--out <dir> --signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]   Virtual-balise passage errors with GUM uncertainty and an overbound check (Pro)\n\
    (the EN 50129 evidence template: kshana-pro regpack assess --catalogue rail-gnss-sil4)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn finish(r: Result<String, String>) -> ExitCode {
    match r {
        Ok(v) => {
            eprintln!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn base_of(path: &str) -> PathBuf {
    Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn parse<T: serde::de::DeserializeOwned>(path: &str) -> Result<(T, String), String> {
    let src = read(path)?;
    let v = crate::transport::parse_toml(&src, path)?;
    Ok((v, src))
}

fn emit<T: serde::Serialize>(args: &[String], r: &T) -> Result<(), String> {
    let text = crate::transport::rounded_json(r);
    match super::flag(args, "--json") {
        Some(o) => std::fs::write(&o, text).map_err(|e| format!("{o}: {e}")),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

fn sign_pack<T: serde::Serialize>(
    s: &Signing,
    r: &T,
    html: &str,
    req_name: &str,
    req: &str,
) -> Result<(), String> {
    crate::transport::write_pack(Path::new(&s.out), r, html, req_name, req)?;
    crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn arg1(args: &[String]) -> Option<String> {
    args.get(1).filter(|a| !a.starts_with("--")).cloned()
}

/// `kshana-pro road ...`.
pub fn road(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("library") => {
            let l = telematics::Library::builtin();
            if super::has_flag(args, "--json") {
                println!("{}", telematics::LIBRARY_JSON);
            } else {
                println!("{} ({})", l.title, l.version);
                for c in &l.classes {
                    println!("  {:<22} {}", c.id, c.meaning);
                }
            }
            ExitCode::SUCCESS
        }
        Some(sub @ ("check" | "obu-eval")) => {
            let Some(path) = arg1(args) else {
                return fail(ROAD_USAGE);
            };
            let feature = if sub == "check" {
                telematics::FEATURE
            } else {
                telematics::FEATURE_OBU
            };
            if let Err(e) = gate(args, feature) {
                return fail(e);
            }
            finish((|| {
                if sub == "check" {
                    let (req, _): (telematics::CheckRequest, _) = parse(&path)?;
                    let r = telematics::check(&req, &base_of(&path))?;
                    emit(args, &r)?;
                    if let Some(h) = super::flag(args, "--html") {
                        std::fs::write(&h, telematics::check_html(&r))
                            .map_err(|e| format!("{h}: {e}"))?;
                    }
                    Ok(format!("[road check] {}", r.verdict))
                } else {
                    let s = signing(args, "road obu-eval")?;
                    let (req, src): (telematics::ObuRequest, _) = parse(&path)?;
                    let r = telematics::evaluate_obu(&req, &base_of(&path), &s.utc)?;
                    sign_pack(&s, &r, &telematics::obu_html(&r), "obu-eval.toml", &src)?;
                    Ok(format!(
                        "[road obu-eval] {}; signed in {}",
                        r.verdict, s.out
                    ))
                }
            })())
        }
        _ => fail(ROAD_USAGE),
    }
}

/// `kshana-pro ecall ...`.
pub fn ecall(args: &[String]) -> ExitCode {
    let json = super::has_flag(args, "--json");
    match args.first().map(String::as_str) {
        Some("suite") => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(ecall::SUITE).unwrap_or_default()
                );
            } else {
                println!("{}\n{}", ecall::SOURCE, ecall::SOURCE_READ);
                for t in ecall::SUITE {
                    println!(
                        "  {:<17} §{:<17} {:<9} {:<22} {}",
                        t.id,
                        t.para,
                        t.limit
                            .map(|l| format!("{l}"))
                            .unwrap_or_else(|| "applicant".into()),
                        t.unit,
                        t.title
                    );
                }
            }
            ExitCode::SUCCESS
        }
        Some("mapping") => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(ecall::MAPPING).unwrap_or_default()
                );
            } else {
                for r in ecall::MAPPING {
                    println!("  §{:<7} {:<50} {}", r.para, r.tests.join(", "), r.note);
                }
                println!("{}\n{}", ecall::R144_NOTE, ecall::ERA_GLONASS_NOTE);
            }
            ExitCode::SUCCESS
        }
        Some("duty") => {
            let Some(path) = arg1(args) else {
                return fail(ECALL_USAGE);
            };
            finish((|| {
                let t = Table::read(Path::new(&path))?;
                let ct = t.need("t")?;
                let cl = t.col("lat");
                let mut times = Vec::new();
                for i in 0..t.rows.len() {
                    if cl.is_none() || t.num(i, cl)?.is_some() {
                        times.extend(t.num(i, Some(ct))?);
                    }
                }
                let r = ecall::duty(&times);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?
                );
                Ok(format!("[ecall duty] {}", r.pattern))
            })())
        }
        Some("battery") => {
            let Some(path) = arg1(args) else {
                return fail(ECALL_USAGE);
            };
            finish((|| {
                let (p, _): (ecall::BatteryParams, _) = parse(&path)?;
                let rows = ecall::battery(&p)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
                    );
                } else {
                    println!("interval_s  start     on_s  J/fix    avg_mW   life_d   age_s");
                    for r in &rows {
                        println!(
                            "{:>10.0}  {:<8} {:>5.1} {:>7.3} {:>8.3} {:>8.1} {:>7.0}",
                            r.interval_s,
                            r.start,
                            r.on_s,
                            r.energy_per_fix_j,
                            r.avg_power_mw,
                            r.life_days,
                            r.mean_position_age_s
                        );
                    }
                }
                Ok("[ecall battery] MODELLED with the stated module figures".into())
            })())
        }
        Some("score") => {
            let Some(path) = arg1(args) else {
                return fail(ECALL_USAGE);
            };
            if let Err(e) = gate(args, ecall::FEATURE) {
                return fail(e);
            }
            finish((|| {
                let s = signing(args, "ecall score")?;
                let (res, src): (ecall::Results, _) = parse(&path)?;
                let r = ecall::score(&res, &base_of(&path), &s.utc)?;
                sign_pack(&s, &r, &ecall::html(&r), "results.toml", &src)?;
                Ok(format!("[ecall score] {}; signed in {}", r.verdict, s.out))
            })())
        }
        Some("location") => {
            let Some(path) = arg1(args) else {
                return fail(ECALL_USAGE);
            };
            if let Err(e) = gate(args, ecall::FEATURE_LOCATION) {
                return fail(e);
            }
            finish((|| {
                let t = Table::read(Path::new(&path))?;
                let title = super::flag(args, "--title").unwrap_or_else(|| path.clone());
                let r = ecall::location(&t, &title)?;
                emit(args, &r)?;
                Ok(format!("[ecall location] {}", r.verdict))
            })())
        }
        _ => fail(ECALL_USAGE),
    }
}

/// Either a signed pack (with `--out`) or JSON.
fn json_or_signed<T: serde::Serialize>(
    args: &[String],
    cmd: &str,
    r: &T,
    html: impl FnOnce() -> String,
    req_name: &str,
    src: &str,
) -> Result<String, String> {
    if super::flag(args, "--out").is_some() {
        let s = signing(args, cmd)?;
        sign_pack(&s, r, &html(), req_name, src)?;
        Ok(format!("; signed in {}", s.out))
    } else {
        emit(args, r)?;
        Ok(String::new())
    }
}

/// `kshana-pro auto ...`.
pub fn auto(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("catalogue") => {
            if super::has_flag(args, "--json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(auto::CATALOGUE).unwrap_or_default()
                );
            } else {
                for c in auto::CATALOGUE {
                    println!("  {:<22} {} | {}", c.id, c.condition, c.insufficiency);
                }
            }
            ExitCode::SUCCESS
        }
        Some(sub @ ("pl" | "v2x" | "inject")) => {
            let Some(path) = arg1(args) else {
                return fail(AUTO_USAGE);
            };
            let feature = match sub {
                "pl" => auto::FEATURE_PL,
                "v2x" => auto::FEATURE_V2X,
                _ => auto::FEATURE_INJECT,
            };
            if let Err(e) = gate(args, feature) {
                return fail(e);
            }
            let base = base_of(&path);
            finish((|| match sub {
                "pl" => {
                    let (req, src): (auto::PlRequest, _) = parse(&path)?;
                    let r = auto::validate_pl(&req, &base)?;
                    let tail =
                        json_or_signed(args, "auto pl", &r, || auto::pl_html(&r), "pl.toml", &src)?;
                    Ok(format!("[auto pl] {}{tail}", r.verdict))
                }
                "v2x" => {
                    let (req, src): (auto::V2xRequest, _) = parse(&path)?;
                    let r = auto::v2x(&req, &base)?;
                    let tail = json_or_signed(
                        args,
                        "auto v2x",
                        &r,
                        || auto::v2x_html(&r),
                        "v2x.toml",
                        &src,
                    )?;
                    Ok(format!("[auto v2x] {}{tail}", r.verdict))
                }
                _ => {
                    let out =
                        super::flag(args, "--csv").ok_or("auto inject needs --csv <out.csv>")?;
                    let (req, _): (auto::InjectRequest, _) = parse(&path)?;
                    let (csv, n) = auto::inject(&req, &base)?;
                    std::fs::write(&out, csv).map_err(|e| format!("{out}: {e}"))?;
                    Ok(format!(
                        "[auto inject] {n} epoch(s) faulted; written to {out}"
                    ))
                }
            })())
        }
        _ => fail(AUTO_USAGE),
    }
}

/// `kshana-pro nr ...`.
pub fn nr(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("vectors") => {
            let v = nr::vectors();
            let text = serde_json::to_string_pretty(&v).unwrap_or_default();
            match super::flag(args, "--out") {
                Some(o) => {
                    if let Err(e) = std::fs::write(&o, text + "\n") {
                        return fail(format!("{o}: {e}"));
                    }
                    eprintln!("[nr vectors] {} cases written to {o}", v.cases.len());
                }
                None => println!("{text}"),
            }
            ExitCode::SUCCESS
        }
        Some("check") => {
            let (Some(v), Some(d)) = (
                arg1(args),
                args.get(2).filter(|a| !a.starts_with("--")).cloned(),
            ) else {
                return fail(NR_USAGE);
            };
            if let Err(e) = gate(args, nr::FEATURE_VECTORS) {
                return fail(e);
            }
            finish((|| {
                let set: nr::VectorSet =
                    serde_json::from_str(&read(&v)?).map_err(|e| format!("{v}: {e}"))?;
                if set.format != nr::VECTORS_FORMAT {
                    return Err(format!("{v}: format '{}'", set.format));
                }
                let r = nr::check_vectors(&set, &Table::read(Path::new(&d))?)?;
                emit(args, &r)?;
                Ok(format!("[nr check] {}", r.verdict))
            })())
        }
        Some("tolerance") => {
            let Some(path) = arg1(args) else {
                return fail(NR_USAGE);
            };
            if let Err(e) = gate(args, tolerance::FEATURE) {
                return fail(e);
            }
            finish((|| {
                let (req, _): (tolerance::Request, _) = parse(&path)?;
                let r = tolerance::run(&req)?;
                emit(args, &r)?;
                Ok(format!("[nr tolerance] {}", r.verdict))
            })())
        }
        Some("agnss-scaffold") => {
            let text = serde_json::to_string_pretty(&agnss::scaffold()).unwrap_or_default() + "\n";
            match super::flag(args, "--out") {
                Some(o) => {
                    if let Err(e) = std::fs::write(&o, text) {
                        return fail(format!("{o}: {e}"));
                    }
                    eprintln!("[nr agnss-scaffold] placeholder vector set written to {o}");
                }
                None => print!("{text}"),
            }
            ExitCode::SUCCESS
        }
        Some("agnss-status") => {
            let Some(path) = arg1(args) else {
                return fail(NR_USAGE);
            };
            finish((|| {
                let set: agnss::VectorSet =
                    serde_json::from_str(&read(&path)?).map_err(|e| format!("{path}: {e}"))?;
                let s = agnss::status(&set).map_err(|e| format!("{path}: {e}"))?;
                emit(args, &s)?;
                Ok(format!(
                    "[nr agnss-status] {} placeholder(s); {} of {} case(s) scorable",
                    s.placeholders,
                    s.cases.iter().filter(|c| c.scorable).count(),
                    s.cases.len()
                ))
            })())
        }
        Some("agnss-score") => {
            let (Some(v), Some(d)) = (
                arg1(args),
                args.get(2).filter(|a| !a.starts_with("--")).cloned(),
            ) else {
                return fail(NR_USAGE);
            };
            if let Err(e) = gate(args, tolerance::FEATURE) {
                return fail(e);
            }
            finish((|| {
                let rule = match super::flag(args, "--rule").as_deref() {
                    None | Some("point") => tolerance::Decision::Point,
                    Some("lower-bound") => tolerance::Decision::LowerBound,
                    Some(o) => return Err(format!("--rule '{o}': point or lower-bound")),
                };
                let set: agnss::VectorSet =
                    serde_json::from_str(&read(&v)?).map_err(|e| format!("{v}: {e}"))?;
                let r = agnss::score(&set, &Table::read(Path::new(&d))?, rule)
                    .map_err(|e| format!("{v}: {e}"))?;
                emit(args, &r)?;
                Ok(format!("[nr agnss-score] {}", r.verdict))
            })())
        }
        Some(sub @ ("tdd" | "prs")) => {
            let Some(path) = arg1(args) else {
                return fail(NR_USAGE);
            };
            if let Err(e) = gate(args, nr::FEATURE) {
                return fail(e);
            }
            finish((|| {
                if sub == "tdd" {
                    let (req, _): (nr::TddRequest, _) = parse(&path)?;
                    let utc = super::flag(args, "--utc").unwrap_or_else(super::utc_iso8601_now);
                    let r = nr::tdd(&req, &utc)?;
                    emit(args, &r)?;
                    Ok(format!("[nr tdd] {}", r.verdict))
                } else {
                    let (req, _): (nr::PrsRequest, _) = parse(&path)?;
                    let r = nr::prs(&req)?;
                    emit(args, &r)?;
                    Ok(format!("[nr prs] {}", r.verdict))
                }
            })())
        }
        _ => fail(NR_USAGE),
    }
}

/// `kshana-pro rail ...`.
pub fn rail(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    if !matches!(sub, Some("thr" | "balise")) {
        return fail(RAIL_USAGE);
    }
    let Some(path) = arg1(args) else {
        return fail(RAIL_USAGE);
    };
    if let Err(e) = gate(args, rail::FEATURE) {
        return fail(e);
    }
    let utc = super::flag(args, "--utc").unwrap_or_else(super::utc_iso8601_now);
    finish((|| {
        if sub == Some("thr") {
            let (req, src): (rail::ThrRequest, _) = parse(&path)?;
            let r = rail::thr(&req, &utc)?;
            let tail = json_or_signed(
                args,
                "rail thr",
                &r,
                || rail::thr_html(&r),
                "thr.toml",
                &src,
            )?;
            Ok(format!("[rail thr] {}{tail}", r.verdict))
        } else {
            let (req, src): (rail::BaliseRequest, _) = parse(&path)?;
            let r = rail::balise(&req, &base_of(&path), &utc)?;
            let tail = json_or_signed(
                args,
                "rail balise",
                &r,
                || rail::balise_html(&r),
                "balise.toml",
                &src,
            )?;
            Ok(format!("[rail balise] {}{tail}", r.verdict))
        }
    })())
}
