// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Hybrid eLoran/GNSS positioning and integrity: GNSS jammed (the solution continues on
//! eLoran), GNSS spoofed in position and in time (the cross-system test flags it against
//! eLoran while GNSS RAIM stays blind), and the receiver's real eLoran observations fused
//! with GNSS.

use kshana::rinex::parse_nav;
use kshana_eloran::geo::{inverse, Geo};
use kshana_eloran::rx::{self, Corrections, RxConfig, RxInput};
use kshana_eloran::time::AbsTime;
use kshana_eloran::Scenario;
use kshana_pro::eloran_fusion::{
    broadcast_ranges, geo_to_ecef, model_eloran_ranges, EloranRange, FusionConfig, HybridFilter,
    Mode, C,
};

const NAV: &str = include_str!("fixtures/eloran/brdc_gps_20240910.rnx");
/// 2024-09-10 01:00 GPS time of week.
const TOW: f64 = 172_800.0 + 3600.0;

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        (-2.0 * self.u().ln()).sqrt() * (2.0 * std::f64::consts::PI * self.u()).cos()
    }
}

fn stations() -> Vec<(String, Geo)> {
    vec![
        ("A/M".into(), Geo::new(56.5, 1.0)),
        ("A/X".into(), Geo::new(53.5, 1.5)),
        ("A/Y".into(), Geo::new(55.5, 6.5)),
        ("A/Z".into(), Geo::new(53.8, 5.5)),
    ]
}

fn horiz_err(fix_lat: f64, fix_lon: f64, truth: Geo) -> f64 {
    inverse(Geo::new(fix_lat, fix_lon), truth).distance_m
}

#[test]
fn jamming_and_spoofing_scenario() {
    let ephs = parse_nav(NAV).unwrap();
    let truth = Geo {
        lat_deg: 55.0,
        lon_deg: 3.0,
        h_m: 30.0,
    };
    let rx = geo_to_ecef(truth);
    // a self-consistent spoofer places the receiver 600 m north
    let fake = geo_to_ecef(Geo {
        lat_deg: 55.0 + 600.0 / 111_250.0,
        ..truth
    });
    let offset_m = 50e-9 * C; // GPS time − eLoran time scale
    let b_g = |t: f64| 1e-6 * C + 0.3 * t;
    let (jam, pos_spoof, time_spoof) = ((100.0, 160.0), (200.0, 230.0), (250.0, 280.0));
    let mut rng = Rng(11);
    let mut f = HybridFilter::new(FusionConfig::default());
    let mut checked = [0usize; 4];
    for k in 0..=300 {
        let t = k as f64;
        let inside = |w: (f64, f64)| t >= w.0 && t < w.1;
        let mut gnss = if inside(jam) {
            vec![]
        } else if inside(pos_spoof) {
            broadcast_ranges(&ephs, TOW + t, fake, b_g(t), 10.0, 3.0)
        } else {
            broadcast_ranges(&ephs, TOW + t, rx, b_g(t), 10.0, 3.0)
        };
        for g in &mut gnss {
            g.range_m += 3.0 * rng.n();
            if inside(time_spoof) {
                g.range_m += 300.0; // a 1 µs timing spoof, consistent across satellites
            }
        }
        let mut lor = model_eloran_ranges(&stations(), truth, b_g(t) + offset_m, 10.0);
        for l in &mut lor {
            l.range_m += 10.0 * rng.n();
        }
        let fix = f.step(t, &gnss, &lor).unwrap();
        let herr = horiz_err(fix.lat_deg, fix.lon_deg, truth);
        let clk_err_ns = fix.gnss_clock_ns - b_g(t) / C * 1e9;
        if inside(jam) {
            checked[0] += 1;
            assert_eq!(fix.mode, Mode::EloranOnly, "{t}");
            assert!(herr < 30.0, "jammed {t}: {herr} m");
            assert!(
                clk_err_ns.abs() < 60.0,
                "jammed {t}: GNSS clock {clk_err_ns} ns"
            );
            assert!(!fix.integrity.flagged, "{t}: {:?}", fix.integrity.reasons);
        } else if inside(pos_spoof) {
            checked[1] += 1;
            let i = &fix.integrity;
            assert!(i.flagged && i.gnss_excluded, "{t}: {i:?}");
            let c = i.cross.unwrap();
            assert!(c.horizontal.flagged, "{t}: {c:?}");
            assert!(c.d_north_m > 400.0, "{t}: {c:?}");
            // the spoof is self-consistent: GNSS RAIM alone sees nothing
            assert!(!i.gnss_raim.unwrap().fault_detected);
            assert_eq!(fix.mode, Mode::EloranOnly);
            assert!(herr < 30.0, "spoofed {t}: {herr} m");
        } else if inside(time_spoof) {
            checked[2] += 1;
            let i = &fix.integrity;
            assert!(i.flagged && i.gnss_excluded, "{t}: {i:?}");
            let c = i.cross.unwrap();
            assert!(c.time.unwrap().flagged, "{t}: {c:?}");
            assert!((c.time_innovation_ns + 1000.0).abs() < 150.0, "{t}: {c:?}");
            assert!(!i.gnss_raim.unwrap().fault_detected);
            assert!(clk_err_ns.abs() < 60.0, "time spoof {t}: {clk_err_ns} ns");
        } else {
            checked[3] += 1;
            assert_eq!(fix.mode, Mode::Hybrid, "{t}");
            assert!(!fix.integrity.flagged, "{t}: {:?}", fix.integrity);
            assert!(fix.n_gnss >= 6);
            assert!(herr < 10.0, "{t}: {herr} m");
            if t > 50.0 {
                assert!(clk_err_ns.abs() < 20.0, "{t}: {clk_err_ns}");
                assert!(
                    (fix.offset_ns - 50.0).abs() < 15.0,
                    "{t}: {}",
                    fix.offset_ns
                );
                assert!((fix.eloran_clock_ns - fix.gnss_clock_ns - fix.offset_ns).abs() < 1e-6);
            }
        }
    }
    assert_eq!(checked, [60, 30, 30, 181]);
}

