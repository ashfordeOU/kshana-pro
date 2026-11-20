// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Customer evidence vault, first slice (research 2026-11-20 gaps B2504): ingest checks the
//! signature; legal hold and retention block deletion; the ledger is hash-chained; trend,
//! calendar and submission skeleton; the open export round-trips and a changed byte is refused.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::vault;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b2504-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn pack(root: &Path, name: &str, format: &str, utc: &str, seed: u8) -> PathBuf {
    let d = root.join(name);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("report.json"),
        format!("{{\"format\": \"{format}\", \"n\": \"{name}\"}}"),
    )
    .unwrap();
    kshana_pro::signed_dossier::sign(&d, &[seed; 32], "Ops", utc).unwrap();
    d
}

fn files(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(r: &Path, d: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(r, &p, out);
            } else {
                out.push((
                    p.strip_prefix(r).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    let mut v = Vec::new();
    walk(root, root, &mut v);
    v.sort();
    v
}

#[test]
fn hold_blocks_deletion_and_export_round_trips() {
    let d = tmp("vault");
    let v = d.join("vault");
    vault::init(&v, "2026-01-01T00:00:00Z").unwrap();
    assert!(vault::init(&v, "2026-01-01T00:00:00Z").is_err());
    let a = pack(&d, "a", "kshana-pro-grid-pmu/1", "2026-01-15T00:00:00Z", 1);
    let b = pack(
        &d,
        "b",
        "kshana-pro-timing-audit/1",
        "2026-02-15T00:00:00Z",
        1,
    );
    let c = pack(
        &d,
        "c",
        "kshana-pro-ais-integrity/1",
        "2026-02-20T00:00:00Z",
        2,
    );
    let trust = [kshana_pro::license::public_key_for(&[1u8; 32])];
    let ea = vault::ingest(&v, &a, &["q1".into()], 1, &trust, "2026-03-01T00:00:00Z").unwrap();
    assert_eq!(ea.trusted_at_ingest, Some(true));
    assert_eq!(ea.retain_until_utc, "2027-03-01T00:00:00Z");
    vault::ingest(&v, &b, &[], 1, &trust, "2026-03-01T00:00:00Z").unwrap();
    let ec = vault::ingest(&v, &c, &[], 1, &trust, "2026-03-01T00:00:00Z").unwrap();
    assert_eq!(
        ec.trusted_at_ingest,
        Some(false),
        "an untrusted key is recorded, not refused"
    );
    assert!(
        vault::ingest(&v, &a, &[], 1, &[], "2026-03-02T00:00:00Z").is_err(),
        "no duplicate"
    );
    // A tampered pack is refused.
    let t = pack(&d, "t", "kshana-pro-grid-pmu/1", "2026-02-01T00:00:00Z", 1);
    std::fs::write(
        t.join("report.json"),
        "{\"format\": \"kshana-pro-grid-pmu/1\"}",
    )
    .unwrap();
    assert!(vault::ingest(&v, &t, &[], 1, &[], "2026-03-01T00:00:00Z").is_err());

    // Retention and legal hold block deletion.
    assert!(
        vault::delete(&v, &ea.id[..12], "2026-06-01T00:00:00Z").is_err(),
        "retained"
    );
    vault::hold(
        &v,
        &ea.id[..12],
        "litigation 2027-01",
        "2027-02-01T00:00:00Z",
    )
    .unwrap();
    let err = vault::delete(&v, &ea.id[..12], "2027-06-01T00:00:00Z").unwrap_err();
    assert!(err.contains("legal hold"), "{err}");
    vault::release(&v, &ea.id[..12], "case closed", "2027-06-02T00:00:00Z").unwrap();
    vault::delete(&v, &ea.id[..12], "2027-06-03T00:00:00Z").unwrap();
    assert_eq!(vault::list(&v).unwrap().len(), 2);
    assert!(vault::check_ledger(&v).unwrap().is_empty());

    // Trend, calendar, submission.
    let tr = vault::trend(&v).unwrap();
    assert_eq!(tr["2026-02"]["timing"], 1);
    assert_eq!(tr["2026-02"]["ais"], 1);
    let ob = vec![vault::Obligation {
        name: "monthly timing audit".into(),
        every: "monthly".into(),
        first_due: "2026-01-31".into(),
        served_by: vec!["kshana-pro-timing-".into()],
    }];
    let cal = vault::calendar(&v, &ob, "2026-02-20T00:00:00Z").unwrap();
    assert_eq!(cal[0].next_due, "2026-02-28T00:00:00Z");
    assert!(
        cal[0].served_this_period,
        "signed 15 February, after the 31 January due date"
    );
    let cal = vault::calendar(&v, &ob, "2026-03-10T00:00:00Z").unwrap();
    assert_eq!(cal[0].next_due, "2026-03-31T00:00:00Z", "month end kept");
    assert!(
        !cal[0].served_this_period,
        "nothing signed since 28 February"
    );
    let ids: Vec<String> = vault::list(&v)
        .unwrap()
        .iter()
        .map(|e| e.id[..10].to_string())
        .collect();
    let md = vault::submission(&v, "Annual return", &ids).unwrap();
    assert!(md.contains("kshana-pro-ais-integrity/1") && md.contains("[to fill]"));

    // Export round-trips; a changed byte is refused on import.
    let x = d.join("export");
    vault::export(&v, &x, "2027-07-01T00:00:00Z").unwrap();
    let back = d.join("back");
    vault::import(&x, &back).unwrap();
    assert_eq!(files(&v), files(&back));
    let ls = files(&x);
    let (victim, _) = ls.iter().find(|(p, _)| p.ends_with("report.json")).unwrap();
    let mut bytes = std::fs::read(x.join(victim)).unwrap();
    bytes[0] ^= 1;
    std::fs::write(x.join(victim), bytes).unwrap();
    assert!(vault::import(&x, &d.join("back2")).is_err());

    // A changed ledger record shows.
    let lp = v.join("ledger.json");
    let t = std::fs::read_to_string(&lp)
        .unwrap()
        .replacen("litigation", "LITIGATION", 1);
    std::fs::write(&lp, t).unwrap();
    assert!(!vault::check_ledger(&v).unwrap().is_empty());
}

#[test]
fn cli_is_enterprise_and_import_is_free() {
    let d = tmp("cli");
    let home = d.join("home");
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let v = d.join("v");
    let vs = v.to_str().unwrap();
    let (ok, _) = run(&["vault", "init", vs]);
    assert!(!ok, "needs a licence");
    let (ok, m) = run(&["vault", "init", vs, "--dev"]);
    assert!(ok, "{m}");
    let p = pack(&d, "p", "kshana-pro-grid-pmu/1", "2026-01-15T00:00:00Z", 3);
    let (ok, m) = run(&[
        "vault",
        "ingest",
        vs,
        p.to_str().unwrap(),
        "--tag",
        "x",
        "--dev",
    ]);
    assert!(ok, "{m}");
    let x = d.join("x");
    let (ok, m) = run(&["vault", "export", vs, x.to_str().unwrap(), "--dev"]);
    assert!(ok, "{m}");
    let b = d.join("b");
    let (ok, m) = run(&["vault", "import", x.to_str().unwrap(), b.to_str().unwrap()]);
    assert!(ok, "{m}");
    let (ok, m) = run(&["vault", "verify", b.to_str().unwrap(), "--dev"]);
    assert!(ok, "{m}");
}
