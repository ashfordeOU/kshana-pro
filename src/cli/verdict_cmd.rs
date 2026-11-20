// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! `kshana-pro verdict ...`: ask the hosted verdict service for a method that ships in no Pro
//! build, and verify what comes back ([`crate::remote_verdict`]).

use std::path::Path;
use std::process::ExitCode;

use super::{flag, has_flag};
use crate::license::activation::Store;
use crate::remote_verdict::{
    builtin_trusted_keys, request, verify, CurlVerdictTransport, SignedVerdict, METHODS,
};

const USAGE: &str = "kshana-pro verdict — hosted methods, answered as signed verdicts\n\
\n\
USAGE:\n\
    kshana-pro verdict <selfcal|survey|cause|calibrate|flight-integrity|rtk-quality|parametric-trigger|policy-wording|spaceweather|perception-eval|feeder-trust|archive-seal> <request.json> [--url <base>] [--license <token>] [--trust <key>] [--out <verdict.json>]\n\
        Send the request to the verdict service, authorised by your licence, and verify the\n\
        signed verdict against the service's public key before printing or saving it.\n\
\n\
    Methods:\n\
      selfcal  self-calibrating simulator loop\n\
      survey   integrity-driven ASF survey planner\n\
      cause    what caused a vessel or aircraft position episode: external jamming,\n\
               external spoofing, self-reported track falsification, equipment fault, or\n\
               insufficient evidence. The request holds the reported AIS/ADS-B track and\n\
               any independent tracks (radar, SAR, MLAT, LRIT, AIS relays), receiver\n\
               observables and regional interference reports (docs/REMOTE-VERDICTS.md)\n\
      calibrate  general parametric calibration of a simulator against telemetry: named\n\
               parameters with priors and bounds, a closed-form model per observable or\n\
               runs of your own simulator over a design (docs/REMOTE-VERDICTS.md)\n\
      flight-integrity  a signed per-flight positioning-integrity verdict for a drone flight:\n\
               the telemetry checks recomputed, your local attestation's hash bound and\n\
               compared, and whether a degradation was shared by other flights nearby or\n\
               regional reports (area) or specific to the aircraft. Build the request with\n\
               `kshana-pro utm attest ... --verdict-request <file>`. Decision support only\n\
      rtk-quality  RTK correction-service quality per mountpoint from monitoring receivers on\n\
               surveyed marks: availability, wrong-fix rate, accuracy, time to fix, outage scope\n\
      parametric-trigger  a neutral evaluation of a parametric GNSS-outage trigger definition:\n\
               fired / not fired / insufficient data, events and outage index; no payout figures\n\
      policy-wording  factual answers on synthetic, structured policy clauses from the\n\
               service's own signed verdicts; never a coverage position, not advice\n\
      spaceweather  space-weather advisory per location and service (RTK, PPP, single\n\
               frequency): go / caution / no-go, not a forecast\n\
      perception-eval  evaluation of a perception-aided positioning system's own outputs\n\
               against tester truth: integrity classes, errors, landmark recall; evaluation only\n\
      feeder-trust  trust class and score per feeder of a crowd-sourced monitoring network\n\
      archive-seal  a signed, chained Merkle seal over an archive period's entry digests\n\
\n\
    kshana-pro verdict verify <verdict.json> [--trust <key>]\n\
        Check a saved verdict: intact, and signed by the trusted key.\n\
\n\
    --url     the service base URL (https://...), else KSHANA_VERDICT_URL\n\
    --license a licence token, else the token activated on this machine\n\
    --trust   the service's Ed25519 public key (64 hex; several comma-separated during a key\n\
              rotation), else KSHANA_VERDICT_KEY, else the key built into this release\n\
\n\
The method runs on the service; only its outcome and signature come back.\n";

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::FAILURE
}

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

fn trusted_key(args: &[String]) -> Result<String, String> {
    flag(args, "--trust")
        .or_else(|| env("KSHANA_VERDICT_KEY"))
        .or_else(|| builtin_trusted_keys().map(str::to_string))
        .ok_or_else(|| "the service's public key is required (--trust <64 hex> or KSHANA_VERDICT_KEY): a verdict is only worth its signature".into())
}

fn summary(v: &SignedVerdict) -> String {
    format!(
        "verdict: {} ({}), signed by key {} at {} — signature valid",
        v.method, v.evidence_class, v.key_id, v.utc
    )
}

pub fn run(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str);
    if sub.is_none() || has_flag(args, "--help") || sub == Some("help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if sub == Some("verify") {
        let Some(path) = args.get(1).filter(|a| !a.starts_with("--")) else {
            return fail(USAGE);
        };
        let key = match trusted_key(args) {
            Ok(k) => k,
            Err(e) => return fail(e),
        };
        let v: SignedVerdict = match std::fs::read_to_string(path)
            .map_err(|e| format!("{path}: {e}"))
            .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("{path}: {e}")))
        {
            Ok(v) => v,
            Err(e) => return fail(e),
        };
        let problems = verify(&v, &key);
        if problems.is_empty() {
            println!("{}", summary(&v));
            return ExitCode::SUCCESS;
        }
        return fail(format!("verdict NOT valid: {}", problems.join("; ")));
    }
    let method = sub.unwrap_or_default();
    if !METHODS.iter().any(|(m, _)| *m == method) {
        return fail(format!("unknown verdict method '{method}'\n\n{USAGE}"));
    }
    let Some(req_path) = args.get(1).filter(|a| !a.starts_with("--")) else {
        return fail(USAGE);
    };
    let Some(url) = flag(args, "--url").or_else(|| env("KSHANA_VERDICT_URL")) else {
        return fail("the verdict service URL is required (--url or KSHANA_VERDICT_URL)");
    };
    let key = match trusted_key(args) {
        Ok(k) => k,
        Err(e) => return fail(e),
    };
    let token = match flag(args, "--license").or_else(|| Store::default_dir().map(Store::open).and_then(|s| s.token())) {
        Some(t) => t,
        None => return fail("a licence is required: --license <token>, or activate one with `kshana-pro license activate`"),
    };
    let body = match std::fs::read_to_string(req_path) {
        Ok(b) => b,
        Err(e) => return fail(format!("{req_path}: {e}")),
    };
    let t = CurlVerdictTransport { base_url: url };
    let v = match request(&t, method, &token, &body) {
        Ok(v) => v,
        Err(e) => return fail(e),
    };
    let problems = verify(&v, &key);
    if !problems.is_empty() {
        return fail(format!(
            "the answer is NOT a valid verdict from the trusted key: {}",
            problems.join("; ")
        ));
    }
    let json = serde_json::to_string_pretty(&v).unwrap_or_default();
    match flag(args, "--out") {
        Some(out) => {
            if let Err(e) = std::fs::write(Path::new(&out), json + "\n") {
                return fail(format!("{out}: {e}"));
            }
            println!("{} -> {out}", summary(&v));
        }
        None => {
            eprintln!("{}", summary(&v));
            println!("{json}");
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_lists_every_method() {
        for (m, _) in METHODS {
            assert!(
                USAGE.contains(&format!("\n{m}  ")),
                "{m} missing from usage"
            );
        }
        assert!(USAGE.contains(
            "<selfcal|survey|cause|calibrate|flight-integrity|rtk-quality|parametric-trigger|policy-wording|spaceweather|perception-eval|feeder-trust|archive-seal>"
        ));
    }
}
