// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Agriculture, construction and survey operations (`kshana_pro::agri`): guidance-line
//! converters and realignment, pass-to-pass drift, the resilience score and outage
//! diagnosis, the field planner, the space-weather advisors with the May 2024 backtest on the
//! published GFZ Kp series, the geotagged-photo checks and the CLI.
//! Evidence: MODELLED (synthetic machine logs and photos; the Kp series is real).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::agri::lines::{Format, GuidanceLine, LineKind, LineSet, Marker, RealignParams};
use kshana_pro::agri::log::{Fix, MachineLog};
use kshana_pro::agri::resilience::{self, Cause, MachineClass, Profile};
use kshana_pro::agri::sample::{self, OutageKind, Spec};
use kshana_pro::agri::spaceweather::{self, AdvisorParams, KpSeries, Risk, Site, Status};
use kshana_pro::agri::{self, drift, field, geo::LocalFrame, parse_utc, photo};
use kshana_pro::license::{self, Tier};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-agri-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn gfz() -> KpSeries {
    KpSeries::parse(
        &std::fs::read_to_string(root().join("agri/data/gfz-kp-2024-05-07-to-14.txt")).unwrap(),
    )
    .unwrap()
}

const ORIGIN: (f64, f64) = (49.9, -97.1);

// ---------------------------------------------------------------- lines

#[test]
fn line_formats_convert_without_loss() {
    let set = LineSet::new(vec![
        GuidanceLine::from_heading("north", ORIGIN, 7.25, 800.0),
        GuidanceLine {
            name: "contour & bend".into(),
            kind: LineKind::Curve,
            points: vec![
                [49.9, -97.1],
                [49.901, -97.0995],
                [49.902, -97.0994],
                [49.903, -97.0996],
            ],
            width_m: Some(9.1),
        },
    ]);
    for from in [
        Format::Neutral,
        Format::GeoJson,
        Format::Kml,
        Format::IsoXml,
        Format::Csv,
    ] {
        let text = set.write(from);
        let back = LineSet::read(&text, None).unwrap();
        for to in [
            Format::Neutral,
            Format::GeoJson,
            Format::Kml,
            Format::IsoXml,
            Format::Csv,
        ] {
            let again = LineSet::read(&back.write(to), Some(to)).unwrap();
            assert_eq!(again.lines.len(), 2);
            assert_eq!(again.lines[1].name, "contour & bend");
            assert_eq!(again.lines[0].kind, LineKind::Ab);
            for (a, b) in again.lines.iter().zip(&set.lines) {
                for (p, q) in a.points.iter().zip(&b.points) {
                    // 1e-8 deg is about 1 mm.
                    assert!(
                        (p[0] - q[0]).abs() < 1e-8 && (p[1] - q[1]).abs() < 1e-8,
                        "{from:?}->{to:?}"
                    );
                }
            }
        }
    }
    assert!((set.lines[0].heading_deg() - 7.25).abs() < 1e-6);
}

#[test]
fn isoxml_guidance_patterns_are_read() {
    // A hand-written task file in the published element structure: an A-B pattern, an A+
    // pattern with a heading, a curve, and a pivot (refused).
    let x = r#"<?xml version="1.0"?>
<ISO11783_TaskData VersionMajor="4" VersionMinor="0" DataTransferOrigin="1">
<PFD A="PFD1" C="North 40" D="160000">
<LSG A="1"><PNT A="2" C="1.0" D="2.0"/></LSG>
<GGP A="GGP1" B="Spring">
<GPN A="GPN1" B="AB east" C="1"><LSG A="5"><PNT A="6" C="49.9" D="-97.1"/><PNT A="7" C="49.9" D="-97.09"/></LSG></GPN>
<GPN A="GPN2" B="A plus" C="2" G="45.0"><LSG A="5"><PNT A="6" C="49.91" D="-97.1"/></LSG></GPN>
<GPN A="GPN3" B="curve" C="3"><LSG A="5"><PNT A="6" C="49.9" D="-97.1"/><PNT A="9" C="49.901" D="-97.0999"/><PNT A="7" C="49.902" D="-97.0997"/></LSG></GPN>
</GGP></PFD></ISO11783_TaskData>"#;
    let s = LineSet::read(x, None).unwrap();
    assert_eq!(s.lines.len(), 3);
    assert_eq!(s.lines[0].name, "AB east");
    assert!((s.lines[0].heading_deg() - 90.0).abs() < 0.01);
    assert!((s.lines[1].heading_deg() - 45.0).abs() < 1e-6);
    assert_eq!(s.lines[2].kind, LineKind::Curve);
    assert_eq!(s.lines[2].points.len(), 3);
    let pivot = x.replace("C=\"3\"", "C=\"4\"");
    assert!(LineSet::read(&pivot, None).unwrap_err().contains("pivot"));
}