#[test]
fn receiver_observations_fuse_with_gnss() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/kshana-eloran/scenarios/rx-high-snr.toml"
    ))
    .unwrap();
    let sc = Scenario::from_toml(&src).unwrap();
    let (sink, _truth, _) = kshana_eloran::run_to_memory(&src).unwrap();
    let start = AbsTime::from_utc_iso(&sc.scene.start_utc).unwrap();
    let chains: Vec<_> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let input = RxInput::new(sink.channels, sc.scene.params.fs_hz, start);
    let (_rx, out) = rx::process(&input, &chains, RxConfig::default()).unwrap();
    let obs = rx::observations(&out.measurements, &chains, start);
    let corr = Corrections::default();
    let lor = EloranRange::from_observations(&obs, &chains, &corr, 5.0);
    assert_eq!(lor.len(), 4);
    let truth = sc.receiver.state(0.0).pos;
    let ephs = parse_nav(NAV).unwrap();
    // receiver clock = GPS time; the stations' UTC(A) runs 50 ns ahead, so
    // GPS time − eLoran time scale = −50 ns
    let gnss = broadcast_ranges(&ephs, TOW, geo_to_ecef(truth), 0.0, 10.0, 3.0);
    let mut f = HybridFilter::new(FusionConfig::default());
    let fix = f.step(0.0, &gnss, &lor).unwrap();
    assert_eq!(fix.mode, Mode::Hybrid);
    assert!(!fix.integrity.flagged, "{:?}", fix.integrity);
    assert!(horiz_err(fix.lat_deg, fix.lon_deg, truth) < 1.0);
    assert!((fix.offset_ns + 50.0).abs() < 5.0, "{}", fix.offset_ns);
    assert!(fix.gnss_clock_ns.abs() < 1.0);
    // eLoran alone (no GNSS): the receiver's own range-domain answer
    let mut f2 = HybridFilter::new(FusionConfig {
        height_aid: Some((0.0, 1.0)),
        ..FusionConfig::default()
    });
    let fix2 = f2.step(0.0, &[], &lor).unwrap();
    assert_eq!(fix2.mode, Mode::EloranOnly);
    assert!(horiz_err(fix2.lat_deg, fix2.lon_deg, truth) < 5.0);
    assert!(
        (fix2.eloran_clock_ns + 50.0).abs() < 5.0,
        "{}",
        fix2.eloran_clock_ns
    );
}
