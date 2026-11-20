// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro` — the Pro CLI. A thin front end over [`crate::registry`] that
//! demonstrates the superset (runs every open scenario AND every Pro pack from one
//! binary) and the licensing mechanics (issue / verify tokens; `--license` enforces
//! tier entitlements on Pro packs).

mod alerts_cmd;
mod area_cmd;
mod attack_iq_cmd;
mod aviation_cmd;
mod dossier_cmd;
mod eloran_cmd;
mod evidence_cmd;
mod evidence_infra_cmd;
mod forensics_cmd;
mod incident_cmd;
mod license_cmd;
mod lunar_cmd;
mod packs_cmd;
mod payments_cmd;
mod pics_cmd;
mod pnt_cmd;
mod pntdp_cmd;
mod receiver_cmd;
mod reports_cmd;
mod scorecard_cmd;
mod testkit_cmd;
mod threats_cmd;
mod timing_cmd;
mod timing_net_cmd;
mod trust_cmd;
mod verdict_cmd;

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::connectors::{self, GateOp};
use crate::license::{self, Entitlement, Tier};

const USAGE_HEAD: &str = "kshana-pro — proprietary overlay on the open Kshana engine\n\
     \n\
     USAGE:\n\
       kshana-pro run <scenario.toml> [--license <token>] [--dev]   Run open OR Pro scenario\n\
       kshana-pro kinds [--json]                                     List kinds + their tier (--json: machine-readable, with counts)\n\
";

