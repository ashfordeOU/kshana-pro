// PROPRIETARY — Kshana Pro.
use super::*;
use crate::ldc::msg::{Message, MsgContext};
use crate::rx::{self, position_error_m, position_solution, PositionConfig};
use crate::scenario::Scenario;
use crate::signal::Role;

/// A scene with a real-ASF ground model (smooth earth over land), at a chosen receiver
/// position. The ASF is spatially smooth, so a reference and a nearby user share most of it.
fn scene(lat: f64, lon: f64, seed: u64) -> String {
    format!(
        r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 1.0
fs_hz = 400000
seed = {seed}
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = {lat}
lon_deg = {lon}
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 0.0
[propagation.ground]
model = "smooth_earth"
sigma_s_m = 0.003
[[interference]]
type = "wgn"
rms_uv_m_20khz = 80.0
[[chains]]
name = "dl-A"
gri_us = 80000
[[chains.stations]]
id = "M"
role = "master"
position = {{ lat_deg = 57.0, lon_deg = 0.0 }}
time_ref = "UTC(A)"
power_kw = 800.0
[[chains.stations]]
id = "X"
role = "secondary"
position = {{ lat_deg = 52.0, lon_deg = 1.0 }}
emission_delay_us = 13000.0
time_ref = "UTC(A)"
power_kw = 800.0
[[chains.stations]]
id = "Y"
role = "secondary"
position = {{ lat_deg = 55.0, lon_deg = 9.0 }}
emission_delay_us = 31000.0
time_ref = "UTC(A)"
power_kw = 800.0
[[chains.stations]]
id = "Z"
role = "secondary"
position = {{ lat_deg = 51.0, lon_deg = 7.0 }}
emission_delay_us = 49000.0
time_ref = "UTC(A)"
power_kw = 800.0
"#
    )
}

struct Site {
    meas: Vec<rx::Measurement>,
    chains: Vec<crate::chain::Chain>,
    start: crate::time::AbsTime,
    truth: Vec<crate::TruthRecord>,
}

fn run_site(src: &str) -> Site {
    let sc = Scenario::from_toml(src).unwrap();
    let (sink, truth, _st) = crate::run_to_memory(src).unwrap();
    let start = sc.start().unwrap();
    let input = rx::RxInput::new(sink.channels, sc.scene.params.fs_hz, start);
    let chains: Vec<_> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let (_r, out) = rx::process(&input, &chains, rx::RxConfig::default()).unwrap();
    Site {
        meas: out.measurements,
        chains,
        start,
        truth,
    }
}

#[test]
fn reference_corrections_reduce_user_position_error() {
    // Reference at a surveyed site; user ~25 km away sharing the smooth-earth ASF.
    let ref_lat = 54.2;
    let ref_lon = 4.0;
    let user_lat = 54.15; // ~6 km from the reference
    let user_lon = 4.04;

    let refsite = run_site(&scene(ref_lat, ref_lon, 11));
    let user = run_site(&scene(user_lat, user_lon, 12));

    // The reference knows its surveyed position exactly.
    let mut station = ReferenceStation::new(
        crate::geo::Geo::new(ref_lat, ref_lon),
        &refsite.chains,
        DloranConfig::default(),
    )
    .unwrap();
    let corrs = station.update(&refsite.meas, refsite.start);
    assert_eq!(corrs.len(), 4);
    assert!(corrs.iter().all(|c| c.usable), "{corrs:?}");
    // the reference's residuals are the real ASF — not negligible
    assert!(
        corrs.iter().any(|c| c.filtered_residual_ns.abs() > 30.0),
        "ASF too small to be a meaningful test: {:?}",
        corrs
            .iter()
            .map(|c| c.filtered_residual_ns)
            .collect::<Vec<_>>()
    );

    // Broadcast as LDC Dloran messages and ingest on the user side.
    let ctx = MsgContext {
        station: 0,
        role: Role::Master,
        first_group: 0,
        gri_us: Some(80000),
    };
    let messages: Vec<Message> = station
        .message_specs(1)
        .iter()
        .map(|s| s.render(&ctx))
        .collect();
    assert!(!messages.is_empty());
    let uc = UserCorrector::from_messages(&messages);
    assert_eq!(uc.reference_site, Some(1));

    let pc = PositionConfig::default();
    // user fix with no corrections
    let sol0 = position_solution(
        &user.meas,
        &user.chains,
        user.start,
        &Corrections::default(),
        &pc,
    )
    .unwrap();
    let e0 = position_error_m(sol0.position, &user.truth).unwrap();

    // user fix with the reference's corrections
    let corr = uc.corrections(&user.chains);
    let sol1 = position_solution(&user.meas, &user.chains, user.start, &corr, &pc).unwrap();
    let e1 = position_error_m(sol1.position, &user.truth).unwrap();

    eprintln!("[dloran] user error: uncorrected {e0:.1} m, corrected {e1:.1} m");
    assert!(e0 > 15.0, "uncorrected error {e0} m unexpectedly small");
    assert!(
        e1 < 0.55 * e0,
        "corrections did not substantially help: {e0:.1} -> {e1:.1} m"
    );

    // The direct Corrections path and the message round-trip agree.
    let direct = station.corrections();
    let sol2 = position_solution(&user.meas, &user.chains, user.start, &direct, &pc).unwrap();
    let e2 = position_error_m(sol2.position, &user.truth).unwrap();
    assert!((e2 - e1).abs() < 2.0, "direct {e2} vs message {e1}");

    // The measurement-domain applier gives the same fix as the prediction-domain table.
    let mut umeas = user.meas.clone();
    let n = uc.apply(&mut umeas);
    assert_eq!(n, umeas.len());
    let sol3 = position_solution(
        &umeas,
        &user.chains,
        user.start,
        &Corrections::default(),
        &pc,
    )
    .unwrap();
    let e3 = position_error_m(sol3.position, &user.truth).unwrap();
    assert!((e3 - e1).abs() < 1.0, "apply() {e3} vs corrections() {e1}");
}

