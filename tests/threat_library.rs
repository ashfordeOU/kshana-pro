// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Threat scenario library (roadmap 0.42, E2): twelve scenarios with provenance and licence
//! notes, no source data, reproducible analogues, I4 exports and a signed feed.

use std::path::{Path, PathBuf};

use kshana_pro::threat_library::{self, export, feed, Family, ThreatScenario};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn the_first_edition_has_twelve_scenarios_from_the_five_families_with_provenance_and_licence_notes()
{
    let lib = threat_library::library().unwrap();
    assert_eq!(lib.len(), 12);
    for f in [
        Family::Baltic,
        Family::Hormuz,
        Family::RedSea,
        Family::Jammertest,
        Family::Texbat,
    ] {
        assert!(lib.iter().filter(|s| s.family == f).count() >= 2, "{f:?}");
    }
    for s in &lib {
        assert!(!s.licence.redistributed);
        assert!(!s.provenance.sources.is_empty());
        if s.family.is_dataset() {
            let r = s.replay.as_ref().unwrap();
            assert!(r.events_from.contains("never from this file"), "{}", s.id);
        }
    }
    // Every file of the library directory is listed in the build, and nothing else is there.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("threat-library/scenarios");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    files.sort();
    let mut ids: Vec<String> = threat_library::LIBRARY
        .iter()
        .map(|(i, _)| format!("{i}.toml"))
        .collect();
    ids.sort();
    assert_eq!(files, ids);
}

#[test]
fn no_source_data_is_carried_anywhere_in_the_library() {
    // No recording, log or capture extension under threat-library/, and no scenario file large
    // enough to hide data.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("threat-library");
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out)
            } else {
                out.push(p)
            }
        }
    }
    let mut all = Vec::new();
    walk(&root, &mut all);
    for p in all {
        assert_eq!(
            p.extension().and_then(|e| e.to_str()),
            Some("toml"),
            "{}",
            p.display()
        );
        assert!(
            std::fs::metadata(&p).unwrap().len() < 16_000,
            "{}",
            p.display()
        );
    }
}

#[test]
fn the_library_rules_refuse_redistribution_missing_provenance_and_forbidden_words() {
    let src = threat_library::source_of("texbat-ds4-matched-power-position-push").unwrap();
    assert!(
        ThreatScenario::parse(&src.replace("redistributed = false", "redistributed = true"))
            .unwrap_err()
            .to_string()
            .contains("never redistributes")
    );
    // A dataset scenario without a [replay] table is refused.
    let cut = &src[..src.find("[replay]").unwrap()];
    let rest = &src[src
        .find("[timing]")
        .or_else(|| src.find("[analogue.scenario]"))
        .unwrap()..];
    assert!(ThreatScenario::parse(&format!("{cut}{rest}")).is_err());
    let base = threat_library::source_of("hormuz-meaconing-delay").unwrap();
    assert!(ThreatScenario::parse(&base.replacen(
        "summary = \"\"\"",
        "summary = \"\"\"A certified test. ",
        1
    ))
    .is_err());
    let one_source = base.replacen("[[provenance.source]]", "[[provenance.ignored]]", 2);
    assert!(ThreatScenario::parse(&one_source).is_err());
}

#[test]
fn analogues_are_deterministic_and_scored_by_the_open_monitors() {
    let s = threat_library::get("jammertest-stepped-power-jamming").unwrap();
    let a = export::run_analogue(&s).unwrap().unwrap();
    let b = export::run_analogue(&s).unwrap().unwrap();
    assert_eq!(a.nmea_sha256, b.nmea_sha256);
    assert_eq!(a.events.len(), 3);
    assert_eq!(a.platform, "static");
    assert!(export::run_analogue(
        &threat_library::get("baltic-timing-holdover-under-jamming").unwrap()
    )
    .unwrap()
    .is_none());
    // Through the dispatcher.
    let out = kshana_pro::registry::run(
        "kind = \"pro-threat-scenario\"\nscenario = \"hormuz-meaconing-delay\"\n",
    )
    .unwrap();
    assert!(out.summary.contains("MODELLED"));
    assert!(out.json.contains("\"redistributed\": false"));
}