const USAGE_TAIL: &str = "kshana-pro benefits [--json|--svg]                            Benefit-vs-baseline reference table (A02, D1.1)\n\
       kshana-pro report <scenario.toml> [--subject <s>] [--format md|json|html|svg] [--pdf <out.pdf>]   Consolidated report (A11)\n\
       kshana-pro version                                            Overlay + engine info\n\
       kshana-pro export <scenario.toml> --format <stk|csv>          Export ephemeris (STK .e / CSV)\n\
       kshana-pro gate <scenario.toml> --metric <path> --op <lt|le|gt|ge> --threshold <v>\n\
       kshana-pro serve [--addr <host:port>] [--data-dir <dir>] [--workers <n>] [--api-token-file <path>] [--keys-file <keys.toml>] [--oidc <oidc.toml>] [--audit-retention-days <n>] [--tls-cert <chain.pem> --tls-key <key.pem>] [--ha-node <name> [--ha-lease-secs <s>]] [--trial [--trial-jobs-per-day <n>] [--trial-max-run-secs <s>] [--trial-tokens]] [--hosted <hosted.toml>] [--license <token>] [--dev]   Host the HTTP(S) service; --data-dir adds the on-premises job service (jobs, ledger, job board, audit log)\n\
       kshana-pro clients --out <dir>                                Write the generated service clients: <dir>/python/kshana_pro_client.py and <dir>/typescript/kshana_pro_client.ts\n\
       kshana-pro keys <add --file <keys.toml> --id <id> --tenant <t> --role <reader|submitter|admin> | revoke --file <keys.toml> --id <id> | list --file <keys.toml>>   Manage the service's API keys (a new key is printed once; the file keeps only its hash)\n\
       kshana-pro ci --pack <manifest.toml> [--baseline <ci.json>] [--thresholds <spec>] [--utc <iso>] [--customer <name>] [--out <report.html>] [--json <ci.json>] [--history <history.jsonl>] [--trend <trend.html>] [--exact] [--license <token>] [--dev]   Run a Pack C scenario-subscription CI, with campaign watch (Pro)\n\
       kshana-pro ci history verify <history.jsonl>                  Verify a campaign-watch history chain (Pro)\n\
       kshana-pro ci diff <old ci.json> <new ci.json> [--json <out>]  List every figure that moved between two CI reports (Pro)\n\
       kshana-pro lab-report <campaign-out-dir> --data-class <synthetic|client-confidential> --html <out.html> [--pdf <out.pdf>] [--client <name>] [--doc-id <id>] [--utc <iso>] [--allow-incomplete] [--license <token>] [--dev]   Branded lab campaign report from `kshana iq campaign` output (Pro)\n\
       kshana-pro study --dossier <scenario.toml> --out <dir> [--utc <iso>] [--license <token>] [--dev]   Reproducible study dossier, HTML + JSON (Pro)\n\
       kshana-pro uncertainty <study.toml> --out <dir> [--resume] [--max-runs <n>] [--threads <n>] [--license <token>] [--dev]   Uncertainty and sensitivity of any open scenario, with stop and resume (Enterprise)\n\
       kshana-pro license <verify|activate|status|deactivate|machine-id|revocations|refresh> ...   Licence on this machine (offline; `kshana-pro license` for details)\n\
       kshana-pro backup <create --data-dir <dir> --out <backup-dir> | verify <backup-dir>>   Back up a service data directory while it runs; check a backup offline\n\
       kshana-pro dossier <keygen|sign|verify> ...   Sign a dossier or any delivery directory (Pro); verify one offline (free; `kshana-pro dossier` for details)\n\
       kshana-pro forensics <run|review|verify|maritime|fdm|reports|log-time|dispute|pack-payload> ...   Incident forensics (E1, Pro pack): signed, reproducible forensic report; review and verify (free); maritime and FDM evidence, log time integrity, dispute evidence (`kshana-pro forensics` for details)\n\
       kshana-pro scorecard <run|verify|osnma|flags|units> ...   Receiver resilience, OSNMA, flag-efficacy and unit scorecards, signed (E3, E13, Pro); verify (free; `kshana-pro scorecard` for details)\n\
       kshana-pro attack-iq <generate|attestation-template|trace> ...   Synthetic attack IQ files for shielded labs and authorised ranges (F7): vetted buyers only, attestation first, files only, watermarked and logged\n\
       kshana-pro threats <list|show|export|feed> ...   Threat scenario library (E2): provenance, analogue, test-bench exports, signed monthly feed (Pro; list and feed verify free)\n\
       kshana-pro payments <check|checkout|stripe-checkout|serve> [--config <payments.toml>] ...   Payments and paywall; inert while payments.enabled and payments.stripe.enabled are false (vendor side)\n\
       kshana-pro packs <build|issue|serve|trace|install|status> ...   Pro pack delivery (C10; `kshana-pro packs` for details)\n\
       kshana-pro timing <assess|console|metrics|history|audit|compliance|timebomb|selfcheck|holdover-evidence|posture|chain|crosscheck|station> ...   Timing assurance: holdover and time to breach per clock, fleet page, metrics, signed audit and evidence packs, time-bomb vectors and fleet audit, clock self-check, holdover evidence, appliance security posture and SBOMs, distribution-chain traceability, multi-source cross-check, ground-station pass assurance (Pro; `kshana-pro timing` for details)\n\
       kshana-pro trust <run|verify> ...   Multi-source trust (F3): choose epoch by epoch among GNSS, OSNMA, LEO, eLoran, PTP and holdover, with hash-chained evidence (Pro; verify free)\n\
       kshana-pro eloran <run|validate|example> ...   eLoran/GNSS signal simulator: multi-chain, multi-time-reference eLoran signals with propagation, interference and truth, to SigMF files or LF hardware (Enterprise; validate and example free)
       kshana-pro lunar <time|rates|w0|circular|holdover|offset|lunex> ...   Lunar time with GUM uncertainty (TCL or TL), W0_L sensitivity, Circular T for the Moon, Earth-Moon TWTT holdover, KLTO-1 offset codec proposal, LUNEX prototype (Pro; decode, vectors, example and validate free; `kshana-pro lunar` for details)\n\
       kshana-pro area picture <console.toml> ...   Area console (F1): fuse Stations, receivers and customer feeds into an area trust picture with event timelines; never emitter positions (Pro)\n\
       kshana-pro alerts <route|raw> ...   Alert policy engine (F5): routes, quiet hours, escalation, de-duplication to I5 outputs (Pro); raw alerts free\n\
       kshana-pro integrity <score|verify> ...   AIS and position integrity scoring at fleet and area scale (E5, Pro); verify free\n\
       kshana-pro verify-service <run|template|verify> ...   Pre-registered user-side verification study of a new PNT service (E4, Pro); template and verify free\n\
       kshana-pro frameworks <list|show|pack|verify> ...   Framework evidence packs: signed evidence mapped to EASA, NIST, DHS, IMO, RTS 25, EU Space Act, NIS 2 duties (E6, Pro; list, show, verify free)\n\
       kshana-pro pics <list|show|mapping|score|export|sample|verify> ...   PICS: PNT Interference Conformance Suite, versioned jamming/spoofing scenarios with pre-registered bars, clause mapping and signed scorecards (score Pro; the rest free)\n\
       kshana-pro testkit <validate|compile|run|diff|from-incident|from-library|fidelity|packs|show|sample|verify> ...   Test-as-code kit: neutral scenarios compiled for lab simulators, CI runner with signed verdicts per build, regression diff, incident-to-regression, replay fidelity, scenario packs (`kshana-pro testkit` for details; validate, packs, sample, verify free)\n\
       kshana-pro pntdp <run|vectors|check> ...   PNT data processing reference (MSC.1/Circ.1575): multi-source plausibility, consistency, selected solution and integrity level (Pro; vectors and check free)\n\
       kshana-pro verdict <selfcal|survey|cause|verify> ...   Hosted methods answered as signed verdicts, verified against the service key (`kshana-pro verdict` for details)\n\
       kshana-pro reports <normalise|validate|episodes|verify|reset-check|ledger|check|params|regimes|synth> ...   Interference reports: normalise crew/ATC/mariner reports, validate per regime, episodes, signed verification, exposure ledger and reset check (Pro; ledger Enterprise; check, params, regimes, synth free)\n\
       kshana-pro aviation <approach|residual|nuisance|areas|check|synth> ...   Aviation operational analytics: approach availability and go-around exposure, residual effects and clean-or-reset, nuisance-alert statistics, unpublished interference areas (Pro; areas Enterprise; check, synth free; decision support only)\n\
       kshana-pro obs-provenance <keygen|sign|verify|keyring-add> ...   Signed feeder provenance on kshana.obs/1 batches: feeder key, time, position, sequence, in a sidecar (sign Pro; the rest free)\n\
       kshana-pro longterm <key-genesis|key-rotate|request|add-token|stamp|history|verify> ...   Long-term signatures: key history and RFC 3161 archive time-stamp chain over a signed directory, verified offline (Pro; request and verify free)\n\
       kshana-pro exposure <run|verify> ...   Insurer scorecard: periodic signed exposure summary of a fleet or route (E7, Pro); verify free\n\
       kshana-pro license <keygen|pubkey|issue|issue-invoice|extend-invoice|trial|renew|revoke|reminders|register> ...   Vendor issuing and the issuance register (vendor only)\n\
     \n\
     --sample (any command): every page of every document written also carries the mark\n\
     \"SAMPLE: illustrative programme on public data, not a real mission\"; use it for\n\
     public showcase outputs. Without it, documents carry the Ashforde seal watermark only.\n\
     \n\
     Tiers: evaluation | pro | enterprise. `run` is FAIL-CLOSED: with no --license it\n\
     enforces Evaluation tier (open scenarios + the free seam-check only). --license\n\
     <token> enforces that token's tier; without it, a token activated with `license\n\
     activate` is used; --dev disables enforcement (debug builds only;\n\
     a release build refuses it). Open (non-pro) scenarios are always free. Tokens are\n\
     Ed25519-signed: `license issue` signs with $KSHANA_PRO_SIGNING_KEY (vendor only);\n\
     a build verifies with the public key compiled in from $KSHANA_PRO_LICENSE_PUBKEY.\n\
     Debug builds fall back to a DEMO key pair that release builds do not contain.";

