// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The evidence pack and the tool-qualification support kit.
//!
//! **Evidence pack.** An applicant's requirements (their own identifiers and text, in a trace
//! file) are mapped to the procedures that test them; the pack collects the signed procedure
//! verdicts, checks their signatures against the keys the applicant trusts, and writes a
//! traceability matrix (requirement, procedure, run, verdict, log hash) and a test-record
//! summary in the shape test records are kept in airborne software and hardware programmes.
//! A requirement with no run, a failing run or an incomplete run is listed as such.
//!
//! **Tool-qualification support kit.** When an applicant uses this library to produce
//! verification evidence and takes credit for it without reviewing every output, their
//! process may call for a tool qualification (the applicant decides, under RTCA DO-178C
//! §12.2 and RTCA DO-330). The kit is material for that work: a draft of the tool operational
//! requirements, the verification cases run now on this build with expected and actual
//! results, the configuration index (tool and open engine versions, procedure hashes) and the
//! known limitations. It is the applicant's input, not a qualification of the tool.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::procedure::{self, ParamsFile, Procedure, ProcedureVerdict};
use super::{araim, library, sbas_msg, sbas_seq, sha256_hex};
use crate::license::KEY_LEN;
use crate::scorecards::levels::Verdict;

/// The format tag of a trace file.
pub const TRACE_FORMAT: &str = "kshana-avionics-trace/1";
/// The format tag of the evidence index.
pub const EVIDENCE_FORMAT: &str = "kshana-avionics-evidence-pack/1";
/// The format tag of the kit's verification results.
pub const KIT_FORMAT: &str = "kshana-avionics-tool-kit/1";

/// One applicant requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    #[serde(default)]
    pub text: String,
    pub procedures: Vec<String>,
}

/// A trace file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trace {
    pub format: String,
    #[serde(default)]
    pub project: String,
    #[serde(rename = "requirement")]
    pub requirements: Vec<Requirement>,
}

impl Trace {
    pub fn parse(src: &str) -> Result<Trace, String> {
        let t: Trace = toml::from_str(src).map_err(|e| format!("trace: {e}"))?;
        if t.format != TRACE_FORMAT {
            return Err(format!(
                "trace: format '{}' is not {TRACE_FORMAT}",
                t.format
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for r in &t.requirements {
            if !ids.insert(&r.id) {
                return Err(format!("trace: requirement '{}' twice", r.id));
            }
            if r.procedures.is_empty() {
                return Err(format!("trace: requirement '{}' names no procedure", r.id));
            }
        }
        Ok(t)
    }
}

/// One procedure run found on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub dir: String,
    pub procedure: String,
    pub procedure_sha256: String,
    pub device: String,
    pub verdict: String,
    pub met: usize,
    pub not_met: usize,
    pub not_assessed: usize,
    pub log_sha256: Option<String>,
    pub verdict_sha256: String,
    /// `trusted`, `untrusted key`, `broken` or `unsigned`.
    pub signature: String,
}

/// One row of the matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceRow {
    pub requirement: String,
    pub procedure: String,
    pub runs: Vec<String>,
    /// `pass`, `fail`, `incomplete` or `no run`.
    pub status: String,
}

/// The evidence index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePack {
    pub format: String,
    pub label: String,
    pub project: String,
    pub runs: Vec<RunRecord>,
    pub matrix: Vec<TraceRow>,
    pub requirements_with_gaps: Vec<String>,
}

