// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Interference reports (`kshana_pro::ireports`, docs/INTERFERENCE-REPORTS.md): the
//! normaliser on every sample shape, validation per regime, episodes with their start and end
//! rules, verification labels against synthetic observations with known truth, the exposure
//! ledger, the reset check, signing and the licence gates. Sample forms:
//! `tests/data/interference-reports/` (all synthetic).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::ireports::episodes::{self, Params};
use kshana_pro::ireports::verify::Label;
use kshana_pro::ireports::{
    self, ledger, normalise, synth, validate, verify, Kind, Layer, Platform, Report, ReportedType,
    ReporterRole, SourceFormat,
};
use kshana_pro::license::{self, Entitlement, Tier};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");
const SEED: [u8; 32] = [7u8; 32];
const UTC: &str = "2026-11-19T12:00:00Z";

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/interference-reports")
        .join(name)
}

fn read(name: &str) -> String {
    std::fs::read_to_string(data(name)).unwrap()
}

fn norm(name: &str) -> Vec<Report> {
    normalise::normalise(&read(name), name, None).unwrap()
}

fn all_reports() -> Vec<Report> {
    [
        "dgca_forms.txt",
        "easa_ecr.txt",
        "asrs_records.txt",
        "notams.txt",
        "ship_log.txt",
        "finnmark_export.csv",
    ]
    .iter()
    .flat_map(|n| norm(n))
    .collect()
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-ireports-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn by_subject<'a>(rs: &'a [Report], id: &str) -> &'a Report {
    rs.iter()
        .find(|r| r.subject_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("no report for {id}"))
}

/// A work folder with the synthetic observations, `reports.json` and signed `episodes/`.
fn work(tag: &str) -> PathBuf {
    let d = tmp(tag);
    synth::write_all(&d).unwrap();
    let doc = ireports::ReportsDoc::new(all_reports());
    std::fs::write(d.join("reports.json"), ireports::json_text(&doc)).unwrap();
    let adsb = synth::punjab_adsb();
    let ep = episodes::build(
        &doc.reports,
        &[("adsb_punjab.csv".into(), adsb.clone())],
        &Params::default(),
        vec![],
    )
    .unwrap();
    let out = d.join("episodes");
    episodes::write_dir(&out, &ep, &[("adsb_punjab.csv".into(), adsb.into_bytes())]).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Ops", UTC).unwrap();
    d
}

// ------------------------------------------------------------------------------------------
// Normaliser

#[test]
fn every_sample_shape_is_detected_and_read() {
    let cases = [
        ("dgca_forms.txt", SourceFormat::Dgca, 4),
        ("easa_ecr.txt", SourceFormat::Eccairs, 1),
        ("asrs_records.txt", SourceFormat::Asrs, 1),
        ("notams.txt", SourceFormat::Notam, 2),
        ("ship_log.txt", SourceFormat::ShipLog, 2),
        ("finnmark_export.csv", SourceFormat::Csv, 3),
    ];
    for (f, fmt, n) in cases {
        assert_eq!(normalise::detect(&read(f)), fmt, "{f}");
        let rs = norm(f);
        assert_eq!(rs.len(), n, "{f}");
        for r in &rs {
            assert_eq!(r.format, ireports::REPORT_FORMAT);
            assert_eq!(r.schema_version, ireports::SCHEMA_VERSION);
            assert!(r.id.starts_with("rep-") && r.id.len() == 16);
        }
    }
    // Ids are stable and distinct.
    let a = all_reports();
    let b = all_reports();
    assert_eq!(
        a.iter().map(|r| &r.id).collect::<Vec<_>>(),
        b.iter().map(|r| &r.id).collect::<Vec<_>>()
    );
    let mut ids: Vec<&String> = a.iter().map(|r| &r.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), a.len());
}

