// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! The low-cost GNSS path: the hybrid scene's GNSS half streamed chunk by chunk into a
//! complex sink (ci16 SigMF, paced or not) on the eLoran scenario's start epoch, with a
//! degradation timeline (C/N0 drops and outages) applied through the scene's channel hook.

use kshana::iq::scene::TruthRecord;
use kshana_eloran::hw::StartTime;
use kshana_eloran::Scenario;
use kshana_pro::eloran_gnss::{gps_week_tow, GnssOptions};
use kshana_pro::eloran_gnss_sdr::{
    build_scene, satellites_in_view, stream, DegradationEvent, DegradationTimeline, Effect,
    SdrOptions, SigmfComplexSink, StartPlan, GNSS_OUTPUT_POLICY,
};

const NAV: &str = include_str!("fixtures/eloran/brdc_gps_20240910.rnx");
const SCENARIO: &str = r#"
[scene]
start_utc = "2024-09-10T01:00:00Z"
duration_s = 0.4
fs_hz = 1000000
seed = 3
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

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-gnss-sdr-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn read_ci16(path: &std::path::Path) -> Vec<(f64, f64)> {
    let b = std::fs::read(path).unwrap();
    b.chunks_exact(4)
        .map(|c| {
            (
                i16::from_le_bytes([c[0], c[1]]) as f64,
                i16::from_le_bytes([c[2], c[3]]) as f64,
            )
        })
        .collect()
}

fn base_opts() -> SdrOptions {
    SdrOptions {
        gnss: GnssOptions {
            fs_hz: 1_000_000.0,
            galileo: false,
            ..Default::default()
        },
        start: StartTime::Now,
        truth_interval_s: 0.05,
        chunk_samples: 50_000,
        ..Default::default()
    }
}

