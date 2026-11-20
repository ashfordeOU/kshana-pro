// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro agri <...>`: agriculture, construction and survey operations
//! ([`crate::agri`]). `drift`, `realign` and `field` need `pro-agri-guidance`; `resilience`
//! needs `pro-agri-resilience`; `advise` needs `pro-agri-advisor`; `photo` needs
//! `pro-agri-photo` (Enterprise); `convert`, `translate`, `sample` and `verify` need none.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::agri::lines::{Format, GuidanceLine, LineSet, Marker, RealignParams};
use crate::agri::log::{Fix, MachineLog};
use crate::agri::resilience::{MachineClass, Profile};
use crate::agri::spaceweather::{self, AdvisorParams, KpSeries, Site};
use crate::agri::{self, drift, field, parse_latlon, parse_utc, photo, resilience, sample};
use crate::license;

use super::forensics_cmd::read_seed;
use super::scorecard_cmd::entitlement;
use super::{flag, has_flag, now_unix, utc_iso8601_now};

pub const USAGE: &str = "usage: kshana-pro agri <command>\n\
    convert <lines> --to json|geojson|kml|isoxml|csv [--from <fmt>] [--out <file>]\n\
        Convert guidance lines between formats (free). Or make an A-B line:\n\
        convert --a <lat,lon> (--b <lat,lon> | --heading <deg> [--length <m>]) [--name <n>] --to <fmt>\n\
    drift --log <log> --line <lines> --width <m> [--implement <m>] [--tolerance <m>] [--max-gap-min <min>]\n\
        [--min-fix <fix>] [--all-epochs] [--json] [--out <dir> --signer <name> --key-file <file>]\n\
        Pass-to-pass drift, within-pass drift, overlap and miss area (Pro)\n\
    realign --lines <lines> [--log <log>] [--marker <lat,lon:lat,lon>]... [--min-fix <fix>] [--capture <m>]\n\
        [--to <fmt>] [--out <file>] [--json]   Shift stored lines onto the rows actually planted (Pro)\n\
    field --boundary <file> --width <m> (--heading <deg> | --line <lines>) [--speed-kmh <v>] [--turn-s <s>]\n\
        [--advisory <advisory.json> [--allow-caution]] [--to <fmt>] [--out <file>] [--json]\n\
        Swath lines over a field, working time and a working window (Pro)\n\
    resilience --log <log> [--profile <profile.toml> | --class agriculture|forestry|construction] [--kp <kp file>]\n\
        [--json] [--out <dir> --signer <name> --key-file <file>]\n\
        Autosteer GNSS-loss resilience score and indicative outage diagnosis (Pro)\n\
    advise --at <lat,lon> --kp <kp file> [--from <utc>] [--hours <n>] [--base <lat,lon> | --baseline-km <km>]\n\
        [--accuracy <m>] [--f107 <sfu>] [--no-rtk] [--no-ppp] [--ionex <file>] [--json] [--out <file>]\n\
        Scintillation and PPP-versus-RTK advisory per hour (Pro)\n\
    translate --at <lat,lon> --kp <kp file> [--from <utc>] [--hours <n>]   Plain-language space weather for growers (free)\n\
    photo <file|dir>... --parcel <boundary> [--from <date>] [--to <date>] [--json]\n\
        [--out <dir> --signer <name> --key-file <file>]   Geotagged-photo consistency checks (Enterprise)\n\
    sample log|line|field|profile|photo [--out <file>] [--outage <start_s>:<dur_s>:<interference|correction|ionosphere>]...\n\
        [--drift <pass>:<m_per_min>]   SYNTHETIC inputs to try the commands (free)\n\
    verify <dir> --trust <key hex>[,...]   Check a signed output directory (free)\n\
    Kp files: the GFZ Kp_ap_Ap_SN_F107 file, NOAA SWPC noaa-planetary-k-index.json or the 3-day forecast text.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