#[test]
fn dgca_forms_map_to_the_schema() {
    let rs = norm("dgca_forms.txt");
    let abc = by_subject(&rs, "VT-ABC");
    assert_eq!(abc.kind, Kind::Spoofing);
    assert_eq!(abc.spoofed_layer, Layer::Gnss);
    assert_eq!(abc.reporter_role, ReporterRole::Pilot);
    assert_eq!(abc.platform, Platform::Aircraft);
    assert_eq!(
        abc.occurred_start_utc.as_deref(),
        Some("2026-11-12T05:42:00Z")
    );
    assert_eq!(abc.reported_utc.as_deref(), Some("2026-11-12T05:49:00Z"));
    let p = abc.position.as_ref().unwrap();
    assert!((p.lat_deg - 31.7).abs() < 1e-9 && (p.lon_deg - 74.8).abs() < 1e-9);
    assert_eq!(abc.altitude.as_ref().unwrap().lower_ft, Some(35_000.0));
    assert_eq!(abc.time_to_recover_min, Some(18.0));
    assert_eq!(abc.fms_screenshot, Some(true));
    assert_eq!(abc.logs_preserved, Some(true));
    for s in ["GNSS receiver", "FMS", "TAWS/EGPWS", "ADS-B"] {
        assert!(abc.systems_affected.contains(&s.to_string()), "{s}");
    }
    assert!(
        abc.narrative.as_deref().unwrap().contains("EGPWS PULL UP"),
        "continuation line kept"
    );
    assert_eq!(
        abc.extra.get("Operator").map(String::as_str),
        Some("Example Airways")
    );

    let xyz = by_subject(&rs, "VT-XYZ");
    assert_eq!(xyz.reported_type, ReportedType::SignalLoss);
    assert_eq!(xyz.kind, Kind::Unknown, "signal loss alone sets no kind");
    assert_eq!(
        xyz.occurred_start_utc.as_deref(),
        Some("2026-11-12T05:50:00Z")
    );
    assert_eq!(xyz.fms_screenshot, Some(false));
    let atc = rs
        .iter()
        .find(|r| r.reporter_role == ReporterRole::Atc)
        .unwrap();
    assert_eq!(atc.kind, Kind::Jamming);
    assert!(atc.subject_id.is_none());
}

#[test]
fn other_shapes_map_to_the_schema() {
    let e = &norm("easa_ecr.txt")[0];
    assert_eq!((e.kind, e.platform), (Kind::Jamming, Platform::Aircraft));
    assert_eq!(e.reported_utc.as_deref(), Some("2026-06-14T09:05:00Z"));
    assert_eq!(e.altitude.as_ref().unwrap().reference, "AMSL");
    assert_eq!(e.time_to_recover_min, Some(25.0));

    let a = &norm("asrs_records.txt")[0];
    assert_eq!(a.kind, Kind::Spoofing, "type read from the narrative");
    assert!(a.time_uncertainty_s > 10.0 * 86_400.0);
    assert!(a.notes.iter().any(|n| n.contains("de-identified")));
    assert_eq!(a.reporter_role, ReporterRole::Pilot);

    let n = norm("notams.txt");
    assert_eq!(n[0].reporter_role, ReporterRole::Notam);
    assert_eq!(n[0].subject_id.as_deref(), Some("A1234/26"));
    assert_eq!(
        n[0].kind,
        Kind::Spoofing,
        "E) names jamming and spoofing; spoofing wins"
    );
    assert_eq!(
        n[0].occurred_start_utc.as_deref(),
        Some("2026-06-14T00:00:00Z")
    );
    assert_eq!(
        n[0].occurred_end_utc.as_deref(),
        Some("2026-06-30T23:59:00Z")
    );
    assert!((n[0].position.as_ref().unwrap().radius_m.unwrap() - 60.0 * 1852.0).abs() < 1e-6);
    assert_eq!(n[0].altitude.as_ref().unwrap().upper_ft, Some(40_000.0));
    assert_eq!(n[1].reported_type, ReportedType::AnnouncedOutage);

    let s = norm("ship_log.txt");
    assert_eq!(s[0].subject_id.as_deref(), Some("9387421"));
    assert_eq!(s[0].subject_id_type.as_deref(), Some("imo"));
    assert_eq!(
        s[0].extra.get("mmsi").map(String::as_str),
        Some("230123000")
    );
    assert_eq!(
        (s[0].kind, s[0].platform),
        (Kind::Spoofing, Platform::Vessel)
    );
    assert_eq!(s[0].time_to_recover_min, Some(29.0));
    assert!(s[0].systems_affected.contains(&"ECDIS".to_string()));
    assert_eq!(s[1].time_to_recover_min, Some(8.0));
    assert_eq!(s[1].spoofed_layer, Layer::Both);

    let c = norm("finnmark_export.csv");
    assert_eq!(
        c[0].occurred_end_utc.as_deref(),
        Some("2026-05-20T07:31:00Z")
    );
    assert_eq!(c[1].time_to_recover_min, Some(9.0));
    assert_eq!(c[1].altitude.as_ref().unwrap().lower_ft, Some(1600.0));
    assert_eq!(c[2].kind, Kind::Spoofing);
    // The systems field wins over the narrative (which may name systems used to recover).
    assert_eq!(c[2].systems_affected, vec!["FMS", "GNSS receiver"]);
}

