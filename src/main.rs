// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! `kshana-pro` — the Pro CLI. A thin front end over [`kshana_pro::registry::run`]
//! that demonstrates the superset: it runs **every open scenario** (delegated to
//! the open engine unchanged) **and** every Pro pack, from one binary.

use std::process::ExitCode;

fn usage() -> &'static str {
    "kshana-pro — proprietary overlay on the open Kshana engine\n\
     \n\
     USAGE:\n\
       kshana-pro run <scenario.toml>   Run an open OR Pro scenario (auto-routed)\n\
       kshana-pro kinds                 List all runnable kinds (open + Pro)\n\
       kshana-pro version               Print overlay + embedded-engine info\n\
     \n\
     Pro kinds carry the 'pro-' prefix and run through the Pro overlay; every other\n\
     kind is delegated to the embedded open engine unchanged."
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => match args.get(1) {
            Some(path) => run_file(path),
            None => {
                eprintln!("error: `run` needs a scenario file\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
        Some("kinds") => {
            list_kinds();
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
        _ => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
    }
}

fn run_file(path: &str) -> ExitCode {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let routed = match kshana_pro::registry::peek_kind(&src) {
        // Label on the prefix so it matches the dispatcher: every `pro-` kind is
        // owned by the overlay (it runs or fails closed there), never the engine.
        Some(k) if k.starts_with(kshana_pro::registry::PRO_KIND_PREFIX) => {
            format!("Pro overlay ({k})")
        }
        Some(k) => format!("open engine ({k})"),
        None => "open engine (default)".to_string(),
    };
    match kshana_pro::registry::run(&src) {
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
    println!("Open engine kinds (inherited by Pro):");
    for m in kshana::api::list_scenario_kinds() {
        println!("  {:<18} {}", m.name, m.description);
    }
    println!("\nPro overlay kinds (added on top):");
    for m in kshana_pro::registry::pro_kinds() {
        println!("  {:<18} {}", m.name, m.description);
    }
}
