// PROPRIETARY — Kshana Pro.
//! Application-domain scenario packs (`scenarios/packs/<domain>/*.toml`): every pack parses,
//! builds and runs for a short time (duration overridden to 0.3 s), produces truth for every
//! chain without clipping, documents itself, and copies real chain geometry from the GRI
//! database unchanged.
use kshana_eloran::gridb::GriDb;
use kshana_eloran::scenario::Scenario;
use kshana_eloran::scene::{NullSink, RunStats};
use kshana_eloran::TruthRecord;
use std::path::{Path, PathBuf};

/// Scene duration used by the tests (the files carry the long, documented durations).
const TEST_DURATION_S: f64 = 0.3;

/// Chain names used in the packs for real (historic) chains → GRI DB entry id.
const DB_CHAINS: &[(&str, &str)] = &[
    ("9960", "9960-northeast-us"),
    ("8970", "8970-great-lakes"),
    ("7980", "7980-southeast-us"),
    ("6731", "6731-lessay"),
];

fn packs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/packs")
}

fn pack_files() -> Vec<PathBuf> {
    let mut out = vec![];
    let mut dirs = vec![packs_dir()];
    while let Some(d) = dirs.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.extension().map(|x| x == "toml").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(packs_dir()).unwrap().display().to_string()
}

fn load(rel_path: &str) -> (String, Scenario) {
    let src = std::fs::read_to_string(packs_dir().join(rel_path)).unwrap();
    let sc = Scenario::from_toml(&src).unwrap_or_else(|e| panic!("{rel_path}: {e}"));
    (src, sc)
}

/// Build and run a scenario with its duration overridden; samples are counted, not kept.
fn run_short(name: &str, mut sc: Scenario) -> (Vec<TruthRecord>, RunStats) {
    sc.scene.params.duration_s = TEST_DURATION_S;
    let mut scene = sc.build().unwrap_or_else(|e| panic!("{name}: build: {e}"));
    let mut sink = NullSink::default();
    let mut truth = vec![];
    let stats = scene.run(&mut sink, &mut truth).unwrap_or_else(|e| panic!("{name}: run: {e}"));
    (truth, stats)
}

