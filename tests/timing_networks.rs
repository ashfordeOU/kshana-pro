// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Timing networks (E19): the appliance security posture (B1902), the time-distribution chain
//! verifier (B1904), the multi-source cross-check with non-GNSS sources (B1905) and the
//! ground-station pass-quality assurance (B1907), on small self-made fixtures.

use std::path::{Path, PathBuf};

use kshana_pro::timing::chain::{self as ch, AlphaCalibration, ChainSpec, FixedDelays};
use kshana_pro::timing::crosscheck::{self as cc, Spec};
use kshana_pro::timing::posture::{self as po, ComponentTable, Inventory, Reference};
use kshana_pro::timing::station::{self as st, StationSpec};
use kshana_pro::timing::timebomb::AdvisoryTable;

/// 2026-11-20T12:00:00Z.
const NOW: i64 = 1_795_176_000;
const UTC: &str = "2026-11-20T12:00:00Z";

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e19-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

// ================================================================ B1902 posture

const INVENTORY: &str = r#"
title = "Exchange grandmasters"
[[appliances]]
id = "gm-ldn-1"
vendor = "Microchip"
model = "TimeProvider 4100"
firmware = "2.4"
management = "internal"
services = ["https", "ssh", "snmpv2c"]

[[appliances]]
id = "gm-ldn-2"
vendor = "Microchip"
model = "TimeProvider 4100"
firmware = "2.5"
management = "isolated"
firmware_check = "gm-ldn-2.fwcheck.json"

[[appliances]]
id = "ntp-edge"
vendor = "Example Vendor"
model = "NTP-1"
firmware = "1.0"
management = "internet"
upgrade_signing = "device-verifies"
services = ["telnet", "ntp-mode6"]
sbom = "ntp-edge.cdx.json"
"#;

const VENDOR_SBOM: &str = r#"{
  "bomFormat": "CycloneDX", "specVersion": "1.5", "version": 1,
  "metadata": {"component": {"type": "firmware", "bom-ref": "fw", "name": "NTP-1 firmware", "version": "1.0"}},
  "components": [
    {"type": "application", "bom-ref": "ossh", "name": "OpenSSH", "version": "9.6p1"},
    {"type": "library", "bom-ref": "curl", "name": "curl", "version": "8.10.0"},
    {"type": "library", "bom-ref": "xml", "name": "libxml2", "version": "2.11.4",
     "components": [{"type": "library", "bom-ref": "zlib", "name": "zlib", "version": "1.2.13"}]}
  ],
  "dependencies": [{"ref": "fw", "dependsOn": ["ossh", "curl", "xml"]}]
}"#;

const COMPONENTS: &str = r#"
[[component_advisory]]
id = "EXAMPLE-OPENSSH-1"
component = "openssh"
fixed_in = "9.8p1"
hazard = "illustrative component advisory"
source = "test fixture"
verification = "example"

[[component_advisory]]
id = "EXAMPLE-CURL-1"
component = "curl"
fixed_in = "8.9.0"
hazard = "illustrative; the SBOM's curl is newer"
source = "test fixture"
verification = "example"

[[component_advisory]]
id = "EXAMPLE-ZLIB-1"
component = "zlib"
affected_from = "1.2.12"
fixed_in = "1.3"
hazard = "illustrative; nested component"
source = "test fixture"
verification = "example"
"#;

fn posture_fixture(dir: &Path, fw_version: &str, image: &[u8]) -> po::PostureReport {
    write(dir, "ntp-edge.cdx.json", VENDOR_SBOM);
    let digest = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(image))
    };
    let mut rec =
        po::check_firmware("tp4100-2.5.img", image, &Reference::Sha256(digest), UTC).unwrap();
    rec.version = Some(fw_version.into());
    write(
        dir,
        "gm-ldn-2.fwcheck.json",
        &serde_json::to_string_pretty(&rec).unwrap(),
    );
    let inv: Inventory = toml::from_str(INVENTORY).unwrap();
    po::check(
        &inv,
        &AdvisoryTable::seed(),
        &ComponentTable::parse(COMPONENTS).unwrap(),
        dir,
        NOW,
        UTC,
    )
    .unwrap()
}

fn appliance<'a>(r: &'a po::PostureReport, id: &str) -> &'a po::ApplianceResult {
    r.appliances.iter().find(|a| a.id == id).unwrap()
}

fn finding<'a>(a: &'a po::ApplianceResult, id: &str) -> &'a po::PostureFinding {
    a.findings
        .iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("{}: no finding {id}", a.id))
}

#[test]
fn cve_2025_47904_fixture_is_flagged() {
    let d = tmp("posture");
    let r = posture_fixture(&d, "2.5", b"firmware image 2.5");
    let gm1 = appliance(&r, "gm-ldn-1");
    // The unsigned-upgrade advisory matches, with no fixed version in the seed: review.
    let cve = finding(gm1, "CVE-2025-47904");
    assert_eq!(cve.category, "advisory");
    assert_eq!(cve.status, "review");
    // No out-of-band check recorded: the upgrade path is exposed, act now.
    let fi = finding(gm1, "upgrade-image");
    assert_eq!(fi.status, "exposed");
    assert_eq!(fi.priority, "act");
    assert!(fi.detail.contains("CVE-2025-47904"), "{}", fi.detail);
    // Firmware 2.4 is below the stated fix of CVE-2025-47900/47901.
    assert_eq!(finding(gm1, "CVE-2025-47900").status, "affected");
    assert_eq!(finding(gm1, "snmpv2c").status, "exposed");
    assert_eq!(gm1.worst, "act");
    // The second grandmaster has a matching out-of-band check: mitigated.
    let gm2 = appliance(&r, "gm-ldn-2");
    assert_eq!(finding(gm2, "upgrade-image").status, "mitigated");
    assert_eq!(finding(gm2, "CVE-2025-47900").status, "not-affected");
    assert!(po::report_csv(&r).contains("CVE-2025-47904"));
}

