// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Coherent LF + L-band capture set (W1119 A15): the eLoran scene and the kshana-pro
//! software GNSS generator recorded on one picosecond timeline through the
//! `kshana_eloran::capset::BasebandGenerator` seam, then replayed with the cross-stream
//! PPS alignment check. Evidence: MODELLED (simulated streams).

use kshana_eloran::capset::{record_capture_set, replay_check, RecordPlan, StreamSetup};
use kshana_eloran::Scenario;
use kshana_pro::eloran_gnss::{GnssBaseband, GnssOptions};

const SCENARIO: &str = r#"
[scene]
start_utc = "2024-09-10T00:59:59.97Z"
duration_s = 0.06
fs_hz = 1000000
seed = 3
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 69.27
lon_deg = 15.98
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 70.0, lon_deg = 20.0 }
"#;

fn nav() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/eloran/brdc_gps_20240910.rnx"
    ))
    .unwrap()
}

fn plan(id: &str) -> RecordPlan {
    let lf = StreamSetup::new("GPSDO 10 MHz + 1PPS", "gpsdo-1", 210_000);
    let mut g = StreamSetup::new("GPSDO 10 MHz + 1PPS", "gpsdo-1", 45_500);
    g.start_offset_ps = 7_654_321;
    RecordPlan::new(id, lf, g)
}

#[test]
fn eloran_and_gnss_capture_set_round_trips_and_misalignment_is_caught() {
    let sc = Scenario::from_toml(SCENARIO).unwrap();
    let dir = std::env::temp_dir().join(format!("kshana-capset-pro-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let opts = GnssOptions {
        fs_hz: 2_000_000.0,
        galileo: false,
        ..Default::default()
    };
    let mut gen = GnssBaseband::new(&nav(), opts.clone()).unwrap();
    let set = record_capture_set(&sc, &mut gen, &plan("hyb"), &dir).unwrap();
    assert!(gen.satellites.len() >= 20, "{:?}", gen.satellites);
    assert_eq!(set.markers.len(), 1, "the PPS at 01:00:00");
    assert_eq!(set.streams[1].center_hz, 1_575_420_000.0);
    assert_eq!(set.rate_ratios[0].num, Some(1));
    assert_eq!(set.rate_ratios[0].den, Some(2));
    assert_eq!(set.streams[1].samples, 120_000);
    assert!(set.streams[1].source.contains("GPS L1 C/A"));
    let rep = replay_check(&dir.join("hyb.capset.json")).unwrap();
    assert!(rep.pass, "{:?}", rep.problems);
    let g = &rep.markers[0].hits[1];
    assert!(g.correlation > 0.9, "{g:?}");
    assert!(g.sigma_ps < 1500.0, "{g:?}");
    assert!(rep.max_abs_cross_ps < 5_000.0);

    // a GNSS recorder that stamps its start 25 ns early is caught
    let mut bad = plan("bad");
    bad.gnss.stamp_error_ps = -25_000;
    let mut gen = GnssBaseband::new(&nav(), opts).unwrap();
    record_capture_set(&sc, &mut gen, &bad, &dir).unwrap();
    let rep = replay_check(&dir.join("bad.capset.json")).unwrap();
    assert!(!rep.pass);
    let c = rep.markers[0].cross_ps.unwrap();
    assert!((c - 25_000.0).abs() < 4_000.0, "cross {c} ps");
    std::fs::remove_dir_all(&dir).ok();
}

/// Robustness audit: a SigMF recording with zero channels, an absurd channel count, a bad sample
/// rate or a non-finite rf32 sample is refused. Zero channels used to divide by zero, and a NaN
/// sample later panicked the impulse-mitigation sort.
#[test]
fn sigmf_headers_and_samples_are_checked() {
    let d = std::env::temp_dir().join(format!("kshana-sigmf-rob-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let write = |name: &str, nch: &str, fs: &str, samples: &[f32]| {
        let base = d.join(name);
        std::fs::write(
            base.with_extension("sigmf-meta"),
            format!(r#"{{"global":{{"core:datatype":"rf32_le","core:sample_rate":{fs},"core:num_channels":{nch}}},"captures":[],"annotations":[]}}"#),
        )
        .unwrap();
        let bytes: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(base.with_extension("sigmf-data"), bytes).unwrap();
        base
    };
    let read = |b: &std::path::Path| kshana_eloran::io::read_sigmf(b, None);
    let ok = write("ok", "1", "1000000", &[0.1, -0.2, 0.3]);
    assert_eq!(read(&ok).unwrap().channels[0].len(), 3);
    let e = read(&write("zero", "0", "1000000", &[0.1])).unwrap_err();
    assert!(e.contains("num_channels 0"), "{e}");
    assert!(read(&write("huge", "1000000000", "1000000", &[0.1])).is_err());
    assert!(read(&write("fs", "1", "0", &[0.1])).is_err());
    let e = read(&write("nan", "1", "1000000", &[0.1, f32::NAN, 0.3])).unwrap_err();
    assert!(
        e.contains("sample 1 of channel 0 is not a finite number"),
        "{e}"
    );
    // The impulse mitigation itself no longer panics on a NaN (total order).
    let mut x = vec![0.0, 1.0, f64::NAN, -1.0, 0.5];
    x.extend(std::iter::repeat_n(0.1, 200));
    let _ = kshana_eloran::rx::impulse::mitigate(
        &mut x,
        &kshana_eloran::rx::impulse::Mitigation::default(),
    );
    std::fs::remove_dir_all(&d).ok();
}
