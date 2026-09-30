// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
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
       kshana-pro kinds [--json]                                     List kinds + their tier (--json: machine-readable, with counts)\n\
       kshana-pro mrd                                                Emit the machine-checked D1.4 MRD FoM-mapping (Markdown)\n\
       kshana-pro benefits [--json|--svg]                            Benefit-vs-baseline reference table (A02, D1.1)\n\
       kshana-pro report <scenario.toml> [--subject <s>] [--format md|json|html|svg] [--pdf <out.pdf>]   Consolidated report (A11)\n\
       kshana-pro navdt <scenario.toml> [--subject <s>]              NAV-DT deterministic twin bundle, JSON (A10)\n\
       kshana-pro spacestream [--schema]                             SpaceStream codec goldens / ICD schema (A09)\n\
       kshana-pro version                                            Overlay + engine info\n\
       kshana-pro export <scenario.toml> --format <stk|csv>          Export ephemeris (STK .e / CSV)\n\
       kshana-pro gate <scenario.toml> --metric <path> --op <lt|le|gt|ge> --threshold <v>\n\
       kshana-pro serve [--addr <host:port>] [--license <token>] [--dev]   Host the HTTP service\n\
       kshana-pro ci --pack <manifest.toml> [--baseline <ci.json>] [--thresholds <spec>] [--utc <iso>] [--customer <name>] [--out <report.html>] [--json <ci.json>] [--license <token>] [--dev]   Run a Pack C scenario-subscription CI (Pro)\n\
       kshana-pro study --dossier <scenario.toml> --out <dir> [--utc <iso>] [--license <token>] [--dev]   Reproducible study dossier, HTML + JSON (Pro)\n\
       kshana-pro mbse <import|trace|vcrm|sysml|impact|pack> ... [--license <token>] [--dev]   Requirements, traceability, VCRM, SysML v2, change impact, evidence pack (Pro; `kshana-pro mbse` for details)\n\
       kshana-pro license issue --tier <t> --licensee <name> [--days <n>]\n\
       kshana-pro license verify <token>\n\
       kshana-pro license keygen                       New vendor key pair (vendor only)\n\
     \n\
     Tiers: evaluation | pro | enterprise. `run` is FAIL-CLOSED: with no --license it\n\
     enforces Evaluation tier (open scenarios + the free seam-check only). --license\n\
     <token> enforces that token's tier; --dev disables enforcement (debug builds only;\n\
     a release build refuses it). Open (non-pro) scenarios are always free. Tokens are\n\
     Ed25519-signed: `license issue` signs with $KSHANA_PRO_SIGNING_KEY (vendor only);\n\
     a build verifies with the public key compiled in from $KSHANA_PRO_LICENSE_PUBKEY.\n\
     Debug builds fall back to a DEMO key pair that release builds do not contain."
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `--dev` turns entitlement enforcement off, so it exists only in debug builds
    // (`cargo run`, `cargo test`). A release build — anything handed to a customer —
    // refuses it here, the one place every subcommand passes through.
    if !DEV_BUILD && has_flag(&args, "--dev") {
        eprintln!("error: --dev is not available in a release build; pass --license <token>");
        return ExitCode::FAILURE;
    }
    match args.first().map(String::as_str) {
        Some("run") => match args.get(1) {
            Some(path) => run_file(path, flag(&args, "--license"), has_flag(&args, "--dev")),
            None => {
                eprintln!("error: `run` needs a scenario file\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
        Some("kinds") => {
            if has_flag(&args, "--json") {
                println!("{}", kshana_pro::registry::kinds_json());
            } else {
                list_kinds();
            }
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
        // Spectrum coexistence: whole signal plans and constellations against the open
        // GNSS signals (feature `pro-spectrum-coexistence`).
        Some("coexist") => coexist_cmd(&args[1..]),
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
        // Pack C scenario-subscription CI: run a versioned scenario pack, score it,
        // optionally gate it against a stored baseline, and emit a branded report
        // (Pro-tier entitlement; --dev bypasses enforcement for local use).
        Some("ci") => ci_cmd(&args),
        // Study dossier: one scenario -> a reproducible evidence dossier (JSON + HTML)
        // laid out as a technical note (Pro-tier entitlement; --dev bypasses it).
        Some("study") => study_cmd(&args),
        // MBSE and programme tooling over the open engine's own outputs: requirements
        // import, traceability, the verification cross-reference matrix, SysML v2
        // export, change impact and the evidence pack (feature `pro-mbse`).
        Some("mbse") => mbse_cmd(&args[1..]),
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
            Some(tok) => match verify_token(&tok, now_unix()) {
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

/// `ci` — Pack C scenario-subscription CI. Parse a versioned scenario pack, enforce
/// the Pro-tier entitlement (or bypass with `--dev`), run every scenario through the
/// open engine, optionally gate the result against a stored baseline, and emit a
/// branded report. The exit code mirrors the gate verdict (0 pass / 1 regressions).
fn ci_cmd(args: &[String]) -> ExitCode {
    use std::fs;

    let pack_path = match flag(args, "--pack") {
        Some(p) => p,
        None => {
            eprintln!("error: ci needs --pack <manifest.toml>\n\n{}", usage());
            return ExitCode::from(2);
        }
    };

    // The pack manifest's parent directory is the base for resolving scenario paths.
    let base_dir = std::path::Path::new(&pack_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));

    let manifest_src = match fs::read_to_string(&pack_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {pack_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let pack = match kshana_pro::pack_c::parse_pack(&manifest_src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Resolve the entitlement EXACTLY as run/serve do: --dev = full (Enterprise, no
    // enforcement); else a token enforces its tier; else Evaluation (fail-closed).
    let now = now_unix();
    let ent = if has_flag(args, "--dev") {
        eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
        Entitlement {
            licensee: "(dev)".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        }
    } else {
        match flag(args, "--license") {
            Some(tok) => match verify_token(&tok, now) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => {
                eprintln!("[no license — Evaluation tier; use --license <token> or --dev]");
                Entitlement::evaluation()
            }
        }
    };
    if let Err(e) = kshana_pro::license::ensure_ci_entitled(&ent, now) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    // Optional baseline to gate against.
    let baseline = match flag(args, "--baseline") {
        Some(path) => match fs::read_to_string(&path) {
            Ok(s) => match kshana_pro::pack_c::CiReport::from_json(&s) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("error: baseline {path}: {e}");
                    return ExitCode::FAILURE;
                }
            },
            Err(e) => {
                eprintln!("error: could not read baseline {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    let thresholds = parse_thresholds(flag(args, "--thresholds").as_deref());
    let generated_utc = Some(flag(args, "--utc").unwrap_or_else(utc_iso8601_now));
    let customer = flag(args, "--customer");

    let ctx = kshana_pro::pack_c::CiContext {
        generated_utc,
        customer,
        thresholds,
    };

    let report = match kshana_pro::pack_c::run_ci(&pack, base_dir, baseline.as_ref(), &ctx) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let branded = kshana_pro::report_pack_c::PackCReport::from_ci(
        format!("Pack C — {} {}", pack.id, pack.version),
        &report,
    );

    let out_html = flag(args, "--out");
    let out_json = flag(args, "--json");
    if let Some(out) = &out_html {
        if let Err(e) = fs::write(out, branded.to_html()) {
            eprintln!("error: could not write {out}: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("[wrote HTML report to {out}]");
    }
    if let Some(out) = &out_json {
        // The JSON also doubles as the next run's baseline.
        if let Err(e) = fs::write(out, report.to_json()) {
            eprintln!("error: could not write {out}: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("[wrote CI report JSON to {out}]");
    }
    if out_html.is_none() && out_json.is_none() {
        println!("{}", report.to_json());
    }

    // One-line human summary + the verdict-driven exit code.
    match &report.verdict {
        None => {
            eprintln!("[ci] informational (no baseline)");
            ExitCode::SUCCESS
        }
        Some(v) if v.passed => {
            eprintln!("[ci] PASS");
            ExitCode::from(v.exit_code() as u8)
        }
        Some(v) => {
            eprintln!("[ci] REGRESSIONS: {}", v.regressions.len());
            ExitCode::from(v.exit_code() as u8)
        }
    }
}

/// `mbse <command>` — resolve the entitlement exactly as `ci` and `study` do (`--dev`
/// in a debug build = full; else a token enforces its tier; else Evaluation), require
/// the `pro-mbse` feature, then run the command.
fn mbse_cmd(args: &[String]) -> ExitCode {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!("{}", kshana_pro::mbse::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    let now = now_unix();
    let ent = if has_flag(args, "--dev") {
        eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
        Entitlement {
            licensee: "(dev)".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        }
    } else {
        match flag(args, "--license") {
            Some(tok) => match verify_token(&tok, now) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => {
                eprintln!("[no license — Evaluation tier; use --license <token> or --dev]");
                Entitlement::evaluation()
            }
        }
    };
    if let Err(e) = license::ensure_mbse_entitled(&ent, now) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    match kshana_pro::mbse::cli::run(args) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `study --dossier` — build the reproducible study dossier for one scenario and
/// write `dossier.json`, `dossier.html` and the scenario as run into `--out`.
fn study_cmd(args: &[String]) -> ExitCode {
    let Some(path) = flag(args, "--dossier") else {
        eprintln!(
            "error: study needs --dossier <scenario.toml>\n\n{}",
            usage()
        );
        return ExitCode::from(2);
    };
    let Some(out_dir) = flag(args, "--out") else {
        eprintln!("error: study needs --out <dir>\n\n{}", usage());
        return ExitCode::from(2);
    };
    let now = now_unix();
    let ent = if has_flag(args, "--dev") {
        eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
        Entitlement {
            licensee: "(dev)".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        }
    } else {
        match flag(args, "--license") {
            Some(tok) => match verify_token(&tok, now) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => {
                eprintln!("[no license — Evaluation tier; use --license <token> or --dev]");
                Entitlement::evaluation()
            }
        }
    };
    if let Err(e) = license::ensure_dossier_entitled(&ent, now) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    // A Pro scenario inside the dossier still needs its own entitlement.
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(kind) = kshana_pro::registry::peek_kind(&src) {
        if kind.starts_with(kshana_pro::registry::PRO_KIND_PREFIX) && !ent.allows(&kind, now) {
            eprintln!(
                "error: '{kind}' requires the '{}' tier",
                license::feature_min_tier(&kind).as_str()
            );
            return ExitCode::FAILURE;
        }
    }
    let opts = kshana_pro::study_dossier::DossierOptions {
        issued_utc: flag(args, "--utc"),
    };
    let dossier = match kshana_pro::study_dossier::build(&src, &opts) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let dir = std::path::Path::new(&out_dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("error: could not create {out_dir}: {e}");
        return ExitCode::FAILURE;
    }
    for (name, body) in [
        ("dossier.json", dossier.to_json()),
        ("dossier.html", dossier.to_html()),
        ("scenario.run.toml", dossier.body.run_scenario_toml.clone()),
    ] {
        let p = dir.join(name);
        if let Err(e) = std::fs::write(&p, body) {
            eprintln!("error: could not write {}: {e}", p.display());
            return ExitCode::FAILURE;
        }
    }
    eprintln!("[wrote dossier.json, dossier.html, scenario.run.toml to {out_dir}]");
    println!("{}", dossier.result_hash);
    ExitCode::SUCCESS
}

/// Parse a `--thresholds` spec into [`Thresholds`](kshana_pro::pack_c::Thresholds).
///
/// Accepts a bare number (the default relative tolerance, e.g. `0.1`), or
/// comma-separated `key=val` pairs (each a per-key override), or a leading bare
/// default followed by per-key pairs (e.g. `0.1,araim::fom.mean_hpl_m=0.02`). An
/// absent or empty spec yields the default (5%). Unparseable pairs are skipped with
/// a warning to stderr rather than failing the run.
fn parse_thresholds(spec: Option<&str>) -> kshana_pro::pack_c::Thresholds {
    use kshana_pro::pack_c::Thresholds;
    let Some(spec) = spec.map(str::trim).filter(|s| !s.is_empty()) else {
        return Thresholds::default();
    };

    let mut thr = Thresholds::default();
    for (idx, raw) in spec.split(',').enumerate() {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('=') {
            Some((key, val)) => {
                let key = key.trim();
                match val.trim().parse::<f64>() {
                    Ok(_) if key.is_empty() => {
                        eprintln!("[warning] ignoring threshold pair with empty key: '{part}'")
                    }
                    Ok(v) => {
                        thr = thr.with(key, v);
                    }
                    Err(_) => {
                        eprintln!("[warning] ignoring unparseable threshold pair: '{part}'")
                    }
                }
            }
            // A bare number is only the default when it leads the spec.
            None if idx == 0 => match part.parse::<f64>() {
                Ok(v) => thr.default_rel = v,
                Err(_) => eprintln!("[warning] ignoring unparseable threshold default: '{part}'"),
            },
            None => eprintln!("[warning] ignoring unparseable threshold entry: '{part}'"),
        }
    }
    thr
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

/// `coexist <study.toml> --out <dir>` — resolve the entitlement exactly as `mbse` does
/// (`--dev` in a debug build = full; else a token enforces its tier; else Evaluation),
/// require the `pro-spectrum-coexistence` feature, then run the study.
fn coexist_cmd(args: &[String]) -> ExitCode {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!("{}", kshana_pro::spectrum_coexistence::cli::USAGE);
        return ExitCode::SUCCESS;
    }
    let now = now_unix();
    let ent = if has_flag(args, "--dev") {
        eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
        Entitlement {
            licensee: "(dev)".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
        }
    } else {
        match flag(args, "--license") {
            Some(tok) => match verify_token(&tok, now) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => {
                eprintln!("[no license — Evaluation tier; use --license <token> or --dev]");
                Entitlement::evaluation()
            }
        }
    };
    let feature = kshana_pro::spectrum_coexistence::KIND;
    if !ent.allows(feature, now) {
        eprintln!(
            "error: spectrum coexistence requires the '{}' tier (or the '{feature}' feature on the licence); current tier is '{}'",
            license::feature_min_tier(feature).as_str(),
            ent.tier.as_str()
        );
        return ExitCode::FAILURE;
    }
    match kshana_pro::spectrum_coexistence::cli::run(args) {
        Ok(code) => ExitCode::from(code),
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
            // `--pdf <out.pdf>` renders the same structured report to a pure-Rust
            // PDF (no external tool, no bundled font) and writes it to the path.
            if let Some(out) = flag(args, "--pdf") {
                match kshana_pro::report_pdf::to_pdf(&r) {
                    Ok(bytes) => match std::fs::write(&out, &bytes) {
                        Ok(()) => {
                            eprintln!("[wrote {} bytes to {out}]", bytes.len());
                            return ExitCode::SUCCESS;
                        }
                        Err(e) => {
                            eprintln!("error: could not write {out}: {e}");
                            return ExitCode::FAILURE;
                        }
                    },
                    Err(e) => {
                        eprintln!("error: PDF render failed: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
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

/// Format the current system time as a UTC ISO-8601 second-precision stamp
/// (e.g. `2026-06-23T14:05:09Z`). The library reads no clock; the timestamp is
/// supplied to it as data, so it lives here in the binary. Implemented with `std`
/// only (no chrono) via a civil-date conversion of the Unix epoch seconds.
fn utc_iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (hour, min, sec) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Convert days since the Unix epoch (1970-01-01) to a proleptic-Gregorian
/// (year, month, day). Howard Hinnant's well-known `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Whether this is a debug build, the only kind that honours `--dev` and the demo
/// key pair.
const DEV_BUILD: bool = cfg!(debug_assertions);

/// Private-key seed of the DEMO key pair. Debug builds only: it is compiled out of
/// release builds, so a release binary never accepts a token signed with it.
#[cfg(debug_assertions)]
const DEMO_SEED: [u8; license::KEY_LEN] = *b"kshana-pro-demo-key-debug-only!!";

/// The public key tokens are verified against: the one compiled in from
/// `KSHANA_PRO_LICENSE_PUBKEY`, or in a debug build without one, the demo key.
fn verifying_key() -> Result<[u8; license::KEY_LEN], kshana::api::KshanaError> {
    if let Some(k) = license::embedded_public_key() {
        return k;
    }
    #[cfg(debug_assertions)]
    {
        eprintln!("[warning] debug build without KSHANA_PRO_LICENSE_PUBKEY: verifying against the DEMO key");
        Ok(license::public_key_for(&DEMO_SEED))
    }
    #[cfg(not(debug_assertions))]
    Err(kshana::api::KshanaError::Unsupported(
        "this build has no licence public key (build with KSHANA_PRO_LICENSE_PUBKEY set), \
         so it cannot verify tokens; without --license it runs at Evaluation tier"
            .into(),
    ))
}

fn verify_token(token: &str, now: i64) -> Result<Entitlement, kshana::api::KshanaError> {
    license::verify(&verifying_key()?, token, now)
}

/// The vendor's private-key seed, for `license issue`: from `KSHANA_PRO_SIGNING_KEY`
/// (64 hex characters, kept in the vendor's secret store), or in a debug build, the
/// demo seed. Customers never need it.
fn signing_seed() -> Result<[u8; license::KEY_LEN], kshana::api::KshanaError> {
    match std::env::var("KSHANA_PRO_SIGNING_KEY") {
        Ok(s) if !s.trim().is_empty() => license::parse_key_hex(&s),
        _ => {
            #[cfg(debug_assertions)]
            {
                eprintln!("[warning] KSHANA_PRO_SIGNING_KEY not set: signing with the DEMO key (debug builds only)");
                Ok(DEMO_SEED)
            }
            #[cfg(not(debug_assertions))]
            Err(kshana::api::KshanaError::InvalidInput(
                "set KSHANA_PRO_SIGNING_KEY to the vendor signing key".into(),
            ))
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
            Some(tok) => match verify_token(&tok, now) {
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
            let seed = match signing_seed() {
                Ok(k) => k,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            match license::issue(&seed, &claims) {
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
            Some(tok) => match verify_token(tok, now_unix()) {
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
        Some("keygen") => {
            // A fresh vendor key pair. The private key goes to the secret store as
            // KSHANA_PRO_SIGNING_KEY; the public key is baked into customer builds as
            // KSHANA_PRO_LICENSE_PUBKEY. Re-keying invalidates every token issued before.
            use rand::RngCore;
            let mut seed = [0u8; license::KEY_LEN];
            rand::rngs::OsRng.fill_bytes(&mut seed);
            eprintln!("[new key pair: keep the private key secret; it is shown once]");
            println!("KSHANA_PRO_SIGNING_KEY={}", hex::encode(seed));
            println!(
                "KSHANA_PRO_LICENSE_PUBKEY={}",
                hex::encode(license::public_key_for(&seed))
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: kshana-pro license <issue|verify|keygen> ...");
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
