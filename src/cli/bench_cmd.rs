// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro product-bench <register|score|synth>` (research 2026-11-20 gaps, B1005;
//! docs/PRODUCT-BENCHMARK.md).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::product_bench;

use super::grid_cmd::{gate, signing};

pub const BENCH_USAGE: &str = "usage: kshana-pro product-bench <command>\n\
    register <protocol.toml>   Print the SHA-256 of a benchmark protocol to register before the data (no licence needed)\n\
    score <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]   Third-party interference product scorecard against truth: detection rate, latency, false reports per hour, affected-area accuracy on victim-receiver cells (Pro)\n\
    synth <dir>   Write a SYNTHETIC truth, two anonymised products, a protocol and a request (no licence needed)";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

/// `kshana-pro product-bench ...`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(arg) = args.get(1).filter(|a| !a.starts_with("--")).cloned() else {
        return fail(BENCH_USAGE);
    };
    let r: Result<String, String> = match args.first().map(String::as_str) {
        Some("register") => (|| {
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let p: product_bench::Protocol =
                toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            product_bench::validate_protocol(&p)?;
            println!("{}", product_bench::protocol_sha256(&p));
            Ok(format!(
                "[product-bench] protocol '{}' registered {}: put the hash above in the request as protocol_sha256",
                p.title, p.registered_utc
            ))
        })(),
        Some("synth") => (|| {
            let d = Path::new(&arg);
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            let (t, a, b, proto) = product_bench::synth();
            let p: product_bench::Protocol = toml::from_str(&proto).map_err(|e| e.to_string())?;
            let h = product_bench::protocol_sha256(&p);
            let req = format!(
                "title = \"SYNTHETIC product benchmark\"\nprotocol_sha256 = \"{h}\"\ntruth = \"truth.json\"\n\n[[products]]\nid = \"PRODUCT-A\"\nreports = \"product-a.json\"\n\n[[products]]\nid = \"PRODUCT-B\"\nreports = \"product-b.json\"\n\n[protocol]\n{proto}"
            );
            for (n, s) in [
                ("truth.json", t),
                ("product-a.json", a),
                ("product-b.json", b),
                ("protocol.toml", proto),
                ("request.toml", req),
            ] {
                std::fs::write(d.join(n), s).map_err(|e| e.to_string())?;
            }
            Ok(format!("[product-bench] SYNTHETIC benchmark in {arg}"))
        })(),
        Some("score") => (|| {
            gate(args, product_bench::FEATURE)?;
            let s = signing(args, "product-bench score")?;
            let src = std::fs::read_to_string(&arg).map_err(|e| format!("{arg}: {e}"))?;
            let q: product_bench::Request =
                toml::from_str(&src).map_err(|e| format!("{arg}: {e}"))?;
            let base = Path::new(&arg)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let b = product_bench::score(&q, &base)?;
            crate::transport::write_pack(
                Path::new(&s.out),
                &b,
                &product_bench::html(&b),
                "request.toml",
                &src,
            )?;
            crate::signed_dossier::sign(Path::new(&s.out), &s.seed, &s.signer, &s.utc)
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "[product-bench] {}\n[product-bench] signed scorecard in {}",
                b.verdict, s.out
            ))
        })(),
        _ => return fail(BENCH_USAGE),
    };
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