/// Stored lines, `n` parallel passes `w` apart, displaced by (de, dn) metres.
fn stored_passes(n: usize, w: f64, de: f64, dn: f64) -> LineSet {
    let f = LocalFrame::new(ORIGIN.0, ORIGIN.1, 0.0);
    LineSet::new(
        (0..n)
            .map(|k| {
                // The sample drives north with passes to the west (left of north).
                let x = -(k as f64) * w + de;
                let (a0, a1) = f.latlon(x, dn);
                let (b0, b1) = f.latlon(x, 300.0 + dn);
                GuidanceLine {
                    name: format!("row-{k}"),
                    kind: LineKind::Ab,
                    points: vec![[a0, a1], [b0, b1]],
                    width_m: Some(w),
                }
            })
            .collect(),
    )
}

#[test]
fn realignment_recovers_a_known_shift() {
    let log = sample::log(&Spec::new(ORIGIN));
    // The stored lines sit 0.30 m east and 2 m north of the rows actually driven.
    let set = stored_passes(12, 12.0, 0.30, 2.0);
    let r = agri::lines::realign(&set, Some(&log), &[], &RealignParams::default()).unwrap();
    assert_eq!(r.observability, "cross_track_only");
    assert!((r.shift_east_m + 0.30).abs() < 0.005, "{r:?}");
    assert_eq!(r.shift_north_m, 0.0, "nothing applied along parallel lines");
    assert!(r.lines.iter().all(|l| !l.flagged));
    // With one re-measured marker the along-line part is observed too.
    let f = LocalFrame::new(ORIGIN.0, ORIGIN.1, 0.0);
    let (s0, s1) = f.latlon(50.0, 52.0);
    let (n0, n1) = f.latlon(49.70, 50.0);
    let m = Marker::parse(&format!("{s0},{s1}:{n0},{n1}")).unwrap();
    let r = agri::lines::realign(&set, Some(&log), &[m], &RealignParams::default()).unwrap();
    assert_eq!(r.observability, "full");
    assert!(
        (r.shift_east_m + 0.30).abs() < 0.005 && (r.shift_north_m + 2.0).abs() < 0.01,
        "{r:?}"
    );
    assert!(r.u_east_m < 0.01 && r.u_north_m < 0.03);
    // The realigned lines carry the log's rows: realigning again finds no shift.
    let again = agri::lines::realign(
        r.realigned.as_ref().unwrap(),
        Some(&log),
        &[],
        &RealignParams::default(),
    )
    .unwrap();
    assert!(again.shift_east_m.abs() < 0.005, "{again:?}");
    // Nothing to work from.
    assert!(agri::lines::realign(&set, None, &[], &RealignParams::default()).is_err());
}

// ---------------------------------------------------------------- drift

#[test]
fn clean_passes_show_no_drift_and_little_overlap() {
    let log = sample::log(&Spec::new(ORIGIN));
    let line = Spec::new(ORIGIN).line();
    let r = drift::analyse(&log, &line, &drift::DriftParams::new(12.0)).unwrap();
    assert_eq!(r.passes.len(), 12);
    assert_eq!(r.pairs.len(), 11);
    assert!(r.pairs.iter().all(|p| p.class == "pass_to_pass"));
    assert!(
        r.pass_to_pass_rms_m.unwrap() < 0.02,
        "{:?}",
        r.pass_to_pass_rms_m
    );
    assert!(r.worst_within_pass_drift_m_per_min.unwrap() < 0.01);
    assert!(r.miss_ha < 0.002 && r.overlap_ha < 0.002);
    assert!(r.passes.iter().all(|p| (p.length_m - 300.0).abs() < 1.0));
}

#[test]
fn a_drifting_pass_is_found_with_its_miss_and_overlap() {
    let mut spec = Spec::new(ORIGIN);
    spec.drift = Some((4, 0.06)); // pass 4 walks 6 cm per minute to the left
    let log = sample::log(&spec);
    let r = drift::analyse(&log, &spec.line(), &drift::DriftParams::new(12.0)).unwrap();
    let p4 = r.passes.iter().find(|p| p.index == 4).unwrap();
    assert!((p4.drift_m_per_min - 0.06).abs() < 0.005, "{p4:?}");
    assert!(
        p4.start_dev_m.abs() < 0.01 && (p4.end_dev_m - 0.1).abs() < 0.01,
        "{p4:?}"
    );
    // Left (positive) moves pass 4 towards pass 5 and away from pass 3.
    let p34 = r.pairs.iter().find(|p| p.passes == [3, 4]).unwrap();
    let p45 = r.pairs.iter().find(|p| p.passes == [4, 5]).unwrap();
    assert!(p34.miss_m2 > 10.0 && p34.overlap_m2 < 1.0, "{p34:?}");
    assert!(p45.overlap_m2 > 10.0 && p45.miss_m2 < 1.0, "{p45:?}");
    assert!(p34.over_tolerance_m > 0.0);
    assert!(r.miss_ha > 0.001 && r.overlap_ha > 0.001);
    // A wider implement turns the miss into overlap.
    let mut p = drift::DriftParams::new(12.0);
    p.implement_m = 12.5;
    let w = drift::analyse(&log, &spec.line(), &p).unwrap();
    assert!(w.miss_ha < 1e-6 && w.overlap_ha > r.overlap_ha);
    // Pairs more than the gap apart are repeatability.
    let mut p = drift::DriftParams::new(12.0);
    p.max_gap_min = 1.0;
    let w = drift::analyse(&log, &spec.line(), &p).unwrap();
    assert!(w.pass_to_pass_rms_m.is_none() && w.repeatability_rms_m.is_some());
}