#[test]
fn firmware_check_record_for_another_version_does_not_mitigate() {
    let d = tmp("posture-ver");
    let r = posture_fixture(&d, "2.4", b"firmware image 2.4");
    let fi = finding(appliance(&r, "gm-ldn-2"), "upgrade-image");
    assert_eq!(fi.status, "exposed");
    assert!(fi.detail.contains("2.4 but 2.5"), "{}", fi.detail);
}

#[test]
fn sbom_components_internet_reach_and_services() {
    let d = tmp("posture-sbom");
    let r = posture_fixture(&d, "2.5", b"x");
    let edge = appliance(&r, "ntp-edge");
    assert_eq!(edge.sbom_components, Some(4));
    assert_eq!(finding(edge, "EXAMPLE-OPENSSH-1").status, "affected");
    assert_eq!(finding(edge, "EXAMPLE-CURL-1").status, "not-affected");
    // Nested component found; 1.2.13 is inside [1.2.12, 1.3).
    assert_eq!(finding(edge, "EXAMPLE-ZLIB-1").status, "affected");
    assert_eq!(finding(edge, "management-reach").priority, "act");
    // Exposed to the internet, clear-text services are "act".
    assert_eq!(finding(edge, "telnet").priority, "act");
    assert_eq!(finding(edge, "ntp-mode6").status, "exposed");
    // The device verifies its own images and no record is given: no integrity finding.
    assert!(edge.findings.iter().all(|f| f.id != "upgrade-image"));
}

#[test]
fn posture_rejects_bad_inventories() {
    let d = tmp("posture-bad");
    let comps = ComponentTable::default();
    let t = AdvisoryTable::seed();
    let mut inv: Inventory = toml::from_str(INVENTORY).unwrap();
    inv.appliances[0].management = Some("dmz".into());
    assert!(po::check(&inv, &t, &comps, &d, NOW, UTC).is_err());
    let dup: Inventory = toml::from_str(
        "[[appliances]]\nid=\"a\"\nvendor=\"v\"\nmodel=\"m\"\n[[appliances]]\nid=\"a\"\nvendor=\"v\"\nmodel=\"m\"\n",
    )
    .unwrap();
    assert!(po::check(&dup, &t, &comps, &d, NOW, UTC).is_err());
    assert!(ComponentTable::parse(
        "[[component_advisory]]\nid=\"x\"\ncomponent=\"c\"\nhazard=\"h\"\nsource=\"s\"\nverification=\"v\"\n"
    )
    .is_err());
}

#[test]
fn firmware_check_by_digest_sums_and_signature() {
    use ed25519_dalek::{Signer, SigningKey};
    let img = b"timing appliance firmware 7.10.006";
    let good = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(img))
    };
    let r = po::check_firmware("/x/fw.img", img, &Reference::Sha256(good.clone()), UTC).unwrap();
    assert_eq!((r.verdict.as_str(), r.image.as_str()), ("match", "fw.img"));
    assert!(r.passed());
    let bad = "0".repeat(64);
    assert_eq!(
        po::check_firmware("fw.img", img, &Reference::Sha256(bad.clone()), UTC)
            .unwrap()
            .verdict,
        "mismatch"
    );
    assert!(po::check_firmware("fw.img", img, &Reference::Sha256("abc".into()), UTC).is_err());
    let sums = format!("{bad}  other.img\n{good} *dist/fw.img\n");
    assert_eq!(
        po::check_firmware("fw.img", img, &Reference::Sums(sums.clone()), UTC)
            .unwrap()
            .verdict,
        "match"
    );
    assert_eq!(
        po::check_firmware("nope.img", img, &Reference::Sums(sums), UTC)
            .unwrap()
            .verdict,
        "not-listed"
    );
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let pk = hex::encode(key.verifying_key().to_bytes());
    let sig = key.sign(img).to_bytes();
    for encoded in [
        sig.to_vec(),
        hex::encode(sig).into_bytes(),
        base64(&sig).into_bytes(),
    ] {
        let r = po::check_firmware(
            "fw.img",
            img,
            &Reference::Ed25519 {
                signature: encoded,
                public_key: pk.clone(),
            },
            UTC,
        )
        .unwrap();
        assert_eq!(r.verdict, "signature-valid");
    }
    let r = po::check_firmware(
        "fw.img",
        b"tampered image",
        &Reference::Ed25519 {
            signature: sig.to_vec(),
            public_key: pk,
        },
        UTC,
    )
    .unwrap();
    assert_eq!(r.verdict, "signature-invalid");
    assert!(!r.passed());
}

fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                o.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                o.push('=');
            }
        }
    }
    o
}

const STATION_FW: &str = r#"
name = "KS station firmware"
version = "0.9.0"
supplier = "Example operator"
[[components]]
name = "linux"
version = "6.6.30"
type = "operating-system"
licenses = ["GPL-2.0-only"]
[[components]]
name = "chrony"
version = "4.6"
type = "application"
purl = "pkg:generic/chrony@4.6"
depends_on = ["linux"]
file = "chronyd.bin"
[[components]]
name = "linuxptp"
version = "4.4"
type = "application"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
depends_on = ["linux"]
"#;