/// Every `verdict.json` under `dirs` (a run directory or a directory of them).
pub fn find_verdicts(dirs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
        if d.join("verdict.json").is_file() {
            out.push(d.to_path_buf());
            return Ok(());
        }
        if depth == 0 {
            return Ok(());
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(d)
            .map_err(|e| format!("{}: {e}", d.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        for e in entries {
            walk(&e, out, depth - 1)?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    for d in dirs {
        walk(d, &mut out, 3)?;
    }
    Ok(out)
}

fn signature_status(dir: &Path, trust: &[[u8; KEY_LEN]]) -> String {
    if !dir.join(crate::signed_dossier::SIGNATURE_FILE).is_file() {
        return "unsigned".into();
    }
    match crate::signed_dossier::verify(dir, trust) {
        Ok(v) if v.passed() => "trusted".into(),
        Ok(v) if v.signature_ok && v.problems.is_empty() => "untrusted key".into(),
        _ => "broken".into(),
    }
}

/// Build the evidence pack from the run directories.
pub fn evidence_pack(
    trace: &Trace,
    run_dirs: &[PathBuf],
    trust: &[[u8; KEY_LEN]],
) -> Result<EvidencePack, String> {
    let mut runs = Vec::new();
    for d in run_dirs {
        let bytes =
            std::fs::read(d.join("verdict.json")).map_err(|e| format!("{}: {e}", d.display()))?;
        let v: ProcedureVerdict = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}/verdict.json: {e}", d.display()))?;
        runs.push(RunRecord {
            dir: d.display().to_string(),
            procedure: v.procedure,
            procedure_sha256: v.procedure_sha256,
            device: v.device.name,
            verdict: v.verdict,
            met: v.met,
            not_met: v.not_met,
            not_assessed: v.not_assessed,
            log_sha256: v.log_sha256,
            verdict_sha256: sha256_hex(&bytes),
            signature: signature_status(d, trust),
        });
    }
    let mut matrix = Vec::new();
    let mut gaps = Vec::new();
    for r in &trace.requirements {
        let mut gap = false;
        for p in &r.procedures {
            // Only runs whose signature is trusted count as evidence.
            let mine: Vec<&RunRecord> = runs
                .iter()
                .filter(|x| &x.procedure == p && x.signature == "trusted")
                .collect();
            let status = if mine.is_empty() {
                "no run"
            } else if mine.iter().any(|x| x.verdict == "fail") {
                "fail"
            } else if mine.iter().all(|x| x.verdict == "pass") {
                "pass"
            } else {
                "incomplete"
            };
            gap |= status != "pass";
            matrix.push(TraceRow {
                requirement: r.id.clone(),
                procedure: p.clone(),
                runs: mine.iter().map(|x| x.dir.clone()).collect(),
                status: status.into(),
            });
        }
        if gap {
            gaps.push(r.id.clone());
        }
    }
    Ok(EvidencePack {
        format: EVIDENCE_FORMAT.into(),
        label: super::LABEL.into(),
        project: trace.project.clone(),
        runs,
        matrix,
        requirements_with_gaps: gaps,
    })
}

/// Write the evidence pack's files into `out` (signing is the caller's).
pub fn write_evidence_pack(out: &Path, trace: &Trace, pack: &EvidencePack) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out).map_err(io)?;
    std::fs::write(
        out.join("evidence-index.json"),
        serde_json::to_string_pretty(pack).unwrap(),
    )
    .map_err(io)?;
    let mut csv = String::from("requirement,procedure,status,runs\n");
    for r in &pack.matrix {
        csv.push_str(&format!(
            "{},{},{},{}\n",
            r.requirement,
            r.procedure,
            r.status,
            r.runs.join(";")
        ));
    }
    std::fs::write(out.join("TRACEABILITY.csv"), csv).map_err(io)?;
    let mut md = format!(
        "# Test records: {}\n\n{}\n\n## Requirements\n\n| Requirement | Text | Procedure | Status |\n|---|---|---|---|\n",
        if pack.project.is_empty() { "(project)" } else { &pack.project },
        pack.label
    );
    for row in &pack.matrix {
        let text = trace
            .requirements
            .iter()
            .find(|r| r.id == row.requirement)
            .map(|r| r.text.replace('|', "/"))
            .unwrap_or_default();
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.requirement, text, row.procedure, row.status
        ));
    }
    md.push_str("\n## Runs\n\n| Run | Procedure | Equipment | Verdict | Met / not met / not assessed | Signature | Log SHA-256 |\n|---|---|---|---|---|---|---|\n");
    for r in &pack.runs {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} / {} / {} | {} | {} |\n",
            r.dir,
            r.procedure,
            r.device,
            r.verdict,
            r.met,
            r.not_met,
            r.not_assessed,
            r.signature,
            r.log_sha256.clone().unwrap_or_else(|| "-".into())
        ));
    }
    if !pack.requirements_with_gaps.is_empty() {
        md.push_str(&format!(
            "\nRequirements without a passing, trusted run for every procedure: {}\n",
            pack.requirements_with_gaps.join(", ")
        ));
    }
    std::fs::write(out.join("TEST-RECORDS.md"), md).map_err(io)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tool-qualification support kit
