// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! The software GNSS bridge: a GPS L1 C/A recording on an eLoran scenario's trajectory and
//! time base, from a real IGS broadcast navigation file.

use kshana_eloran::Scenario;
use kshana_pro::eloran_gnss::{generate, gps_week_tow, GnssOptions};

const SCENARIO: &str = r#"
[scene]
start_utc = "2024-09-10T01:00:00Z"
duration_s = 0.05
fs_hz = 1000000
seed = 3
[receiver]
type = "static"
lat_deg = 69.27
lon_deg = 15.98
[[outputs]]
type = "e_field"
[gnss_time_residual_ns]
Gpst = 4.0
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 70.0, lon_deg = 20.0 }
"#;

#[test]
fn gnss_half_shares_trajectory_and_time_base() {
    let sc = Scenario::from_toml(SCENARIO).unwrap();
    sc.build().unwrap();
    let nav = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/eloran/brdc_gps_20240910.rnx"
    ))
    .unwrap();
    let dir = std::env::temp_dir().join(format!("kshana-eloran-gnss-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = dir.join("hybrid");
    let opts = GnssOptions {
        fs_hz: 2_000_000.0,
        galileo: false,
        ..Default::default()
    };
    let run = generate(&sc, &nav, &opts, &base).unwrap();
    let (w, tow) =
        gps_week_tow(kshana_eloran::time::AbsTime::from_utc_iso("2024-09-10T01:00:00Z").unwrap());
    assert_eq!((run.gps_week, run.start_tow_s), (w, tow));
    assert_eq!(w, 2331);
    assert!((tow - (172_800.0 + 3600.0 + 18.0)).abs() < 1e-9);
    assert!(run.satellites.len() >= 20, "{:?}", run.satellites);
    assert_eq!(run.samples, 100_000);
    assert!(run.truth_records > 0);
    assert!((run.receiver_clock_bias_ns + 4.0).abs() < 1e-9);
    let meta = std::fs::read_to_string(base.with_extension("gnss.sigmf-meta")).unwrap();
    assert!(meta.contains(&sc.hash()));
    assert_eq!(std::fs::metadata(&run.data).unwrap().len(), 100_000 * 4);
    std::fs::remove_dir_all(&dir).ok();
}

/// Time edge audit: ephemeris age is measured across the week boundary from the week numbers,
/// not by wrapping the time of week. A same-week toe of Sunday 00:00 is 6.9 days from a
/// Saturday 22:40 scene and is refused; next week's Sunday 00:00 is 80 minutes away and taken,
/// with or without a Galileo-style +1024 week offset.
#[test]
fn ephemeris_age_spans_the_week_boundary_by_week_number() {
    use kshana_pro::eloran_gnss::select_ephemerides;
    let nav = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/eloran/brdc_gps_20240910.rnx"
    ))
    .unwrap();
    let base = kshana::rinex::parse_nav(&nav)
        .unwrap()
        .into_iter()
        .find(|e| e.system == 'G' && e.sv_health == 0.0)
        .unwrap();
    let (week, tow) = (2331_i64, 600_000.0);
    let with = |w: i64, toe: f64| {
        let mut e = base;
        e.gps_week = w as _;
        e.toe = toe;
        vec![e]
    };
    // Same week, toe at the start of the week: 6.9 days old.
    assert!(select_ephemerides(&with(week, 0.0), 'G', week, tow).is_empty());
    // Next week's first toe: 4800 s ahead.
    assert_eq!(
        select_ephemerides(&with(week + 1, 0.0), 'G', week, tow).len(),
        1
    );
    assert_eq!(
        select_ephemerides(&with(week + 1 + 1024, 0.0), 'G', week, tow).len(),
        1
    );
    // Previous week's last toe, seen from early Sunday: within 2 h.
    assert_eq!(
        select_ephemerides(&with(week - 1, 601_200.0), 'G', week, 1_800.0).len(),
        1
    );
    // Same week, 1 h away: taken; 3 h away: refused.
    assert_eq!(
        select_ephemerides(&with(week, 596_400.0), 'G', week, tow).len(),
        1
    );
    assert!(select_ephemerides(&with(week, 589_200.0), 'G', week, tow).is_empty());
}
