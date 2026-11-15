// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Roadmap 0.42 X6 (results-only service API) and C3 (hosted Kshana Pro service for secret
//! methods), over real sockets. A stand-in method is registered under an existing Pro kind
//! name; its full result carries intermediates that must never leave the server. Checked:
//! only verdict, evidence class, input hash, query ID, watermark and signature come back;
//! the signature verifies; the watermark traces to the subscription's licensee; a verdict
//! outside the kind's closed set is refused; per-tenant monthly query and compute budgets;
//! probing alerts and blocking; the access log is hash-chained and holds no input values;
//! anonymous and unsubscribed callers are refused.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kshana::api::KshanaError;
use kshana_pro::license::{public_key_for, Entitlement, Tier};
use kshana_pro::results_only::{
    self, EvidenceClass, Gateway, GatewayConfig, Query, Refusal, ResultsOnlyKind, Verdict,
};
use kshana_pro::serve::auth::{new_key, KeysFile, Role};
use kshana_pro::serve::hosted::HostedConfig;
use kshana_pro::serve::{self, HostedSetup, ServeConfig};
use kshana_pro::watermark;
use serde_json::{json, Value};

const SEED: [u8; 32] = [0x31; 32];
const WM_KEY: [u8; 32] = [0x77; 32];
/// The stand-in kind runs under this existing Pro kind name, so the licence gate applies.
const KIND: &str = "pro-twin";
const SECRET_INTERMEDIATE: f64 = 0.618_033_988_749;

/// A stand-in secret method: its result holds an intermediate that must stay inside.
struct StandIn {
    lie: Arc<AtomicBool>,
}

impl ResultsOnlyKind for StandIn {
    fn kind(&self) -> &str {
        KIND
    }
    fn verdicts(&self) -> &[&str] {
        &["pass", "fail"]
    }
    fn run(&self, input: &Value) -> Result<Value, KshanaError> {
        let x = input["x"]
            .as_f64()
            .ok_or_else(|| KshanaError::InvalidInput("x must be a number".into()))?;
        Ok(json!({
            "intermediate_gain": SECRET_INTERMEDIATE,
            "internal_margin": x * SECRET_INTERMEDIATE,
            "pass": x * SECRET_INTERMEDIATE < 10.0,
        }))
    }
    fn reduce(&self, full: &Value) -> Result<Verdict, KshanaError> {
        if self.lie.load(Ordering::SeqCst) {
            // Tries to leak a number through the verdict.
            return Ok(Verdict {
                verdict: format!("{}", full["internal_margin"]),
                evidence_class: EvidenceClass::Modelled,
            });
        }
        Ok(Verdict {
            verdict: if full["pass"] == true { "pass" } else { "fail" }.into(),
            evidence_class: EvidenceClass::Modelled,
        })
    }
}

fn lie_flag() -> Arc<AtomicBool> {
    static FLAG: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
    FLAG.get_or_init(|| {
        let f = Arc::new(AtomicBool::new(false));
        results_only::register_kind(Arc::new(StandIn { lie: f.clone() }));
        f
    })
    .clone()
}

/// The registry is process-wide; tests that flip the stand-in's behaviour take this lock.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|p| p.into_inner())
}

