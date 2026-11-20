// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Regulator, type-approval and framework pre-assessment packs (E29, B2901 to B2906).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{self, Tier};
use kshana_pro::regpacks::{self, bam, bid, complaint, links, mass};
use kshana_pro::signed_dossier;

const SEED: [u8; 32] = [7u8; 32];
const OTHER: [u8; 32] = [9u8; 32];
const UTC: &str = "2026-11-20T09:00:00Z";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-regpack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ex(name: &str) -> &'static str {
    regpacks::example(name).unwrap()
}

fn signed_bam(dir: &Path, vector: &str, seed: &[u8; 32]) {
    let m = bam::AlertMap::parse(ex("bam")).unwrap();
    let v = kshana_pro::pntdp::vectors::all()
        .into_iter()
        .find(|v| v.name == vector)
        .unwrap();
    let cfg = kshana_pro::pntdp::Config::parse(&v.config).unwrap();
    let out = kshana_pro::pntdp::run(&cfg, &v.samples).unwrap();
    let b = bam::run(&m, ex("bam"), &bam::epochs_from(&out), vector);
    bam::write_dir(dir, &b).unwrap();
    signed_dossier::sign(dir, seed, "lab", UTC).unwrap();
}

#[test]
fn catalogues_are_well_formed_and_cite_with_a_basis() {
    let cs = regpacks::catalogues().unwrap();
    assert_eq!(cs.len(), regpacks::CATALOGUES.len());
    assert!(cs.len() >= 8);
    for (c, (id, _)) in cs.iter().zip(regpacks::CATALOGUES) {
        assert_eq!(&c.id, id);
        assert!(c.problems().is_empty(), "{:?}", c.problems());
        assert!(!c.source.read.is_empty() && !c.source.version.is_empty());
        // Every row either names evidence or is honestly not assessed, with a gap either way.
        for r in &c.rows {
            assert!(!r.gap.is_empty());
            if r.clause != regpacks::CLAUSE_UNCONFIRMED {
                assert!(
                    r.basis.len() > 10,
                    "{} {}: a cited clause needs a basis",
                    c.id,
                    r.id
                );
            }
        }
        // Every catalogue says what Kshana does not cover.
        assert!(
            c.rows.iter().any(|r| r.evidence.is_empty()),
            "{}: no row marks what Kshana does not assess",
            c.id
        );
    }
    let ids: Vec<&str> = cs.iter().map(|c| c.id.as_str()).collect();
    for want in [
        "imo-msc401-432",
        "imo-circ-1575",
        "bam-pnt-alerts",
        "iacs-ur-e27",
        "etsi-en-303-413",
        "imo-mass-code",
        "nist-csf2-pnt",
        "eu-space-act-cyber",
    ] {
        assert!(ids.contains(&want), "{want}");
    }
    // Circ.1575 paragraphs match the open engine's reading.
    let c = regpacks::catalogue("imo-circ-1575").unwrap();
    assert!(c
        .rows
        .iter()
        .any(|r| r.clause == "annex paragraphs 53 to 57"));
    assert!(regpacks::catalogue("nope").is_err());
}

#[test]
fn wording_never_claims_a_regulation_is_met() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = vec![
        root.join("src/cli/regpack_cmd.rs"),
        root.join("docs/REGULATOR-PACKS.md"),
    ];
    for d in [
        "src/regpacks",
        "report-templates/regpacks",
        "report-templates/regpacks/examples",
    ] {
        for e in std::fs::read_dir(root.join(d)).unwrap() {
            let p = e.unwrap().path();
            if p.is_file() {
                files.push(p);
            }
        }
    }
    for p in files {
        let t = std::fs::read_to_string(&p).unwrap().to_ascii_lowercase();
        for w in ["complian", "conform", "certified"] {
            assert!(!t.contains(w), "{}: contains '{w}'", p.display());
        }
    }
    // The CHANGELOG entry too.
    let ch = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
    let entry = ch
        .split("\n## ")
        .find(|s| s.contains("regpack"))
        .expect("a CHANGELOG entry names regpack");
    let part: String = entry
        .lines()
        .skip_while(|l| !l.contains("regpack"))
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    for w in ["complian", "conform", "certified"] {
        assert!(!part.contains(w), "CHANGELOG entry contains '{w}'");
    }
}

