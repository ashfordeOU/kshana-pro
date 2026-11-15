// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Roadmap 0.42 fusion and operations: F1 area console (two-site demo, tenancy, service page,
//! HA failover), F2 (no emitter positions: a hard switch), F3 multi-source trust (the rule's
//! behaviour, evidence, command line), F5 alert policy engine (routes, quiet hours, escalation,
//! de-duplication; raw alerts free), F6 planner Pareto front with cost (B1b).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use kshana_pro::alerts::{self, Policy, Severity};
use kshana_pro::area::{self, Fused, Inputs, PositionSwitch, EMITTER_POSITIONS};
use kshana_pro::license::{Entitlement, Tier};
use kshana_pro::multi_source::{self, Choice, Config, Obs, Selector, Status, Verdict};
use kshana_pro::planner_front::{self, PlannerFrontScenario};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fusion-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ent(tier: Tier) -> Entitlement {
    Entitlement {
        licensee: "ACME".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

fn cli(home: &Path, args: &[&str]) -> (Option<i32>, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .current_dir(root())
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        o.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

const T0: i64 = 1_794_614_400;

fn demo() -> area::Picture {
    let i = Inputs::load(&root().join("examples/area-console/console.toml")).unwrap();
    area::picture(&i)
}

// ------------------------------------------------------------------ F1

#[test]
fn f1_two_site_demo_fuses_stations_receivers_and_feeds_into_events() {
    let p = demo();
    assert_eq!(p.areas.len(), 2);
    let ev: Vec<(&str, i64, i64, Fused, &str, bool)> = p
        .events()
        .iter()
        .map(|e| {
            (
                e.id.as_str(),
                e.start_g - T0,
                e.duration_s,
                e.worst,
                e.basis.as_str(),
                e.corroborated,
            )
        })
        .collect();
    assert_eq!(
        ev,
        vec![
            (
                "gulf-of-finland-1",
                630,
                879,
                Fused::Degraded,
                "network+feed",
                true
            ),
            (
                "tartu-approach-1",
                660,
                669,
                Fused::Degraded,
                "network+feed",
                true
            ),
            (
                "tartu-approach-2",
                3000,
                121,
                Fused::Degraded,
                "feed",
                false
            ),
        ]
    );
    // The wide-area event links the two areas' simultaneous events.
    assert_eq!(p.wide_area.len(), 1);
    assert_eq!(
        p.wide_area[0].areas,
        vec!["gulf-of-finland", "tartu-approach"]
    );
    let gulf = &p.areas[0];
    assert_eq!(gulf.events[0].wide_area.as_deref(), Some("wide-1"));
    assert_eq!(
        gulf.events[0].sources,
        vec!["gof-st-1", "gof-st-2", "gof-st-3", "gof-rx-1"]
    );
    // The receiver's own untrusted spell (minutes 40-45) is one source of four: the open area
    // rule (v2) does not raise the area, so it is no event.
    assert!(gulf.events.iter().all(|e| e.end_g < T0 + 40 * 60));
    // The low-confidence feed detection is left out.
    assert!(gulf
        .events
        .iter()
        .all(|e| e.detections.iter().all(|d| d.confidence >= 0.5)));
    assert_eq!(gulf.current, Fused::Trusted);
    // Reproducible: the same bytes twice.
    assert_eq!(demo().to_json(), p.to_json());
    // The service page renders every area and event.
    let page = area::page(&p, "");
    for id in [
        "gulf-of-finland-1",
        "tartu-approach-2",
        "wide-1",
        "(uncorroborated)",
    ] {
        assert!(page.contains(id), "{id}");
    }
}

#[test]
fn f1_tenancy_filters_and_more_than_one_tenant_needs_enterprise() {
    let text = std::fs::read_to_string(root().join("examples/area-console/console.toml"))
        .unwrap()
        .replacen(
            "name = \"Tartu approach\"",
            "name = \"Tartu approach\"\ntenant = \"tartu-city\"",
            1,
        );
    let i = Inputs::load_text(&text, &root().join("examples/area-console")).unwrap();
    assert_eq!(i.spec.tenants(), vec!["default", "tartu-city"]);
    let p = area::picture(&i);
    let t = p.for_tenant("tartu-city");
    assert_eq!(t.areas.len(), 1);
    assert_eq!(t.events().len(), 2);
    assert_eq!(t.wide_area.len(), 1);
    assert_eq!(t.wide_area[0].areas, vec!["tartu-approach"]);
    assert_eq!(t.wide_area[0].events, vec!["tartu-approach-1"]);
    assert!(!t.to_json().contains("gulf-of-finland"));
    assert!(!area::multi_tenant_allowed(&ent(Tier::Pro)));
    assert!(area::multi_tenant_allowed(&ent(Tier::Enterprise)));
}

#[test]
fn f1_console_files_are_checked() {
    let base = root().join("examples/area-console");
    let good = std::fs::read_to_string(base.join("console.toml")).unwrap();
    for (bad, why) in [
        (good.replace("kshana-pro-area-console/1", "x/1"), "format"),
        (
            good.replacen("area = \"tartu-approach\"", "area = \"nowhere\"", 1),
            "no area",
        ),
        (
            good.replacen(
                "file = \"sources/gof-st-1.csv\"",
                "file = \"../../Cargo.toml\"",
                1,
            ),
            "relative path",
        ),
        (
            good.replacen(
                "provider = \"customer-supplied national network feed (demo, synthetic)\"\n",
                "",
                1,
            ),
            "provider",
        ),
    ] {
        let e = Inputs::load_text(&bad, &base).unwrap_err();
        assert!(e.contains(why), "{why}: {e}");
    }
}

// ------------------------------------------------------------------ F2

#[test]
fn f2_emitter_positions_are_a_hard_switch_that_is_off() {
    // The switch has exactly one value; this match stops compiling if another is added.
    match EMITTER_POSITIONS {
        PositionSwitch::Off => {}
    }
    assert_eq!(EMITTER_POSITIONS, PositionSwitch::Off);
    // No code path names an "on" value, and nothing reads a setting for it.
    let src = std::fs::read_to_string(root().join("src/area/mod.rs")).unwrap();
    assert!(!src.contains("PositionSwitch::On"));
    assert!(!src.contains("env::var"));
    // A console cannot ask for positions: unknown keys are refused.
    let base = root().join("examples/area-console");
    let good = std::fs::read_to_string(base.join("console.toml")).unwrap();
    for key in ["show_positions = true", "emitter_positions = \"on\""] {
        let bad = good.replacen("title =", &format!("{key}\ntitle ="), 1);
        assert!(Inputs::load_text(&bad, &base).is_err(), "{key}");
    }
    // The roadmap's condition for turning it on has not been met: no published JammerTest 2027
    // locator result exists in this repository.
    assert!(!root().join("docs/evidence/b3c-jammertest-2027").exists());
}

#[test]
fn f2_feed_emitter_estimates_are_withheld_from_every_output() {
    let p = demo();
    assert_eq!(p.positions.emitter_positions, "off");
    assert_eq!(p.positions.withheld, 2);
    let json = p.to_json();
    let page = area::page(&p, "");
    let alerts = alerts::raw_log(&alerts::from_picture(&p));
    // The Tartu feed carries 58.38/26.72 and 58.31/26.95; none of them appears anywhere.
    for coord in ["58.38", "26.72", "58.31", "26.95", "lat_deg", "lon_deg"] {
        assert!(!json.contains(coord), "{coord} in the picture");
        assert!(!page.contains(coord), "{coord} in the page");
        assert!(!alerts.contains(coord), "{coord} in the alerts");
    }
    assert!(page.contains("Emitter positions: off."));
    // A malformed estimate is still an error (the field is validated, then dropped).
    let e = area::parse_feed("f", "1,2,jamming,0.9,north,east\n", 0.5).unwrap_err();
    assert!(e.contains("lat_deg"), "{e}");
}

// ------------------------------------------------------------------ F1 service and HA

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

fn install_console(data: &Path, tenant: &str) {
    let to = data.join("area").join(tenant);
    std::fs::create_dir_all(to.join("sources")).unwrap();
    let from = root().join("examples/area-console");
    std::fs::copy(from.join("console.toml"), to.join("console.toml")).unwrap();
    for e in std::fs::read_dir(from.join("sources")).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join("sources").join(e.file_name())).unwrap();
    }
}

#[test]
fn f1_service_serves_the_console_under_the_licence_and_survives_failover() {
    use kshana_pro::serve::{self, ha, ServeConfig};
    let d = tmp("svc");
    let start = |tier: Tier| {
        let mut cfg = ServeConfig::new("127.0.0.1:0", ent(tier));
        cfg.data_dir = Some(d.clone());
        serve::start(cfg).unwrap()
    };
    let h = start(Tier::Pro);
    assert_eq!(get(h.local_addr(), "/area/console").0, 404);
    install_console(&d, "default");
    let (st, page) = get(h.local_addr(), "/area/console");
    assert_eq!(st, 200);
    assert!(page.contains("Gulf of Finland approaches") && page.contains("wide-1"));
    let (st, json) = get(h.local_addr(), "/area/picture");
    assert_eq!(st, 200);
    let served: area::Picture = serde_json::from_str(&json).unwrap();
    assert_eq!(served, demo());
    h.stop();
    // Below Pro: refused.
    let h = start(Tier::Evaluation);
    assert_eq!(get(h.local_addr(), "/area/console").0, 403);
    h.stop();
    // Two tenants in one console: Pro is refused, Enterprise serves it.
    let multi = std::fs::read_to_string(d.join("area/default/console.toml"))
        .unwrap()
        .replacen(
            "name = \"Tartu approach\"",
            "name = \"Tartu approach\"\ntenant = \"t2\"",
            1,
        );
    std::fs::write(d.join("area/default/console.toml"), &multi).unwrap();
    let h = start(Tier::Pro);
    assert_eq!(get(h.local_addr(), "/area/console").0, 403);
    h.stop();
    install_console(&d, "default");

    // HA: two Enterprise nodes on one data directory; the lease holder fails, the other node
    // takes over and serves the same picture.
    let node = |name: &str| {
        let mut cfg = ServeConfig::new("127.0.0.1:0", ent(Tier::Enterprise));
        cfg.data_dir = Some(d.clone());
        cfg.ha = Some(ha::HaConfig::new(name, 600).unwrap());
        serve::start(cfg).unwrap()
    };
    let na = node("node-a");
    let t = Instant::now();
    while ha::read_lease(&d).map(|l| l.node) != Some("node-a".into()) {
        assert!(t.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(20));
    }
    let nb = node("node-b");
    let before = get(na.local_addr(), "/area/picture");
    assert_eq!(before.0, 200);
    na.abandon();
    let t = Instant::now();
    while ha::read_lease(&d).map(|l| l.node) != Some("node-b".into()) {
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "node-b takes the lease"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(get(nb.local_addr(), "/area/picture"), before);
    assert_eq!(get(nb.local_addr(), "/area/console").0, 200);
    nb.stop();
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ F3

fn obs(status: Status, ns: f64) -> Obs {
    Obs {
        status,
        offset_s: ns * 1e-9,
    }
}

/// One epoch of the V4 sources: osnma, gnss, leo, ptp, eloran (offsets in ns; `None` unavailable).
fn epoch(v: [Option<f64>; 5], osnma_failed: bool) -> Vec<Obs> {
    v.iter()
        .enumerate()
        .map(|(i, o)| match o {
            _ if i == 0 && osnma_failed => obs(Status::Failed, 0.0),
            Some(x) => obs(Status::Ok, *x),
            None => obs(Status::Unavailable, 0.0),
        })
        .collect()
}

#[test]
fn f3_cross_check_leaves_a_pushed_gnss_and_osnma_failure_blocks_the_group() {
    let mut s = Selector::new(Config::v4("rafs", 1.0)).unwrap();
    // Nominal: OSNMA (best ranked) is chosen.
    for t in 0..20 {
        let d = s.step(
            t as f64,
            &epoch([Some(0.0), Some(1.0), None, Some(2.0), Some(-3.0)], false),
        );
        assert_eq!(d.choice, Choice::Source(0));
        assert!(d.pl_s.unwrap() > 0.0);
    }
    // Both GNSS sources pushed by 400 ns (OSNMA does not flag it): left at once, for PTP.
    let d = s.step(
        20.0,
        &epoch(
            [Some(400.0), Some(401.0), None, Some(0.0), Some(1.0)],
            false,
        ),
    );
    assert_eq!(d.choice, Choice::Source(3));
    assert_eq!(d.reason, "current source failed the cross-check");
    assert!(matches!(d.verdicts[0], Verdict::NotQualified { .. }));
    // OSNMA fails: the whole group is out until OSNMA is ok for 60 s.
    let d = s.step(
        21.0,
        &epoch([Some(0.0), Some(0.0), None, Some(0.0), Some(0.0)], true),
    );
    assert_eq!(d.verdicts[0], Verdict::AuthFailed);
    assert_eq!(d.verdicts[1], Verdict::AuthRecovering);
    for t in 22..81 {
        let d = s.step(
            t as f64,
            &epoch([Some(0.0), Some(0.0), None, Some(0.0), Some(0.0)], false),
        );
        assert_ne!(d.choice, Choice::Source(0), "t={t}");
        assert_eq!(d.verdicts[0], Verdict::AuthRecovering);
    }
    // Recovered at 82 (ok since 22); then held for max(2 tau0, 10 s) before switching up.
    let mut up = None;
    for t in 81..100 {
        let d = s.step(
            t as f64,
            &epoch([Some(0.0), Some(0.0), None, Some(0.0), Some(0.0)], false),
        );
        if d.choice == Choice::Source(0) && up.is_none() {
            up = Some(t);
        }
    }
    assert_eq!(up, Some(92));
}

#[test]
fn f3_falls_back_to_holdover_then_none_is_never_needed_once_anchored() {
    let mut s = Selector::new(Config::v4("rafs", 1.0)).unwrap();
    // No source at all from the start: no output.
    let d = s.step(0.0, &epoch([None; 5], false));
    assert_eq!(d.choice, Choice::None);
    assert_eq!(d.offset_s, None);
    for t in 1..2000 {
        s.step(
            t as f64,
            &epoch([Some(5.0), Some(5.0), None, Some(5.0), Some(5.0)], false),
        );
    }
    // Everything lost: holdover, with an offset near the last outputs and a growing PL.
    let d1 = s.step(2000.0, &epoch([None; 5], false));
    assert_eq!(d1.choice, Choice::Holdover);
    assert!((d1.offset_s.unwrap() - 5e-9).abs() < 5e-9);
    let d2 = s.step(3500.0, &epoch([None; 5], false));
    assert!(d2.pl_s.unwrap() > d1.pl_s.unwrap());
    // A single source back, agreeing with the holdover: it qualifies on the holdover's vote.
    let d3 = s.step(3501.0, &epoch([None, None, None, Some(5.0), None], false));
    assert_eq!(d3.choice, Choice::Source(3));
}

#[test]
fn f3_source_tables_are_checked() {
    let cfg = Config::v4("rafs", 1.0);
    let head = "t_s,osnma_status,osnma_offset_ns,gnss_status,gnss_offset_ns,leo_status,leo_offset_ns,ptp_status,ptp_offset_ns,eloran_status,eloran_offset_ns\n";
    let ok = format!("{head}0,ok,1,ok,1,unavailable,,ok,2,ok,3\n1,failed,,ok,1,ok,2,ok,2,ok,3\n");
    assert_eq!(multi_source::parse_table(&cfg, &ok).unwrap().len(), 2);
    for (bad, why) in [
        (
            format!("{head}0,ok,1,failed,,ok,1,ok,2,ok,3\n"),
            "authenticating",
        ),
        (
            format!("{head}1,ok,1,ok,1,ok,1,ok,2,ok,3\n0,ok,1,ok,1,ok,1,ok,2,ok,3\n"),
            "increase",
        ),
        (
            format!("{head}0,ok,x,ok,1,ok,1,ok,2,ok,3\n"),
            "not a number",
        ),
        (
            head.replace(",eloran_status,eloran_offset_ns", ""),
            "eloran",
        ),
    ] {
        let e = multi_source::parse_table(&cfg, &bad).unwrap_err();
        assert!(e.contains(why), "{why}: {e}");
    }
    let mut c = Config::v4("quartz", 1.0);
    assert!(c.validate().is_err());
    c.clock_class = "rafs".into();
    c.sources[1].id = "osnma".into();
    assert!(c.validate().unwrap_err().contains("twice"));
}

#[test]
fn f3_command_line_runs_under_the_licence_and_verifies_for_free() {
    let d = tmp("trust-cli");
    let home = d.join("home");
    let out = d.join("out");
    let o = out.to_str().unwrap();
    let args = [
        "trust",
        "run",
        "examples/multi-source/config.toml",
        "--input",
        "examples/multi-source/sources.csv",
        "--out",
        o,
    ];
    let (code, text) = cli(&home, &args);
    assert_ne!(code, Some(0), "refused without a licence: {text}");
    let mut dev = args.to_vec();
    dev.push("--dev");
    let (code, text) = cli(&home, &dev);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("3600 epochs"), "{text}");
    let (code, text) = cli(&home, &["trust", "verify", o]);
    assert_eq!(code, Some(0), "{text}");
    // The demo: the time push is left for an independent source, never shown as trusted.
    let csv = std::fs::read_to_string(out.join("trust-decisions.csv")).unwrap();
    let pushed_trusted = csv
        .lines()
        .skip(1)
        .filter(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let t: f64 = f[0].parse().unwrap();
            (750.0..1500.0).contains(&t) && (f[1] == "osnma" || f[1] == "gnss")
        })
        .count();
    assert_eq!(pushed_trusted, 0);
    // Tampering is detected.
    std::fs::write(
        out.join("trust-decisions.csv"),
        csv.replacen("osnma", "gnss", 1),
    )
    .unwrap();
    let (code, text) = cli(&home, &["trust", "verify", o]);
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("FAILED"), "{text}");
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------ F5

fn policy() -> Policy {
    Policy::parse(&std::fs::read_to_string(root().join("examples/alerts/policy.toml")).unwrap())
        .unwrap()
}

fn demo_alerts() -> Vec<alerts::Alert> {
    alerts::parse_alerts(
        &std::fs::read_to_string(root().join("examples/alerts/alerts.csv")).unwrap(),
    )
    .unwrap()
}

#[test]
fn f5_routes_quiet_hours_escalation_and_dedup_on_the_demo() {
    let a = demo_alerts();
    let acks = alerts::parse_acks(
        &std::fs::read_to_string(root().join("examples/alerts/acks.csv")).unwrap(),
    )
    .unwrap();
    let plan = alerts::evaluate(&policy(), &a, &acks, T0 + 7 * 86_400);
    assert!(plan.accounts_for(&a));
    assert_eq!(plan.raw_alerts, 9);
    // Quiet hours (22:00-07:00 at UTC+2): the four warnings of 02:10-02:25 local time are held
    // into one digest at 07:00 local (05:00 UTC).
    let digest: Vec<_> = plan
        .notifications
        .iter()
        .filter(|n| n.reason == "digest")
        .collect();
    assert_eq!(digest.len(), 1);
    assert_eq!(digest[0].t_unix, T0 + 5 * 3600);
    assert_eq!(
        digest[0].alerts,
        vec!["gof-1", "tartu-1", "gof-2", "site-a-switch-1"]
    );
    // Critical alerts are sent at any hour, to PagerDuty and Slack.
    let crit: Vec<_> = plan
        .notifications
        .iter()
        .filter(|n| n.route == "critical-oncall" && n.reason == "new")
        .collect();
    assert_eq!(crit.len(), 3);
    assert_eq!(
        crit[0].deliveries,
        vec![
            ("ops-oncall".to_string(), "pagerduty".to_string()),
            ("noc-channel".to_string(), "slack".to_string())
        ]
    );
    // Escalation: the acknowledged alert (gof-3) is not escalated; the two holdovers are.
    let esc: Vec<_> = plan
        .notifications
        .iter()
        .filter(|n| n.reason.starts_with("escalation"))
        .collect();
    assert_eq!(esc.len(), 2);
    assert!(esc.iter().all(|n| !n.alerts.contains(&"gof-3".to_string())));
    assert_eq!(esc[0].deliveries[0].0, "duty-manager");
    // De-duplication: the repeat inside the window is folded on the SIEM route.
    assert_eq!(plan.folded, vec!["gof-2"]);
    assert!(plan.unrouted.is_empty());
    // Reproducible.
    assert_eq!(
        plan,
        alerts::evaluate(&policy(), &a, &acks, T0 + 7 * 86_400)
    );
}

#[test]
fn f5_dedup_never_hides_a_worse_alert_and_unrouted_alerts_stay_raw() {
    let p = Policy::parse(
        r#"format = "kshana-pro-alert-policy/1"
dedup_window_s = 600
[[recipient]]
id = "ops"
outputs = ["webhook"]
[[route]]
name = "areas"
kinds = ["area"]
notify = ["ops"]
dedup_key = ["area"]
"#,
    )
    .unwrap();
    let a = |t: i64, id: &str, kind: &str, sev: Severity| alerts::Alert {
        t_unix: t,
        id: id.into(),
        source: "s".into(),
        kind: kind.into(),
        severity: sev,
        area: "x".into(),
        text: "t".into(),
    };
    let list = vec![
        a(0, "a1", "area", Severity::Warning),
        a(10, "a2", "area", Severity::Warning),
        a(20, "a3", "area", Severity::Critical),
        a(30, "a4", "area", Severity::Info),
        a(700, "a5", "area", Severity::Warning),
        a(40, "z1", "other", Severity::Critical),
    ];
    let plan = alerts::evaluate(&p, &list, &[], 10_000);
    let reasons: Vec<(&str, String)> = plan
        .notifications
        .iter()
        .map(|n| (n.alerts[0].as_str(), n.reason.clone()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("a1", "new".into()),
            ("a3", "worse".into()),
            ("a5", "new".into())
        ]
    );
    assert_eq!(plan.folded, vec!["a2", "a4"]);
    assert_eq!(
        plan.notifications[2].folded, 1,
        "a5 reports the repeat folded since a3"
    );
    assert_eq!(plan.unrouted, vec!["z1"]);
    assert!(plan.accounts_for(&list));
    // The raw log keeps every alert unchanged, the unrouted one included.
    let raw = alerts::raw_log(&list);
    assert_eq!(raw.lines().count(), list.len() + 1);
    assert_eq!(alerts::parse_alerts(&raw).unwrap().len(), list.len());
    // An output that is not an I5 output is refused.
    let bad = r#"format = "kshana-pro-alert-policy/1"
[[recipient]]
id = "ops"
outputs = ["fax"]
[[route]]
name = "r"
notify = ["ops"]
"#;
    assert!(Policy::parse(bad).unwrap_err().contains("not an I5 output"));
}

#[test]
fn f5_raw_alerts_are_free_and_routing_needs_the_licence() {
    let d = tmp("alerts-cli");
    let home = d.join("home");
    let (code, text) = cli(&home, &["alerts", "raw", "examples/alerts/alerts.csv"]);
    assert_eq!(code, Some(0), "{text}");
    assert_eq!(text.lines().count(), 10);
    let route = [
        "alerts",
        "route",
        "examples/alerts/policy.toml",
        "--alerts",
        "examples/alerts/alerts.csv",
    ];
    let (code, text) = cli(&home, &route);
    assert_ne!(code, Some(0));
    assert!(text.contains("raw alerts stay free"), "{text}");
    let mut dev = route.to_vec();
    dev.push("--dev");
    let (code, text) = cli(&home, &dev);
    assert_eq!(code, Some(0), "{text}");
    assert!(text.contains("9 raw alerts"), "{text}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn f5_alerts_come_from_the_console_and_the_trust_selector() {
    let p = demo();
    let a = alerts::from_picture(&p);
    assert_eq!(a.len(), 3);
    assert_eq!(a[0].kind, "area-degraded");
    assert_eq!(a[2].kind, "area-feed-detection");
    assert_eq!(a[2].severity, Severity::Info);
    let cfg = Config::v4("rafs", 1.0);
    let text = std::fs::read_to_string(root().join("examples/multi-source/sources.csv")).unwrap();
    let rows = multi_source::parse_table(&cfg, &text).unwrap();
    let r = multi_source::run_table(&cfg, &rows, "").unwrap();
    let t = alerts::from_trust(&r.evidence, "site-a", T0);
    assert_eq!(t.len(), r.evidence.switches.len() - 1);
    assert!(t.iter().any(|x| x.kind == "timing-source-switch"));
}

// ------------------------------------------------------------------ F6 (B1b)

fn front_scenario() -> PlannerFrontScenario {
    let mut s: PlannerFrontScenario = toml::from_str(
        &std::fs::read_to_string(root().join("examples/planner-front/front.toml")).unwrap(),
    )
    .unwrap();
    s.planner = Some(
        root()
            .join("examples/planner-front/planner.json")
            .display()
            .to_string(),
    );
    s
}

#[test]
fn f6_b1b_front_is_exact_non_dominated_and_reproducible_with_a_cost_axis() {
    let s = front_scenario();
    let f = s.compute().unwrap();
    assert_eq!(f.evaluated, 120 + 210 + 252 + 210);
    assert!(f.front.len() >= 2);
    // Cost strictly increases and objective strictly increases along the front.
    for w in f.front.windows(2) {
        assert!(w[1].cost > w[0].cost && w[1].objective > w[0].objective);
    }
    // Brute force: no evaluated network dominates a front point, and every network is
    // dominated by or equal to a front point.
    let spec: kshana::planner::PlanSpec = serde_json::from_str(
        &std::fs::read_to_string(root().join("examples/planner-front/planner.json")).unwrap(),
    )
    .unwrap();
    let mut all = Vec::new();
    for m in 3..=6usize {
        for mask in 0u32..1024 {
            if mask.count_ones() as usize != m {
                continue;
            }
            let sub: Vec<usize> = (0..10).filter(|i| mask & (1 << i) != 0).collect();
            let (obj, med) = kshana::planner::objective_of(&spec, &sub);
            let cost: f64 = sub
                .iter()
                .map(|&i| s.costs[i].capex + s.years * s.costs[i].opex_per_year)
                .sum();
            all.push(planner_front::Point {
                subset: sub,
                cost,
                objective: obj,
                median_drms_m: med.is_finite().then_some(med),
            });
        }
    }
    for p in &f.front {
        assert!(!all.iter().any(|q| planner_front::dominates(q, p)), "{p:?}");
    }
    for q in &all {
        assert!(f
            .front
            .iter()
            .any(|p| planner_front::dominates(p, q)
                || (p.cost == q.cost && p.objective == q.objective)));
    }
    // The best network per m agrees with the open planner's own exhaustive placement.
    for (m, best) in &f.best_per_m {
        let open = kshana::planner::place(&kshana::planner::PlanSpec {
            m: Some(*m),
            ..spec.clone()
        })
        .unwrap();
        assert_eq!(&open.subset, &best.subset, "m = {m}");
        assert_eq!(open.objective, best.objective);
    }
    // The target answer is the cheapest front point meeting it.
    let t = f.cheapest_meeting_target.clone().unwrap();
    assert!(t.objective >= 0.95);
    assert!(f
        .front
        .iter()
        .all(|p| p.cost >= t.cost || p.objective < 0.95));
    // Byte-identical twice; the CSV carries the cost axis.
    assert_eq!(
        serde_json::to_string(&f).unwrap(),
        serde_json::to_string(&s.compute().unwrap()).unwrap()
    );
    assert!(f.csv().starts_with("cost_eur,objective,"));
}

#[test]
fn f6_b1b_inputs_are_checked() {
    let mut s = front_scenario();
    s.costs.pop();
    assert!(s
        .compute()
        .unwrap_err()
        .contains("9 costs for 10 candidates"));
    let mut s = front_scenario();
    s.m_max = 11;
    assert!(s.compute().is_err());
    let mut s = front_scenario();
    s.costs[0].capex = -1.0;
    assert!(s.compute().is_err());
}

#[test]
fn f6_station_fleet_and_ha_stay_wired_as_pro_and_enterprise_features() {
    use kshana_pro::license::{self, tiers::Edition};
    for f in ["pro-station-fleet", "pro-service-ha"] {
        assert_eq!(
            license::tiers::compiled_edition(f),
            Some(Edition::Pro),
            "{f}"
        );
    }
    assert_eq!(license::feature_min_tier("pro-station-fleet"), Tier::Pro);
    assert_eq!(
        license::feature_min_tier("pro-service-ha"),
        Tier::Enterprise
    );
    // The new kinds and features are Pro, in the signed manifest.
    let m = license::tiers::manifest().unwrap();
    let named: Vec<&str> = m.named().into_iter().map(|(n, _, _)| n).collect();
    for k in [
        "pro-area-picture",
        "pro-area-console",
        "pro-multi-source-trust",
        "pro-alert-policy",
        "pro-planner-front",
    ] {
        assert!(named.contains(&k), "{k} in the tier manifest");
        assert_eq!(
            license::tiers::compiled_edition(k),
            Some(Edition::Pro),
            "{k}"
        );
        assert_eq!(license::feature_min_tier(k), Tier::Pro, "{k}");
        assert!(license::check(&ent(Tier::Evaluation), k, 0).is_err(), "{k}");
        assert!(license::check(&ent(Tier::Pro), k, 0).is_ok(), "{k}");
    }
}
