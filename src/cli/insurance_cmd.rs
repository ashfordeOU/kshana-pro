// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro insurance <...>`: insurance and finance analytics (research 2026-11-20 gaps E28;
//! docs/INSURANCE-ANALYTICS.md). Producing commands write a directory and sign it; `verify`,
//! `schema`, `sample` and `bi models` are free.

use std::path::Path;
use std::process::ExitCode;

use crate::insurance::{self, basis, bi, feed, trigger};

use super::evidence_cmd::{producing, sign};
use super::flag;
use super::forensics_cmd::keys;
use super::scorecard_cmd::verify_dir;

pub const USAGE: &str = "usage: kshana-pro insurance <command>\n\
    trigger <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Parametric PNT-outage trigger on your own station C/N0 series: site episodes, k-of-n rule, outage index, modelled payouts, backtest (Pro)\n\
    basis <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Basis risk of a trigger against your loss records: shortfall, excess, ratio with bootstrap interval, hit rate, location basis (Pro)\n\
    accumulation <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Portfolio accumulation under historical event footprints and scenarios: per footprint, per cell, exceedance (Enterprise)\n\
    bi <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Business-interruption and loss quantification (timing, yield, port, grounding, cascade) with assumptions and sensitivity (Pro)\n\
    bi models\n\
        The models and their parameters (free)\n\
    feed <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>] [--dev]\n\
        Underwriting variables per vessel or aircraft as a schema'd JSON Lines feed (Pro)\n\
    triage <request.toml> --out <dir> --signer <name> --key-file <file> [--trust <hex>[,...]] [--verdict-key <hex>[,...]] [--utc <iso>] [--license <token>] [--dev]\n\
        Claims triage aid: a claimed interference loss against events, signed evidence and a signed cause verdict (Pro)\n\
    schema\n\
        The feed's JSON Schema (free)\n\
    sample --out <dir>\n\
        A synthetic sample: inputs and the unsigned feed built from them (free)\n\
    verify <dir> --trust <signer public key hex>[,...]\n\
        Check a result's signature and every file, offline (free)\n\
\n\
Outputs are evidence, indicators and calculations for your own decision: not insurance or\n\
financial advice, not an underwriting, pricing or claims decision. All MODELLED.";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn produce(
    args: &[String],
    feature: &str,
    f: impl FnOnce(&Path, &Path) -> Result<String, String>,
) -> ExitCode {
    let Some(file) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let r = producing(args, feature).and_then(|(out, seed, signer, utc)| {
        let msg = f(Path::new(file), Path::new(&out))?;
        sign(&out, &seed, &signer, &utc)?;
        Ok(format!("{msg}; signed in {out}"))
    });
    match r {
        Ok(m) => {
            eprintln!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn models() -> ExitCode {
    for m in bi::MODELS {
        println!("{m}");
        for (name, unit, def, _, _, meaning) in bi::specs(m) {
            let d = def
                .map(|d| format!("default {d}"))
                .unwrap_or_else(|| "required".into());
            println!("  {name:<26} {unit:<14} {d:<14} {meaning}");
        }
    }
    ExitCode::SUCCESS
}

fn sample(args: &[String]) -> ExitCode {
    let Some(out) = flag(args, "--out") else {
        return fail("sample needs --out <dir>");
    };
    let r = (|| {
        let dir = Path::new(&out);
        let (req, tracks, events, areas) = feed::sample();
        for (n, t) in [
            ("request.toml", &req),
            ("tracks.csv", &tracks),
            ("events.json", &events),
            ("areas.toml", &areas),
        ] {
            insurance::write_text(dir, n, t.as_bytes())?;
        }
        let q: feed::FeedRequest = toml::from_str(&req).map_err(|e| e.to_string())?;
        let d = feed::load(&q, &Some(dir.to_path_buf()))?;
        let f = feed::build_feed(&q, d)?;
        insurance::write_text(dir, feed::FEED_FILE, feed::to_jsonl(&f).as_bytes())?;
        insurance::write_json(dir, "schema.json", &feed::schema())?;
        Ok::<usize, String>(f.records.len())
    })();
    match r {
        Ok(n) => {
            eprintln!("[insurance sample] SYNTHETIC inputs and a feed of {n} subjects in {out}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

/// `kshana-pro insurance ...`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("trigger") => produce(args, insurance::FEATURE_TRIGGER, |p, out| {
            let (q, src, base) = insurance::read_request::<trigger::TriggerRequest>(p)?;
            let r = trigger::evaluate(&q, &base)?;
            trigger::write_dir(out, &r, &src)?;
            Ok(format!(
                "[insurance trigger] {} event(s), {} fired, modelled payout {}",
                r.totals.events, r.totals.fired, r.totals.total_payout
            ))
        }),
        Some("basis") => produce(args, insurance::FEATURE_BASIS, |p, out| {
            let (r, src) = basis::basis(p)?;
            insurance::write_pack(out, basis::BASIS_FILE, &r, &basis::basis_html(&r), &src)?;
            Ok(format!(
                "[insurance basis] basis-risk ratio {}",
                r.metrics
                    .basis_risk_ratio
                    .map(|x| format!("{x:.3}"))
                    .unwrap_or_else(|| "n/a".into())
            ))
        }),
        Some("accumulation") => produce(args, insurance::FEATURE_ACCUMULATION, |p, out| {
            let (r, src) = basis::accumulation(p)?;
            insurance::write_pack(
                out,
                basis::ACC_FILE,
                &r,
                &basis::accumulation_html(&r),
                &src,
            )?;
            Ok(format!(
                "[insurance accumulation] {} footprints; largest historical event {} {}",
                r.footprints.len(),
                r.totals.largest_event_loss,
                r.currency
            ))
        }),
        Some("bi") if args.get(1).map(String::as_str) == Some("models") => models(),
        Some("bi") => produce(args, insurance::FEATURE_BI, |p, out| {
            let (r, src) = bi::run(p)?;
            bi::write_dir(out, &r, &src)?;
            Ok(format!(
                "[insurance bi] {}: {} {}",
                r.headline_name, r.headline, r.currency
            ))
        }),
        Some("feed") => produce(args, insurance::FEATURE_FEED, |p, out| {
            let (q, src, base) = insurance::read_request::<feed::FeedRequest>(p)?;
            let f = feed::build_feed(&q, feed::load(&q, &base)?)?;
            feed::write_feed(out, &f, &src)?;
            Ok(format!("[insurance feed] {} subject(s)", f.records.len()))
        }),
        Some("triage") => {
            let trust = match keys(args, "--trust") {
                Ok(k) => k,
                Err(e) => return fail(e),
            };
            let vk = flag(args, "--verdict-key").or_else(|| {
                std::env::var("KSHANA_VERDICT_KEY")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            });
            produce(args, insurance::FEATURE_TRIAGE, |p, out| {
                let (r, src) = feed::triage(p, &trust, vk.as_deref())?;
                insurance::write_pack(out, feed::TRIAGE_FILE, &r, &feed::triage_html(&r), &src)?;
                Ok(format!("[insurance triage] {}", r.outcome))
            })
        }
        Some("schema") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&feed::schema()).unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Some("sample") => sample(args),
        Some("verify") => verify_dir(args),
        _ => fail(USAGE),
    }
}