const WITH_VALUE: [&str; 41] = [
    "--out",
    "--signer",
    "--key-file",
    "--utc",
    "--license",
    "--to",
    "--from",
    "--a",
    "--b",
    "--heading",
    "--length",
    "--name",
    "--log",
    "--line",
    "--lines",
    "--width",
    "--implement",
    "--tolerance",
    "--max-gap-min",
    "--min-fix",
    "--marker",
    "--capture",
    "--boundary",
    "--speed-kmh",
    "--turn-s",
    "--advisory",
    "--profile",
    "--class",
    "--kp",
    "--at",
    "--hours",
    "--base",
    "--baseline-km",
    "--accuracy",
    "--f107",
    "--ionex",
    "--parcel",
    "--outage",
    "--drift",
    "--trust",
    "--seed",
];

fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if WITH_VALUE.contains(&a.as_str()) {
            i += 2;
            continue;
        }
        if !a.starts_with("--") {
            out.push(a.clone());
        }
        i += 1;
    }
    out
}

/// Every value of a repeatable flag.
fn flags(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == name)
        .map(|w| w[1].clone())
        .collect()
}

fn gate(args: &[String], feature: &str) -> Result<(), String> {
    let now = now_unix();
    let ent = entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| e.to_string())
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn num(args: &[String], name: &str) -> Result<Option<f64>, String> {
    flag(args, name)
        .map(|v| {
            v.parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("{name} {v}: not a number"))
        })
        .transpose()
}

fn need_num(args: &[String], name: &str) -> Result<f64, String> {
    num(args, name)?.ok_or_else(|| format!("needs {name} <value>"))
}

fn load_log(args: &[String]) -> Result<MachineLog, String> {
    let p = flag(args, "--log").ok_or("needs --log <file>")?;
    let l = MachineLog::parse(&read(&p)?)?;
    if !l.skipped.is_empty() {
        eprintln!("[agri] {}: {} line(s) skipped", p, l.skipped.len());
    }
    Ok(l)
}

fn load_lines(path: &str, fmt: Option<&str>) -> Result<LineSet, String> {
    LineSet::read(&read(path)?, fmt.map(Format::parse).transpose()?)
}

fn load_kp(args: &[String]) -> Result<KpSeries, String> {
    KpSeries::parse(&read(&flag(args, "--kp").ok_or("needs --kp <file>")?)?)
}

