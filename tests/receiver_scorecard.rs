// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Receiver resilience scorecards (roadmap 0.42, E3). The development inputs here are the
//! open engine's training scenarios (tests/fixtures/forensics/training), not the published
//! sample campaign's cases.

use std::path::{Path, PathBuf};

use kshana_pro::scorecards::levels::Verdict;
use kshana_pro::scorecards::{self, Campaign};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e3-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Generate a training scenario's NMEA and its truth as an I4 motion CSV into `dir`.
fn training_case(name: &str, dir: &Path) -> String {
    use kshana::interop::testbench::{MotionSample, Trajectory};
    let text = std::fs::read_to_string(
        root().join(format!("tests/fixtures/forensics/training/{name}.toml")),
    )
    .unwrap();
    let mut scn = kshana::nmea_synth::TrainingScenario::parse(&text).unwrap();
    scn.output.log_interval_s = 1.0;
    let g = kshana::nmea_synth::generate(&scn).unwrap();
    std::fs::write(dir.join(format!("{name}.nmea")), g.nmea_text()).unwrap();
    let samples = g
        .log
        .track
        .iter()
        .map(|r| {
            let gd = kshana::frames::Geodetic {
                lat_rad: r.true_lat_deg.to_radians(),
                lon_rad: r.true_lon_deg.to_radians(),
                alt_m: 0.0,
            };
            MotionSample {
                t_s: r.t_s,
                lat_deg: r.true_lat_deg,
                lon_deg: r.true_lon_deg,
                h_m: 0.0,
                ecef_m: kshana::frames::geodetic_to_ecef(gd),
                v_ecef_m_s: [0.0; 3],
                heading_deg: 0.0,
                pitch_deg: 0.0,
                roll_deg: 0.0,
            }
        })
        .collect();
    let t = Trajectory {
        kind: "test".into(),
        scenario_hash: "000000000000".into(),
        epoch: kshana::interop::UtcEpoch::parse_iso(&scn.scenario.start_utc).unwrap(),
        note: "test".into(),
        samples,
        events: vec![],
    };
    std::fs::write(
        dir.join(format!("{name}.motion.csv")),
        kshana::interop::testbench::write_motion_csv(&t),
    )
    .unwrap();
    let mut ev = String::new();
    for e in &scn.events {
        ev.push_str(&format!(
            "[[case.events]]\nlabel = \"{}\"\nkind = \"{}\"\nonset_s = {}\nend_s = {}\n",
            e.kind.name(),
            kshana_pro::threat_library::ThreatScenario::event_kind(e.kind),
            e.start_s,
            e.start_s + e.duration_s + e.recovery_s
        ));
    }
    ev
}

fn dev_campaign(dir: &Path) -> String {
    let drag = training_case("coastal-drag-off", dir);
    let jam = training_case("open-sea-jamming", dir);
    format!(
        "kind = \"pro-receiver-scorecard\"\ntitle = \"Development campaign\"\n\
         [receiver]\nmaker = \"Open engine\"\nmodel = \"NMEA receiver model\"\n\
         [campaign]\nid = \"DEV-1\"\nfacility = \"desk (synthetic)\"\n\
         [bars]\nfixed_utc = \"2026-11-15T12:00:00Z\"\n\
         [[case]]\nid = \"drag\"\n[case.log]\nformat = \"nmea\"\npath = \"coastal-drag-off.nmea\"\nplatform = \"vessel\"\n[case.truth]\nmotion_csv = \"coastal-drag-off.motion.csv\"\n{drag}\
         [[case]]\nid = \"jam\"\n[case.log]\nformat = \"nmea\"\npath = \"open-sea-jamming.nmea\"\nplatform = \"vessel\"\n[case.truth]\nmotion_csv = \"open-sea-jamming.motion.csv\"\n{jam}\
         [[case]]\nid = \"jam-no-truth\"\n[case.log]\nformat = \"nmea\"\npath = \"open-sea-jamming.nmea\"\nplatform = \"vessel\"\n{jam}"
    )
}