#[test]
fn station_sbom_is_cyclonedx_and_reproducible() {
    let d = tmp("sbom");
    write(&d, "chronyd.bin", "chronyd");
    let spec: po::StationSpec = toml::from_str(STATION_FW).unwrap();
    let a = po::station_bom(&spec, &d, UTC).unwrap();
    let b = po::station_bom(&spec, &d, UTC).unwrap();
    assert_eq!(a, b, "same inputs, same SBOM (serial included)");
    assert!(po::check_bom(&a).is_empty(), "{:?}", po::check_bom(&a));
    assert_eq!(a["bomFormat"], "CycloneDX");
    assert_eq!(a["specVersion"], "1.5");
    assert_eq!(a["components"].as_array().unwrap().len(), 3);
    assert_eq!(a["components"][1]["hashes"][0]["alg"], "SHA-256");
    assert!(a["serialNumber"].as_str().unwrap().starts_with("urn:uuid:"));
    assert_eq!(po::bom_components(&a).len(), 3);
    // Defects are caught.
    let mut bad = a.clone();
    bad["components"][2]["bom-ref"] = bad["components"][1]["bom-ref"].clone();
    bad["components"][0]["type"] = "widget".into();
    bad["components"][1]["hashes"][0]["content"] = "abc".into();
    bad["dependencies"][0]["dependsOn"][0] = "missing".into();
    bad["specVersion"] = "2.0".into();
    let issues = po::check_bom(&bad);
    for needle in ["not unique", "widget", "64 hex", "missing", "specVersion"] {
        assert!(
            issues.iter().any(|i| i.contains(needle)),
            "{needle}: {issues:?}"
        );
    }
    // Unknown dependency in the input is refused.
    let mut s2 = spec.clone();
    s2.components[1].depends_on = vec!["glibc".into()];
    assert!(po::station_bom(&s2, &d, UTC).is_err());
}

#[test]
fn release_sbom_from_the_bundle_script_passes_the_check() {
    // The Pro release's SBOM comes from scripts/sbom.py; run it on a small metadata document.
    let meta = r#"{"packages":[
        {"id":"root","name":"kshana-pro","version":"0.2.0","license":null},
        {"id":"a","name":"serde","version":"1.0.200","license":"MIT OR Apache-2.0"},
        {"id":"b","name":"sha2","version":"0.10.8","license":"MIT OR Apache-2.0"}],
        "resolve":{"root":"root","nodes":[{"id":"root"},{"id":"a"},{"id":"b"}]}}"#;
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/sbom.py");
    let child = std::process::Command::new("python3")
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        eprintln!("python3 not available: skipped");
        return;
    };
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(meta.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let bom: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(po::check_bom(&bom).is_empty(), "{:?}", po::check_bom(&bom));
    assert_eq!(po::bom_components(&bom).len(), 2);
}

// ================================================================ B1904 chain

fn delays() -> FixedDelays {
    FixedDelays {
        tx_m: 300e-9,
        rx_m: 250e-9,
        tx_s: 310e-9,
        rx_s: 260e-9,
    }
}

/// A synthetic calibration: the spool's true α and the PPS offset it produces when the link
/// is configured with α = 0.
fn calibration(alpha_true: f64, fibre_rt: f64, u_pps: f64) -> AlphaCalibration {
    let d = delays();
    let rt = fibre_rt + d.tx_m + d.rx_m + d.tx_s + d.rx_s;
    let pps = ch::wr_delay_ms(rt, &d, alpha_true) - ch::wr_delay_ms(rt, &d, 0.0);
    AlphaCalibration {
        round_trip_s: rt,
        u_round_trip_s: 5e-12,
        fixed_delays_s: d,
        u_fixed_s: 2e-12,
        pps_offset_s: pps,
        u_pps_offset_s: u_pps,
        alpha_configured: 0.0,
    }
}

#[test]
fn wr_alpha_is_recovered_within_its_uncertainty() {
    let alpha = 2.6e-4;
    // 5 km spool: about 49 µs of fibre round trip.
    let c = calibration(alpha, 49e-6, 20e-12);
    let (a, u) = ch::wr_alpha_from_calibration(&c).unwrap();
    assert!((a - alpha).abs() < 1e-9, "noise-free: {a}");
    // u(α) ≈ (2+α)² u(pps) / δfibre ≈ 4 × 20 ps / 49 µs.
    assert!(u > 1.4e-6 && u < 2.0e-6, "{u}");
    // With a measurement error of one standard uncertainty the estimate stays within 2u.
    let mut c2 = c.clone();
    c2.pps_offset_s += 20e-12;
    let (a2, u2) = ch::wr_alpha_from_calibration(&c2).unwrap();
    assert!((a2 - alpha).abs() <= 2.0 * u2, "{a2} vs {alpha} ± {u2}");
    let mut bad = c;
    bad.round_trip_s = 1e-9;
    assert!(ch::wr_alpha_from_calibration(&bad).is_err());
}

const CIRCULAR_T: &str =
    "mjd,utc_minus_utck_ns,u_ns\n61334,1.0,2.0\n61339,2.0,2.0\n61344,3.0,2.2\n";

