// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
use super::convert::*;
use super::csv::{from_csv, to_csv};
use super::*;
use crate::campaign::conformance::{rx_config, simulate, ConformanceFile, SimRun};
use crate::rx::{self, RxInput, RxOutput};
use std::sync::OnceLock;

const CASE: &str = include_str!("../../scenarios/conformance/timing-accuracy.toml");

struct Fixture {
    run: SimRun,
    out: RxOutput,
    ts: rx::TimeSolution,
}

/// One simulated stimulus and one software-receiver run, shared by the tests.
fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let cf = ConformanceFile::from_toml(CASE).unwrap();
        let text = cf.scenario_text(None, &Default::default()).unwrap();
        let run = simulate(&text).unwrap();
        let cfg = rx_config(&cf.case, &run.scenario).unwrap();
        let input = RxInput::new(
            run.channels.clone(),
            run.scenario.scene.params.fs_hz,
            run.start,
        );
        let (_, out) = rx::process(&input, &run.chains(), cfg).unwrap();
        let pos = run.scenario.receiver.state(0.0).pos;
        let ts = rx::time_solution(
            &out.measurements,
            &run.chains(),
            run.start,
            pos,
            &Default::default(),
        );
        Fixture { run, out, ts }
    })
}

fn rx_file() -> ElofFile {
    let f = fixture();
    from_rx_output(
        &f.run.scenario,
        "timing-accuracy",
        &f.out,
        Some(&f.ts),
        Some(&f.run.truth),
    )
    .unwrap()
}

#[test]
fn nominal_emission_matches_the_receiver_solver() {
    let f = fixture();
    let chains = f.run.chains();
    let st = stations_of(&chains, &[]);
    for (si, m) in st.iter().enumerate() {
        for g in [0i64, 12_345_678, 25_000_000] {
            assert_eq!(
                nominal_emission_s(m, g, f.run.start),
                rx::nominal_emission_s(&chains[0], si, g, f.run.start)
            );
        }
    }
}

#[test]
fn truth_round_trips_through_jsonl() {
    let f = fixture();
    let file = from_truth(&f.run.scenario, "timing-accuracy", &f.run.truth).unwrap();
    let rep = validate(&file);
    assert!(rep.ok(), "{:?}", rep.errors);
    assert_eq!(rep.counts["truth"], f.run.truth.len());
    assert!(file.header.truth_included);
    assert_eq!(file.header.scenarios[0].sha256, f.run.scenario.hash());
    let text = file.to_jsonl().unwrap();
    assert!(text
        .lines()
        .next()
        .unwrap()
        .contains("\"record\":\"header\""));
    let back = ElofFile::from_jsonl(&text).unwrap();
    assert_eq!(back, file, "JSON-lines round trip is exact");
    // back to the simulator's records: exact except toa_s (rebuilt, within 1 ps)
    let mut tr = to_truth(&back).unwrap();
    let mut orig = f.run.truth.clone();
    let key = |r: &crate::TruthRecord| (r.chain.clone(), r.station.clone(), r.group);
    tr.sort_by_key(key);
    orig.sort_by_key(key);
    assert_eq!(tr.len(), orig.len());
    for (a, b) in tr.iter().zip(&orig) {
        assert!((a.toa_s - b.toa_s).abs() < 1e-12, "{} {}", a.toa_s, b.toa_s);
        let mut a2 = a.clone();
        a2.toa_s = b.toa_s;
        assert_eq!(&a2, b);
    }
    // the observable is a pseudo-delay: propagation + the 50 ns time-reference offset
    // (the emission error carries the station's −50 ns time-reference offset)
    for t in back.truth() {
        assert!((t.toa_ns - t.delay_ns - t.emission_error_ns).abs() < 1e-3);
        assert!((t.emission_error_ns + 50.0).abs() < 1e-6);
    }
}