#[test]
fn hostile_input_never_panics() {
    for junk in [
        "",
        "\u{0}\u{1}::::",
        "Q) E)",
        "a,b,c,d\n\"unterminated",
        "---\n---\n",
        "2026-13-45 9999Z 99N 999E",
    ] {
        let _ = normalise::normalise(junk, "junk", None);
        for f in [
            SourceFormat::Dgca,
            SourceFormat::Csv,
            SourceFormat::Notam,
            SourceFormat::ShipLog,
        ] {
            let _ = normalise::normalise(junk, "junk", Some(f));
        }
    }
}

// ------------------------------------------------------------------------------------------
// Validation

#[test]
fn dgca_rules_flag_the_ten_minute_rule_and_missing_evidence() {
    let rs = norm("dgca_forms.txt");
    let v = |id: &str| validate::validate(by_subject(&rs, id), "dgca").unwrap();
    let abc = v("VT-ABC");
    assert!(abc.complete(), "{:?}", abc.issues);
    assert_eq!(abc.report_delay_min, Some(7.0));
    let xyz = v("VT-XYZ");
    assert!(!xyz.complete());
    assert_eq!(xyz.report_delay_min, Some(17.0));
    assert!(xyz
        .issues
        .iter()
        .any(|i| i.rule.contains("10 minutes") && i.rule.contains("17")));
    assert!(xyz.issues.iter().any(|i| i.field == "fms_screenshot"));
    assert!(
        !xyz.issues.iter().any(|i| i.field == "reported_type"),
        "signal loss is one of the four DGCA types"
    );
    let atc = rs
        .iter()
        .find(|r| r.reporter_role == ReporterRole::Atc)
        .unwrap();
    let a = validate::validate(atc, "dgca").unwrap();
    assert!(a.issues.iter().any(|i| i.field == "subject_id"));
    assert!(v("VT-PQR").complete());
}

#[test]
fn easa_maritime_asrs_and_generic_rules() {
    let e = &norm("easa_ecr.txt")[0];
    let v = validate::validate(e, "easa").unwrap();
    assert!(v.complete(), "{:?}", v.issues);
    assert!(v.report_delay_min.unwrap() < 72.0 * 60.0);
    let s = norm("ship_log.txt");
    assert!(validate::validate(&s[0], "maritime").unwrap().complete());
    // The same ship report fails the DGCA rules (no time of report, logs, altitude...).
    assert!(!validate::validate(&s[0], "dgca").unwrap().complete());
    let a = &norm("asrs_records.txt")[0];
    assert!(validate::validate(a, "asrs").unwrap().complete());
    assert!(
        !validate::validate(a, "generic").unwrap().complete(),
        "month-level time fails generic"
    );
    assert_eq!(validate::default_regime(a), "asrs");
    assert!(validate::validate(a, "nonsense").is_err());
}

#[test]
fn a_vessel_spoofing_report_must_say_which_layer() {
    let mut r = norm("ship_log.txt")[0].clone();
    r.spoofed_layer = Layer::NotStated;
    let v = validate::validate(&r, "maritime").unwrap();
    assert!(v.issues.iter().any(|i| i.field == "spoofed_layer"));
}

// ------------------------------------------------------------------------------------------
// Episodes

fn episodes_all(p: &Params) -> episodes::EpisodesDoc {
    episodes::build(
        &all_reports(),
        &[("adsb_punjab.csv".into(), synth::punjab_adsb())],
        p,
        vec![],
    )
    .unwrap()
}

fn episode_with<'a>(
    d: &'a episodes::EpisodesDoc,
    rs: &[Report],
    subject: &str,
) -> &'a episodes::Episode {
    let id = &by_subject(rs, subject).id;
    d.episodes
        .iter()
        .find(|e| e.reports.contains(id))
        .unwrap_or_else(|| panic!("no episode with {subject}"))
}

