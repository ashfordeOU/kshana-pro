// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Roadmap 0.42 E4 to E7: the PNT service verification study, AIS integrity scoring, framework
//! evidence packs and the insurer exposure summary, with their public samples.
//!
//! Samples are signed with a demo key whose public half is published next to each sample.
//! Regenerate (the key file holds the demo seed as hex; never committed):
//!
//!     KSHANA_PRO_REGEN_E5_DATA=1           cargo test --test evidence_products e5_sample_data
//!     KSHANA_PRO_REGEN_EVIDENCE=<key file> cargo test --test evidence_products -- --test-threads 1
//!
//! The E4 sample's bar outcomes are the ones published in docs/evidence/e4-pnt-verification/RESULTS.md.

use std::path::{Path, PathBuf};

use kshana_pro::frameworks;
use kshana_pro::integrity::{self, IntegrityScenario};
use kshana_pro::license::{self, Entitlement, Tier};
use kshana_pro::pnt_study::{self, StudySpec};
use kshana_pro::{exposure, signed_dossier};

const SIGNER: &str = "Kshana Pro sample (demo key)";
const UTC: &str = "2026-11-15T16:00:00Z";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn samples() -> PathBuf {
    root().join("report-templates/samples")
}

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("ev-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn regen_seed() -> Option<[u8; 32]> {
    std::env::var_os("KSHANA_PRO_REGEN_EVIDENCE").map(|f| {
        let hex = std::fs::read_to_string(f).unwrap();
        license::parse_key_hex(hex.trim()).unwrap()
    })
}

fn demo_key(name: &str) -> [u8; 32] {
    let t = std::fs::read_to_string(samples().join(format!("{name}.demo-public-key.txt"))).unwrap();
    license::parse_key_hex(t.trim()).unwrap()
}

/// Write `name`'s sample with `write`, sign it with the regeneration key, publish the key.
fn regen(name: &str, seed: &[u8; 32], write: impl Fn(&Path)) {
    let dir = samples().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    write(&dir);
    signed_dossier::sign(&dir, seed, SIGNER, UTC).unwrap();
    std::fs::write(
        samples().join(format!("{name}.demo-public-key.txt")),
        hex::encode(license::public_key_for(seed)) + "\n",
    )
    .unwrap();
}

fn verify_sample(name: &str) {
    let v = signed_dossier::verify(&samples().join(name), &[demo_key(name)]).unwrap();
    assert!(v.passed(), "{name}: {:?}", v.problems);
}