fn chain_spec(date: &str, cal_date: &str) -> String {
    let c = calibration(2.6e-4, 49e-6, 20e-12);
    let d = delays();
    format!(
        r#"title = "Exchange time feed"
end_point = "PoP-1"
date = "{date}"
coverage_k = 2
limit_s = 100e-9
[reference]
lab = "UTC(k)"
circular_t = "circt.csv"

[[hops]]
name = "wr"
type = "wr"
round_trip_s = {rt:e}
u_round_trip_s = 5e-12
fixed_delays_s = {{ tx_m = {a:e}, rx_m = {b:e}, tx_s = {cc:e}, rx_s = {dd:e} }}
u_fixed_s = 2e-12
alpha_configured = 0.0
calibration = {{ id = "CAL-1", date = "{cal_date}", valid_days = 365 }}
[hops.alpha_calibration]
round_trip_s = {crt:e}
u_round_trip_s = 5e-12
fixed_delays_s = {{ tx_m = {a:e}, rx_m = {b:e}, tx_s = {cc:e}, rx_s = {dd:e} }}
u_fixed_s = 2e-12
pps_offset_s = {pps:e}
u_pps_offset_s = 20e-12

[[hops]]
name = "bc"
type = "ptp"
te_class = "class-b"
asymmetry_s = 2e-9
u_asymmetry_s = 1e-9
calibration = {{ id = "CAL-2", date = "2026-09-01", valid_days = 365 }}
"#,
        rt = 490e-6 + d.tx_m + d.rx_m + d.tx_s + d.rx_s,
        a = d.tx_m,
        b = d.rx_m,
        cc = d.tx_s,
        dd = d.rx_s,
        crt = c.round_trip_s,
        pps = c.pps_offset_s,
    )
}

#[test]
fn wr_asymmetry_of_the_deployed_link_is_estimated_within_budget() {
    let d = tmp("chain");
    write(&d, "circt.csv", CIRCULAR_T);
    // 2026-11-12 is MJD 61356 + ... use a date inside the rows: MJD 61341 = 2026-10-28.
    let spec = ChainSpec::parse(&chain_spec("2026-10-28", "2026-07-01")).unwrap();
    let r = ch::assess(&spec, &d, UTC).unwrap();
    let wr = r.hops[0].wr.as_ref().unwrap();
    // Truth: α = 2.6e-4 over 490 µs of fibre round trip: δms − δsm = α·δfibre/(2+α).
    let truth = 2.6e-4 * 490e-6 / (2.0 + 2.6e-4);
    assert!(
        (wr.asymmetry_s - truth).abs() <= 2.0 * wr.u_asymmetry_s,
        "{} vs {truth} ± {}",
        wr.asymmetry_s,
        wr.u_asymmetry_s
    );
    assert!((wr.asymmetry_s - truth).abs() < 0.01e-9);
    // The link is configured with α = 0, so the slave reads behind by half the asymmetry.
    assert!(
        (r.hops[0].bias_s + truth / 2.0).abs() < 0.01e-9,
        "{}",
        r.hops[0].bias_s
    );
    // Reference interpolated between MJD 61339 (2 ns) and 61344 (3 ns): 2.4 ns.
    assert_eq!(r.reference_status, "published");
    assert!((r.utc_minus_utck_s - 2.4e-9).abs() < 1e-15);
    // Offset = reference + WR bias + PTP asymmetry.
    let expect = 2.4e-9 - truth / 2.0 + 2e-9;
    assert!((r.offset_from_utc_s - expect).abs() < 1e-14);
    // u_c is the root sum of squares of the budget; the class-B cTE is 20 ns/√3.
    let rss = r.budget.iter().map(|c| c.u_s * c.u_s).sum::<f64>().sqrt();
    assert!((r.u_c_s - rss).abs() < 1e-15);
    assert!(r
        .budget
        .iter()
        .any(|c| (c.u_s - 20e-9 / 3f64.sqrt()).abs() < 1e-15));
    assert_eq!(r.traceability, "unbroken");
    assert_eq!(r.within_limit, Some(true));
}

#[test]
fn chain_traceability_breaks_and_provisional_reference() {
    let d = tmp("chain-breaks");
    write(&d, "circt.csv", CIRCULAR_T);
    // After the last row (MJD 61344 = 2026-10-31) by 20 days: provisional.
    let r = ch::assess(
        &ChainSpec::parse(&chain_spec("2026-11-20", "2026-07-01")).unwrap(),
        &d,
        UTC,
    )
    .unwrap();
    assert_eq!(r.reference_status, "extrapolated");
    assert_eq!(r.traceability, "provisional");
    assert!(r.u_reference_s > 2.2e-9);
    // Calibration expired: broken, naming the hop.
    let r = ch::assess(
        &ChainSpec::parse(&chain_spec("2026-10-28", "2025-01-01")).unwrap(),
        &d,
        UTC,
    )
    .unwrap();
    assert_eq!(r.traceability, "broken");
    assert!(r
        .breaks
        .iter()
        .any(|b| b.contains("hop wr") && b.contains("expired")));
    // Far beyond the last Circular T row: broken at the reference.
    let r = ch::assess(
        &ChainSpec::parse(&chain_spec("2027-03-01", "2026-07-01")).unwrap(),
        &d,
        UTC,
    )
    .unwrap();
    assert!(r.breaks.iter().any(|b| b.starts_with("reference")));
    // Before the first row: refused.
    assert!(ch::assess(
        &ChainSpec::parse(&chain_spec("2026-01-01", "2025-07-01")).unwrap(),
        &d,
        UTC
    )
    .is_err());
}

