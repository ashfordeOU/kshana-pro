// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Third-party interference product benchmark (research 2026-11-20 gaps B1005) on a SYNTHETIC
//! fixture with two anonymised products: detection rate, latency, false reports per hour and
//! affected-area accuracy match hand-computed values; a changed protocol is refused; a report
//! carrying an emitter estimate is refused, never scored.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::product_bench::{self, Protocol, Request};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b1005-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn setup(d: &Path) -> Request {
    let (t, a, b, proto) = product_bench::synth();
    std::fs::write(d.join("truth.json"), t).unwrap();
    std::fs::write(d.join("product-a.json"), a).unwrap();
    std::fs::write(d.join("product-b.json"), b).unwrap();
    let p: Protocol = toml::from_str(&proto).unwrap();
    Request {
        title: "t".into(),
        protocol_sha256: product_bench::protocol_sha256(&p),
        protocol: p,
        truth: "truth.json".into(),
        products: vec![
            product_bench::Product {
                id: "PRODUCT-A".into(),
                reports: "product-a.json".into(),
            },
            product_bench::Product {
                id: "PRODUCT-B".into(),
                reports: "product-b.json".into(),
            },
        ],
    }
}

#[test]
fn metrics_match_hand_computed_values() {
    let d = tmp("m");
    let q = setup(&d);
    let b = product_bench::score(&q, &d).unwrap();
    let a = &b.products[0];
    assert_eq!(a.detection_rate, 1.0);
    assert_eq!(a.median_latency_s, Some(40.0));
    assert_eq!(a.worst_latency_s, Some(60.0));
    assert_eq!(a.false_reports, 1);
    assert_eq!(a.area_iou, Some(0.833));
    assert_eq!(a.events[2].area_iou, Some(0.5));
    assert!(a.bars.values().all(|m| *m), "{:?}", a.bars);
    let p = &b.products[1];
    assert_eq!(p.detection_rate, 0.667);
    assert_eq!(p.median_latency_s, Some(120.0));
    assert_eq!(p.false_reports, 0);
    assert_eq!(p.area_iou, Some(0.625));
    assert!(!p.events[1].detected);
    assert!(!p.bars["min_detection_rate"]);
    assert!(p.bars["max_median_latency_s"]);
    // No vendor named in the fixture.
    let j = serde_json::to_string(&b).unwrap();
    assert!(j.contains("PRODUCT-A") && b.referee.contains("unchanged"));
}

#[test]
fn changed_protocols_and_emitter_estimates_are_refused() {
    let d = tmp("r");
    let mut q = setup(&d);
    q.protocol.min_area_iou = Some(0.1);
    assert!(product_bench::score(&q, &d)
        .unwrap_err()
        .contains("changed after registration"));
    let q = setup(&d);
    std::fs::write(
        d.join("product-a.json"),
        r#"[{"utc":"2026-06-01T08:10:20Z","affected_cells":[],"emitter":{"lat":50.0,"lon":1.0}}]"#,
    )
    .unwrap();
    let e = product_bench::score(&q, &d).unwrap_err();
    assert!(e.contains("emitter estimates are not accepted"), "{e}");
}

#[test]
fn register_is_free_and_score_is_signed() {
    let d = tmp("cli");
    let home = d.join("home");
    let run = |a: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(a)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr),
        )
    };
    let (ok, m) = run(&["product-bench", "synth", d.to_str().unwrap()]);
    assert!(ok, "{m}");
    let (ok, m) = run(&[
        "product-bench",
        "register",
        d.join("protocol.toml").to_str().unwrap(),
    ]);
    assert!(
        ok && m.contains(&product_bench::protocol_sha256(
            &toml::from_str(&std::fs::read_to_string(d.join("protocol.toml")).unwrap()).unwrap()
        )),
        "{m}"
    );
    let seed = [8u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("key.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("card");
    let (r, kf) = (d.join("request.toml"), d.join("key.txt"));
    let args = [
        "product-bench",
        "score",
        r.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        kf.to_str().unwrap(),
        "--utc",
        "2026-11-20T00:00:00Z",
    ];
    assert!(!run(&args).0, "needs a licence");
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(ok, "{m}");
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}