fn dev_score() -> scorecards::Scorecard {
    // One directory per call: tests calling this in parallel must not clear each other's files.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let d = tmp(&format!(
        "dev{}",
        N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let src = dev_campaign(&d);
    std::fs::write(d.join("campaign.toml"), &src).unwrap();
    let (c, src) = Campaign::from_file(&d.join("campaign.toml")).unwrap();
    scorecards::score(&c, &src).unwrap()
}

#[test]
fn a_campaign_scores_detection_error_misleading_output_recovery_and_levels() {
    let s = dev_score();
    let by = |id: &str| s.cases.iter().find(|c| c.id == id).unwrap();
    // A drag-off: the receiver keeps a valid, wrong fix with no warning of its own, so level 2
    // is not met; the open monitor alarms through it, so nothing is left unmonitored.
    let drag = by("drag");
    assert_eq!(drag.truth, "motion-csv");
    assert!(drag.max_error_m.unwrap() > 1000.0);
    assert!(drag.misleading.unwarned_epochs.unwrap() > 100);
    assert_eq!(drag.misleading.unmonitored_epochs, Some(0));
    assert_eq!(drag.levels.l1, Verdict::Met);
    assert_eq!(drag.levels.l2, Verdict::NotMet);
    // Jamming: the fix is mostly lost (no wrong output), so level 2 holds and level 3 does not.
    let jam = by("jam");
    assert!(jam.fix_availability.unwrap() < 0.5);
    assert_eq!(jam.misleading.unwarned_epochs, Some(0));
    assert_eq!(
        (jam.levels.l2, jam.levels.l3),
        (Verdict::Met, Verdict::NotMet)
    );
    // Without truth, error, misleading output and levels 2 to 4 are not assessed.
    let nt = by("jam-no-truth");
    assert_eq!(nt.truth, "none");
    assert_eq!(nt.max_error_m, None);
    assert_eq!(nt.misleading.unwarned_epochs, None);
    assert_eq!(nt.levels.l2, Verdict::NotAssessed);
    assert_eq!(s.levels.highest_supported, 1);
    assert!(s.limits.iter().any(|l| l.contains("not assessed")));
}

#[test]
fn the_scorecard_format_is_frozen() {
    let s = dev_score();
    let v = serde_json::to_value(&s).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    let mut want: Vec<&str> = scorecards::SCORECARD_FIELDS.to_vec();
    let mut got = keys.clone();
    got.sort();
    want.sort();
    assert_eq!(
        got,
        want,
        "top-level fields of {} changed: that is a new format",
        scorecards::FORMAT
    );
    let c = v["cases"][0].as_object().unwrap();
    let mut got: Vec<&str> = c.keys().map(String::as_str).collect();
    let mut want: Vec<&str> = scorecards::CASE_FIELDS.to_vec();
    got.sort();
    want.sort();
    assert_eq!(got, want, "case fields changed: that is a new format");
    assert_eq!(scorecards::FORMAT, "kshana-pro-receiver-scorecard/1");
}

#[test]
fn campaigns_are_validated_before_scoring() {
    let base = "kind = \"pro-receiver-scorecard\"\ntitle = \"t\"\n[receiver]\nmaker = \"m\"\nmodel = \"x\"\n[campaign]\nid = \"c\"\n";
    let log_no_events =
        format!("{base}[[case]]\nid = \"a\"\n[case.log]\nformat = \"nmea\"\npath = \"x.nmea\"\n");
    assert!(Campaign::parse(&log_no_events)
        .unwrap_err()
        .to_string()
        .contains("states its events"));
    let twice = format!("{base}[[case]]\nid = \"a\"\nlibrary = \"hormuz-meaconing-delay\"\n[[case]]\nid = \"a\"\nlibrary = \"hormuz-meaconing-delay\"\n");
    assert!(Campaign::parse(&twice).is_err());
    let bad_bars = format!("{base}[bars]\nfull_error_m = 900.0\n[[case]]\nid = \"a\"\nlibrary = \"hormuz-meaconing-delay\"\n");
    assert!(Campaign::parse(&bad_bars).is_err());
    assert!(Campaign::parse(base).is_err());
    let unknown = format!("{base}[[case]]\nid = \"a\"\nlibrary = \"no-such-scenario\"\n");
    let c = Campaign::parse(&unknown).unwrap();
    assert!(scorecards::score(&c, &unknown).is_err());
}