fn usage() -> String {
    let mut s = String::from(USAGE_HEAD);
    // The usage lines of a registered server-side extension's commands (crate::extension).
    for c in crate::extension::commands() {
        s.push_str(c.usage.trim());
        s.push('\n');
    }
    s.push_str(USAGE_TAIL);
    s
}

/// The commands this crate itself answers. An extension command may not take one of these
/// names ([`crate::extension::register`]).
pub const BUILTIN_COMMANDS: &[&str] = &[
    "run",
    "kinds",
    "benefits",
    "report",
    "version",
    "export",
    "uncertainty",
    "gate",
    "serve",
    "keys",
    "clients",
    "ci",
    "study",
    "lab-report",
    "healthcheck",
    "license",
    "payments",
    "packs",
    "dossier",
    "forensics",
    "backup",
    "timing",
    "scorecard",
    "threats",
    "attack-iq",
    "trust",
    "eloran",
    "lunar",
    "area",
    "alerts",
    "integrity",
    "verify-service",
    "frameworks",
    "exposure",
    "pics",
    "pntdp",
    "verdict",
    "reports",
    "testkit",
    "aviation",
    "obs-provenance",
    "longterm",
];

/// The `kshana-pro` command line over the process arguments.
pub fn main() -> ExitCode {
    main_with_args(std::env::args().skip(1).collect())
}