#[test]
fn episodes_follow_the_start_and_end_rules() {
    let rs = all_reports();
    let d = episodes_all(&Params::default());
    assert_eq!(d.format, episodes::FORMAT);
    assert_eq!(d.params_sha256, ireports::params_hash(&Params::default()));
    assert!(d.definition.iter().any(|l| l.starts_with("Start:")));

    let p = episode_with(&d, &rs, "VT-ABC");
    // ABC, XYZ and the ATC report in one episode with four degraded ADS-B aircraft.
    assert!(p.reports.contains(&by_subject(&rs, "VT-XYZ").id));
    assert_eq!(p.reports.len(), 3);
    assert_eq!(
        p.adsb_aircraft,
        vec!["800ABC", "800BBB", "800CCC", "800DEF"]
    );
    assert_eq!(p.kind, "mixed");
    // Start: the first degraded ADS-B sample when the jamming starts (05:38); end: the reports'
    // recovery at 06:00; closed, since data continues past the quiet gap.
    assert_eq!(p.start_utc, "2026-11-12T05:38:00Z");
    assert_eq!(p.end_utc, "2026-11-12T06:00:00Z");
    assert!(!p.open_at_data_end);
    assert!(p.start_rule.contains("adsb:"));
    assert_eq!(p.max_time_to_recover_min, Some(18.0));
    assert_eq!(
        p.context_notams,
        vec![rs
            .iter()
            .find(|r| r.subject_id.as_deref() == Some("A1301/26"))
            .unwrap()
            .id
            .clone()]
    );
    assert!(p.kind_basis["adsb_jumps"] >= 1);

    // VT-PQR near Delhi: its own episode (one report meets min_reports = 1), jamming.
    let q = episode_with(&d, &rs, "VT-PQR");
    assert_eq!(
        (q.kind.as_str(), q.reports.len(), q.adsb_aircraft.len()),
        ("jamming", 1, 0)
    );

    // The two ship entries are 44 minutes apart: two episodes, both with the Gulf NOTAM.
    let s = norm("ship_log.txt");
    let e1 = d
        .episodes
        .iter()
        .find(|e| e.reports.contains(&s[0].id))
        .unwrap();
    let e2 = d
        .episodes
        .iter()
        .find(|e| e.reports.contains(&s[1].id))
        .unwrap();
    assert_ne!(e1.id, e2.id);
    assert_eq!(e1.end_utc, "2026-06-14T04:41:00Z");
    assert_eq!(e1.context_notams.len(), 1);

    // Finnmark: 07:14-07:31, 07:22-07:31, 07:40-07:52; a 9 min gap joins them.
    let f = norm("finnmark_export.csv");
    let fe = d
        .episodes
        .iter()
        .find(|e| e.reports.contains(&f[0].id))
        .unwrap();
    assert_eq!(fe.reports.len(), 3);
    assert_eq!(
        (fe.start_utc.as_str(), fe.end_utc.as_str()),
        ("2026-05-20T07:14:00Z", "2026-05-20T07:52:00Z")
    );
    assert_eq!(fe.kind, "mixed");

    // The ASRS-like record is too coarse; NOTAMs are never members.
    let a = &norm("asrs_records.txt")[0];
    assert!(d
        .not_clustered
        .iter()
        .any(|l| l.id == a.id && l.reason.contains("uncertain")));
    for e in &d.episodes {
        assert!(e.members.iter().all(|m| !m.id.contains('/')));
    }
    assert_eq!(d.adsb.aircraft, 5);
    assert_eq!(d.adsb.unassessed, 0);
}

#[test]
fn the_quiet_gap_is_a_parameter() {
    let f = norm("finnmark_export.csv");
    let p = Params {
        quiet_gap_min: 5.0,
        ..Params::default()
    };
    let d = episodes_all(&p);
    let ids: std::collections::BTreeSet<&str> = f
        .iter()
        .map(|r| {
            d.episodes
                .iter()
                .find(|e| e.reports.contains(&r.id))
                .unwrap()
                .id
                .as_str()
        })
        .collect();
    assert_eq!(
        ids.len(),
        2,
        "07:31 to 07:40 is more than 5 min: the third report opens a new episode"
    );
    assert!(Params {
        join_radius_km: 0.0,
        ..Params::default()
    }
    .check()
    .is_err());
    let t: Params = toml::from_str(&Params::default_toml()).unwrap();
    assert_eq!(t, Params::default());
}

