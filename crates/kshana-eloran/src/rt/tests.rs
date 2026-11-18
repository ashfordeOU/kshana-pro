// PROPRIETARY — Kshana Pro.
use super::*;
use crate::rx::{self, position_error_m};
use crate::scene::MemSink;
use crate::scenario::Scenario;

fn scene_static() -> String {
    r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 6.0
fs_hz = 400000
seed = 3
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 40.0
[propagation.ground]
model = "simple"
[[interference]]
type = "wgn"
rms_uv_m_20khz = 300.0
[[chains]]
name = "rt-A"
gri_us = 80000
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
time_ref = "UTC(A)"
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
time_ref = "UTC(A)"
ecd_us = 1.5
[[chains.stations]]
id = "Y"
role = "secondary"
position = { lat_deg = 55.5, lon_deg = 6.5 }
emission_delay_us = 31000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Z"
role = "secondary"
position = { lat_deg = 53.8, lon_deg = 5.5 }
emission_delay_us = 49000.0
time_ref = "UTC(A)"
"#
    .into()
}

struct Sim {
    sink: MemSink,
    chains: Vec<crate::chain::Chain>,
    start: AbsTime,
    fs: f64,
    truth: Vec<crate::TruthRecord>,
}

fn simulate(src: &str) -> Sim {
    let sc = Scenario::from_toml(src).unwrap();
    let (sink, truth, _st) = crate::run_to_memory(src).unwrap();
    Sim {
        chains: sc.chains.iter().map(|c| c.chain.clone()).collect(),
        start: sc.start().unwrap(),
        fs: sc.scene.params.fs_hz,
        truth,
        sink,
    }
}

fn stream(sim: &Sim, cfg: RtConfig, block: usize) -> (Vec<RtMeasurement>, Vec<RtSolution>, StatusSnapshot) {
    let n_ch = sim.sink.channels.len();
    let mut rt = RtReceiver::new(sim.fs, sim.start, &sim.chains, n_ch, cfg).unwrap();
    let total = sim.sink.channels[0].len();
    let mut n0 = 0;
    while n0 < total {
        let n = block.min(total - n0);
        let blk: Vec<Vec<f64>> = sim
            .sink
            .channels
            .iter()
            .map(|c| c[n0..n0 + n].to_vec())
            .collect();
        rt.push_block(&blk).unwrap();
        n0 += n;
    }
    rt.flush().unwrap();
    let st = rt.status();
    (rt.drain_measurements(), rt.drain_solutions(), st)
}

#[test]
fn streaming_matches_batch_for_several_block_sizes() {
    let sim = simulate(&scene_static());
    // batch reference
    let input = RxInput::new(sim.sink.channels.clone(), sim.fs, sim.start);
    let (_b, bout) = rx::process(&input, &sim.chains, RxConfig::default()).unwrap();
    // Reference/window phasing differs between a sub-window and the whole recording, so the
    // per-window "reference group" differs; the alignment-independent quantity is the
    // per-station pseudo-delay (what the solutions use). Compare those.
    use std::collections::HashMap;
    let bobs: HashMap<(usize, usize), (f64, usize)> =
        rx::observations(&bout.measurements, &sim.chains, sim.start)
            .into_iter()
            .map(|o| ((o.chain_index, o.station_index), (o.pseudo_delay_s, o.n)))
            .collect();

    for block in [777usize, 4096, 40_000, 131_072] {
        let cfg = RtConfig {
            stride_s: 1.0,
            ..Default::default()
        };
        let (meas, sols, st) = stream(&sim, cfg, block);
        assert!(
            meas.len() as f64 > 0.9 * bout.measurements.len() as f64,
            "block {block}: streamed {} vs batch {}",
            meas.len(),
            bout.measurements.len()
        );
        let sobs = rx::observations(
            &meas.iter().map(|r| r.m.clone()).collect::<Vec<_>>(),
            &sim.chains,
            sim.start,
        );
        assert_eq!(sobs.len(), bobs.len(), "block {block}: station count differs");
        let mut worst = 0.0f64;
        for o in &sobs {
            let (bd, bn) = bobs[&(o.chain_index, o.station_index)];
            worst = worst.max((o.pseudo_delay_s - bd).abs());
            assert!(
                (o.n as i64 - bn as i64).abs() <= 2,
                "block {block}: {}/{} count {} vs batch {bn}",
                o.chain,
                o.station,
                o.n
            );
        }
        assert!(
            worst < 20e-9,
            "block {block}: worst streamed-vs-batch pseudo-delay diff {} ns",
            worst * 1e9
        );
        // bounded memory: never more than a window of samples buffered
        assert!(st.buffered_samples < (2.0 * sim.fs) as usize, "block {block}: buffer {}", st.buffered_samples);
        // solutions are produced and near truth
        assert!(!sols.is_empty(), "block {block}: no solutions");
        let good = sols.iter().filter_map(|s| match &s.fix {
            RtFix::Position(p) => position_error_m(p.position, &sim.truth).map(|e| (s.at_s, e)),
            _ => None,
        });
        let mut any = false;
        for (_at, e) in good {
            any = true;
            assert!(e < 15.0, "block {block}: position error {e} m");
        }
        assert!(any, "block {block}: no position fixes");
    }
}