#[test]
fn pre_assessment_uses_only_verified_evidence_and_lists_the_gaps() {
    let d = tmp("assess");
    let good = d.join("good");
    let bad = d.join("bad");
    let other = d.join("other");
    signed_bam(&good, "jump", &SEED);
    signed_bam(&bad, "nominal", &SEED);
    signed_bam(&other, "frozen", &OTHER);
    // Tamper with one signed directory after signing.
    let p = bad.join(bam::RESULT_FILE);
    let t = std::fs::read_to_string(&p).unwrap().replace("3061", "9999");
    std::fs::write(&p, t).unwrap();
    let trust = [license::public_key_for(&SEED)];
    let ev: Vec<_> = [&good, &bad, &other]
        .iter()
        .map(|x| regpacks::read_evidence(x, &trust).unwrap())
        .collect();
    assert!(ev[0].usable());
    assert!(!ev[1].usable() && !ev[1].problems.is_empty());
    assert_eq!(ev[2].signer_trusted, Some(false));
    assert!(!ev[2].usable());
    assert_eq!(ev[0].formats, vec![bam::FORMAT.to_string()]);

    // An open-engine standards-pack run as unsigned, hashed evidence.
    let rep = kshana::standards_pack::run("imo-circ-1575").unwrap();
    let rp = d.join("std.json");
    std::fs::write(&rp, serde_json::to_string(&rep).unwrap()).unwrap();
    let sr = regpacks::read_standards_report(&rp).unwrap();
    assert!(sr.usable() && sr.signer.is_none() && sr.sha256.len() == 64);
    let not_report = d.join("x.json");
    std::fs::write(&not_report, "{\"schema\":\"other\"}").unwrap();
    assert!(regpacks::read_standards_report(&not_report).is_err());

    let c = regpacks::catalogue("imo-circ-1575").unwrap();
    let subject = regpacks::Subject::parse(ex("subject")).unwrap();
    let mut all = ev.clone();
    all.push(sr);
    let p = regpacks::assess(&c, subject, all);
    assert_eq!(p.format, regpacks::FORMAT);
    assert_eq!(p.counts.iter().sum::<usize>(), c.rows.len());
    let row = |id: &str| p.rows.iter().find(|r| r.id == id).unwrap();
    assert_eq!(row("PNTDP-STATUS").status, regpacks::SUPPLIED);
    assert_eq!(row("PNTDP-STATUS").evidence.len(), 1, "only the good dir");
    assert!(row("PNTDP-STATUS").evidence[0].contains("good"));
    assert_eq!(row("PNTDP-ACC").status, regpacks::SUPPLIED);
    assert_eq!(row("PNTDP-DOC").status, regpacks::NOT_ASSESSED);
    assert!(row("PNTDP-ACC")
        .missing
        .contains(&"Test-kit build verdicts".to_string()));
    assert_eq!(p.catalogue_sha256.len(), 64);
    let out = d.join("pack");
    regpacks::write_dir(&out, &p).unwrap();
    let html = std::fs::read_to_string(out.join("report.html")).unwrap();
    assert!(html.contains("Evidence register") && html.contains("signature INVALID"));
    assert!(html.contains("Gaps (rows without supplied evidence)"));

    // Nothing supplied: every row with evidence named is "no evidence supplied".
    let empty = regpacks::assess(
        &regpacks::catalogue("imo-msc401-432").unwrap(),
        Default::default(),
        vec![],
    );
    assert_eq!(empty.counts[0], 0);
    assert_eq!(empty.counts[2], 1);
    // A directory with no Kshana format is refused.
    let none = d.join("none");
    std::fs::create_dir_all(&none).unwrap();
    std::fs::write(none.join("a.json"), "{\"format\":\"something/1\"}").unwrap();
    assert!(regpacks::read_evidence(&none, &trust).is_err());
}

