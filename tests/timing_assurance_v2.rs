// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Timing assurance v2 (W1120 TA1-TA4): time-bomb vectors and their analyser, the fleet check
//! against the advisory table, the clock self-check and cross-site correlation on the
//! SYNTHETIC demo logs of `examples/timing-assurance-v2/`, the holdover evidence against
//! closed forms, and the new items of the timing evidence packs.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::timing::compliance::{self, Inputs, Verdict};
use kshana_pro::timing::holdover_evidence::{self as he, HoldoverSpec};
use kshana_pro::timing::selfcheck::{self as sc, NodeSpec, SitesSpec};
use kshana_pro::timing::timebomb::{self as tb, CatalogueOptions, DeviceRecord, Label};

const EX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/timing-assurance-v2");
/// 2026-11-19T12:00:00Z.
const NOW: i64 = 1_795_089_600;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ta2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cat() -> Vec<tb::VectorSpec> {
    tb::catalogue(&CatalogueOptions {
        now_unix: NOW,
        pivots: vec![Label::parse_iso("1999-06-01").unwrap()],
        ..Default::default()
    })
}

fn vector(id: &str) -> tb::VectorSpec {
    cat().into_iter().find(|v| v.id == id).unwrap()
}

/// The expected epochs as device records (a correct device), optionally transformed.
fn records(v: &tb::VectorSpec, f: impl Fn(usize, Label) -> Option<Label>) -> Vec<DeviceRecord> {
    v.epochs()
        .iter()
        .filter_map(|e| {
            f(e.k, e.utc).map(|utc| DeviceRecord {
                lab_s: Some(1_900_000_000.0 + e.k as f64),
                utc,
                valid: true,
            })
        })
        .collect()
}

// ---------------------------------------------------------------- vectors

#[test]
fn hazard_epochs_are_at_the_right_dates() {
    let at = |id: &str| vector(id).boundary.iso();
    assert_eq!(at("gps-wnro-2038"), "2038-11-20T23:59:42Z");
    assert_eq!(at("gps-wn8192"), "2137-01-05T23:59:42Z");
    assert_eq!(at("ntp-era-2036"), "2036-02-07T06:28:16Z");
    assert_eq!(at("unix-2038"), "2038-01-19T03:14:08Z");
    assert_eq!(at("rmc-century-2080"), "2080-01-01T00:00:00Z");
    assert_eq!(at("leap-insert"), "2027-07-01T00:00:00Z");
    // Pivot 1999-06-01 is in GPS week 1012: the fall-back is at week 2036.
    let p = vector("pivot-rollover-1999-06-01");
    assert_eq!(p.boundary.iso(), "2019-01-12T23:59:42Z");
    // The next 128-week crossing after 19 Nov 2026 of the 2016 leap week (1929): week 2569.
    let l = vector("lsf-ambiguity");
    assert!(
        l.title.contains("640 weeks") && l.title.contains("week 2569"),
        "{}",
        l.title
    );
    assert_eq!(
        tb::gps_week_tow(l.boundary.seconds() - tb::GPS_EPOCH_UNIX + 18),
        (2569, 0)
    );
    assert_eq!(l.lnav.wn_lsf_8bit, 1929 % 256);
}

#[test]
fn leap_vectors_insert_and_delete_the_second_and_gps_time_runs_on() {
    let v = vector("leap-insert");
    let e = v.epochs();
    assert_eq!(e.len(), 300);
    let i = e
        .iter()
        .position(|x| x.utc.sod == 86_400)
        .expect("23:59:60 present");
    assert_eq!(e[i].utc.iso(), "2027-06-30T23:59:60Z");
    assert_eq!(e[i - 1].utc.iso(), "2027-06-30T23:59:59Z");
    assert_eq!(e[i + 1].utc.iso(), "2027-07-01T00:00:00Z");
    assert_eq!(v.leap_index(), Some(i));
    assert_eq!((e[i].gps_minus_utc_s, e[i + 1].gps_minus_utc_s), (18, 19));
    assert!(e.windows(2).all(|w| w[1].gps_s == w[0].gps_s + 1));
    let d = vector("leap-delete");
    let e = d.epochs();
    let i = e
        .iter()
        .position(|x| x.utc.iso() == "2027-06-30T23:59:58Z")
        .unwrap();
    assert_eq!(e[i + 1].utc.iso(), "2027-07-01T00:00:00Z");
    assert_eq!(e[i + 1].gps_minus_utc_s, 17);
    // The event marker of a deletion is the epoch after 23:59:58 (it used to be lost).
    assert_eq!(d.leap_index(), Some(i + 1));
    assert_eq!(v.lnav.delta_t_lsf_s, 19);
    assert_eq!(d.lnav.delta_t_lsf_s, 17);
}

#[test]
fn nmea_and_ubx_vectors_read_back_to_the_expected_epochs() {
    for v in cat() {
        let want: Vec<Label> = v.epochs().iter().map(|e| e.utc).collect();
        let (n, bad) = tb::read_nmea(&tb::nmea(&v, &tb::Position::default()));
        assert_eq!(bad, 0, "{}", v.id);
        assert_eq!(
            n.iter().map(|r| r.utc).collect::<Vec<_>>(),
            want,
            "{} nmea",
            v.id
        );
        let (u, bad) = tb::read_ubx(&tb::ubx(&v));
        assert_eq!(bad, 0);
        assert_eq!(
            u.iter().map(|r| r.utc).collect::<Vec<_>>(),
            want,
            "{} ubx",
            v.id
        );
        assert!(u.iter().all(|r| r.valid));
        // The analyser finds nothing in a correct device's output.
        let a = tb::analyse(&v, &n, "nmea", 0);
        assert!(a.faults.is_empty(), "{}: {:?}", v.id, a.faults);
        assert_eq!(a.verdict, "no fault seen in this replay");
    }
    // A corrupted checksum is counted, not read.
    let text = tb::nmea(&vector("unix-2038"), &tb::Position::default()).replacen("*", "*0", 1);
    assert!(tb::read_nmea(&text).1 >= 1);
}