#[test]
fn the_kind_is_licence_gated_at_pro_and_listed_in_the_tier_manifest() {
    use kshana_pro::license::{self, tiers, Entitlement, Tier};
    let ent = |tier| Entitlement {
        licensee: "t".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&ent(Tier::Evaluation), scorecards::KIND, 0).is_err());
    assert!(license::check(&ent(Tier::Pro), scorecards::KIND, 0).is_ok());
    assert_eq!(
        tiers::edition_of(scorecards::KIND),
        Some(tiers::Edition::Pro)
    );
    assert!(kshana_pro::registry::is_pro_kind(scorecards::KIND));
}

#[test]
fn reports_are_signed_verify_for_free_and_say_mapped_to_only() {
    let s = dev_score();
    let out = tmp("report").join("r");
    scorecards::report::write_dir(&out, &s, "campaign").unwrap();
    let seed = [7u8; 32];
    kshana_pro::signed_dossier::sign(&out, &seed, "Test lab", "2026-11-15T00:00:00Z").unwrap();
    let pk = kshana_pro::license::public_key_for(&seed);
    assert!(kshana_pro::signed_dossier::verify(&out, &[pk])
        .unwrap()
        .passed());
    let html = std::fs::read_to_string(out.join("report.html"))
        .unwrap()
        .to_ascii_lowercase();
    for w in ["compliant", "conformant", "certified", "conforms"] {
        assert!(!html.contains(w), "the report says '{w}'");
    }
    assert!(html.contains("test evidence"));
    // A one-byte change is caught.
    let p = out.join("cases.csv");
    let mut t = std::fs::read_to_string(&p).unwrap();
    t.push(' ');
    std::fs::write(&p, t).unwrap();
    assert!(!kshana_pro::signed_dossier::verify(&out, &[pk])
        .unwrap()
        .passed());
    // A non-empty directory is refused.
    assert!(scorecards::report::write_dir(&out, &s, "campaign").is_err());
}

#[test]
fn the_engine_pin_in_scorecards_matches_cargo_toml() {
    let cargo = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    assert!(cargo.contains(&format!("kshana = \"={}\"", scorecards::OPEN_ENGINE_PIN)));
}

// ---------------------------------------------------------------------------------------
// The public sample campaign (pre-registered in docs/evidence/e3-receiver-scorecard/).
// ---------------------------------------------------------------------------------------

const SAMPLE_UTC: &str = "2026-11-15T14:00:00Z";
const SAMPLE_SIGNER: &str = "Kshana Pro sample (demo key)";

fn sample() -> (scorecards::Scorecard, String) {
    let (c, src) =
        Campaign::from_file(&root().join("examples/scorecards/public-analogue-campaign.toml"))
            .unwrap();
    (scorecards::score(&c, &src).unwrap(), src)
}

fn sample_dir() -> PathBuf {
    root().join("report-templates/samples/receiver-scorecard-public")
}