fn evaluation() -> Entitlement {
    Entitlement {
        licensee: "eval".into(),
        tier: Tier::Evaluation,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

// ─── E5 ─────────────────────────────────────────────────────────────────────────────────

const E5_DIR: &str = "examples/ais-integrity";

/// The E5 sample data: a subset of the open M4 generator's development seed 50 (synthetic,
/// never a scored seed): twelve identities with their AIS reports, and the RF fixes near them.
#[test]
fn e5_sample_data_is_a_subset_of_the_open_generator() {
    if std::env::var_os("KSHANA_PRO_REGEN_E5_DATA").is_none() {
        return;
    }
    use kshana::m4_dark_vessel::generate;
    let scene = generate(50);
    let vs = scene.truth["vessels"].as_array().unwrap();
    let pick = |label: &str, n: usize| -> Vec<u64> {
        vs.iter()
            .filter(|v| v["label"] == label)
            .take(n)
            .map(|v| v["id"].as_u64().unwrap())
            .collect()
    };
    let mut ids = pick("NORMAL", 6);
    ids.extend(pick("POSITION-OFFSET", 3));
    ids.extend(pick("GAP", 2));
    ids.extend(pick("IDENTITY-DUPLICATE", 1));
    let ais: Vec<_> = scene.ais.iter().filter(|r| ids.contains(&r.1)).collect();
    let (mut la0, mut la1, mut lo0, mut lo1) = (90.0f64, -90.0f64, 180.0f64, -180.0f64);
    for r in &ais {
        la0 = la0.min(r.2);
        la1 = la1.max(r.2);
        lo0 = lo0.min(r.3);
        lo1 = lo1.max(r.3);
    }
    let (dla, dlo) = (0.3, 0.6);
    let mut a = String::from("t_s,id,lat,lon\n");
    for r in &ais {
        a.push_str(&format!("{},{},{:.6},{:.6}\n", r.0, r.1, r.2, r.3));
    }
    let mut f = String::from("t_s,lat,lon\n");
    for r in scene
        .rf
        .iter()
        .filter(|r| r.1 >= la0 - dla && r.1 <= la1 + dla && r.2 >= lo0 - dlo && r.2 <= lo1 + dlo)
    {
        f.push_str(&format!("{},{:.6},{:.6}\n", r.0, r.1, r.2));
    }
    let mid = (lo0 + lo1) / 2.0;
    let req = format!(
        "# E5 public sample: synthetic data from the open M4 generator (development seed 50; a subset of\n\
         # twelve identities and the RF fixes near them). Not real vessels.\n\
         kind = \"pro-ais-integrity\"\ntitle = \"AIS and position integrity: public sample (synthetic)\"\n\
         ais = \"ais.csv\"\nrf = \"rf.csv\"\n\n\
         [[fleet]]\nname = \"Fleet A\"\nids = {:?}\n\n[[fleet]]\nname = \"Fleet B\"\nids = {:?}\n\n\
         [[area]]\nname = \"West\"\nlat_min = {:.3}\nlat_max = {:.3}\nlon_min = {:.3}\nlon_max = {:.3}\n\n\
         [[area]]\nname = \"East\"\nlat_min = {:.3}\nlat_max = {:.3}\nlon_min = {:.3}\nlon_max = {:.3}\n",
        &ids[..6],
        &ids[6..],
        la0 - 0.01,
        la1 + 0.01,
        lo0 - 0.01,
        mid,
        la0 - 0.01,
        la1 + 0.01,
        mid,
        lo1 + 0.01
    );
    let d = root().join(E5_DIR);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("ais.csv"), a).unwrap();
    std::fs::write(d.join("rf.csv"), f).unwrap();
    std::fs::write(d.join("request.toml"), req).unwrap();
}

fn e5_sample() -> integrity::IntegrityScore {
    let (s, _) = IntegrityScenario::from_file(&root().join(E5_DIR).join("request.toml")).unwrap();
    integrity::score(&s).unwrap()
}

#[test]
fn e5_sample_is_reproducible_signed_and_counts_add_up() {
    let a = e5_sample();
    assert_eq!(a, e5_sample(), "the same input gives the same scoring");
    // Group counts are recounted from the per-identity rows.
    for g in &a.groups {
        let rows: Vec<_> = a
            .vessels
            .iter()
            .filter(|v| match g.group {
                "all" => true,
                "fleet" => v.fleets.contains(&g.name),
                _ => v.areas.contains(&g.name),
            })
            .collect();
        assert_eq!(rows.len(), g.vessels, "{}", g.name);
        assert_eq!(g.counts.values().sum::<usize>(), g.vessels, "{}", g.name);
        let normal = rows.iter().filter(|v| v.class == "NORMAL").count();
        assert_eq!(g.counts["NORMAL"], normal);
        if g.vessels > 0 {
            assert_eq!(g.integrity_score, Some(normal as f64 / g.vessels as f64));
        }
        assert_eq!(
            g.offset_candidates,
            rows.iter().filter(|v| v.offset_candidate.is_some()).count()
        );
    }
    // Offset candidates never change a class (V3 and V3b were not met).
    assert!(a
        .vessels
        .iter()
        .all(|v| v.offset_candidate.is_none() || v.class == "NORMAL"));
    assert!(a.validation_status.contains("RESULTS.md"));
    if let Some(seed) = regen_seed() {
        regen("ais-integrity-public", &seed, |d| {
            integrity::write_dir(d, &a).unwrap()
        });
    }
    let pub_json = std::fs::read_to_string(
        samples()
            .join("ais-integrity-public")
            .join(integrity::RESULT_FILE),
    )
    .unwrap();
    assert_eq!(
        pub_json,
        serde_json::to_string_pretty(&a).unwrap() + "\n",
        "the E5 sample is stale; regenerate it"
    );
    verify_sample("ais-integrity-public");
}

#[test]
fn e5_without_rf_runs_only_the_ais_checks_and_bad_input_is_refused() {
    let d = tmp("e5-norf");
    std::fs::write(
        d.join("ais.csv"),
        "t_s,id,lat,lon\n0,1,59.0,20.0\n10,1,59.0001,20.0\n2000,1,59.01,20.0\n0,2,59.5,21.0\n10,2,59.5,21.0001\n20,2,60.5,21.0\n30,2,59.5,21.0002\n",
    )
    .unwrap();
    std::fs::write(
        d.join("r.toml"),
        "kind = \"pro-ais-integrity\"\nais = \"ais.csv\"\n",
    )
    .unwrap();
    let (s, _) = IntegrityScenario::from_file(&d.join("r.toml")).unwrap();
    let r = integrity::score(&s).unwrap();
    assert!(!r.rf_cross_check);
    let class = |id: u64| r.vessels.iter().find(|v| v.id == id).unwrap().class;
    assert_eq!(class(1), "GAP");
    assert_eq!(class(2), "IDENTITY-DUPLICATE");
    assert!(r.vessels.iter().all(|v| v.decided_by == "ais-only"));
    assert!(integrity::parse_ais("t,id,lat,lon\n1,2,x,4\n").is_err());
    assert!(integrity::parse_rf("1,2\n").is_err());
}

#[test]
fn e5_to_e7_need_a_pro_licence_and_are_in_the_signed_manifest() {
    let now = 1_800_000_000;
    for k in [
        integrity::KIND,
        pnt_study::KIND,
        frameworks::FEATURE,
        exposure::FEATURE,
    ] {
        assert!(license::check(&evaluation(), k, now).is_err(), "{k}");
        assert_eq!(license::feature_min_tier(k), Tier::Pro, "{k}");
        assert!(
            license::tiers::EMBEDDED_MANIFEST.contains(&format!("\"kind\": \"{k}\"")),
            "{k} is not in the signed tier manifest"
        );
    }
}

// ─── E4 ─────────────────────────────────────────────────────────────────────────────────

/// The published outcome of each E4 sample bar (docs/evidence/e4-pnt-verification/RESULTS.md).
const E4_OUTCOMES: &[(&str, bool)] = &[
    ("E4-S1", false),
    ("E4-S2", false),
    ("E4-S3", true),
    ("E4-S4", true),
    ("E4-S5a", false),
    ("E4-S5b", true),
];

fn e4_spec() -> (StudySpec, String) {
    StudySpec::from_file(&root().join("examples/pnt-verification/study.toml")).unwrap()
}

#[test]
fn e4_sample_study_is_as_published_and_signed() {
    let (s, src) = e4_spec();
    let r = pnt_study::run_study(&s).unwrap();
    for b in &r.bars {
        eprintln!(
            "{} {} {} {}: worst {:?} at {:?} -> {}",
            b.id, b.quantity, b.op, b.value, b.worst, b.worst_lat_deg, b.met
        );
    }
    if let Some(i) = &r.integrity {
        eprintln!(
            "OSNMA: clean {} ({} failed); {} of {} detected; undetected {:?}",
            i.clean_overall, i.clean_failed_satellites, i.detected, i.flips, i.undetected
        );
    }
    if let Some(seed) = regen_seed() {
        regen("pnt-verification-public", &seed, |d| {
            pnt_study::write_dir(d, &r, &src).unwrap()
        });
    }
    let got: Vec<(&str, bool)> = r.bars.iter().map(|b| (b.id.as_str(), b.met)).collect();
    assert_eq!(
        got, E4_OUTCOMES,
        "the E4 sample outcomes differ from the published ones"
    );
    let pub_json = std::fs::read_to_string(
        samples()
            .join("pnt-verification-public")
            .join(pnt_study::RESULT_FILE),
    )
    .unwrap();
    assert_eq!(
        pub_json,
        serde_json::to_string_pretty(&r).unwrap() + "\n",
        "the E4 sample is stale; regenerate it"
    );
    verify_sample("pnt-verification-public");
}

#[test]
fn e4_refuses_a_pre_registration_that_does_not_match() {
    let (mut s, _) = e4_spec();
    s.pre_registration_sha256 = "0".repeat(64);
    let e = pnt_study::run_study(&s).unwrap_err();
    assert!(e.contains("another file"), "{e}");
}

#[test]
fn e4_word_bits_map_to_the_page_bits_the_open_engine_reads() {
    use kshana::osnma::page::InavPage;
    let pages = kshana::osnma::input::parse_pages(
        &std::fs::read_to_string(root().join("examples/pnt-verification/pages-good.txt")).unwrap(),
    )
    .unwrap();
    let p = &pages[0];
    for wb in [0usize, 6, 111, 112, 127] {
        let pb = pnt_study::page_bit_of_word_bit(wb);
        let mut b = [0u8; 30];
        b.copy_from_slice(p.bytes());
        b[pb / 8] ^= 1 << (7 - pb % 8);
        let q = InavPage::from_bytes(p.svid, p.gst, &b);
        let (w0, w1) = (p.data_word(), q.data_word());
        let diff: Vec<usize> = (0..128)
            .filter(|i| (w0[i / 8] ^ w1[i / 8]) >> (7 - i % 8) & 1 == 1)
            .collect();
        assert_eq!(diff, vec![wb]);
    }
}

// ─── E6 ─────────────────────────────────────────────────────────────────────────────────

fn forbidden() -> Vec<String> {
    // Built at run time so this file does not carry the words itself.
    [
        "compl".to_string() + "iant",
        "compl".to_string() + "iance with",
        "con".to_string() + "form",
        "cert".to_string() + "if",
    ]
    .into_iter()
    .collect()
}

#[test]
fn e6_templates_cover_the_seven_frameworks_and_say_mapped_to_only() {
    let ts = frameworks::templates().unwrap();
    let ids: Vec<&str> = ts.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "easa-sib-2022-02r4",
            "nist-ir-8323r1",
            "dhs-rpcf",
            "imo-cl-5037",
            "mifid2-rts25",
            "eu-space-act",
            "nis2"
        ]
    );
    for t in &ts {
        assert!(t.wording.contains("mapped to"), "{}", t.id);
        for d in &t.duties {
            for m in &d.mapped_to {
                assert!(frameworks::product_name(m).is_some(), "{}: {m}", d.id);
            }
            assert!(!d.gap.is_empty(), "{}", d.id);
        }
        // The DHS framework's own name ("... PNT Conformance Framework") is a title, not a claim.
        let text = serde_json::to_string(t)
            .unwrap()
            .to_lowercase()
            .replace("pnt) conformance framework", "")
            .replace("pnt conformance framework", "")
            .replace("conformance_framework", "");
        for w in forbidden() {
            assert!(!text.contains(&w), "{} uses '{w}'", t.id);
        }
    }
    // The template that rests on a circular not yet read says so.
    let imo = ts.iter().find(|t| t.id == "imo-cl-5037").unwrap();
    assert_eq!(imo.status.as_deref(), Some("draft-pending-source"));
    assert!(imo.source.checked.contains("NOT read"));
}