#[test]
fn adsb_alone_needs_two_aircraft() {
    let adsb = synth::punjab_adsb();
    let d = episodes::build(
        &[],
        &[("a.csv".into(), adsb.clone())],
        &Params::default(),
        vec![],
    )
    .unwrap();
    assert_eq!(d.episodes.len(), 1);
    assert_eq!(d.episodes[0].reports.len(), 0);
    assert_eq!(d.episodes[0].adsb_aircraft.len(), 4);
    let one = Params {
        adsb_min_aircraft: 5,
        ..Params::default()
    };
    let d = episodes::build(&[], &[("a.csv".into(), adsb)], &one, vec![]).unwrap();
    assert!(d.episodes.is_empty());
    assert!(!d.below_threshold.is_empty());
}

#[test]
fn episodes_become_valid_unified_events() {
    let d = episodes_all(&Params::default());
    let ev = episodes::to_events(&d);
    assert_eq!(ev.len(), d.episodes.len());
    for e in &ev {
        assert!(
            kshana_pro::ievents::validate(e).is_empty(),
            "{:?}",
            kshana_pro::ievents::validate(e)
        );
        assert!(e.basis.contains("not a probability"));
    }
}

// ------------------------------------------------------------------------------------------
// Verification

fn record<'a>(d: &'a verify::VerificationDoc, id: &str) -> &'a verify::Record {
    d.records.iter().find(|r| r.report_id == id).unwrap()
}