/// E3-B1: byte-identical scorings; the published sample is this build's scorecard and
/// verifies with its published demo key. Regenerate with KSHANA_PRO_REGEN_E3_SAMPLE=<key file>.
#[test]
fn e3_b1_the_sample_is_reproducible_and_verifies_with_its_published_demo_key() {
    let (a, src) = sample();
    let (b, _) = sample();
    let ja = serde_json::to_string_pretty(&a).unwrap() + "\n";
    assert_eq!(ja, serde_json::to_string_pretty(&b).unwrap() + "\n");
    let dir = sample_dir();
    if let Some(kf) = std::env::var_os("KSHANA_PRO_REGEN_E3_SAMPLE") {
        let hex = std::fs::read_to_string(kf).unwrap();
        let seed = kshana_pro::license::parse_key_hex(hex.trim()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        scorecards::report::write_dir(&dir, &a, &src).unwrap();
        kshana_pro::signed_dossier::sign(&dir, &seed, SAMPLE_SIGNER, SAMPLE_UTC).unwrap();
        std::fs::write(
            root().join("report-templates/samples/receiver-scorecard-public.demo-public-key.txt"),
            hex::encode(kshana_pro::license::public_key_for(&seed)) + "\n",
        )
        .unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("receiver-scorecard.json")).unwrap(),
        ja,
        "the sample is stale; regenerate it"
    );
    let key = std::fs::read_to_string(
        root().join("report-templates/samples/receiver-scorecard-public.demo-public-key.txt"),
    )
    .unwrap();
    let pk = kshana_pro::license::parse_key_hex(key.trim()).unwrap();
    let v = kshana_pro::signed_dossier::verify(&dir, &[pk]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    let tmpd = tmp("sample-tamper").join("s");
    copy_dir(&dir, &tmpd);
    let p = tmpd.join("report.html");
    let mut t = std::fs::read(&p).unwrap();
    t[100] ^= 1;
    std::fs::write(&p, t).unwrap();
    assert!(!kshana_pro::signed_dossier::verify(&tmpd, &[pk])
        .unwrap()
        .passed());
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// E3-B2: the instructor log (the generator's own truth-versus-reported rows) gives the same
/// errors and misleading counts by a different route. Published outcome (RESULTS.md): part (a)
/// MISSED on five events: on the meaconing case the scorecard aligns truth by the receiver's own
/// (delayed) epoch time and so sees 6.4 m where the generator's true-time error is 136 m; on the
/// four drag-offs of 0.6 to 8 km the two distance computations differ by up to 0.4 %. Part (b)
/// met (worst difference 1 epoch).
const E3_B2A_MET: bool = false;
const E3_B2B_MET: bool = true;

#[test]
fn e3_b2_the_independent_oracle_is_as_published() {
    let (s, _) = sample();
    let (mut a_ok, mut b_ok) = (true, true);
    let (mut worst_abs, mut worst_rel) = (0.0f64, 0.0f64);
    let mut worst_count = 0i64;
    let mut misses: Vec<String> = Vec::new();
    for c in &s.cases {
        let id = c.source.strip_prefix("library:").unwrap();
        let sc = kshana_pro::threat_library::get(id).unwrap();
        let an = kshana_pro::threat_library::export::analogue(&sc)
            .unwrap()
            .unwrap();
        let rows = &an.log().track;
        let end_log = rows.last().unwrap().t_s;
        for (k, e) in c.events.iter().enumerate() {
            let in_w = rows
                .iter()
                .filter(|r| r.t_s >= e.onset_s && r.t_s <= e.end_s && r.fix_valid);
            let max = in_w
                .filter_map(|r| r.position_error_m)
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
            match (max, e.max_error_m) {
                (Some(o), Some(g)) => {
                    let d = (o - g).abs();
                    if d > 1.0 {
                        misses.push(format!("{}/{}", c.id, e.label));
                        if !c.id.contains("meaconing") {
                            worst_rel = worst_rel.max(d / o);
                        }
                    }
                    worst_abs = worst_abs.max(d);
                    if d > 1.0 {
                        a_ok = false;
                    }
                }
                (None, None) => {}
                _ => a_ok = false,
            }
            let next = c
                .events
                .get(k + 1)
                .map(|n| n.onset_s)
                .unwrap_or(f64::INFINITY);
            let stop = match (e.recovered, e.recovery_s) {
                (Some(true), Some(r)) => e.end_s + r,
                _ => next.min(end_log + 1.0),
            };
            let n = rows
                .iter()
                .filter(|r| r.t_s >= e.onset_s && r.t_s < stop && r.fix_valid)
                .filter(|r| {
                    r.position_error_m.is_some_and(|d| d > 50.0) || r.time_offset_s.abs() > 0.5
                })
                .count() as i64;
            let g = e.misleading_unwarned.unwrap() as i64;
            worst_count = worst_count.max((n - g).abs());
            if (n - g).abs() > 2 {
                b_ok = false;
            }
        }
    }
    eprintln!("E3-B2: worst |max error| difference {worst_abs:.2} m ({:.3} %), worst misleading-count difference {worst_count}", worst_rel * 100.0);
    assert_eq!(
        (a_ok, b_ok),
        (E3_B2A_MET, E3_B2B_MET),
        "the oracle outcome changed; re-publish"
    );
    assert_eq!(
        misses,
        [
            "hormuz-displacement/Position displaced",
            "hormuz-meaconing/Re-broadcast delay",
            "red-sea-jam-then-drag/Counterfeit fix after re-acquisition",
            "jammertest-position/Position spoofing",
            "texbat-ds4-analogue/Matched-power position push",
        ]
    );
    assert!(
        worst_rel < 0.005,
        "a drag-off difference beyond the published 0.4 %"
    );
}

/// E3-B3: over the spoofing-class events, unmonitored misleading epochs are at most half the
/// unwarned ones. The published outcome (RESULTS.md) is E3_B3_MET.
const E3_B3_MET: bool = true;

#[test]
fn e3_b3_what_the_open_monitor_adds_is_as_published() {
    let (s, _) = sample();
    let (mut unwarned, mut unmonitored) = (0usize, 0usize);
    for c in &s.cases {
        for e in c.events.iter().filter(|e| e.kind == "spoofing") {
            unwarned += e.misleading_unwarned.unwrap();
            unmonitored += e.misleading_unmonitored.unwrap();
        }
    }
    assert_eq!(
        (unwarned, unmonitored),
        (4091, 1696),
        "the scored quantities changed; re-publish"
    );
    assert_eq!(2 * unmonitored <= unwarned, E3_B3_MET);
}

// ---------------------------------------------------------------------------------------
// E3b (docs/evidence/e3-receiver-scorecard/2026-11-16_E3b_PREREGISTRATION.md): method v2,
// WGS-84 geodesic error and truth at the arrival time.
// ---------------------------------------------------------------------------------------

/// One event compared with the independent oracle (the generator's instructor log).
#[derive(serde::Serialize, Clone, Debug)]
struct OracleRow {
    case: String,
    event: String,
    scorecard_max_error_m: Option<f64>,
    generator_max_error_m: Option<f64>,
    abs_difference_m: Option<f64>,
    b2a_within_1m: bool,
    scorecard_misleading_unwarned: Option<usize>,
    generator_misleading: i64,
    b2b_within_2: bool,
    misleading_unmonitored: Option<usize>,
    spoofing: bool,
}

fn oracle_rows(c: &Campaign, s: &scorecards::Scorecard) -> Vec<OracleRow> {
    let mut out = Vec::new();
    for (cs, cr) in c.cases.iter().zip(&s.cases) {
        assert_eq!(cs.id, cr.id);
        let sc = scorecards::case_scenario(c, cs).unwrap().unwrap();
        let an = kshana_pro::threat_library::export::analogue(&sc)
            .unwrap()
            .unwrap();
        let rows = &an.log().track;
        let end_log = rows.last().unwrap().t_s;
        for (k, e) in cr.events.iter().enumerate() {
            let max = rows
                .iter()
                .filter(|r| r.t_s >= e.onset_s && r.t_s <= e.end_s && r.fix_valid)
                .filter_map(|r| r.position_error_m)
                .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
            let (diff, a_ok) = match (max, e.max_error_m) {
                (Some(o), Some(g)) => (
                    Some(((o - g).abs() * 1000.0).round() / 1000.0),
                    (o - g).abs() <= 1.0,
                ),
                (None, None) => (None, true),
                _ => (None, false),
            };
            let next = cr
                .events
                .get(k + 1)
                .map(|n| n.onset_s)
                .unwrap_or(f64::INFINITY);
            let stop = match (e.recovered, e.recovery_s) {
                (Some(true), Some(r)) => e.end_s + r,
                _ => next.min(end_log + 1.0),
            };
            let n = rows
                .iter()
                .filter(|r| r.t_s >= e.onset_s && r.t_s < stop && r.fix_valid)
                .filter(|r| {
                    r.position_error_m
                        .is_some_and(|d| d > c.bars.misleading_error_m)
                        || r.time_offset_s.abs() > c.bars.misleading_time_error_s
                })
                .count() as i64;
            let g = e.misleading_unwarned.map(|v| v as i64);
            out.push(OracleRow {
                case: cr.id.clone(),
                event: e.label.clone(),
                scorecard_max_error_m: e.max_error_m,
                generator_max_error_m: max,
                abs_difference_m: diff,
                b2a_within_1m: a_ok,
                scorecard_misleading_unwarned: e.misleading_unwarned,
                generator_misleading: n,
                b2b_within_2: g.is_some_and(|g| (n - g).abs() <= 2),
                misleading_unmonitored: e.misleading_unmonitored,
                spoofing: e.kind == "spoofing",
            });
        }
    }
    out
}

/// E3b development (allowed by the pre-registration, published before the held-out campaign
/// was generated): the E3 public campaign re-scored with method v2. Write the table with
/// KSHANA_PRO_E3B_DEV_WRITE=1; afterwards the test keeps it equal to the code.
#[test]
fn e3b_dev_the_public_campaign_rescored_with_v2_is_as_published() {
    let (mut c, _) =
        Campaign::from_file(&root().join("examples/scorecards/public-analogue-campaign.toml"))
            .unwrap();
    c.method = Some(scorecards::METHOD_ID_V2.into());
    let src =
        std::fs::read_to_string(root().join("examples/scorecards/public-analogue-campaign.toml"))
            .unwrap();
    let s = scorecards::score(&c, &src).unwrap();
    assert_eq!(s.method, scorecards::METHOD_ID_V2);
    let rows = oracle_rows(&c, &s);
    let out = serde_json::json!({
        "what": "E3b development table: the E3 public campaign (development data) re-scored with kshana-pro/receiver-scorecard/v2, against the generator's instructor log. Not the held-out result.",
        "method": scorecards::METHOD_ID_V2,
        "events": rows,
        "b2a_events_within_1m": rows.iter().filter(|r| r.b2a_within_1m).count(),
        "b2b_events_within_2": rows.iter().filter(|r| r.b2b_within_2).count(),
        "n_events": rows.len(),
    });
    let text = serde_json::to_string_pretty(&out).unwrap() + "\n";
    let p = root().join("docs/evidence/e3-receiver-scorecard/E3b_dev.json");
    if std::env::var_os("KSHANA_PRO_E3B_DEV_WRITE").is_some() {
        std::fs::write(&p, &text).unwrap();
    }
    for r in &rows {
        eprintln!("{r:?}");
    }
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        text,
        "E3b_dev.json is stale"
    );
}

const E3B_CAMPAIGN: &str = "examples/scorecards/e3b-heldout-campaign.toml";
const E3B_SIGNER: &str = "Kshana Pro E3b held-out (demo key)";
const E3B_UTC: &str = "2026-11-16T12:00:00Z";

fn e3b_dir() -> PathBuf {
    root().join("report-templates/samples/receiver-scorecard-e3b-heldout")
}

/// The held-out campaign is well formed (parsing only: nothing is generated here).
#[test]
fn e3b_the_heldout_campaign_parses_with_its_sixteen_seeds() {
    let (c, _) = Campaign::from_file(&root().join(E3B_CAMPAIGN)).unwrap();
    assert_eq!(c.method.as_deref(), Some(scorecards::METHOD_ID_V2));
    let seeds: Vec<u64> = c.cases.iter().map(|k| k.analogue_seed.unwrap()).collect();
    assert_eq!(seeds, (31001..=31016).collect::<Vec<_>>());
    for cs in &c.cases {
        let s = scorecards::case_scenario(&c, cs).unwrap().unwrap();
        assert_eq!(
            s.analogue.as_ref().unwrap().scenario.seed,
            cs.analogue_seed.unwrap()
        );
    }
}

/// E3b, scored once (pre-registration section 5). Write with KSHANA_PRO_E3B_WRITE=<key file>
/// (the signed held-out scorecard and `E3b_results.json`); afterwards the test keeps both equal
/// to the code and pins every bar's outcome.
#[test]
fn e3b_scored_the_heldout_campaign_is_as_published() {
    let path = root().join(E3B_CAMPAIGN);
    let (c, src) = Campaign::from_file(&path).unwrap();
    let a = scorecards::score(&c, &src).unwrap();
    let b = scorecards::score(&c, &src).unwrap();
    let ja = serde_json::to_string_pretty(&a).unwrap() + "\n";
    let b1_identical = ja == serde_json::to_string_pretty(&b).unwrap() + "\n";
    let dir = e3b_dir();
    let res_path = root().join("docs/evidence/e3-receiver-scorecard/E3b_results.json");
    let rows = oracle_rows(&c, &a);
    let (mut unwarned, mut unmonitored) = (0usize, 0usize);
    for r in rows.iter().filter(|r| r.spoofing) {
        unwarned += r.scorecard_misleading_unwarned.unwrap();
        unmonitored += r.misleading_unmonitored.unwrap();
    }
    let b2a = rows.iter().all(|r| r.b2a_within_1m);
    let b2b = rows.iter().all(|r| r.b2b_within_2);
    let b3 = 2 * unmonitored <= unwarned;
    if let Some(kf) = std::env::var_os("KSHANA_PRO_E3B_WRITE") {
        let hex = std::fs::read_to_string(kf).unwrap();
        let seed = kshana_pro::license::parse_key_hex(hex.trim()).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        scorecards::report::write_dir(&dir, &a, &src).unwrap();
        kshana_pro::signed_dossier::sign(&dir, &seed, E3B_SIGNER, E3B_UTC).unwrap();
        std::fs::write(
            root().join(
                "report-templates/samples/receiver-scorecard-e3b-heldout.demo-public-key.txt",
            ),
            hex::encode(kshana_pro::license::public_key_for(&seed)) + "\n",
        )
        .unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("receiver-scorecard.json")).unwrap(),
        ja,
        "the held-out scorecard is stale"
    );
    let key = std::fs::read_to_string(
        root().join("report-templates/samples/receiver-scorecard-e3b-heldout.demo-public-key.txt"),
    )
    .unwrap();
    let pk = kshana_pro::license::parse_key_hex(key.trim()).unwrap();
    let verifies = kshana_pro::signed_dossier::verify(&dir, &[pk])
        .unwrap()
        .passed();
    let tmpd = tmp("e3b-tamper").join("s");
    copy_dir(&dir, &tmpd);
    let p = tmpd.join("report.html");
    let mut t = std::fs::read(&p).unwrap();
    t[100] ^= 1;
    std::fs::write(&p, t).unwrap();
    let tamper_fails = !kshana_pro::signed_dossier::verify(&tmpd, &[pk])
        .unwrap()
        .passed();
    let b1 = b1_identical && verifies && tamper_fails;
    let out = serde_json::json!({
        "what": "E3b held-out result (SYNTHETIC campaign, seeds 31001-31016), method kshana-pro/receiver-scorecard/v2, scored once.",
        "campaign": E3B_CAMPAIGN,
        "campaign_sha256": a.campaign_sha256,
        "method": a.method,
        "b1": {"byte_identical": b1_identical, "signed_dir_verifies": verifies, "one_byte_change_fails": tamper_fails, "met": b1},
        "b2a": {"events_within_1m": rows.iter().filter(|r| r.b2a_within_1m).count(), "events": rows.len(), "met": b2a},
        "b2b": {"events_within_2": rows.iter().filter(|r| r.b2b_within_2).count(), "events": rows.len(), "met": b2b},
        "b3": {"unwarned": unwarned, "unmonitored": unmonitored, "met": b3},
        "e3b_met": b1 && b2a && b2b && b3,
        "rows": rows,
    });
    let text = serde_json::to_string_pretty(&out).unwrap() + "\n";
    if std::env::var_os("KSHANA_PRO_E3B_WRITE").is_some() {
        std::fs::write(&res_path, &text).unwrap();
    }
    for r in &rows {
        eprintln!("{r:?}");
    }
    eprintln!("E3b: B1 {b1} B2a {b2a} B2b {b2b} B3 {b3} ({unmonitored}/{unwarned})");
    assert_eq!(
        std::fs::read_to_string(&res_path).unwrap(),
        text,
        "E3b_results.json is stale; the outcome changed"
    );
}

