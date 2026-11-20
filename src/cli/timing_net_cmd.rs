// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro timing <posture|chain|crosscheck|station>` (E19 timing networks: B1902, B1904,
//! B1905, B1907). Usage lines live in [`super::timing_cmd::USAGE`]. Checking a CycloneDX SBOM,
//! printing the chain template and the cross-check profiles need no licence; verifying a
//! signed statement is `kshana-pro dossier verify` (free).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::timing_cmd::{fail, flags_all, gate, utc_unix, write_or_print, USAGE};
use super::{flag, has_flag, utc_iso8601_now};

fn base_of(path: &str) -> PathBuf {
    Path::new(path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn done(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}

/// `timing posture <check|firmware|sbom>` (B1902).
pub fn posture(args: &[String], sub: &str) -> ExitCode {
    use crate::timing::posture as po;
    use crate::timing::timebomb::AdvisoryTable;
    if sub == "sbom" && args.get(2).map(String::as_str) == Some("check") {
        let Some(path) = args.get(3) else {
            return fail(USAGE);
        };
        let res = (|| -> Result<Vec<String>, String> {
            let v: serde_json::Value =
                serde_json::from_str(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
            Ok(po::check_bom(&v))
        })();
        return match res {
            Ok(issues) if issues.is_empty() => {
                eprintln!("[sbom] {path}: no structural issue");
                ExitCode::SUCCESS
            }
            Ok(issues) => {
                for i in &issues {
                    eprintln!("  {i}");
                }
                fail(format!("{path}: {} structural issue(s)", issues.len()))
            }
            Err(e) => fail(e),
        };
    }
    if let Err(e) = gate(args, po::FEATURE) {
        return fail(e);
    }
    let Some(path) = args
        .get(if sub == "sbom" { 3 } else { 2 })
        .filter(|a| !a.starts_with("--"))
        .cloned()
    else {
        return fail(USAGE);
    };
    done((|| match sub {
        "check" => {
            let inv: po::Inventory =
                toml::from_str(&read(&path)?).map_err(|e| format!("{path}: {e}"))?;
            let mut table = if has_flag(args, "--no-seed") {
                AdvisoryTable::default()
            } else {
                AdvisoryTable::seed()
            };
            for extra in flags_all(args, "--advisories") {
                table.advisory.extend(
                    AdvisoryTable::parse(&read(&extra)?)
                        .map_err(|e| format!("{extra}: {e}"))?
                        .advisory,
                );
            }
            let mut comps = po::ComponentTable::default();
            for extra in flags_all(args, "--components") {
                comps.component_advisory.extend(
                    po::ComponentTable::parse(&read(&extra)?)
                        .map_err(|e| format!("{extra}: {e}"))?
                        .component_advisory,
                );
            }
            let (now, utc) = utc_unix(args)?;
            let r = po::check(&inv, &table, &comps, &base_of(&path), now, &utc)?;
            write_or_print(
                flag(args, "--json"),
                &serde_json::to_string_pretty(&r).expect("serialises"),
            )?;
            if let Some(c) = flag(args, "--csv") {
                std::fs::write(&c, po::report_csv(&r)).map_err(|e| format!("{c}: {e}"))?;
            }
            for a in &r.appliances {
                let top = a.findings.first();
                eprintln!(
                    "{:<16} {:<6} {}",
                    a.id,
                    a.worst,
                    top.map(|f| format!("{} {} {}", f.category, f.id, f.status))
                        .unwrap_or_default()
                );
            }
            Ok(())
        }
        "firmware" => {
            let image = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
            let reference = if let Some(h) = flag(args, "--sha256") {
                po::Reference::Sha256(h)
            } else if let Some(s) = flag(args, "--sums") {
                po::Reference::Sums(read(&s)?)
            } else if let (Some(sig), Some(pk)) = (flag(args, "--sig"), flag(args, "--pubkey")) {
                po::Reference::Ed25519 {
                    signature: std::fs::read(&sig).map_err(|e| format!("{sig}: {e}"))?,
                    public_key: pk,
                }
            } else {
                return Err("firmware needs --sha256, --sums or --sig with --pubkey".into());
            };
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            let mut r = po::check_firmware(&path, &image, &reference, &utc)?;
            r.vendor = flag(args, "--vendor");
            r.model = flag(args, "--model");
            r.version = flag(args, "--version");
            write_or_print(
                flag(args, "--json"),
                &serde_json::to_string_pretty(&r).expect("serialises"),
            )?;
            eprintln!("[firmware] {} {} ({})", r.image, r.verdict, r.method);
            if r.passed() {
                Ok(())
            } else {
                Err(format!("{}: {}", r.image, r.verdict))
            }
        }
        "sbom" if args.get(2).map(String::as_str) == Some("station") => {
            let spec: po::StationSpec =
                toml::from_str(&read(&path)?).map_err(|e| format!("{path}: {e}"))?;
            let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
            let bom = po::station_bom(&spec, &base_of(&path), &utc)?;
            let issues = po::check_bom(&bom);
            if !issues.is_empty() {
                return Err(format!(
                    "generated SBOM failed its check: {}",
                    issues.join("; ")
                ));
            }
            write_or_print(
                flag(args, "--out"),
                &serde_json::to_string_pretty(&bom).expect("serialises"),
            )
        }
        _ => Err(USAGE.into()),
    })())
}

/// `timing chain <chain.toml> ...` (B1904).
pub fn chain(args: &[String], path: &str) -> ExitCode {
    use crate::timing::chain as ch;
    if path == "template" {
        print!("{}", ch::TEMPLATE);
        return ExitCode::SUCCESS;
    }
    if let Err(e) = gate(args, ch::FEATURE) {
        return fail(e);
    }
    let (Some(out), Some(signer), Some(kf)) = (
        flag(args, "--out"),
        flag(args, "--signer"),
        flag(args, "--key-file"),
    ) else {
        return fail("chain needs --out, --signer and --key-file");
    };
    let res: Result<String, String> = (|| {
        let seed = super::forensics_cmd::read_seed(&kf)?;
        let src = read(path)?;
        let spec = ch::ChainSpec::parse(&src)?;
        let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
        let base = base_of(path);
        let r = ch::assess(&spec, &base, &utc)?;
        let dir = Path::new(&out);
        ch::write_dir(dir, &r, &src, &spec, &base)?;
        crate::signed_dossier::sign(dir, &seed, &signer, &utc).map_err(|e| e.to_string())?;
        Ok(r.verdict)
    })();
    match res {
        Ok(v) => {
            eprintln!("[chain] {v}; signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `timing crosscheck <request.toml> ...` (B1905).
pub fn crosscheck(args: &[String], path: &str) -> ExitCode {
    use crate::timing::crosscheck as cc;
    if path == "profile" {
        let name = args.get(2).map(String::as_str);
        return match cc::PROFILES.iter().find(|p| Some(p.0) == name) {
            Some(p) => {
                print!("# {}\n{}", p.1, p.2);
                ExitCode::SUCCESS
            }
            None => {
                for p in cc::PROFILES {
                    println!("{:<12} {}", p.0, p.1);
                }
                if name.is_some() {
                    fail("unknown profile")
                } else {
                    ExitCode::SUCCESS
                }
            }
        };
    }
    if let Err(e) = gate(args, cc::FEATURE) {
        return fail(e);
    }
    done((|| {
        let spec: cc::Spec = toml::from_str(&read(path)?).map_err(|e| format!("{path}: {e}"))?;
        let r = cc::check(&spec, &base_of(path))?;
        write_or_print(
            flag(args, "--json"),
            &serde_json::to_string_pretty(&r).expect("serialises"),
        )?;
        if let Some(c) = flag(args, "--csv") {
            std::fs::write(&c, cc::bins_csv(&r)).map_err(|e| format!("{c}: {e}"))?;
        }
        eprintln!("[crosscheck] {}", r.verdict);
        for e in &r.events {
            eprintln!("  {:<22} {:<14} {}", e.kind, e.subject, e.detail);
        }
        Ok(())
    })())
}

/// `timing station <station.toml> ...` (B1907).
pub fn station(args: &[String], path: &str) -> ExitCode {
    use crate::timing::station as st;
    if let Err(e) = gate(args, st::FEATURE) {
        return fail(e);
    }
    done((|| {
        let src = read(path)?;
        let spec: st::StationSpec = toml::from_str(&src).map_err(|e| format!("{path}: {e}"))?;
        let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
        let base = base_of(path);
        let r = st::assess(&spec, &base, &utc)?;
        write_or_print(
            flag(args, "--json"),
            &serde_json::to_string_pretty(&r).expect("serialises"),
        )?;
        if let Some(c) = flag(args, "--csv") {
            std::fs::write(&c, st::passes_csv(&r)).map_err(|e| format!("{c}: {e}"))?;
        }
        if let Some(out) = flag(args, "--out") {
            let (Some(signer), Some(kf)) = (flag(args, "--signer"), flag(args, "--key-file"))
            else {
                return Err("--out needs --signer and --key-file".into());
            };
            let seed = super::forensics_cmd::read_seed(&kf)?;
            let dir = Path::new(&out);
            st::write_dir(dir, &r, &src, &spec, &base)?;
            crate::signed_dossier::sign(dir, &seed, &signer, &utc).map_err(|e| e.to_string())?;
            eprintln!("signed in {out}");
        }
        eprintln!("[station] {}", r.verdict);
        Ok(())
    })())
}
