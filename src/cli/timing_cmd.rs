// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro timing <assess|console|metrics|history|scorecard|compliance|audit|attack|timebomb|selfcheck|holdover-evidence|posture|chain|crosscheck|station|clock-bench>`
//! (roadmap 0.42 T1-T3; W1119 C4, C8; W1120 TA1-TA4; E19 in `timing_net_cmd`).
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
    compliance list | compliance <mifid2-rts25|dora-rts-2024-1774|eu-cer|india-ist-draft|itu-g8271-1> <inputs.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Signed annual timing evidence pack, requirement -> evidence -> verdict (Pro); verify with `kshana-pro dossier verify` (free)\n\
    audit <fleet.toml> --profile <rts25-traceability|nist-ir-8323-profile> --out <dir> --signer <name> --key-file <file> [--evidence <dir>] [--operator <operator.toml>] [--utc <iso>] [--license <token>] [--dev]   Signed audit report mapped to the profile (Pro); verify with `kshana-pro dossier verify` (free)\n\
    recovery <request.toml> --out <dir> --signer <name> --key-file <file> [--dev] | recovery-synth <dir>   Timing-module attack recovery from time-error logs: recovery time, residual, holdover exit step, multi-receiver comparison, signed (Pro, T4)\n\
    attack <scenario.toml|prtc|eprtc|substation> --out <dir> --signer <name> --key-file <file> [--dev]   Holdover under a GNSS drag-and-step spoof: detector, fallback, time error against masks, signed (Pro; `kshana-pro timing attack` for details)\n\
    timebomb vectors --out <dir> [--only <id,id>] [--leap-date <YYYY-MM-DD>] [--pivot <YYYY-MM-DD>]... [--at <iso>]... [--pre <s>] [--post <s>] [--lat <deg> --lon <deg> --h <m>] [--utc <iso>] [--license <token>] [--dev]   Future-epoch replay vectors (NMEA, UBX, simulator sheet, expected output) for receivers and grandmasters (Pro)\n\
    timebomb analyse <vector-dir> <capture> --format <nmea|ubx|csv> [--start-index <k>] [--replay-start <lab unix>] [--json <out>] [--strict] [--license <token>] [--dev]   Faults in a device's output during a replay: time jumps, week rollover, century, 32-bit wrap, leap-second handling, stuck time (Pro)\n\
    timebomb fleet <inventory.toml> [--advisories <table.toml>]... [--no-seed] [--utc <iso>] [--json <out>] [--csv <out>] [--license <token>] [--dev]   Every device against the epoch hazards and the vendor-advisory table (Pro)\n\
    timebomb advisories                                                       Print the seed advisory table (no licence needed)\n\
    selfcheck node <node.toml> [--json <out>] [--csv <bins.csv>] [--license <token>] [--dev]   Vote a node's time against its independent sources: wrong time while GNSS looks healthy (Pro)\n\
    selfcheck sites <sites.toml> [--json <out>] [--license <token>] [--dev]   One site or operator diverging from the others; common-mode shifts (Enterprise)\n\
    holdover-evidence <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Holdover time-error growth with a GUM budget against a limit for a period (e.g. Finland's two weeks), signed (Pro) | holdover-evidence presets\n\
    posture check <inventory.toml> [--advisories <table.toml>]... [--components <table.toml>]... [--no-seed] [--utc <iso>] [--json <out>] [--csv <out>] [--license <token>] [--dev]   Appliance security posture: advisories, vendor SBOM components, management reach, services, firmware integrity (Pro)\n\
    posture firmware <image> (--sha256 <hex> | --sums <SHA256SUMS> | --sig <file> --pubkey <hex>) [--vendor <v>] [--model <m>] [--version <v>] [--json <out>] [--utc <iso>] [--license <token>] [--dev]   Check a firmware image against the vendor's digest or Ed25519 signature (Pro)\n\
    posture sbom station <components.toml> [--out <bom.cdx.json>] [--utc <iso>] [--license <token>] [--dev]   CycloneDX 1.5 SBOM of a station's firmware (Pro)\n\
    posture sbom check <bom.json>                                              Structural check of a CycloneDX JSON SBOM (no licence needed)\n\
    chain <chain.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Time-distribution chain from UTC(k) over WR/PTP/fibre/common-view hops: GUM budget, calibration currency, onboarding test, signed traceability statement (Pro) | chain template\n\
    crosscheck <request.toml> [--json <out>] [--csv <bins.csv>] [--license <token>] [--dev]   Multi-source cross-check grouped by traceability root (GNSS, long-wave, BPS, eLoran, LEO, fibre, NTS): single-source fault isolation, node-off, silent sources, triad monitor (Pro) | crosscheck profile <wisp|timing-node>\n\
    station <station.toml> [--json <out>] [--csv <passes.csv>] [--out <dir> --signer <name> --key-file <file>] [--utc <iso>] [--license <token>] [--dev]   Ground-station pass-quality SLA and station timing during passes, optionally signed (Pro)\n\
    clock-bench register <bench.toml> [--json <protocol.json>] | clock-bench score <bench.toml> [--json <out>] [--csv <out>] [--out <dir> --signer <name> --key-file <file>] [--utc <iso>] [--license <token>] [--dev]   Neutral clock benchmark under a pre-registered protocol: ADEV with intervals, TDEV, MTIE, holdover test, environmental sensitivity, datasheet claims, reference floor; no ranking (Pro; register is free)";

pub(super) fn fail(msg: impl std::fmt::Display) -> ExitCode {
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
        (Some("attack"), _) => super::pnt_cmd::attack(args),
        (Some("attack-template"), _) => super::pnt_cmd::attack_template(args),
        (Some("recovery"), _) => super::receiver_cmd::recovery(args),
        (Some("recovery-synth"), _) => super::receiver_cmd::recovery_synth(args),
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
        (Some("timebomb"), Some(x)) => timebomb_cmd(args, &x),
        (Some("selfcheck"), Some(x)) => selfcheck_cmd(args, &x),
        (Some("holdover-evidence"), Some(x)) => holdover_evidence_cmd(args, &x),
        (Some("posture"), Some(x)) => super::timing_net_cmd::posture(args, &x),
        (Some("chain"), Some(x)) => super::timing_net_cmd::chain(args, &x),
        (Some("crosscheck"), Some(x)) => super::timing_net_cmd::crosscheck(args, &x),
        (Some("station"), Some(x)) => super::timing_net_cmd::station(args, &x),
        (Some("clock-bench"), Some(x)) => super::clock_bench_cmd::clock_bench(args, &x),
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

/// Every value of a repeated `--flag value`.
pub(super) fn flags_all(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == name)
        .map(|w| w[1].clone())
        .collect()
}

pub(super) fn write_or_print(out: Option<String>, text: &str) -> Result<(), String> {
    match out {
        Some(o) => std::fs::write(&o, text).map_err(|e| format!("{o}: {e}")),
        None => {
            println!("{text}");
            Ok(())
        }
    }
}

pub(super) fn utc_unix(args: &[String]) -> Result<(i64, String), String> {
    use crate::timing::timebomb::Label;
    match flag(args, "--utc") {
        Some(u) => Ok((Label::parse_iso(&u)?.seconds(), u)),
        None => Ok((now_unix(), utc_iso8601_now())),
    }
}

/// `timing timebomb <vectors|analyse|fleet|advisories>` (W1120 TA1).
fn timebomb_cmd(args: &[String], sub: &str) -> ExitCode {
    use crate::timing::timebomb::{self as tb, Label};
    if sub == "advisories" {
        print!("{}", tb::SEED_ADVISORIES);
        return ExitCode::SUCCESS;
    }
    if let Err(e) = gate(args, tb::FEATURE) {
        return fail(e);
    }
    let res: Result<(), String> = (|| match sub {
        "vectors" => {
            let out = flag(args, "--out").ok_or("vectors needs --out <dir>")?;
            let (now, _) = utc_unix(args)?;
            let num = |n: &str, d: f64| -> Result<f64, String> {
                flag(args, n)
                    .map(|v| v.parse::<f64>().map_err(|e| format!("{n}: {e}")))
                    .unwrap_or(Ok(d))
            };
            let mut o = tb::CatalogueOptions {
                now_unix: now,
                pre_s: num("--pre", 120.0)? as i64,
                post_s: num("--post", 180.0)? as i64,
                ..Default::default()
            };
            if o.pre_s < 1 || o.post_s < 1 || o.pre_s + o.post_s > 86_400 {
                return Err("--pre and --post must be >= 1 s and together at most one day".into());
            }
            if let Some(d) = flag(args, "--leap-date") {
                let l = Label::parse_iso(&d)?;
                let (y, m, _) = l.ymd();
                let last = kshana_eloran::time::days_from_civil(
                    if m == 12 { y + 1 } else { y },
                    if m == 12 { 1 } else { m + 1 },
                    1,
                ) - 1;
                if l.days != last {
                    eprintln!("note: {d} is not the last day of a month (where IERS puts leap seconds); used as given");
                }
                o.leap_day = l;
            }
            for p in flags_all(args, "--pivot") {
                o.pivots.push(Label::parse_iso(&p)?);
            }
            for c in flags_all(args, "--at") {
                o.custom.push(Label::parse_iso(&c)?);
            }
            let pos = tb::Position {
                lat_deg: num("--lat", tb::Position::default().lat_deg)?,
                lon_deg: num("--lon", tb::Position::default().lon_deg)?,
                h_m: num("--h", tb::Position::default().h_m)?,
            };
            let mut specs = tb::catalogue(&o);
            if let Some(only) = flag(args, "--only") {
                let want: Vec<&str> = only.split(',').map(str::trim).collect();
                specs.retain(|s| {
                    want.iter()
                        .any(|w| s.id == *w || s.id.starts_with(&format!("{w}-")))
                });
                if specs.is_empty() {
                    return Err(format!(
                        "--only {only}: no vector (built-in ids: {})",
                        tb::BUILTIN_IDS.join(", ")
                    ));
                }
            }
            tb::generate(Path::new(&out), &specs, &pos)?;
            for s in &specs {
                eprintln!(
                    "{:<28} boundary {}{}",
                    s.id,
                    s.boundary.iso(),
                    if s.synthetic {
                        "  (synthetic date)"
                    } else {
                        ""
                    }
                );
            }
            eprintln!("[wrote {} vectors and manifest.json to {out}]", specs.len());
            Ok(())
        }
        "analyse" | "analyze" => {
            let (Some(dir), Some(cap)) = (
                args.get(2).filter(|a| !a.starts_with("--")),
                args.get(3).filter(|a| !a.starts_with("--")),
            ) else {
                return Err("analyse needs <vector-dir> <capture>".into());
            };
            let fmt = flag(args, "--format").ok_or("analyse needs --format <nmea|ubx|csv>")?;
            let spec = tb::read_vector(Path::new(dir))?;
            let bytes = std::fs::read(cap).map_err(|e| format!("{cap}: {e}"))?;
            let (recs, bad) = tb::read_capture(&fmt, &bytes)?;
            let pairing = tb::Pairing {
                start_k: flag(args, "--start-index")
                    .map(|v| {
                        v.parse::<usize>()
                            .map_err(|e| format!("--start-index: {e}"))
                    })
                    .transpose()?
                    .unwrap_or(0),
                replay_start_lab: flag(args, "--replay-start")
                    .map(|v| v.parse::<f64>().map_err(|e| format!("--replay-start: {e}")))
                    .transpose()?,
            };
            let a = tb::analyse_with(&spec, &recs, &fmt, bad, pairing);
            write_or_print(
                flag(args, "--json"),
                &serde_json::to_string_pretty(&a).expect("serialises"),
            )?;
            eprintln!(
                "[{}] {} records, {} paired: {}",
                a.vector, a.device_records, a.paired, a.verdict
            );
            for f in &a.faults {
                eprintln!(
                    "  {:<18} at {} device {}: {}",
                    f.kind, f.expected_utc, f.device_utc, f.detail
                );
            }
            for n in &a.notes {
                eprintln!("  note: {n}");
            }
            if has_flag(args, "--strict") && !a.faults.is_empty() {
                return Err("faults found (--strict)".into());
            }
            Ok(())
        }
        "fleet" => {
            let path = args
                .get(2)
                .filter(|a| !a.starts_with("--"))
                .ok_or("fleet needs <inventory.toml>")?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let inv: tb::Inventory = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
            let mut table = if has_flag(args, "--no-seed") {
                tb::AdvisoryTable::default()
            } else {
                tb::AdvisoryTable::seed()
            };
            for extra in flags_all(args, "--advisories") {
                let t = std::fs::read_to_string(&extra).map_err(|e| format!("{extra}: {e}"))?;
                table.advisory.extend(
                    tb::AdvisoryTable::parse(&t)
                        .map_err(|e| format!("{extra}: {e}"))?
                        .advisory,
                );
            }
            let (now, utc) = utc_unix(args)?;
            let r = tb::fleet_check(&inv, &table, now, &utc)?;
            write_or_print(
                flag(args, "--json"),
                &serde_json::to_string_pretty(&r).expect("serialises"),
            )?;
            if let Some(c) = flag(args, "--csv") {
                std::fs::write(&c, tb::fleet_csv(&r)).map_err(|e| format!("{c}: {e}"))?;
            }
            for d in &r.devices {
                let top = d.findings.first();
                eprintln!(
                    "{:<16} {:<6} {}",
                    d.id,
                    d.worst,
                    top.map(|f| format!(
                        "{} {} {}",
                        f.id,
                        f.status,
                        f.date.as_deref().unwrap_or("")
                    ))
                    .unwrap_or_default()
                );
            }
            Ok(())
        }
        _ => Err(USAGE.into()),
    })();
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}

/// `timing selfcheck <node|sites>` (W1120 TA2).
fn selfcheck_cmd(args: &[String], sub: &str) -> ExitCode {
    use crate::timing::selfcheck as sc;
    let feature = match sub {
        "node" => sc::FEATURE,
        "sites" => sc::CORRELATION_FEATURE,
        _ => return fail(USAGE),
    };
    if let Err(e) = gate(args, feature) {
        return fail(e);
    }
    let Some(path) = args.get(2).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let res: Result<(), String> = (|| {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let base = Path::new(path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        if sub == "node" {
            let spec: sc::NodeSpec = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
            let r = sc::check_node(&spec, &base)?;
            write_or_print(
                flag(args, "--json"),
                &serde_json::to_string_pretty(&r).expect("serialises"),
            )?;
            if let Some(c) = flag(args, "--csv") {
                std::fs::write(&c, sc::node_bins_csv(&r)).map_err(|e| format!("{c}: {e}"))?;
            }
            eprintln!("[{}] {}", r.node, r.verdict);
            for e in &r.events {
                eprintln!("  {:<20} {} bins: {}", e.status, e.bins, e.detail);
            }
        } else {
            let spec: sc::SitesSpec = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
            let r = sc::check_sites(&spec, &base)?;
            write_or_print(
                flag(args, "--json"),
                &serde_json::to_string_pretty(&r).expect("serialises"),
            )?;
            eprintln!("[sites] {}", r.verdict);
            for e in &r.events {
                eprintln!("  {:<16} {:<15} {}", e.kind, e.scope, e.detail);
            }
        }
        Ok(())
    })();
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}

/// `timing holdover-evidence <request.toml> ...` (W1120 TA3).
fn holdover_evidence_cmd(args: &[String], path: &str) -> ExitCode {
    use crate::timing::holdover_evidence as he;
    if path == "presets" {
        for (n, p, l, b) in he::PRESETS {
            println!("{n:<14} {p} h, limit {l:e} s\n{:<14} {b}", "");
        }
        for (n, v, b) in he::EQUIPMENT_CLASSES {
            println!("equipment_class {n:<13} {v:e} s  {b}");
        }
        return ExitCode::SUCCESS;
    }
    if let Err(e) = gate(args, he::FEATURE) {
        return fail(e);
    }
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return fail("holdover-evidence needs --out, --signer and --key-file");
    };
    let res: Result<String, String> = (|| {
        let seed = super::forensics_cmd::read_seed(&kf)?;
        let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let spec = he::HoldoverSpec::parse(&src)?;
        let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
        let r = he::assess(&spec, &utc)?;
        let dir = Path::new(&out);
        he::write_dir(dir, &r, &src)?;
        crate::signed_dossier::sign(dir, &seed, &signer, &utc).map_err(|e| e.to_string())?;
        Ok(r.verdict)
    })();
    match res {
        Ok(v) => {
            eprintln!("[holdover evidence] {v}; signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
