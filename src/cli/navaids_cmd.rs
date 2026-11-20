// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro navaids <coverage|rnp|inspect|ssv>` and the E37 eLoran / R-Mode subcommands
//! reached through `kshana-pro eloran` (`rmode-perf`, `wwrns-trust`, `claim`, `interop`,
//! `sol-evidence`); research 2026-11-20 gaps E37, docs/ELORAN-RMODE-NAVAIDS.md.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::eloran_claims;
use crate::license::{self, Entitlement, Product};
use crate::multi_source::wwrns;
use crate::navaids::{self, inspection, rnp};

use super::{flag, has_flag, license_cmd, now_unix, utc_iso8601_now};

/// R-Mode receiver performance tests against a supplied limits profile.
pub const FEATURE_RMODE_PERF: &str = "pro-rmode-perf";
/// Position-domain multi-source trust for WWRNS users.
pub const FEATURE_WWRNS: &str = "pro-wwrns-trust";
/// eLoran timing-claim verification and the interoperability test.
pub const FEATURE_CLAIMS: &str = "pro-eloran-claims";
/// Signed safety-of-life evidence index.
pub const FEATURE_SOL: &str = "pro-eloran-sol-evidence";

pub const USAGE: &str = "usage: kshana-pro navaids <command>\n\
    coverage <request.toml> [--json <out>] [--csv <grid.csv>] [--license <token>] [--dev]\n\
        DME/VOR network twin at a flight level: line of sight, service volumes or DOC, ICAO Doc 9613\n\
        DME/DME geometry and error, VOR/DME, critical DMEs (single outages that remove DME/DME) (Pro)\n\
    rnp <request.toml> [--json <out>] [--license <token>] [--dev]\n\
        Multi-sensor RNP scenarios along a route: GNSS off, facilities out, inertial coast, a declared\n\
        backup source; TSE against RNP, monitoring alert, ADS-B NACp for the e-conspicuity backup test (Pro)\n\
    inspect <request.toml> [--json <out>] [--license <token>] [--dev]\n\
        Flight-inspection truth assurance: consistent, facility deviation, truth suspect (common offset)\n\
        or unresolved per epoch, from DME ranges and VOR radials against the inspection truth (Pro)\n\
    ssv\n\
        The VOR standard service volumes used (FAA AIM 1-1-8, classic T/L/H; free)";

pub const ELORAN_USAGE: &str = "E37 subcommands of kshana-pro eloran:\n\
    rmode-perf <limits.toml> (--records <records.csv|.json> | --simulate <plan.toml>) --out <report.json> [--records-out <records.csv>] [--license <token>] [--dev]\n\
        Draft R-Mode receiver performance tests (RMP-01..08) against the limits the profile supplies\n\
        (none built in; the NCSR 13 draft is not adopted): pre-assessment with confidence bounds (Pro)\n\
    wwrns-trust <config.toml> --fixes <fixes.csv> --out <report.json> [--license <token>] [--dev]\n\
        GNSS, R-Mode and eLoran fixes: per-epoch trust decision (F3 vote rule in the position domain),\n\
        GNSS-suspect epochs, accuracy against the stated requirement (Pro)\n\
    claim <request.toml> --out <report.json> [--license <token>] [--dev]\n\
        Timing-claim verification (e.g. 20 ns to UTC): GUM budget with Circular T, guard-banded mean and\n\
        95 % instantaneous decisions (Pro)\n\
    interop <request.toml> --out <report.json> [--license <token>] [--dev]\n\
        Interoperability test across stations steered to different UTC(k): E_n per pair, Birge ratio (Pro)\n\
    sol-evidence <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Safety-of-life evidence index over the supplied artefacts, copied and signed (Enterprise)\n\
    ldc-fde <request.toml> --out <report.json> --license <machine-bound token>\n\
        LDC/ASF fault detection and exclusion on TOA residuals (patent-pending; on-prem builds only)";

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

fn base_of(path: &str) -> PathBuf {
    Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn parse<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    if path.ends_with(".json") {
        serde_json::from_str(&src).map_err(|e| format!("{path}: {e}"))
    } else {
        toml::from_str(&src).map_err(|e| format!("{path}: {e}"))
    }
}

fn write(path: &str, text: &str) -> Result<(), String> {
    let p = Path::new(path);
    if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(p, text).map_err(|e| format!("{path}: {e}"))
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).expect("serialisable")
}

fn emit<T: serde::Serialize>(args: &[String], key: &str, v: &T) -> Result<(), String> {
    match flag(args, key) {
        Some(o) => write(&o, &json(v)),
        None => {
            println!("{}", json(v));
            Ok(())
        }
    }
}

