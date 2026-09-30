// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `kshana-pro uncertainty` command: run a study with a checkpoint, so that it can
//! be stopped and resumed, and write its result, charts and report.

use std::path::Path;

use super::{render, run_study, RunOptions, StudyResult};

/// The file the finished realisations are appended to.
pub const CHECKPOINT_FILE: &str = "runs.jsonl";

/// Help text of the command.
pub const USAGE: &str = "kshana-pro uncertainty — uncertainty and sensitivity of any open scenario\n\
\n\
USAGE:\n\
  kshana-pro uncertainty <study.toml> --out <dir> [--resume] [--max-runs <N>] [--threads <N>] [--license <token>|--dev]\n\
\n\
  <study.toml>    a study file, kind = \"pro-uncertainty\" (docs/UNCERTAINTY.md)\n\
  --out <dir>     where the files are written (created if missing)\n\
  --resume        continue the runs.jsonl already in <dir>; finished realisations are\n\
                  skipped. Refused if the study changed since that file was started\n\
  --max-runs <N>  stop cleanly after N new runs; continue later with --resume\n\
  --threads <N>   worker threads (the result does not depend on it)\n\
\n\
Files written to <dir>:\n\
  runs.jsonl           one line per finished realisation (the checkpoint)\n\
  uq.result.json       every number, with units, tiers and the reproducibility stamp\n\
  uq.bands.svg         5th to 95th percentile band and median of each series output\n\
  uq.sensitivity.svg   ranked sensitivity of each output to each input\n\
  uq.convergence.svg   the estimates settling as runs accumulate\n\
  uq.report.html       a self-contained page with the probability of meeting each limit\n\
\n\
`kshana-pro run <study.toml>` runs the same study in memory, with no checkpoint, and\n\
prints the result document.";

/// The files a finished study is written as: `(file name, content)`.
pub fn output_files(result: &StudyResult) -> Vec<(&'static str, String)> {
    vec![
        ("uq.result.json", format!("{}\n", result.json)),
        (
            "uq.bands.svg",
            format!("{}\n", render::bands_svg(&result.doc)),
        ),
        (
            "uq.sensitivity.svg",
            format!("{}\n", render::sensitivity_svg(&result.doc)),
        ),
        (
            "uq.convergence.svg",
            format!("{}\n", render::convergence_svg(&result.doc)),
        ),
        ("uq.report.html", render::report_html(&result.doc)),
    ]
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn count_flag(args: &[String], name: &str) -> Result<Option<usize>, String> {
    match args.iter().position(|a| a == name) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse::<usize>().ok())
            .map(Some)
            .ok_or_else(|| format!("{name} needs a whole number")),
    }
}

/// Run the command. `args` are the words after `uncertainty`. Returns the process exit
/// code: 0 when the study finished or stopped cleanly at `--max-runs`.
pub fn run(args: &[String]) -> Result<u8, String> {
    let Some(study_path) = args.first().filter(|a| !a.starts_with("--")) else {
        println!("{USAGE}");
        return Ok(if args.is_empty() { 0 } else { 2 });
    };
    let out_dir = flag(args, "--out").ok_or("uncertainty needs --out <dir>")?;
    let max_runs = count_flag(args, "--max-runs")?;
    let threads = count_flag(args, "--threads")?;
    if threads == Some(0) {
        return Err("--threads must be at least 1".into());
    }
    let src = std::fs::read_to_string(study_path)
        .map_err(|e| format!("could not read {study_path}: {e}"))?;
    let dir = Path::new(&out_dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {out_dir}: {e}"))?;

    let opts = RunOptions {
        threads,
        checkpoint: Some(dir.join(CHECKPOINT_FILE)),
        resume: args.iter().any(|a| a == "--resume"),
        max_runs,
    };
    let run = run_study(&src, &opts)?;
    for note in &run.notes {
        eprintln!("[uncertainty] {note}");
    }
    eprintln!(
        "[uncertainty] study hash {} · {} of {} realisations finished ({} new)",
        run.study_hash, run.runs_done, run.runs_total, run.new_runs
    );
    match &run.result {
        Some(result) => {
            for (name, body) in output_files(result) {
                let p = dir.join(name);
                std::fs::write(&p, body)
                    .map_err(|e| format!("could not write {}: {e}", p.display()))?;
            }
            eprintln!(
                "[uncertainty] wrote uq.result.json, uq.bands.svg, uq.sensitivity.svg, \
                 uq.convergence.svg, uq.report.html and {CHECKPOINT_FILE} to {out_dir}"
            );
            println!("{}", result.summary);
        }
        None => {
            println!(
                "incomplete: {} of {} realisations finished; no result files yet",
                run.runs_done, run.runs_total
            );
        }
    }
    Ok(0)
}