#[test]
fn e6_pack_maps_signed_samples_and_ignores_tampered_evidence() {
    let e3 = samples().join("receiver-scorecard-public");
    let e5 = samples().join("ais-integrity-public");
    let rts = samples().join("timing-audit-rts25");
    let trust = [
        demo_key("receiver-scorecard-public"),
        demo_key("ais-integrity-public"),
        demo_key("timing-audit-rts25"),
        demo_key("pnt-verification-public"),
    ];
    let e4 = samples().join("pnt-verification-public");
    let items: Vec<_> = [&e3, &e5, &rts, &e4]
        .iter()
        .map(|d| frameworks::read_evidence(d, &trust).unwrap())
        .collect();
    assert!(items
        .iter()
        .all(|i| i.signature_valid && i.signer_trusted == Some(true)));
    let t = frameworks::template("nist-ir-8323r1").unwrap();
    let p = frameworks::build(&t, items);
    let de = p.duties.iter().find(|d| d.id == "nist-de").unwrap();
    assert_eq!(de.status, "evidence mapped");
    assert_eq!(p.counts.iter().sum::<usize>(), t.duties.len());
    let html = frameworks::render_html(&p).to_lowercase();
    for w in forbidden() {
        assert!(!html.contains(&w), "the pack page uses '{w}'");
    }
    if let Some(seed) = regen_seed() {
        regen("framework-pack-nist-public", &seed, |d| {
            // Paths in the sample are relative to the samples directory.
            let rel: Vec<_> = [
                "receiver-scorecard-public",
                "ais-integrity-public",
                "timing-audit-rts25",
                "pnt-verification-public",
            ]
            .iter()
            .map(|n| {
                let mut i = frameworks::read_evidence(&samples().join(n), &trust).unwrap();
                i.path = format!("../{n}");
                i
            })
            .collect();
            frameworks::write_dir(d, &frameworks::build(&t, rel)).unwrap();
        });
    }
    verify_sample("framework-pack-nist-public");
    // A tampered copy is listed, not mapped.
    let bad = tmp("e6-bad").join("e5");
    std::fs::create_dir_all(&bad).unwrap();
    for f in std::fs::read_dir(&e5).unwrap() {
        let f = f.unwrap().path();
        std::fs::copy(&f, bad.join(f.file_name().unwrap())).unwrap();
    }
    let csv = bad.join("vessels.csv");
    let mut b = std::fs::read(&csv).unwrap();
    b[20] ^= 1;
    std::fs::write(&csv, b).unwrap();
    let i = frameworks::read_evidence(&bad, &trust).unwrap();
    assert!(!i.signature_valid);
    let t = frameworks::template("easa-sib-2022-02r4").unwrap();
    let p = frameworks::build(&t, vec![i]);
    assert!(p.duties.iter().all(|d| d.mapped_evidence.is_empty()));
    let _ = std::fs::remove_dir_all(bad.parent().unwrap());
}