// ---------------------------------------------------------------------------

/// One verification case of the tool, run now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationCase {
    pub id: String,
    pub function: String,
    pub expected: String,
    pub actual: String,
    /// `pass` or `fail`.
    pub result: String,
}

/// The kit's results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KitResults {
    pub format: String,
    pub label: String,
    pub tool: String,
    pub tool_version: String,
    pub open_engine_version: String,
    pub method: String,
    pub cases: Vec<VerificationCase>,
    pub passed: usize,
    pub failed: usize,
}

/// The configuration index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigIndex {
    pub tool: String,
    pub tool_version: String,
    pub open_engine_version: String,
    pub method: String,
    pub target: String,
    pub procedures: Vec<(String, u32, String)>,
    pub formats: Vec<String>,
}

fn case(id: &str, function: &str, expected: &str, actual: String, ok: bool) -> VerificationCase {
    VerificationCase {
        id: id.into(),
        function: function.into(),
        expected: expected.into(),
        actual,
        result: if ok { "pass" } else { "fail" }.into(),
    }
}

/// Demonstration limits for the scoring cases (not values from any document).
fn demo_params() -> ParamsFile {
    let mut values = std::collections::BTreeMap::new();
    for (k, v) in [
        ("alert_time_s", 10.0),
        ("return_time_s", 30.0),
        ("fast_timeout_s", 30.0),
        ("message_loss_s", 10.0),
        ("hal_m", 40.0),
        ("val_m", 35.0),
    ] {
        values.insert(k.to_string(), v);
    }
    ParamsFile {
        format: procedure::PARAMS_FORMAT.into(),
        supplied_by: "tool verification (demonstration values, not from any document)".into(),
        values,
        sources: Default::default(),
    }
}