fn first_path(args: &[String], usage: &str) -> Result<String, String> {
    args.get(1)
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .ok_or_else(|| usage.to_string())
}

/// `kshana-pro navaids ...`.
pub fn run(args: &[String]) -> ExitCode {
    let res: Result<String, String> = match args.first().map(String::as_str) {
        Some("ssv") => {
            let mut t = String::from(
                "VOR standard service volumes (FAA AIM 1-1-8 Table 1-1-1, classic; altitudes above the facility):\n",
            );
            for (c, rows) in [
                ("T", "1,000-12,000 ft: 25 NM"),
                ("L", "1,000-18,000 ft: 40 NM"),
                (
                    "H",
                    "1,000-14,500 ft: 40 NM; 14,500-60,000 ft: 100 NM; 18,000-45,000 ft: 130 NM",
                ),
            ] {
                t += &format!("  {c}: {rows}\n");
            }
            t += "Check against the AIM edition in force; a facility's own DOC overrides these.";
            Ok(t)
        }
        Some(sub @ ("coverage" | "rnp" | "inspect")) => (|| {
            let path = first_path(args, USAGE)?;
            gate(args, navaids::FEATURE)?;
            match sub {
                "coverage" => {
                    let req: navaids::CoverageRequest = parse(&path)?;
                    let r = navaids::coverage(&req)?;
                    if let Some(c) = flag(args, "--csv") {
                        write(&c, &navaids::coverage_csv(&r))?;
                    }
                    let crit = r
                        .critical
                        .first()
                        .filter(|c| c.loss_fraction > 0.0)
                        .map(|c| {
                            format!(
                                "; most critical DME {} ({:.1} % of the area loses DME/DME)",
                                c.id,
                                c.loss_fraction * 100.0
                            )
                        })
                        .unwrap_or_default();
                    let msg = format!(
                        "[navaids coverage] {} at {} ft: DME/DME {:.1} %, VOR/DME {:.1} %, either {:.1} % of {} points{crit} (MODELLED)",
                        r.name,
                        r.altitude_ft,
                        r.dme_dme_fraction * 100.0,
                        r.vor_dme_fraction * 100.0,
                        r.any_fraction * 100.0,
                        r.points
                    );
                    emit(args, "--json", &r)?;
                    Ok(msg)
                }
                "rnp" => {
                    let req: rnp::RnpRequest = parse(&path)?;
                    let r = rnp::run(&req)?;
                    let mut t = format!(
                        "[navaids rnp] {} ({} {} NM):",
                        r.route, r.nav_spec, r.rnp_nm
                    );
                    for s in &r.scenarios {
                        t += &format!(
                            "\n  {:<28} meets RNP {:5.1} %, alert {:5.1} %, min NACp {}{}",
                            s.name,
                            s.meets_rnp_fraction * 100.0,
                            s.alert_fraction * 100.0,
                            s.nacp_min,
                            s.first_alert_nm
                                .map(|a| format!(", first alert at {a:.0} NM"))
                                .unwrap_or_default()
                        );
                    }
                    emit(args, "--json", &r)?;
                    Ok(t)
                }
                _ => {
                    let req: inspection::InspectionRequest = parse(&path)?;
                    let base = base_of(&path);
                    let rd = |f: &str| {
                        let p = base.join(f);
                        std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
                    };
                    let track = inspection::parse_track(&rd(&req.track)?)?;
                    let meas = inspection::parse_measurements(&rd(&req.measurements)?)?;
                    let r = inspection::assess(&req, &track, &meas)?;
                    emit(args, "--json", &r)?;
                    Ok(format!(
                        "[navaids inspect] {} epochs: {} consistent, {} truth suspect, {} facility deviations, {} unresolved",
                        r.epochs,
                        r.consistent,
                        r.truth_suspect,
                        r.facility_deviation.values().sum::<usize>(),
                        r.unresolved
                    ))
                }
            }
        })(),
        _ => Err(USAGE.into()),
    };
    match res {
        Ok(t) => {
            eprintln!("{t}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// The E37 subcommands of `kshana-pro eloran`.
pub fn eloran(args: &[String]) -> ExitCode {
    let res: Result<String, String> = (|| {
        let sub = args.first().map(String::as_str).unwrap_or("");
        let path = first_path(args, ELORAN_USAGE)?;
        let base = base_of(&path);
        match sub {
            "rmode-perf" => {
                use kshana_eloran::rmode::perf;
                gate(args, FEATURE_RMODE_PERF)?;
                let out = flag(args, "--out").ok_or("--out <report.json> is required")?;
                let lim: perf::Limits = parse(&path)?;
                let recs = match (flag(args, "--records"), flag(args, "--simulate")) {
                    (Some(r), None) => perf::parse_records(
                        &std::fs::read_to_string(&r).map_err(|e| format!("{r}: {e}"))?,
                    )?,
                    (None, Some(p)) => perf::simulate(&parse::<perf::SimPlan>(&p)?)?,
                    _ => {
                        return Err("give one of --records <file> or --simulate <plan.toml>".into())
                    }
                };
                if let Some(ro) = flag(args, "--records-out") {
                    write(&ro, &perf::records_csv(&recs))?;
                }
                let r = perf::assess(&recs, &lim)?;
                write(&out, &json(&r))?;
                let mut t = format!(
                    "R-Mode performance pre-assessment ({}; limits {}): {}",
                    if r.profile.is_empty() {
                        "unnamed profile"
                    } else {
                        &r.profile
                    },
                    r.limits_source_status,
                    r.summary
                );
                for x in &r.tests {
                    t += &format!("\n  {} {:<36} {:?}", x.id, x.parameter, x.outcome);
                }
                Ok(t + &format!("\n-> {out}"))
            }
            "wwrns-trust" => {
                gate(args, FEATURE_WWRNS)?;
                let out = flag(args, "--out").ok_or("--out <report.json> is required")?;
                let fx = flag(args, "--fixes").ok_or("--fixes <fixes.csv> is required")?;
                let cfg: wwrns::WwrnsConfig = parse(&path)?;
                let fixes = wwrns::parse_fixes(
                    &std::fs::read_to_string(&fx).map_err(|e| format!("{fx}: {e}"))?,
                )?;
                let r = wwrns::run(&cfg, &fixes)?;
                write(&out, &json(&r))?;
                Ok(format!(
                    "WWRNS trust: {} epochs, chosen {:?}, {} switches, {} with no qualified source, {} GNSS-suspect -> {out}",
                    r.epochs, r.chosen, r.switches, r.no_choice, r.gnss_suspect_epochs
                ))
            }
            "claim" => {
                gate(args, FEATURE_CLAIMS)?;
                let out = flag(args, "--out").ok_or("--out <report.json> is required")?;
                let req: eloran_claims::ClaimRequest = parse(&path)?;
                let r = eloran_claims::verify_claim(&req, &base)?;
                write(&out, &json(&r))?;
                Ok(format!(
                    "{}: {} -> {out}",
                    r.statement, r.statement_of_result
                ))
            }
            "interop" => {
                gate(args, FEATURE_CLAIMS)?;
                let out = flag(args, "--out").ok_or("--out <report.json> is required")?;
                let req: eloran_claims::InteropRequest = parse(&path)?;
                let r = eloran_claims::interop(&req, &base)?;
                write(&out, &json(&r))?;
                Ok(format!(
                    "interoperability: {} of {} station pairs consistent (E_n ≤ 1), Birge ratio {} -> {out}",
                    r.consistent_pairs,
                    r.pairs.len(),
                    r.birge_ratio.map(|b| format!("{b:.2}")).unwrap_or("-".into())
                ))
            }
            "sol-evidence" => {
                gate(args, FEATURE_SOL)?;
                let (Some(out), Some(signer), Some(kf)) = (
                    flag(args, "--out"),
                    flag(args, "--signer"),
                    flag(args, "--key-file"),
                ) else {
                    return Err("sol-evidence needs --out, --signer and --key-file".into());
                };
                let seed = super::forensics_cmd::read_seed(&kf)?;
                let req: eloran_claims::SolRequest = parse(&path)?;
                let (ix, copies) = eloran_claims::sol_index(&req, &base)?;
                let dir = Path::new(&out);
                std::fs::create_dir_all(dir.join("artifacts"))
                    .map_err(|e| format!("{out}: {e}"))?;
                for (src, name) in &copies {
                    std::fs::copy(src, dir.join("artifacts").join(name))
                        .map_err(|e| format!("{}: {e}", src.display()))?;
                }
                write(&format!("{out}/index.json"), &json(&ix))?;
                write(
                    &format!("{out}/README.md"),
                    &eloran_claims::sol_markdown(&ix),
                )?;
                let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
                crate::signed_dossier::sign(dir, &seed, &signer, &utc)
                    .map_err(|e| e.to_string())?;
                Ok(format!(
                    "safety-of-life evidence index: {} of {} categories with evidence; gaps: {} -> {out} (signed; verify with `kshana-pro dossier verify`)",
                    ix.covered,
                    ix.categories.len(),
                    if ix.gaps.is_empty() { "none".into() } else { ix.gaps.join(", ") }
                ))
            }
            _ => Err(ELORAN_USAGE.into()),
        }
    })();
    match res {
        Ok(t) => {
            println!("{t}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