// ─── E7 ─────────────────────────────────────────────────────────────────────────────────

const E7_DIR: &str = "report-templates/samples/insurer-exposure-public";

/// The E7 sample's forensic reports: three of the open engine's training scenarios analysed by
/// the E1 method (receiver only, standard variant), signed with the demo key.
fn e7_forensics(seed: &[u8; 32]) {
    use kshana_pro::forensics::detect::analyse;
    use kshana_pro::forensics::inputs::{iso, parse_utc};
    use kshana_pro::forensics::IncidentScenario;
    let fix = root().join("tests/fixtures/forensics/training");
    for (k, name) in [
        "open-sea-jamming",
        "coastal-drag-off",
        "fjord-natural-degradation",
    ]
    .iter()
    .enumerate()
    {
        let t = std::fs::read_to_string(fix.join(format!("{name}.toml"))).unwrap();
        let g = kshana::nmea_synth::generate_from_toml(&t, None).unwrap();
        let v: toml::Value = toml::from_str(&t).unwrap();
        let t0 = parse_utc(v["scenario"]["start_utc"].as_str().unwrap()).unwrap();
        let dur = v["scenario"]["duration_s"].as_integer().unwrap() as f64;
        let w = tmp(&format!("e7-{name}"));
        std::fs::write(w.join("bridge.nmea"), g.nmea_text()).unwrap();
        std::fs::write(
            w.join("incident.toml"),
            format!(
                "kind = \"pro-incident-forensics\"\n[incident]\nid = \"SAMPLE-{k}\"\ntitle = \"{name} (synthetic training scenario)\"\n\
                 subject_id = \"MV Sample {k}\"\nstart_utc = \"{}\"\nend_utc = \"{}\"\n\
                 [[receiver]]\nlabel = \"Bridge GNSS\"\npath = \"bridge.nmea\"\nformat = \"nmea\"\nplatform = \"vessel\"\n",
                iso(t0 + 200.0),
                iso(t0 + dur)
            ),
        )
        .unwrap();
        let scn = IncidentScenario::from_file(&w.join("incident.toml")).unwrap();
        let r = analyse(&scn, Some(UTC)).unwrap();
        let out = root().join(E7_DIR).join(format!("forensics/{k}-{name}"));
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            out.join("report.json"),
            serde_json::to_string_pretty(&r).unwrap() + "\n",
        )
        .unwrap();
        signed_dossier::sign(&out, seed, SIGNER, UTC).unwrap();
        let _ = std::fs::remove_dir_all(&w);
    }
}

