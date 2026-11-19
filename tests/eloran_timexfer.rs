// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! GNSS time transfer for traceability: receiver clock against GPS time at a known antenna
//! position, CGGTTS-like tracks read back by the open engine's CGGTTS reader, common view of
//! two sites, and the local reference against UTC(k).
//!
//! The kshana 0.41.0 package carries no RINEX observation file, so observations are
//! synthesised from the real IGS broadcast navigation fixture (2024-09-10) with the engine's
//! measurement models; the estimation path never sees the truth clocks.

use kshana::frames::{geodetic_to_ecef, Geodetic};
use kshana::pvt::klobuchar_from_nav_header;
use kshana::rinex::parse_nav;
use kshana_pro::eloran_timexfer::{
    clock_offsets_from_files, common_view, ref_vs_utck, synthesize_obs, tracks, write_cggtts,
    CggttsHeader, ClockEpoch, ObsSynth, TimexferConfig, UncertaintyBudget, UtckGpsOffset,
};

const NAV: &str = include_str!("fixtures/eloran/brdc_gps_20240910.rnx");
/// 2024-09-10 02:00:00 GPS time, seconds from 1980-01-06.
const T0: f64 = (2331.0 * 7.0 + 2.0) * 86_400.0 + 7200.0;

fn ecef(lat: f64, lon: f64, h: f64) -> [f64; 3] {
    geodetic_to_ecef(Geodetic {
        lat_rad: lat.to_radians(),
        lon_rad: lon.to_radians(),
        alt_m: h,
    })
}

fn site_a() -> [f64; 3] {
    ecef(52.296, 10.460, 130.0)
}
fn site_b() -> [f64; 3] {
    ecef(48.836, 2.335, 120.0)
}
fn clock_a(t: f64) -> f64 {
    120e-9 + 2e-12 * t
}
fn clock_b(_t: f64) -> f64 {
    -45e-9
}

fn synth(antenna: [f64; 3], dual: bool, seed: u64, clock: &dyn Fn(f64) -> f64) -> String {
    let ephs = parse_nav(NAV).unwrap();
    let cfg = ObsSynth {
        antenna_ecef: antenna,
        start: (2024, 9, 10, 2, 0, 0.0),
        epochs: 240,
        interval_s: 30.0,
        dual_freq: dual,
        noise_m: 0.1,
        seed,
        mask_deg: 5.0,
        iono: klobuchar_from_nav_header(NAV).unwrap(),
        iono_scale: 1.0,
    };
    synthesize_obs(&ephs, &cfg, clock)
}

fn check_epochs(eps: &[ClockEpoch], clock: &dyn Fn(f64) -> f64, tol_ns: f64) {
    assert!(eps.len() >= 235, "{} epochs", eps.len());
    let mut sum = 0.0;
    for e in eps {
        let t = e.t_gps_s + e.refsys_s - T0; // tag time since start
        let err = (e.refsys_s - clock(t)) * 1e9;
        assert!(err.abs() < tol_ns, "t={t}: {err} ns, n={}", e.n_used);
        assert!(e.n_used >= 6);
        sum += err;
    }
    assert!(
        (sum / eps.len() as f64).abs() < 0.5,
        "mean error {}",
        sum / eps.len() as f64
    );
}

#[test]
fn receiver_clock_against_gps_time_dual_and_single_frequency() {
    let obs = synth(site_a(), true, 1, &clock_a);
    assert!(obs.contains("C2W"));
    let eps = clock_offsets_from_files(&obs, NAV, site_a(), &TimexferConfig::default()).unwrap();
    check_epochs(&eps, &clock_a, 3.0);
    assert!(eps[0].sats.iter().all(|s| s.iono_free));
    // the epoch's GPS time is the tag minus the clock
    assert!((eps[0].t_gps_s - (T0 - clock_a(0.0))).abs() < 1e-12);

    let obs1 = synth(site_a(), false, 2, &clock_a);
    let cfg1 = TimexferConfig {
        dual_freq: false,
        ..Default::default()
    };
    let eps1 = clock_offsets_from_files(&obs1, NAV, site_a(), &cfg1).unwrap();
    check_epochs(&eps1, &clock_a, 2.0);
    assert!(eps1[0].sats.iter().all(|s| !s.iono_free && s.iono_m > 0.0));

    // a wrong antenna position (20 m east) biases the clock and inflates the scatter
    let off = ecef(52.296, 10.460 + 20.0 / 68_000.0, 130.0);
    let bad = clock_offsets_from_files(&obs, NAV, off, &TimexferConfig::default()).unwrap();
    let mean_sigma = |e: &[ClockEpoch]| e.iter().map(|x| x.sigma_s).sum::<f64>() / e.len() as f64;
    assert!(mean_sigma(&bad) > 3.0 * mean_sigma(&eps));
}