fn dir(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hosted_service-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn http(a: SocketAddr, method: &str, path: &str, body: &str, key: Option<&str>) -> (u16, String) {
    let mut s = TcpStream::connect(a).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let auth = key
        .map(|k| format!("Authorization: Bearer {k}\r\n"))
        .unwrap_or_default();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: x\r\n{auth}Content-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    (head[9..12].parse().unwrap(), body.to_string())
}

fn ent() -> Entitlement {
    Entitlement {
        licensee: "host".into(),
        tier: Tier::Enterprise,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

#[test]
fn the_hosted_service_answers_signed_verdicts_only_within_allowance() {
    let _g = serial();
    lie_flag().store(false, Ordering::SeqCst);
    let d = dir("main");
    let (k_acme, e_acme) = new_key("acme-1", "acme", Role::Submitter);
    let (k_none, e_none) = new_key("other-1", "other", Role::Submitter);
    let keys = KeysFile {
        keys: vec![e_acme, e_none],
    };
    let hosted = HostedConfig::parse(
        r#"
requests_per_minute = 1000
probe_alert_after = 100
probe_block_after = 200
[default]
queries_per_month = 1000
[[subscription]]
tenant = "acme"
licensee = "Acme Ltd"
queries_per_month = 4
"#,
    )
    .unwrap();
    let mut cfg = ServeConfig::new("127.0.0.1:0", ent());
    cfg.data_dir = Some(d.clone());
    cfg.keys = Some(keys);
    cfg.hosted = Some(HostedSetup {
        config: hosted,
        signer: Arc::new(SEED),
        watermark_key: WM_KEY,
    });
    let srv = serve::start(cfg).unwrap();
    let a = srv.local_addr();

    // Anonymous: 401. Not subscribed: 403.
    assert_eq!(http(a, "GET", "/v1/verdict", "", None).0, 401);
    assert_eq!(http(a, "GET", "/v1/verdict", "", Some(&k_none)).0, 403);
    let (st, b) = http(a, "GET", "/v1/verdict", "", Some(&k_acme));
    assert_eq!(st, 200, "{b}");
    let v: Value = serde_json::from_str(&b).unwrap();
    assert!(v["kinds"].as_array().unwrap().iter().any(|k| k == KIND));
    assert_eq!(v["allowance"]["queries_per_month"], 4);

    // A verdict: inputs in, verdict out.
    let body = json!({"input": {"x": 3.5}}).to_string();
    let (st, b) = http(
        a,
        "POST",
        &format!("/v1/verdict/{KIND}"),
        &body,
        Some(&k_acme),
    );
    assert_eq!(st, 200, "{b}");
    let v: Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["verdict"], "pass");
    assert_eq!(v["evidence_class"], "modelled");
    for leak in ["0.618", "intermediate", "internal_margin", "Acme"] {
        assert!(!b.contains(leak), "{leak} leaked: {b}");
        let decoded = String::from_utf8(
            hex::decode(v["signed"].as_str().unwrap().split('.').next().unwrap()).unwrap(),
        )
        .unwrap();
        assert!(
            !decoded.contains(leak),
            "{leak} leaked in envelope: {decoded}"
        );
    }
    let env =
        results_only::verify_envelope(&[public_key_for(&SEED)], v["signed"].as_str().unwrap())
            .unwrap();
    assert_eq!(env.kind, KIND);
    assert!(results_only::verify_envelope(
        &[public_key_for(&[1; 32])],
        v["signed"].as_str().unwrap()
    )
    .is_err());
    // The watermark names the licensee only to the key holder.
    let who = vec!["Other GmbH".to_string(), "Acme Ltd".to_string()];
    assert_eq!(
        watermark::trace(&WM_KEY, &who, &env.marked_content(), &env.watermark),
        Some("Acme Ltd")
    );

    // An unknown or unlicensed kind looks the same: 404.
    assert_eq!(
        http(
            a,
            "POST",
            "/v1/verdict/pro-not-a-kind",
            &body,
            Some(&k_acme)
        )
        .0,
        404
    );
    // Monthly query allowance (4): three more, then 429 with Retry-After.
    for x in [20.0, 1.0, 2.0] {
        let body = json!({"input": {"x": x}}).to_string();
        assert_eq!(
            http(
                a,
                "POST",
                &format!("/v1/verdict/{KIND}"),
                &body,
                Some(&k_acme)
            )
            .0,
            200
        );
    }
    let (st, b) = http(
        a,
        "POST",
        &format!("/v1/verdict/{KIND}"),
        &body,
        Some(&k_acme),
    );
    assert_eq!(st, 429, "{b}");
    assert!(b.contains("allowance"));
    srv.stop();

    // The access log: chained, no input values.
    let log = d.join("hosted").join("results-only-access.log");
    assert_eq!(results_only::verify_access_log(&log).unwrap(), 5);
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains("3.5") && !text.contains("\"x\""), "{text}");
    assert!(text.contains("refused:query-budget"));
    let _ = std::fs::remove_dir_all(&d);
}

fn gateway(cfg: GatewayConfig, d: Option<&Path>) -> Gateway {
    Gateway::new(cfg, Arc::new(SEED), WM_KEY, d).unwrap()
}

fn q<'a>(tenant: &'a str, input: &'a Value) -> Query<'a> {
    Query {
        tenant,
        licensee: "Acme Ltd",
        kind: KIND,
        input,
        allowance: None,
    }
}

#[test]
fn probing_raises_alerts_then_blocks_the_tenant_only() {
    let _g = serial();
    lie_flag().store(false, Ordering::SeqCst);
    let alerts = Arc::new(Mutex::new(Vec::new()));
    let a2 = alerts.clone();
    let g = gateway(
        GatewayConfig {
            queries_per_period: 0,
            compute_ms_per_period: 0,
            probe_window_secs: 600,
            probe_alert_after: 3,
            probe_block_after: 5,
        },
        None,
    )
    .with_alert_sink(Box::new(move |a| a2.lock().unwrap().push(a.clone())));
    let t0 = 1_800_000_000;
    // A sweep around one point: same shape, different exact inputs.
    for i in 0..4 {
        let input = json!({"x": 12.30 + 0.001 * i as f64, "mode": "a"});
        assert!(g.query(&q("prober", &input), t0 + i).is_ok());
    }
    assert_eq!(alerts.lock().unwrap().len(), 2, "alerts at 3 and 4");
    assert!(!alerts.lock().unwrap()[0].blocked);
    let input = json!({"x": 12.305, "mode": "a"});
    assert_eq!(
        g.query(&q("prober", &input), t0 + 5),
        Err(Refusal::Probing { retry_after: 600 })
    );
    assert!(alerts.lock().unwrap().last().unwrap().blocked);
    // Still blocked inside the window, for any input; others are not affected.
    let other = json!({"x": 500.0});
    assert!(matches!(
        g.query(&q("prober", &other), t0 + 100),
        Err(Refusal::Probing { .. })
    ));
    assert!(g.query(&q("honest", &input), t0 + 100).is_ok());
    // Repeating one identical query is not probing.
    for i in 0..10 {
        assert!(g.query(&q("repeat", &other), t0 + i).is_ok());
    }
    // After the window the tenant may query again.
    assert!(g.query(&q("prober", &other), t0 + 5 + 601).is_ok());
}

#[test]
fn compute_budgets_persist_and_verdicts_outside_the_set_are_refused() {
    let _g = serial();
    let d = dir("budget");
    let cfg = GatewayConfig {
        queries_per_period: 0,
        compute_ms_per_period: 2,
        probe_window_secs: 60,
        probe_alert_after: 0,
        probe_block_after: 0,
    };
    let t0 = 1_800_000_000;
    let input = json!({"x": 1.0});
    lie_flag().store(false, Ordering::SeqCst);
    {
        let g = gateway(cfg.clone(), Some(&d));
        // Each answered query costs at least 1 ms of the 2 ms allowance.
        assert!(g.query(&q("t", &input), t0).is_ok());
        let mut refused = false;
        for _ in 0..2 {
            if matches!(
                g.query(&q("t", &input), t0),
                Err(Refusal::ComputeBudget { .. })
            ) {
                refused = true;
                break;
            }
        }
        assert!(refused, "the compute allowance is enforced");
    }
    // A restart does not reset the month's usage; a new month does.
    let g = gateway(cfg, Some(&d));
    assert!(matches!(
        g.query(&q("t", &input), t0 + 60),
        Err(Refusal::ComputeBudget { .. })
    ));
    assert!(g.query(&q("t", &input), t0 + 40 * 86_400).is_ok());

    // A kind that tries to answer a number through the verdict is refused.
    lie_flag().store(true, Ordering::SeqCst);
    let r = g.query(&q("u", &input), t0 + 40 * 86_400);
    lie_flag().store(false, Ordering::SeqCst);
    assert_eq!(r, Err(Refusal::MethodFailed));
    assert!(!Refusal::MethodFailed.message().contains("0.618"));
    // Bad input reaches the caller as 422 with the method's first line only.
    let r = g.query(&q("u", &json!({"x": "nope"})), t0 + 40 * 86_400);
    assert!(matches!(r, Err(Refusal::BadInput(_))));
    assert!(results_only::verify_access_log(&d.join("results-only-access.log")).unwrap() >= 6);
    let _ = std::fs::remove_dir_all(&d);
}