#[test]
fn receiver_output_round_trips_and_scores_identically() {
    let f = fixture();
    let file = rx_file();
    let rep = validate(&file);
    assert!(rep.ok(), "{:?}", rep.errors);
    assert_eq!(rep.counts["obs"], f.out.measurements.len());
    assert!(rep.counts["timing"] >= f.ts.stations.len());
    let back = ElofFile::from_jsonl(&file.to_jsonl().unwrap()).unwrap();
    assert_eq!(back, file);
    // ELOF observations → measurements: identical ToA error statistics against truth
    let chains = f.run.chains();
    let meas = to_measurements(&back, &chains).unwrap();
    assert_eq!(meas.len(), f.out.measurements.len());
    let e0 = rx::toa_errors(&f.out.measurements, &f.run.truth);
    let e1 = rx::toa_errors(&meas, &f.run.truth);
    assert_eq!(e0.len(), e1.len());
    for (a, b) in e0.iter().zip(&e1) {
        assert_eq!((a.n, a.cycle_errors), (b.n, b.cycle_errors));
        assert!((a.toa_rms_ns - b.toa_rms_ns).abs() < 1e-3);
        assert!((a.ecd_rms_err_us - b.ecd_rms_err_us).abs() < 1e-9);
        assert!((a.snr_meas_db - b.snr_meas_db).abs() < 1e-9);
    }
    // the receiver really is within tens of ns of truth on this stimulus
    assert!(e1.iter().all(|e| e.toa_rms_ns < 100.0), "{e1:?}");
    // time solution: the same clock errors
    let ts = to_time_solution(&back);
    let c0 = rx::clock_errors(&f.ts, &f.run.truth);
    let c1 = rx::clock_errors(&ts, &f.run.truth);
    assert_eq!(c0, c1);
    assert_eq!(ts.by_time_ref, f.ts.by_time_ref);
}

#[test]
fn csv_profile_round_trips_observations_exactly() {
    let file = rx_file();
    let (text, left_out) = to_csv(&file).unwrap();
    assert_eq!(
        left_out,
        file.records.iter().filter(|r| r.kind() != "obs").count()
    );
    assert!(super::csv::is_csv(&text));
    let back = from_csv(&text).unwrap();
    let obs0: Vec<&Obs> = file.obs().collect();
    let obs1: Vec<&Obs> = back.obs().collect();
    assert_eq!(obs0, obs1, "CSV round trip is exact");
    assert!(!back.header.truth_included);
    let mut h = file.header.clone();
    h.truth_included = false;
    assert_eq!(back.header, h);
    let (_, rep) = validate_text(&text);
    assert!(rep.ok(), "{:?}", rep.errors);
    // a short row is refused with its line number
    let bad = text.replacen(",A,", ",", 1);
    let e = from_csv(&bad).unwrap_err();
    assert!(e.contains("line 4"), "{e}");
}

#[test]
fn importer_records_round_trip() {
    let file = rx_file();
    let recs = to_obs_records(&file).unwrap();
    let (obs, skipped) = obs_from_obs_records(&file.header, &recs).unwrap();
    assert_eq!(skipped, 0);
    let orig: Vec<&Obs> = file.obs().collect();
    assert_eq!(obs.len(), orig.len());
    for (r, o) in obs.iter().zip(orig) {
        let Record::Obs(x) = r else { panic!() };
        assert_eq!(x.group, o.group, "group recovered from the time tag");
        assert!((x.toa_ns - o.toa_ns).abs() < 1e-3);
        assert!((x.t_s - o.t_s).abs() < 1e-6);
    }
}

fn mutate(f: &ElofFile, edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let text = f.to_jsonl().unwrap();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let mut all: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let mut v = serde_json::Value::Array(std::mem::take(&mut all));
    edit(&mut v);
    lines = v
        .as_array()
        .unwrap()
        .iter()
        .map(|x| serde_json::to_string(x).unwrap())
        .collect();
    lines.join("\n")
}

fn errors_of(text: &str) -> Vec<String> {
    let (_, r) = validate_text(text);
    r.errors.into_iter().map(|e| e.message).collect()
}