#[test]
fn e7_sample_summary_is_signed_and_counts_come_from_the_evidence() {
    if let Some(seed) = regen_seed() {
        let _ = std::fs::remove_dir_all(root().join(E7_DIR));
        std::fs::write(
            samples().join("insurer-exposure-public.demo-public-key.txt"),
            hex::encode(license::public_key_for(&seed)) + "\n",
        )
        .unwrap();
        e7_forensics(&seed);
    }
    let req = root().join("examples/insurer-exposure/request.toml");
    let (q, src) = exposure::ExposureRequest::from_file(&req).unwrap();
    let key = demo_key("insurer-exposure-public");
    let trust = [key, demo_key("ais-integrity-public")];
    let x = exposure::build(&q, &trust).unwrap();
    assert_eq!(x.totals.sources_rejected, 0);
    assert_eq!(x.totals.incidents, x.incidents.len());
    assert_eq!(
        x.totals.jamming_indicated,
        x.incidents
            .iter()
            .filter(|i| i.jamming == "indicated")
            .count()
    );
    assert!(x.integrity.is_some());
    if let Some(seed) = regen_seed() {
        let dir = root().join(E7_DIR).join("summary");
        let _ = std::fs::remove_dir_all(&dir);
        exposure::write_dir(&dir, &x, &src).unwrap();
        signed_dossier::sign(&dir, &seed, SIGNER, UTC).unwrap();
    }
    let pub_json = std::fs::read_to_string(
        root()
            .join(E7_DIR)
            .join("summary")
            .join(exposure::RESULT_FILE),
    )
    .unwrap();
    assert_eq!(
        pub_json,
        serde_json::to_string_pretty(&x).unwrap() + "\n",
        "the E7 sample is stale; regenerate it"
    );
    let v = signed_dossier::verify(&root().join(E7_DIR).join("summary"), &[key]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    // A report outside the period is not counted; an untrusted signer is rejected.
    let mut q2 = q.clone();
    q2.period_start_utc = "2001-01-01T00:00:00Z".into();
    q2.period_end_utc = "2001-01-02T00:00:00Z".into();
    assert_eq!(exposure::build(&q2, &trust).unwrap().totals.incidents, 0);
    let other = license::public_key_for(&[9u8; 32]);
    let y = exposure::build(&q, &[other]).unwrap();
    assert_eq!(y.totals.incidents, 0);
    assert!(y.totals.sources_rejected > 0);
}

// ---------------------------------------------------------------------------------------
// E4b (docs/evidence/e4-pnt-verification/2026-11-16_E4b_PREREGISTRATION.md)
// ---------------------------------------------------------------------------------------

use kshana_pro::pnt_study::osnma_v2::{self, Policies};

fn e4b_policy_sets() -> Vec<(&'static str, Policies)> {
    vec![
        ("open verifier (E4)", Policies::NONE),
        (
            "1 session-long memory",
            Policies {
                session_memory: true,
                hold_until_chain: false,
                bits_as_received: false,
            },
        ),
        (
            "1 + 2 hold until the chain verifies",
            Policies {
                session_memory: true,
                hold_until_chain: true,
                bits_as_received: false,
            },
        ),
        ("1 + 2 + 3 (method v2)", Policies::V2),
    ]
}

/// E4b development (allowed; committed before the test vectors were opened): the three
/// policies on the E4 sample stream with E4's seed 4242, cumulatively, to check the code and
/// to settle H1 against H2. Write with KSHANA_PRO_E4B_DEV_WRITE=1.
#[test]
fn e4b_dev_policies_on_the_e4_sample_stream_are_as_published() {
    let pages = kshana::osnma::input::parse_pages(
        &std::fs::read_to_string(root().join("examples/pnt-verification/pages-good.txt")).unwrap(),
    )
    .unwrap();
    let key = kshana::osnma::cli::parse_public_key(
        std::fs::read_to_string(root().join("examples/pnt-verification/public-key.txt"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for (name, pol) in e4b_policy_sets() {
        let c =
            osnma_v2::campaign(&pages, std::slice::from_ref(&key), None, 200, 4242, pol).unwrap();
        let early = c.undetected.iter().filter(|u| u.4 >= 18).count();
        let iodnav = c
            .undetected
            .iter()
            .filter(|u| (1..=4).contains(&u.2) && (6..16).contains(&u.3))
            .count();
        rows.push(serde_json::json!({
            "policies": name,
            "flags": pol,
            "detected": c.detected, "flips": c.flips, "detection_share": c.detection_share,
            "undetected_in_first_six_subframes": early,
            "undetected_iodnav_bits": iodnav,
            "clean_failed": c.clean_failed,
            "clean_chain_verified_gst": c.clean_chain_verified_gst,
            "undetected": c.undetected,
        }));
    }
    // The open-verifier row must reproduce E4's published 136 of 200.
    assert_eq!(rows[0]["detected"], 136);
    let doc = serde_json::json!({
        "what": "E4b development: the E4 sample stream (examples/pnt-verification/pages-good.txt, published) with E4's seed 4242, 200 flips, under the policies of method kshana-pro/pnt-verification/v2 added one at a time. Development data, not the scored result.",
        "rows": rows,
    });
    let text = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    let p = root().join("docs/evidence/e4-pnt-verification/E4b_dev.json");
    if std::env::var_os("KSHANA_PRO_E4B_DEV_WRITE").is_some() {
        std::fs::write(&p, &text).unwrap();
    }
    for r in &rows {
        eprintln!(
            "{} {} early {} iodnav {} clean {}",
            r["policies"],
            r["detected"],
            r["undetected_in_first_six_subframes"],
            r["undetected_iodnav_bits"],
            r["clean_failed"]
        );
    }
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        text,
        "E4b_dev.json is stale"
    );
}