#[test]
fn bam_map_rules_and_replay() {
    let m = bam::AlertMap::parse(ex("bam")).unwrap();
    assert!(bam::check_map(&m).is_empty(), "{:?}", bam::check_map(&m));

    let broken = r#"
equipment = "broken"
[[alert]]
condition = "integrity:none"
alert_id = "1"
priority = "caution"
category = "A"
text = "x"
[[alert]]
condition = "flag:jump"
alert_id = "1"
priority = "alarm"
category = "D"
[[alert]]
condition = "flag:teleport"
alert_id = ""
priority = "emergency-alarm"
category = "B"
"#;
    let b = bam::AlertMap::parse(broken).unwrap();
    let f = bam::check_map(&b);
    let rules: Vec<&str> = f.iter().map(|x| x.rule).collect();
    for r in ["MAP-0", "MAP-1", "MAP-2", "MAP-3", "MAP-4", "MAP-5"] {
        assert!(rules.contains(&r), "{r}: {f:?}");
    }
    assert!(f
        .iter()
        .any(|x| x.rule == "MAP-2" && x.text.contains("emergency-alarm")));
    assert!(f
        .iter()
        .any(|x| x.rule == "MAP-3" && x.text.contains("different priorities")));

    // Replay over the jump vector: the jump is flagged and reaches the bridge as 3072.
    let v = kshana_pro::pntdp::vectors::all()
        .into_iter()
        .find(|v| v.name == "jump")
        .unwrap();
    let cfg = kshana_pro::pntdp::Config::parse(&v.config).unwrap();
    let out = kshana_pro::pntdp::run(&cfg, &v.samples).unwrap();
    let ep = bam::epochs_from(&out);
    let r = bam::run(&m, ex("bam"), &ep, "jump");
    assert_eq!(r.epochs, out.len());
    let jump = r
        .conditions
        .iter()
        .find(|c| c.condition == "flag:jump")
        .expect("jump flagged");
    assert!(jump.onsets >= 1 && jump.alert.starts_with("3072"));
    assert!(r
        .timeline
        .iter()
        .any(|e| e.condition == "flag:jump" && e.event == "raised"));
    // Without the jump mapping its onsets are counted as unmapped and the map has a gap.
    let mut m2 = m.clone();
    m2.alerts.retain(|a| a.condition != "flag:jump");
    let r2 = bam::run(&m2, "m2", &ep, "jump");
    assert!(r2.unmapped_onsets >= jump.onsets);
    assert!(r2.findings.iter().any(|x| x.rule == "MAP-1"));
    // The JSON route reads the same epochs as the processor's own output.
    let json = serde_json::to_string(&serde_json::json!({ "epochs": out })).unwrap();
    assert_eq!(bam::parse_epochs(&json).unwrap(), ep);
    assert!(bam::parse_epochs("{}").is_err());
}

#[test]
fn mass_override_chains_to_gnss_and_controlled_systems() {
    let d = mass::MassDecl::parse(ex("mass")).unwrap();
    let r = mass::run(&d, ex("mass"));
    assert!(!r.override_gnss_independent);
    assert!(r.override_independent_of_controlled);
    assert_eq!(
        r.chains[0].path,
        vec!["roc-override", "sat-link", "time-server", "gnss-a"]
    );
    assert_eq!(r.oe_without_evidence, vec!["OE-7".to_string()]);

    // Fix the time source: independent.
    let fixed = ex("mass").replace("depends_on = [\"gnss-a\"]\nnotes", "depends_on = []\nnotes");
    let r = mass::run(&mass::MassDecl::parse(&fixed).unwrap(), &fixed);
    assert!(r.override_gnss_independent && r.chains.is_empty());

    // An override that routes through the autopilot reaches a controlled system and GNSS.
    let through = fixed.replace(
        "id = \"override-controller\"\nkind = \"controller\"\ndepends_on = [\"gyro\"]",
        "id = \"override-controller\"\nkind = \"controller\"\ndepends_on = [\"autopilot\"]",
    );
    let r = mass::run(&mass::MassDecl::parse(&through).unwrap(), &through);
    assert!(!r.override_independent_of_controlled && !r.override_gnss_independent);
    assert!(r
        .chains
        .iter()
        .any(|c| c.reaches == "controlled system" && c.path.last().unwrap() == "autopilot"));

    // Declarations are checked.
    assert!(mass::MassDecl::parse(&ex("mass").replace("\"gyro\"]", "\"ghost\"]")).is_err());
    assert!(
        mass::MassDecl::parse(&ex("mass").replace("kind = \"link\"", "kind = \"warp\"")).is_err()
    );
    let cyc = fixed.replace(
        "id = \"gyro\"\nkind = \"sensor\"",
        "id = \"gyro\"\nkind = \"sensor\"\ndepends_on = [\"override-controller\"]",
    );
    let r = mass::run(&mass::MassDecl::parse(&cyc).unwrap(), &cyc);
    assert!(r.override_gnss_independent, "a cycle terminates");
}

