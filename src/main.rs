// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! `kshana-pro` — the Pro CLI. A thin front end over [`kshana_pro::registry`] that
//! demonstrates the superset (runs every open scenario AND every Pro pack from one
//! binary) and the licensing mechanics (issue / verify tokens; `--license` enforces
//! tier entitlements on Pro packs).

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use kshana_pro::connectors::{self, GateOp};
use kshana_pro::license::{self, Entitlement, LicenseClaims, Tier};

fn usage() -> &'static str {
    "kshana-pro — proprietary overlay on the open Kshana engine\n\
     \n\
     USAGE:\n\
       kshana-pro run <scenario.toml> [--license <token>] [--dev]   Run open OR Pro scenario\n\
       kshana-pro kinds                                              List kinds + their tier\n\
       kshana-pro mrd                                                Emit the machine-checked D1.4 MRD FoM-mapping (Markdown)\n\
       kshana-pro benefits [--json|--svg]                            Benefit-vs-baseline reference table (A02, D1.1)\n\
       kshana-pro report <scenario.toml> [--subject <s>] [--format md|json|html|svg]   Consolidated report (A11)\n\
       kshana-pro navdt <scenario.toml> [--subject <s>]              NAV-DT deterministic twin bundle, JSON (A10)\n\
       kshana-pro spacestream [--schema]                             SpaceStream codec goldens / ICD schema (A09)\n\
       kshana-pro version                                            Overlay + engine info\n\
       kshana-pro export <scenario.toml> --format <stk|csv>          Export ephemeris (STK .e / CSV)\n\
       kshana-pro gate <scenario.toml> --metric <path> --op <lt|le|gt|ge> --threshold <v>\n\
       kshana-pro serve [--addr <host:port>] [--license <token>] [--dev]   Host the HTTP service\n\
       kshana-pro license issue --tier <t> --licensee <name> [--days <n>]\n\
       kshana-pro license verify <token>\n\
     \n\
     Tiers: evaluation | pro | enterprise. `run` is FAIL-CLOSED: with no --license it\n\
     enforces Evaluation tier (open scenarios + the free seam-check only). --license\n\
     <token> enforces that token's tier; --dev disables enforcement (local use only).\n\
     Open (non-pro) scenarios are always free. Signing secret: $KSHANA_PRO_LICENSE_SECRET\n\
     (a built-in DEMO secret is used otherwise)."
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => match args.get(1) {
            Some(path) => run_file(path, flag(&args, "--license"), has_flag(&args, "--dev")),
            None => {
                eprintln!("error: `run` needs a scenario file\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
        Some("kinds") => {
            list_kinds();
            ExitCode::SUCCESS
        }
        // The D1.4 Mission Requirements Document FoM-mapping skeleton, rendered from
        // the machine-checked `mrd` table (A03). Deterministic, ungated (a capability
        // demonstrator like `kinds`/`version`, not a Pro analysis pack). The committed
        // deliverable doc is this exact output; `tests/mrd_deliverable_in_sync.rs`
        // asserts the two never drift apart.
        Some("mrd") => {
            print!("{}", kshana_pro::mrd::to_markdown());
            ExitCode::SUCCESS
        }
        // Benefit-vs-baseline reference deliverable (A02). Default = the float-free
        // provenance skeleton (the committed D1.1 deliverable); --json / --svg emit
        // the numeric artifact (reproducible in process).
        Some("benefits") => match kshana_pro::benefits::reference_case() {
            Ok(c) => {
                if has_flag(&args, "--json") {
                    println!("{}", c.to_json());
                } else if has_flag(&args, "--svg") {
                    println!("{}", c.to_svg());
                } else {
                    print!("{}", c.to_provenance_markdown());
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        // Consolidated performance/resilience report over a scenario (A11).
        Some("report") => report_cmd(&args),
        // NAV-DT deterministic twin bundle over a scenario (A10).
        Some("navdt") => navdt_cmd(&args),
        // SpaceStream NAVEX→NAV-DT codec: golden messages (the codec, demonstrated +
        // reproducible) or the ICD JSON schema (A09).
        Some("spacestream") => {
            use kshana_pro::connectors::spacestream;
            if has_flag(&args, "--schema") {
                println!("{}", spacestream::json_schema());
            } else {
                let msgs: Vec<serde_json::Value> = spacestream::golden_messages()
                    .into_iter()
                    .map(|(name, m)| {
                        serde_json::json!({
                            "name": name,
                            "message": serde_json::from_str::<serde_json::Value>(&m.to_json())
                                .unwrap_or(serde_json::Value::Null),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&msgs).unwrap_or_default()
                );
            }
            ExitCode::SUCCESS
        }
        Some("version") => {
            println!(
                "kshana-pro {} (overlay) · embeds open Kshana engine reporting {} scenario kinds",
                kshana_pro::version(),
                kshana_pro::open_engine_kinds(),
            );
            ExitCode::SUCCESS
        }
        Some("export") => match args.get(1) {
            Some(path) => export_cmd(path, flag(&args, "--format")),
            None => {
                eprintln!("error: `export` needs a scenario file\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
        Some("gate") => match args.get(1) {
            Some(path) => gate_cmd(path, &args),
            None => {
                eprintln!("error: `gate` needs a scenario file\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
        Some("serve") => serve_cmd(&args),
        Some("healthcheck") => {
            let addr = flag(&args, "--addr").unwrap_or_else(|| "127.0.0.1:8088".to_string());
            if kshana_pro::serve::healthcheck(&addr) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some("license") => license_cmd(&args[1..]),
        _ => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
    }
}

fn serve_cmd(args: &[String]) -> ExitCode {
    let addr = flag(args, "--addr").unwrap_or_else(|| "127.0.0.1:8088".to_string());
    // The service runs at a fixed entitlement: a token sets the tier; --dev =
    // Enterprise (everything) for local use; otherwise Evaluation (open + free only).
    let ent = if has_flag(args, "--dev") {
        eprintln!("[DEV MODE — serving at Enterprise tier, no enforcement; not for distribution]");
        Entitlement {
            licensee: "(dev)".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        }
    } else {
        match flag(args, "--license") {
            Some(tok) => match license::verify(&signing_secret(), &tok, now_unix()) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => {
                eprintln!(
                    "[no license — serving at Evaluation tier; use --license <token> or --dev]"
                );
                Entitlement::evaluation()
            }
        }
    };
    match kshana_pro::serve::serve(&addr, ent) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: could not serve on {addr}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Run a scenario through the open engine and return its result JSON (exports and
/// the gate operate on raw open-engine results, so they bypass Pro entitlements —
/// they consume open outputs, not Pro packs).
fn run_to_json(path: &str) -> Result<String, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("could not read {path}: {e}"))?;
    kshana::api::run_scenario(&src)
        .map(|o| o.json)
        .map_err(|e| format!("{e}"))
}

fn export_cmd(path: &str, format: Option<String>) -> ExitCode {
    let json = match run_to_json(path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let samples = match connectors::parse_ephem_samples(&json) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = match format.as_deref() {
        Some("stk") => connectors::to_stk_ephemeris(&samples),
        Some("csv") => connectors::to_csv(&samples),
        other => {
            eprintln!("error: --format must be 'stk' or 'csv' (got {other:?})");
            return ExitCode::FAILURE;
        }
    };
    match out {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Consolidated performance/resilience report over a scenario (A11).
fn report_cmd(args: &[String]) -> ExitCode {
    let path = match args.get(1) {
        Some(p) if !p.starts_with("--") => p,
        _ => {
            eprintln!("error: `report` needs a scenario file\n\n{}", usage());
            return ExitCode::FAILURE;
        }
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let subject = flag(args, "--subject").unwrap_or_else(|| "quantum".to_string());
    match kshana_pro::report::build_report("kshana-pro report", &src, &subject) {
        Ok(r) => {
            match flag(args, "--format").as_deref() {
                Some("json") => println!("{}", r.to_evidence_json()),
                Some("html") => println!("{}", r.to_html()),
                Some("svg") => println!("{}", r.to_svg()),
                _ => print!("{}", r.to_markdown()),
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// NAV-DT deterministic twin bundle over a scenario (A10).
fn navdt_cmd(args: &[String]) -> ExitCode {
    let path = match args.get(1) {
        Some(p) if !p.starts_with("--") => p,
        _ => {
            eprintln!("error: `navdt` needs a scenario file\n\n{}", usage());
            return ExitCode::FAILURE;
        }
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let subject = flag(args, "--subject").unwrap_or_else(|| "quantum".to_string());
    match kshana_pro::twin::navdt::run_navdt_bundle(&src, &subject) {
        Ok(b) => {
            println!("{}", b.to_json());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn gate_cmd(path: &str, args: &[String]) -> ExitCode {
    let metric = match flag(args, "--metric") {
        Some(m) => m,
        None => {
            eprintln!("error: gate needs --metric <dot.path>");
            return ExitCode::FAILURE;
        }
    };
    let op = match flag(args, "--op").map(|o| GateOp::parse(&o)) {
        Some(Ok(o)) => o,
        Some(Err(e)) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
        None => {
            eprintln!("error: gate needs --op <lt|le|gt|ge>");
            return ExitCode::FAILURE;
        }
    };
    let threshold: f64 = match flag(args, "--threshold").and_then(|t| t.parse().ok()) {
        Some(t) => t,
        None => {
            eprintln!("error: gate needs --threshold <number>");
            return ExitCode::FAILURE;
        }
    };
    let json = match run_to_json(path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let value = match dot_path_number(&json, &metric) {
        Some(v) => v,
        None => {
            eprintln!("error: metric '{metric}' not found or non-numeric in result");
            return ExitCode::FAILURE;
        }
    };
    let pass = op.passes(value, threshold);
    eprintln!(
        "[gate] {metric} = {value} {} {threshold} → {}",
        op.as_str(),
        if pass { "PASS" } else { "FAIL" }
    );
    if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE // nonzero → fails the CI pipeline
    }
}

/// Local dot-path metric extraction (mirrors the internal util used by the packs).
/// A numeric path segment indexes into a JSON array (e.g. `samples.0.alt_km`).
fn dot_path_number(result_json: &str, path: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(result_json).ok()?;
    let mut cur = &v;
    for key in path.split('.') {
        cur = match key.parse::<usize>() {
            Ok(idx) => cur.get(idx)?,
            Err(_) => cur.get(key)?,
        };
    }
    cur.as_f64().filter(|x| x.is_finite())
}

/// Read `--flag value` from args.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Presence of a boolean `--flag`.
fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The vendor signing secret: from the environment, or a clearly-labelled demo key.
fn signing_secret() -> Vec<u8> {
    match std::env::var("KSHANA_PRO_LICENSE_SECRET") {
        Ok(s) if !s.is_empty() => s.into_bytes(),
        _ => {
            eprintln!("[warning] using the built-in DEMO signing secret — set KSHANA_PRO_LICENSE_SECRET for real licenses");
            b"kshana-pro-demo-secret-do-not-use-in-production".to_vec()
        }
    }
}

fn run_file(path: &str, license_token: Option<String>, dev_mode: bool) -> ExitCode {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let routed = match kshana_pro::registry::peek_kind(&src) {
        Some(k) if k.starts_with(kshana_pro::registry::PRO_KIND_PREFIX) => {
            format!("Pro overlay ({k})")
        }
        Some(k) => format!("open engine ({k})"),
        None => "open engine (default)".to_string(),
    };

    // One `now` for the whole request — no drift between verify and the gate check.
    let now = now_unix();
    // Default is FAIL-CLOSED: no license → Evaluation tier (only the free packs).
    // A real license enforces its tier; `--dev` is an explicit, loud escape hatch.
    let result = if dev_mode {
        eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
        kshana_pro::registry::run(&src)
    } else {
        match license_token {
            Some(tok) => match license::verify(&signing_secret(), &tok, now) {
                Ok(ent) => {
                    eprintln!(
                        "[license: {} tier, licensee: {}]",
                        ent.tier.as_str(),
                        ent.licensee
                    );
                    kshana_pro::registry::run_gated(&src, &ent, now)
                }
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => {
                eprintln!("[no license — Evaluation tier; use --license <token> or --dev]");
                kshana_pro::registry::run_gated(&src, &Entitlement::evaluation(), now)
            }
        }
    };

    match result {
        Ok(out) => {
            eprintln!("[routed to: {routed}]");
            println!("{}", out.json);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn list_kinds() {
    println!("Open engine kinds (inherited by Pro · free):");
    for m in kshana::api::list_scenario_kinds() {
        println!("  {:<18} {}", m.name, m.description);
    }
    println!("\nPro overlay kinds (added on top · min tier in brackets):");
    for m in kshana_pro::registry::pro_kinds() {
        println!(
            "  {:<18} [{}] {}",
            m.name,
            license::feature_min_tier(m.name).as_str(),
            m.description
        );
    }
}

fn license_cmd(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("issue") => {
            let tier = match flag(args, "--tier").map(|t| Tier::parse(&t)) {
                Some(Ok(t)) => t,
                Some(Err(e)) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
                None => {
                    eprintln!("error: license issue needs --tier <evaluation|pro|enterprise>");
                    return ExitCode::FAILURE;
                }
            };
            let licensee = flag(args, "--licensee").unwrap_or_else(|| "unnamed".to_string());
            let days: i64 = flag(args, "--days")
                .and_then(|d| d.parse().ok())
                .unwrap_or(0);
            let expires_unix = if days > 0 {
                match days
                    .checked_mul(86_400)
                    .and_then(|s| now_unix().checked_add(s))
                {
                    Some(e) => e,
                    None => {
                        eprintln!("error: --days too large (expiry timestamp overflows)");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                0
            };
            let claims = LicenseClaims {
                licensee,
                tier: tier.as_str().to_string(),
                expires_unix,
                features: Vec::new(),
            };
            match license::issue(&signing_secret(), &claims) {
                Ok(tok) => {
                    eprintln!(
                        "[issued {} license for '{}'{}]",
                        tier.as_str(),
                        claims.licensee,
                        if expires_unix > 0 {
                            format!(" (expires in {days}d)")
                        } else {
                            " (perpetual)".into()
                        },
                    );
                    println!("{tok}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("verify") => match args.get(1) {
            Some(tok) => match license::verify(&signing_secret(), tok, now_unix()) {
                Ok(ent) => {
                    print_entitlement(&ent);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("error: license verify needs a <token>");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: kshana-pro license <issue|verify> ...");
            ExitCode::FAILURE
        }
    }
}

fn print_entitlement(ent: &Entitlement) {
    println!("licensee:  {}", ent.licensee);
    println!("tier:      {}", ent.tier.as_str());
    println!(
        "expires:   {}",
        if ent.expires_unix == 0 {
            "never (perpetual)".to_string()
        } else {
            format!("unix {}", ent.expires_unix)
        }
    );
    println!("entitled Pro kinds:");
    for m in kshana_pro::registry::pro_kinds() {
        let ok = ent.allows(m.name, now_unix());
        println!("  [{}] {}", if ok { "✓" } else { "✗" }, m.name);
    }
}