#[test]
fn every_pack_parses_builds_runs_and_is_documented() {
    let files = pack_files();
    let domains = ["timing", "maritime", "rail", "aviation"];
    for d in domains {
        assert!(files.iter().any(|f| rel(f).starts_with(d)), "no pack in {d}/");
    }
    assert!(files.len() >= 11, "{} pack files", files.len());
    for f in &files {
        let name = rel(f);
        let src = std::fs::read_to_string(f).unwrap();
        // header: purpose, coverage, metrics, provenance and duration notes
        for key in ["PURPOSE", "EXERCISES", "METRICS", "PROVENANCE", "DURATION"] {
            assert!(src.contains(&format!("# {key}")), "{name}: header lacks {key}");
        }
        let sc = Scenario::from_toml(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let t0 = std::time::Instant::now();
        let (truth, stats) = run_short(&name, sc.clone());
        let fs = sc.scene.params.fs_hz;
        assert_eq!(stats.samples_per_channel, (TEST_DURATION_S * fs).round() as u64, "{name}");
        assert_eq!(stats.channels, sc.outputs.len(), "{name}");
        assert!(!truth.is_empty() && stats.truth_records as usize == truth.len(), "{name}: no truth");
        assert_eq!(stats.clipped_samples, 0, "{name}: clipping (peak {})", stats.peak_abs);
        assert!(stats.peak_abs > 0.0 && stats.peak_abs < 1.0, "{name}: peak {}", stats.peak_abs);
        for c in &sc.chains {
            assert!(truth.iter().any(|r| r.chain == c.chain.name), "{name}: no truth for chain {}", c.chain.name);
        }
        for r in &truth {
            let parts = r.geom_ns + r.pf_ns + r.sf_ns + r.asf_ns;
            assert!((r.delay_ns - parts).abs() < 0.01, "{name}: delay parts {r:?}");
            assert!(r.field_uv_m > 0.0 && r.distance_km > 0.0, "{name}: {r:?}");
            assert!(r.snr_db.is_finite(), "{name}: SNR {r:?}");
        }
        eprintln!(
            "{name}: {} truth records, peak {:.3}, {:.2} s wall",
            truth.len(),
            stats.peak_abs,
            t0.elapsed().as_secs_f64()
        );
    }
}

#[test]
fn real_chain_geometry_is_copied_from_the_gri_db_unchanged() {
    let db = GriDb::embedded().unwrap();
    let mut checked = 0;
    for f in pack_files() {
        let name = rel(&f);
        let (src, sc) = load(&name);
        for c in &sc.chains {
            let Some((_, id)) = DB_CHAINS.iter().find(|(n, _)| *n == c.chain.name) else { continue };
            assert!(src.contains(id), "{name}: chain {} does not cite GRI DB entry {id}", c.chain.name);
            let dbc = db.get(id).unwrap_or_else(|| panic!("GRI DB has no {id}"));
            assert_eq!(dbc.gri_us, c.chain.gri_us, "{name}: GRI of {id}");
            for s in &c.chain.stations {
                let d = dbc
                    .stations
                    .iter()
                    .find(|d| d.name == s.name)
                    .unwrap_or_else(|| panic!("{name}: {id} has no station named {:?}", s.name));
                assert_eq!(d.id, s.id, "{name}: {id} {}", s.name);
                assert_eq!(d.role, s.role, "{name}: {id} {}", s.name);
                let p = d.position.unwrap();
                assert_eq!((p.lat_deg, p.lon_deg), (s.position.lat_deg, s.position.lon_deg), "{name}: {id} {}", s.name);
                assert_eq!(d.power_kw, Some(s.power_kw), "{name}: {id} {} power", s.name);
                assert_eq!(d.emission_delay_us.unwrap_or(0.0), s.emission_delay_us, "{name}: {id} {} ED", s.name);
                checked += 1;
            }
        }
    }
    assert!(checked >= 40, "{checked} stations checked");
}

#[test]
fn multi_time_reference_step_shows_in_one_chain_only() {
    let (_, sc) = load("timing/multi-time-reference.toml");
    let (truth, _) = run_short("multi-time-reference", sc);
    for r in &truth {
        let want = match (r.chain.as_str(), r.t_emit_s < 0.15) {
            ("9960", _) => 3.0,
            ("8970", true) => -8.0,
            ("8970", false) => 17.0,
            _ => unreachable!(),
        };
        // drift and white-FM noise move the offset by well under 0.5 ns within 0.3 s
        assert!((r.time_ref_offset_ns - want).abs() < 0.5, "{r:?}");
    }
    assert!(truth.iter().any(|r| r.chain == "8970" && r.t_emit_s >= 0.15));
}

#[test]
fn transmitter_faults_appear_in_truth() {
    let (_, sc) = load("timing/transmitter-faults.toml");
    let (truth, _) = run_short("transmitter-faults", sc);
    let of = |id: &'static str| truth.iter().filter(move |r| r.station == id);
    // W: +150 ns timing fault, minus the transmitter clock's +60 ns step from 0.12 s
    for r in of("W") {
        let want = match r.t_emit_s {
            t if t < 0.05 => -0.2,
            t if t < 0.12 => 149.8,
            t if t < 0.25 => 89.8,
            _ => -60.2,
        };
        assert!((r.emission_error_ns - want).abs() < 0.01, "{r:?}");
    }
    // X silent in [0.10, 0.20)
    assert!(of("X").all(|r| !(0.10..0.20).contains(&r.t_emit_s)));
    assert!(of("X").count() >= 1);
    // Y blinks (pulse 1 omitted) for the whole run: no truth records
    assert_eq!(of("Y").count(), 0);
    // M: transmitted ECD +1 µs from 0.2 s
    assert!(of("M").any(|r| r.t_emit_s >= 0.2));
    for r in of("M") {
        let m = truth.iter().find(|q| q.station == "M" && q.t_emit_s < 0.2).unwrap();
        if r.t_emit_s >= 0.2 {
            assert!((r.ecd_us - m.ecd_us - 1.0).abs() < 1e-6, "{r:?}");
        }
    }
}

#[test]
fn receiver_calibration_delay_is_the_primary_factor_of_the_known_distance() {
    let (_, sc) = load("timing/receiver-calibration.toml");
    let (truth, _) = run_short("receiver-calibration", sc);
    for r in &truth {
        assert!((r.distance_km - 10.0).abs() < 0.05, "{r:?}");
        let pf = r.distance_km * 1e3 * 1.000_338 / 299_792_458.0 * 1e9;
        assert!((r.delay_ns - pf).abs() < 0.01, "{r:?}");
        assert!(((r.toa_s - r.t_emit_s) * 1e9 - r.delay_ns).abs() < 0.01);
        assert!(r.snr_db > 85.0, "{r:?}");
    }
}