/// Write `name` = text to `--out` file or stdout.
fn emit(args: &[String], text: &str) -> Result<(), String> {
    match flag(args, "--out") {
        Some(f) => std::fs::write(&f, text).map_err(|e| format!("{f}: {e}")),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

/// A signed report directory: `report.json`, `report.md`, extra files, signature.
fn signed_dir(
    args: &[String],
    json: &str,
    md: &str,
    extra: &[(&str, String)],
) -> Result<PathBuf, String> {
    let out = PathBuf::from(flag(args, "--out").ok_or("needs --out <dir>")?);
    let (Some(signer), Some(kf)) = (flag(args, "--signer"), flag(args, "--key-file")) else {
        return Err("--out writes signed evidence: needs --signer and --key-file".into());
    };
    let seed = read_seed(&kf)?;
    if out.join(crate::signed_dossier::SIGNATURE_FILE).exists() {
        return Err(format!(
            "{} already holds signed output; it is never overwritten",
            out.display()
        ));
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let w = |n: &str, t: &str| {
        std::fs::write(out.join(n), t).map_err(|e| format!("{}/{n}: {e}", out.display()))
    };
    w("report.json", json)?;
    w("report.md", md)?;
    for (n, t) in extra {
        w(n, t)?;
    }
    let utc = flag(args, "--utc").unwrap_or_else(utc_iso8601_now);
    crate::signed_dossier::sign(&out, &seed, &signer, &utc).map_err(|e| e.to_string())?;
    Ok(out)
}

fn min_fix(args: &[String], default: Fix) -> Result<Fix, String> {
    flag(args, "--min-fix")
        .map(|f| Fix::parse(&f))
        .transpose()
        .map(|f| f.unwrap_or(default))
}

pub fn run(args: &[String]) -> ExitCode {
    match run_inner(args) {
        Ok(c) => c,
        Err(e) => fail(e),
    }
}

fn run_inner(args: &[String]) -> Result<ExitCode, String> {
    let pos = positional(args);
    match args.first().map(String::as_str) {
        Some("convert") => {
            let to = Format::parse(&flag(args, "--to").ok_or("needs --to <format>")?)?;
            let set = if let Some(a) = flag(args, "--a") {
                let a = parse_latlon(&a)?;
                let name = flag(args, "--name").unwrap_or_else(|| "AB".into());
                let line = match (flag(args, "--b"), num(args, "--heading")?) {
                    (Some(b), None) => {
                        let b = parse_latlon(&b)?;
                        GuidanceLine {
                            name,
                            kind: agri::lines::LineKind::Ab,
                            points: vec![[a.0, a.1], [b.0, b.1]],
                            width_m: None,
                        }
                    }
                    (None, Some(h)) => GuidanceLine::from_heading(
                        &name,
                        a,
                        h,
                        num(args, "--length")?.unwrap_or(1000.0),
                    ),
                    _ => return Err("with --a give exactly one of --b or --heading".into()),
                };
                LineSet::new(vec![line])
            } else {
                let p = pos.first().ok_or(USAGE)?;
                load_lines(p, flag(args, "--from").as_deref())?
            };
            emit(args, &set.write(to))?;
            for l in &set.lines {
                eprintln!(
                    "[agri] {}: {:?}, {} points, heading {:.4} deg, {:.2} m",
                    l.name,
                    l.kind,
                    l.points.len(),
                    l.heading_deg(),
                    l.length_m()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("drift") => {
            gate(args, agri::FEATURE_GUIDANCE)?;
            let log = load_log(args)?;
            let set = load_lines(&flag(args, "--line").ok_or("needs --line <lines>")?, None)?;
            let line = &set.lines[0];
            let mut p = drift::DriftParams::new(need_num(args, "--width")?);
            if let Some(v) = num(args, "--implement")? {
                p.implement_m = v;
            }
            if let Some(v) = num(args, "--tolerance")? {
                p.tolerance_m = v;
            }
            if let Some(v) = num(args, "--max-gap-min")? {
                p.max_gap_min = v;
            }
            p.min_fix = min_fix(args, p.min_fix)?;
            p.engaged_only = !has_flag(args, "--all-epochs");
            let r = drift::analyse(&log, line, &p)?;
            let json = serde_json::to_string_pretty(&r).unwrap();
            let md = drift::markdown(&r);
            if flag(args, "--out").is_some() {
                let d = signed_dir(args, &json, &md, &[])?;
                eprintln!("[agri] signed drift report -> {}", d.display());
            }
            print!(
                "{}",
                if has_flag(args, "--json") {
                    json + "\n"
                } else {
                    md
                }
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("realign") => {
            gate(args, agri::FEATURE_GUIDANCE)?;
            let set = load_lines(&flag(args, "--lines").ok_or("needs --lines <lines>")?, None)?;
            let log = flag(args, "--log").map(|_| load_log(args)).transpose()?;
            let markers: Vec<Marker> = flags(args, "--marker")
                .iter()
                .map(|m| Marker::parse(m))
                .collect::<Result<_, _>>()?;
            let mut p = RealignParams {
                min_fix: min_fix(args, Fix::RtkFixed)?,
                ..Default::default()
            };
            if let Some(c) = num(args, "--capture")? {
                p.capture_m = c;
            }
            if let Some(t) = num(args, "--tolerance")? {
                p.tolerance_m = t;
            }
            let r = agri::lines::realign(&set, log.as_ref(), &markers, &p)?;
            let to = Format::parse(&flag(args, "--to").unwrap_or_else(|| "json".into()))?;
            emit(args, &r.realigned.as_ref().unwrap().write(to))?;
            let summary = serde_json::to_string_pretty(&r).unwrap();
            if has_flag(args, "--json") {
                eprintln!("{summary}");
            } else {
                eprintln!(
                    "[agri] shift east {:+.4} m (u {:.4}), north {:+.4} m (u {:.4}); {}; {} epochs, {} markers ({})",
                    r.shift_east_m, r.u_east_m, r.shift_north_m, r.u_north_m, r.observability, r.epochs_used, r.markers, r.label
                );
                for l in &r.lines {
                    eprintln!(
                        "[agri]   {}: offset {:+.4} m, residual {:+.4} m{}",
                        l.line,
                        l.mean_offset_m,
                        l.residual_m,
                        if l.flagged { " FLAGGED" } else { "" }
                    );
                }
                for n in &r.notes {
                    eprintln!("[agri] note: {n}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("field") => {
            gate(args, agri::FEATURE_GUIDANCE)?;
            let b = field::read_boundary(&read(
                &flag(args, "--boundary").ok_or("needs --boundary <file>")?,
            )?)?;
            let heading = match (num(args, "--heading")?, flag(args, "--line")) {
                (Some(h), None) => h,
                (None, Some(l)) => load_lines(&l, None)?.lines[0].heading_deg(),
                _ => return Err("give exactly one of --heading or --line".into()),
            };
            let mut plan = field::plan(
                &b,
                &field::PlanParams {
                    heading_deg: heading,
                    width_m: need_num(args, "--width")?,
                    speed_kmh: num(args, "--speed-kmh")?.unwrap_or(10.0),
                    turn_s: num(args, "--turn-s")?.unwrap_or(20.0),
                    min_line_m: 5.0,
                },
            )?;
            if let Some(a) = flag(args, "--advisory") {
                plan.window =
                    field::window(&read(&a)?, plan.work_h, has_flag(args, "--allow-caution"))?;
                if plan.window.is_none() {
                    plan.notes.push(format!(
                        "No run of {:.0} consecutive suitable hours in the advisory.",
                        plan.work_h.ceil()
                    ));
                }
            }
            let to = Format::parse(&flag(args, "--to").unwrap_or_else(|| "geojson".into()))?;
            emit(args, &plan.lines.write(to))?;
            let s = serde_json::to_string_pretty(&plan).unwrap();
            if has_flag(args, "--json") {
                eprintln!("{s}");
            } else {
                eprintln!(
                    "[agri] {} passes, {:.0} m, field {:.2} ha, covered {:.2} ha, {:.2} h of work{} ({})",
                    plan.passes,
                    plan.path_m,
                    plan.field_ha,
                    plan.covered_ha,
                    plan.work_h,
                    match &plan.window {
                        Some(w) => format!(", window from {} ({} h, {} caution)", w.start_utc, w.hours, w.caution_hours),
                        None => String::new(),
                    },
                    plan.label
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("resilience") => {
            gate(args, agri::FEATURE_RESILIENCE)?;
            let log = load_log(args)?;
            let profile = match (flag(args, "--profile"), flag(args, "--class")) {
                (Some(p), _) => Profile::parse(&read(&p)?)?,
                (None, Some(c)) => Profile::for_class(match c.as_str() {
                    "agriculture" => MachineClass::Agriculture,
                    "forestry" => MachineClass::Forestry,
                    "construction" => MachineClass::Construction,
                    o => return Err(format!("unknown class '{o}'")),
                }),
                (None, None) => Profile::default(),
            };
            let kp = flag(args, "--kp").map(|_| load_kp(args)).transpose()?;
            let r = resilience::score(&log, &profile, kp.as_ref());
            let json = serde_json::to_string_pretty(&r).unwrap();
            let md = resilience::markdown(&r);
            if flag(args, "--out").is_some() {
                let d = signed_dir(
                    args,
                    &json,
                    &md,
                    &[(
                        "profile.toml",
                        toml::to_string(&profile).unwrap_or_default(),
                    )],
                )?;
                eprintln!("[agri] signed resilience report -> {}", d.display());
            }
            print!(
                "{}",
                if has_flag(args, "--json") {
                    json + "\n"
                } else {
                    md
                }
            );
            Ok(ExitCode::SUCCESS)
        }
        Some(c @ ("advise" | "translate")) => {
            if c == "advise" {
                gate(args, agri::FEATURE_ADVISOR)?;
            }
            let (lat, lon) = parse_latlon(&flag(args, "--at").ok_or("needs --at <lat,lon>")?)?;
            let kp = load_kp(args)?;
            let t0 = match flag(args, "--from") {
                Some(t) => parse_utc(&t)?,
                None => kp.values[0].0,
            };
            let hours = num(args, "--hours")?
                .unwrap_or(72.0)
                .clamp(1.0, 24.0 * 31.0) as usize;
            let base = flag(args, "--base").map(|b| parse_latlon(&b)).transpose()?;
            let baseline_km = match (base, num(args, "--baseline-km")?) {
                (Some(b), _) => kshana::geodesic::distance_m(lat, lon, b.0, b.1) / 1000.0,
                (None, Some(k)) => k,
                (None, None) => 10.0,
            };
            let site = Site {
                lat,
                lon,
                baseline_km: agri::round(baseline_km, 3),
                base,
                rtk_available: !has_flag(args, "--no-rtk"),
                ppp_available: !has_flag(args, "--no-ppp"),
            };
            let mut p = AdvisorParams::default();
            if let Some(a) = num(args, "--accuracy")? {
                p.accuracy_m = a;
            }
            let ionex_file = flag(args, "--ionex")
                .map(|f| read(&f).and_then(|t| kshana::ionex::parse_ionex_file(&t)))
                .transpose()?;
            let ionex = ionex_file.as_ref().map(|file| spaceweather::Ionex { file });
            let a = spaceweather::advise(
                &site,
                &kp,
                num(args, "--f107")?,
                t0,
                hours,
                &p,
                ionex.as_ref(),
            );
            if c == "translate" {
                print!("{}", spaceweather::translate(&a));
                return Ok(ExitCode::SUCCESS);
            }
            let json = serde_json::to_string_pretty(&a).unwrap();
            if flag(args, "--out").is_some() {
                emit(args, &json)?;
            }
            if has_flag(args, "--json") {
                println!("{json}");
            } else {
                println!(
                    "Advisory ({}) at {:.4}, {:.4}: {}, geomagnetic latitude {:.1} deg, modip {:.1} deg; baseline {:.1} km; Kp from {}",
                    a.label, lat, lon, a.region, a.geomagnetic_lat_deg, a.modip_deg, site.baseline_km, a.kp_source
                );
                println!(
                    "Fades deeper than {} dB (engine Cornell model): low {:.4}, moderate {:.4}, high {:.4} of the time",
                    p.margin_db, a.fade_fraction[0], a.fade_fraction[1], a.fade_fraction[2]
                );
                for h in &a.hours {
                    println!(
                        "  {}  Kp {:>5}  G{}  scint {:<8}  RTK {:>7}  PPP {:>7}  {:<10} {}",
                        h.utc,
                        h.kp.map(|k| format!("{k:.2}"))
                            .unwrap_or_else(|| "-".into()),
                        h.g,
                        serde_json::to_value(h.scintillation)
                            .unwrap()
                            .as_str()
                            .unwrap(),
                        h.rtk_sigma_m
                            .map(|s| format!("{s:.3}"))
                            .unwrap_or_else(|| "-".into()),
                        h.ppp_sigma_m
                            .map(|s| format!("{s:.3}"))
                            .unwrap_or_else(|| "-".into()),
                        h.method,
                        serde_json::to_value(h.status).unwrap().as_str().unwrap()
                    );
                }
                for n in &a.notes {
                    println!("note: {n}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("photo") => {
            gate(args, agri::FEATURE_PHOTO)?;
            if pos.is_empty() {
                return Err("needs photo files or directories".into());
            }
            let parcel = field::read_boundary(&read(
                &flag(args, "--parcel").ok_or("needs --parcel <boundary>")?,
            )?)?;
            let files = photo::collect(&pos.iter().map(PathBuf::from).collect::<Vec<_>>())?;
            if files.is_empty() {
                return Err("no .jpg files found".into());
            }
            let mut p = photo::PhotoParams::default();
            match (flag(args, "--from"), flag(args, "--to")) {
                (Some(a), Some(b)) => {
                    p.window = Some((
                        parse_utc(&a)?,
                        parse_utc(&b)? + if b.len() <= 10 { 86_399 } else { 0 },
                    ))
                }
                (None, None) => {}
                _ => return Err("give both --from and --to for the claimed window".into()),
            }
            let r = photo::check(&files, &parcel, &p);
            let json = serde_json::to_string_pretty(&r).unwrap();
            let md = photo::markdown(&r);
            if flag(args, "--out").is_some() {
                let d = signed_dir(args, &json, &md, &[])?;
                eprintln!("[agri] signed photo report -> {}", d.display());
            }
            print!(
                "{}",
                if has_flag(args, "--json") {
                    json + "\n"
                } else {
                    md
                }
            );
            Ok(ExitCode::SUCCESS)
        }
        Some("sample") => {
            let what = pos.first().ok_or(USAGE)?;
            let mut spec = sample::Spec::new((49.9, -97.1));
            for o in flags(args, "--outage") {
                let f: Vec<&str> = o.split(':').collect();
                if f.len() != 3 {
                    return Err(format!("--outage {o}: expected start_s:dur_s:kind"));
                }
                spec.outages.push((
                    f[0].parse().map_err(|_| format!("--outage {o}"))?,
                    f[1].parse().map_err(|_| format!("--outage {o}"))?,
                    sample::OutageKind::parse(f[2])?,
                ));
            }
            if let Some(d) = flag(args, "--drift") {
                let (k, r) = d.split_once(':').ok_or("--drift <pass>:<m_per_min>")?;
                spec.drift = Some((
                    k.parse().map_err(|_| "--drift pass")?,
                    r.parse().map_err(|_| "--drift rate")?,
                ));
            }
            match what.as_str() {
                "log" => emit(args, &sample::log(&spec).to_csv())?,
                "line" => emit(
                    args,
                    &LineSet::new(vec![spec.line()]).write(Format::Neutral),
                )?,
                "field" => {
                    let f = agri::geo::LocalFrame::new(spec.origin.0, spec.origin.1, 0.0);
                    let pts: Vec<[f64; 2]> =
                        [[-6.0, -10.0], [80.0, -10.0], [80.0, 310.0], [-6.0, 310.0]]
                            .iter()
                            .map(|q| {
                                let (a, b) = f.latlon(q[0], q[1]);
                                [b, a]
                            })
                            .collect();
                    let mut ring = pts.clone();
                    ring.push(pts[0]);
                    emit(
                        args,
                        &serde_json::to_string_pretty(&serde_json::json!({
                            "type": "Feature", "properties": {"name": "sample field (SYNTHETIC)"},
                            "geometry": {"type": "Polygon", "coordinates": [ring]}
                        }))
                        .unwrap(),
                    )?
                }
                "profile" => emit(args, &toml::to_string(&Profile::default()).unwrap())?,
                "photo" => {
                    let out = flag(args, "--out").ok_or("sample photo needs --out <file.jpg>")?;
                    let t = parse_utc("2024-05-10T15:00:00Z")?;
                    let x = photo::Exif {
                        lat: Some(49.9005123),
                        lon: Some(-97.0996789),
                        alt_m: Some(232.0),
                        gps_utc: Some(t),
                        hpe_m: Some(3.5),
                        make: Some("SampleCam".into()),
                        model: Some("S1".into()),
                        camera_time: Some(t - 5 * 3600),
                        offset: Some("-05:00".into()),
                        ..Default::default()
                    };
                    std::fs::write(&out, photo::synth_jpeg(&x))
                        .map_err(|e| format!("{out}: {e}"))?;
                }
                o => {
                    return Err(format!(
                        "unknown sample '{o}' (log, line, field, profile, photo)"
                    ))
                }
            }
            if let Some(o) = flag(args, "--out") {
                eprintln!("[agri] wrote SYNTHETIC {what} {o}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("verify") => Ok(super::scorecard_cmd::verify_dir(args)),
        _ => {
            println!("{USAGE}");
            let _ = Path::new("");
            Ok(ExitCode::SUCCESS)
        }
    }
}