#[test]
fn verification_labels_against_known_truth() {
    let w = work("verify");
    let (d, copies) = verify::run(&w.join("verify.toml")).unwrap();
    assert_eq!(d.format, verify::FORMAT);
    assert!(d.evidence.starts_with("MODELLED"));
    let rs = all_reports();

    // VT-ABC: its own GNSS shows the C/N0 drop, loss of fix and the jump; its ADS-B jumps.
    let abc = record(&d, &by_subject(&rs, "VT-ABC").id);
    assert_eq!(abc.label, Label::Corroborated);
    assert_eq!(abc.kind_agreement, Some(true));
    let direct: Vec<&str> = abc
        .checks
        .iter()
        .filter(|c| c.direct)
        .flat_map(|c| c.indicators.iter().map(|i| i.name.as_str()))
        .collect();
    for n in [
        "fix-lost",
        "position-jump",
        "adsb-position-jump",
        "adsb-integrity-drop",
    ] {
        assert!(direct.contains(&n), "{n} in {direct:?}");
    }

    // VT-XYZ (signal loss, kind unknown): corroborated by its own ADS-B (alias 800DEF) and
    // by VT-ABC's receiver nearby, which counts as independent.
    let xyz = record(&d, &by_subject(&rs, "VT-XYZ").id);
    assert_eq!(xyz.label, Label::Corroborated);
    assert!(xyz
        .checks
        .iter()
        .any(|c| c.label.contains("800DEF") && c.direct));
    assert!(xyz
        .checks
        .iter()
        .any(|c| c.source_type == "receiver" && !c.direct));

    // VT-PQR reports jamming; its own ADS-B (800AAA) is clean and covers the window.
    let pqr = record(&d, &by_subject(&rs, "VT-PQR").id);
    assert_eq!(pqr.label, Label::Contradicted, "{:?}", pqr.reasons);
    assert!(pqr.reasons[0].contains("800AAA"));

    // The ATC report has no subject: nearby aircraft corroborate it.
    let atc = rs
        .iter()
        .find(|r| r.reporter_role == ReporterRole::Atc)
        .unwrap();
    let a = record(&d, &atc.id);
    assert_eq!(a.label, Label::Corroborated);
    assert!(
        a.checks.iter().all(|c| !c.direct),
        "no subject: every source is independent"
    );

    // The vessel: GNSS spoofing first, then AIS-only manipulation.
    let s = norm("ship_log.txt");
    let v1 = record(&d, &s[0].id);
    assert_eq!(v1.label, Label::Corroborated);
    assert_eq!(v1.observed_layer.as_deref(), Some("gnss"));
    let rx: Vec<&str> = v1
        .checks
        .iter()
        .filter(|c| c.source_type == "receiver" && c.direct)
        .flat_map(|c| c.indicators.iter().map(|i| i.name.as_str()))
        .collect();
    assert!(
        rx.contains(&"cn0-raised-uniform") && rx.contains(&"position-jump"),
        "{rx:?}"
    );
    let v2 = record(&d, &s[1].id);
    assert_eq!(v2.label, Label::Corroborated);
    assert_eq!(
        v2.observed_layer.as_deref(),
        Some("ais-only"),
        "{:?}",
        v2.reasons
    );

    // Coarse and unplaced reports stay uncorroborated, with the reason.
    let asrs = record(&d, &norm("asrs_records.txt")[0].id);
    assert_eq!(asrs.label, Label::Uncorroborated);
    assert!(asrs.reasons[0].contains("too coarse"));
    let f = norm("finnmark_export.csv");
    assert_eq!(record(&d, &f[0].id).label, Label::Uncorroborated);

    // Signed record, verified; a tampered file is caught.
    let out = w.join("verification");
    verify::write_dir(&out, &d, &copies).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Ops", UTC).unwrap();
    let key = license::public_key_for(&SEED);
    assert!(kshana_pro::signed_dossier::verify(&out, &[key])
        .unwrap()
        .passed());
    assert!(out.join("inputs/vt-abc_gnss.csv").exists());
    let j = std::fs::read_to_string(out.join(verify::RESULT_FILE)).unwrap();
    std::fs::write(
        out.join(verify::RESULT_FILE),
        j.replace("CONTRADICTED", "CORROBORATED"),
    )
    .unwrap();
    assert!(!kshana_pro::signed_dossier::verify(&out, &[key])
        .unwrap()
        .passed());
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn a_clean_subject_receiver_contradicts_and_kind_mismatch_is_recorded() {
    let w = work("contradict");
    let rs = all_reports();
    // A copy of VT-ABC's report moved to 05:16, before the jamming: its receiver is clean.
    let mut early = by_subject(&rs, "VT-ABC").clone();
    early.id = "rep-early000000".into();
    early.occurred_start_utc = Some("2026-11-12T05:16:00Z".into());
    early.time_to_recover_min = Some(4.0);
    early.kind = Kind::Jamming;
    // The vessel's GNSS spoofing entry, re-labelled jamming by the crew.
    let mut wrong_kind = norm("ship_log.txt")[0].clone();
    wrong_kind.id = "rep-wrongkind00".into();
    wrong_kind.kind = Kind::Jamming;
    wrong_kind.reported_type = ReportedType::Jamming;
    let doc = ireports::ReportsDoc::new(vec![early, wrong_kind]);
    std::fs::write(w.join("two.json"), ireports::json_text(&doc)).unwrap();
    let req = std::fs::read_to_string(w.join("verify.toml"))
        .unwrap()
        .replace("reports = \"reports.json\"", "reports = \"two.json\"");
    std::fs::write(
        w.join("two.toml"),
        format!("{req}\n[params]\nwindow_pad_min = 2\n"),
    )
    .unwrap();
    let (d, _) = verify::run(&w.join("two.toml")).unwrap();
    let e = record(&d, "rep-early000000");
    assert_eq!(e.label, Label::Contradicted, "{:?}", e.reasons);
    let k = record(&d, "rep-wrongkind00");
    assert_eq!(k.label, Label::Corroborated);
    assert_eq!(
        k.kind_agreement,
        Some(false),
        "data shows spoofing during the jump, the report says jamming"
    );
    assert!(k
        .reasons
        .iter()
        .any(|r| r.contains("not the reported jamming")));
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn verification_with_no_data_is_uncorroborated() {
    let r = norm("dgca_forms.txt")[0].clone();
    let obs = verify::Observations {
        aliases: vec![],
        receivers: vec![],
        adsb: vec![],
        ais: vec![],
    };
    let rec = verify::verify_report(&r, &obs, &verify::Params::default());
    assert_eq!(rec.label, Label::Uncorroborated);
    assert!(rec
        .reasons
        .iter()
        .any(|r| r.contains("no observation data")));
}

// ------------------------------------------------------------------------------------------
// Reset check and ledger

#[test]
fn reset_check_flags_a_receiver_still_off_after_spoofing() {
    let w = work("reset");
    let (abc, copies) = ledger::run_reset(&w.join("reset-abc.toml")).unwrap();
    assert_eq!(abc.outcome, "RESET RECOMMENDED", "{:?}", abc.reasons);
    assert_eq!(abc.persisting_at_end, Some(true));
    assert!(abc.median_disagreement_m.unwrap() > 2000.0);
    assert!(abc.evidence.starts_with("MODELLED"));
    assert!(abc.statement.contains("recommendation only"));
    let (xyz, _) = ledger::run_reset(&w.join("reset-xyz.toml")).unwrap();
    assert_eq!(xyz.outcome, "NO RESET INDICATED", "{:?}", xyz.reasons);
    let out = w.join("reset-abc");
    ledger::write_reset(&out, &abc, &copies).unwrap();
    kshana_pro::signed_dossier::sign(&out, &SEED, "Ops", UTC).unwrap();
    assert!(
        kshana_pro::signed_dossier::verify(&out, &[license::public_key_for(&SEED)])
            .unwrap()
            .passed()
    );
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn reset_check_needs_samples_and_one_reference() {
    let w = work("reset2");
    let t = std::fs::read_to_string(w.join("reset-abc.toml")).unwrap();
    std::fs::write(
        w.join("both.toml"),
        format!("reference_position = [31.7, 74.8]\n{t}"),
    )
    .unwrap();
    assert!(ledger::run_reset(&w.join("both.toml"))
        .unwrap_err()
        .contains("exactly one"));
    std::fs::write(
        w.join("late.toml"),
        t.replace("2026-11-12T06:00:00Z", "2026-11-12T06:39:00Z"),
    )
    .unwrap();
    let (c, _) = ledger::run_reset(&w.join("late.toml")).unwrap();
    assert_eq!(c.outcome, "NOT ASSESSABLE");
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn ledger_counts_exposure_per_tail_and_vessel() {
    let w = work("ledger");
    let key = license::public_key_for(&SEED);
    let (l, copies) = ledger::run_ledger(&w.join("ledger.toml"), &[key]).unwrap();
    assert!(l.refused.is_empty(), "{:?}", l.refused);
    let e = |id: &str| {
        l.entries
            .iter()
            .find(|e| e.subject_id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let abc = e("VT-ABC");
    // Report 05:42-06:00 plus ADS-B integrity (jamming-like) runs and two jumps, merged.
    assert!(
        abc.exposure_minutes >= 18.0 && abc.exposure_minutes <= 25.0,
        "{}",
        abc.exposure_minutes
    );
    assert_eq!(abc.spoofing_events, 1);
    assert_eq!(
        abc.spoofing_events_since_reset, 1,
        "reset on 1 Nov was before the episode"
    );
    assert_eq!(abc.flag, "review");
    assert_eq!(abc.last_reset_utc.as_deref(), Some("2026-11-01T10:00:00Z"));
    let xyz = e("VT-XYZ");
    assert_eq!(xyz.spoofing_events, 0);
    assert_eq!(
        xyz.exposure_minutes_since_reset, 0.0,
        "reset at 08:30 after the episode"
    );
    assert_eq!(xyz.flag, "none");
    let ship = e("9387421");
    assert_eq!(ship.platform, Platform::Vessel);
    assert_eq!(ship.spoofing_events, 2);
    assert_eq!(ship.flag, "review");
    assert_eq!(l.entries[0].flag, "review", "flagged subjects first");
    let out = w.join("ledger-out");
    ledger::write_ledger(&out, &l, &copies).unwrap();
    let csv = std::fs::read_to_string(out.join("ledger.csv")).unwrap();
    assert!(csv.lines().any(|r| r.starts_with("VT-ABC,aircraft,")));
    // An untrusted signer's episodes are refused.
    let (l2, _) = ledger::run_ledger(
        &w.join("ledger.toml"),
        &[license::public_key_for(&[9u8; 32])],
    )
    .unwrap();
    assert_eq!(l2.refused.len(), 1);
    assert!(l2.entries.is_empty());
    let _ = std::fs::remove_dir_all(&w);
}

// ------------------------------------------------------------------------------------------
// Licence and command line

fn ent(tier: Tier) -> Entitlement {
    Entitlement {
        licensee: "ACME".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

#[test]
fn features_have_explicit_tiers() {
    assert_eq!(
        license::explicit_tier(ireports::FEATURE_REPORTS),
        Some(Tier::Pro)
    );
    assert_eq!(
        license::explicit_tier(ireports::FEATURE_VERIFICATION),
        Some(Tier::Pro)
    );
    assert_eq!(
        license::explicit_tier(ireports::FEATURE_LEDGER),
        Some(Tier::Enterprise)
    );
    for f in [
        ireports::FEATURE_REPORTS,
        ireports::FEATURE_VERIFICATION,
        ireports::FEATURE_LEDGER,
    ] {
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
        assert!(license::check(&ent(Tier::Evaluation), f, 0).is_err());
        assert!(license::check(&ent(Tier::Enterprise), f, 0).is_ok());
    }
    assert!(license::check(&ent(Tier::Pro), ireports::FEATURE_REPORTS, 0).is_ok());
    assert!(license::check(&ent(Tier::Pro), ireports::FEATURE_LEDGER, 0).is_err());
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env_remove("KSHANA_PRO_LICENSE")
        .output()
        .unwrap()
}

#[test]
fn command_line_end_to_end() {
    let w = work("cli");
    let key = w.join("issuer.key");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let files = [
        "dgca_forms.txt",
        "easa_ecr.txt",
        "asrs_records.txt",
        "notams.txt",
        "ship_log.txt",
        "finnmark_export.csv",
    ]
    .iter()
    .map(|f| data(f).display().to_string())
    .collect::<Vec<_>>()
    .join(",");
    let rj = w.join("cli-reports.json");
    let s = |p: &Path| p.display().to_string();

    // Without a licence the Pro commands are refused (evaluation tier).
    let o = cli(&["reports", "normalise", &files, "--out", &s(&rj)]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("pro-interference-reports"));

    let o = cli(&[
        "reports",
        "normalise",
        &files,
        "--out",
        &s(&rj),
        "--regime",
        "generic",
        "--dev",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let doc = ireports::ReportsDoc::from_json(&std::fs::read_to_string(&rj).unwrap()).unwrap();
    assert_eq!(doc.reports.len(), 13);
    assert_eq!(doc.validation.len(), 13);

    let o = cli(&[
        "reports",
        "validate",
        &s(&data("dgca_forms.txt")),
        "--regime",
        "dgca",
        "--strict",
        "--dev",
    ]);
    assert_eq!(o.status.code(), Some(3));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("INCOMPLETE") && out.contains("10 minutes"));

    let ep = w.join("cli-episodes");
    let o = cli(&[
        "forensics",
        "reports",
        "episodes",
        &s(&rj),
        "--adsb",
        &s(&w.join("adsb_punjab.csv")),
        "--out",
        &s(&ep),
        "--signer",
        "Ops",
        "--key-file",
        &s(&key),
        "--utc",
        UTC,
        "--dev",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("mixed"));
    for f in [
        "episodes.json",
        "episodes.csv",
        "events.json",
        "signature.json",
        "inputs/adsb_punjab.csv",
    ] {
        assert!(ep.join(f).exists(), "{f}");
    }
    let pk = hex::encode(license::public_key_for(&SEED));
    let o = cli(&["reports", "check", &s(&ep), "--trust", &pk]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));

    let o = cli(&[
        "reports",
        "verify",
        &s(&w.join("verify.toml")),
        "--out",
        &s(&w.join("cli-verify")),
        "--signer",
        "Ops",
        "--key-file",
        &s(&key),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("CONTRADICTED") && out.contains("CORROBORATED") && out.contains("ais-only")
    );

    let o = cli(&[
        "reports",
        "reset-check",
        &s(&w.join("reset-abc.toml")),
        "--out",
        &s(&w.join("cli-reset")),
        "--signer",
        "Ops",
        "--key-file",
        &s(&key),
        "--dev",
    ]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("RESET RECOMMENDED"));

    let o = cli(&[
        "reports",
        "ledger",
        &s(&w.join("ledger.toml")),
        "--out",
        &s(&w.join("cli-ledger")),
        "--signer",
        "Ops",
        "--key-file",
        &s(&key),
        "--trust",
        &pk,
        "--dev",
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("VT-ABC (aircraft)"));

    let o = cli(&["reports", "params"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("quiet_gap_min = 15.0"));
    let o = cli(&["reports", "regimes"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("dgca"));
    let _ = std::fs::remove_dir_all(&w);
}
