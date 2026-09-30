// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro design <study.toml> --out <dir>`: the command-line front end. The binary
//! resolves the licence first (exactly as for the other Pro commands) and then calls
//! [`run`].

use std::path::Path;

use kshana::api::KshanaError;

use super::{run_study, Options};

/// Usage text for `kshana-pro design`.
pub const USAGE: &str = "kshana-pro design — the Pareto front of a design space, over any open Kshana scenario kind\n\
\n\
  kshana-pro design <study.toml> --out <dir> [--threads <n>]\n\
\n\
Writes into <dir>:\n\
  design.result.json   every evaluated design, the front, the knee, the reference design and the labels\n\
  design.pareto.svg    every evaluated design, the front as a line, the knee and the reference marked\n\
  design.report.html   one self-contained page: chart, front table, labels, reproducibility\n\
  front/index.csv      one row per front design\n\
  front/NNN.toml       each front design as a plain open scenario the free engine runs to the same numbers\n\
\n\
--threads changes the run time, never the result. `kshana-pro run <study.toml>` prints the same JSON.\n\
Every command takes [--license <token>] or, in a debug build, [--dev]. See docs/DESIGN-OPTIMISER.md.";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// The first positional argument (skipping flags and their values).
fn positional(args: &[String]) -> Option<String> {
    let takes_value = ["--out", "--threads", "--license"];
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if takes_value.contains(&a.as_str()) {
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

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

fn write(path: &Path, content: &str) -> Result<(), KshanaError> {
    std::fs::write(path, content)
        .map_err(|e| err(format!("could not write {}: {e}", path.display())))
}

/// True for a file name this command writes into `front/`: `index.csv`, or digits and
/// `.toml` (`001.toml`).
fn is_front_file(name: &str) -> bool {
    name == "index.csv"
        || name
            .strip_suffix(".toml")
            .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()))
}

/// Remove what an earlier run of this command wrote into `front/`, and nothing else: the
/// folder may hold files of the user's, and those are left alone.
fn clear_front_files(front_dir: &Path) -> Result<(), KshanaError> {
    let entries = std::fs::read_dir(front_dir)
        .map_err(|e| err(format!("could not read {}: {e}", front_dir.display())))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| err(format!("could not read {}: {e}", front_dir.display())))?;
        let path = entry.path();
        if path.is_file() && is_front_file(&entry.file_name().to_string_lossy()) {
            std::fs::remove_file(&path)
                .map_err(|e| err(format!("could not remove {}: {e}", path.display())))?;
        }
    }
    Ok(())
}

/// Run `kshana-pro design` (`args` are the arguments after `design`). Returns the
/// process exit code.
pub fn run(args: &[String]) -> Result<u8, KshanaError> {
    let Some(study_path) = positional(args) else {
        println!("{USAGE}");
        return Ok(if args.is_empty() { 0 } else { 2 });
    };
    let out =
        flag(args, "--out").ok_or_else(|| err(format!("design needs --out <dir>\n\n{USAGE}")))?;
    let threads = match flag(args, "--threads") {
        Some(t) => Some(
            t.parse::<usize>()
                .ok()
                .filter(|n| *n >= 1)
                .ok_or_else(|| err(format!("--threads {t}: give a whole number, 1 or more")))?,
        ),
        None => None,
    };
    let src = std::fs::read_to_string(&study_path)
        .map_err(|e| err(format!("could not read {study_path}: {e}")))?;
    let outcome = run_study(&src, &Options { threads })?;

    let dir = Path::new(&out);
    let front_dir = dir.join("front");
    std::fs::create_dir_all(&front_dir)
        .map_err(|e| err(format!("could not create {}: {e}", front_dir.display())))?;
    // A front folder from an earlier run would leave stale scenario files behind.
    clear_front_files(&front_dir)?;
    write(
        &dir.join("design.result.json"),
        &(outcome.json.clone() + "\n"),
    )?;
    write(&dir.join("design.pareto.svg"), &outcome.svg)?;
    write(&dir.join("design.report.html"), &outcome.html)?;
    write(&front_dir.join("index.csv"), &outcome.index_csv)?;
    for (name, text) in &outcome.front_files {
        write(&front_dir.join(name), text)?;
    }
    eprintln!("{}", outcome.summary);
    eprintln!(
        "[wrote design.result.json, design.pareto.svg, design.report.html, front/index.csv and {} front scenario file(s) to {}]",
        outcome.front_files.len(),
        dir.display()
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_commands_own_front_files_are_cleared() {
        for name in ["index.csv", "001.toml", "0042.toml", "9.toml"] {
            assert!(is_front_file(name), "{name}");
        }
        for name in [
            "notes.txt",
            "my-design.toml",
            ".toml",
            "001.toml.bak",
            "index.csv.old",
            "12a.toml",
            "001.json",
        ] {
            assert!(!is_front_file(name), "{name}");
        }
    }
}
