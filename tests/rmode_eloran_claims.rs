// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! eLoran and R-Mode (backlog E37: B3701, B3702) on SYNTHETIC records built here: R-Mode
//! receiver logs with an injected fault, GNSS/R-Mode/eLoran fixes with a GNSS push, 1PPS
//! readings against a UTC(k) with a Circular T excerpt in the published layout (values
//! invented for the test), station-pair time comparisons, and a signed evidence index.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_eloran::rmode::perf::{self, Epoch, Limits, Outcome};
use kshana_pro::eloran_claims::{self, ClaimDecision};
use kshana_pro::multi_source::{self, wwrns};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e37-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Deterministic pseudo-noise in [-1, 1).
fn noise(i: u64) -> f64 {
    let x = i
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

fn run(home: &Path, args: &[&str]) -> (bool, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        o.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

fn no_claim_words(text: &str) {
    let l = text.to_lowercase();
    for w in ["compliant", "conformance-certified", "certified"] {
        assert!(!l.contains(w), "output uses {w:?}: {text}");
    }
}

// ------------------------------------------------------------------ B3701 R-Mode tests

/// One hour of 1 s receiver records: error a few metres, a 40 m fault from 1800 s alarmed
/// after `tta_s`, and a 30 s outage at 2400 s.
fn rmode_log(tta_s: f64) -> String {
    let mut recs = vec![];
    for i in 0..3600u64 {
        let t = i as f64;
        let mut err = 3.0 + 2.0 * noise(i);
        let mut alarm = false;
        if (1800.0..1830.0).contains(&t) {
            err = 40.0;
            alarm = t >= 1800.0 + tta_s;
        }
        let valid = !(2400.0..2430.0).contains(&t);
        recs.push(Epoch {
            t_s: t,
            valid,
            lat_deg: Some(55.0 + err / 111_195.0),
            lon_deg: Some(13.8),
            truth_lat_deg: 55.0,
            truth_lon_deg: 13.8,
            alarm,
            hpl_m: None,
            beacons_tracked: Some(4),
            ambiguities_ok: None,
        });
    }
    perf::records_csv(&recs)
}

const PROFILE: &str = r#"
profile = "test profile (illustrative numbers)"
source = "invented for this test; the NCSR 13 draft text is not read"
source_status = "placeholder"
accuracy_95_m = 10.0
availability_min = 0.95
continuity_min = 0.9
continuity_interval_s = 60.0
alert_limit_m = 25.0
time_to_alarm_s = 10.0
max_update_interval_s = 2.0
acquisition_max_s = 60.0
"#;

#[test]
fn rmode_perf_cli_reports_each_test_and_a_late_alarm() {
    let d = tmp("rmode");
    std::fs::write(d.join("limits.toml"), PROFILE).unwrap();
    std::fs::write(d.join("late.csv"), rmode_log(15.0)).unwrap();
    std::fs::write(d.join("prompt.csv"), rmode_log(2.0)).unwrap();
    let out = d.join("late.json");
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "rmode-perf",
            d.join("limits.toml").to_str().unwrap(),
            "--records",
            d.join("late.csv").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    no_claim_words(&text);
    let r: perf::PerfReport =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    let get = |id: &str| r.tests.iter().find(|t| t.id == id).unwrap().clone();
    assert_eq!(r.tests.len(), perf::LIBRARY.len());
    assert_eq!(get("RMP-01").outcome, Outcome::Pass, "{:?}", get("RMP-01"));
    assert_eq!(get("RMP-04").outcome, Outcome::Fail);
    assert_eq!(get("RMP-05").outcome, Outcome::Fail);
    assert_eq!(get("RMP-05").estimate, Some(15.0));
    assert_eq!(get("RMP-08").outcome, Outcome::NotExercised);
    assert_eq!(r.limits_source_status, "placeholder");
    assert!(r.statement.contains("not adopted"));

    // Prompt alarms: TTA passes; the alarmed fault is not misleading information.
    let out2 = d.join("prompt.json");
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "rmode-perf",
            d.join("limits.toml").to_str().unwrap(),
            "--records",
            d.join("prompt.csv").to_str().unwrap(),
            "--out",
            out2.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    let r: perf::PerfReport =
        serde_json::from_str(&std::fs::read_to_string(&out2).unwrap()).unwrap();
    let get = |id: &str| r.tests.iter().find(|t| t.id == id).unwrap().clone();
    assert_eq!(get("RMP-05").outcome, Outcome::Pass);
    assert_eq!(
        get("RMP-04").outcome,
        Outcome::Fail,
        "2 s of unalarmed error over the alert limit"
    );
    // One loss in an hour of valid output.
    assert!(get("RMP-03").estimate.unwrap() > 0.95);
}

#[test]
fn rmode_profile_without_limits_assesses_nothing() {
    let recs = perf::parse_records(&rmode_log(1.0)).unwrap();
    let r = perf::assess(&recs, &Limits::default()).unwrap();
    assert!(r
        .tests
        .iter()
        .all(|t| matches!(t.outcome, Outcome::NotAssessed | Outcome::NotExercised)));
    assert_eq!(r.limits_source_status, "placeholder");
}

// ------------------------------------------------------------------ B3701 WWRNS trust

#[test]
fn wwrns_trust_flags_a_gnss_push_against_two_terrestrial_systems() {
    let d = tmp("wwrns");
    std::fs::write(
        d.join("wwrns.toml"),
        r#"
required_accuracy_95_m = 100.0
requirement_source = "user requirement for the test (not the resolution text)"
[[source]]
id = "gnss"
group = "gnss"
[[source]]
id = "rmode"
group = "rmode"
[[source]]
id = "eloran"
group = "eloran"
"#,
    )
    .unwrap();
    let mut csv = String::from("t_s,source,status,lat_deg,lon_deg,sigma_m\n");
    for i in 0..120u64 {
        // GNSS pushed north by 0.01 deg (1.1 km) more every second from 60 s.
        let push = if i >= 60 { 0.01 * (i - 59) as f64 } else { 0.0 };
        let n = |k: u64| 1e-5 * noise(i * 7 + k);
        csv += &format!("{i},gnss,ok,{},{},3\n", 55.0 + push + n(1), 13.8 + n(2));
        csv += &format!("{i},rmode,ok,{},{},8\n", 55.0 + n(3), 13.8 + n(4));
        if i % 30 != 0 {
            csv += &format!("{i},eloran,ok,{},{},12\n", 55.0 + n(5), 13.8 + n(6));
        } else {
            csv += &format!("{i},eloran,unavailable,,,\n");
        }
    }
    std::fs::write(d.join("fixes.csv"), csv).unwrap();
    let out = d.join("wwrns.json");
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "wwrns-trust",
            d.join("wwrns.toml").to_str().unwrap(),
            "--fixes",
            d.join("fixes.csv").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    let r: wwrns::WwrnsReport =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(r.epochs, 120);
    assert_eq!(r.decisions[10].choice.as_deref(), Some("gnss"));
    assert_eq!(r.decisions[100].choice.as_deref(), Some("rmode"));
    // From 60 s the push (≥ 1.1 km) is out. At the eLoran gaps (60 s, 90 s) GNSS and R-Mode
    // disagree with no third group: neither qualifies, nothing is chosen and GNSS is not
    // flagged (one terrestrial system alone does not convict it).
    assert_eq!(r.gnss_suspect_epochs, 58);
    assert!(r.decisions[90].choice.is_none());
    assert_eq!(r.no_choice, 2);
}

#[test]
fn the_time_selector_takes_rmode_as_its_own_group() {
    let mut c = multi_source::Config::v4("ocxo", 1.0);
    c.sources.push(multi_source::SourceDef {
        id: "rmode".into(),
        group: "rmode".into(),
        sigma_ns: 100.0,
        authenticates_group: false,
    });
    c.validate().unwrap();
}

// ------------------------------------------------------------------ B3702 claims

/// Circular T section 1 in the published layout (values invented for the test).
const CIRCULAR: &str = "\
CIRCULAR T 999 (synthetic test excerpt)
1 - Coordinated Universal Time UTC and its local realizations UTC(k).
    From 2017 January 1, 0h UTC, TAI-UTC = 37 s.
Date 2026     0h UTC    OCT 26   OCT 31   NOV  5   NOV 10   NOV 15   Uncertainty/ns  Notes
              MJD        61339    61344    61349    61354    61359    uA    uB    u
Laboratory k                       [UTC-UTC(k)]/ns
NPL  (Teddington)          2.0      3.0      4.0      5.0      6.0    0.3   1.1   1.2
OP   (Paris)              -1.0     -2.0     -3.0     -4.0     -5.0    0.3   1.4   1.4
2 - Difference between the normalized frequencies
";

fn claim_fixture(d: &Path, mean_ns: f64, name: &str) -> PathBuf {
    let mut csv = String::from("# rx 1PPS minus UTC(NPL), ns\n");
    for i in 0..3600u64 {
        csv += &format!("{}\n", mean_ns + 3.0 * noise(i + 77));
    }
    std::fs::write(d.join(format!("{name}.csv")), csv).unwrap();
    std::fs::write(d.join("circt.txt"), CIRCULAR).unwrap();
    let req = format!(
        r#"
[claim]
statement = "eLoran timing receiver within 20 ns of UTC"
limit_ns = 20.0

[readings]
file = "{name}.csv"
unit = "ns"
reference = "UTC(NPL)"

[reference_offset]
circular_t = "circt.txt"
lab = "NPL"
mjd = 61346.5
u_interp_ns = 0.5

[[budget]]
name = "receiver delay calibration"
value_ns = 0.0
u_ns = 2.0

[[budget]]
name = "antenna cable (rectangular half-width)"
value_ns = -1.0
u_ns = 1.5
distribution = "rectangular"
"#
    );
    let p = d.join(format!("{name}.toml"));
    std::fs::write(&p, req).unwrap();
    p
}

#[test]
fn timing_claim_budget_and_guard_banded_decisions() {
    let d = tmp("claim");
    let base = d.clone();
    for (mean, want) in [
        (5.0, ClaimDecision::Supported),
        (24.0, ClaimDecision::Inconclusive),
        (40.0, ClaimDecision::NotSupported),
    ] {
        let p = claim_fixture(&d, mean, &format!("c{mean}"));
        let req: eloran_claims::ClaimRequest =
            toml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let r = eloran_claims::verify_claim(&req, &base).unwrap();
        // UTC(NPL) − UTC at 61346.5: −(3.0 + 0.5·1.0) = −3.5 ns.
        assert!((r.ref_offset_ns + 3.5).abs() < 1e-9, "{}", r.ref_offset_ns);
        assert!((r.offset_ns - (r.mean_rx_minus_ref_ns - 3.5 - 1.0)).abs() < 1e-9);
        // u_B = √(1.2² + 0.5² + 2² + (1.5/√3)²).
        let ub = (1.2f64.powi(2) + 0.25 + 4.0 + 1.5f64.powi(2) / 3.0).sqrt();
        assert!((r.u_b_ns - ub).abs() < 1e-9, "{} vs {ub}", r.u_b_ns);
        assert!((r.expanded_u_ns - 2.0 * r.u_c_ns).abs() < 1e-12);
        assert_eq!(
            r.mean_decision, want,
            "mean {mean}: {}",
            r.statement_of_result
        );
        no_claim_words(&r.statement_of_result);
    }
    // Outside the circular's span: refused, never extrapolated.
    let p = claim_fixture(&d, 5.0, "late");
    let mut req: eloran_claims::ClaimRequest =
        toml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    req.reference_offset.mjd = Some(61400.0);
    assert!(eloran_claims::verify_claim(&req, &base)
        .unwrap_err()
        .contains("no extrapolation"));
}

fn interop_toml(err_ns: f64) -> String {
    // Receiver clock − UTC = 100 ns. Station time = UTC(k) + steering; the receiver sees
    // ref_minus_rx = (UTC(k) − UTC) + steering − 100.
    // UTC(NPL) − UTC at 61349 = −4.0; UTC(OP) − UTC = +3.0.
    format!(
        r#"
circular_t = "circt.txt"
mjd = 61349
tolerance_ns = 50.0
[[station]]
id = "Anthorn"
time_ref = "NPL"
ref_minus_rx_ns = {a}
u_meas_ns = 3.0
u_prop_ns = 5.0
steering_ns = 2.0
u_steer_ns = 1.0
[[station]]
id = "Lessay"
time_ref = "OP"
ref_minus_rx_ns = {b}
u_meas_ns = 3.0
u_prop_ns = 5.0
[[station]]
id = "Sylt"
time_ref = "NPL"
ref_minus_rx_ns = {c}
u_meas_ns = 4.0
u_prop_ns = 6.0
"#,
        a = -4.0 + 2.0 - 100.0 + 1.0,
        b = 3.0 - 100.0 - 2.0 + err_ns,
        c = -4.0 - 100.0 + 0.5,
    )
}

#[test]
fn interoperability_e_n_and_shared_laboratory_terms() {
    let d = tmp("interop");
    std::fs::write(d.join("circt.txt"), CIRCULAR).unwrap();
    let good: eloran_claims::InteropRequest = toml::from_str(&interop_toml(0.0)).unwrap();
    let r = eloran_claims::interop(&good, &d).unwrap();
    assert_eq!(r.consistent_pairs, 3, "{:?}", r.pairs);
    assert!(
        (r.weighted_mean_ns - 100.0).abs() < 3.0,
        "{}",
        r.weighted_mean_ns
    );
    let same = r.pairs.iter().find(|p| p.same_time_ref).unwrap();
    let cross = r
        .pairs
        .iter()
        .find(|p| !p.same_time_ref && p.a == "Anthorn")
        .unwrap();
    // The NPL term cancels between two NPL-steered stations; a cross pair carries both
    // laboratory terms.
    let own = |m: f64, p: f64, s: f64| m * m + p * p + s * s;
    assert!(
        (same.expanded_u_ns - 2.0 * (own(3.0, 5.0, 1.0) + own(4.0, 6.0, 0.0)).sqrt()).abs() < 1e-9
    );
    assert!(
        (cross.expanded_u_ns
            - 2.0
                * (own(3.0, 5.0, 1.0) + own(3.0, 5.0, 0.0) + 1.2f64.powi(2) + 1.4f64.powi(2))
                    .sqrt())
        .abs()
            < 1e-9
    );
    let bad: eloran_claims::InteropRequest = toml::from_str(&interop_toml(60.0)).unwrap();
    let r = eloran_claims::interop(&bad, &d).unwrap();
    assert_eq!(r.consistent_pairs, 1);
    assert!(r.birge_ratio.unwrap() > 2.0);
    assert!(r
        .pairs
        .iter()
        .filter(|p| p.a == "Lessay" || p.b == "Lessay")
        .all(|p| !p.consistent && p.tolerance.as_deref() != Some("within")));
}

#[test]
fn signed_safety_of_life_index_lists_evidence_and_gaps() {
    let d = tmp("sol");
    let p = claim_fixture(&d, 5.0, "claim");
    let out = d.join("claim.json");
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "claim",
            p.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    std::fs::write(d.join("interop.toml"), interop_toml(0.0)).unwrap();
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "interop",
            d.join("interop.toml").to_str().unwrap(),
            "--out",
            d.join("interop.json").to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    std::fs::write(
        d.join("sol.toml"),
        r#"
title = "Timing receiver evidence (synthetic)"
subject = "Receiver model X, firmware 1.0 (synthetic)"
[[artifact]]
path = "claim.json"
category = "timing_traceability"
[[artifact]]
path = "interop.json"
category = "interoperability"
[[artifact]]
path = "claim.csv"
category = "reproducibility"
note = "raw readings"
"#,
    )
    .unwrap();
    let seed = [21u8; 32];
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    std::fs::write(d.join("key.txt"), format!("private_key={}\n", hex(&seed))).unwrap();
    let pk = hex(&kshana_pro::license::public_key_for(&seed));
    let pack = d.join("pack");
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "sol-evidence",
            d.join("sol.toml").to_str().unwrap(),
            "--out",
            pack.to_str().unwrap(),
            "--signer",
            "Test Lab",
            "--key-file",
            d.join("key.txt").to_str().unwrap(),
            "--utc",
            "2026-11-20T12:00:00Z",
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    no_claim_words(&std::fs::read_to_string(pack.join("README.md")).unwrap());
    let ix: eloran_claims::SolIndex =
        serde_json::from_str(&std::fs::read_to_string(pack.join("index.json")).unwrap()).unwrap();
    assert_eq!(ix.covered, 3);
    assert!(ix.gaps.contains(&"integrity_fde".to_string()));
    let tt = ix
        .categories
        .iter()
        .find(|c| c.id == "timing_traceability")
        .unwrap();
    assert!(tt.entries[0]
        .headline
        .as_deref()
        .unwrap()
        .contains("supported by this evidence"));
    let io = ix
        .categories
        .iter()
        .find(|c| c.id == "interoperability")
        .unwrap();
    assert!(io.entries[0]
        .headline
        .as_deref()
        .unwrap()
        .contains("3 of 3"));
    let (ok, text) = run(
        &d,
        &["dossier", "verify", pack.to_str().unwrap(), "--trust", &pk],
    );
    assert!(ok, "{text}");
    // Tampering breaks the signature.
    std::fs::write(pack.join("artifacts/claim.json"), "{}").unwrap();
    let (ok, _) = run(
        &d,
        &["dossier", "verify", pack.to_str().unwrap(), "--trust", &pk],
    );
    assert!(!ok);
}

#[test]
fn e37_commands_need_a_licence() {
    let d = tmp("lic");
    std::fs::write(d.join("limits.toml"), PROFILE).unwrap();
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "rmode-perf",
            d.join("limits.toml").to_str().unwrap(),
            "--simulate",
            "x.toml",
            "--out",
            d.join("o.json").to_str().unwrap(),
        ],
    );
    assert!(!ok);
    assert!(
        text.contains("pro-rmode-perf") || text.to_lowercase().contains("tier"),
        "{text}"
    );
    let (ok, text) = run(&d, &["eloran", "ldc-fde", "x.toml", "--out", "o.json"]);
    assert!(!ok, "{text}");
}

