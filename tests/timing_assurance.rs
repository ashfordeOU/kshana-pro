// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Timing assurance (roadmap 0.42 T1-T3) on the recorded demo fleet
//! (`examples/timing-fleet/`: two real IGS clocks, the rest synthetic logs): every clock of a
//! mixed fleet is read from its own source and modelled; the licence gates the kind and both
//! features; the evidence history is hash-chained; the page, the metrics and the Grafana
//! dashboard agree; an audit report is signed, verifies with the free path, and never claims
//! conformity.

use std::path::{Path, PathBuf};

use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::timing::{self, audit, console, evidence, fleet, metrics};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn demo() -> fleet::FleetSpec {
    let p = root().join("examples/timing-fleet/fleet.toml");
    let mut spec = fleet::FleetSpec::parse(&std::fs::read_to_string(&p).unwrap()).unwrap();
    spec.resolve_paths(p.parent().unwrap());
    spec
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-timing-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn pro() -> Entitlement {
    Entitlement {
        licensee: "test".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

#[test]
fn every_clock_of_the_mixed_fleet_is_read_from_its_own_source_and_modelled() {
    let a = fleet::assess(&demo()).unwrap();
    assert_eq!(a.counts.clocks, 8);
    let formats: std::collections::BTreeSet<&str> =
        a.clocks.iter().map(|c| c.source_format.as_str()).collect();
    for f in [
        "timelab",
        "ticc",
        "ptp4l",
        "chrony-tracking",
        "wr-stat",
        "ntp-loopstats",
        "pps-assert",
    ] {
        assert!(formats.contains(f), "{f} not read");
    }
    for c in &a.clocks {
        assert!(c.n_used >= 16, "{}", c.id);
        assert!(["ok", "watch", "act", "breached"].contains(&c.status.as_str()));
        let measured = c.noise_source == timing::model::NoiseSource::Measured;
        assert_eq!(measured, c.record == "free-running", "{}", c.id);
        assert_eq!(c.prior.is_some(), !measured, "{}", c.id);
        // The bound grows with the horizon.
        let s: Vec<f64> = c.predictions.iter().map(|p| p.sigma_s).collect();
        assert!(s.windows(2).all(|w| w[1] >= w[0]), "{}: {s:?}", c.id);
        for b in &c.breaches {
            if let (Some(w), Some(m)) = (b.t_warn_s, b.t_50_s) {
                assert!(w <= m, "{}", c.id);
            }
        }
    }
    // A clock with a subset of limits gets only those.
    let srv = a.clock("srv-ldn-3").unwrap();
    assert_eq!(srv.breaches.len(), 1);
    // Ordering: earliest warning first.
    let warns: Vec<Option<f64>> = a
        .order
        .iter()
        .map(|id| a.clock(id).unwrap().worst_warn_s())
        .collect();
    let first_none = warns
        .iter()
        .position(|w| w.is_none())
        .unwrap_or(warns.len());
    assert!(warns[first_none..].iter().all(|w| w.is_none()));
    let some: Vec<f64> = warns[..first_none].iter().map(|w| w.unwrap()).collect();
    assert!(some.windows(2).all(|w| w[0] <= w[1]));
    assert!(a.label.starts_with("MODELLED"));
    assert!(a.label.contains("V5"));
}

#[test]
fn a_servo_record_without_a_prior_is_refused_with_a_reason() {
    let mut spec = demo();
    spec.clocks.retain(|c| c.id == "bc-ldn-2");
    spec.clocks[0].class = None;
    let e = fleet::assess(&spec).unwrap_err().to_string();
    assert!(e.contains("servo offset") && e.contains("class"), "{e}");
}

#[test]
fn limits_are_required_and_ids_are_plain() {
    let base = "kind = \"pro-timing-holdover\"\n[[clocks]]\nid = \"a\"\n[clocks.source]\nformat = \"timelab\"\ntext = \"1\\n2\\n\"\n";
    assert!(fleet::FleetSpec::parse(base).is_err());
    let bad_id =
        base.replace("id = \"a\"", "id = \"../x\"") + "[[limits]]\nname = \"l\"\nvalue_s = 1e-6\n";
    assert!(fleet::FleetSpec::parse(&bad_id).is_err());
}

#[test]
fn the_kind_and_both_features_are_licence_gated_at_pro() {
    let src = include_str!("../scenarios/timing-holdover-inline.toml");
    let now = 1_800_000_000;
    let e = kshana_pro::registry::run_gated(src, &Entitlement::evaluation(), now).unwrap_err();
    assert!(e.to_string().contains("'pro' tier"), "{e}");
    let out = kshana_pro::registry::run_gated(src, &pro(), now).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
    assert_eq!(v["kind"], "pro-timing-holdover");
    assert_eq!(v["counts"]["clocks"], 2);
    for f in [timing::KIND, timing::FLEET_FEATURE, timing::AUDIT_FEATURE] {
        assert!(kshana_pro::license::check(&Entitlement::evaluation(), f, now).is_err());
        assert!(kshana_pro::license::check(&pro(), f, now).is_ok(), "{f}");
        assert_eq!(
            kshana_pro::license::tiers::edition_of(f),
            Some(kshana_pro::license::tiers::Edition::Pro)
        );
    }
}

#[test]
fn evidence_history_is_hash_chained_and_a_change_is_caught() {
    let dir = tmp("evidence");
    let a = fleet::assess(&demo()).unwrap();
    evidence::append(&dir, &a, "2026-11-15T09:00:00Z").unwrap();
    evidence::append(&dir, &a, "2026-11-15T10:00:00Z").unwrap();
    let h = evidence::read(&dir, "gm-ldn-1").unwrap();
    assert_eq!(h.len(), 2);
    assert!(evidence::verify_chain(&h).is_empty());
    assert_eq!(h[1].prev_sha256, h[0].sha256);
    // Change one figure of the first entry: the chain breaks, and appending is refused.
    let p = dir.join("gm-ldn-1.jsonl");
    let text = std::fs::read_to_string(&p).unwrap();
    let first = text.lines().next().unwrap();
    let mut e: serde_json::Value = serde_json::from_str(first).unwrap();
    e["status"] = serde_json::json!("ok-edited");
    let changed = text.replacen(first, &e.to_string(), 1);
    std::fs::write(&p, changed).unwrap();
    let h = evidence::read(&dir, "gm-ldn-1").unwrap();
    assert!(!evidence::verify_chain(&h).is_empty());
    assert!(evidence::append(&dir, &a, "2026-11-15T11:00:00Z").is_err());
    // Dropping an entry also breaks it.
    let other = std::fs::read_to_string(dir.join("bc-ldn-2.jsonl")).unwrap();
    let second: String = other.lines().skip(1).map(|l| format!("{l}\n")).collect();
    std::fs::write(dir.join("bc-ldn-2.jsonl"), second).unwrap();
    assert!(!evidence::verify_chain(&evidence::read(&dir, "bc-ldn-2").unwrap()).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_fleet_page_metrics_and_grafana_dashboard_agree() {
    let a = fleet::assess(&demo()).unwrap();
    let dir = tmp("page");
    evidence::append(&dir, &a, "2026-11-15T09:00:00Z").unwrap();
    let hist: Vec<_> = a
        .clocks
        .iter()
        .map(|c| (c.id.clone(), evidence::read(&dir, &c.id).unwrap()))
        .collect();
    let page = console::page(&a, &hist, "test");
    for c in &a.clocks {
        assert!(page.contains(&format!("id=\"c-{}\"", c.id)));
    }
    assert!(page.contains("Evidence history"));
    assert!(
        !page.contains("http://") && !page.contains("https://"),
        "the page is self-contained"
    );
    let prom = metrics::exposition(&a);
    let names: std::collections::BTreeSet<&str> = prom
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('{').next().unwrap())
        .collect();
    for line in prom
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let v = line.rsplit(' ').next().unwrap();
        assert!(v == "+Inf" || v.parse::<f64>().is_ok(), "{line}");
    }
    let dash: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("deploy/grafana/kshana-pro-timing-fleet.json"))
            .unwrap(),
    )
    .unwrap();
    let mut used = 0;
    for p in dash["panels"].as_array().unwrap() {
        for t in p["targets"].as_array().unwrap() {
            let expr = t["expr"].as_str().unwrap();
            for word in expr.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                if word.starts_with("kshana_pro_timing_") {
                    assert!(names.contains(word), "dashboard uses {word}, not exported");
                    used += 1;
                }
            }
        }
    }
    assert!(used >= 8);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn audit_reports_are_signed_verify_for_free_and_say_mapped_to_only() {
    let spec = demo();
    let a = fleet::assess(&spec).unwrap();
    let ev = tmp("audit-ev");
    evidence::append(&ev, &a, "2026-11-15T09:00:00Z").unwrap();
    let hist: Vec<_> = a
        .clocks
        .iter()
        .map(|c| (c.id.clone(), evidence::read(&ev, &c.id).unwrap()))
        .collect();
    let seed = [7u8; 32];
    let pk = kshana_pro::license::public_key_for(&seed);
    for profile in ["rts25-traceability", "nist-ir-8323-profile"] {
        let op = audit::OperatorStatement {
            organisation: Some("Example operator".into()),
            utc_source: Some("UTC(k) via GNSS-disciplined grandmasters".into()),
            ..Default::default()
        };
        let r = audit::build(&spec, &a, &hist, profile, &op, "2026-11-15T09:30:00Z").unwrap();
        assert!(!r.items.is_empty());
        let out = tmp(&format!("audit-{profile}"));
        audit::write_dir(&out, &r, &a, &hist).unwrap();
        kshana_pro::signed_dossier::sign(&out, &seed, "Example signer", "2026-11-15T09:30:00Z")
            .unwrap();
        let v = kshana_pro::signed_dossier::verify(&out, &[pk]).unwrap();
        assert!(v.passed(), "{:?}", v.problems);
        for f in ["report.html", "report.json", "README.txt"] {
            let text = std::fs::read_to_string(out.join(f)).unwrap().to_lowercase();
            assert!(text.contains("mapped to"), "{profile} {f}");
            for w in audit::FORBIDDEN_WORDS {
                assert!(!text.contains(w), "{profile} {f} uses '{w}'");
            }
        }
        // A changed byte anywhere fails verification.
        let html = out.join("report.html");
        let mut t = std::fs::read_to_string(&html).unwrap();
        t.push(' ');
        std::fs::write(&html, t).unwrap();
        assert!(!kshana_pro::signed_dossier::verify(&out, &[pk])
            .unwrap()
            .passed());
        let _ = std::fs::remove_dir_all(&out);
    }
    // The RTS 25 divergence block uses the open engine's assessment for servo records.
    let r = audit::build(
        &spec,
        &a,
        &hist,
        "rts25-traceability",
        &Default::default(),
        "t",
    )
    .unwrap();
    let div = r
        .evidence
        .iter()
        .find(|b| b.key == "clock.divergence")
        .unwrap();
    assert!(div
        .rows
        .iter()
        .any(|row| row[4] == "within limit" || row[4] == "beyond limit"));
    assert!(div.rows.iter().any(|row| row[4] == "not assessed"));
    assert!(audit::build(&spec, &a, &hist, "iso-9001", &Default::default(), "t").is_err());
    let _ = std::fs::remove_dir_all(&ev);
}

#[test]
fn the_sample_report_verifies_with_its_published_demo_key() {
    let dir = root().join("report-templates/samples/timing-audit-rts25");
    let key =
        std::fs::read_to_string(dir.join("../timing-audit-rts25.demo-public-key.txt")).unwrap();
    let pk = kshana_pro::license::parse_key_hex(key.trim()).unwrap();
    let v = kshana_pro::signed_dossier::verify(&dir, &[pk]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

fn get(a: std::net::SocketAddr, path: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(a).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n"
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    (
        out[9..12].parse().unwrap(),
        out.split_once("\r\n\r\n").unwrap().1.to_string(),
    )
}

#[test]
fn the_station_console_serves_the_fleet_page_and_metrics_under_the_licence() {
    use kshana_pro::serve::{self, ServeConfig};
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("timing-console-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let start = |tier: Tier, dir: &Path| {
        let mut cfg = ServeConfig::new("127.0.0.1:0", Entitlement { tier, ..pro() });
        cfg.data_dir = Some(dir.to_path_buf());
        serve::start(cfg).unwrap()
    };
    // No fleet configured yet: 404.
    let h = start(Tier::Pro, &d);
    assert_eq!(get(h.local_addr(), "/timing/fleet").0, 404);
    // The demo fleet as the default tenant's fleet, with one evidence entry per clock.
    let t = d.join("timing").join("default");
    copy_dir(&root().join("examples/timing-fleet"), &t);
    let mut spec =
        fleet::FleetSpec::parse(&std::fs::read_to_string(t.join("fleet.toml")).unwrap()).unwrap();
    spec.resolve_paths(&t);
    evidence::append(
        &t.join("evidence"),
        &fleet::assess(&spec).unwrap(),
        "2026-11-15T09:00:00Z",
    )
    .unwrap();
    let (st, page) = get(h.local_addr(), "/timing/fleet");
    assert_eq!(st, 200);
    assert!(page.contains("gm-ldn-1") && page.contains("Evidence history"));
    assert!(page.contains("2026-11-15T09:00:00Z"));
    let (st, prom) = get(h.local_addr(), "/timing/metrics");
    assert_eq!(st, 200);
    assert!(prom.contains("kshana_pro_timing_breach_warn_seconds{"));
    h.stop();
    // Below Pro: refused.
    let h = start(Tier::Evaluation, &d);
    assert_eq!(get(h.local_addr(), "/timing/fleet").0, 403);
    assert_eq!(get(h.local_addr(), "/timing/metrics").0, 403);
    h.stop();
    let _ = std::fs::remove_dir_all(&d);
}