#[test]
fn complaint_reports_effects_by_area_never_positions() {
    let src = ex("complaint");
    let r = complaint::Request::parse(src).unwrap();
    assert_eq!(complaint::cell_of(59.43, 24.77, 0.5), (59.0, 24.5));
    assert_eq!(complaint::cell_of(-0.1, -0.1, 0.5), (-0.5, -0.5));
    assert_eq!(complaint::cell_of(90.0, 180.0, 0.5), (89.5, 179.5));

    // Episodes from the interference-report module's format.
    let episodes = serde_json::json!({
        "format": "kshana-pro-interference-episodes/1",
        "episodes": [{
            "id": "EP-0001", "start_utc": "2026-04-03T10:00:00Z", "end_utc": "2026-04-03T11:30:00Z",
            "centroid_lat_deg": 57.123456, "centroid_lon_deg": 21.987654, "radius_km": 12.3,
            "kind": "jamming", "observers": 4, "adsb_aircraft": ["ABC123"]
        }]
    })
    .to_string();
    let eo = complaint::observations_from_episodes(&episodes).unwrap();
    assert_eq!(eo.len(), 1);
    assert!(complaint::observations_from_episodes("{\"format\":\"x\"}").is_err());

    let d = tmp("complaint");
    let ev_dir = d.join("ev");
    signed_bam(&ev_dir, "nominal", &SEED);
    let ev = vec![regpacks::read_evidence(&ev_dir, &[]).unwrap()];
    let c = complaint::build(&r, src, Some((&episodes, eo)), ev).unwrap();
    assert_eq!(c.areas.len(), 3);
    assert_eq!(c.period_utc[0], "2026-04-02T06:10:00Z");
    assert_eq!(c.period_utc[1], "2026-04-03T11:30:00Z");
    assert!(c.episodes_sha256.is_some());
    assert_eq!(c.interfering_station.location, complaint::NOT_DETERMINED);
    assert!(c.interfering_station.nature_of_effects.contains("jamming"));
    assert!(c.interfering_station.nature_of_effects.contains("spoofing"));
    for a in &c.areas {
        assert_eq!((a.south_deg / 0.5).fract(), 0.0);
        assert_eq!((a.west_deg / 0.5).fract(), 0.0);
    }
    let out = d.join("pack");
    complaint::write_dir(&out, &c).unwrap();
    for f in [complaint::RESULT_FILE, "report.html"] {
        let t = std::fs::read_to_string(out.join(f)).unwrap();
        for precise in ["59.43", "24.77", "59.71", "25.02", "57.12", "21.98", "12.3"] {
            assert!(!t.contains(precise), "{f} leaks {precise}");
        }
    }

    // Finer cells and bad inputs are refused.
    assert!(complaint::Request::parse(&src.replace("cell_deg = 0.5", "cell_deg = 0.1")).is_err());
    assert!(
        complaint::Request::parse(&src.replace("kind = \"jamming\"", "kind = \"loud\"")).is_err()
    );
    let coarse =
        complaint::Request::parse(&src.replace("cell_deg = 0.5", "cell_deg = 2.0")).unwrap();
    let c1 = complaint::build(&coarse, src, None, vec![]).unwrap();
    assert_eq!(c1.areas.len(), 1);
    let empty = r#"
reporting_administration = "x"
rr_edition = "x"
action_requested = "x"
[victim]
service = "RNSS"
stations = "x"
frequencies_mhz = [1575.42]
"#;
    let e = complaint::Request::parse(empty).unwrap();
    assert!(complaint::build(&e, empty, None, vec![]).is_err());
}

