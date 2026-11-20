// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! PNT dependency audit (`kshana_pro::dependency`): a ship and a port inventory audited
//! through the CLI from TOML, signed and verified; GraphML and JSON graph exports; the
//! questionnaire kit; the national register with pseudonyms, small-group suppression and a
//! tampered audit rejected; the licence gates. Evidence: MODELLED (self-made inventories).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::dependency::{self, register, templates, Inventory};
use kshana_pro::license::{self, Tier};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-deps-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SEED: [u8; 32] = [3u8; 32];

fn cli(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
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

fn ship(name: &str, with_eloran: bool) -> String {
    let eloran = if with_eloran {
        "\n[[node]]\nid = \"eloran\"\ntype = \"eloran\"\nprovides = [\"position\", \"time\"]\n"
    } else {
        ""
    };
    let fallback = if with_eloran {
        "[\"eloran\", \"dr\"]"
    } else {
        "[\"dr\"]"
    };
    format!(
        r#"
format = "kshana-pro-pnt-inventory/1"
asset = "{name}"
operator = "Operator {name}"
sector = "ship"
required_autonomy_s = 3600
[[node]]
id = "gnss1"
type = "gnss"
provides = ["position", "velocity", "time"]
[[node]]
id = "gnss2"
type = "gnss"
provides = ["position", "velocity", "time"]
[[node]]
id = "gyro"
type = "gyro"
provides = ["heading"]
[[node]]
id = "log"
type = "log"
provides = ["velocity"]
[[node]]
id = "radar"
type = "system"
critical = true
isolation_switch = true
provides = ["position"]
[[node.needs]]
pnt = "heading"
from = ["gyro"]
{eloran}
[[node]]
id = "dr"
type = "system"
provides = ["position"]
[[node.needs]]
pnt = "heading"
from = ["gyro"]
[[node.needs]]
pnt = "velocity"
from = ["log"]
[[node]]
id = "ecdis"
name = "ECDIS"
type = "system"
critical = true
isolation_switch = true
[[node.needs]]
pnt = "position"
from = ["gnss1", "gnss2"]
fallback = {fallback}
cross_check = ["radar"]
[[node]]
id = "ais"
type = "system"
critical = true
[[node.needs]]
pnt = "position"
from = ["gnss1"]
[[node.needs]]
pnt = "time"
from = ["gnss1"]
"#
    )
}

const PORT: &str = r#"
format = "kshana-pro-pnt-inventory/1"
asset = "Terminal 1"
operator = "Port Operator"
sector = "port"
[[node]]
id = "rtk"
name = "RTK base and rovers"
type = "gnss"
provides = ["position"]
[[node]]
id = "gm"
name = "PTP grandmaster"
type = "gnss"
provides = ["time"]
holdover_s = 7200
[[node]]
id = "network-time"
type = "system"
critical = true
provides = ["time"]
[[node.needs]]
pnt = "time"
from = ["gm"]
[[node]]
id = "cranes"
type = "system"
critical = true
[[node.needs]]
pnt = "position"
from = ["rtk"]
"#;

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn audit_from_toml_finds_the_dependencies_and_exports_the_graph() {
    let src = ship("MV A", false);
    let inv = Inventory::parse(&src).unwrap();
    let a = dependency::audit(&inv, &src).unwrap();
    let cls = |id: &str| {
        a.denial
            .iter()
            .find(|r| r.node == id)
            .unwrap()
            .class
            .clone()
    };
    assert_eq!(cls("ecdis"), "gnss-with-fallback");
    assert_eq!(cls("ais"), "gnss-dependent");
    assert_eq!(cls("radar"), "gnss-independent");
    assert_eq!(a.status.level, "L1");
    // With eLoran as the first fallback the ECDIS keeps a radio position (degraded).
    let src2 = ship("MV B", true);
    let b = dependency::audit(&Inventory::parse(&src2).unwrap(), &src2).unwrap();
    assert_eq!(
        b.denial.iter().find(|r| r.node == "ecdis").unwrap().class,
        "gnss-with-fallback"
    );
    // The port: the network time runs on the grandmaster's holdover for two hours.
    let p = dependency::audit(&Inventory::parse(PORT).unwrap(), PORT).unwrap();
    let nt = p.denial.iter().find(|r| r.node == "network-time").unwrap();
    assert_eq!(nt.holdover_s, Some(7200.0));
    assert_eq!(
        p.status.min_autonomy_s,
        Some(0.0),
        "the cranes stop at once"
    );
    assert!(p.template_missing.contains(&"vts".to_string()));
    let g = dependency::graph_json(&inv);
    assert!(g["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "cross-check" && e["from"] == "radar"));
    assert!(dependency::graphml(&inv).starts_with("<?xml"));
}

#[test]
fn every_sector_has_a_template_with_a_starter() {
    for s in [
        "ship",
        "port",
        "aircraft",
        "water",
        "pipeline",
        "emergency-calls",
        "tetra",
        "rescue",
    ] {
        let st = templates::starter(s).unwrap();
        assert!(Inventory::parse(&st).is_ok(), "{s}");
    }
}

#[test]
fn features_are_registered_with_their_tiers() {
    assert_eq!(
        license::explicit_tier(dependency::FEATURE_AUDIT),
        Some(Tier::Pro)
    );
    assert_eq!(
        license::explicit_tier(dependency::FEATURE_REGISTER),
        Some(Tier::Enterprise)
    );
    assert!(license::pro_command_features().contains(&dependency::FEATURE_REGISTER));
    let mut pro = license::Entitlement::evaluation();
    pro.tier = Tier::Pro;
    assert!(license::check(&pro, dependency::FEATURE_AUDIT, 0).is_ok());
    assert!(license::check(&pro, dependency::FEATURE_REGISTER, 0).is_err());
}

#[test]
fn cli_audit_questionnaire_and_register() {
    let d = tmp("cli");
    let kf = write(&d, "sign.key", &hex::encode(SEED));
    let pk = hex::encode(license::public_key_for(&SEED));
    let pkf = write(&d, "pseudo.key", &hex::encode([1u8; 32]));
    let (ok, out) = cli(&["dependency", "templates"]);
    assert!(ok && out.contains("emergency-calls"), "{out}");
    let (ok, out) = cli(&["dependency", "template", "tetra"]);
    assert!(ok && out.contains("base-stations"), "{out}");

    let mut audits = Vec::new();
    for (i, text) in [
        ship("MV A", false),
        ship("MV B", true),
        ship("MV C", false),
        PORT.into(),
    ]
    .iter()
    .enumerate()
    {
        let inv = write(&d, &format!("inv{i}.toml"), text);
        let out_dir = d.join(format!("audit{i}"));
        let o = out_dir.to_string_lossy().into_owned();
        // Gated: the evaluation tier cannot audit.
        if i == 0 {
            let (ok, out) = cli(&[
                "dependency",
                "audit",
                &inv,
                "--out",
                &o,
                "--signer",
                "Op",
                "--key-file",
                &kf,
            ]);
            assert!(!ok && out.contains("requires"), "{out}");
        }
        let (ok, out) = cli(&[
            "dependency",
            "audit",
            &inv,
            "--out",
            &o,
            "--signer",
            "Op",
            "--key-file",
            &kf,
            "--utc",
            "2026-11-20T10:00:00Z",
            "--dev",
        ]);
        assert!(ok, "{out}");
        for f in [
            "audit.json",
            "findings.csv",
            "graph.graphml",
            "graph.json",
            "signature.json",
        ] {
            assert!(out_dir.join(f).exists(), "{f}");
        }
        audits.push(o);
    }
    let (ok, out) = cli(&["dependency", "verify", &audits[0], "--trust", &pk]);
    assert!(ok, "{out}");
    let (ok, out) = cli(&[
        "dependency",
        "questionnaire",
        &audits[0],
        "--trust",
        &pk,
        "--json",
        "--dev",
    ]);
    assert!(ok, "{out}");
    let q: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(q["answers"].as_array().unwrap().len(), 7);
    assert!(q["answers"][0]["answer"].as_str().unwrap().contains("ais"));

    // A fifth audit, tampered after signing: rejected by the register.
    let bad = d.join("audit-bad");
    std::fs::create_dir_all(&bad).unwrap();
    for f in std::fs::read_dir(&audits[1]).unwrap() {
        let f = f.unwrap().path();
        std::fs::copy(&f, bad.join(f.file_name().unwrap())).unwrap();
    }
    let a = std::fs::read_to_string(bad.join("audit.json")).unwrap();
    std::fs::write(bad.join("audit.json"), a.replace("\"L1\"", "\"L3\"")).unwrap();
    let list = format!("{},{}", audits.join(","), bad.to_string_lossy());
    let reg = d.join("register");
    let rs = reg.to_string_lossy().into_owned();
    let base = [
        "dependency",
        "register",
        list.as_str(),
        "--pseudonym-key-file",
        pkf.as_str(),
        "--out",
        rs.as_str(),
        "--signer",
        "Office",
        "--key-file",
        kf.as_str(),
        "--trust",
        pk.as_str(),
        "--utc",
        "2026-11-20T10:00:00Z",
    ];
    // Enterprise: the evaluation tier (no licence) is refused.
    let (ok, out) = cli(&base);
    assert!(!ok && out.contains("enterprise"), "{out}");
    let mut with_dev: Vec<&str> = base.to_vec();
    with_dev.push("--dev");
    let (ok, out) = cli(&with_dev);
    assert!(ok, "{out}");
    let r: register::Register =
        serde_json::from_str(&std::fs::read_to_string(reg.join("register.json")).unwrap()).unwrap();
    assert_eq!(r.entries.len(), 4);
    assert_eq!(r.rejected.len(), 1);
    let ship_agg = r.sectors.iter().find(|s| s.sector == "ship").unwrap();
    assert!(!ship_agg.suppressed);
    assert_eq!(ship_agg.operators, 3);
    assert_eq!(ship_agg.levels["L1"], 3);
    let port = r.sectors.iter().find(|s| s.sector == "port").unwrap();
    assert!(port.suppressed && port.levels.is_empty());
    // No operator or asset name and no node id leaves the audits.
    let text = std::fs::read_to_string(reg.join("register.json")).unwrap();
    for leak in ["Operator MV", "MV A", "Terminal 1", "gnss1", "ecdis"] {
        assert!(!text.contains(leak), "{leak} in the register");
    }
    let (ok, out) = cli(&["dependency", "verify", &rs, "--trust", &pk]);
    assert!(ok, "{out}");
}