/// The `kshana-pro` command line over `args` (the program name left out). A server binary
/// that registered an extension ([`crate::extension::register`]) calls this, and gets every
/// Pro command plus the extension's.
pub fn main_with_args(args: Vec<String>) -> ExitCode {
    let mut args = args;
    // The signed tier manifest (roadmap 0.42 X2) must verify and agree with this build
    // before anything runs: it decides the edition of every Pro kind.
    if let Err(e) = license::tiers::startup_check() {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    // `--sample`, accepted by every command: each page of every document this run writes
    // also carries the SAMPLE mark, so a public sample can never be taken for a customer
    // deliverable. Without it, documents carry the seal watermark only.
    if has_flag(&args, "--sample") {
        args.retain(|a| a != "--sample");
        crate::report_brand::set_sample(true);
    }
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
                println!("{}", crate::registry::kinds_json());
            } else {
                list_kinds();
            }
            ExitCode::SUCCESS
        }
        // Benefit-vs-baseline reference deliverable (A02). Default = the float-free
        // provenance skeleton (the committed D1.1 deliverable); --json / --svg emit
        // the numeric artifact (reproducible in process).
        Some("benefits") => match crate::benefits::reference_case() {
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
        Some("version") => {
            println!(
                "kshana-pro {} (overlay) · embeds open Kshana engine reporting {} scenario kinds",
                crate::version(),
                crate::open_engine_kinds(),
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
        // Uncertainty and sensitivity study of any open scenario, with a checkpoint so
        // that it can be stopped and resumed (feature `pro-uncertainty`).
        Some("uncertainty") => uncertainty_cmd(&args[1..]),
        Some("gate") => match args.get(1) {
            Some(path) => gate_cmd(path, &args),
            None => {
                eprintln!("error: `gate` needs a scenario file\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
        Some("serve") => serve_cmd(&args),
        Some("keys") => keys_cmd(&args),
        Some("clients") => clients_cmd(&args),
        // Pack C scenario-subscription CI: run a versioned scenario pack, score it,
        // optionally gate it against a stored baseline, and emit a branded report
        // (Pro-tier entitlement; --dev bypasses enforcement for local use).
        Some("ci") => ci_cmd(&args),
        // Study dossier: one scenario -> a reproducible evidence dossier (JSON + HTML)
        // laid out as a technical note (Pro-tier entitlement; --dev bypasses it).
        Some("study") => study_cmd(&args),
        Some("lab-report") => lab_report_cmd(&args),
        Some("healthcheck") => {
            let addr = flag(&args, "--addr").unwrap_or_else(|| "127.0.0.1:8088".to_string());
            if crate::serve::healthcheck(&addr) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some("license") => license_cmd::run(&args[1..]),
        Some("payments") => payments_cmd::run(&args[1..]),
        Some("packs") => packs_cmd::run(&args[1..]),
        Some("dossier") => dossier_cmd::run(&args[1..]),
        Some("forensics") => forensics_cmd::run(&args[1..]),
        Some("backup") => backup_cmd(&args[1..]),
        Some("timing") => timing_cmd::run(&args[1..]),
        Some("scorecard") => scorecard_cmd::run(&args[1..]),
        Some("threats") => threats_cmd::run(&args[1..]),
        Some("attack-iq") => attack_iq_cmd::run(&args[1..]),
        Some("trust") => trust_cmd::run(&args[1..]),
        Some("eloran") => eloran_cmd::run(&args[1..]),
        Some("lunar") => lunar_cmd::run(&args[1..]),
        Some("area") => area_cmd::run(&args[1..]),
        Some("alerts") => alerts_cmd::run(&args[1..]),
        Some("integrity") => evidence_cmd::integrity(&args[1..]),
        Some("verify-service") => evidence_cmd::verify_service(&args[1..]),
        Some("frameworks") => evidence_cmd::frameworks(&args[1..]),
        Some("exposure") => evidence_cmd::exposure(&args[1..]),
        Some("pics") => pics_cmd::run(&args[1..]),
        Some("pntdp") => pntdp_cmd::run(&args[1..]),
        Some("verdict") => verdict_cmd::run(&args[1..]),
        Some("reports") => reports_cmd::run(&args[1..]),
        Some("testkit") => testkit_cmd::run(&args[1..]),
        Some("aviation") => aviation_cmd::run(&args[1..]),
        Some("obs-provenance") => evidence_infra_cmd::provenance(&args[1..]),
        Some("longterm") => evidence_infra_cmd::longterm(&args[1..]),
        // A command of a registered server-side extension (crate::extension), if any.
        Some(name) if crate::extension::commands().iter().any(|c| c.name == name) => {
            let c = crate::extension::commands()
                .iter()
                .find(|c| c.name == name)
                .expect("checked above");
            (c.run)(&args)
        }
        _ => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
    }
}

/// `kshana-pro clients --out <dir>`: write the Python and TypeScript service clients, generated
/// from the route table (`crate::serve::clients`).
fn clients_cmd(args: &[String]) -> ExitCode {
    let Some(out) = flag(args, "--out") else {
        eprintln!("usage: kshana-pro clients --out <dir>");
        return ExitCode::from(2);
    };
    let out = std::path::Path::new(&out);
    for (sub, file, text) in [
        (
            "python",
            "kshana_pro_client.py",
            crate::serve::clients::python(),
        ),
        (
            "typescript",
            "kshana_pro_client.ts",
            crate::serve::clients::typescript(),
        ),
    ] {
        let dir = out.join(sub);
        if let Err(e) =
            std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(file), text))
        {
            eprintln!("error: {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

/// `kshana-pro keys add|revoke|list --file <keys.toml> ...`: manage the service's API keys.
/// A new key is printed once on standard output; the file keeps only its SHA-256.
fn keys_cmd(args: &[String]) -> ExitCode {
    use crate::serve::auth::{is_name, new_key, KeysFile, Role, ALL_TENANTS};
    let usage = || {
        eprintln!("usage: kshana-pro keys <add --file <keys.toml> --id <id> --tenant <t> --role <reader|submitter|admin> | revoke --file <keys.toml> --id <id> | list --file <keys.toml>>");
        ExitCode::from(2)
    };
    let Some(file) = flag(args, "--file") else {
        return usage();
    };
    let mut keys = match std::fs::read_to_string(&file) {
        Ok(t) => match KeysFile::parse(&t) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("error: {file}: {e}");
                return ExitCode::FAILURE;
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => KeysFile::default(),
        Err(e) => {
            eprintln!("error: {file}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let save = |k: &KeysFile| -> ExitCode {
        let text = format!(
            "# Kshana Pro service API keys. Only SHA-256 hashes are stored; manage with `kshana-pro keys`.\n{}",
            k.to_toml()
        );
        match std::fs::write(&file, text) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {file}: {e}");
                ExitCode::FAILURE
            }
        }
    };
    match args.get(2).map(String::as_str) {
        Some("add") => {
            let (Some(id), Some(tenant), Some(role)) = (
                flag(args, "--id"),
                flag(args, "--tenant"),
                flag(args, "--role"),
            ) else {
                return usage();
            };
            let role = match role.as_str() {
                "reader" => Role::Reader,
                "submitter" => Role::Submitter,
                "admin" => Role::Admin,
                _ => return usage(),
            };
            if !is_name(&tenant) && tenant != ALL_TENANTS {
                return usage();
            }
            if keys.keys.iter().any(|k| k.id == id) {
                eprintln!("error: a key with id '{id}' exists; revoke it or pick another id");
                return ExitCode::FAILURE;
            }
            let (key, entry) = new_key(&id, &tenant, role);
            keys.keys.push(entry);
            if let Err(e) = KeysFile::parse(&keys.to_toml()) {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
            let rc = save(&keys);
            if rc == ExitCode::SUCCESS {
                println!("{key}");
                eprintln!(
                    "key '{id}' ({}, tenant {tenant}) added to {file}; it is shown only now",
                    role.as_str()
                );
            }
            rc
        }
        Some("revoke") => {
            let Some(id) = flag(args, "--id") else {
                return usage();
            };
            let Some(k) = keys.keys.iter_mut().find(|k| k.id == id) else {
                eprintln!("error: no key '{id}' in {file}");
                return ExitCode::FAILURE;
            };
            k.revoked = true;
            save(&keys)
        }
        Some("list") => {
            for k in &keys.keys {
                println!(
                    "{}\t{}\t{}\t{}",
                    k.id,
                    k.tenant,
                    k.role.as_str(),
                    if k.revoked { "revoked" } else { "active" }
                );
            }
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

fn serve_cmd(args: &[String]) -> ExitCode {
    // --config <serve.conf>: the settings file systemd reads, for a service manager
    // without environment files (the Windows service). The command line wins over it.
    let mut args = args.to_vec();
    if let Some(path) = flag(&args, "--config") {
        let p = std::path::Path::new(&path);
        let dir = p.parent().unwrap_or(std::path::Path::new("."));
        let st = std::fs::read_to_string(p)
            .map_err(|e| format!("could not read {path}: {}", e.kind()))
            .and_then(|t| crate::serve::conf::parse(&t, dir, &args));
        match st {
            Ok(st) => {
                if let Some(h) = st.licence_home {
                    // Read by the licence store below; nothing else runs yet.
                    std::env::set_var("KSHANA_PRO_HOME", h);
                }
                args.extend(st.args);
            }
            Err(e) => {
                eprintln!("error: --config {path}: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let args = &args[..];
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
            arksa: true,
        }
    } else {
        match flag(args, "--license") {
            Some(tok) => {
                match license_cmd::verify_token_for(&tok, now_unix(), license::Product::Service) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("error: license invalid: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            None => license_cmd::stored_or_evaluation(now_unix(), license::Product::Service),
        }
    };
    // The on-premises job service: --data-dir turns it on; --workers sizes the pool;
    // --api-token-file guards every route except /health. The token is read from the
    // file, never from the command line, and is never printed.
    let mut config = crate::serve::ServeConfig::new(&addr, ent);
    // The Station fleet limit (L3, F10e): the licence's stations claim; a development
    // server is not limited.
    config.stations_claim = if has_flag(args, "--dev") {
        u32::MAX
    } else {
        license_cmd::stations_claim(
            flag(args, "--license").as_deref(),
            now_unix(),
            license::Product::Service,
        )
    };
    config.data_dir = flag(args, "--data-dir").map(std::path::PathBuf::from);
    if let Some(w) = flag(args, "--workers") {
        match w.parse::<usize>() {
            Ok(n) if (1..=crate::serve::worker::MAX_WORKERS).contains(&n) => config.workers = n,
            _ => {
                eprintln!(
                    "error: --workers must be an integer from 1 to {}",
                    crate::serve::worker::MAX_WORKERS
                );
                return ExitCode::from(2);
            }
        }
    }
    if let Some(path) = flag(args, "--api-token-file") {
        match std::fs::read_to_string(&path) {
            Ok(s) if !s.trim().is_empty() => config.api_token = Some(s.trim().to_string()),
            Ok(_) => {
                eprintln!("error: the token file {path} is empty");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("error: could not read the token file {path}: {}", e.kind());
                return ExitCode::FAILURE;
            }
        }
    }
    if let Some(path) = flag(args, "--keys-file") {
        match std::fs::read_to_string(&path)
            .map_err(|e| e.kind().to_string())
            .and_then(|t| crate::serve::auth::KeysFile::parse(&t))
        {
            Ok(k) => config.keys = Some(k),
            Err(e) => {
                eprintln!("error: {path}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    config.oidc = flag(args, "--oidc").map(std::path::PathBuf::from);
    if let Some(d) = flag(args, "--audit-retention-days") {
        match d.parse::<u32>() {
            Ok(n) if n >= 1 => config.audit_retention_days = n,
            _ => {
                eprintln!(
                    "error: --audit-retention-days must be a whole number of days, at least 1"
                );
                return ExitCode::from(2);
            }
        }
    }
    match (flag(args, "--tls-cert"), flag(args, "--tls-key")) {
        (Some(c), Some(k)) => config.tls = Some((c.into(), k.into())),
        (None, None) => {}
        _ => {
            eprintln!("error: --tls-cert and --tls-key go together");
            return ExitCode::from(2);
        }
    }
    if has_flag(args, "--trial") {
        let mut t = crate::serve::trial::TrialConfig::default();
        for (name, slot) in [
            (
                "--trial-jobs-per-day",
                &mut t.jobs_per_day as &mut dyn std::any::Any,
            ),
            (
                "--trial-max-run-secs",
                &mut t.max_run_secs as &mut dyn std::any::Any,
            ),
        ] {
            if let Some(v) = flag(args, name) {
                let ok = match slot.downcast_mut::<u32>() {
                    Some(x) => v.parse().map(|n: u32| *x = n).is_ok(),
                    None => slot
                        .downcast_mut::<u64>()
                        .map(|x| v.parse().map(|n: u64| *x = n).is_ok())
                        .unwrap_or(false),
                };
                if !ok {
                    eprintln!("error: {name} needs a whole number");
                    return ExitCode::from(2);
                }
            }
        }
        config.trial = Some(t);
        if has_flag(args, "--trial-tokens") {
            // S6: 14-day trial tokens, signed by the configured licence signer.
            match signer() {
                Ok(sg) => config.trial_signer = Some(sg),
                Err(e) => {
                    eprintln!("error: --trial-tokens: {e}");
                    return ExitCode::from(2);
                }
            }
        }
    }
    if let Some(p) = flag(args, "--hosted") {
        // C3: the hosted results-only service for secret methods.
        let setup = crate::serve::hosted::HostedConfig::from_file(std::path::Path::new(&p))
            .map_err(kshana::api::KshanaError::InvalidInput)
            .and_then(|c| {
                Ok(crate::serve::HostedSetup {
                    config: c,
                    signer: signer()?,
                    watermark_key: crate::watermark::key_from_env()?,
                })
            });
        match setup {
            Ok(s) => config.hosted = Some(s),
            Err(e) => {
                eprintln!("error: --hosted: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(node) = flag(args, "--ha-node") {
        let lease_ms = match flag(args, "--ha-lease-secs").map(|v| v.parse::<f64>()) {
            None => 30_000,
            Some(Ok(v)) if v > 0.0 => (v * 1000.0) as u64,
            _ => {
                eprintln!("error: --ha-lease-secs must be a positive number of seconds");
                return ExitCode::from(2);
            }
        };
        match crate::serve::ha::HaConfig::new(&node, lease_ms) {
            Ok(h) => config.ha = Some(h),
            Err(e) => {
                eprintln!("error: --ha-node: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if has_flag(args, "--windows-service") {
        return windows_service_run(config, &addr, flag(args, "--config"));
    }
    match crate::serve::serve_with(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: could not serve on {addr}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `serve --windows-service`: run under the Windows Service Control Manager (the MSI's
/// `kshana-pro` service). Elsewhere the flag is refused.
#[cfg(windows)]
fn windows_service_run(
    config: crate::serve::ServeConfig,
    addr: &str,
    conf: Option<String>,
) -> ExitCode {
    // A service has no console: errors go to service.log beside the settings file.
    let log = conf
        .as_deref()
        .and_then(|c| std::path::Path::new(c).parent())
        .map(|d| d.join("service.log"));
    match crate::serve::winsvc::run(config, log) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: the Windows service on {addr}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn windows_service_run(
    _config: crate::serve::ServeConfig,
    _addr: &str,
    _conf: Option<String>,
) -> ExitCode {
    eprintln!("error: --windows-service runs only on Windows (here, use systemd: deploy/linux/kshana-pro.service)");
    ExitCode::from(2)
}

/// `ci` — Pack C scenario-subscription CI. Parse a versioned scenario pack, enforce
/// the Pro-tier entitlement (or bypass with `--dev`), run every scenario through the
/// open engine, optionally gate the result against a stored baseline, and emit a
/// branded report. The exit code mirrors the gate verdict (0 pass / 1 regressions).
fn ci_cmd(args: &[String]) -> ExitCode {
    use std::fs;

    // Resolve the entitlement EXACTLY as run/serve do: --dev = full (Enterprise, no
    // enforcement); else a token enforces its tier; else Evaluation (fail-closed).
    let entitled = || -> Result<(), ExitCode> {
        let now = now_unix();
        let ent = if has_flag(args, "--dev") {
            eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
            Entitlement {
                licensee: "(dev)".into(),
                tier: Tier::Enterprise,
                expires_unix: 0,
                features: vec![],
                arksa: true,
            }
        } else {
            match flag(args, "--license") {
                Some(tok) => match verify_token(&tok, now) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("error: license invalid: {e}");
                        return Err(ExitCode::FAILURE);
                    }
                },
                None => license_cmd::stored_or_evaluation(now, license::Product::Cli),
            }
        };
        if let Err(e) = crate::license::ensure_ci_entitled(&ent, now) {
            eprintln!("{e}");
            return Err(ExitCode::FAILURE);
        }
        Ok(())
    };

    // Sub-forms: `ci history verify <file>` and `ci diff <old.json> <new.json>`.
    match args.get(1).map(String::as_str) {
        Some("history") => {
            if let Err(code) = entitled() {
                return code;
            }
            let (Some("verify"), Some(file)) = (args.get(2).map(String::as_str), args.get(3))
            else {
                eprintln!("error: usage: kshana-pro ci history verify <history.jsonl>");
                return ExitCode::from(2);
            };
            let text = match fs::read_to_string(file) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("error: could not read {file}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            return match crate::pack_c::history::verify(&text) {
                Ok(recs) => {
                    match recs.last() {
                        Some(last) => println!(
                            "history {file}: {} record(s), chain intact, last hash {}",
                            recs.len(),
                            last.hash
                        ),
                        None => println!("history {file}: empty (0 records)"),
                    }
                    ExitCode::SUCCESS
                }
                Err(b) => {
                    println!("history {file}: BROKEN: {b}");
                    ExitCode::FAILURE
                }
            };
        }
        Some("diff") => {
            if let Err(code) = entitled() {
                return code;
            }
            let (Some(a), Some(b)) = (args.get(2), args.get(3)) else {
                eprintln!(
                    "error: usage: kshana-pro ci diff <old ci.json> <new ci.json> [--json <out>]"
                );
                return ExitCode::from(2);
            };
            let read = |p: &String| -> Result<crate::pack_c::CiReport, String> {
                let t = fs::read_to_string(p).map_err(|e| format!("could not read {p}: {e}"))?;
                crate::pack_c::CiReport::from_json(&t).map_err(|e| format!("{p}: {e}"))
            };
            let (old, new) = match (read(a), read(b)) {
                (Ok(o), Ok(n)) => (o, n),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let d = crate::pack_c::diff::diff(&old, &new);
            print!("{}", crate::pack_c::diff::to_text(&d));
            if let Some(out) = flag(args, "--json") {
                if let Err(e) = fs::write(&out, crate::pack_c::diff::to_json(&d)) {
                    eprintln!("error: could not write {out}: {e}");
                    return ExitCode::FAILURE;
                }
                eprintln!("[wrote diff JSON to {out}]");
            }
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

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
    let pack = match crate::pack_c::parse_pack(&manifest_src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(code) = entitled() {
        return code;
    }

    // Optional baseline to gate against.
    let baseline = match flag(args, "--baseline") {
        Some(path) => match fs::read_to_string(&path) {
            Ok(s) => match crate::pack_c::CiReport::from_json(&s) {
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

    let ctx = crate::pack_c::CiContext {
        generated_utc,
        customer,
        thresholds,
    };

    let opts = crate::pack_c::WatchOptions {
        exact: has_flag(args, "--exact"),
    };
    let history_path = flag(args, "--history");
    let trend_path = flag(args, "--trend");
    if trend_path.is_some() && history_path.is_none() {
        eprintln!("error: --trend needs --history <file>");
        return ExitCode::from(2);
    }
    // Refuse to run onto a broken history: verify it before spending the run.
    if let Some(h) = &history_path {
        let checked = crate::pack_c::history::load(std::path::Path::new(h)).and_then(|r| {
            crate::pack_c::history::check_pack(&r, &pack.id)
                .map_err(|e| format!("history {h}: {e}"))
        });
        if let Err(e) = checked {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    }

    let report = match crate::pack_c::run_ci_with(&pack, base_dir, baseline.as_ref(), &ctx, &opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let branded = crate::report_pack_c::PackCReport::from_ci(
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

    if let Some(h) = &history_path {
        let rec = crate::pack_c::history::record_from_report(&report);
        match crate::pack_c::history::append(std::path::Path::new(h), rec) {
            Ok(c) => eprintln!("[appended history record {} to {h}]", c.record.index),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let (Some(h), Some(out)) = (&history_path, &trend_path) {
        let recs = match crate::pack_c::history::load(std::path::Path::new(h)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        };
        let title = format!("Campaign watch: {} {}", pack.id, pack.version);
        let html = crate::pack_c::trend::render_html(&recs, &title, pack.description.as_deref());
        let json_out = match out.strip_suffix(".html") {
            Some(stem) => format!("{stem}.json"),
            None => format!("{out}.json"),
        };
        if let Err(e) = fs::write(out, html)
            .and_then(|_| fs::write(&json_out, crate::pack_c::trend::render_json(&recs, &title)))
        {
            eprintln!("error: could not write the trend report: {e}");
            return ExitCode::FAILURE;
        }
        eprintln!("[wrote trend report to {out} and {json_out}]");
    }

    if let Some(ex) = report.watch.as_ref().and_then(|w| w.exact.as_ref()) {
        if ex.compared {
            eprintln!(
                "[ci] exact: {} result(s) changed{}{}",
                ex.changed.len(),
                if ex.changed.is_empty() { "" } else { ": " },
                ex.changed.join(", ")
            );
        } else {
            eprintln!("[ci] exact: no baseline member hashes to compare with");
        }
    }

    // Watched pack: the watches decide.
    if let Some(w) = report.watch.as_ref().filter(|w| w.watched) {
        let failed: Vec<&crate::pack_c::WatchOutcome> =
            w.outcomes.iter().filter(|o| o.failed()).collect();
        if failed.is_empty() {
            eprintln!("[ci] PASS ({} watch(es))", w.outcomes.len());
        } else {
            eprintln!(
                "[ci] WATCH FAILURES: {} of {}",
                failed.len(),
                w.outcomes.len()
            );
            for o in failed {
                let req = o
                    .requirement
                    .as_ref()
                    .filter(|r| r.verdict != "met")
                    .map(|r| format!("; requirement `{}` {}", r.criterion, r.verdict))
                    .unwrap_or_default();
                eprintln!("[ci]   {}: {}{req}", o.id, o.status);
            }
        }
        return ExitCode::from(report.exit_code() as u8);
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

/// `study --dossier` — build the reproducible study dossier for one scenario and
/// write `dossier.json`, `dossier.html` and the scenario as run into `--out`.
/// `lab-report`: the branded lab campaign report over an engine campaign output directory.
/// Client data is read at run time only and is never written into the repository (N6).
fn lab_report_cmd(args: &[String]) -> ExitCode {
    use crate::lab_campaign::{load, DataClass};
    use crate::report_lab_campaign::{render_html, LabReportOptions};
    let dir = match args.get(1) {
        Some(p) if !p.starts_with("--") => std::path::PathBuf::from(p),
        _ => {
            eprintln!(
                "error: lab-report needs a campaign output directory\n\n{}",
                usage()
            );
            return ExitCode::from(2);
        }
    };
    let data_class = match flag(args, "--data-class").as_deref() {
        Some("synthetic") => DataClass::Synthetic,
        Some("client-confidential") => DataClass::ClientConfidential,
        Some(other) => {
            eprintln!(
                "error: --data-class must be synthetic or client-confidential, not {other:?}"
            );
            return ExitCode::from(2);
        }
        None => {
            eprintln!(
                "error: lab-report needs --data-class <synthetic|client-confidential>; it is stated \
                 on every page and is never assumed"
            );
            return ExitCode::from(2);
        }
    };
    let Some(out) = flag(args, "--html") else {
        eprintln!("error: lab-report needs --html <out.html>\n\n{}", usage());
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
            arksa: true,
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
            None => license_cmd::stored_or_evaluation(now, license::Product::Cli),
        }
    };
    if let Err(e) = license::ensure_dossier_entitled(&ent, now) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    let campaign = match load(&dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    };
    let mut opts = LabReportOptions::new(data_class);
    opts.client = flag(args, "--client");
    opts.doc_id = flag(args, "--doc-id");
    opts.prepared_utc = flag(args, "--utc");
    opts.allow_incomplete = has_flag(args, "--allow-incomplete");
    if let Some(pdf_out) = flag(args, "--pdf") {
        match crate::report_lab_campaign_pdf::render_pdf(&campaign, &opts) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&pdf_out, &bytes) {
                    eprintln!("error: could not write {pdf_out}: {e}");
                    return ExitCode::FAILURE;
                }
                eprintln!("[wrote {pdf_out}: {} bytes]", bytes.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    match render_html(&campaign, &opts) {
        Ok(html) => match std::fs::write(&out, html.as_bytes()) {
            Ok(()) => {
                eprintln!(
                    "[wrote {out}: {} cell(s), DIGEST {} verified, {}]",
                    campaign.cells.len(),
                    campaign.digest,
                    data_class.label()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: could not write {out}: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

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
            arksa: true,
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
            None => license_cmd::stored_or_evaluation(now, license::Product::Cli),
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
    if let Some(kind) = crate::registry::peek_kind(&src) {
        if kind.starts_with(crate::registry::PRO_KIND_PREFIX) && !ent.allows(&kind, now) {
            eprintln!(
                "error: '{kind}' requires the '{}' tier",
                license::feature_min_tier(&kind).as_str()
            );
            return ExitCode::FAILURE;
        }
    }
    let opts = crate::study_dossier::DossierOptions {
        issued_utc: flag(args, "--utc"),
    };
    let dossier = match crate::study_dossier::build(&src, &opts) {
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

/// Parse a `--thresholds` spec into [`Thresholds`](crate::pack_c::Thresholds).
///
/// Accepts a bare number (the default relative tolerance, e.g. `0.1`), or
/// comma-separated `key=val` pairs (each a per-key override), or a leading bare
/// default followed by per-key pairs (e.g. `0.1,araim::fom.mean_hpl_m=0.02`). An
/// absent or empty spec yields the default (5%). Unparseable pairs are skipped with
/// a warning to stderr rather than failing the run.
fn parse_thresholds(spec: Option<&str>) -> crate::pack_c::Thresholds {
    use crate::pack_c::Thresholds;
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
    match crate::report::build_report("kshana-pro report", &src, &subject) {
        Ok(r) => {
            // `--pdf <out.pdf>` renders the same structured report to a pure-Rust
            // PDF (no external tool, no bundled font) and writes it to the path.
            if let Some(out) = flag(args, "--pdf") {
                match crate::report_pdf::to_pdf(&r) {
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

/// `uncertainty <study.toml> --out <dir>` — resolve the entitlement exactly as `mbse`
/// does (`--dev` in a debug build = full; else a token enforces its tier; else
/// Evaluation), require the `pro-uncertainty` feature, then run the study.
fn uncertainty_cmd(args: &[String]) -> ExitCode {
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!("{}", crate::batch::uq::cli::USAGE);
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
            arksa: true,
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
            None => license_cmd::stored_or_evaluation(now, license::Product::Cli),
        }
    };
    let feature = crate::batch::uq::FEATURE;
    if !ent.allows(feature, now) {
        eprintln!(
            "error: the uncertainty study requires the '{}' tier (or the '{feature}' feature on the licence); current license tier is '{}'",
            license::feature_min_tier(feature).as_str(),
            ent.tier.as_str()
        );
        return ExitCode::FAILURE;
    }
    match crate::batch::uq::cli::run(args) {
        Ok(code) => ExitCode::from(code),
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
pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Presence of a boolean `--flag`.
pub fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// The current Unix time in seconds.
pub fn now_unix() -> i64 {
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

/// A Unix time as a UTC ISO-8601 date and time (`2026-06-23T14:05:09Z`).
fn utc_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
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

fn verify_token(token: &str, now: i64) -> Result<Entitlement, kshana::api::KshanaError> {
    license_cmd::verify_token_for(token, now, license::Product::Cli)
}

/// The entitlement a command runs under, resolved as every Pro command does: `--dev` in a
/// debug build = everything, with no enforcement; else a `--license <token>`; else the
/// licence activated on this machine, or Evaluation. For an extension command
/// ([`crate::extension::ExtensionCommand`]). On an invalid token it prints the error and
/// returns the exit code.
pub fn resolve_entitlement(args: &[String], now: i64) -> Result<Entitlement, ExitCode> {
    if DEV_BUILD && has_flag(args, "--dev") {
        eprintln!("[DEV MODE — entitlement enforcement DISABLED (not for distribution)]");
        return Ok(Entitlement {
            licensee: "(dev)".into(),
            tier: Tier::Enterprise,
            expires_unix: 0,
            features: vec![],
            arksa: true,
        });
    }
    match flag(args, "--license") {
        Some(tok) => verify_token(&tok, now).map_err(|e| {
            eprintln!("error: license invalid: {e}");
            ExitCode::FAILURE
        }),
        None => Ok(license_cmd::stored_or_evaluation(
            now,
            license::Product::Cli,
        )),
    }
}

/// Resolve the entitlement ([`resolve_entitlement`]) and require `feature` through
/// [`license::check`]. The refusal for a hidden-tier feature names no feature and no
/// method. Returns the entitlement it checked.
pub fn require_feature(args: &[String], feature: &str) -> Result<Entitlement, ExitCode> {
    let now = now_unix();
    let ent = resolve_entitlement(args, now)?;
    license::check(&ent, feature, now).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::FAILURE
    })?;
    Ok(ent)
}

/// The vendor's licence signer, for `license issue` and the other vendor commands (key
/// custody S7-a, [`license::signer::from_env`]): `KSHANA_PRO_SIGNER=seed` (default) takes
/// the seed from `KSHANA_PRO_SIGNING_KEY` (64 hex characters, kept in the vendor's secret
/// store; a debug build without it uses the demo seed); `KSHANA_PRO_SIGNER=kms` signs
/// through AWS KMS (a build with the `kms` feature). Customers never need it.
fn signer() -> Result<license::signer::SharedSigner, kshana::api::KshanaError> {
    #[cfg(debug_assertions)]
    let demo = Some(DEMO_SEED);
    #[cfg(not(debug_assertions))]
    let demo = None;
    license::signer::from_env(demo)
}

fn run_file(path: &str, license_token: Option<String>, dev_mode: bool) -> ExitCode {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let routed = match crate::registry::peek_kind(&src) {
        Some(k) if k.starts_with(crate::registry::PRO_KIND_PREFIX) => {
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
        crate::registry::run(&src)
    } else {
        match license_token {
            Some(tok) => match verify_token(&tok, now) {
                Ok(ent) => {
                    eprintln!(
                        "[license: {} tier, licensee: {}]",
                        ent.tier.as_str(),
                        ent.licensee
                    );
                    crate::registry::run_gated(&src, &ent, now)
                }
                Err(e) => {
                    eprintln!("error: license invalid: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => crate::registry::run_gated(
                &src,
                &license_cmd::stored_or_evaluation(now, license::Product::Cli),
                now,
            ),
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
    for m in crate::registry::pro_kinds() {
        println!(
            "  {:<18} [{}] {}",
            m.name,
            license::feature_min_tier(m.name).as_str(),
            m.description
        );
    }
}

/// `backup create|verify` (roadmap F2a).
fn backup_cmd(args: &[String]) -> ExitCode {
    use crate::serve::backup;
    match args.first().map(String::as_str) {
        Some("create") => {
            let (Some(d), Some(o)) = (flag(args, "--data-dir"), flag(args, "--out")) else {
                eprintln!("usage: kshana-pro backup create --data-dir <dir> --out <backup-dir>");
                return ExitCode::from(2);
            };
            match backup::create(d.as_ref(), o.as_ref(), &utc_iso8601_now()) {
                Ok(m) => {
                    println!("backup of {d} written to {o}: {} file(s)", m.files.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("verify") => {
            let Some(dir) = args.get(1) else {
                eprintln!("usage: kshana-pro backup verify <backup-dir>");
                return ExitCode::from(2);
            };
            match backup::verify(dir.as_ref()) {
                Ok(r) => {
                    println!(
                        "{} file(s); ledger {} ({} delivered job(s))",
                        r.files,
                        if r.ledger_verified {
                            "verified"
                        } else {
                            "NOT verified"
                        },
                        r.delivered_jobs
                    );
                    for p in &r.problems {
                        println!("problem: {p}");
                    }
                    if r.passed() {
                        println!("VERIFIED");
                        ExitCode::SUCCESS
                    } else {
                        println!("NOT VERIFIED");
                        ExitCode::FAILURE
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("usage: kshana-pro backup <create|verify> ...");
            ExitCode::from(2)
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
    for m in crate::registry::pro_kinds() {
        let ok = ent.allows(m.name, now_unix());
        println!("  [{}] {}", if ok { "✓" } else { "✗" }, m.name);
    }
}