/// The LDC/ASF FDE through the command line (patent-candidate builds only): one corrupted
/// differential correction among six stations is excluded and the station kept.
#[cfg(feature = "unfiled-methods")]
#[test]
fn ldc_fde_cli_separates_a_correction_fault() {
    use kshana_eloran::geo::{inverse, Geo};
    use kshana_eloran::guard::asf_fde::{FdeEpoch, FdeReport, StationObs};
    let d = tmp("fde");
    let truth = Geo::new(55.0, 10.0);
    let sites = [
        (57.0, 9.0),
        (56.5, 13.0),
        (54.0, 13.5),
        (53.0, 9.5),
        (54.5, 6.5),
        (56.8, 6.8),
    ];
    let epochs: Vec<FdeEpoch> = (0..3u64)
        .map(|e| FdeEpoch {
            t_s: e as f64,
            approx_lat_deg: 55.05,
            approx_lon_deg: 10.05,
            truth_lat_deg: Some(55.0),
            truth_lon_deg: Some(10.0),
            stations: sites
                .iter()
                .enumerate()
                .map(|(i, &(la, lo))| {
                    let dist = inverse(truth, Geo::new(la, lo)).distance_m * 1.000_338;
                    let temporal = 30.0 * noise(e * 13 + i as u64);
                    let bad = if e == 1 && i == 3 { 100.0 } else { 0.0 };
                    StationObs {
                        id: format!("S{i}"),
                        lat_deg: la,
                        lon_deg: lo,
                        pseudo_delay_ns: dist / 0.299_792_458
                            + 250.0
                            + temporal
                            + 4_000.0
                            + 4.0 * noise(e * 101 + i as u64),
                        sigma_ns: 10.0,
                        asf_map_ns: 250.0,
                        asf_temporal_sigma_ns: 30.0,
                        ldc_correction_ns: Some(temporal + bad),
                        ldc_age_s: Some(5.0),
                        ldc_sigma_ns: 5.0,
                        ldc_unhealthy: false,
                    }
                })
                .collect(),
        })
        .collect();
    let req = serde_json::json!({
        "config": { "p_fa": 1e-4, "p_md": 1e-3, "hal_m": 500.0 },
        "epoch": epochs,
    });
    std::fs::write(d.join("fde.json"), req.to_string()).unwrap();
    let out = d.join("fde-report.json");
    let (ok, text) = run(
        &d,
        &[
            "eloran",
            "ldc-fde",
            d.join("fde.json").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    let r: FdeReport = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(r.nominal, 2, "{:?}", r.epochs);
    assert_eq!(r.corrections_excluded, 1);
    assert_eq!(r.epochs[1].excluded.as_deref(), Some("S3"));
    assert_eq!(r.misleading, 0);
}
