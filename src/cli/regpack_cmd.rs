// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro regpack ...`: regulator, type-approval and framework pre-assessment packs
//! (E29). `assess`, `bam`, `mass` and `links` need `pro-regulator-pack`; `complaint` needs
//! `pro-interference-complaint`; `bid` needs `pro-bid-attestation`; `list`, `show`,
//! `example` and `verify` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::regpacks::{self, bam, bid, complaint, links, mass};

use super::evidence_cmd::{producing, sign};
use super::flag;
use super::forensics_cmd::keys;

pub const USAGE: &str = "usage: kshana-pro regpack <command>\n\
    list\n\
        The requirement catalogues (free)\n\
    show <id>\n\
        One catalogue as JSON (free)\n\
    example <subject|bam|mass|complaint|links|bid|bom>\n\
        Print an example input (free)\n\
    assess <id> [--evidence <dir>[,<dir>...]] [--standards-report <report.json>] [--subject <subject.toml>] --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Pre-assessment: which signed Kshana evidence speaks to each requirement and what is missing (Pro)\n\
    bam <alert-map.toml> (--pntdp <pntdp-run.json> | --pntdp-vector <name>) --out <dir> --signer <name> --key-file <file> [...]\n\
        Bridge alert mapping check of PNT-integrity alerts, replayed over PNT data processing output (Pro)\n\
    mass <mass.toml> --out <dir> --signer <name> --key-file <file> [...]\n\
        MASS Code override independence over the declared architecture and the OE PNT template (Pro)\n\
    links <links.toml> --out <dir> --signer <name> --key-file <file> [...]\n\
        Space link-security evidence from the declared link and command configuration (Pro)\n\
    complaint <request.toml> [--episodes <episodes.json>] [--evidence <dirs>] --out <dir> --signer <name> --key-file <file> [...]\n\
        Harmful-interference report annex under the headings of ITU RR Appendix 10; effects and 0.5-degree areas only (Pro)\n\
    bid <bid.toml> --bom <bom.csv> [--evidence <dirs>] --out <dir> --signer <name> --key-file <file> [...]\n\
        Signed attestation of the declared export status of a bill of materials and its test evidence chain (Pro)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a pack's signature (and a bid attestation's chain), offline (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn read(p: &str) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))
}