#[test]
fn onboarding_test_and_signed_statement() {
    let d = tmp("chain-onboard");
    write(&d, "circt.csv", CIRCULAR_T);
    let base = chain_spec("2026-10-28", "2026-07-01");
    let r0 = ch::assess(&ChainSpec::parse(&base).unwrap(), &d, UTC).unwrap();
    // A delivered service that agrees with the chain's prediction, and one 200 ns off.
    for (name, shift, expect) in [("good", 0.0, "consistent"), ("off", 200e-9, "inconsistent")] {
        let mut csv = String::new();
        for i in 0..120 {
            let wobble = if i % 2 == 0 { 1e-9 } else { -1e-9 };
            csv.push_str(&format!(
                "{},{:e}\n",
                1_793_000_000 + i * 10,
                r0.offset_from_utc_s + shift + wobble
            ));
        }
        write(&d, &format!("{name}.csv"), &csv);
        let src = format!(
            "{base}\n[onboarding]\nformat = \"csv\"\npath = \"{name}.csv\"\nu_ref_s = 5e-9\n"
        );
        let spec = ChainSpec::parse(&src).unwrap();
        let r = ch::assess(&spec, &d, UTC).unwrap();
        let o = r.onboarding.as_ref().unwrap();
        assert_eq!(o.status, expect, "{name}: {o:?}");
        assert_eq!(o.n, 120);
        if name == "good" {
            let out = d.join("statement");
            ch::write_dir(&out, &r, &src, &spec, &d).unwrap();
            let seed = [3u8; 32];
            kshana_pro::signed_dossier::sign(&out, &seed, "Test operator", UTC).unwrap();
            let v = kshana_pro::signed_dossier::verify(&out, &[]).unwrap();
            assert!(v.signature_ok && v.problems.is_empty(), "{v:?}");
            for f in [
                "statement.json",
                "statement.html",
                "budget.csv",
                "inputs/chain.toml",
                "inputs/circt.csv",
                "inputs/good.csv",
            ] {
                assert!(out.join(f).exists(), "{f}");
            }
            // Tampering is caught.
            std::fs::write(out.join("budget.csv"), "changed").unwrap();
            let v = kshana_pro::signed_dossier::verify(&out, &[]).unwrap();
            assert!(!v.problems.is_empty());
        }
    }
    assert!(ChainSpec::parse(ch::TEMPLATE).is_ok());
}

// ================================================================ B1905 cross-check

/// Paris, about 480 km from Mainflingen.
const PARIS: (f64, f64) = (48.8566, 2.3522);

fn delay_dcf77_paris() -> f64 {
    cc::ground_wave_delay_s(
        "dcf77",
        &cc::Location {
            lat_deg: PARIS.0,
            lon_deg: PARIS.1,
        },
    )
    .unwrap()
}

/// Writes one CSV per source: `f(i, t)` the offset at sample `i` (None: no sample).
fn series(dir: &Path, name: &str, n: usize, f: impl Fn(usize) -> Option<f64>) {
    let mut s = String::new();
    for i in 0..n {
        if let Some(o) = f(i) {
            s.push_str(&format!("{},{:e}\n", 1_793_000_000 + 10 * i, o));
        }
    }
    write(dir, &format!("{name}.csv"), &s);
}

