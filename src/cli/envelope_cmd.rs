// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro envelope <format|dir>`: the operating envelope of a Pro output format, or of the
//! evidence directory given (research 2026-11-20 gaps, B2508). Free: it states scope only.

use std::path::Path;
use std::process::ExitCode;

pub const ENVELOPE_USAGE: &str = "usage: kshana-pro envelope <format-tag|evidence-dir>   The operating envelope (valid inputs, ranges, exclusions) of a Pro output (no licence needed)";

/// `kshana-pro envelope ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.first() else {
        eprintln!("{ENVELOPE_USAGE}");
        return ExitCode::FAILURE;
    };
    let p = Path::new(arg);
    let fmt = if p.is_dir() {
        match crate::envelope::detect(p) {
            Some(f) => f,
            None => {
                eprintln!("error: {arg}: no Kshana format in its top-level JSON");
                return ExitCode::FAILURE;
            }
        }
    } else {
        arg.clone()
    };
    let e = crate::envelope::for_format(&fmt);
    println!("{}", serde_json::to_string_pretty(&e).unwrap_or_default());
    ExitCode::SUCCESS
}