/// Run the tool's verification cases on this build.
pub fn verification_cases() -> Vec<VerificationCase> {
    let mut out = Vec::new();
    // F1: CRC-24Q.
    let c = sbas_msg::crc24q(b"123456789");
    out.push(case(
        "F1-crc24q-check-value",
        "F1 SBAS framing",
        "CRC-24Q of \"123456789\" = cde703",
        format!("{c:06x}"),
        c == 0xCDE703,
    ));
    // F1: every message type round-trips and a flipped bit fails parity.
    let msgs = [
        sbas_msg::Message::DoNotUse,
        sbas_msg::Message::Null,
        sbas_msg::Message::Mask {
            slots: vec![1, 2, 37, 120],
            iodp: 3,
        },
        sbas_msg::Message::Fast {
            mt: 4,
            iodf: 2,
            iodp: 1,
            prc_m: (0..13).map(|i| i as f64 * -1.5).collect(),
            udrei: (0..13).map(|i| i as u8).collect(),
        },
        sbas_msg::Message::Integrity {
            iodf: [3, 0, 1, 2],
            udrei: (0..51).map(|i| (i % 16) as u8).collect(),
        },
        sbas_msg::Message::Degradation {
            t_lat_s: 2,
            iodp: 2,
            ai: (0..51).map(|i| (i % 16) as u8).collect(),
        },
        sbas_msg::Message::Iono {
            band: 3,
            block: 1,
            delay_m: (0..15).map(|i| i as f64).collect(),
            givei: (0..15).map(|i| i as u8).collect(),
            iodi: 1,
        },
    ];
    for (k, m) in msgs.iter().enumerate() {
        let r = sbas_msg::encode(m, k as u64).and_then(|f| {
            let back = sbas_msg::decode(&f)?;
            let mut g = f;
            let v = g.get(100, 1);
            g.put(100, 1, v ^ 1);
            Ok((back == *m, !g.parity_ok()))
        });
        let ok = matches!(r, Ok((true, true)));
        out.push(case(
            &format!("F1-mt{}-round-trip", m.mt()),
            "F1 SBAS framing",
            "decode(encode(m)) = m; one flipped bit fails the parity",
            format!("{r:?}"),
            ok,
        ));
    }
    // F2 and F3: each SBAS procedure's stream is deterministic and decodes; scoring separates a
    // synthetic good log from a synthetic bad one.
    let params_file = demo_params();
    for (id, _) in library::PROCEDURES {
        let p = match library::get(id) {
            Ok(p) => p,
            Err(e) => {
                out.push(case(
                    &format!("F2-{id}-parses"),
                    "F2 procedures",
                    "the built-in procedure parses and validates",
                    e,
                    false,
                ));
                continue;
            }
        };
        let params = p.resolve_params(Some(&params_file));
        if let Some(seq) = &p.sequence {
            let a = sbas_seq::generate(seq);
            let b = sbas_seq::generate(seq);
            let ok = match (&a, &b) {
                (Ok(a), Ok(b)) => {
                    a.frames_sha256 == b.frames_sha256
                        && a.frames.iter().all(|f| match &f.hex {
                            None => true,
                            Some(h) => sbas_msg::Frame::from_hex(h)
                                .and_then(|fr| sbas_msg::decode(&fr))
                                .is_ok(),
                        })
                }
                _ => false,
            };
            out.push(case(
                &format!("F2-{id}-stream"),
                "F2 SBAS sequence generator",
                "two generations give the same frames hash; every frame decodes with valid parity",
                a.as_ref()
                    .map(|s| format!("frames {} hash {}", s.frames.len(), s.frames_sha256))
                    .unwrap_or_else(|e| e.clone()),
                ok,
            ));
        }
        if p.kind == procedure::Kind::Araim
            && p.araim.as_ref().map(|a| a.check) == Some(araim::AraimCheck::PublishedVectors)
        {
            match procedure::score(&p, &params, None, None) {
                Ok(v) => {
                    let ok = v.not_met == 0 && v.met > 0;
                    out.push(case(
                        &format!("F4-{id}"),
                        "F4 ARAIM reference",
                        "every reproducible published output within the source's tolerance",
                        format!(
                            "met {}, not met {}, not assessed {} (as the source notes)",
                            v.met, v.not_met, v.not_assessed
                        ),
                        ok,
                    ));
                }
                Err(e) => out.push(case(
                    &format!("F4-{id}"),
                    "F4 ARAIM reference",
                    "runs",
                    e,
                    false,
                )),
            }
            continue;
        }
        for good in [true, false] {
            let r = procedure::synth_log(&p, good).and_then(|l| {
                let bytes = l.to_json();
                procedure::score(&p, &params, Some(&l), Some(bytes.as_bytes()))
            });
            let (ok, actual) = match &r {
                Ok(v) if good => (
                    v.not_met == 0,
                    format!("{}: met {}, not met {}", v.verdict, v.met, v.not_met),
                ),
                Ok(v) => (
                    v.not_met > 0 || p.sequence.as_ref().is_some_and(only_continuity),
                    format!("{}: met {}, not met {}", v.verdict, v.met, v.not_met),
                ),
                Err(e) => (false, e.clone()),
            };
            out.push(case(
                &format!("F3-{id}-{}", if good { "good" } else { "bad" }),
                "F3 scoring",
                if good {
                    "a log that reacts in 1 s has no check not met"
                } else {
                    "a log that never reacts has a check not met"
                },
                actual,
                ok,
            ));
        }
    }
    out
}