#[test]
fn generate_writes_hashed_files_and_a_readable_vector() {
    let d = tmp("gen");
    let specs: Vec<_> = cat()
        .into_iter()
        .filter(|v| v.id == "gps-wnro-2038")
        .collect();
    let m = tb::generate(&d, &specs, &tb::Position::default()).unwrap();
    for f in [
        "vector.json",
        "replay.nmea",
        "replay.ubx",
        "expected.csv",
        "simulator.json",
    ] {
        assert!(d.join("gps-wnro-2038").join(f).exists(), "{f}");
    }
    assert_eq!(m["vectors"][0]["files"].as_array().unwrap().len(), 5);
    let back = tb::read_vector(&d.join("gps-wnro-2038")).unwrap();
    assert_eq!(back, specs[0]);
    let sim: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(d.join("gps-wnro-2038/simulator.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sim["start_gps"]["week"], 3071);
    assert_eq!(sim["start_gps"]["week_mod_1024"], 1023);
    let cl = sim["gps_sdr_sim"]["command_line"].as_str().unwrap();
    assert!(
        cl.contains("-T 2038/11/20,23:58:00") && cl.contains("-d 300"),
        "{cl}"
    );
    let csv = std::fs::read_to_string(d.join("gps-wnro-2038/expected.csv")).unwrap();
    assert!(
        csv.contains("120,2038-11-20T23:59:42Z,3072,0,18,boundary"),
        "{csv}"
    );
}

// ---------------------------------------------------------------- analyser

#[test]
fn analyser_names_the_classic_faults() {
    let w = vector("gps-wnro-2038");
    let b = w.boundary;
    let a = tb::analyse(
        &w,
        &records(&w, |_, l| {
            Some(if l >= b {
                Label::from_unix(l.seconds() - 1024 * tb::WEEK_S)
            } else {
                l
            })
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults.len(), 1, "{:?}", a.faults);
    assert_eq!(a.faults[0].kind, "week-rollover");
    assert_eq!(a.faults[0].step_s, -619_315_200);
    assert_eq!(a.faults[0].expected_utc, "2038-11-20T23:59:42Z");
    assert_eq!(a.faults[0].device_utc, "2019-04-06T23:59:42Z");
    // NTP era: a jump back by 2^32 s.
    let n = vector("ntp-era-2036");
    let nb = n.boundary;
    let a = tb::analyse(
        &n,
        &records(&n, |_, l| {
            Some(if l >= nb {
                Label::from_unix(l.seconds() - (1i64 << 32))
            } else {
                l
            })
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults[0].kind, "wrap-32");
    // Century: a device that reads 2080 as 1980.
    let c = vector("rmc-century-2080");
    let cb = c.boundary;
    let a = tb::analyse(
        &c,
        &records(&c, |_, l| {
            Some(if l >= cb {
                let (_, m, d) = l.ymd();
                Label {
                    days: kshana_eloran::time::days_from_civil(1980, m, d),
                    sod: l.sod,
                }
            } else {
                l
            })
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults[0].kind, "century");
    // GPS time given as UTC from the start.
    let u = vector("unix-2038");
    let a = tb::analyse(
        &u,
        &records(&u, |_, l| Some(Label::from_unix(l.seconds() + 18))),
        "csv",
        0,
    );
    assert_eq!(a.faults[0].kind, "gps-utc-offset");
    assert_eq!(a.initial_offset_s, Some(18));
    // A spurious leap second at the 128-week crossing.
    let s = vector("lsf-ambiguity");
    let sb = s.boundary;
    let a = tb::analyse(
        &s,
        &records(&s, |_, l| {
            Some(if l > sb {
                Label::from_unix(l.seconds() - 1)
            } else {
                l
            })
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults[0].kind, "unexpected-leap");
    // Stuck time: the device freezes for 20 s.
    let a = tb::analyse(
        &u,
        &records(&u, |k, l| {
            Some(if (150..170).contains(&k) {
                Label::from_unix(u.epochs()[150].utc.seconds())
            } else {
                l
            })
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults.len(), 1, "{:?}", a.faults);
    assert_eq!(a.faults[0].kind, "stuck-time");
    // No output for 30 s.
    let a = tb::analyse(
        &u,
        &records(&u, |k, l| (!(100..130).contains(&k)).then_some(l)),
        "csv",
        0,
    );
    assert_eq!(a.faults[0].kind, "no-output");
    assert!(a.faults[0].detail.contains("30 s"));
}

#[test]
fn analyser_reads_leap_second_handling() {
    let v = vector("leap-insert");
    let li = v.leap_index().unwrap();
    let eps = v.epochs();
    // Not inserted: the device counts on without 23:59:60.
    let a = tb::analyse(
        &v,
        &records(&v, |k, _| {
            Some(Label::from_unix(eps[0].utc.seconds() + k as i64))
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults.len(), 1, "{:?}", a.faults);
    assert_eq!(a.faults[0].kind, "leap-not-applied");
    assert_eq!(a.faults[0].step_s, 1);
    // Shown as a repeated 23:59:59: a note, not a fault.
    let a = tb::analyse(
        &v,
        &records(&v, |k, l| {
            Some(if k + 1 == li { eps[k - 1].utc } else { l })
        }),
        "csv",
        0,
    );
    assert!(a.faults.is_empty(), "{:?}", a.faults);
    assert!(
        a.notes.iter().any(|n| n.contains("repeated 23:59:59")),
        "{:?}",
        a.notes
    );
    // Applied a minute early.
    let a = tb::analyse(
        &v,
        &records(&v, |k, l| {
            Some(if k + 61 >= li && k + 1 < li {
                Label::from_unix(l.seconds() - 1)
            } else {
                l
            })
        }),
        "csv",
        0,
    );
    assert!(
        a.faults.iter().any(|f| f.kind == "leap-mistimed"),
        "{:?}",
        a.faults
    );
    // The committed demo capture.
    let cap = std::fs::read(format!("{EX}/timebomb/captures/leap-not-inserted.nmea")).unwrap();
    let (r, _) = tb::read_capture("nmea", &cap).unwrap();
    let a = tb::analyse(&v, &r, "nmea", 0);
    assert_eq!(
        a.faults.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(),
        ["leap-not-applied"]
    );
    assert!(a.pairing.contains("in order"));
}

#[test]
fn demo_wnro_capture_with_lab_time_stamps() {
    let cap = std::fs::read(format!("{EX}/timebomb/captures/wnro-faulty.nmea")).unwrap();
    let (r, bad) = tb::read_capture("nmea", &cap).unwrap();
    assert_eq!((r.len(), bad), (300, 0));
    let a = tb::analyse(&vector("gps-wnro-2038"), &r, "nmea", bad);
    assert!(a.pairing.contains("lab time"));
    assert_eq!(a.faults.len(), 1);
    assert_eq!(a.faults[0].kind, "week-rollover");
    assert!(a.faults[0].detail.contains("1 x 1024 weeks"));
    // UBX NAV-TIMEGPS only (no TIMEUTC): GPS - leapS.
    let v = vector("gps-wnro-2038");
    let bytes: Vec<u8> = {
        let all = tb::ubx(&v);
        let mut out = Vec::new();
        let mut i = 0;
        while i + 6 <= all.len() {
            let len = u16::from_le_bytes([all[i + 4], all[i + 5]]) as usize;
            if all[i + 3] == 0x20 {
                out.extend_from_slice(&all[i..i + 8 + len]);
            }
            i += 8 + len;
        }
        out
    };
    let (r, _) = tb::read_ubx(&bytes);
    assert_eq!(r.len(), 300);
    assert!(tb::analyse(&v, &r, "ubx", 0).faults.is_empty());
}

// ---------------------------------------------------------------- fleet

#[test]
fn fleet_check_matches_advisories_and_epoch_hazards() {
    let inv: tb::Inventory =
        toml::from_str(&std::fs::read_to_string(format!("{EX}/timebomb/inventory.toml")).unwrap())
            .unwrap();
    let r = tb::fleet_check(
        &inv,
        &tb::AdvisoryTable::seed(),
        NOW,
        "2026-11-19T12:00:00Z",
    )
    .unwrap();
    let dev = |id: &str| r.devices.iter().find(|d| d.id == id).unwrap();
    let find = |d: &tb::DeviceResult, id: &str| d.findings.iter().find(|f| f.id == id).cloned();
    let gm1 = dev("gm-ldn-1");
    assert_eq!(find(gm1, "CVE-2025-47900").unwrap().status, "affected");
    assert_eq!(find(gm1, "CVE-2024-9054").unwrap().status, "review");
    assert_eq!(gm1.worst, "act");
    assert_eq!(
        find(dev("gm-ldn-2"), "CVE-2025-47901").unwrap().status,
        "not-affected"
    );
    // The firmware-date pivot: 2024-03-01 -> week 2303 + 1024.
    let p = find(gm1, "pivot-rollover-2024-03-01").unwrap();
    assert_eq!(p.date.as_deref(), Some("2043-10-10"));
    // The legacy GPSDO: pivot date long past, 32-bit time_t stated, community report matched.
    let g = dev("gpsdo-lab");
    assert_eq!(
        find(g, "pivot-rollover-1999-06-01").unwrap().priority,
        "act"
    );
    assert_eq!(find(g, "unix-2038").unwrap().status, "affected");
    assert!(find(g, "TIME-NUTS-2024-11-Z3805A").is_some());
    // Meinberg: below the 7.10.006 of MBGSA-2025.07; reads RMC so the 2080 vector is listed.
    let m = dev("ntp-fra-1");
    assert_eq!(find(m, "MBGSA-2025.07").unwrap().status, "affected");
    assert!(find(m, "rmc-century-2080").is_some());
    // Full week resolution and no NTP / 32-bit time: those hazards are not listed.
    let e = dev("edge-gm-7");
    assert!(
        find(e, "gps-wnro-2038").is_none()
            && find(e, "ntp-era-2036").is_none()
            && find(e, "unix-2038").is_none()
    );
    assert_eq!(find(e, "EXAMPLE-0001").unwrap().status, "affected");
    assert!(tb::fleet_csv(&r).lines().count() > 20);
    // Table rules: a CVE id belongs to a vendor advisory; an example id starts EXAMPLE-.
    let bad = "[[advisory]]\nid = \"CVE-2099-1\"\nkind = \"community-report\"\nvendor = \"x\"\nmodels = [\"*\"]\nhazard = \"h\"\nsource = \"s\"\nverification = \"v\"\n";
    assert!(tb::AdvisoryTable::parse(bad).is_err());
    assert!(tb::AdvisoryTable::parse(
        &bad.replace("CVE-2099-1", "X-1")
            .replace("community-report", "example")
    )
    .is_err());
    // Every seed CVE is one the research report names.
    let research = [
        "CVE-2024-9054",
        "CVE-2024-43684",
        "CVE-2024-43685",
        "CVE-2025-47900",
        "CVE-2025-47901",
        "CVE-2025-47904",
    ];
    for a in tb::AdvisoryTable::seed().advisory {
        if a.id.starts_with("CVE-") {
            assert!(research.contains(&a.id.as_str()), "{}", a.id);
        }
        assert!(!a.source.is_empty() && !a.verification.is_empty());
    }
    assert!(tb::version_cmp("2.4.1", "2.5").is_lt() && tb::version_cmp("v2.10", "2.9").is_gt());
}

// ---------------------------------------------------------------- self-check

#[test]
fn node_self_check_finds_the_wrong_time_behind_a_healthy_gnss() {
    let spec: NodeSpec =
        toml::from_str(&std::fs::read_to_string(format!("{EX}/node/node.toml")).unwrap()).unwrap();
    let r = sc::check_node(&spec, Path::new(&format!("{EX}/node"))).unwrap();
    let g: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.status == "gnss-outlier")
        .collect();
    assert_eq!(g.len(), 1, "{:#?}", r.events);
    // 03:00 to 05:00 UTC on 2026-11-19; 6.25 ms.
    assert_eq!(g[0].start_unix, 1_795_046_400.0 + 3.0 * 3600.0);
    assert_eq!(g[0].end_unix, 1_795_046_400.0 + 5.0 * 3600.0);
    let m = g[0].max_abs_consensus_s.unwrap();
    assert!((m - 6.25e-3).abs() < 2e-6, "{m}");
    assert_eq!(g[0].sources, ["gnss-pps"]);
    assert!(
        r.verdict.contains("while its GNSS looked healthy"),
        "{}",
        r.verdict
    );
    // The PTP path-delay step at 06:00.
    let d: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.status == "path-delay-step")
        .collect();
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].start_unix, 1_795_046_400.0 + 6.0 * 3600.0);
    assert_eq!(r.counts.get("gnss-outlier"), Some(&60));
    assert!(sc::node_bins_csv(&r).lines().count() > 200);
    // With the GNSS alone, the time cannot be validated.
    let mut one = spec.clone();
    one.sources.truncate(1);
    let r = sc::check_node(&one, Path::new(&format!("{EX}/node"))).unwrap();
    assert!(r.verdict.contains("cannot be validated"), "{}", r.verdict);
    // Two sources: a disagreement is seen but not attributed.
    let mut two = spec.clone();
    two.sources.truncate(2);
    let r = sc::check_node(&two, Path::new(&format!("{EX}/node"))).unwrap();
    assert_eq!(r.counts.get("pair-disagree"), Some(&60));
}

#[test]
fn vote_rules() {
    let s = |n: &str, o: f64, u: f64, d: bool| (n.to_string(), o, u, d, Some(true));
    let v = sc::vote(
        &[
            s("g", 0.0, 1e-8, true),
            s("a", 1e-6, 1e-8, false),
            s("b", 1e-6, 1e-8, false),
        ],
        3.0,
        0.0,
    );
    assert_eq!(v.status, "gnss-outlier");
    assert!((v.consensus_s.unwrap() - 1e-6).abs() < 1e-12);
    let v = sc::vote(
        &[
            s("g", 0.0, 1e-8, true),
            s("a", 0.0, 1e-8, false),
            s("b", 1e-6, 1e-8, false),
        ],
        3.0,
        0.0,
    );
    assert_eq!(
        (v.status.as_str(), v.outliers.as_slice()),
        ("source-outlier", ["b".to_string()].as_slice())
    );
    let v = sc::vote(
        &[
            s("g", 1e-6, 1e-8, true),
            s("a", 1e-6, 1e-8, false),
            s("b", 1e-6, 1e-8, false),
        ],
        3.0,
        0.0,
    );
    assert_eq!(v.status, "node-off");
    let v = sc::vote(
        &[
            s("g", 5e-7, 1e-8, true),
            s("a", 1e-6, 1e-8, false),
            s("b", 1e-6, 1e-8, false),
        ],
        3.0,
        0.0,
    );
    assert_eq!(v.status, "disciplining-outlier");
    let mut unlocked = s("g", 0.0, 1e-8, true);
    unlocked.4 = Some(false);
    let v = sc::vote(
        &[
            unlocked,
            s("a", 1e-6, 1e-8, false),
            s("b", 1e-6, 1e-8, false),
        ],
        3.0,
        0.0,
    );
    assert_eq!(v.status, "disciplining-outlier");
    let v = sc::vote(
        &[
            s("a", 0.0, 1e-8, false),
            s("b", 1e-6, 1e-8, false),
            s("c", 2e-6, 1e-8, false),
        ],
        3.0,
        0.0,
    );
    assert_eq!(v.status, "no-majority");
}

#[test]
fn cross_site_correlation_scopes_each_event() {
    let spec: SitesSpec =
        toml::from_str(&std::fs::read_to_string(format!("{EX}/sites/sites.toml")).unwrap())
            .unwrap();
    let r = sc::check_sites(&spec, Path::new(&format!("{EX}/sites"))).unwrap();
    let t0 = 1_795_046_400.0;
    let ev = |scope: &str| {
        r.events
            .iter()
            .filter(|e| e.scope == scope)
            .collect::<Vec<_>>()
    };
    let single = ev("single-site");
    assert!(
        single.iter().any(|e| e.sites == ["b-2"]
            && e.kind == "diverged"
            && e.start_unix == t0 + 7200.0
            && e.end_unix == t0 + 10800.0),
        "{:#?}",
        r.events
    );
    let op = ev("operator-wide");
    assert_eq!(op.len(), 1, "{:#?}", r.events);
    assert_eq!(op[0].sites, ["a-1", "a-2"]);
    assert_eq!(op[0].operators, ["OpA"]);
    assert_eq!(
        (op[0].start_unix, op[0].end_unix),
        (t0 + 5.0 * 3600.0, t0 + 7.0 * 3600.0)
    );
    let cm = ev("common-mode");
    assert_eq!(cm.len(), 1, "{:#?}", r.events);
    assert_eq!(
        (cm[0].start_unix, cm[0].end_unix),
        (t0 + 9.0 * 3600.0, t0 + 9.5 * 3600.0)
    );
    assert!((cm[0].max_abs_divergence_s - 13e-6).abs() < 0.5e-6);
    assert!(r
        .events
        .iter()
        .all(|e| e.kind != "diverged" || e.scope != "multi-operator"));
    // Too few sites.
    let mut few = spec.clone();
    few.sites.truncate(2);
    assert!(sc::check_sites(&few, Path::new(&format!("{EX}/sites"))).is_err());
}

#[test]
fn slow_divergence_is_caught_by_the_cusum_before_the_threshold() {
    let d = tmp("slow");
    let mut sites = String::from("bin_s = 60\nk = 4\n");
    for (i, id) in ["s1", "s2", "s3", "s4", "s5"].iter().enumerate() {
        let mut rows = String::new();
        for j in 0..600 {
            // A deterministic small noise; s5 drifts by 1 ns per minute.
            let n = (((j * 7 + i * 13) % 11) as f64 - 5.0) * 4e-9;
            let drift = if *id == "s5" { j as f64 * 1e-9 } else { 0.0 };
            rows.push_str(&format!("{},{:e}\n", 1_795_046_400 + j * 60, n + drift));
        }
        std::fs::write(d.join(format!("{id}.csv")), rows).unwrap();
        sites.push_str(&format!("[[sites]]\nid = \"{id}\"\noperator = \"Op{i}\"\nformat = \"csv\"\npath = \"{id}.csv\"\nu_s = 20e-9\n"));
    }
    let spec: SitesSpec = toml::from_str(&sites).unwrap();
    let r = sc::check_sites(&spec, &d).unwrap();
    let s5 = r.sites.iter().find(|s| s.id == "s5").unwrap();
    let alarm = s5.cusum_alarm_unix.expect("CUSUM alarm");
    assert!(s5.first_flag_unix.is_none_or(|f| alarm < f), "{s5:?}");
    assert!(r
        .events
        .iter()
        .any(|e| e.kind == "slow-divergence" && e.sites == ["s5"]));
    assert!(
        r.sites
            .iter()
            .filter(|s| s.id != "s5")
            .all(|s| s.cusum_alarm_unix.is_none()),
        "{:?}",
        r.sites
    );
}

// ---------------------------------------------------------------- holdover evidence

fn hspec(src: &str) -> HoldoverSpec {
    HoldoverSpec::parse(src).unwrap()
}

#[test]
fn holdover_evidence_matches_closed_forms() {
    // A frequency offset alone: |TE| = |y0| t, so the limit is reached at L / |y0|.
    let r = he::assess(
        &hspec("site = \"x\"\nperiod_h = 48\nlimit_s = 1e-6\ncoverage_k = 2\n[oscillator]\nadev = [[1, 1e-30]]\ninitial_freq = 1e-11\nu_initial_freq = 0.0\nu_ageing_per_day = 0.0\n"),
        "2026-11-19T12:00:00Z",
    )
    .unwrap();
    let t_star = r.max_supported_holdover_s.unwrap();
    assert!((t_star - 1e5).abs() < 1.0, "{t_star}");
    assert!(!r.supported && r.verdict.starts_with("not supported"));
    // Ageing alone: mean D t^2 / 2 at the period.
    let r = he::assess(
        &hspec("site = \"x\"\nperiod_h = 24\nlimit_s = 1e-3\n[oscillator]\nadev = [[1, 1e-30]]\nageing_per_day = 8.64e-9\nu_initial_freq = 0.0\nu_ageing_per_day = 0.0\n"),
        "u",
    )
    .unwrap();
    assert!((r.mean_at_period_s - 1e-13 * 86_400f64.powi(2) / 2.0).abs() < 1e-12);
    // White FM alone: u_c = sqrt(q_wf t) with q_wf = ADEV(1 s)^2 * 1 s.
    let r = he::assess(
        &hspec("site = \"x\"\nperiod_h = 1\nlimit_s = 1\n[oscillator]\nadev = [[1, 1e-11], [10, 3.1623e-12], [100, 1e-12], [1000, 3.1623e-13]]\nu_initial_freq = 0.0\nu_ageing_per_day = 0.0\n"),
        "u",
    )
    .unwrap();
    let want = (1e-22f64 * 3600.0).sqrt();
    assert!(
        (r.u_c_at_period_s - want).abs() / want < 0.02,
        "{} vs {want}",
        r.u_c_at_period_s
    );
    let shares: f64 = r.budget.iter().map(|c| c.share).sum();
    assert!((shares - 1.0).abs() < 1e-9);
    // An equipment class enters as a rectangular bound a / sqrt(3).
    let r = he::assess(
        &hspec("site = \"x\"\nperiod_h = 1\nlimit_s = 1\nequipment_class = \"prtc-a\"\n[oscillator]\nadev = [[1, 1e-30]]\nu_initial_freq = 0.0\nu_ageing_per_day = 0.0\n"),
        "u",
    )
    .unwrap();
    assert!((r.u_c_at_period_s - 100e-9 / 3f64.sqrt()).abs() < 1e-12);
    // TDEV converted under white FM gives the same q_wf as the matching ADEV.
    let a = he::assess(&hspec("site = \"x\"\nperiod_h = 1\nlimit_s = 1\n[oscillator]\nadev = [[10, 1e-11]]\nu_initial_freq = 0.0\n"), "u").unwrap();
    let tdev = 1e-11 * 10.0 / 6f64.sqrt();
    let b = he::assess(&hspec(&format!("site = \"x\"\nperiod_h = 1\nlimit_s = 1\n[oscillator]\ntdev = [[10, {tdev:e}]]\nu_initial_freq = 0.0\n")), "u").unwrap();
    assert!((a.u_c_at_period_s - b.u_c_at_period_s).abs() / a.u_c_at_period_s < 1e-6);
    // Missing inputs and unknown names are refused.
    assert!(he::assess(
        &hspec("site = \"x\"\n[oscillator]\nclass = \"ocxo\"\n"),
        "u"
    )
    .is_err());
    assert!(he::assess(
        &hspec("site = \"x\"\npreset = \"nope\"\n[oscillator]\nclass = \"ocxo\"\n"),
        "u"
    )
    .is_err());
    assert!(he::assess(
        &hspec("site = \"x\"\npreset = \"finland-reg54\"\n[oscillator]\nclass = \"quartz\"\n"),
        "u"
    )
    .is_err());
}

#[test]
fn finland_two_week_examples_rubidium_short_caesium_through() {
    let rb = he::assess(
        &hspec(&std::fs::read_to_string(format!("{EX}/holdover/rubidium-reg54.toml")).unwrap()),
        "u",
    )
    .unwrap();
    assert_eq!(rb.period_s, 336.0 * 3600.0);
    assert_eq!(rb.limit_s, 1.5e-6);
    assert!(
        rb.limit_basis.contains("Regulation 54")
            && rb.limit_basis.contains("not a figure of the Regulation")
    );
    assert!(!rb.supported, "{}", rb.verdict);
    let t = rb.max_supported_holdover_s.unwrap();
    assert!(t > 86_400.0 && t < 14.0 * 86_400.0, "{t}");
    let cs = he::assess(
        &hspec(&std::fs::read_to_string(format!("{EX}/holdover/cesium-reg54.toml")).unwrap()),
        "u",
    )
    .unwrap();
    assert!(cs.supported, "{}", cs.verdict);
    assert!(cs.defaults.is_empty(), "{:?}", cs.defaults);
    for r in [&rb, &cs] {
        let text = serde_json::to_string(r).unwrap().to_lowercase() + &he::html(r).to_lowercase();
        for w in kshana_pro::timing::audit::FORBIDDEN_WORDS {
            assert!(!text.contains(w), "{w}");
        }
    }
}

// ---------------------------------------------------------------- evidence packs

/// A TimeLab record of `n` samples at 1 s with amplitude `amp`.
fn record(d: &Path, name: &str, amp: f64, n: usize) {
    let s: String = (0..n)
        .map(|i| format!("{:e}\n", amp * ((i as f64) * 0.01).sin()))
        .collect();
    std::fs::write(d.join(name), s).unwrap();
}

fn build(fw: &str, d: &Path, src: &str) -> compliance::Pack {
    compliance::build(fw, &Inputs::parse(src).unwrap(), d, "2026-11-19T12:00:00Z").unwrap()
}

fn verdict(p: &compliance::Pack, id: &str) -> Verdict {
    p.requirements
        .iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .verdict
}

#[test]
fn rts25_pack_checks_the_cable_delay_and_the_holdover_documentation() {
    let d = tmp("rts25-cable");
    record(&d, "clock.txt", 20e-6, 600);
    let hold = he::assess(
        &hspec(&std::fs::read_to_string(format!("{EX}/holdover/cesium-reg54.toml")).unwrap()),
        "u",
    )
    .unwrap();
    std::fs::write(
        d.join("holdover-evidence.json"),
        serde_json::to_string(&hold).unwrap(),
    )
    .unwrap();
    let base = "[clock]\nreference = \"UTC(NPL)\"\n[clock.source]\nformat = \"timelab\"\npath = \"clock.txt\"\ntau0_s = 1.0\n";
    // 100 m of solid-PE cable (velocity factor 0.66): about 505 ns, not set.
    let unset = format!("{base}[cable_delay]\nlength_m = 100\nvelocity_factor = 0.66\n");
    let p = build("mifid2-rts25", &d, &unset);
    assert_eq!(verdict(&p, "RTS25-6"), Verdict::NotSupported);
    let c = p.evidence.cable.as_ref().unwrap();
    assert!((c.expected_s - 100.0 / (0.66 * 299_792_458.0)).abs() < 1e-15);
    assert!((c.expected_s - 505.4e-9).abs() < 0.1e-9);
    let r6 = p.requirements.iter().find(|r| r.id == "RTS25-6").unwrap();
    assert!(
        r6.reason.contains("no cable delay configured")
            && r6.limit_basis.contains("not set at all")
    );
    assert_eq!(verdict(&p, "RTS25-7"), Verdict::ToComplete);
    // Configured right, with a documented procedure and holdover evidence over two weeks.
    let ok = format!(
        "holdover_evidence = {{ path = \"holdover-evidence.json\" }}\n{base}[cable_delay]\nlength_m = 100\nvelocity_factor = 0.66\nconfigured_s = 505e-9\n[operator]\nholdover_procedure = \"TIME-OPS-07\"\nrequired_holdover_h = 336\n"
    );
    let p = build("mifid2-rts25", &d, &ok);
    assert_eq!(verdict(&p, "RTS25-6"), Verdict::Supported);
    assert_eq!(verdict(&p, "RTS25-7"), Verdict::Supported);
    // Configured 300 ns off.
    let p = build("mifid2-rts25", &d, &ok.replace("505e-9", "205e-9"));
    assert_eq!(verdict(&p, "RTS25-6"), Verdict::NotSupported);
    // A required holdover longer than the evidence covers.
    let p = build(
        "mifid2-rts25",
        &d,
        &ok.replace("required_holdover_h = 336", "required_holdover_h = 100000"),
    );
    assert_eq!(verdict(&p, "RTS25-7"), Verdict::NotSupported);
    // Holdover evidence without a procedure.
    let p = build(
        "mifid2-rts25",
        &d,
        &ok.replace("holdover_procedure = \"TIME-OPS-07\"\n", ""),
    );
    assert_eq!(verdict(&p, "RTS25-7"), Verdict::Insufficient);
}

#[test]
fn g8271_pack_applies_the_network_limits() {
    let d = tmp("g8271");
    record(&d, "point-c.txt", 400e-9, 3600);
    record(&d, "prtc.txt", 30e-9, 3600);
    let node: NodeSpec =
        toml::from_str(&std::fs::read_to_string(format!("{EX}/node/node.toml")).unwrap()).unwrap();
    let r = sc::check_node(&node, Path::new(&format!("{EX}/node"))).unwrap();
    std::fs::write(d.join("selfcheck.json"), serde_json::to_string(&r).unwrap()).unwrap();
    let rb = he::assess(
        &hspec(&std::fs::read_to_string(format!("{EX}/holdover/rubidium-reg54.toml")).unwrap()),
        "u",
    )
    .unwrap();
    std::fs::write(
        d.join("holdover-evidence.json"),
        serde_json::to_string(&rb).unwrap(),
    )
    .unwrap();
    let src = "organisation = \"SYNTHETIC operator\"\n\
selfcheck = { path = \"selfcheck.json\" }\nholdover_evidence = { path = \"holdover-evidence.json\" }\n\
[clock]\nreference = \"PRTC (reference point C)\"\n[clock.source]\nformat = \"timelab\"\npath = \"point-c.txt\"\ntau0_s = 1.0\n\
[prtc]\nreference = \"UTC(k)\"\n[prtc.source]\nformat = \"timelab\"\npath = \"prtc.txt\"\ntau0_s = 1.0\n\
[network]\nu_measurement_s = 50e-9\nmtie_mask = [[1, 300e-9], [100, 300e-9], [1000, 900e-9]]\n\
[cable_delay]\nlength_m = 30\nvelocity_factor = 0.85\nconfigured_s = 118e-9\n\
[operator]\nholdover_procedure = \"RAN-TIME-3\"\nrequired_holdover_h = 336\n";
    let p = build("itu-g8271-1", &d, src);
    assert!(p.evidence.problems.is_empty(), "{:?}", p.evidence.problems);
    assert_eq!(verdict(&p, "G8271-1"), Verdict::Supported);
    assert_eq!(verdict(&p, "G8271-2"), Verdict::Supported);
    // MTIE over 100 s of a 400 ns sine with a 628 s period exceeds 300 ns.
    assert_eq!(verdict(&p, "G8271-3"), Verdict::NotSupported);
    assert_eq!(verdict(&p, "G8271-4"), Verdict::Supported);
    // Rubidium falls short of two weeks; the self-check saw the GNSS-healthy wrong time.
    assert_eq!(verdict(&p, "G8271-5"), Verdict::NotSupported);
    assert_eq!(verdict(&p, "G8271-6"), Verdict::NotSupported);
    assert_eq!(verdict(&p, "G8271-7"), Verdict::Insufficient);
    let p = build(
        "itu-g8271-1",
        &d,
        &src.replace(
            "u_measurement_s = 50e-9",
            "max_te_s = 400e-9\nu_measurement_s = 50e-9",
        ),
    );
    assert_eq!(verdict(&p, "G8271-2"), Verdict::NotSupported);
    let p = build(
        "itu-g8271-1",
        &d,
        &src.replace("prtc_class", "x")
            .replace("[operator]\n", "[operator]\nprtc_class = \"prtc-b\"\n"),
    );
    assert_eq!(verdict(&p, "G8271-1"), Verdict::Supported);
    // Nothing supplied: verdicts say what is missing.
    let p = build("itu-g8271-1", &d, "");
    assert_eq!(verdict(&p, "G8271-2"), Verdict::Insufficient);
    assert_eq!(verdict(&p, "G8271-3"), Verdict::ToComplete);
    assert_eq!(verdict(&p, "G8271-4"), Verdict::ToComplete);
    assert!(
        compliance::mtie(&[0.0, 1.0, 0.0, 3.0], 1) == 3.0
            && compliance::mtie(&[0.0, 1.0, 0.0, 3.0], 2) == 3.0
    );
    assert_eq!(compliance::mtie(&[0.0, 1.0, 2.0, 3.0], 1), 1.0);
}

// ---------------------------------------------------------------- CLI

#[test]
fn cli_vectors_analyse_and_signed_holdover_evidence() {
    let d = tmp("cli");
    let home = d.join("home");
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        )
    };
    let vec = d.join("vec");
    let v = vec.to_str().unwrap();
    let (ok, out) = run(&[
        "timing",
        "timebomb",
        "vectors",
        "--out",
        v,
        "--only",
        "gps-wnro-2038,leap-insert",
    ]);
    assert!(!ok && out.contains("requires"), "needs a licence: {out}");
    let (ok, out) = run(&[
        "timing",
        "timebomb",
        "vectors",
        "--out",
        v,
        "--only",
        "gps-wnro-2038,leap-insert",
        "--utc",
        "2026-11-19T12:00:00Z",
        "--dev",
    ]);
    assert!(ok, "{out}");
    assert!(vec.join("manifest.json").exists() && vec.join("leap-insert/replay.ubx").exists());
    let cap = format!("{EX}/timebomb/captures/wnro-faulty.nmea");
    let wnro = vec.join("gps-wnro-2038");
    let (ok, out) = run(&[
        "timing",
        "timebomb",
        "analyse",
        wnro.to_str().unwrap(),
        &cap,
        "--format",
        "nmea",
        "--strict",
        "--dev",
    ]);
    assert!(!ok && out.contains("week-rollover"), "{out}");
    let (ok, out) = run(&["timing", "timebomb", "advisories"]);
    assert!(ok && out.contains("CVE-2025-47904"), "{out}");
    let (ok, out) = run(&[
        "timing",
        "selfcheck",
        "sites",
        &format!("{EX}/sites/sites.toml"),
        "--license",
        "x",
    ]);
    assert!(!ok, "{out}");
    let (ok, out) = run(&[
        "timing",
        "selfcheck",
        "node",
        &format!("{EX}/node/node.toml"),
        "--json",
        d.join("n.json").to_str().unwrap(),
        "--dev",
    ]);
    assert!(ok && out.contains("GNSS looked healthy"), "{out}");
    // Signed holdover evidence, verified offline; a changed file fails.
    let key = d.join("key.txt");
    let seed = [7u8; 32];
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    std::fs::write(&key, format!("private_key={}\n", hex(&seed))).unwrap();
    let outdir = d.join("hold");
    let (ok, out) = run(&[
        "timing",
        "holdover-evidence",
        &format!("{EX}/holdover/cesium-reg54.toml"),
        "--out",
        outdir.to_str().unwrap(),
        "--signer",
        "Ops",
        "--key-file",
        key.to_str().unwrap(),
        "--utc",
        "2026-11-19T12:00:00Z",
        "--dev",
    ]);
    assert!(ok && out.contains("supported by the model"), "{out}");
    for f in [
        "report.json",
        "report.html",
        "budget.csv",
        "curve.csv",
        "inputs/holdover.toml",
        "signature.json",
    ] {
        assert!(outdir.join(f).exists(), "{f}");
    }
    let pk = hex(&kshana_pro::license::public_key_for(&seed));
    let (ok, out) = run(&[
        "dossier",
        "verify",
        outdir.to_str().unwrap(),
        "--trust",
        &pk,
    ]);
    assert!(ok, "{out}");
    std::fs::write(outdir.join("budget.csv"), "changed\n").unwrap();
    let (ok, _) = run(&[
        "dossier",
        "verify",
        outdir.to_str().unwrap(),
        "--trust",
        &pk,
    ]);
    assert!(!ok);
    let (ok, out) = run(&["timing", "holdover-evidence", "presets"]);
    assert!(ok && out.contains("finland-reg54"), "{out}");
}

/// Time edge audit: a deletion vector carries its leap marker into the expected epochs and the
/// analyser, so a device that does not delete 23:59:59 is `leap-not-applied`, not
/// `leap-mistimed`.
#[test]
fn analyser_reads_a_leap_second_deletion() {
    let v = vector("leap-delete");
    let li = v.leap_index().expect("deletion has an event");
    let eps = v.epochs();
    assert_eq!(eps[li].utc.iso(), "2027-07-01T00:00:00Z");
    assert!(tb::expected_csv(&v).lines().any(|l| l.contains("leap")));
    // Not deleted: the device counts on through 23:59:59, one second behind from then on.
    let a = tb::analyse(
        &v,
        &records(&v, |k, _| {
            Some(Label::from_unix(eps[0].utc.seconds() + k as i64))
        }),
        "csv",
        0,
    );
    assert_eq!(a.faults.len(), 1, "{:?}", a.faults);
    assert_eq!(a.faults[0].kind, "leap-not-applied", "{:?}", a.faults);
    // A correct device: no fault.
    let a = tb::analyse(&v, &records(&v, |_, l| Some(l)), "csv", 0);
    assert!(a.faults.is_empty(), "{:?}", a.faults);
}
