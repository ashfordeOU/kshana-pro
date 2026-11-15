// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Roadmap 0.42 F7 (owner gate K6): synthetic attack IQ with guardrails. Pro only and
//! licence-gated (the `pro-attack-iq` feature and the `vetted_buyer` flag, no trial, no
//! expired licence); a lab or test-range attestation valid today; files only (devices,
//! pipes, standard output, existing files and driver-like targets refused; no radio,
//! network or process code in the module); every file watermarked to the licence and every
//! generation logged. Every file here is written to a temporary folder, never the
//! repository (tests/lab_campaign_no_client_data.rs forbids IQ files in the tree).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::attack_iq::{self, Attestation, Request, Waveform, STATEMENT};
use kshana_pro::license::{
    self, issue_v2, public_key_for, verify_with, Keyring, Licence, LicenseClaims, VerifyOptions,
};

const SEED: [u8; 32] = [0x6b; 32];

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn today_offset(days: i64) -> String {
    kshana_pro::serve::jobs::utc_stamp(now() + days * 86_400)[..10].to_string()
}

fn tmp(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("attack_iq-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn licence(features: &[&str], vetted: bool, trial: bool) -> (Licence, String) {
    let t = now();
    let claims = LicenseClaims {
        licensee: "Test Range Lab".into(),
        tier: "pro".into(),
        expires_unix: t + 7 * 86_400,
        products: vec!["cli".into()],
        features: features.iter().map(|s| s.to_string()).collect(),
        vetted_buyer: vetted,
        trial,
        issued_unix: t,
        ..Default::default()
    };
    let tok = issue_v2(&SEED, &claims).unwrap();
    let lic = verify_with(
        &Keyring::single(public_key_for(&SEED)),
        &tok,
        &VerifyOptions::at(t),
    )
    .unwrap();
    (lic, tok)
}

fn allows(lic: &Licence) -> bool {
    license::check(&lic.entitlement, attack_iq::FEATURE, now()).is_ok()
}

fn attestation() -> Attestation {
    Attestation {
        facility: "Shielded chamber 2, Test Range Lab".into(),
        authorisation_ref: "LAB-ACC-0042".into(),
        purpose: "Receiver interference detection tests".into(),
        valid_from: today_offset(-1),
        valid_to: today_offset(30),
        signed_by: "Head of laboratory".into(),
        ..Attestation::template("Test Range Lab")
    }
}

fn req(w: Waveform) -> Request {
    Request {
        waveform: w,
        sample_rate_hz: 2.0e6,
        duration_s: 0.01,
        inr_db: 20.0,
        seed: 7,
    }
}

#[test]
fn only_a_vetted_licence_with_the_feature_generates() {
    let d = tmp("gate");
    let log = d.join("log");
    let cw = req(Waveform::Cw { offset_hz: 1.0e5 });
    let cases = [
        (licence(&[], true, false), "needs a licence granting"),
        (
            licence(&["pro-attack-iq"], false, false),
            "vetted buyers only",
        ),
        (licence(&["pro-attack-iq"], true, true), "trial"),
    ];
    for (i, ((lic, tok), why)) in cases.iter().enumerate() {
        let out = d.join(format!("refused-{i}.ci16"));
        let e = attack_iq::generate(
            lic,
            tok,
            allows(lic),
            &attestation(),
            &cw,
            &out,
            &log,
            now(),
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains(why), "{why}: {e}");
        assert!(!out.exists(), "nothing written on refusal");
    }
    // Expired.
    let (lic, tok) = licence(&["pro-attack-iq"], true, false);
    let late = now() + 8 * 86_400;
    let e = attack_iq::generate(
        &lic,
        &tok,
        true,
        &attestation(),
        &cw,
        &d.join("late.ci16"),
        &log,
        late,
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("expired"), "{e}");
    assert!(
        !attack_iq::log_path(&log).exists(),
        "refusals generate nothing to log"
    );
    // Granted and vetted: it works.
    let out = d.join("ok.ci16");
    assert!(allows(&lic));
    attack_iq::generate(
        &lic,
        &tok,
        allows(&lic),
        &attestation(),
        &cw,
        &out,
        &log,
        now(),
    )
    .unwrap();
    assert!(out.exists());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_attestation_must_hold_today_for_this_licensee() {
    let (lic, tok) = licence(&["pro-attack-iq"], true, false);
    let d = tmp("attest");
    let cw = req(Waveform::Cw { offset_hz: 0.0 });
    let mut bad = Vec::new();
    let mut a = attestation();
    a.licensee = "Someone Else".into();
    bad.push((a, "another licensee"));
    let mut a = attestation();
    a.valid_to = today_offset(-1);
    a.valid_from = today_offset(-10);
    bad.push((a, "is valid"));
    let mut a = attestation();
    a.environment = "open-air".into();
    bad.push((a, "environment"));
    let mut a = attestation();
    a.statement = STATEMENT.replace("will not be radiated", "may be radiated");
    bad.push((a, "word for word"));
    let mut a = attestation();
    a.authorisation_ref = "".into();
    bad.push((a, "authorisation_ref"));
    for (i, (a, why)) in bad.into_iter().enumerate() {
        let out = d.join(format!("a{i}.ci16"));
        let e = attack_iq::generate(&lic, &tok, true, &a, &cw, &out, &d, now())
            .unwrap_err()
            .to_string();
        assert!(e.contains(why), "{why}: {e}");
        assert!(!out.exists());
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn files_only_never_a_device_pipe_or_radio() {
    let d = tmp("paths");
    let existing = d.join("exists.ci16");
    std::fs::write(&existing, b"x").unwrap();
    let mut refused: Vec<PathBuf> = vec![
        PathBuf::from("-"),
        PathBuf::from("/dev/null"),
        PathBuf::from("/proc/self/fd/1"),
        PathBuf::from("uhd:type=b200"),
        PathBuf::from("soapy:driver=hackrf"),
        PathBuf::from("tcp://127.0.0.1:1234"),
        existing,
        d.join("no-such-folder").join("x.ci16"),
    ];
    #[cfg(unix)]
    {
        let fifo = d.join("fifo");
        let ok = Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .map(|s| s.success());
        if ok.unwrap_or(false) {
            refused.push(fifo);
        }
    }
    for p in refused {
        assert!(attack_iq::check_output_path(&p).is_err(), "{}", p.display());
    }
    assert!(attack_iq::check_output_path(&d.join("new.ci16")).is_ok());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_module_has_no_radio_network_or_process_code() {
    let src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/attack_iq.rs"))
            .unwrap();
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "std::process",
        "Command::new",
        "std::net",
        "TcpStream",
        "UdpSocket",
        "libc::",
        "ioctl",
        "extern \"C\"",
        "unsafe",
    ] {
        assert!(!code.contains(forbidden), "{forbidden} in src/attack_iq.rs");
    }
}

#[test]
fn every_file_is_watermarked_described_and_logged() {
    let (lic, tok) = licence(&["pro-attack-iq"], true, false);
    let d = tmp("mark");
    let log = d.join("log");
    let waves = [
        Waveform::Cw { offset_hz: 2.5e5 },
        Waveform::Chirp {
            sweep_hz: 1.0e6,
            period_s: 1e-3,
        },
        Waveform::Noise { bandwidth_hz: 5e5 },
        Waveform::Pulsed {
            offset_hz: 0.0,
            period_s: 1e-3,
            duty: 0.25,
        },
        Waveform::Replay {
            chip_rate_hz: 5.115e5,
            delay_s: 1e-4,
            gain_db: 3.0,
        },
    ];
    for (i, w) in waves.into_iter().enumerate() {
        let out = d.join(format!("w{i}.ci16"));
        let r = req(w);
        let meta =
            attack_iq::generate(&lic, &tok, true, &attestation(), &r, &out, &log, now()).unwrap();
        let samples = attack_iq::read_ci16(&out).unwrap();
        assert_eq!(samples.len(), 2 * meta.samples);
        assert_eq!(meta.samples, 20_000);
        // Traced to this licence from the samples alone.
        let (_, other_tok) = licence(&["pro-attack-iq"], true, false);
        let cands = vec![
            ("Other Lab".to_string(), other_tok),
            ("Test Range Lab".to_string(), tok.clone()),
        ];
        assert_eq!(attack_iq::trace(&samples, &cands), Some("Test Range Lab"));
        assert_eq!(attack_iq::trace(&samples, &cands[..1]), None);
        // The sidecar: notice, attestation hash, no licensee name.
        let side = std::fs::read_to_string(format!("{}{}", out.display(), attack_iq::META_SUFFIX))
            .unwrap();
        assert!(side.contains("SYNTHETIC TEST SIGNAL"));
        assert!(side.contains(&attestation().sha256()));
        assert!(!side.contains("Test Range Lab"));
        // Generating onto the same file again is refused.
        assert!(
            attack_iq::generate(&lic, &tok, true, &attestation(), &r, &out, &log, now()).is_err()
        );
    }
    // Interference is present at the requested level (20 dB over unit noise).
    let s = attack_iq::read_ci16(&d.join("w0.ci16")).unwrap();
    let p: f64 = s.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / s.len() as f64;
    assert!(p > 1.0e8, "signal well above quantisation: {p}");
    // The log: one chained line per file, token named, no licensee name.
    let text = std::fs::read_to_string(attack_iq::log_path(&log)).unwrap();
    assert_eq!(text.lines().count(), 5);
    assert!(text.contains(&lic.token_id));
    assert!(!text.contains("Test Range Lab"));
    let mut prev = String::new();
    for l in text.lines() {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        assert_eq!(v["prev"].as_str().unwrap(), prev);
        use sha2::{Digest, Sha256};
        prev = hex::encode(Sha256::digest(l.as_bytes()));
    }
    // Bounds.
    let mut long = req(Waveform::Cw { offset_hz: 0.0 });
    long.duration_s = 61.0;
    assert!(attack_iq::generate(
        &lic,
        &tok,
        true,
        &attestation(),
        &long,
        &d.join("long.ci16"),
        &log,
        now()
    )
    .is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_cli_takes_the_licence_from_the_command_only_and_ignores_dev() {
    let bin = env!("CARGO_BIN_EXE_kshana-pro");
    let d = tmp("cli");
    let home = d.join("home");
    let att = d.join("attestation.json");
    std::fs::write(&att, serde_json::to_string(&attestation()).unwrap()).unwrap();
    let run = |args: &[&str]| {
        Command::new(bin)
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .env_remove("KSHANA_PRO_SIGNING_KEY")
            .env_remove("KSHANA_PRO_SIGNER")
            .output()
            .unwrap()
    };
    let out_file = d.join("cli.ci16");
    let gen = |lic: Option<&str>, extra: &[&str]| {
        let mut a = vec![
            "attack-iq",
            "generate",
            "--attestation",
            att.to_str().unwrap(),
            "--out",
            out_file.to_str().unwrap(),
            "--waveform",
            "cw",
            "--sample-rate",
            "1000000",
            "--duration",
            "0.01",
            "--log-dir",
            d.to_str().unwrap(),
        ];
        if let Some(l) = lic {
            a.push("--license");
            a.push(l);
        }
        a.extend_from_slice(extra);
        run(&a)
    };
    // No licence (even with --dev): refused, nothing written.
    let o = gen(None, &["--dev"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("--license"));
    assert!(!out_file.exists());
    // The debug build signs with its demo key: a licence without the vetted flag is refused.
    let issue = |extra: &[&str]| {
        let mut a = vec![
            "license",
            "issue",
            "--tier",
            "pro",
            "--licensee",
            "Test Range Lab",
            "--days",
            "3",
            "--product",
            "cli",
            "--feature",
            "pro-attack-iq",
        ];
        a.extend_from_slice(extra);
        let o = run(&a);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let plain = issue(&[]);
    let o = gen(Some(&plain), &[]);
    if String::from_utf8_lossy(&o.stderr).contains("does not know") {
        // A build with a vendor key compiled in does not trust the demo key: nothing more to
        // check through the binary here (the library tests above cover the gate).
        let _ = std::fs::remove_dir_all(&d);
        return;
    }
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("vetted"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let vetted = issue(&["--vetted-buyer"]);
    let o = gen(Some(&vetted), &[]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out_file.exists());
    let cands = d.join("cands.json");
    std::fs::write(
        &cands,
        serde_json::to_string(&vec![("Test Range Lab", vetted.as_str())]).unwrap(),
    )
    .unwrap();
    let o = run(&[
        "attack-iq",
        "trace",
        "--file",
        out_file.to_str().unwrap(),
        "--candidates",
        cands.to_str().unwrap(),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("Test Range Lab"));
    let _ = std::fs::remove_dir_all(&d);
}