#[test]
fn an_export_carries_provenance_the_analogue_and_the_open_i4_test_bench_files() {
    let s = threat_library::get("red-sea-jamming-then-drag-off").unwrap();
    let out = tmp("export");
    let files = export::write(&s, &out).unwrap();
    for f in [
        "scenario.toml",
        "PROVENANCE.txt",
        "analogue/receiver.nmea",
        "analogue/receiver-trust.toml",
        "README.txt",
    ] {
        assert!(files.iter().any(|x| x == f), "{f}");
    }
    let id = &s.id;
    for suffix in [
        ".motion.csv",
        ".motion.json",
        ".nmea",
        ".events.csv",
        ".events.toml",
    ] {
        assert!(
            out.join(format!("testbench/{id}{suffix}")).exists(),
            "{suffix}"
        );
    }
    // The I4 files read back with the open engine's own readers.
    let motion = std::fs::read_to_string(out.join(format!("testbench/{id}.motion.csv"))).unwrap();
    let m = kshana::interop::testbench::read_motion_csv(&motion).unwrap();
    assert!(m.len() > 1000);
    let ev = kshana::interop::testbench::read_events_csv(
        &std::fs::read_to_string(out.join(format!("testbench/{id}.events.csv"))).unwrap(),
    )
    .unwrap();
    assert_eq!(ev.len(), 2);
    // The receiver-trust scenario runs on the analogue stream with the open engine.
    let mut scn: kshana::receiver_trust::scenario::ReceiverTrustScenario =
        toml::from_str(&std::fs::read_to_string(out.join("analogue/receiver-trust.toml")).unwrap())
            .unwrap();
    kshana::receiver_trust::scenario::resolve_paths(&mut scn, &out.join("analogue"));
    assert_eq!(
        kshana::receiver_trust::scenario::run_receiver_trust(&scn)
            .unwrap()
            .events
            .len(),
        2
    );
    let prov = std::fs::read_to_string(out.join("PROVENANCE.txt")).unwrap();
    assert!(prov.contains("Source data included in this library: no."));
    assert!(
        export::write(&s, &out).is_err(),
        "a non-empty directory is refused"
    );
    // A timing scenario exports its profile.
    let t = tmp("export-timing");
    export::write(
        &threat_library::get("texbat-ds3-matched-power-time-push").unwrap(),
        &t,
    )
    .unwrap();
    let csv = std::fs::read_to_string(t.join("timing/profile.csv")).unwrap();
    assert!(csv.lines().last().unwrap().ends_with("2.000000e-6"));
}

#[test]
fn the_feed_is_signed_chained_and_verifies_for_free() {
    let seed = [9u8; 32];
    let pk = kshana_pro::license::public_key_for(&seed);
    let a = tmp("feed-a");
    let m1 = feed::build(
        &a,
        threat_library::LIBRARY,
        "2026-11",
        None,
        &seed,
        "Publisher",
        "2026-11-15T00:00:00Z",
    )
    .unwrap();
    assert_eq!(m1.scenarios.len(), 12);
    assert!(m1.scenarios.iter().all(|e| e.changed_in == "2026-11"));
    assert!(feed::verify(&a, &[pk], None).unwrap().passed());
    // The next edition: one scenario changed, the rest carried over.
    let prev = std::fs::read(a.join("feed.json")).unwrap();
    let mut lib: Vec<(&str, String)> = threat_library::LIBRARY
        .iter()
        .map(|(i, s)| (*i, s.to_string()))
        .collect();
    lib[0].1 = lib[0].1.replace("version = 1", "version = 2");
    let refs: Vec<(&str, &str)> = lib.iter().map(|(i, s)| (*i, s.as_str())).collect();
    let b = tmp("feed-b");
    let m2 = feed::build(
        &b,
        &refs,
        "2026-12",
        Some(&prev),
        &seed,
        "Publisher",
        "2026-12-15T00:00:00Z",
    )
    .unwrap();
    assert_eq!(
        m2.scenarios
            .iter()
            .filter(|e| e.changed_in == "2026-12")
            .count(),
        1
    );
    assert!(feed::verify(&b, &[pk], Some(&prev)).unwrap().passed());
    // An edition that does not move forward is refused.
    assert!(feed::build(
        &tmp("feed-c"),
        threat_library::LIBRARY,
        "2026-10",
        Some(&prev),
        &seed,
        "P",
        "t"
    )
    .is_err());
    // A data file slipped in is caught even when re-signed.
    std::fs::write(b.join("scenarios/ds4.bin"), [0u8; 16]).unwrap();
    kshana_pro::signed_dossier::sign(&b, &seed, "Publisher", "2026-12-15T00:00:00Z").unwrap();
    let v = feed::verify(&b, &[pk], None).unwrap();
    assert!(!v.passed() && v.problems.iter().any(|p| p.contains("never data")));
    // Tampering with a scenario breaks the signature; an untrusted key does not pass.
    let p = a.join("scenarios/hormuz-meaconing-delay.toml");
    let t = std::fs::read_to_string(&p).unwrap().replace("20 s", "25 s");
    std::fs::write(&p, t).unwrap();
    assert!(!feed::verify(&a, &[pk], None).unwrap().passed());
    assert!(!feed::verify(&b, &[[1u8; 32]], None).unwrap().passed());
}

#[test]
fn the_kind_and_feature_are_licence_gated_at_pro() {
    use kshana_pro::license::{self, tiers, Entitlement, Tier};
    let ent = |tier| Entitlement {
        licensee: "t".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    for id in [threat_library::KIND, threat_library::FEATURE] {
        assert!(
            license::check(&ent(Tier::Evaluation), id, 0).is_err(),
            "{id}"
        );
        assert!(license::check(&ent(Tier::Pro), id, 0).is_ok(), "{id}");
        assert_eq!(tiers::edition_of(id), Some(tiers::Edition::Pro));
    }
}
