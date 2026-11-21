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

/// Frame audit: the bridge has one receiver clock offset for GPS and Galileo, so a Galileo
/// (GST) residual other than the GPS one is refused while Galileo is on, instead of being
/// silently replaced by the GPS residual.
#[test]
fn a_gst_residual_other_than_gpst_is_refused_with_galileo() {
    let src = SCENARIO.replace("Gpst = 4.0\n", "Gpst = 4.0\nGst = 9.0\n");
    let sc = Scenario::from_toml(&src).unwrap();
    let nav = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/eloran/brdc_gps_20240910.rnx"
    ))
    .unwrap();
    let dir = std::env::temp_dir().join(format!("kshana-eloran-gst-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = GnssOptions {
        fs_hz: 2_000_000.0,
        galileo: true,
        ..Default::default()
    };
    let e = generate(&sc, &nav, &opts, &dir.join("h")).unwrap_err();
    assert!(e.contains("Gst (9 ns) differs from Gpst (4 ns)"), "{e}");
    // Equal residuals, or Galileo off, are accepted.
    let same =
        Scenario::from_toml(&SCENARIO.replace("Gpst = 4.0\n", "Gpst = 4.0\nGst = 4.0\n")).unwrap();
    let r = generate(&same, &nav, &opts, &dir.join("s"));
    assert!(
        r.is_ok() || !r.as_ref().unwrap_err().contains("Gst"),
        "{r:?}"
    );
    let off = GnssOptions {
        galileo: false,
        ..opts
    };
    assert!(generate(&sc, &nav, &off, &dir.join("o")).is_ok());
    std::fs::remove_dir_all(&dir).ok();
}