// ---------------------------------------------------------------- resilience

fn outage_log() -> MachineLog {
    let mut spec = Spec::new(ORIGIN);
    // Pass k runs from 120 k to 120 k + 100 s after the start.
    spec.outages = vec![
        (245.0, 30.0, OutageKind::Interference),
        (610.0, 60.0, OutageKind::Correction),
        (850.0, 40.0, OutageKind::Ionosphere),
    ];
    sample::log(&spec)
}

#[test]
fn outages_are_scored_and_told_apart() {
    let clean = resilience::score(&sample::log(&Spec::new(ORIGIN)), &Profile::default(), None);
    assert_eq!(clean.score, 100.0);
    assert_eq!(clean.grade, 'A');
    assert!(clean.episodes.is_empty());
    let r = resilience::score(&outage_log(), &Profile::default(), Some(&gfz()));
    assert_eq!(r.episodes.len(), 3, "{:#?}", r.episodes);
    let causes: Vec<Cause> = r.episodes.iter().map(|e| e.cause).collect();
    assert_eq!(
        causes,
        vec![
            Cause::InterferenceLike,
            Cause::CorrectionFeedLike,
            Cause::IonosphereLike
        ]
    );
    let i = &r.episodes[0];
    assert!(i.steering_dropped && i.worst_fix == Fix::None);
    assert!((i.recovery_s.unwrap() - 40.0).abs() < 1.5, "{i:?}");
    assert!(!r.episodes[1].steering_dropped);
    assert!(r.episodes[1].max_abs_xte_m.unwrap() > 0.1);
    assert!(r.score < 90.0 && r.score > 50.0, "{}", r.score);
    // The Kp series (same clock: the sample starts 2024-05-10T16:00Z, Kp 7.67) is cited as context.
    assert!(
        r.episodes[2]
            .evidence
            .iter()
            .any(|e| e.contains("Kp up to 7.67")),
        "{:?}",
        r.episodes[2]
    );
    // Weights renormalise when the log cannot inform a component.
    let mut no_xte = outage_log();
    for e in &mut no_xte.epochs {
        e.xte = None;
        e.steer = None;
    }
    let n = resilience::score(&no_xte, &Profile::default(), None);
    let assessed: Vec<&str> = n
        .components
        .iter()
        .filter(|c| c.value.is_some())
        .map(|c| c.name)
        .collect();
    assert_eq!(assessed, vec!["availability", "recovery"]);
    // Forestry needs only DGNSS: the RTK float spell no longer counts; the loss of
    // position and the fall to an autonomous fix still do.
    let f = resilience::score(
        &outage_log(),
        &Profile::for_class(MachineClass::Forestry),
        None,
    );
    let fc: Vec<Cause> = f.episodes.iter().map(|e| e.cause).collect();
    assert_eq!(
        fc,
        vec![Cause::InterferenceLike, Cause::CorrectionFeedLike],
        "{:?}",
        f.episodes
    );
    // Profiles parse and are checked.
    let p =
        Profile::parse(&toml::to_string(&Profile::for_class(MachineClass::Construction)).unwrap())
            .unwrap();
    assert_eq!(p.tolerance_m, 0.03);
    assert!(Profile::parse("format = \"x\"").is_err());
}

#[test]
fn nmea_logs_score_like_csv() {
    let log = outage_log();
    let nmea: String = log
        .epochs
        .iter()
        .map(|e| agri::log::gga(e) + "\n")
        .collect();
    let back = MachineLog::parse(&nmea).unwrap();
    assert_eq!(back.format, "nmea");
    assert_eq!(back.epochs.len(), log.epochs.len());
    let r = resilience::score(&back, &Profile::default(), None);
    // NMEA carries no steering, C/N0 or cross-track error: same episodes, fewer components.
    assert_eq!(r.episodes.len(), 3);
    assert_eq!(r.episodes[0].cause, Cause::InterferenceLike); // satellite count collapses
}

// ---------------------------------------------------------------- space weather