/// A sequence whose only expectations are continuity (the never-reacting log passes them).
fn only_continuity(s: &sbas_seq::SequenceSpec) -> bool {
    s.events
        .iter()
        .all(|e| e.kind == sbas_seq::EventKind::FcStep)
}

/// The configuration index of this build.
pub fn config_index() -> ConfigIndex {
    ConfigIndex {
        tool: "kshana-pro avionics".into(),
        tool_version: env!("CARGO_PKG_VERSION").into(),
        open_engine_version: kshana::iq::campaign::runner::engine_version().to_string(),
        method: procedure::METHOD_ID.into(),
        target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        procedures: library::PROCEDURES
            .iter()
            .filter_map(|(id, _)| {
                let p: Procedure = library::get(id).ok()?;
                Some((p.id.clone(), p.version, p.sha256()))
            })
            .collect(),
        formats: [
            procedure::FORMAT,
            procedure::PARAMS_FORMAT,
            procedure::LOG_FORMAT,
            procedure::VERDICT_FORMAT,
            sbas_seq::FORMAT,
            sbas_seq::OUTPUT_FORMAT,
            super::recovery::FLEET_FORMAT,
            super::recovery::FORMAT,
            TRACE_FORMAT,
            EVIDENCE_FORMAT,
            KIT_FORMAT,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    }
}

const TOR: &str = "# Tool operational requirements (draft for the applicant)

This draft describes what the tool does, so the applicant can state the tool operational
requirements in their own process. Edit it; it is not a qualification of the tool.

## Tool and environment

- Tool: `kshana-pro avionics` (see CONFIGURATION-INDEX.json for the version, the open engine
  version and the hash of every procedure).
- Runs as a command-line program on the host listed in the configuration index; no network
  use; inputs and outputs are files.

## Functions

- **F1 SBAS framing.** Encodes and decodes SBAS L1 messages (types 0, 1, 2 to 5, 6, 7, 26,
  63): 250-bit frames, rotating preamble, CRC-24Q parity. Refuses out-of-range fields.
- **F2 SBAS sequence generator and procedures.** From a procedure file, produces a one-frame-
  per-second stream (or silence) with scripted events, deterministic for a given input
  (frames hash), and the expected-behaviour timeline, each item naming its reference time and
  the parameter that holds its limit.
- **F3 Scoring.** Reads the equipment log and the applicant's parameter file and gives each
  check met, not met or not assessed. A check whose limit was not supplied, or whose data the
  log lacks, is not assessed, never met. The verdict is pass only when every scored check is
  met.
- **F4 ARAIM reference.** Computes VPL, HPL, EMT and the vertical accuracy sigma with the open
  engine's reference airborne algorithm; reproduces the published WG-C worked examples within
  the tolerance the source states; compares equipment outputs within a tolerance.
- **F5 Recovery scorecard.** Aggregates recovery times per avionics type from the test kit's
  verdicts.
- **F6 Evidence pack.** Maps requirements to procedures and signed runs; only runs whose
  signature verifies against a trusted key count.
- **F7 Signing.** Every output directory is signed (Ed25519, SHA-256 per file); verification
  needs no licence.

## Inputs the applicant controls

Procedure files, the parameter file (limits, each with its source), the equipment logs, the
trace file, the trusted public keys.

## Abnormal inputs

Malformed files, unknown formats, out-of-range fields and non-increasing log times are refused
with an error; the tool writes no partial verdict.
";

const ACTIONS: &str = "# Applicant actions

This kit is input to the applicant's own tool qualification. The applicant:

1. Decides whether the tool needs qualification and at which level, under RTCA DO-178C §12.2
   (or the hardware or ground-system equivalent their programme applies), and records it in
   their plan.
2. Writes the tool operational requirements in their process, starting from
   TOOL-OPERATIONAL-REQUIREMENTS.md, and limits the use of the tool to the functions they
   take credit for.
3. Reviews VERIFICATION-RESULTS.json, re-runs `kshana-pro avionics kit` in their own
   environment, and adds verification cases of their own for their use of the tool (their
   procedures, their parameter values, their equipment log formats).
4. Places the configuration index under their configuration management and re-runs the kit
   when the tool, the open engine or a procedure changes (the hashes show what changed).
5. Maps this material to the objectives of RTCA DO-330 Annex A that apply at the level
   they determined; this kit does not make that mapping for them.
6. Checks each procedure against their own copy of the documents it is modelled on; the
   procedures cite sections only and the limits come from the applicant.
";

const LIMITS: &str = "# Known limitations

- Evidence label MODELLED: no avionics unit has been tested with this library in this build.
- SBAS: L1 message types 0, 1, 2 to 5, 6, 7, 26 and 63 only; no GEO navigation (MT9),
  long-term corrections (MT24/25), clock-ephemeris covariance (MT28), service (MT27) or
  degradation parameters (MT10). DFMC (L5) message formats are not built.
- The generator writes message bits; it does not drive a simulator. The frame file is neutral
  (t_s, mt, hex); importing it into a simulator is the laboratory's step.
- The equipment log is a neutral JSON format; converting a unit's output into it is the
  applicant's step and part of their verification of the tool's use.
- ARAIM: error models are constant per constellation in the case files; elevation-dependent
  airborne models are the applicant's input. Vertical (V-ARAIM) results are readiness evidence
  only. LEO ISM values in the built-in case are hypothetical.
- Recovery scorecard: depends on the scenario verdicts given; the operator's target is a
  parameter.
";

/// Write the kit into `out` (signing is the caller's).
pub fn write_kit(out: &Path) -> Result<KitResults, String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out).map_err(io)?;
    let cases = verification_cases();
    let passed = cases.iter().filter(|c| c.result == "pass").count();
    let cfg = config_index();
    let res = KitResults {
        format: KIT_FORMAT.into(),
        label: super::LABEL.into(),
        tool: cfg.tool.clone(),
        tool_version: cfg.tool_version.clone(),
        open_engine_version: cfg.open_engine_version.clone(),
        method: cfg.method.clone(),
        failed: cases.len() - passed,
        passed,
        cases,
    };
    std::fs::write(
        out.join("VERIFICATION-RESULTS.json"),
        serde_json::to_string_pretty(&res).unwrap(),
    )
    .map_err(io)?;
    std::fs::write(
        out.join("CONFIGURATION-INDEX.json"),
        serde_json::to_string_pretty(&cfg).unwrap(),
    )
    .map_err(io)?;
    std::fs::write(out.join("TOOL-OPERATIONAL-REQUIREMENTS.md"), TOR).map_err(io)?;
    std::fs::write(out.join("APPLICANT-ACTIONS.md"), ACTIONS).map_err(io)?;
    std::fs::write(out.join("KNOWN-LIMITATIONS.md"), LIMITS).map_err(io)?;
    Ok(res)
}

/// Whether a verification case set has no failure (used by tests and the CLI exit code).
pub fn all_passed(r: &KitResults) -> bool {
    r.failed == 0
}

/// Count of checks by verdict in a set (helper for reports).
pub fn tally(checks: &[procedure::Check]) -> (usize, usize, usize) {
    let n = |v: Verdict| checks.iter().filter(|c| c.verdict == v).count();
    (n(Verdict::Met), n(Verdict::NotMet), n(Verdict::NotAssessed))
}