/// Deterministic noise, uniform in [-1, 1) (splitmix64 of the sample and stream).
fn noise(i: usize, k: usize) -> f64 {
    let mut z = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (k as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

fn cc_spec(sources: &str, extra: &str) -> Spec {
    toml::from_str(&format!(
        "node = \"pop-1\"\nbin_s = 60\nk = 3\n{extra}\n[location]\nlat_deg = {}\nlon_deg = {}\n{sources}",
        PARIS.0, PARIS.1
    ))
    .unwrap()
}

const FOUR: &str = r#"
[[sources]]
name = "gnss"
class = "gnss"
format = "csv"
path = "gnss.csv"
u_s = 1e-6
[[sources]]
name = "dcf77"
class = "radio-time"
transmitter = "dcf77"
format = "csv"
path = "dcf77.csv"
u_s = 1e-3
[[sources]]
name = "nts-a"
class = "nts"
root = "operator-a"
format = "csv"
path = "nts-a.csv"
u_s = 3e-3
[[sources]]
name = "eloran"
class = "eloran"
format = "csv"
path = "eloran.csv"
u_s = 1e-6
"#;

#[test]
fn ground_wave_delay_is_plausible() {
    let d = delay_dcf77_paris();
    // Mainflingen to Paris is about 480 km: 1.6 ms.
    assert!(d > 1.5e-3 && d < 1.7e-3, "{d}");
    assert!(cc::ground_wave_delay_s(
        "chu",
        &cc::Location {
            lat_deg: 0.0,
            lon_deg: 0.0
        }
    )
    .is_err());
    // A quarter of the meridian: 10,007 km.
    assert!((cc::distance_m(0.0, 0.0, 90.0, 0.0) - 10_007_543.0).abs() < 100.0);
}

#[test]
fn single_source_fault_is_isolated() {
    let d = tmp("cc-iso");
    let delay = delay_dcf77_paris();
    let n = 720; // two hours at 10 s
                 // The node is right; the GNSS source reads it 20 ms off between samples 300 and 420.
    series(&d, "gnss", n, |i| {
        Some(if (300..420).contains(&i) { 20e-3 } else { 0.0 } + 1e-7 * noise(i, 1))
    });
    series(&d, "dcf77", n, |i| Some(delay + 5e-4 * noise(i, 2)));
    series(&d, "nts-a", n, |i| Some(1e-3 * noise(i, 3)));
    series(&d, "eloran", n, |i| Some(1e-7 * noise(i, 4)));
    let r = cc::check(&cc_spec(FOUR, ""), &d).unwrap();
    let iso: Vec<&cc::Event> = r.events.iter().filter(|e| e.kind == "isolated").collect();
    assert_eq!(iso.len(), 1, "{:?}", r.events);
    assert_eq!(iso[0].subject, "gnss");
    assert_eq!(iso[0].bins, 20);
    assert!((iso[0].magnitude_s.unwrap() - 20e-3).abs() < 1e-4);
    assert_eq!(r.counts["isolated"], 20);
    assert_eq!(r.counts["consistent"], 100);
    // The radio-time source carries its ground-wave delay; nts has a stated root.
    let dcf = r.sources.iter().find(|s| s.name == "dcf77").unwrap();
    assert!((dcf.delay_s - delay).abs() < 1e-11);
    assert!(r.unstated_independence.is_empty());
    assert_eq!(r.roots.len(), 4);
    assert!(cc::bins_csv(&r).lines().count() == 121);
}

#[test]
fn shared_root_does_not_outvote_an_independent_source() {
    let d = tmp("cc-dep");
    let n = 360;
    let delay = delay_dcf77_paris();
    // GNSS and a PTP grandmaster disciplined by it both read 10 ms off; DCF77 is right.
    series(&d, "gnss", n, |i| Some(10e-3 + 1e-7 * noise(i, 1)));
    series(&d, "ptp", n, |i| Some(10e-3 + 1e-7 * noise(i, 5)));
    series(&d, "dcf77", n, |i| Some(delay + 5e-4 * noise(i, 2)));
    let src = r#"
[[sources]]
name = "gnss"
class = "gnss"
format = "csv"
path = "gnss.csv"
u_s = 1e-6
[[sources]]
name = "ptp"
class = "ptp"
depends_on = "gnss"
format = "csv"
path = "ptp.csv"
u_s = 1e-6
[[sources]]
name = "dcf77"
class = "radio-time"
transmitter = "dcf77"
format = "csv"
path = "dcf77.csv"
u_s = 1e-3
"#;
    let r = cc::check(&cc_spec(src, ""), &d).unwrap();
    // Two roots: a disagreement is detected but not attributed (a naive two-against-one vote
    // would blame DCF77).
    assert_eq!(r.roots, vec!["dcf77".to_string(), "gnss".to_string()]);
    assert_eq!(r.counts["detected-not-isolated"], r.bins.len());
    assert!(r.bins.iter().all(|b| b.isolated.is_none()));
    // Undeclared dependency: the PTP source becomes its own root and is listed as such.
    let src2 = src.replace("depends_on = \"gnss\"\n", "");
    let r2 = cc::check(&cc_spec(&src2, ""), &d).unwrap();
    assert_eq!(r2.unstated_independence, vec!["ptp".to_string()]);
}

#[test]
fn member_fault_node_off_and_silent_source() {
    let d = tmp("cc-member");
    let n = 720;
    let delay = delay_dcf77_paris();
    // Three GNSS receivers, one 2 µs off throughout; DCF77 and eLoran; the node itself is
    // 20 ms off for samples 100..200 (all sources see it); eLoran stops after sample 500.
    let node = |i: usize| if (100..200).contains(&i) { 20e-3 } else { 0.0 };
    series(&d, "g1", n, |i| Some(node(i) + 1e-8 * noise(i, 1)));
    series(&d, "g2", n, |i| Some(node(i) + 1e-8 * noise(i, 2)));
    series(&d, "g3", n, |i| Some(node(i) + 2e-6 + 1e-8 * noise(i, 3)));
    series(&d, "dcf77", n, |i| {
        Some(node(i) + delay + 5e-4 * noise(i, 4))
    });
    series(&d, "eloran", n, |i| {
        (i < 500).then(|| node(i) + 1e-8 * noise(i, 6))
    });
    let mut src = String::new();
    for g in ["g1", "g2", "g3"] {
        src.push_str(&format!(
            "[[sources]]\nname = \"{g}\"\nclass = \"gnss\"\nformat = \"csv\"\npath = \"{g}.csv\"\nu_s = 1e-7\n"
        ));
    }
    src.push_str("[[sources]]\nname = \"dcf77\"\nclass = \"radio-time\"\ntransmitter = \"dcf77\"\nformat = \"csv\"\npath = \"dcf77.csv\"\nu_s = 1e-3\n");
    src.push_str("[[sources]]\nname = \"eloran\"\nclass = \"eloran\"\nformat = \"csv\"\npath = \"eloran.csv\"\nu_s = 1e-7\n");
    let r = cc::check(&cc_spec(&src, "node_limit_s = 0.010\nstale_s = 30"), &d).unwrap();
    let kinds = |k: &str| r.events.iter().filter(|e| e.kind == k).collect::<Vec<_>>();
    let mf = kinds("member-fault");
    assert_eq!(mf.len(), 1, "{:?}", r.events);
    assert_eq!(mf[0].subject, "g3");
    assert_eq!(mf[0].bins, 120);
    let off = kinds("node-off");
    assert_eq!(off.len(), 1);
    // Samples 100..200 (1000 s to 1990 s); a bin's value is the median of its six samples, so
    // the partly covered bins 16 and 33 stay right: bins 17 to 32.
    assert_eq!(off[0].bins, 16);
    assert!((off[0].magnitude_s.unwrap() - 20e-3).abs() < 1e-5);
    let sil = kinds("silent");
    assert_eq!(sil.len(), 1);
    assert_eq!(sil[0].subject, "eloran");
    // Last eLoran sample at +4990 s; the alarm is 30 s later (within the 10 s acceptance of a
    // stopped source once stale_s is set to the operator's budget).
    assert!((sil[0].start_s - (1_793_000_000.0 + 4990.0)).abs() < 1e-6);
    assert!(sil[0].detail.contains("1793005020"), "{}", sil[0].detail);
}

#[test]
fn triad_monitor_flags_a_noisier_than_stated_source() {
    let d = tmp("cc-triad");
    let n = 2400;
    // Three independent roots with white noise; the third is ten times noisier than stated.
    let gauss = |i: usize, k: usize| {
        // Sum of four uniforms: close enough to Gaussian for a variance estimate.
        (noise(i, k) + noise(i, k + 100) + noise(i, k + 200) + noise(i, k + 300)) / 2.0
    };
    series(&d, "a", n, |i| Some(1e-6 * gauss(i, 1)));
    series(&d, "b", n, |i| Some(1e-6 * gauss(i, 2)));
    series(&d, "c", n, |i| Some(1e-5 * gauss(i, 3)));
    let mut src = String::new();
    for (name, root) in [("a", "ra"), ("b", "rb"), ("c", "rc")] {
        src.push_str(&format!(
            "[[sources]]\nname = \"{name}\"\nclass = \"fibre\"\nroot = \"{root}\"\nformat = \"csv\"\npath = \"{name}.csv\"\nu_s = 1e-6\n"
        ));
    }
    let spec: Spec = toml::from_str(&format!("node = \"n\"\nbin_s = 10\nk = 100\n{src}")).unwrap();
    let r = cc::check(&spec, &d).unwrap();
    let t = |root: &str| r.triad.iter().find(|x| x.root == root).unwrap();
    assert_eq!(t("rc").status, "noisier-than-stated", "{:?}", r.triad);
    assert_eq!(t("ra").status, "as-stated", "{:?}", r.triad);
    assert!(t("rc").estimated_sigma_s.unwrap() > 3e-6);
}

#[test]
fn crosscheck_requests_are_validated_and_profiles_parse() {
    let d = tmp("cc-bad");
    let bad_class = r#"
[[sources]]
name = "x"
class = "sundial"
format = "csv"
path = "x.csv"
"#;
    let s: Spec = toml::from_str(&format!("node = \"n\"\n{bad_class}")).unwrap();
    assert!(cc::check(&s, &d).is_err());
    let no_loc: Spec = toml::from_str(
        "node = \"n\"\n[[sources]]\nname = \"m\"\nclass = \"radio-time\"\ntransmitter = \"msf\"\nformat = \"csv\"\npath = \"m.csv\"\n",
    )
    .unwrap();
    assert!(cc::resolve(&no_loc).unwrap_err().contains("location"));
    let cyc: Spec = toml::from_str(
        "node = \"n\"\n[[sources]]\nname = \"a\"\nclass = \"ptp\"\ndepends_on = \"b\"\nformat = \"csv\"\npath = \"a\"\n[[sources]]\nname = \"b\"\nclass = \"ptp\"\ndepends_on = \"a\"\nformat = \"csv\"\npath = \"b\"\n",
    )
    .unwrap();
    assert!(cc::resolve(&cyc).unwrap_err().contains("cycle"));
    for (name, _, toml_text) in cc::PROFILES {
        let s: Spec = toml::from_str(toml_text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let r = cc::resolve(&s).unwrap();
        assert!(
            r.iter()
                .map(|x| &x.root)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= 3,
            "{name}"
        );
    }
}

// ================================================================ B1907 station

const PASSES: &str = "pass_id,satellite,antenna,provider,customer,scheduled_aos,scheduled_los,actual_aos,actual_los,bytes_planned,bytes_delivered,frames_bad,frames_total,status
p1,SAT-A,ant-1,direct,cust-1,2026-10-01T00:00:00Z,2026-10-01T00:10:00Z,2026-10-01T00:00:05Z,2026-10-01T00:10:00Z,1000,1000,0,10000,ok
p2,SAT-A,ant-1,direct,cust-1,2026-10-01T02:00:00Z,2026-10-01T02:10:00Z,2026-10-01T02:00:05Z,2026-10-01T02:10:00Z,1000,1000,0,10000,ok
p3,SAT-B,ant-2,broker-x,cust-2,2026-10-01T04:00:00Z,2026-10-01T04:10:00Z,2026-10-01T04:00:10Z,2026-10-01T04:10:00Z,1000,900,0,10000,ok
p4,SAT-B,ant-2,broker-x,cust-2,2026-10-01T06:00:00Z,2026-10-01T06:10:00Z,,,1000,0,,,failed
p5,SAT-A,ant-1,broker-x,cust-1,2026-10-01T08:00:00Z,2026-10-01T08:10:00Z,,,,,,,cancelled-customer
p6,SAT-A,ant-1,direct,cust-1,2026-10-01T10:00:00Z,2026-10-01T10:10:00Z,2026-10-01T10:02:00Z,2026-10-01T10:10:00Z,1000,1000,0,10000,ok
p7,SAT-C,ant-1,direct,cust-3,2026-10-01T12:00:00Z,2026-10-01T12:10:00Z,2026-10-01T12:00:02Z,2026-10-01T12:10:00Z,1000,1000,5,10000,ok
p8,SAT-C,ant-2,direct,cust-3,2026-10-01T14:00:00Z,2026-10-01T14:10:00Z,2026-10-01T14:00:02Z,2026-10-01T14:10:00Z,1000,1000,0,10000,ok
p9,SAT-C,ant-2,direct,cust-3,2026-11-01T14:00:00Z,2026-11-01T14:10:00Z,2026-11-01T14:00:02Z,2026-11-01T14:10:00Z,1000,1000,0,10000,ok
";

#[test]
fn antenna_timing_limit_closed_form() {
    // v = sqrt(μ/(R+h)) at 500 km: 7,612 m/s; ω = v/h = 0.01522 rad/s = 0.872°/s.
    let w = st::max_angular_rate(500.0);
    assert!((w - 7612.6 / 500e3).abs() < 1e-4, "{w}");
    let a = st::Antenna {
        beamwidth_deg: 2.0,
        pointing_fraction: 0.1,
        altitude_km: 500.0,
    };
    let l = st::antenna_limit_s(&a);
    assert!((l - 0.1 * 2f64.to_radians() / w).abs() < 1e-12);
    assert!(l > 0.2 && l < 0.25, "{l}");
}

#[test]
fn station_sla_and_timing_during_passes() {
    let d = tmp("station");
    write(&d, "passes.csv", PASSES);
    // Station clock: right, except 0.5 s off during pass p8 (14:00-14:10 on 1 Oct).
    let t0 = 1_790_812_800i64; // 2026-10-01T00:00:00Z
    let mut clock = String::new();
    for i in 0..(24 * 60) {
        let t = t0 + 60 * i;
        let off = if (14 * 60..14 * 60 + 10).contains(&i) {
            0.5
        } else {
            1e-6
        };
        clock.push_str(&format!("{t},{off}\n"));
    }
    write(&d, "clock.csv", &clock);
    let src = r#"
station = "GS-1"
operator = "Example teleport"
passes = "passes.csv"
from = "2026-10-01"
to = "2026-11-01"
[clock]
format = "csv"
path = "clock.csv"
[antenna]
beamwidth_deg = 2.0
altitude_km = 500
[sla]
target_success_rate = 0.8
"#;
    write(&d, "station.toml", src);
    let spec: StationSpec = toml::from_str(src).unwrap();
    let r = st::assess(&spec, &d, UTC).unwrap();
    let v = |id: &str| r.passes.iter().find(|p| p.id == id).unwrap();
    assert_eq!(r.passes.len(), 8, "p9 is outside the period");
    assert_eq!(v("p1").verdict, "delivered");
    assert_eq!(v("p3").verdict, "degraded"); // 90 % of the data
    assert!(v("p3").missed_criteria[0].contains("data delivered"));
    assert_eq!(v("p4").verdict, "missed");
    assert_eq!(v("p5").verdict, "excluded");
    assert_eq!(v("p6").verdict, "degraded"); // AOS 120 s late, contact 80 %
    assert_eq!(v("p6").missed_criteria.len(), 2);
    assert_eq!(v("p7").verdict, "degraded"); // FER 5e-4
    assert_eq!(v("p8").verdict, "degraded");
    assert_eq!(v("p8").timing, "exceeded");
    assert_eq!(v("p1").timing, "ok");
    assert_eq!(r.timing_exceeded, 1);
    let agg = |by: &str, key: &str| {
        r.aggregates
            .iter()
            .find(|a| a.by == by && a.key == key)
            .unwrap()
    };
    let s = agg("station", "GS-1");
    assert_eq!(
        (s.counted, s.delivered, s.degraded, s.missed, s.excluded),
        (7, 2, 4, 1, 1)
    );
    assert_eq!(s.meets_target, Some(false));
    let b = agg("provider", "broker-x");
    assert_eq!((b.counted, b.delivered, b.excluded), (2, 0, 1));
    assert!(r.verdict.contains("2 of 7"), "{}", r.verdict);
    // Signed evidence directory.
    let out = d.join("evidence");
    st::write_dir(&out, &r, src, &spec, &d).unwrap();
    kshana_pro::signed_dossier::sign(&out, &[5u8; 32], "Example teleport", UTC).unwrap();
    let ver = kshana_pro::signed_dossier::verify(&out, &[]).unwrap();
    assert!(ver.signature_ok && ver.problems.is_empty(), "{ver:?}");
    assert!(st::passes_csv(&r).lines().count() == 9);
}

#[test]
fn station_inputs_are_validated() {
    assert!(st::read_passes("pass_id,satellite\np1,A\n").is_err());
    assert!(st::read_passes(
        "pass_id,satellite,scheduled_aos,scheduled_los\np1,A,2026-10-01T00:10:00Z,2026-10-01T00:00:00Z\n"
    )
    .is_err());
    let p = st::read_passes(
        "pass_id,satellite,scheduled_aos,scheduled_los\np1,A,1790812800,1790813400\n",
    )
    .unwrap();
    assert_eq!(p[0].status, "ok");
    assert_eq!(p[0].scheduled_los - p[0].scheduled_aos, 600.0);
}

/// Robustness audit: a pass with a NaN, infinite or unreadable optional cell is refused. It
/// used to read as no value (the check skipped) or as NaN (the check passed): a pass with
/// `bytes_delivered = NaN` met the delivery SLA.
#[test]
fn station_passes_refuse_non_finite_or_unreadable_cells() {
    let head = "pass_id,satellite,scheduled_aos,scheduled_los,actual_aos,bytes_planned,bytes_delivered,frames_bad,frames_total\n";
    let row = |aos: &str, del: &str, bad: &str| {
        format!("{head}p1,A,2026-11-01T10:00:00Z,2026-11-01T10:10:00Z,{aos},1000,{del},{bad},100\n")
    };
    assert!(st::read_passes(&row("2026-11-01T10:00:05Z", "990", "1")).is_ok());
    // Empty optional cells are still allowed.
    assert!(st::read_passes(&row("", "", "")).is_ok());
    for (aos, del, bad) in [
        ("2026-11-01T10:00:05Z", "NaN", "1"),
        ("2026-11-01T10:00:05Z", "990", "inf"),
        ("2026-11-01T10:00:05Z", "n/a", "1"),
        ("NaN", "990", "1"),
        ("yesterday", "990", "1"),
    ] {
        let e = st::read_passes(&row(aos, del, bad)).unwrap_err();
        assert!(
            e.contains("not a finite number or time"),
            "{aos} {del} {bad}: {e}"
        );
    }
}