#[test]
fn kp_sources_parse() {
    let g = gfz();
    assert_eq!(g.values.len(), 64);
    let t = parse_utc("2024-05-11T00:00:00Z").unwrap();
    assert_eq!(g.at(t), Some(9.0));
    assert_eq!(g.at(t + 2 * 3600), Some(9.0));
    assert_eq!(g.at(t + 3 * 3600), Some(8.333));
    assert_eq!(g.f107_at(t), Some(213.7));
    assert_eq!(spaceweather::g_level(g.at(t).unwrap()), 5);
    let fc = KpSeries::parse(
        &std::fs::read_to_string(root().join("agri/data/swpc-3-day-forecast-2026-11-20.txt"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(fc.values.len(), 24);
    assert_eq!(
        fc.values[0],
        (parse_utc("2026-11-20T00:00Z").unwrap(), 0.67)
    );
    assert_eq!(fc.at(parse_utc("2026-11-22T12:30Z").unwrap()), Some(3.67));
    let js = KpSeries::parse(
        &std::fs::read_to_string(root().join("agri/data/swpc-planetary-k-index-sample.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(js.values.len(), 8);
    let old = r#"[["time_tag","Kp","a_running","station_count"],["2024-05-11 00:00:00.000","9.00","400","8"]]"#;
    assert_eq!(KpSeries::parse(old).unwrap().values[0].1, 9.0);
    // A forecast with a storm annotation and a year boundary.
    let storm = ":Issued: 2026 Dec 31 1230 UTC\nNOAA Kp index breakdown Dec 31-Jan 02 2027\n\n             Dec 31       Jan 01       Jan 02\n00-03UT       5.33 (G1)    2.00         1.00\n";
    let s = KpSeries::parse(storm).unwrap();
    assert_eq!(s.values[0], (parse_utc("2026-12-31T00:00Z").unwrap(), 5.33));
    assert_eq!(s.values[1].0, parse_utc("2027-01-01T00:00Z").unwrap());
}

fn site(lat: f64, lon: f64) -> Site {
    Site {
        lat,
        lon,
        baseline_km: 15.0,
        base: None,
        rtk_available: true,
        ppp_available: true,
    }
}

/// The May 2024 storm (Kp 9, G5, 10-11 May): farmers in the Canadian Prairies and the US
/// Midwest reported RTK autosteer problems on 10-11 May (Western Producer, 21 May 2024; 404
/// Media). On the published GFZ Kp series the advisory must be quiet before the storm and
/// no-go through it at those fields. Qualitative: dates, not receiver outcomes.
#[test]
fn may_2024_storm_backtest() {
    let kp = gfz();
    let p = AdvisorParams::default();
    let t0 = parse_utc("2024-05-08T00:00Z").unwrap();
    for (name, lat, lon) in [
        ("Manitoba", 49.9, -97.1),
        ("Iowa", 42.0, -93.5),
        ("Saskatchewan", 52.1, -106.6),
    ] {
        let a = spaceweather::advise(&site(lat, lon), &kp, None, t0, 7 * 24, &p, None);
        assert_eq!(a.hours.len(), 168);
        let at = |utc: &str| {
            a.hours
                .iter()
                .find(|h| h.utc == utc)
                .unwrap_or_else(|| panic!("{utc}"))
        };
        // Quiet days: nothing no-go on 8 and 9 May.
        for h in a
            .hours
            .iter()
            .filter(|h| h.utc.starts_with("2024-05-08") || h.utc.starts_with("2024-05-09"))
        {
            assert_ne!(h.status, Status::NoGo, "{name} {}", h.utc);
        }
        // The storm: no-go from 18 UT on 10 May through 21 UT on 11 May.
        let s = parse_utc("2024-05-10T18:00Z").unwrap();
        let e = parse_utc("2024-05-11T21:00Z").unwrap();
        for h in &a.hours {
            let t = parse_utc(&h.utc).unwrap();
            if t >= s && t <= e {
                assert_eq!(h.status, Status::NoGo, "{name} {}", h.utc);
                assert_eq!(h.scintillation, Risk::High);
                assert_eq!(h.method, "none");
            }
        }
        assert_eq!(at("2024-05-11T00:00:00Z").g, 5);
        // Recovery by 14 May.
        assert!(
            a.hours
                .iter()
                .filter(|h| h.utc.starts_with("2024-05-14"))
                .all(|h| h.status != Status::NoGo),
            "{name}"
        );
        let text = spaceweather::translate(&a);
        assert!(
            text.contains("2024-05-11: an extreme geomagnetic storm (G5)"),
            "{text}"
        );
        assert!(
            text.contains(
                "2024-05-09: no geomagnetic storm in the data. RTK autosteer should work normally."
            ),
            "{text}"
        );
    }
    // Mato Grosso, Brazil: near the magnetic equator; the high-latitude storm rules do not
    // apply; the post-sunset window is flagged (May is outside the American-sector season, so
    // caution, not no-go), daytime is go.
    let a = spaceweather::advise(&site(-12.5, -55.7), &kp, None, t0, 7 * 24, &p, None);
    assert!(a.region.starts_with("low latitude"), "{}", a.region);
    for h in &a.hours {
        let lst = h.local_solar_hour;
        if (19.0..24.0).contains(&lst) || lst < 2.0 {
            assert_eq!(h.scintillation, Risk::Moderate, "{}", h.utc);
        } else if h.g == 0 {
            assert_eq!(h.status, Status::Go, "{}", h.utc);
        }
    }
    // In season with high flux (November, F10.7 200): high.
    let nov = KpSeries {
        source: "test".into(),
        values: vec![(parse_utc("2024-11-05T00:00Z").unwrap(), 1.0)],
        f107: vec![],
    };
    let b = spaceweather::advise(
        &site(-12.5, -55.7),
        &nov,
        Some(200.0),
        parse_utc("2024-11-05T00:00Z").unwrap(),
        3,
        &p,
        None,
    );
    assert_eq!(b.hours[0].scintillation, Risk::High, "{:?}", b.hours[0]);
}

#[test]
fn rtk_error_grows_with_baseline_and_storm_and_ppp_is_offered() {
    let kp = gfz();
    let p = AdvisorParams {
        accuracy_m: 0.10,
        ..Default::default()
    };
    let quiet = parse_utc("2024-05-08T12:00Z").unwrap();
    let mut s = site(42.0, -93.5);
    s.baseline_km = 60.0;
    let a = spaceweather::advise(&s, &kp, None, quiet, 1, &p, None);
    // 10 mm + 60 ppm x 1 = 0.07 m; 2 sigma 0.14 > 0.10: RTK out, PPP (0.04, 2 sigma 0.08) in.
    assert!((a.hours[0].rtk_sigma_m.unwrap() - 0.07).abs() < 1e-9);
    assert_eq!(a.hours[0].method, "ppp");
    s.baseline_km = 5.0;
    let b = spaceweather::advise(&s, &kp, None, quiet, 1, &p, None);
    assert_eq!(b.hours[0].method, "rtk_or_ppp");
    s.ppp_available = false;
    let c = spaceweather::advise(
        &s,
        &kp,
        None,
        parse_utc("2024-05-11T00:00Z").unwrap(),
        1,
        &p,
        None,
    );
    assert!(c.hours[0].rtk_sigma_m.unwrap() > b.hours[0].rtk_sigma_m.unwrap());
    // The engine's Cornell model makes the levels concrete: deeper fades with S4.
    assert!(a.fade_fraction[0] < a.fade_fraction[1] && a.fade_fraction[1] < a.fade_fraction[2]);
}

/// A strict IONEX file (engine format) with two maps and a north-south TEC gradient.
fn ionex_with_gradient() -> String {
    let rec = |c: &str, label: &str| format!("{c:<60}{label}");
    let mut l = vec![
        rec(
            "     1.0            IONOSPHERE MAPS     GPS",
            "IONEX VERSION / TYPE",
        ),
        rec("  2024     5    10     0     0     0", "EPOCH OF FIRST MAP"),
        rec("  2024     5    11     0     0     0", "EPOCH OF LAST MAP"),
        rec(" 86400", "INTERVAL"),
        rec("     2", "# OF MAPS IN FILE"),
        rec("  NONE", "MAPPING FUNCTION"),
        rec("  6371.0", "BASE RADIUS"),
        rec("   450.0 450.0   0.0", "HGT1 / HGT2 / DHGT"),
        rec("    50.0  40.0 -10.0", "LAT1 / LAT2 / DLAT"),
        rec("  -180.0 180.0  20.0", "LON1 / LON2 / DLON"),
        rec("    -1", "EXPONENT"),
        rec("", "END OF HEADER"),
    ];
    let row = |v: i32| -> String {
        let s: Vec<String> = (0..19).map(|_| format!("{v:5}")).collect();
        format!("{}\n{}", s[..16].concat(), s[16..].concat())
    };
    for (k, ep) in [
        (1, "  2024     5    10     0     0     0"),
        (2, "  2024     5    11     0     0     0"),
    ] {
        l.push(rec(&format!("{k:6}"), "START OF TEC MAP"));
        l.push(rec(ep, "EPOCH OF CURRENT MAP"));
        // 50 N: 60 TECU; 40 N: 20 TECU (values x 0.1).
        l.push(rec(
            "    50.0-180.0 180.0  20.0 450.0",
            "LAT/LON1/LON2/DLON/H",
        ));
        l.push(row(600));
        l.push(rec(
            "    40.0-180.0 180.0  20.0 450.0",
            "LAT/LON1/LON2/DLON/H",
        ));
        l.push(row(200));
        l.push(rec(&format!("{k:6}"), "END OF TEC MAP"));
    }
    l.push(rec("", "END OF FILE"));
    l.join("\n")
}

#[test]
fn ionex_gradient_enters_the_rtk_estimate() {
    let file = kshana::ionex::parse_ionex_file(&ionex_with_gradient()).unwrap();
    let ix = spaceweather::Ionex { file: &file };
    let mut s = site(45.0, -93.5);
    s.base = Some((44.0, -93.5)); // 1 deg south: 4 TECU lower
    s.baseline_km = 111.0;
    let kp = gfz();
    let t = parse_utc("2024-05-10T06:00Z").unwrap();
    let p = AdvisorParams::default();
    let with = spaceweather::advise(&s, &kp, None, t, 1, &p, Some(&ix));
    let without = spaceweather::advise(&s, &kp, None, t, 1, &p, None);
    let d = with.hours[0].iono_l1_diff_m.unwrap();
    // 4 TECU on L1 is about 0.65 m.
    assert!(
        (d - 40.3e16 * 4.0 / 1575.42e6f64.powi(2)).abs() < 0.01,
        "{d}"
    );
    assert!(with.hours[0].rtk_sigma_m.unwrap() > without.hours[0].rtk_sigma_m.unwrap() + 0.3);
}

// ---------------------------------------------------------------- field planner

#[test]
fn field_plan_and_working_window() {
    let b = field::read_boundary(
        &std::fs::read_to_string(root().join("agri/examples/field.example.geojson")).unwrap(),
    )
    .unwrap();
    let line = LineSet::read(
        &std::fs::read_to_string(root().join("agri/examples/ab-line.example.json")).unwrap(),
        None,
    )
    .unwrap();
    let plan = field::plan(
        &b,
        &field::PlanParams {
            heading_deg: line.lines[0].heading_deg(),
            width_m: 12.0,
            speed_kmh: 8.0,
            turn_s: 30.0,
            min_line_m: 5.0,
        },
    )
    .unwrap();
    // The sample field is 86 m by 320 m: seven 12 m swaths of 320 m.
    assert_eq!(plan.passes, 7);
    assert!((plan.path_m - 2240.0).abs() < 1.0);
    assert!((plan.field_ha - 2.752).abs() < 0.01);
    // A concave (L-shaped) field splits the long swaths.
    let f = LocalFrame::new(ORIGIN.0, ORIGIN.1, 0.0);
    let c = |e: f64, n: f64| {
        let (a, b) = f.latlon(e, n);
        [a, b]
    };
    let l_shape = vec![
        c(0.0, 0.0),
        c(100.0, 0.0),
        c(100.0, 50.0),
        c(40.0, 50.0),
        c(40.0, 200.0),
        c(0.0, 200.0),
    ];
    let p = field::plan(
        &l_shape,
        &field::PlanParams {
            heading_deg: 90.0,
            width_m: 10.0,
            speed_kmh: 10.0,
            turn_s: 0.0,
            min_line_m: 5.0,
        },
    )
    .unwrap();
    assert_eq!(p.passes, 20);
    assert!(
        (p.path_m - (5.0 * 100.0 + 15.0 * 40.0)).abs() < 1.0,
        "{}",
        p.path_m
    );
    // A window on the May 2024 advisory for Manitoba: the first suitable run, and none of
    // the storm hours.
    let a = spaceweather::advise(
        &site(49.9, -97.1),
        &gfz(),
        None,
        parse_utc("2024-05-10T15:00Z").unwrap(),
        24,
        &AdvisorParams::default(),
        None,
    );
    let j = serde_json::to_string(&a).unwrap();
    let w = field::window(&j, 3.0, false).unwrap();
    assert!(w.is_none(), "{w:?}");
    let a = spaceweather::advise(
        &site(49.9, -97.1),
        &gfz(),
        None,
        parse_utc("2024-05-12T00:00Z").unwrap(),
        72,
        &AdvisorParams::default(),
        None,
    );
    let w = field::window(&serde_json::to_string(&a).unwrap(), 3.0, false)
        .unwrap()
        .unwrap();
    assert!(w.start_utc.as_str() >= "2024-05-12T12:00:00Z", "{w:?}");
}

// ---------------------------------------------------------------- photos

#[test]
fn photo_checks_flag_inconsistencies() {
    let f = LocalFrame::new(ORIGIN.0, ORIGIN.1, 0.0);
    let c = |e: f64, n: f64| {
        let (a, b) = f.latlon(e, n);
        [a, b]
    };
    let parcel = vec![c(0.0, 0.0), c(200.0, 0.0), c(200.0, 200.0), c(0.0, 200.0)];
    let t = parse_utc("2024-06-03T14:00:00Z").unwrap();
    let at = |e: f64, n: f64, dt: i64| {
        let (a, b) = f.latlon(e, n);
        photo::Exif {
            lat: Some(a),
            lon: Some(b),
            gps_utc: Some(t + dt),
            hpe_m: Some(4.0),
            make: Some("Cam".into()),
            camera_time: Some(t + dt - 5 * 3600),
            offset: Some("-05:00".into()),
            ..Default::default()
        }
    };
    let good = at(100.0, 100.0, 0);
    let outside = at(400.0, 100.0, 600);
    let mut clock = at(110.0, 120.0, 1200);
    clock.camera_time = Some(t + 1200 - 5 * 3600 + 3000);
    let mut no_offset = at(120.0, 130.0, 1800);
    no_offset.offset = None; // -5 h zone, consistent
    let mut rounded = at(10.0, 10.0, 2400);
    rounded.lat = Some(49.9001);
    rounded.lon = Some(-97.0999);
    let mut copied = good.clone();
    copied.gps_utc = Some(t + 3600);
    copied.camera_time = Some(t + 3600 - 5 * 3600);
    copied.software = Some("Photo Editor Pro".into());
    let none = photo::Exif::default();
    let files: Vec<(String, Vec<u8>)> = [
        ("a-good.jpg", &good),
        ("b-outside.jpg", &outside),
        ("c-clock.jpg", &clock),
        ("d-no-offset.jpg", &no_offset),
        ("e-rounded.jpg", &rounded),
        ("f-copied.jpg", &copied),
        ("g-none.jpg", &none),
    ]
    .iter()
    .map(|(n, x)| (n.to_string(), photo::synth_jpeg(x)))
    .collect();
    let mut p = photo::PhotoParams {
        window: Some((
            parse_utc("2024-06-03").unwrap(),
            parse_utc("2024-06-03T15:30:00Z").unwrap(),
        )),
        ..Default::default()
    };
    let r = photo::check(&files, &parcel, &p);
    let res = |file: &str, id: &str| {
        r.photos
            .iter()
            .find(|x| x.file == file)
            .unwrap()
            .checks
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.result)
            .unwrap_or("absent")
    };
    assert_eq!(
        r.photos
            .iter()
            .find(|x| x.file == "a-good.jpg")
            .unwrap()
            .flags,
        0
    );
    assert_eq!(res("b-outside.jpg", "inside_parcel"), "flag");
    assert_eq!(res("c-clock.jpg", "gps_vs_camera_time"), "flag");
    assert_eq!(res("d-no-offset.jpg", "gps_vs_camera_time"), "pass");
    assert_eq!(res("e-rounded.jpg", "coordinate_precision"), "flag");
    assert_eq!(res("f-copied.jpg", "software_tag"), "info");
    assert_eq!(res("f-copied.jpg", "claimed_window"), "pass");
    assert_eq!(res("g-none.jpg", "gps_present"), "flag");
    assert_eq!(res("g-none.jpg", "inside_parcel"), "not_assessed");
    let set = |id: &str| r.set_checks.iter().find(|c| c.id == id).unwrap();
    assert_eq!(set("repeated_coordinates").result, "flag");
    assert!(set("repeated_coordinates")
        .detail
        .contains("a-good.jpg and f-copied.jpg"));
    // 300 m in 600 s is fine; the outside photo is 300 m from a-good in 600 s: no speed flag.
    assert_eq!(set("travel_speed").result, "pass");
    // Out of the claimed window.
    p.window = Some((
        parse_utc("2024-06-04").unwrap(),
        parse_utc("2024-06-05").unwrap(),
    ));
    let r2 = photo::check(&files[..1], &parcel, &p);
    assert_eq!(
        r2.photos[0]
            .checks
            .iter()
            .find(|c| c.id == "claimed_window")
            .unwrap()
            .result,
        "flag"
    );
    assert!(r.caveat.contains("not a finding"));
    let sample = std::fs::read(root().join("agri/examples/photo.example.jpg")).unwrap();
    assert!(photo::read_exif(&sample).unwrap().lat.is_some());
}

// ---------------------------------------------------------------- licence, wording, CLI

#[test]
fn licence_features_and_tiers() {
    for (f, t) in [
        (agri::FEATURE_GUIDANCE, Tier::Pro),
        (agri::FEATURE_RESILIENCE, Tier::Pro),
        (agri::FEATURE_ADVISOR, Tier::Pro),
        (agri::FEATURE_PHOTO, Tier::Enterprise),
    ] {
        assert_eq!(license::explicit_tier(f), Some(t), "{f}");
        assert!(license::PRO_COMMAND_FEATURES.contains(&f), "{f}");
    }
}

/// The wording rule: no claim words in the module, the CLI or the docs page.
#[test]
fn wording_rule_holds() {
    let words = [
        concat!("compl", "iant"),
        concat!("compl", "iance"),
        concat!("con", "form"),
        concat!("certi", "fied"),
    ];
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("src/agri"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.push(root().join("src/cli/agri_cmd.rs"));
    files.push(root().join("docs/AGRI-SURVEY-OPERATIONS.md"));
    for f in files {
        let t = std::fs::read_to_string(&f).unwrap().to_lowercase();
        for w in words {
            assert!(!t.contains(w), "{} contains '{w}'", f.display());
        }
    }
}

fn bin(args: &[&str], home: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

const SEED: [u8; 32] = [35u8; 32];

#[test]
fn cli_end_to_end() {
    let d = tmp("cli");
    let home = d.join("home");
    let p = |x: &Path| x.to_str().unwrap().to_string();
    let log = d.join("log.csv");
    let line = d.join("line.json");
    let kp = p(&root().join("agri/data/gfz-kp-2024-05-07-to-14.txt"));
    let (ok, out) = bin(
        &[
            "agri",
            "sample",
            "log",
            "--out",
            &p(&log),
            "--drift",
            "4:0.06",
            "--outage",
            "245:30:interference",
        ],
        &home,
    );
    assert!(ok, "{out}");
    let (ok, out) = bin(&["agri", "sample", "line", "--out", &p(&line)], &home);
    assert!(ok, "{out}");
    // Free: convert and translate.
    let (ok, out) = bin(&["agri", "convert", &p(&line), "--to", "isoxml"], &home);
    assert!(
        ok && out.contains("<GPN") && out.contains("LSG A=\"5\""),
        "{out}"
    );
    let (ok, out) = bin(
        &[
            "agri",
            "convert",
            "--a",
            "49.9,-97.1",
            "--heading",
            "30",
            "--to",
            "geojson",
        ],
        &home,
    );
    assert!(
        ok && out.contains("LineString") && out.contains("heading 30.0000"),
        "{out}"
    );
    let (ok, out) = bin(
        &[
            "agri",
            "translate",
            "--at",
            "49.9,-97.1",
            "--kp",
            &kp,
            "--from",
            "2024-05-10T00:00Z",
            "--hours",
            "48",
        ],
        &home,
    );
    assert!(
        ok && out.contains("G5") && out.contains("from 15:00 to 24:00"),
        "{out}"
    );
    // Gated without a licence.
    for args in [
        vec![
            "agri",
            "drift",
            "--log",
            &p(&log),
            "--line",
            &p(&line),
            "--width",
            "12",
        ],
        vec!["agri", "advise", "--at", "49.9,-97.1", "--kp", &kp],
    ] {
        let (ok, out) = bin(&args, &home);
        assert!(!ok && out.contains("requires the 'pro' tier"), "{out}");
    }
    let (ok, out) = bin(&["agri", "photo", &p(&d), "--parcel", "x"], &home);
    assert!(!ok && out.contains("tier"), "{out}");
    // Drift with signed output, then verify.
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let signed = d.join("drift");
    let (ok, out) = bin(
        &[
            "agri",
            "drift",
            "--log",
            &p(&log),
            "--line",
            &p(&line),
            "--width",
            "12",
            "--out",
            &p(&signed),
            "--signer",
            "Dealer",
            "--key-file",
            &p(&key),
            "--utc",
            "2026-11-20T10:00:00Z",
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && out.contains("Pass-to-pass drift") && out.contains("| 4 |"),
        "{out}"
    );
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, out) = bin(&["agri", "verify", &p(&signed), "--trust", &pk], &home);
    assert!(ok && out.contains("trusted"), "{out}");
    let (ok, out) = bin(
        &[
            "agri",
            "drift",
            "--log",
            &p(&log),
            "--line",
            &p(&line),
            "--width",
            "12",
            "--out",
            &p(&signed),
            "--signer",
            "Dealer",
            "--key-file",
            &p(&key),
            "--dev",
        ],
        &home,
    );
    assert!(!ok && out.contains("never overwritten"), "{out}");
    // Resilience (JSON), advise, field with the advisory, realign.
    let (ok, out) = bin(
        &["agri", "resilience", "--log", &p(&log), "--json", "--dev"],
        &home,
    );
    assert!(
        ok && out.contains("\"interference_like\"") && out.contains("\"grade\""),
        "{out}"
    );
    let adv = d.join("adv.json");
    let (ok, out) = bin(
        &[
            "agri",
            "advise",
            "--at",
            "49.9,-97.1",
            "--kp",
            &kp,
            "--from",
            "2024-05-12T00:00Z",
            "--hours",
            "48",
            "--base",
            "49.95,-97.2",
            "--out",
            &p(&adv),
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && out.contains("geomagnetic latitude 58") && out.contains("no-go"),
        "{out}"
    );
    let (ok, out) = bin(
        &[
            "agri",
            "field",
            "--boundary",
            &p(&root().join("agri/examples/field.example.geojson")),
            "--width",
            "12",
            "--line",
            &p(&line),
            "--advisory",
            &p(&adv),
            "--to",
            "kml",
            "--out",
            &p(&d.join("plan.kml")),
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && out.contains("7 passes") && out.contains("window from 2024-05-12T"),
        "{out}"
    );
    let (ok, out) = bin(
        &[
            "agri",
            "realign",
            "--lines",
            &p(&line),
            "--marker",
            "49.9,-97.1:49.90001,-97.1",
            "--min-fix",
            "rtk_fixed",
            "--to",
            "csv",
            "--out",
            &p(&d.join("re.csv")),
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && out.contains("north +1.11") && out.contains("full"),
        "{out}"
    );
    // Photos (Enterprise via --dev).
    let photos = d.join("photos");
    std::fs::create_dir_all(&photos).unwrap();
    let (ok, out) = bin(
        &[
            "agri",
            "sample",
            "photo",
            "--out",
            &p(&photos.join("p1.jpg")),
        ],
        &home,
    );
    assert!(ok, "{out}");
    let (ok, out) = bin(
        &[
            "agri",
            "photo",
            &p(&photos),
            "--parcel",
            &p(&root().join("agri/examples/field.example.geojson")),
            "--from",
            "2024-05-10",
            "--to",
            "2024-05-10",
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && out.contains("1 photos, 0 with at least one flag"),
        "{out}"
    );
}