#[test]
fn validator_rejects_bad_files() {
    let file = rx_file();
    let has = |errs: &[String], s: &str| errs.iter().any(|e| e.contains(s));
    // newer major / minor version
    let e = errors_of(&mutate(&file, |v| v[0]["version"] = "1.0".into()));
    assert!(has(&e, "not supported"), "{e:?}");
    let e = errors_of(&mutate(&file, |v| v[0]["version"] = "0.2".into()));
    assert!(has(&e, "not supported"), "{e:?}");
    let e = errors_of(&mutate(&file, |v| v[0]["format"] = "RINEX".into()));
    assert!(has(&e, "not an ELOF file"), "{e:?}");
    // wrong units
    let e = errors_of(&mutate(&file, |v| v[0]["units"]["toa_ns"] = "us".into()));
    assert!(has(&e, "units: toa_ns"), "{e:?}");
    let e = errors_of(&mutate(&file, |v| {
        v[0]["units"].as_object_mut().unwrap().remove("ecd_us");
    }));
    assert!(has(&e, "units: ecd_us missing"), "{e:?}");
    // required header field missing
    let e = errors_of(&mutate(&file, |v| {
        v[0].as_object_mut().unwrap().remove("time_reference");
    }));
    assert!(has(&e, "time_reference"), "{e:?}");
    // header not first
    let e = errors_of(&mutate(&file, |v| {
        v.as_array_mut().unwrap().swap(0, 1);
    }));
    assert!(has(&e, "first record must be the header"), "{e:?}");
    // non-monotonic time
    let e = errors_of(&mutate(&file, |v| {
        let a = v.as_array_mut().unwrap();
        a.swap(2, 6);
    }));
    assert!(has(&e, "monotonic"), "{e:?}");
    // undeclared station
    let e = errors_of(&mutate(&file, |v| v[1]["station"] = "Q".into()));
    assert!(has(&e, "not declared"), "{e:?}");
    // ToA in microseconds instead of ns (value far beyond a GRI once misread)
    let e = errors_of(&mutate(&file, |v| {
        let x = v[1]["toa_ns"].as_f64().unwrap();
        v[1]["toa_ns"] = (x * 1e3).into();
    }));
    assert!(has(&e, "beyond ± one GRI"), "{e:?}");
    // phase code disagreeing with the group parity
    let e = errors_of(&mutate(&file, |v| {
        let a = v.as_array_mut().unwrap();
        let i = a.iter().position(|x| x["record"] == "obs").unwrap();
        let p = if a[i]["phase_code"] == "A" { "B" } else { "A" };
        a[i]["phase_code"] = p.into();
    }));
    assert!(has(&e, "phase_code"), "{e:?}");
    // truth records without the declaration
    let e = errors_of(&mutate(&file, |v| v[0]["truth_included"] = false.into()));
    assert!(has(&e, "truth_included"), "{e:?}");
    // scenario reference must be a SHA-256
    let e = errors_of(&mutate(&file, |v| {
        v[0]["scenarios"][0]["sha256"] = "abc".into()
    }));
    assert!(has(&e, "SHA-256"), "{e:?}");
    // a missing required record field
    let e = errors_of(&mutate(&file, |v| {
        let a = v.as_array_mut().unwrap();
        let i = a.iter().position(|x| x["record"] == "obs").unwrap();
        a[i].as_object_mut().unwrap().remove("toa_ns");
    }));
    assert!(has(&e, "toa_ns"), "{e:?}");
    // NaN is not representable: a null value is refused
    let e = errors_of(&mutate(&file, |v| {
        v[1]["t_s"] = serde_json::Value::Null;
    }));
    assert!(!e.is_empty());
    // and the untouched file is valid
    assert!(errors_of(&file.to_jsonl().unwrap()).is_empty());
    assert!(read_valid(&file.to_jsonl().unwrap()).is_ok());
    assert!(read_valid("").is_err());
}

#[test]
fn truth_carries_transmitted_ldc_symbols() {
    let src = include_str!("../../scenarios/conformance/ldc-eurofix-decode.toml");
    let cf = ConformanceFile::from_toml(src).unwrap();
    let mut text = cf.scenario_text(None, &Default::default()).unwrap();
    text = text.replace("duration_s = 6.0", "duration_s = 0.4");
    let sc = crate::Scenario::from_toml(&text).unwrap();
    let (_, truth, _) = crate::run_to_memory(&text).unwrap();
    let file = from_truth(&sc, "ldc", &truth).unwrap();
    let rep = validate(&file);
    assert!(rep.ok(), "{:?}", rep.errors);
    assert_eq!(
        file.header.stations[0].ldc_scheme.as_deref(),
        Some("eurofix")
    );
    let syms: Vec<u32> = file.truth().filter_map(|t| t.ldc_symbol).collect();
    assert_eq!(
        syms.len(),
        file.truth().count(),
        "every group carries a symbol"
    );
    assert!(syms.iter().any(|&s| s != syms[0]), "symbols vary");
}
