// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro briefing <command>`: exposure history, route risk and briefings
//! (`crate::briefings`, research backlog E09).
//!
//! Licence: `transits` and `brief` need `pro-route-exposure`; `tails`, `lowalt` and `occ`
//! (`preflight`) need `pro-aviation-exposure`; `regional` needs `pro-regional-datasets`;
//! `road` needs `pro-road-exposure`; `spillover` needs `pro-spillover-footprint` (all Pro).
//! `areas`, `notam` and `verify` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::briefings::{self, aviation, brief, regional, road, spillover, transit};

use super::evidence_cmd::{producing, sign};
use super::flag;
use super::scorecard_cmd::verify_dir;

pub const USAGE: &str = "usage: kshana-pro briefing <command>\n\
    transits <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Per-transit exposure records from AIS tracks, listed-area records and the fleet view; signed (Pro)\n\
    brief <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Briefing for chokepoints, regions, listed areas and a planned route, with stated confidence, data window and provenance (JSON, HTML, PDF); signed (Pro)\n\
    tails <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Per-tail exposure ledger from ADS-B tracks over a period (90 days by default), by altitude band; signed (Pro)\n\
    lowalt <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Low-altitude cell map (below and above a ceiling, default 2,000 ft) of degraded ADS-B integrity, no identifiers; signed (Pro)\n\
    occ <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Flight briefings and an operations-control alert list for planned flights, with confidence and data latency (alias: preflight); signed (Pro)\n\
    regional <request.toml> --out <dir> [--signer <name> --key-file <file> [--utc <iso>]] [--license <token>] [--dev]\n\
        Versioned regional exposure dataset with pattern separation and airport pattern libraries; rebuilds byte-identically (Pro)\n\
    road <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Corridor exposure index and heat map from telematics jammer detections, under the privacy rule; signed (Pro)\n\
    spillover <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Modelled spillover footprint of declared test areas by receiver altitude, affected users, agreement with ADS-B layers; signed (Pro)\n\
    notam <file> [--out <geometry.json>] [--geojson <file>]\n\
        NOTAM text to geometry with harmonised start and end criteria (free)\n\
    areas\n\
        Print the built-in chokepoints and regions as an area file (free)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a signed output's signature and every file, offline (free)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn file_arg(args: &[String]) -> Option<&String> {
    args.get(1).filter(|a| !a.starts_with("--"))
}

