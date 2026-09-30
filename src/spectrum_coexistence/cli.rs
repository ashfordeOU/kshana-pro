// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro coexist <study.toml> --out <dir>`: the command-line front end. The binary
//! resolves the licence first (exactly as for the other Pro commands) and then calls
//! [`run`].

use std::path::Path;

use kshana::api::KshanaError;

use super::render;

/// Usage text for `kshana-pro coexist`.
pub const USAGE: &str =
    "kshana-pro coexist — a whole signal plan and constellation against the open GNSS signals\n\
\n\
  kshana-pro coexist <study.toml> --out <dir> [--license <token>]\n\
\n\
Writes coexistence.result.json, coexistence.matrix.svg, coexistence.map.svg,\n\
coexistence.pareto.svg and coexistence.report.html into <dir>. In a debug build,\n\
--dev disables licence enforcement. See docs/SPECTRUM-COEXISTENCE.md.";

/// The files `coexist` writes, in order.
pub const OUTPUTS: [&str; 5] = [
    "coexistence.result.json",
    "coexistence.matrix.svg",
    "coexistence.map.svg",
    "coexistence.pareto.svg",
    "coexistence.report.html",
];

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// The first positional argument (skipping flags and their values).
fn positional(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--out" || a == "--license" {
            i += 2;
            continue;
        }
        if a.starts_with("--") {
            i += 1;
            continue;
        }
        return Some(a.clone());
    }
    None
}

/// Run the study and write every output into `dir`. Returns the result document.
pub fn write_outputs(src: &str, dir: &Path) -> Result<serde_json::Value, KshanaError> {
    let result = super::run_study(src)?;
    let matrix = render::matrix_svg(&result);
    let map = render::map_svg(&result);
    let pareto = render::pareto_svg(&result);
    let html = render::report_html(&result, &matrix, &map, &pareto);
    std::fs::create_dir_all(dir)
        .map_err(|e| err(format!("could not create {}: {e}", dir.display())))?;
    let contents = [super::to_json_string(&result), matrix, map, pareto, html];
    for (name, body) in OUTPUTS.iter().zip(contents.iter()) {
        let p = dir.join(name);
        std::fs::write(&p, body)
            .map_err(|e| err(format!("could not write {}: {e}", p.display())))?;
    }
    Ok(result)
}

/// Run `coexist` with its arguments (after the command word). Returns the exit code.
pub fn run(args: &[String]) -> Result<u8, KshanaError> {
    let Some(study) = positional(args) else {
        println!("{USAGE}");
        return Ok(2);
    };
    let dir =
        flag(args, "--out").ok_or_else(|| err(format!("coexist needs --out <dir>\n\n{USAGE}")))?;
    let src =
        std::fs::read_to_string(&study).map_err(|e| err(format!("could not read {study}: {e}")))?;
    let result = write_outputs(&src, Path::new(&dir))?;
    eprintln!(
        "[wrote {} files into {dir}; scenario hash {}]",
        OUTPUTS.len(),
        result["scenario_hash"].as_str().unwrap_or_default()
    );
    println!("{}", super::summary_line(&result));
    Ok(0)
}