// ---------------------------------------------------------------------------------------------
// B2603: per-firmware behaviour fingerprint for the population-benchmark give-back.

mod fingerprint {
    use super::*;
    use kshana_pro::fingerprint::{self, Consent, Request};

    const SEED: [u8; 32] = [21u8; 32];

    fn key_hex() -> String {
        hex::encode(kshana_pro::license::public_key_for(&SEED))
    }

    /// Two signed scorecard directories for one firmware: a receiver scorecard and an OSNMA
    /// scorecard (receiver fields set to the same identity).
    fn cards(d: &Path, firmware_osnma: &str) {
        let mut sc = dev_score();
        sc.receiver.maker = "Synthetic".into();
        sc.receiver.model = "RX-1".into();
        sc.receiver.firmware = Some("2.4.0".into());
        let a = d.join("rx");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(
            a.join("receiver-scorecard.json"),
            serde_json::to_string(&sc).unwrap(),
        )
        .unwrap();
        kshana_pro::signed_dossier::sign(&a, &SEED, "lab", "2026-11-20T00:00:00Z").unwrap();
        let log = kshana_pro::osnma_conformance::synth::log(true);
        let bytes = serde_json::to_vec(&log).unwrap();
        let o = kshana_pro::osnma_conformance::score(&log, &bytes, &Default::default()).unwrap();
        let mut v = serde_json::to_value(&o).unwrap();
        v["receiver"] =
            serde_json::json!({"make": "Synthetic", "model": "RX-1", "firmware": firmware_osnma});
        let b = d.join("osnma");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("osnma-scorecard.json"), v.to_string()).unwrap();
        kshana_pro::signed_dossier::sign(&b, &SEED, "lab", "2026-11-20T00:00:00Z").unwrap();
    }

    fn req(give_back: bool) -> Request {
        Request {
            title: "t".into(),
            customer: "Customer Ltd".into(),
            pseudonym_salt: "salt".into(),
            scorecards: vec!["rx".into(), "osnma".into()],
            trust: vec![key_hex()],
            consent: Consent {
                give_back,
                scope: fingerprint::SCOPE.into(),
                granted_by: "DPO".into(),
                granted_utc: "2026-11-20T00:00:00Z".into(),
            },
        }
    }

    #[test]
    fn fingerprint_from_signed_scorecards_with_consent() {
        let d = tmp("fp");
        cards(&d, "2.4.0");
        let (f, c) = fingerprint::build(&req(true), &d).unwrap();
        assert_eq!(f.identity.firmware, "2.4.0");
        assert_eq!(f.sources.len(), 2);
        for k in ["rx.rpcf_level", "osnma.met_share", "osnma.ttfaf"] {
            assert!(f.features.contains_key(k), "{k}: {:?}", f.features);
        }
        let c = c.unwrap();
        assert_eq!(c.fingerprint_id, f.fingerprint_id);
        let j = serde_json::to_string(&c).unwrap();
        assert!(!j.contains("Customer Ltd") && !j.contains("salt"), "{j}");
        // Stable: the same scorecards give the same identifier.
        assert_eq!(
            fingerprint::build(&req(true), &d).unwrap().0.fingerprint_id,
            f.fingerprint_id
        );
        // Without consent the fingerprint stays local.
        let (f2, c2) = fingerprint::build(&req(false), &d).unwrap();
        assert!(c2.is_none() && !f2.contributable);
    }

    #[test]
    fn fingerprint_refuses_other_scopes_mixed_firmware_and_untrusted_keys() {
        let d = tmp("fp-bad");
        cards(&d, "2.5.0");
        assert!(fingerprint::build(&req(true), &d)
            .unwrap_err()
            .contains("per firmware"));
        let d = tmp("fp-scope");
        cards(&d, "2.4.0");
        let mut q = req(true);
        q.consent.scope = "marketing".into();
        assert!(fingerprint::build(&q, &d).is_err());
        let mut q = req(true);
        q.trust = vec![hex::encode(kshana_pro::license::public_key_for(&[1u8; 32]))];
        assert!(fingerprint::build(&q, &d)
            .unwrap_err()
            .contains("does not verify"));
    }
}