#[test]
fn file_sink_with_degradation_timeline() {
    let sc = Scenario::from_toml(SCENARIO).unwrap();
    // the satellites in view, to aim the events
    let (_, _, plan) = build_scene(&sc, NAV, &base_opts()).unwrap();
    let view = satellites_in_view(&sc, NAV, &base_opts()).unwrap();
    assert!(view.len() >= 6, "{view:?}");
    let prn = |i: usize| view[i].0;
    let (a, b) = (prn(0), prn(1));
    let timeline = DegradationTimeline {
        events: vec![
            DegradationEvent {
                sats: Some(vec![a]),
                start_s: 0.1,
                end_s: 0.3,
                effect: Effect::Cn0Drop { db: 10.0 },
            },
            DegradationEvent {
                sats: Some(vec![b]),
                start_s: 0.15,
                end_s: 0.25,
                effect: Effect::Outage,
            },
            DegradationEvent {
                sats: None,
                start_s: 0.3,
                end_s: 0.4,
                effect: Effect::Ramp {
                    from_db: 0.0,
                    to_db: -6.0,
                },
            },
        ],
    };
    // the TOML form parses to the same timeline
    let toml = format!(
        "[[event]]\nsats = [{a}]\nstart_s = 0.1\nend_s = 0.3\nkind = \"cn0_drop\"\ndb = 10.0\n\
         [[event]]\nsats = [{b}]\nstart_s = 0.15\nend_s = 0.25\nkind = \"outage\"\n\
         [[event]]\nstart_s = 0.3\nend_s = 0.4\nkind = \"ramp\"\nfrom_db = 0.0\nto_db = -6.0\n"
    );
    assert_eq!(DegradationTimeline::from_toml(&toml).unwrap(), timeline);

    let opts = SdrOptions {
        timeline: Some(timeline),
        ..base_opts()
    };
    let d = tmp("timeline");
    let mut sink = SigmfComplexSink::new(&d.join("gnss"), false);
    let mut truth: Vec<TruthRecord> = Vec::new();
    let rep = stream(&sc, NAV, &opts, &mut sink, &mut truth).unwrap();
    assert_eq!(rep.samples, 400_000);
    assert_eq!(rep.sink.samples, 400_000);
    assert_eq!(rep.policy, GNSS_OUTPUT_POLICY);
    assert!(rep.satellites.len() >= 8);
    // same start epoch as the eLoran scene
    let (w, tow) =
        gps_week_tow(kshana_eloran::time::AbsTime::from_utc_iso(&sc.scene.start_utc).unwrap());
    assert_eq!((rep.plan.gps_week, rep.plan.start_tow_s), (w, tow));
    assert_eq!(rep.plan, plan);
    assert_eq!(rep.truth_records as usize, truth.len());
    assert_eq!(truth.len(), 8 * rep.satellites.len());

    let cn0 = |sat: u32, t: f64| {
        truth
            .iter()
            .find(|r| r.sat_id == sat && (r.t_s - t).abs() < 1e-9)
            .copied()
            .unwrap()
    };
    // C/N0 drop on `a`, nothing on another satellite
    let a0 = cn0(a, 0.05).cn0_dbhz;
    assert!((cn0(a, 0.15).cn0_dbhz - (a0 - 10.0)).abs() < 0.05);
    assert!((cn0(a, 0.25).cn0_dbhz - (a0 - 10.0)).abs() < 0.05);
    // outage on `b`
    assert!(!cn0(b, 0.2).visible && cn0(b, 0.2).cn0_dbhz == f64::NEG_INFINITY);
    assert!(cn0(b, 0.1).visible && cn0(b, 0.25).visible);
    let b0 = cn0(b, 0.1).cn0_dbhz;
    assert!((cn0(b, 0.25).cn0_dbhz - b0).abs() < 0.05);
    // the ramp on every satellite: −3 dB half way
    let c = prn(2);
    assert!((cn0(c, 0.35).cn0_dbhz - (cn0(c, 0.25).cn0_dbhz - 3.0)).abs() < 0.05);

    let data = read_ci16(&sink.data_path());
    assert_eq!(data.len(), 400_000);
    let p: f64 = data.iter().map(|s| s.0 * s.0 + s.1 * s.1).sum::<f64>() / data.len() as f64;
    // unit noise power at 2000 counts per unit, plus the satellites' signal power
    let rms = p.sqrt() / 2000.0;
    assert!((1.0..1.4).contains(&rms), "rms {rms}");
    let meta = std::fs::read_to_string(sink.meta_path()).unwrap();
    assert!(meta.contains("conducted use only"));
    assert!(meta.contains("\"kshana:start_utc\": \"2024-09-10T01:00:00Z\""));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn degradation_changes_the_generated_signal() {
    // one satellite, no noise, fixed 45 dB-Hz: −10 dB from 0.1 s, outage from 0.2 s
    let sc = Scenario::from_toml(SCENARIO).unwrap();
    let a = satellites_in_view(&sc, NAV, &base_opts()).unwrap()[0].0;
    let mut opts = base_opts();
    opts.noise = false;
    opts.only = Some(vec![a]);
    opts.gnss.cn0_dbhz = Some(45.0);
    opts.timeline = Some(DegradationTimeline {
        events: vec![
            DegradationEvent {
                sats: Some(vec![a]),
                start_s: 0.1,
                end_s: 0.2,
                effect: Effect::Cn0Drop { db: 10.0 },
            },
            DegradationEvent {
                sats: None,
                start_s: 0.2,
                end_s: 0.3,
                effect: Effect::Outage,
            },
        ],
    });
    let d = tmp("signal");
    let mut sink = SigmfComplexSink::new(&d.join("one"), false);
    let mut truth: Vec<TruthRecord> = Vec::new();
    let rep = stream(&sc, NAV, &opts, &mut sink, &mut truth).unwrap();
    assert_eq!(rep.satellites.len(), 1);
    let data = read_ci16(&sink.data_path());
    let power = |a: usize, b: usize| {
        data[a..b]
            .iter()
            .map(|s| s.0 * s.0 + s.1 * s.1)
            .sum::<f64>()
            / (b - a) as f64
    };
    let (p0, p1, p2, p3) = (
        power(5_000, 95_000),
        power(105_000, 195_000),
        power(205_000, 295_000),
        power(305_000, 395_000),
    );
    // amplitude √(C/N0 / fs) = 0.178 units = 356 counts: power ≈ 1.26e5
    assert!(
        (p0 / (2000.0f64.powi(2) * 10f64.powf(4.5) / 1e6) - 1.0).abs() < 0.02,
        "{p0}"
    );
    assert!((p1 / p0 - 0.1).abs() < 0.005, "{}", p1 / p0);
    assert_eq!(p2, 0.0);
    assert!((p3 / p0 - 1.0).abs() < 0.01);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn paced_sink_runs_in_real_time_and_shares_the_start() {
    let sc = Scenario::from_toml(SCENARIO).unwrap();
    let mut opts = base_opts();
    opts.gnss.fs_hz = 200_000.0;
    opts.gnss.duration_s = Some(0.3);
    opts.chunk_samples = 10_000;
    let at = StartPlan::shared_device_start(2_300_000_000, 0.5);
    assert_eq!(at, StartTime::AtDeviceTimeNs(3_000_000_000));
    opts.start = at;
    let d = tmp("paced");
    let mut sink = SigmfComplexSink::new(&d.join("paced"), true);
    let mut truth: Vec<TruthRecord> = Vec::new();
    let t0 = std::time::Instant::now();
    let rep = stream(&sc, NAV, &opts, &mut sink, &mut truth).unwrap();
    assert!(t0.elapsed().as_secs_f64() >= 0.29);
    assert_eq!(rep.samples, 60_000);
    assert_eq!(rep.sink.start_time_ns, Some(3_000_000_000));
    assert!(rep.realtime_factor <= 1.05, "{}", rep.realtime_factor);
    let meta = std::fs::read_to_string(sink.meta_path()).unwrap();
    assert!(meta.contains("\"kshana:paced\": true"));
    std::fs::remove_dir_all(&d).ok();
}

// ---------------------------------------------------------------------------------------------
// Combined eLoran + GNSS signal-file scenario with injected faults and a shared truth.

mod hybrid {
    use kshana_pro::hybrid_sim::{self, Request};

    fn setup(name: &str) -> (std::path::PathBuf, Request) {
        let d = super::tmp(&format!("hybrid-{name}"));
        let (s, q) = hybrid_sim::synth();
        std::fs::write(d.join("scenario.toml"), s).unwrap();
        std::fs::write(d.join("nav.rnx"), super::NAV).unwrap();
        (d, toml::from_str(&q).unwrap())
    }

    #[test]
    fn one_scenario_writes_aligned_files_faults_and_shared_truth() {
        let (d, q) = setup("run");
        let out = d.join("out");
        let t = hybrid_sim::run(&q, &d, &out).unwrap();
        // Both streams start at the same instant and cover the run.
        assert_eq!(t.eloran.first_sample_utc, t.gnss.first_sample_utc);
        assert_eq!(t.eloran.samples, 200_000);
        assert_eq!(t.gnss.samples, 500_000);
        assert_eq!(t.eloran.center_hz, 100_000.0);
        assert_eq!(t.faults.len(), 4);
        for f in [
            "eloran.sigmf-data",
            "gnss.sigmf-data",
            "eloran.truth.csv",
            "gnss.truth.csv",
            "truth.json",
            "truth-timeline.csv",
        ] {
            assert!(out.join(f).is_file(), "{f}");
        }
        assert_eq!(t.files.len(), 6);
        assert!(t.files_only.contains("no transmit path"));
        // The eLoran truth shows the ASF error on X in its window and the skywave throughout.
        let et = std::fs::read_to_string(out.join("eloran.truth.csv")).unwrap();
        let rows = kshana_eloran::scene::truth_from_csv(&et).unwrap();
        let x: Vec<_> = rows.iter().filter(|r| r.station == "X").collect();
        assert!(x.iter().any(|r| r.t_emit_s >= 0.2
            && r.t_emit_s < 0.4
            && (r.emission_error_ns - 500.0).abs() < 1e-6));
        assert!(x
            .iter()
            .any(|r| r.t_emit_s < 0.2 && r.emission_error_ns.abs() < 1e-6));
        assert!(rows.iter().all(|r| r.sky_delay_us.is_some()));
        // The shared timeline marks the GNSS outage and the interference drop.
        let tl = std::fs::read_to_string(out.join("truth-timeline.csv")).unwrap();
        assert!(tl.contains("0.150,skywave,outage,"), "{tl}");
        let drop = hybrid_sim::cn0_drop_db(30.0, 45.0, 1.023e6);
        assert!((drop - 10.0 * (1.0f64 + 10f64.powf(4.5) * 1e3 / 1.023e6).log10()).abs() < 1e-9);
        assert!(
            tl.contains(&format!(
                "0.350,asf-error:SYN/X;skywave,interference,-{drop:.2}"
            )),
            "{tl}"
        );
        assert!(tl.contains("0.050,skywave,nominal,0"), "{tl}");
        // The GNSS signal is gone in the outage window.
        let iq = super::read_ci16(&out.join("gnss.sigmf-data"));
        let p = |a: usize, b: usize| {
            iq[a..b].iter().map(|(i, q)| i * i + q * q).sum::<f64>() / (b - a) as f64
        };
        assert!(
            p(120_000, 180_000) < p(20_000, 80_000),
            "outage lowers the received power"
        );
    }

    #[test]
    fn faults_are_checked() {
        let (d, mut q) = setup("bad");
        q.faults.push(hybrid_sim::Fault::GnssOutage {
            from_s: 0.4,
            to_s: 0.9,
            sats: None,
        });
        assert!(hybrid_sim::run(&q, &d, &d.join("o"))
            .unwrap_err()
            .contains("inside the run"));
        let (d, mut q) = setup("bad2");
        q.faults[0] = hybrid_sim::Fault::AsfError {
            chain: "SYN".into(),
            station: "Y".into(),
            from_s: 0.1,
            to_s: 0.2,
            error_ns: 1.0,
        };
        assert!(hybrid_sim::run(&q, &d, &d.join("o"))
            .unwrap_err()
            .contains("no station"));
    }
}