#[test]
fn moving_receiver_is_tracked() {
    let src = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 6.0
fs_hz = 400000
seed = 5
output_scale_uv_m = 1000000.0
[receiver]
type = "waypoints"
points = [
  { t_s = 0.0, lat_deg = 55.0, lon_deg = 3.0 },
  { t_s = 6.0, lat_deg = 55.0006, lon_deg = 3.0009 },
]
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 0.0
[propagation.ground]
model = "simple"
[[interference]]
type = "wgn"
rms_uv_m_20khz = 150.0
[[chains]]
name = "rt-A"
gri_us = 80000
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
time_ref = "UTC(A)"
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Y"
role = "secondary"
position = { lat_deg = 55.5, lon_deg = 6.5 }
emission_delay_us = 31000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Z"
role = "secondary"
position = { lat_deg = 53.8, lon_deg = 5.5 }
emission_delay_us = 49000.0
time_ref = "UTC(A)"
"#;
    let sim = simulate(src);
    let cfg = RtConfig {
        stride_s: 0.8,
        solution_every_s: 1.0,
        solution_window_s: 0.4,
        ..Default::default()
    };
    let (meas, sols, _st) = stream(&sim, cfg, 20_000);
    assert!(meas.len() > 50);
    // the receiver moved ~2.6 km; each epoch solution should still be close to the
    // instantaneous truth position at that time.
    let mut checked = 0;
    for s in &sols {
        if let RtFix::Position(p) = &s.fix {
            // truth nearest this solution time
            let near: Vec<_> = sim
                .truth
                .iter()
                .filter(|t| (t.toa_s - s.at_s).abs() < 0.9)
                .collect();
            if near.is_empty() {
                continue;
            }
            let lat = near.iter().map(|t| t.rx_lat_deg).sum::<f64>() / near.len() as f64;
            let lon = near.iter().map(|t| t.rx_lon_deg).sum::<f64>() / near.len() as f64;
            let d = crate::geo::inverse(p.position, crate::geo::Geo::new(lat, lon)).distance_m;
            assert!(d < 40.0, "moving fix at {} s off by {d} m", s.at_s);
            checked += 1;
        }
    }
    assert!(checked >= 3, "only {checked} moving fixes checked");
}

#[test]
fn station_off_then_on_is_handled() {
    let src = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 6.0
fs_hz = 400000
seed = 9
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
[propagation.ground]
model = "simple"
[[interference]]
type = "wgn"
rms_uv_m_20khz = 150.0
[[chains]]
name = "rt-A"
gri_us = 80000
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
time_ref = "UTC(A)"
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Y"
role = "secondary"
position = { lat_deg = 55.5, lon_deg = 6.5 }
emission_delay_us = 31000.0
time_ref = "UTC(A)"
faults = [ { type = "off", from_s = 0.0, to_s = 3.0 } ]
[[chains.stations]]
id = "Z"
role = "secondary"
position = { lat_deg = 53.8, lon_deg = 5.5 }
emission_delay_us = 49000.0
time_ref = "UTC(A)"
"#;
    let sim = simulate(src);
    let cfg = RtConfig {
        stride_s: 0.8,
        ..Default::default()
    };
    let (meas, _sols, st) = stream(&sim, cfg, 16_000);
    // station Y (index 2) is silent for the first 3 s, then transmits.
    let y_times: Vec<f64> = meas
        .iter()
        .filter(|m| m.m.station == "Y")
        .map(|m| m.m.toa_s)
        .collect();
    assert!(!y_times.is_empty(), "Y never acquired after it turned on");
    let before = y_times.iter().filter(|&&t| t < 2.5).count();
    let after = y_times.iter().filter(|&&t| t > 3.5).count();
    assert_eq!(before, 0, "Y produced {before} measurements while off");
    assert!(after >= 4, "Y only produced {after} measurements after turning on");
    // the lock snapshot should list Y with measurements.
    assert!(st.locks.iter().any(|l| l.station == "Y" && l.measurements > 0));
}
