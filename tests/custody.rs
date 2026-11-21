// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Sealed incident capture and chain of custody (research 2026-11-20 gaps B0604) on a SYNTHETIC
//! SigMF recording: the ring buffer freezes the samples around the trigger byte for byte; the
//! seal, the custody form and the handover chain verify; any changed byte or re-ordered handover
//! fails; the chain exports as JSON; the capture needs a licence and the rest is free.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana::sdr::Cf64;
use kshana::sigmf::{DataType, Meta, Recording};
use kshana_pro::custody::{self, Ring, Trigger};

const FS: f64 = 100_000.0;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b0604-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 1 s of low noise with a strong burst from 0.6 s; ci16.
fn recording(d: &Path) {
    let n = FS as usize;
    let mut s: u64 = 7;
    let samples: Vec<Cf64> = (0..n)
        .map(|k| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let a = ((s >> 33) as f64 / (1u64 << 31) as f64 - 0.5) * 0.02;
            let burst = if k >= 60_000 { 0.5 } else { 0.0 };
            Cf64::new(a + burst, -a)
        })
        .collect();
    let rec = Recording {
        meta: Meta::new(
            DataType::Ci16Le,
            FS,
            1_575_420_000.0,
            "SYNTHETIC custody test",
        ),
        samples,
    };
    let (m, data, _) = kshana::sigmf::write(&rec, 1.0).unwrap();
    std::fs::write(d.join("src.sigmf-meta"), m).unwrap();
    std::fs::write(d.join("src.sigmf-data"), data).unwrap();
}

fn request(trigger: &str) -> String {
    format!(
        "title = \"SYNTHETIC capture\"\nmeta = \"src.sigmf-meta\"\ndata = \"src.sigmf-data\"\n\
         pre_s = 0.1\npost_s = 0.05\ncalibration_id = \"CAL-SYNTH-001\"\ncollected_by = \"Analyst A\"\n\
         location = \"synthetic bench\"\nreason = \"test\"\n{trigger}"
    )
}

#[test]
fn ring_keeps_the_latest_samples_in_order() {
    let mut r = Ring::new(3, 2);
    for k in 0u8..5 {
        r.push(&[k, k]);
    }
    assert_eq!(r.freeze(), vec![2, 2, 3, 3, 4, 4]);
    let mut short = Ring::new(4, 1);
    short.push(&[9]);
    assert_eq!(short.freeze(), vec![9]);
}

#[test]
fn capture_freezes_the_trigger_window_byte_for_byte() {
    let d = tmp("cap");
    recording(&d);
    let src = std::fs::read(d.join("src.sigmf-data")).unwrap();
    let seed = [11u8; 32];
    for (trig, expect) in [
        ("[trigger]\nkind = \"sample\"\nsample = 30000\n", 30_000u64),
        (
            "[trigger]\nkind = \"power\"\nthreshold_db = 10.0\nblock_samples = 1000\n",
            60_000,
        ),
    ] {
        let q: custody::Request = toml::from_str(&request(trig)).unwrap();
        let out = d.join(format!("out-{expect}"));
        let c = custody::capture(&q, &d, &out, &seed, "2026-11-20T00:00:00Z").unwrap();
        assert_eq!(c.pre_samples, 10_000);
        assert_eq!(c.post_samples, 5_000);
        assert_eq!(c.first_source_sample, expect - 10_000);
        let cap = std::fs::read(out.join(custody::DATA)).unwrap();
        let bps = 4;
        assert_eq!(
            cap,
            src[(expect as usize - 10_000) * bps..(expect as usize + 5_000) * bps].to_vec()
        );
        let v = custody::verify(&out, &[kshana_pro::license::public_key_for(&seed)]).unwrap();
        assert!(v.passed(), "{:?}", v.problems);
        assert_eq!(v.trusted, Some(true));
        // The sealed metadata is valid SigMF the open reader takes.
        let rec = kshana::sigmf::read(
            &std::fs::read_to_string(out.join(custody::META)).unwrap(),
            &cap,
            1.0,
        )
        .unwrap();
        assert_eq!(rec.samples.len(), 15_000);
    }
    // A trigger beyond the recording is refused.
    let q: custody::Request =
        toml::from_str(&request("[trigger]\nkind = \"sample\"\nsample = 999999\n")).unwrap();
    assert!(custody::capture(&q, &d, &d.join("x"), &seed, "2026-11-20T00:00:00Z").is_err());
    let _ = Trigger::Sample { sample: 0 };
}