#[test]
fn link_security_findings_and_policy() {
    let d = links::LinkDecl::parse(ex("links")).unwrap();
    let r = links::run(&d, ex("links"));
    assert_eq!(r.cleartext_links, vec!["tm-s", "gs-network"]);
    assert_eq!(r.unauthenticated_command_paths, vec!["gs-network"]);
    assert_eq!(r.no_encryption_no_fly, "not met as declared");
    assert!(r
        .findings
        .iter()
        .any(|f| f.link == "gs-network" && f.rule == "LNK-1" && f.severity == "gap"));
    assert!(r
        .findings
        .iter()
        .any(|f| f.link == "payload-x" && f.rule == "LNK-3" && f.severity == "note"));
    assert!(!r.findings.iter().any(|f| f.link == "tc-s"));

    let only_tc = ex("links")
        .split("[[link]]\nid = \"tm-s\"")
        .next()
        .unwrap()
        .to_string();
    let r = links::run(&links::LinkDecl::parse(&only_tc).unwrap(), &only_tc);
    assert_eq!(r.no_encryption_no_fly, "met as declared");
    assert_eq!(r.gaps, 0);
    let disable = only_tc.replace(
        "crypto_can_be_disabled = false",
        "crypto_can_be_disabled = true",
    );
    let r = links::run(&links::LinkDecl::parse(&disable).unwrap(), &disable);
    assert_eq!(r.no_encryption_no_fly, "not met as declared");
    assert!(r
        .findings
        .iter()
        .any(|f| f.rule == "LNK-3" && f.severity == "gap"));
    assert!(links::LinkDecl::parse(&only_tc.replace("telecommand", "teleport")).is_err());
}

#[test]
fn bid_attestation_lists_declarations_and_its_chain_verifies() {
    let b = bid::Bid::parse(ex("bid")).unwrap();
    let a = bid::build(b.clone(), ex("bom"), vec![]).unwrap();
    assert_eq!(a.lines.len(), 4);
    assert!(a.itar_declared.is_empty() && a.not_declared.is_empty());
    assert!(a.summary.starts_with("None of the 4 lines"));
    assert_eq!(a.lines[3].description, "Enclosure, machined");
    assert_eq!(a.lines[1].status, "not listed (declared)");

    let bom2 = format!(
        "{}EX-005,Antenna,Example US Antennas (fictitious),US,US ITAR,USML XII,supplier\nEX-006,Cable,Unknown,,,,\n",
        ex("bom")
    );
    let a2 = bid::build(b.clone(), &bom2, vec![]).unwrap();
    assert_eq!(a2.itar_declared, vec!["EX-005"]);
    assert_eq!(a2.not_declared, vec!["EX-006"]);
    assert_eq!(a2.origin_missing, vec!["EX-006"]);
    assert!(a2.summary.starts_with("1 of 6 lines declare"));
    assert!(bid::parse_bom("a,b\n1,2\n").is_err());
    assert!(bid::parse_bom(&format!("{}\nX,only,three\n", bid::BOM_HEADER)).is_err());

    let d = tmp("bid");
    let ev_dir = d.join("ev");
    signed_bam(&ev_dir, "jump", &SEED);
    let ev = vec![regpacks::read_evidence(&ev_dir, &[license::public_key_for(&SEED)]).unwrap()];
    let a3 = bid::build(b, ex("bom"), ev).unwrap();
    assert_ne!(a3.chain_sha256, a.chain_sha256);
    let out = d.join("att");
    bid::write_dir(&out, &a3, ex("bom")).unwrap();
    signed_dossier::sign(&out, &SEED, "bidder", UTC).unwrap();
    assert!(bid::check_dir(&out).unwrap().is_empty());
    let v = signed_dossier::verify(&out, &[license::public_key_for(&SEED)]).unwrap();
    assert!(v.passed());
    // Change the BOM: the chain check and the signature both catch it.
    std::fs::write(out.join(bid::BOM_FILE), bom2).unwrap();
    assert!(!bid::check_dir(&out).unwrap().is_empty());
    assert!(!signed_dossier::verify(&out, &[])
        .unwrap()
        .problems
        .is_empty());
}

#[test]
fn features_have_explicit_pro_tiers() {
    for f in [
        regpacks::FEATURE,
        regpacks::FEATURE_COMPLAINT,
        regpacks::FEATURE_BID,
    ] {
        assert_eq!(license::explicit_tier(f), Some(Tier::Pro), "{f}");
        assert!(license::pro_command_features().contains(&f), "{f}");
    }
    // Refusal without a licence (evaluation tier) is exercised by `cli_end_to_end`.
}