/// Gate, build, write, sign: the shape every producing command shares.
fn produce<F>(args: &[String], feature: &str, what: &str, f: F) -> ExitCode
where
    F: FnOnce(&Path, &Path) -> Result<String, String>,
{
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let r = producing(args, feature).and_then(|(out, seed, signer, utc)| {
        let line = f(Path::new(file), Path::new(&out))?;
        sign(&out, &seed, &signer, &utc)?;
        Ok((line, out))
    });
    match r {
        Ok((line, out)) => {
            eprintln!("[{what}] {line}; signed in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn regional_cmd(args: &[String]) -> ExitCode {
    // Signing is optional for a dataset: the files themselves rebuild byte-identically.
    if flag(args, "--signer").is_some() {
        return produce(args, briefings::FEATURE_REGIONAL, "regional", |f, out| {
            let (q, src) = regional::RegionalRequest::from_file(f)?;
            let d = regional::build(&q)?;
            regional::write_dir(out, &d, &src)?;
            Ok(format!(
                "{} {}: {} region(s), {} day(s)",
                d.dataset,
                d.version,
                d.regions.len(),
                d.dates.len()
            ))
        });
    }
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let Some(out) = flag(args, "--out") else {
        return fail("regional needs --out <dir>");
    };
    let now = super::now_unix();
    let r = super::scorecard_cmd::entitlement(args, now)
        .and_then(|ent| {
            crate::license::check(&ent, briefings::FEATURE_REGIONAL, now).map_err(|e| e.to_string())
        })
        .and_then(|_| {
            let (q, src) = regional::RegionalRequest::from_file(Path::new(file))?;
            let d = regional::build(&q)?;
            regional::write_dir(Path::new(&out), &d, &src)?;
            Ok(d)
        });
    match r {
        Ok(d) => {
            eprintln!(
                "[regional] {} {}: {} region(s), {} day(s); unsigned dataset in {out} (MANIFEST.sha256)",
                d.dataset,
                d.version,
                d.regions.len(),
                d.dates.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn notam_cmd(args: &[String]) -> ExitCode {
    let Some(file) = file_arg(args) else {
        return fail(USAGE);
    };
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) => return fail(format!("{file}: {e}")),
    };
    let d = spillover::parse_notams(&text);
    let json = serde_json::to_string_pretty(&d).unwrap_or_default() + "\n";
    match flag(args, "--out") {
        Some(o) => {
            if let Err(e) = std::fs::write(&o, &json) {
                return fail(format!("{o}: {e}"));
            }
        }
        None => print!("{json}"),
    }
    if let Some(g) = flag(args, "--geojson") {
        let t = serde_json::to_string_pretty(&spillover::notams_geojson(&d)).unwrap_or_default();
        if let Err(e) = std::fs::write(&g, t + "\n") {
            return fail(format!("{g}: {e}"));
        }
    }
    for r in &d.refused {
        eprintln!("[notam] refused: {r}");
    }
    eprintln!(
        "[notam] {} NOTAM(s) read, {} refused",
        d.notams.len(),
        d.refused.len()
    );
    if d.notams.is_empty() && !d.refused.is_empty() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("transits") => produce(args, briefings::FEATURE_ROUTE, "transits", |f, out| {
            let (q, src) = transit::TransitRequest::from_file(f)?;
            let h = transit::build(&q)?;
            transit::write_dir(out, &h, &src)?;
            Ok(format!(
                "{} transit(s), {} vessel(s), {} listed-area record(s)",
                h.transits.len(),
                h.fleet.len(),
                h.listed_area_records.len()
            ))
        }),
        Some("brief") => produce(args, briefings::FEATURE_ROUTE, "brief", |f, out| {
            let (q, src) = brief::BriefRequest::from_file(f)?;
            let b = brief::build(&q)?;
            brief::write_dir(out, &b, &src)?;
            Ok(format!(
                "{} area(s){}; confidence {}",
                b.areas.len(),
                if b.route.is_some() {
                    " and a route"
                } else {
                    ""
                },
                b.areas
                    .iter()
                    .map(|a| format!("{} {}", a.area_id, a.confidence.level))
                    .chain(
                        b.route
                            .iter()
                            .map(|r| format!("route {}", r.confidence.level))
                    )
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }),
        Some("tails") => produce(args, briefings::FEATURE_AVIATION, "tails", |f, out| {
            let (q, src) = aviation::LedgerRequest::from_file(f)?;
            let l = aviation::tail_ledger(&q)?;
            aviation::write_ledger(out, &l, &src)?;
            Ok(format!(
                "{} tail(s), {} flagged for review",
                l.entries.len(),
                l.entries.iter().filter(|e| e.flag == "review").count()
            ))
        }),
        Some("lowalt") => produce(args, briefings::FEATURE_AVIATION, "lowalt", |f, out| {
            let (q, src) = aviation::LowAltRequest::from_file(f)?;
            let m = aviation::low_altitude_map(&q)?;
            briefings::write_json(out, aviation::LOWALT_FILE, &m)?;
            briefings::write_text(out, "request.toml", src.as_bytes())?;
            Ok(format!(
                "{} published cell-band(s)",
                m["features"].as_array().map_or(0, Vec::len)
            ))
        }),
        Some("occ") | Some("preflight") => {
            produce(args, briefings::FEATURE_AVIATION, "occ", |f, out| {
                let (q, src) = aviation::OccRequest::from_file(f)?;
                let a = aviation::occ(&q)?;
                aviation::write_occ(out, &a, &src)?;
                Ok(format!(
                    "{} flight(s), {} advisory",
                    a.alerts.len(),
                    a.alerts.iter().filter(|x| x.level == "advisory").count()
                ))
            })
        }
        Some("regional") => regional_cmd(args),
        Some("road") => produce(args, briefings::FEATURE_ROAD, "road", |f, out| {
            let (q, src) = road::RoadRequest::from_file(f)?;
            let (ix, heat) = road::build(&q)?;
            road::write_dir(out, &ix, &heat, &src)?;
            Ok(format!(
                "{} corridor(s), {} segment(s) published, {} withheld",
                ix.corridors.len(),
                ix.segments
                    .iter()
                    .filter(|s| s.status == "published")
                    .count(),
                ix.segments
                    .iter()
                    .filter(|s| s.status == "withheld")
                    .count()
            ))
        }),
        Some("spillover") => produce(args, briefings::FEATURE_SPILLOVER, "spillover", |f, out| {
            let (q, src) = spillover::FootprintRequest::from_file(f)?;
            let (d, geo) = spillover::footprint(&q)?;
            spillover::write_dir(out, &d, &geo, &src)?;
            Ok(format!(
                "{} area(s), {} affected user(s)",
                d.areas.len(),
                d.areas
                    .iter()
                    .map(|a| a.affected_users.len())
                    .sum::<usize>()
            ))
        }),
        Some("notam") => notam_cmd(args),
        Some("areas") => {
            print!("{}", briefings::areas::builtin_toml());
            ExitCode::SUCCESS
        }
        Some("verify") => verify_dir(args),
        _ => fail(USAGE),
    }
}