#[test]
fn any_changed_byte_or_reordered_handover_fails() {
    let d = tmp("tamper");
    recording(&d);
    let seed = [12u8; 32];
    let q: custody::Request =
        toml::from_str(&request("[trigger]\nkind = \"sample\"\nsample = 50000\n")).unwrap();
    let out = d.join("c");
    custody::capture(&q, &d, &out, &seed, "2026-11-20T00:00:00Z").unwrap();
    custody::handover(
        &out,
        "Analyst A",
        "Lab B",
        "analysis",
        &seed,
        "2026-11-21T00:00:00Z",
    )
    .unwrap();
    custody::handover(
        &out,
        "Lab B",
        "Counsel C",
        "review",
        &[13u8; 32],
        "2026-11-22T00:00:00Z",
    )
    .unwrap();
    assert!(custody::verify(&out, &[]).unwrap().passed());
    let chain = custody::export(&out, &[]).unwrap();
    assert_eq!(chain.handovers.len(), 2);
    assert_eq!(chain.format, custody::CHAIN_FORMAT);
    let fresh = |name: &str| {
        let c = d.join(name);
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(&c).unwrap();
        for f in [custody::META, custody::DATA, custody::FORM, custody::LOG] {
            std::fs::copy(out.join(f), c.join(f)).unwrap();
        }
        c
    };
    // One flipped bit in each file.
    for f in [custody::DATA, custody::META, custody::FORM, custody::LOG] {
        let c = fresh(&format!("flip-{f}"));
        let mut b = std::fs::read(c.join(f)).unwrap();
        let i = b.len() / 2;
        b[i] ^= 0x01;
        std::fs::write(c.join(f), &b).unwrap();
        let ok = custody::verify(&c, &[])
            .map(|v| v.passed())
            .unwrap_or(false);
        assert!(!ok, "a flipped bit in {f} must fail");
    }
    // Re-ordered handovers.
    let c = fresh("reorder");
    let mut log: Vec<custody::Handover> =
        serde_json::from_str(&std::fs::read_to_string(c.join(custody::LOG)).unwrap()).unwrap();
    log.swap(0, 1);
    std::fs::write(c.join(custody::LOG), serde_json::to_string(&log).unwrap()).unwrap();
    assert!(!custody::verify(&c, &[]).unwrap().passed());
    // A dropped handover.
    let c = fresh("drop");
    let mut log: Vec<custody::Handover> =
        serde_json::from_str(&std::fs::read_to_string(c.join(custody::LOG)).unwrap()).unwrap();
    log.remove(0);
    std::fs::write(c.join(custody::LOG), serde_json::to_string(&log).unwrap()).unwrap();
    assert!(!custody::verify(&c, &[]).unwrap().passed());
    // No handover is appended to a directory that does not verify.
    let c = fresh("refuse");
    let mut b = std::fs::read(c.join(custody::DATA)).unwrap();
    b[0] ^= 1;
    std::fs::write(c.join(custody::DATA), &b).unwrap();
    assert!(custody::handover(&c, "x", "y", "z", &seed, "2026-11-23T00:00:00Z").is_err());
    // An untrusted sealer is reported.
    let v = custody::verify(&out, &[[99u8; 32]]).unwrap();
    assert_eq!(v.trusted, Some(false));
}

#[test]
fn cli_capture_needs_a_licence_and_the_chain_is_free() {
    let d = tmp("cli");
    recording(&d);
    let home = d.join("home");
    let hex: String = [21u8; 32].iter().map(|b| format!("{b:02x}")).collect();
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={hex}\n")).unwrap();
    std::fs::write(
        d.join("req.toml"),
        request("[trigger]\nkind = \"sample\"\nsample = 20000\n"),
    )
    .unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
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
    };
    let out = d.join("c");
    let (o, k, r) = (
        out.to_str().unwrap(),
        key.to_str().unwrap(),
        d.join("req.toml"),
    );
    let r = r.to_str().unwrap();
    let (ok, m) = run(&["custody", "capture", r, "--out", o, "--key-file", k]);
    assert!(!ok, "{m}");
    let (ok, m) = run(&[
        "custody",
        "capture",
        r,
        "--out",
        o,
        "--key-file",
        k,
        "--dev",
    ]);
    assert!(ok, "{m}");
    let (ok, m) = run(&[
        "custody",
        "handover",
        o,
        "--from",
        "A",
        "--to",
        "B",
        "--purpose",
        "lab",
        "--key-file",
        k,
    ]);
    assert!(ok, "{m}");
    let (ok, m) = run(&["custody", "verify", o]);
    assert!(ok && m.contains("1 handover"), "{m}");
    let j = d.join("chain.json");
    let (ok, m) = run(&["custody", "export", o, "--json", j.to_str().unwrap()]);
    assert!(ok, "{m}");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&j).unwrap()).unwrap();
    assert_eq!(v["format"], custody::CHAIN_FORMAT);
}

/// Ground software deployment integrity and drift (`kshana-pro deploy-attest`,
/// docs/DEPLOY-ATTESTATION.md).
mod deploy_attest {
    use kshana_pro::deploy_attest::{self, Request};
    use kshana_pro::license::public_key_for;

    const SEED: [u8; 32] = [21; 32];