fn bin(args: &[&str], home: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn cli_end_to_end() {
    let d = tmp("cli");
    let home = d.join("home");
    let key = d.join("key");
    std::fs::write(&key, hex::encode(SEED)).unwrap();
    let pk = hex::encode(license::public_key_for(&SEED));
    let s = |p: &Path| p.to_str().unwrap().to_string();
    let k = s(&key);
    let sign = ["--signer", "lab", "--key-file", k.as_str(), "--utc", UTC];

    let (ok, out) = bin(&["regpack", "list"], &home);
    assert!(
        ok && out.contains("imo-mass-code") && out.contains("nist-csf2-pnt"),
        "{out}"
    );
    let (ok, out) = bin(&["regpack", "show", "iacs-ur-e27"], &home);
    assert!(ok && out.contains("Appendix 2"), "{out}");
    let (ok, out) = bin(&["regpack", "example", "bom"], &home);
    assert!(ok && out.starts_with(bid::BOM_HEADER), "{out}");

    // Producing without a licence is refused (evaluation tier).
    let map = d.join("alert-map.toml");
    std::fs::write(&map, ex("bam")).unwrap();
    let bam_out = d.join("bam");
    let mut a = vec![
        "regpack",
        "bam",
        map.to_str().unwrap(),
        "--pntdp-vector",
        "frozen",
        "--out",
    ];
    let bo = s(&bam_out);
    a.push(&bo);
    a.extend(sign);
    let (ok, out) = bin(&a, &home);
    assert!(!ok, "{out}");
    a.push("--dev");
    let (ok, out) = bin(&a, &home);
    assert!(ok && out.contains("0 map gaps"), "{out}");

    let pa = s(&d.join("pa"));
    let mut a = vec![
        "regpack",
        "assess",
        "bam-pnt-alerts",
        "--evidence",
        bo.as_str(),
        "--trust",
        pk.as_str(),
        "--out",
        pa.as_str(),
        "--dev",
    ];
    a.extend(sign);
    let (ok, out) = bin(&a, &home);
    assert!(
        ok && out
            .contains("4 rows with evidence supplied, 0 with no evidence supplied, 1 not assessed"),
        "{out}"
    );
    let (ok, out) = bin(&["regpack", "verify", &pa, "--trust", &pk], &home);
    assert!(ok && out.contains("trusted"), "{out}");
    let (ok, _) = bin(&["regpack", "verify", &pa], &home);
    assert!(!ok, "an unpinned key does not pass");

    for (cmd, file, extra) in [
        ("mass", "mass", vec![]),
        ("links", "links", vec![]),
        ("complaint", "complaint", vec![]),
        ("bid", "bid", vec!["--bom"]),
    ] {
        let f = d.join(format!("{file}.toml"));
        std::fs::write(&f, ex(file)).unwrap();
        let bom = d.join("bom.csv");
        std::fs::write(&bom, ex("bom")).unwrap();
        let o = s(&d.join(cmd));
        let fs = s(&f);
        let bs = s(&bom);
        let mut a = vec!["regpack", cmd, fs.as_str(), "--out", o.as_str(), "--dev"];
        if !extra.is_empty() {
            a.push(extra[0]);
            a.push(bs.as_str());
        }
        a.extend(sign);
        let (ok, out) = bin(&a, &home);
        assert!(ok, "{cmd}: {out}");
        let (ok, out) = bin(&["regpack", "verify", &o, "--trust", &pk], &home);
        assert!(ok, "{cmd}: {out}");
    }
    // A tampered bid attestation fails verification.
    let bd = d.join("bid");
    std::fs::write(bd.join(bid::BOM_FILE), "changed").unwrap();
    let (ok, out) = bin(&["regpack", "verify", &s(&bd), "--trust", &pk], &home);
    assert!(!ok && out.contains("problem"), "{out}");
    let (ok, _) = bin(
        &["regpack", "assess", "unknown-id", "--out", &pa, "--dev"],
        &home,
    );
    assert!(!ok);
}