#[test]
fn tracks_common_view_and_utck() {
    let cfg = TimexferConfig::default();
    let ea =
        clock_offsets_from_files(&synth(site_a(), true, 3, &clock_a), NAV, site_a(), &cfg).unwrap();
    let eb =
        clock_offsets_from_files(&synth(site_b(), true, 4, &clock_b), NAV, site_b(), &cfg).unwrap();
    let ta = tracks(&ea, &cfg, 30.0);
    let tb = tracks(&eb, &cfg, 30.0);
    assert!(ta.len() >= 30, "{} tracks", ta.len());
    let utc_s = |t: &kshana_pro::eloran_timexfer::CggttsTrack| {
        (t.mjd - 44_244) as f64 * 86_400.0 + t.start_s as f64 + 390.0
    };
    for t in &ta {
        assert_eq!(t.frc, "L3P");
        assert!(t.n >= 13);
        // midpoint in GPS time since the first epoch
        let tt = utc_s(t) + cfg.leap_seconds - T0;
        assert!(
            (t.refsys_s - clock_a(tt)).abs() < 1.5e-9,
            "{} {}: {} vs {}",
            t.sat,
            t.start_s,
            t.refsys_s,
            clock_a(tt)
        );
        assert!((t.srsys - 2e-12).abs() < 6e-12, "{}", t.srsys);
    }
    // written as CGGTTS and read back by the open engine (checksums hold)
    let header = |lab: &str, x: [f64; 3]| CggttsHeader {
        lab: lab.into(),
        receiver: "synthetic geodetic receiver".into(),
        antenna_ecef: x,
        int_dly_ns: 0.0,
        cab_dly_ns: 0.0,
        ref_dly_ns: 0.0,
        reference: format!("REF({lab})"),
    };
    let fa = write_cggtts(&header("LABA", site_a()), &ta);
    let fb = write_cggtts(&header("LABB", site_b()), &tb);
    let ra = kshana::cggtts::read_tracks(&fa);
    let rb = kshana::cggtts::read_tracks(&fb);
    assert_eq!((ra.bad_checksum, ra.malformed), (0, 0));
    assert_eq!((rb.bad_checksum, rb.malformed), (0, 0));
    assert_eq!(ra.tracks.len(), ta.len());
    // common view (engine) on the read-back files: ref(A) − ref(B)
    let cv = kshana::cggtts::common_view(&ra.tracks, &rb.tracks);
    assert!(cv.len() >= 6, "{} common-view epochs", cv.len());
    for e in &cv {
        let tt = e.t + cfg.leap_seconds - T0;
        let want = clock_a(tt) - clock_b(tt);
        assert!(
            (e.value_s - want).abs() < 1.2e-9,
            "{}: {} vs {want}",
            e.t,
            e.value_s
        );
        assert!(e.n_sats >= 2);
    }
    // the in-memory path gives the same answer within the file's 0.1 ns resolution
    let cv2 = common_view(&ta, &tb);
    assert_eq!(cv.len(), cv2.len());
    for (x, y) in cv.iter().zip(&cv2) {
        assert!((x.value_s - y.value_s).abs() < 2e-10);
    }

    // against UTC(k): Circular T-style inputs (illustrative values)
    let mjd = 60_563.0;
    let offs = vec![
        UtckGpsOffset::from_circular_t(mjd, 3.2, -1.1, 1.5),
        UtckGpsOffset::from_circular_t(mjd + 5.0, 2.2, -0.9, 1.5),
    ];
    assert!((offs[0].utck_minus_gpst_ns + 4.3).abs() < 1e-12);
    let budget = UncertaintyBudget::default();
    let r = ref_vs_utck(&ta, &offs, &budget);
    assert!(!r.is_empty());
    for v in &r {
        let tt = (v.mjd - 44_244.0) * 86_400.0 + cfg.leap_seconds - T0;
        let (off, _) = kshana_pro::eloran_timexfer::interpolate_offset(&offs, v.mjd).unwrap();
        let want = clock_a(tt) * 1e9 - off;
        assert!((v.ref_minus_utck_ns - want).abs() < 1.5, "{v:?} vs {want}");
        assert!(v.u_ns >= budget.calibration_ns && v.u_ns < 8.0, "{v:?}");
    }
    // the broadcast UTC parameters give UTC(USNO) − GPS time
    let b = UtckGpsOffset::from_broadcast_utc(T0, 2.0e-9, 1e-14, T0 - 3600.0, 10.0);
    assert!((b.utck_minus_gpst_ns + 2.036).abs() < 1e-9);
}