    fn setup(name: &str) -> (std::path::PathBuf, Request) {
        let d = std::env::temp_dir().join(format!("kshana-deploy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (rel, snaps, req) = deploy_attest::synth();
        for (p, t) in &rel {
            let f = d.join("release").join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, t).unwrap();
        }
        for (n, t) in &snaps {
            std::fs::write(d.join(n), t).unwrap();
        }
        let b = deploy_attest::baseline_of_dir(
            &d.join("release"),
            "SYN-MCS",
            "4.2.0",
            &["logs/*".to_string()],
            "2026-10-30T00:00:00Z",
        )
        .unwrap();
        assert_eq!(b.files.len(), 5);
        let bp = d.join("baseline-pack");
        deploy_attest::write_baseline(&bp, &b).unwrap();
        kshana_pro::signed_dossier::sign(&bp, &SEED, "SYNTHETIC release", "2026-10-30T00:00:00Z")
            .unwrap();
        (d, toml::from_str(&req).unwrap())
    }

    #[test]
    fn drift_timeline_and_accepted_change() {
        let (d, q) = setup("timeline");
        let (r, inputs) = deploy_attest::check(&q, &d, &[public_key_for(&SEED)]).unwrap();
        assert!(r.baseline_signature.starts_with("verified"));
        assert_eq!(inputs.len(), 7);
        let snap = |h: &str, day: &str| {
            r.snapshots
                .iter()
                .find(|s| s.host == h && s.utc.starts_with(day))
                .unwrap()
        };
        // Excluded log changes do not count.
        assert_eq!(snap("gs-a", "2026-11-01").state, "matches baseline");
        let a8 = snap("gs-a", "2026-11-08");
        assert_eq!((a8.modified, a8.added, a8.missing), (1, 1, 0));
        assert_eq!(a8.new_drift, vec!["bin/helper.sh", "etc/mcs.conf"]);
        assert_eq!(snap("gs-b", "2026-11-08").missing, 1);
        let a15 = snap("gs-a", "2026-11-15");
        assert_eq!(a15.accepted, 1);
        assert_eq!(a15.reverted, vec!["bin/helper.sh"]);
        assert!(a15.new_drift.is_empty());
        let acc = a15.findings.iter().find(|f| f.kind == "accepted").unwrap();
        assert!(acc.reference.as_deref().unwrap().contains("CR-0042"));
        assert_eq!(snap("gs-b", "2026-11-15").reverted, vec!["lib/libframe.so"]);
        let open: Vec<_> = r.episodes.iter().filter(|e| e.open).collect();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].path, "etc/mcs.conf");
        assert!((open[0].duration_days - 7.0).abs() < 1e-9);
        let gs_a = r.hosts.iter().find(|h| h.host == "gs-a").unwrap();
        assert_eq!(gs_a.latest_state, "drifted");
        assert_eq!(
            gs_a.first_drift_utc.as_deref(),
            Some("2026-11-08T06:00:00Z")
        );
        assert!(r.verdict.starts_with("1 of 2 hosts"), "{}", r.verdict);
        assert!(r.verdict.contains("1 drift findings open, 2 reverted"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn baseline_must_verify_against_a_trusted_key() {
        let (d, q) = setup("trust");
        assert!(deploy_attest::check(&q, &d, &[])
            .unwrap_err()
            .contains("--trust"));
        let e = deploy_attest::check(&q, &d, &[public_key_for(&[1; 32])]).unwrap_err();
        assert!(e.contains("does not verify"), "{e}");
        // A baseline whose manifest was edited after signing.
        let m = d.join("baseline-pack").join("manifest.json");
        let t = std::fs::read_to_string(&m)
            .unwrap()
            .replace("4.2.0", "4.2.1");
        std::fs::write(&m, t).unwrap();
        let e = deploy_attest::check(&q, &d, &[public_key_for(&SEED)]).unwrap_err();
        assert!(e.contains("manifest.json: changed"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hash_lists_in_both_forms() {
        let h = "a".repeat(64);
        let l = deploy_attest::parse_hash_list(&format!(
            "# sha256sum\n{h}  ./bin/x\n{h} *etc/y.conf\n"
        ))
        .unwrap();
        assert_eq!(l[0].path, "bin/x");
        assert_eq!(l[1].path, "etc/y.conf");
        let c =
            deploy_attest::parse_hash_list(&format!("path,sha256\nbin/x,{}\n", h.to_uppercase()))
                .unwrap();
        assert_eq!(c[0].sha256, h);
        assert!(deploy_attest::parse_hash_list("abc  x\n").is_err());
        assert!(deploy_attest::parse_hash_list(&format!("{h}  x\n{h}  ./x\n")).is_err());
        assert!(deploy_attest::glob("logs/*", "logs/a/b.log"));
        assert!(!deploy_attest::glob("*.pid", "run/x.pidfile"));
    }
}