#[test]
fn smoothing_filters_noise_and_flags_unusable() {
    let chains = run_site(&scene(54.0, 4.0, 1)).chains;
    let mut station = ReferenceStation::new(
        crate::geo::Geo::new(54.0, 4.0),
        &chains,
        DloranConfig {
            smoothing: Smoothing::Exponential { alpha: 0.3 },
            max_correction_ns: 100.0,
            max_residual_jump_ns: 1e9,
            reference_site: 7,
        },
    )
    .unwrap();
    // Feed synthetic measurements: a station with a steady 50 ns residual plus noise, and
    // one with a huge residual that must be flagged unusable.
    let start = crate::time::AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap();
    use crate::rx::Measurement;
    let gri = 0.08;
    let mk = |si: usize, group: i64, extra_s: f64| -> Measurement {
        let st = &chains[0].stations[si];
        let pred = rx::predicted_delay_s(
            st,
            crate::geo::Geo::new(54.0, 4.0),
            "dl-A",
            &Corrections::default(),
        );
        let nom = rx::nominal_emission_s(&chains[0], si, group, start);
        Measurement {
            chain_index: 0,
            station_index: si,
            chain: "dl-A".into(),
            station: st.id.clone(),
            channel: None,
            group,
            toa_s: nom + pred + extra_s,
            envelope_toa_s: nom + pred + extra_s,
            ecd_us: 0.0,
            snr_db: 30.0,
            signal_rms: 1.0,
            field_uv_m: 1.0,
            cycle_slip: false,
            bearing_deg: None,
            pulses: 8,
            samples: 100,
        }
    };
    let mut last = Vec::new();
    for g in 0..40i64 {
        let noise = if g % 2 == 0 { 8e-9 } else { -8e-9 };
        let meas = vec![
            mk(0, g, 50e-9 + noise), // M: 50 ns, within limit
            mk(1, g, 500e-9),        // X: 500 ns, over the 100 ns limit → unusable
        ];
        last = station.update(&meas, start);
        let _ = gri;
    }
    let m = last.iter().find(|c| c.station == "M").unwrap();
    let x = last.iter().find(|c| c.station == "X").unwrap();
    assert!(
        (m.filtered_residual_ns - 50.0).abs() < 5.0,
        "M filtered {}",
        m.filtered_residual_ns
    );
    assert!(m.usable);
    assert!(!x.usable && x.flag.contains("over limit"), "{x:?}");
    // the broadcast omits the unusable station
    let specs = station.message_specs(0);
    let codes: Vec<u8> = specs
        .iter()
        .flat_map(|s| match s {
            MessageSpec::Dloran { corrections, .. } => {
                corrections.iter().map(|c| c.station_code).collect()
            }
            _ => vec![],
        })
        .collect();
    assert!(codes.contains(&default_station_code(0)) && !codes.contains(&default_station_code(1)));
}