fn arg1(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

fn evidence(args: &[String], trust: &[[u8; 32]]) -> Result<Vec<regpacks::EvidenceItem>, String> {
    flag(args, "--evidence")
        .map(|l| {
            regpacks::paths(&l)
                .iter()
                .map(|d| regpacks::read_evidence(d, trust))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// Run a producing command: licence, inputs, build, write, sign, report.
fn produce(
    args: &[String],
    gate: &str,
    tag: &str,
    body: impl FnOnce(&Path, &[[u8; 32]]) -> Result<String, String>,
) -> ExitCode {
    let r = producing(args, gate).and_then(|(out, seed, signer, utc)| {
        let trust = keys(args, "--trust")?;
        let msg = body(Path::new(&out), &trust)?;
        sign(&out, &seed, &signer, &utc)?;
        Ok(format!("{msg}; signed in {out}"))
    });
    match r {
        Ok(m) => {
            eprintln!("[regpack {tag}] {m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("list") => match regpacks::catalogues() {
            Ok(cs) => {
                for c in cs {
                    println!(
                        "{:<20} {:<14} {} ({} rows)",
                        c.id,
                        c.kind,
                        c.title,
                        c.rows.len()
                    );
                }
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        Some("show") => match arg1(args).map(|id| regpacks::catalogue(id)) {
            Some(Ok(c)) => {
                println!("{}", serde_json::to_string_pretty(&c).expect("plain data"));
                ExitCode::SUCCESS
            }
            Some(Err(e)) => fail(e),
            None => fail(USAGE),
        },
        Some("example") => match arg1(args).and_then(|n| regpacks::example(n)) {
            Some(t) => {
                print!("{t}");
                ExitCode::SUCCESS
            }
            None => fail(USAGE),
        },
        Some("assess") => {
            let Some(id) = arg1(args) else {
                return fail(USAGE);
            };
            produce(args, regpacks::FEATURE, "assess", |out, trust| {
                let c = regpacks::catalogue(id)?;
                let mut ev = evidence(args, trust)?;
                if let Some(r) = flag(args, "--standards-report") {
                    ev.push(regpacks::read_standards_report(Path::new(&r))?);
                }
                let subject = match flag(args, "--subject") {
                    Some(p) => regpacks::Subject::parse(&read(&p)?)?,
                    None => regpacks::Subject::default(),
                };
                let p = regpacks::assess(&c, subject, ev);
                regpacks::write_dir(out, &p)?;
                Ok(format!(
                    "{}: {} rows with evidence supplied, {} with no evidence supplied, {} not assessed by Kshana",
                    p.catalogue, p.counts[0], p.counts[1], p.counts[2]
                ))
            })
        }
        Some("bam") => {
            let Some(file) = arg1(args) else {
                return fail(USAGE);
            };
            produce(args, regpacks::FEATURE, "bam", |out, _| {
                let map_text = read(file)?;
                let m = bam::AlertMap::parse(&map_text)?;
                let (epochs, text) = match (flag(args, "--pntdp"), flag(args, "--pntdp-vector")) {
                    (Some(p), _) => {
                        let t = read(&p)?;
                        (bam::parse_epochs(&t)?, t)
                    }
                    (None, Some(name)) => {
                        let v = crate::pntdp::vectors::all()
                            .into_iter()
                            .find(|v| v.name == name)
                            .ok_or_else(|| format!("unknown PNT-DP vector '{name}'"))?;
                        let cfg = crate::pntdp::Config::parse(&v.config)?;
                        let o = crate::pntdp::run(&cfg, &v.samples)?;
                        (bam::epochs_from(&o), format!("pntdp vector {name}"))
                    }
                    _ => return Err("bam needs --pntdp <file> or --pntdp-vector <name>".into()),
                };
                let b = bam::run(&m, &map_text, &epochs, &text);
                bam::write_dir(out, &b)?;
                Ok(format!(
                    "{} map gaps; {} epochs; {} onsets with no alert",
                    b.gaps, b.epochs, b.unmapped_onsets
                ))
            })
        }
        Some("mass") => {
            let Some(file) = arg1(args) else {
                return fail(USAGE);
            };
            produce(args, regpacks::FEATURE, "mass", |out, _| {
                let src = read(file)?;
                let m = mass::run(&mass::MassDecl::parse(&src)?, &src);
                mass::write_dir(out, &m)?;
                Ok(format!(
                    "override GNSS-independent as declared: {}; independent of controlled systems: {}; {} chains",
                    m.override_gnss_independent,
                    m.override_independent_of_controlled,
                    m.chains.len()
                ))
            })
        }
        Some("links") => {
            let Some(file) = arg1(args) else {
                return fail(USAGE);
            };
            produce(args, regpacks::FEATURE, "links", |out, _| {
                let src = read(file)?;
                let s = links::run(&links::LinkDecl::parse(&src)?, &src);
                links::write_dir(out, &s)?;
                Ok(format!(
                    "{} gaps; {} cleartext links; policy {}",
                    s.gaps,
                    s.cleartext_links.len(),
                    s.no_encryption_no_fly
                ))
            })
        }
        Some("complaint") => {
            let Some(file) = arg1(args) else {
                return fail(USAGE);
            };
            produce(
                args,
                regpacks::FEATURE_COMPLAINT,
                "complaint",
                |out, trust| {
                    let src = read(file)?;
                    let r = complaint::Request::parse(&src)?;
                    let ep_text = flag(args, "--episodes").map(|p| read(&p)).transpose()?;
                    let ep = match &ep_text {
                        Some(t) => Some((t.as_str(), complaint::observations_from_episodes(t)?)),
                        None => None,
                    };
                    let c = complaint::build(&r, &src, ep, evidence(args, trust)?)?;
                    complaint::write_dir(out, &c)?;
                    Ok(format!(
                        "{} area cells, {} to {}",
                        c.areas.len(),
                        c.period_utc[0],
                        c.period_utc[1]
                    ))
                },
            )
        }
        Some("bid") => {
            let Some(file) = arg1(args) else {
                return fail(USAGE);
            };
            produce(args, regpacks::FEATURE_BID, "bid", |out, trust| {
                let b = bid::Bid::parse(&read(file)?)?;
                let bom = read(&flag(args, "--bom").ok_or("bid needs --bom <bom.csv>")?)?;
                let a = bid::build(b, &bom, evidence(args, trust)?)?;
                bid::write_dir(out, &a, &bom)?;
                Ok(a.summary.clone())
            })
        }
        Some("verify") => {
            let Some(dir) = arg1(args) else {
                return fail(USAGE);
            };
            let trust = match keys(args, "--trust") {
                Ok(k) => k,
                Err(e) => return fail(e),
            };
            let d = Path::new(dir);
            let v = match crate::signed_dossier::verify(d, &trust) {
                Ok(v) => v,
                Err(e) => return fail(e),
            };
            let mut problems = v.problems.clone();
            if d.join(bid::RESULT_FILE).exists() {
                match bid::check_dir(d) {
                    Ok(p) => problems.extend(p),
                    Err(e) => problems.push(e),
                }
            }
            println!(
                "signer: {} ({}), {} files, signature {}, key {}",
                v.signer,
                v.key_id,
                v.files,
                if v.signature_ok { "valid" } else { "INVALID" },
                match v.trusted {
                    Some(true) => "trusted",
                    Some(false) => "NOT trusted",
                    None => "not pinned (give --trust)",
                }
            );
            for p in &problems {
                println!("  problem: {p}");
            }
            if v.passed() && problems.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        _ => fail(USAGE),
    }
}
