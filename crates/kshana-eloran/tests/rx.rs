// PROPRIETARY — Kshana Pro.
//! Software receiver (S10) end-to-end tests on simulated scenes.
use kshana_eloran::chain::Chain;
use kshana_eloran::geo::Geo;
use kshana_eloran::rx::{self, Antenna, LoopMode, RxConfig, RxInput, RxOutput, StationErrors};
use kshana_eloran::scenario::Scenario;
use kshana_eloran::time::AbsTime;
use kshana_eloran::TruthRecord;

fn scenario(name: &str) -> String {
    std::fs::read_to_string(format!("{}/scenarios/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

struct Run {
    input: RxInput,
    chains: Vec<Chain>,
    truth: Vec<TruthRecord>,
    rx_pos: Geo,
}

fn simulate(src: &str) -> Run {
    let sc = Scenario::from_toml(src).unwrap();
    let (sink, truth, _stats) = kshana_eloran::run_to_memory(src).unwrap();
    let start = AbsTime::from_utc_iso(&sc.scene.start_utc).unwrap();
    let rx_pos = sc.receiver.state(0.0).pos;
    Run {
        input: RxInput::new(sink.channels, sc.scene.params.fs_hz, start),
        chains: sc.chains.iter().map(|c| c.chain.clone()).collect(),
        truth,
        rx_pos,
    }
}

fn receive(run: &Run, cfg: RxConfig) -> (rx::Receiver, RxOutput) {
    rx::process(&run.input, &run.chains, cfg).unwrap()
}

fn report(tag: &str, errs: &[StationErrors]) {
    for e in errs {
        eprintln!(
            "[{tag}] {}/{} ch={:?} n={} toa mean={:.3} std={:.3} rms={:.3} max={:.3} ns | ecd err mean={:.4} rms={:.4} us | cyc_err={} | snr meas={:.1} truth={:.1} dB | bearing err={:?}",
            e.chain, e.station, e.channel, e.n, e.toa_mean_ns, e.toa_std_ns, e.toa_rms_ns, e.toa_max_abs_ns,
            e.ecd_mean_err_us, e.ecd_rms_err_us, e.cycle_errors, e.snr_meas_db, e.snr_truth_db, e.bearing_err_deg
        );
    }
}

fn station_count(run: &Run) -> usize {
    run.chains.iter().map(|c| c.stations.len()).sum()
}

#[test]
fn high_snr_toa_ecd_cycle_position_and_time() {
    let run = simulate(&scenario("rx-high-snr.toml"));
    let (_rx, out) = receive(&run, RxConfig::default());
    assert!(out.acquisitions.iter().all(|a| a.found), "{:?}", out.acquisitions);
    let errs = rx::toa_errors(&out.measurements, &run.truth);
    report("high-snr", &errs);
    assert_eq!(errs.len(), station_count(&run));
    for e in &errs {
        assert!(e.n >= 2);
        assert_eq!(e.cycle_errors, 0);
        assert!(e.toa_rms_ns < 5.0, "{}/{} rms {}", e.chain, e.station, e.toa_rms_ns);
        assert!(e.ecd_rms_err_us < 0.01, "{}/{} ecd {}", e.chain, e.station, e.ecd_rms_err_us);
    }
    assert!(out.measurements.iter().all(|m| !m.cycle_slip));

    // position + clock, range domain, no corrections (simple model = PF only)
    let corr = rx::Corrections::default();
    let sol = rx::position_solution(&out.measurements, &run.chains, run.input.start, &corr, &Default::default()).unwrap();
    let perr = rx::position_error_m(sol.position, &run.truth).unwrap();
    eprintln!("[high-snr] position error {perr:.3} m, hdop {:.2}, bias {:?}, residuals {:?}", sol.hdop, sol.clock_bias_ns, sol.residuals_ns);
    assert!(sol.converged);
    assert!(perr < 3.0, "position error {perr} m");
    // the common bias is receiver − UTC(A) = −50 ns
    assert!((sol.clock_bias_ns[0].1 + 50.0).abs() < 5.0, "{:?}", sol.clock_bias_ns);

    // TDoA fix for comparison
    let td = rx::tdoa_fix(&out.measurements, &run.chains, run.input.start, &corr, None).unwrap();
    let tderr = rx::position_error_m(td.position, &run.truth).unwrap();
    eprintln!("[high-snr] TDoA error {tderr:.3} m, hdop {:.2}, tds {:?}", td.hdop, td.tds);
    assert!(tderr < 5.0);

    // time solution at the known position recovers UTC(A) − receiver = +50 ns
    let ts = rx::time_solution(&out.measurements, &run.chains, run.input.start, run.rx_pos, &corr);
    for s in &ts.stations {
        eprintln!("[high-snr] clock vs {}/{}: {:.3} ns", s.chain, s.station, s.ref_minus_rx_ns);
        assert!((s.ref_minus_rx_ns - 50.0).abs() < 3.0, "{s:?}");
    }
    assert!((ts.by_time_ref[0].1 - 50.0).abs() < 2.0);
    for c in rx::clock_errors(&ts, &run.truth) {
        assert!(c.error_ns.abs() < 3.0, "{c:?}");
    }
}

#[test]
fn noise_20db_cycle_identification_and_statistics() {
    let run = simulate(&scenario("rx-noise.toml"));
    let (_rx, out) = receive(&run, RxConfig::default());
    assert!(out.acquisitions.iter().all(|a| a.found), "{:?}", out.acquisitions);
    let errs = rx::toa_errors(&out.measurements, &run.truth);
    report("noise-20dB", &errs);
    assert_eq!(errs.len(), station_count(&run));
    for e in &errs {
        assert!(e.n >= 2);
        assert_eq!(e.cycle_errors, 0, "{e:?}");
        assert!(e.snr_truth_db < 22.0 && e.snr_truth_db > 19.0);
        // measured SNR agrees with truth within 1.5 dB
        assert!((e.snr_meas_db - e.snr_truth_db).abs() < 1.5, "{e:?}");
        assert!(e.toa_rms_ns < 30.0, "{e:?}");
        // envelope (ECD) noise is far larger than carrier noise; the 5 µs cycle margin
        // leaves > 5σ at this level
        assert!(e.ecd_rms_err_us < 1.5, "{e:?}");
    }
    let corr = rx::Corrections::default();
    let sol = rx::position_solution(&out.measurements, &run.chains, run.input.start, &corr, &Default::default()).unwrap();
    let perr = rx::position_error_m(sol.position, &run.truth).unwrap();
    let ts = rx::time_solution(&out.measurements, &run.chains, run.input.start, run.rx_pos, &corr);
    eprintln!("[noise-20dB] position error {perr:.2} m (hdop {:.2}); UTC(A)-rx {:.2} ns", sol.hdop, ts.by_time_ref[0].1);
    assert!(perr < 20.0);
    assert!((ts.by_time_ref[0].1 - 50.0).abs() < 15.0);
}

#[test]
fn two_chain_cri_tracks_all_stations() {
    let run = simulate(&scenario("rx-cri.toml"));
    for blank in [true, false] {
        let mut cfg = RxConfig::default();
        cfg.cri.blank = blank;
        let (_rx, out) = receive(&run, cfg);
        assert!(out.acquisitions.iter().all(|a| a.found), "{:?}", out.acquisitions);
        let errs = rx::toa_errors(&out.measurements, &run.truth);
        report(if blank { "cri-blank" } else { "cri-noblank" }, &errs);
        assert_eq!(errs.len(), station_count(&run));
        for e in &errs {
            assert!(e.n >= 2);
            // without blanking the stronger chain's overlapping pulses bias the estimates
            // (reported only; cycle errors can occur over such short averaging)
            if blank {
                assert_eq!(e.cycle_errors, 0, "{e:?}");
                assert!(e.toa_rms_ns < 10.0, "{e:?}");
            }
        }
        if blank {
            // multi-chain position with one bias per chain (UTC(A) differs from UTC(B))
            let corr = rx::Corrections::default();
            let pc = rx::PositionConfig { bias_per_chain: true, ..Default::default() };
            let sol = rx::position_solution(&out.measurements, &run.chains, run.input.start, &corr, &pc).unwrap();
            let perr = rx::position_error_m(sol.position, &run.truth).unwrap();
            eprintln!("[cri-blank] position error {perr:.2} m, biases {:?}", sol.clock_bias_ns);
            assert!(perr < 10.0);
            let ts = rx::time_solution(&out.measurements, &run.chains, run.input.start, run.rx_pos, &corr);
            for (id, v, _) in &ts.by_time_ref {
                let want = if id == "UTC(A)" { 20.0 } else { -30.0 };
                assert!((v - want).abs() < 5.0, "{id}: {v}");
            }
        }
    }
}

#[test]
fn crossed_loops_beam_steered_and_per_loop() {
    let run = simulate(&scenario("rx-loops.toml"));
    assert_eq!(run.input.channels.len(), 2);
    let cfg = RxConfig { antenna: Antenna::CrossedLoop { a: 0, b: 1, mode: LoopMode::BeamSteer }, ..Default::default() };
    let (_rx, out) = receive(&run, cfg);
    let errs = rx::toa_errors(&out.measurements, &run.truth);
    report("loops-steer", &errs);
    assert_eq!(errs.len(), station_count(&run));
    for e in &errs {
        assert_eq!(e.cycle_errors, 0, "{e:?}");
        assert!(e.toa_rms_ns < 5.0, "{e:?}");
        assert!(e.bearing_err_deg.unwrap() < 0.5, "{e:?}");
    }
    let corr = rx::Corrections::default();
    let sol = rx::position_solution(&out.measurements, &run.chains, run.input.start, &corr, &Default::default()).unwrap();
    assert!(rx::position_error_m(sol.position, &run.truth).unwrap() < 3.0);

    let cfg = RxConfig { antenna: Antenna::CrossedLoop { a: 0, b: 1, mode: LoopMode::PerLoop }, ..Default::default() };
    let (_rx, out) = receive(&run, cfg);
    let errs = rx::toa_errors(&out.measurements, &run.truth);
    report("loops-per-loop", &errs);
    assert_eq!(errs.len(), 2 * station_count(&run));
    for e in &errs {
        assert_eq!(e.cycle_errors, 0, "{e:?}");
        assert!(e.toa_rms_ns < 5.0, "{e:?}");
    }
}

#[test]
fn ldc_hook_reports_nominal_pulses_at_zero_offset() {
    let run = simulate(&scenario("rx-high-snr.toml"));
    let (rx, out) = receive(&run, RxConfig::default());
    for t in &out.tracks {
        let g = t.windows[0].group;
        let pt = rx.pulse_timing(t, g + 1, &[8000.0]).unwrap();
        let st = &run.chains[t.chain_index].stations[t.station_index];
        let nominal = if st.role == kshana_eloran::signal::Role::Master { 9 } else { 8 };
        assert_eq!(pt.pulses.len(), nominal + 1);
        for p in &pt.pulses[..nominal] {
            assert!(p.offset_ns.abs() < 0.5, "{p:?}");
            assert!(p.phase_rad.abs() < 1e-3, "{p:?}");
            assert!((p.amplitude_rel - 1.0).abs() < 0.01, "{p:?}");
        }
        // the extra slot at 8000 µs carries no pulse (free in a secondary group; between
        // the eighth and ninth pulses for the master)
        assert!(pt.pulses[nominal].amplitude_rel < 0.05, "{:?}", pt.pulses[nominal]);
    }
}
