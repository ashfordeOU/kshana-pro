// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro fleet ...` (research 2026-11-20 gaps, B2604; docs/STATION-FLEET.md). Checking a
//! configuration history is free; the rest needs the Pro feature. Statements verify offline with
//! `kshana-pro dossier verify <dir> --trust <key>` (free).

use std::path::Path;
use std::process::ExitCode;

use crate::station_fleet as sf;
use kshana_eloran::calreg::{EntryBody, Register, SubjectKind};

use super::flag;
use super::grid_cmd::{gate, signing};

pub const FLEET_USAGE: &str = "usage: kshana-pro fleet <command>\n\
    config-add <config.jsonl> --station <s> --item <receiver|antenna|cable|firmware|position|clock|site|other> --value <v> --by <name> [--serial <n>] [--notes <t>] [--utc <iso>]   Append a hash-chained configuration change (Pro)\n\
    config-verify <config.jsonl>                                     Check the configuration chain (no licence needed)\n\
    site-log <config.jsonl> --station <s> --by <name> [--out <file>] Site log in the IGS section layout from the history (Pro)\n\
    cal-add <register.jsonl> --subject <serial> --quantity <q> --value <v> --unit <u> --u <U> --k <k> --date <YYYY-MM-DD> --operator <name> --report-sha256 <hex> [--interval-days <n>]   Append a calibration result (Pro)\n\
    ageing <register.jsonl> --subject <serial> --quantity <q> [--tolerance <lo,hi>] [--today <YYYY-MM-DD>] [--json <out>]   Ageing curve, prediction and the date it leaves tolerance (Pro)\n\
    statement <offsets.csv> --station <s> --reference <r> --month <YYYY-MM> [--windows <1,10,100,1000>] --out <dir> --signer <name> --key-file <file> [--utc <iso>]   Signed monthly clock-quality statement (Pro)\n\
    dashboard <config.jsonl> [--ageing <a.json,b.json>] --html <out>   Static host dashboard of the fleet (Pro)\n\
    (Pro commands take [--license <token>] [--dev])";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn need(args: &[String], n: &str) -> Result<String, String> {
    flag(args, n).ok_or_else(|| format!("{n} is needed"))
}

/// `kshana-pro fleet ...`.
pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("");
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(FLEET_USAGE);
    };
    if sub != "config-verify" {
        if let Err(e) = gate(args, sf::FEATURE) {
            return fail(e);
        }
    }
    let utc = flag(args, "--utc").unwrap_or_else(super::utc_iso8601_now);
    let p = Path::new(&arg);
    let r: Result<String, String> = (|| match sub {
        "config-add" => {
            let e = sf::config_append(
                p,
                sf::ConfigBody {
                    seq: 0,
                    station: need(args, "--station")?,
                    utc: utc.clone(),
                    item: need(args, "--item")?,
                    value: need(args, "--value")?,
                    serial: flag(args, "--serial").unwrap_or_default(),
                    notes: flag(args, "--notes").unwrap_or_default(),
                    by: need(args, "--by")?,
                },
            )?;
            Ok(format!(
                "[fleet] change {} recorded; head {}",
                e.body.seq, e.hash
            ))
        }
        "config-verify" => {
            let c = sf::config_verify(p)?;
            if c.problems.is_empty() {
                Ok(format!(
                    "[fleet] {} entries verify; head {}",
                    c.entries, c.head
                ))
            } else {
                Err(c.problems.join("; "))
            }
        }
        "site-log" => {
            let t = sf::site_log(p, &need(args, "--station")?, &need(args, "--by")?, &utc)?;
            match flag(args, "--out") {
                Some(o) => std::fs::write(&o, t).map_err(|e| format!("{o}: {e}"))?,
                None => print!("{t}"),
            }
            Ok("[fleet] site log written".into())
        }
        "cal-add" => {
            let num = |n: &str| -> Result<f64, String> {
                need(args, n)?
                    .parse()
                    .map_err(|_| format!("{n} must be a number"))
            };
            let e = Register::open(p).append(EntryBody {
                seq: 0,
                subject_id: need(args, "--subject")?,
                subject_kind: SubjectKind::Instrument,
                date: need(args, "--date")?,
                quantity: need(args, "--quantity")?,
                value: num("--value")?,
                unit: need(args, "--unit")?,
                expanded_u: num("--u")?,
                k: num("--k")?,
                operator: need(args, "--operator")?,
                report_sha256: need(args, "--report-sha256")?,
                interval_days: flag(args, "--interval-days").and_then(|d| d.parse().ok()),
                notes: String::new(),
            })?;
            Ok(format!("[fleet] calibration {} recorded", e.body.seq))
        }
        "ageing" => {
            let tol = match flag(args, "--tolerance") {
                Some(t) => {
                    let v: Vec<f64> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    match v.as_slice() {
                        [a, b] if a < b => Some([*a, *b]),
                        _ => return Err("--tolerance takes lo,hi".into()),
                    }
                }
                None => None,
            };
            let today = flag(args, "--today").unwrap_or_else(|| utc[..10].to_string());
            let a = sf::ageing(
                p,
                &need(args, "--subject")?,
                &need(args, "--quantity")?,
                tol,
                &today,
            )?;
            let j = crate::transport::rounded_json(&a);
            match flag(args, "--json") {
                Some(o) => std::fs::write(&o, j).map_err(|e| format!("{o}: {e}"))?,
                None => println!("{j}"),
            }
            Ok(format!(
                "[fleet] drift {:+.4} per year; leaves tolerance {}",
                a.drift_per_year,
                a.leaves_tolerance
                    .unwrap_or_else(|| "not within 50 years".into())
            ))
        }
        "statement" => {
            let csv = std::fs::read_to_string(p).map_err(|e| format!("{arg}: {e}"))?;
            let w: Vec<f64> = flag(args, "--windows")
                .unwrap_or_else(|| "1,10,100,1000".into())
                .split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect();
            let st = sf::statement(
                &need(args, "--station")?,
                &need(args, "--reference")?,
                &need(args, "--month")?,
                &csv,
                &w,
            )?;
            let s = signing(args, "fleet statement")?;
            sf::write_statement(Path::new(&s.out), &st, &csv)?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[fleet] signed statement {} {} in {}",
                st.station, st.month, s.out
            ))
        }
        "dashboard" => {
            let mut ag = Vec::new();
            if let Some(list) = flag(args, "--ageing") {
                for f in list.split(',') {
                    let t = std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"))?;
                    ag.push(
                        serde_json::from_str::<sf::AgeingIn>(&t)
                            .map_err(|e| format!("{f}: {e}"))?
                            .into(),
                    );
                }
            }
            let h = sf::dashboard(p, &ag, &utc)?;
            let o = need(args, "--html")?;
            std::fs::write(&o, h).map_err(|e| format!("{o}: {e}"))?;
            Ok(format!("[fleet] dashboard in {o}"))
        }
        _ => Err(FLEET_USAGE.into()),
    })();
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
