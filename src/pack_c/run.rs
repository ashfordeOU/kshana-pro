// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **CI run orchestration** — drive a [`ScenarioPack`] through the open engine,
//! collect figures of merit plus provenance, and (optionally) gate the result
//! against a stored baseline, producing a [`CiReport`].
//!
//! This is the orchestrator that ties the earlier Pack C pieces together: the
//! versioned [`manifest`](crate::pack_c::manifest), the [`metrics`](crate::pack_c::metrics)
//! extractor, and the regression [`gate`](crate::pack_c::gate). For each pack
//! entry it reads the scenario file, runs it **only** through
//! [`crate::registry::run`] (the overlay never forks the open core), flattens the
//! result JSON into a [`MetricSet`], and records the engine version + scenario
//! hashes for reproducibility. The report renderer and the CLI are later concerns
//! and are *not* built here.
//!
//! Determinism: this library reads **no** clock and **no** randomness. Every
//! timestamp is caller-supplied via [`CiContext`]; the engine scenarios are
//! themselves deterministic given their seed, and the [`BTreeMap`]-backed
//! [`MetricSet`] keeps serialisation byte-stable.
//!
//! [`BTreeMap`]: std::collections::BTreeMap

use crate::pack_c::gate::{self, GateVerdict, Thresholds};
use crate::pack_c::manifest::ScenarioPack;
use crate::pack_c::metrics::{self, MetricSet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Caller-supplied context for a CI run: the wall-clock stamp (the library reads
/// no clock), an optional customer label, and the regression-gate thresholds.
#[derive(Clone, Debug, Default)]
pub struct CiContext {
    /// Caller-supplied UTC timestamp to stamp into the report (e.g. RFC 3339).
    pub generated_utc: Option<String>,
    /// Optional customer/tenant label carried into the report.
    pub customer: Option<String>,
    /// Thresholds used when a baseline is provided to gate against.
    pub thresholds: Thresholds,
}

/// The honesty label every scenario-subscription CI report carries, in the open engine's
/// `label` form (its HTML report shows the same tier inline).
pub const CI_LABEL: &str = "MODELLED - a scenario-subscription CI run: every figure is simulation output of the open engine for the pack's scenarios, compared with a stored baseline or the pack's stated limits. It shows whether modelled figures moved; it is not a measurement and not a certification.";

fn ci_label() -> String {
    CI_LABEL.to_string()
}

/// A reproducible, scored report for one CI run of a scenario pack.
///
/// Pins the pack identity + content hash, the engine version, and the per-scenario
/// hashes so a run can be reproduced exactly, alongside the flattened metrics and
/// (when a baseline was supplied) the regression-gate [`verdict`](CiReport::verdict).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CiReport {
    /// The pack's stable identifier.
    pub pack_id: String,
    /// The pack's semantic version.
    pub pack_version: String,
    /// The pack's deterministic content digest.
    pub pack_hash: String,
    /// The engine version that produced the run (first non-empty seen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    /// Per-scenario content hashes, sorted and deduplicated.
    pub scenario_hashes: Vec<String>,
    /// Caller-supplied UTC timestamp, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_utc: Option<String>,
    /// Caller-supplied customer label, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    /// The honesty label, [`CI_LABEL`]; a report written before the label existed reads
    /// back with it.
    #[serde(default = "ci_label")]
    pub label: String,
    /// The flattened figures of merit collected across every scenario.
    pub metrics: MetricSet,
    /// The regression-gate verdict, present only when a baseline was supplied.
    #[serde(default)]
    pub verdict: Option<GateVerdict>,
    /// The campaign-watch section: present only for a pack with `[[watch]]` tables or a
    /// run with `--exact`, so the report of any other run is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<WatchSection>,
}

impl CiReport {
    /// Serialise to deterministic pretty JSON. The [`MetricSet`] is `BTreeMap`-backed
    /// so the output is byte-stable for equal reports.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Parse a [`CiReport`] from JSON, mapping any serde error to a `String`.
    pub fn from_json(s: &str) -> Result<CiReport, String> {
        serde_json::from_str(s).map_err(|e| format!("invalid CI report: {e}"))
    }

    /// Whether the run passes: the watch verdict when the pack has watches, else the
    /// legacy regression gate (a run with no baseline passes as informational).
    pub fn passed(&self) -> bool {
        if let Some(w) = self.watch.as_ref().filter(|w| w.watched) {
            return w.passed;
        }
        self.verdict.as_ref().map(|v| v.passed).unwrap_or(true)
    }

    /// `0` when the run passes, `1` for a regression, an unmet or unverifiable
    /// requirement, or a watched figure that could not be read.
    pub fn exit_code(&self) -> i32 {
        if self.passed() {
            0
        } else {
            1
        }
    }
}

/// One member scenario of a run, pinned by content hashes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemberResult {
    /// The entry label.
    pub label: String,
    /// The scenario path, relative to the pack.
    pub path: String,
    /// The scenario kind the result states.
    #[serde(default)]
    pub kind: Option<String>,
    /// SHA-256 of the scenario file as read.
    pub scenario_sha256: String,
    /// SHA-256 of the result document the engine returned.
    pub result_sha256: String,
    /// The evidence label the result states: VALIDATED, MODELLED, PARTNER or MIXED, or
    /// `not stated` when the result carries no label.
    pub evidence: String,
}

/// A requirement's verdict for one watch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RequirementOutcome {
    /// The criterion as written.
    pub criterion: String,
    /// `met`, `not met` or `not verified`.
    pub verdict: String,
    /// Why it is not met or not verified.
    #[serde(default)]
    pub reason: Option<String>,
    /// What was compared, in words, one line per field read.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// The outcome of one watch in one run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WatchOutcome {
    /// The watch identity (its name, or `entry::metric`).
    pub id: String,
    /// The entry label.
    pub entry: String,
    /// The watched figure's name.
    pub metric: String,
    /// The figure's unit.
    #[serde(default)]
    pub unit: Option<String>,
    /// The figure's value in this run (the mean, for an ensemble watch).
    #[serde(default)]
    pub value: Option<f64>,
    /// The baseline value.
    #[serde(default)]
    pub baseline: Option<f64>,
    /// `value - baseline`.
    #[serde(default)]
    pub change: Option<f64>,
    /// `(value - baseline) / |baseline|`, when the baseline is not zero.
    #[serde(default)]
    pub rel_change: Option<f64>,
    /// The better-or-worse direction applied.
    pub direction: crate::pack_c::watch::WatchDirection,
    /// The worsening allowed before a regression, in the figure's unit.
    #[serde(default)]
    pub allowance: Option<f64>,
    /// `regressed`, `improved`, `within-tolerance`, `unchanged`, `changed` (no declared
    /// direction), `no-significant-change`, `no-baseline` or `not-found`.
    pub status: String,
    /// Why the figure could not be read, or what the statistical gate found.
    #[serde(default)]
    pub reason: Option<String>,
    /// The requirement's verdict.
    #[serde(default)]
    pub requirement: Option<RequirementOutcome>,
    /// The ensemble statistics of this run (kept so this report can be a baseline).
    #[serde(default)]
    pub ensemble: Option<crate::pack_c::stat_gate::EnsembleStats>,
    /// The statistical verdict against the baseline ensemble.
    #[serde(default)]
    pub stat: Option<crate::pack_c::stat_gate::StatVerdict>,
    /// Requirement limits on the figure, in its unit.
    #[serde(default)]
    pub limits: Vec<crate::pack_c::watch::Limit>,
    /// The entry result's evidence label.
    pub evidence: String,
    /// The watch's note.
    #[serde(default)]
    pub note: Option<String>,
}

impl WatchOutcome {
    /// Whether this watch fails the run.
    pub fn failed(&self) -> bool {
        matches!(self.status.as_str(), "regressed" | "not-found")
            || self
                .requirement
                .as_ref()
                .map(|r| r.verdict != "met")
                .unwrap_or(false)
    }
}

/// The `--exact` comparison of member result hashes against the baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExactReport {
    /// False when the baseline carries no member hashes to compare with.
    pub compared: bool,
    /// Entries whose result SHA-256 changed.
    pub changed: Vec<String>,
    /// Entries whose result SHA-256 is identical.
    pub unchanged: Vec<String>,
    /// Entries new in this run.
    pub added: Vec<String>,
    /// Entries of the baseline absent from this run.
    pub removed: Vec<String>,
}

/// The campaign-watch section of a report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WatchSection {
    /// True when the pack has `[[watch]]` tables (false for an `--exact`-only run of a
    /// pack without them).
    pub watched: bool,
    /// Whether every watch holds.
    pub passed: bool,
    /// The rule that decided which leaves are figures.
    pub figures_rule: String,
    /// Entries whose result had no `units` block, so every numeric leaf was taken.
    #[serde(default)]
    pub fallback_entries: Vec<String>,
    /// The unit of each figure in `metrics`.
    #[serde(default)]
    pub units: BTreeMap<String, String>,
    /// Every member result, pinned by hashes.
    pub members: Vec<MemberResult>,
    /// The statistical test, stated when any watch uses it.
    #[serde(default)]
    pub stat_test: Option<String>,
    /// Every watch's outcome, in manifest order.
    #[serde(default)]
    pub outcomes: Vec<WatchOutcome>,
    /// The `--exact` comparison, when asked for.
    #[serde(default)]
    pub exact: Option<ExactReport>,
}

/// Options of a campaign-watch run that the legacy [`CiContext`] does not carry.
#[derive(Clone, Debug, Default)]
pub struct WatchOptions {
    /// Report any change of a member result's SHA-256 against the baseline.
    pub exact: bool,
}

/// Run every scenario in `pack` (in declaration order) through the open engine,
/// returning the merged [`MetricSet`], the sorted-deduplicated scenario hashes, and
/// the engine version (the first non-empty one seen across the runs).
///
/// Scenario files are read relative to `base_dir`. Any IO error, engine error, or
/// metric-extraction error is mapped to a descriptive `Err(String)` naming the pack
/// and the offending scenario path.
pub fn run_pack(
    pack: &ScenarioPack,
    base_dir: &Path,
) -> Result<(MetricSet, Vec<String>, Option<String>), String> {
    let mut metrics = MetricSet::default();
    let mut scenario_hashes: Vec<String> = Vec::new();
    let mut engine_version: Option<String> = None;

    for entry in &pack.entries {
        let path = base_dir.join(&entry.path);
        let src = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "pack '{}': cannot read scenario '{}': {e}",
                pack.id, entry.path
            )
        })?;

        let out = crate::registry::run(&src)
            .map_err(|e| format!("pack '{}': scenario '{}' failed: {e}", pack.id, entry.path))?;

        let extracted = metrics::extract(&entry.label(), &out.json)?;
        metrics.merge(extracted);

        // Pull top-level provenance from the result JSON (parsed once).
        let value: serde_json::Value = serde_json::from_str(&out.json).map_err(|e| {
            format!(
                "pack '{}': scenario '{}' produced invalid result JSON: {e}",
                pack.id, entry.path
            )
        })?;
        if let Some(hash) = value.get("scenario_hash").and_then(|v| v.as_str()) {
            if !hash.is_empty() {
                scenario_hashes.push(hash.to_string());
            }
        }
        if engine_version.is_none() {
            if let Some(ver) = value.get("engine_version").and_then(|v| v.as_str()) {
                if !ver.is_empty() {
                    engine_version = Some(ver.to_string());
                }
            }
        }
    }

    scenario_hashes.sort();
    scenario_hashes.dedup();

    Ok((metrics, scenario_hashes, engine_version))
}

/// Assemble a [`CiReport`] from already-collected run results — **pure**: no IO, no
/// engine, no clock. The verdict is computed only when a `baseline` is supplied, by
/// gating `metrics` against `baseline.metrics` under `ctx.thresholds`.
pub fn assemble_report(
    pack: &ScenarioPack,
    metrics: MetricSet,
    scenario_hashes: Vec<String>,
    engine_version: Option<String>,
    baseline: Option<&CiReport>,
    ctx: &CiContext,
) -> CiReport {
    let verdict = baseline.map(|b| gate::compare(&b.metrics, &metrics, &ctx.thresholds));
    CiReport {
        pack_id: pack.id.clone(),
        pack_version: pack.version.clone(),
        pack_hash: pack.pack_hash(),
        engine_version,
        scenario_hashes,
        generated_utc: ctx.generated_utc.clone(),
        customer: ctx.customer.clone(),
        label: ci_label(),
        metrics,
        verdict,
        watch: None,
    }
}

/// Run a pack end to end: [`run_pack`] to collect metrics + provenance, then
/// [`assemble_report`] to build the (optionally gated) [`CiReport`].
pub fn run_ci(
    pack: &ScenarioPack,
    base_dir: &Path,
    baseline: Option<&CiReport>,
    ctx: &CiContext,
) -> Result<CiReport, String> {
    let (metrics, scenario_hashes, engine_version) = run_pack(pack, base_dir)?;
    Ok(assemble_report(
        pack,
        metrics,
        scenario_hashes,
        engine_version,
        baseline,
        ctx,
    ))
}

/// One member scenario as run.
struct MemberRun {
    label: String,
    path: String,
    scenario_sha256: String,
    json: String,
    doc: serde_json::Value,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// The evidence label a result states in its `label` field.
fn evidence_label(doc: &serde_json::Value) -> String {
    let text = doc.get("label").and_then(|v| v.as_str()).unwrap_or("");
    let first: String = text
        .chars()
        .take_while(|c| c.is_ascii_uppercase())
        .collect();
    match first.as_str() {
        "VALIDATED" | "MODELLED" | "PARTNER" | "MIXED" => first,
        _ => "not stated".to_string(),
    }
}

fn run_members(pack: &ScenarioPack, base_dir: &Path) -> Result<Vec<MemberRun>, String> {
    let mut out = Vec::new();
    for entry in &pack.entries {
        let path = base_dir.join(&entry.path);
        let src = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "pack '{}': cannot read scenario '{}': {e}",
                pack.id, entry.path
            )
        })?;
        let run = crate::registry::run(&src)
            .map_err(|e| format!("pack '{}': scenario '{}' failed: {e}", pack.id, entry.path))?;
        let doc: serde_json::Value = serde_json::from_str(&run.json).map_err(|e| {
            format!(
                "pack '{}': scenario '{}' produced invalid result JSON: {e}",
                pack.id, entry.path
            )
        })?;
        out.push(MemberRun {
            label: entry.label(),
            path: entry.path.clone(),
            scenario_sha256: sha256_hex(src.as_bytes()),
            json: run.json,
            doc,
        });
    }
    Ok(out)
}

/// The embedded open engine's version, for a pack none of whose results echo it: the
/// smallest open run that does (a 20 s clock run), exactly as the study dossier asks.
fn embedded_engine_version() -> Option<String> {
    let probe = "kind = \"clock\"\nseed = 1\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 20.0\n[gnss]\nwindows = [ { t0 = 0.0, t1 = 20.0, state = \"nominal\" } ]\n[clock_quantum]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n[clock_classical]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n";
    let out = crate::registry::run(probe).ok()?;
    let v: serde_json::Value = serde_json::from_str(&out.json).ok()?;
    v.get("engine_version")
        .and_then(|x| x.as_str())
        .filter(|x| !x.is_empty())
        .map(str::to_string)
}

/// The statement of the figure rule carried in every watched report.
pub const FIGURES_RULE: &str = "a figure is a numeric leaf whose entry in the result's own \
    units block has provenance computed, modelled or derived, excluding leaves that only \
    describe the output's layout (grid sizes, bin widths, exported-file bookkeeping, run \
    counts); plus the named campaign figures; arrays longer than 8 are read through watches";

/// Run a pack with its watches (and, with `opts.exact`, the member-hash comparison).
///
/// A pack with no `[[watch]]` tables and no `--exact` takes exactly the legacy
/// [`run_ci`] path, so its report is byte-identical to the report before campaign watch
/// existed. With watches, the metrics are the figures of [`crate::pack_c::semantics`]
/// plus the campaign figures of [`crate::pack_c::campaign`], and only the watches decide
/// the verdict: every other figure is informational.
pub fn run_ci_with(
    pack: &ScenarioPack,
    base_dir: &Path,
    baseline: Option<&CiReport>,
    ctx: &CiContext,
    opts: &WatchOptions,
) -> Result<CiReport, String> {
    if pack.watches.is_empty() && !opts.exact {
        return run_ci(pack, base_dir, baseline, ctx);
    }
    let watched = !pack.watches.is_empty();
    // A watch finds its baseline by identity; a report of another pack shares none, so
    // every watch would read "no baseline" and the run would pass without comparing
    // anything. Refuse it instead of passing silently.
    if let Some(b) = baseline.filter(|b| watched && b.pack_id != pack.id) {
        return Err(format!(
            "pack '{}': the baseline is a report of pack '{}'; a watched pack is gated only \
             against a report of the same pack",
            pack.id, b.pack_id
        ));
    }
    let members = run_members(pack, base_dir)?;

    let mut metrics = MetricSet::default();
    let mut units: BTreeMap<String, String> = BTreeMap::new();
    let mut fallback_entries = Vec::new();
    let mut docs: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut scenario_hashes = Vec::new();
    let mut engine_version: Option<String> = None;
    for m in &members {
        if watched {
            let figs = crate::pack_c::semantics::figures_of(&m.label, &m.doc);
            if figs.fallback_all_leaves {
                fallback_entries.push(m.label.clone());
            }
            metrics.merge(figs.metrics);
            units.extend(figs.units);
            for nf in crate::pack_c::campaign::figures(&m.doc) {
                let key = format!("{}::campaign.{}", m.label, nf.name);
                metrics.0.insert(key.clone(), nf.value);
                units.insert(key, nf.unit);
            }
            let aug = crate::pack_c::campaign::augment(&m.doc)
                .map_err(|e| format!("pack '{}': entry '{}': {e}", pack.id, m.label))?;
            docs.insert(m.label.clone(), aug);
        } else {
            metrics.merge(metrics::extract(&m.label, &m.json)?);
        }
        if let Some(h) = m.doc.get("scenario_hash").and_then(|v| v.as_str()) {
            if !h.is_empty() {
                scenario_hashes.push(h.to_string());
            }
        }
        if engine_version.is_none() {
            if let Some(v) = m.doc.get("engine_version").and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    engine_version = Some(v.to_string());
                }
            }
        }
    }
    scenario_hashes.sort();
    scenario_hashes.dedup();
    if engine_version.is_none() {
        engine_version = embedded_engine_version();
    }

    let member_results: Vec<MemberResult> = members
        .iter()
        .map(|m| MemberResult {
            label: m.label.clone(),
            path: m.path.clone(),
            kind: m
                .doc
                .get("kind")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            scenario_sha256: m.scenario_sha256.clone(),
            result_sha256: sha256_hex(m.json.as_bytes()),
            evidence: evidence_label(&m.doc),
        })
        .collect();

    let mut outcomes = Vec::new();
    for w in &pack.watches {
        let evidence = member_results
            .iter()
            .find(|m| m.label == w.entry)
            .map(|m| m.evidence.clone())
            .unwrap_or_else(|| "not stated".into());
        let base_outcome = baseline
            .and_then(|b| b.watch.as_ref())
            .and_then(|s| s.outcomes.iter().find(|o| o.id == w.id()));
        let base_metric =
            baseline.and_then(|b| b.metrics.get(&format!("{}::{}", w.entry, w.figure_name())));
        outcomes.push(evaluate_watch(
            w,
            docs.get(&w.entry),
            evidence,
            base_outcome,
            base_metric,
            ctx.thresholds.default_rel,
        ));
    }

    let exact = if opts.exact {
        Some(exact_compare(baseline, &member_results))
    } else {
        None
    };
    let stat_test = pack
        .watches
        .iter()
        .any(|w| w.ensemble.is_some())
        .then(|| crate::pack_c::stat_gate::TEST_DESCRIPTION.to_string());
    let passed = outcomes.iter().all(|o| !o.failed());
    let verdict = if watched {
        None
    } else {
        baseline.map(|b| gate::compare(&b.metrics, &metrics, &ctx.thresholds))
    };
    Ok(CiReport {
        pack_id: pack.id.clone(),
        pack_version: pack.version.clone(),
        pack_hash: pack.pack_hash(),
        engine_version,
        scenario_hashes,
        generated_utc: ctx.generated_utc.clone(),
        customer: ctx.customer.clone(),
        label: ci_label(),
        metrics,
        verdict,
        watch: Some(WatchSection {
            watched,
            passed,
            figures_rule: if watched {
                FIGURES_RULE.to_string()
            } else {
                "every numeric leaf (legacy extraction; the pack has no watches)".to_string()
            },
            fallback_entries,
            units,
            members: member_results,
            stat_test,
            outcomes,
            exact,
        }),
    })
}

fn exact_compare(baseline: Option<&CiReport>, current: &[MemberResult]) -> ExactReport {
    let base: Option<BTreeMap<&str, &str>> = baseline.and_then(|b| b.watch.as_ref()).map(|w| {
        w.members
            .iter()
            .map(|m| (m.label.as_str(), m.result_sha256.as_str()))
            .collect()
    });
    let Some(base) = base else {
        return ExactReport {
            compared: false,
            changed: vec![],
            unchanged: vec![],
            added: vec![],
            removed: vec![],
        };
    };
    let mut r = ExactReport {
        compared: true,
        changed: vec![],
        unchanged: vec![],
        added: vec![],
        removed: vec![],
    };
    for m in current {
        match base.get(m.label.as_str()) {
            Some(h) if *h == m.result_sha256 => r.unchanged.push(m.label.clone()),
            Some(_) => r.changed.push(m.label.clone()),
            None => r.added.push(m.label.clone()),
        }
    }
    for label in base.keys() {
        if !current.iter().any(|m| m.label == *label) {
            r.removed.push(label.to_string());
        }
    }
    r
}

fn evaluate_watch(
    w: &crate::pack_c::watch::Watch,
    doc: Option<&serde_json::Value>,
    evidence: String,
    base_outcome: Option<&WatchOutcome>,
    base_metric: Option<f64>,
    default_rel: f64,
) -> WatchOutcome {
    use crate::pack_c::stat_gate::{self, EnsembleStats};
    use crate::pack_c::watch::resolve_metric;
    let source = format!("result units block of entry '{}'", w.entry);
    let direction = w.resolved_direction();
    let mut out = WatchOutcome {
        id: w.id(),
        entry: w.entry.clone(),
        metric: w.figure_name(),
        unit: None,
        value: None,
        baseline: None,
        change: None,
        rel_change: None,
        direction,
        allowance: None,
        status: "not-found".into(),
        reason: None,
        requirement: None,
        ensemble: None,
        stat: None,
        limits: vec![],
        evidence,
        note: w.note.clone(),
    };
    let Some(doc) = doc else {
        out.reason = Some(format!("entry '{}' has no result", w.entry));
        return out;
    };
    // The figure.
    let resolved: Result<(f64, Option<String>), String> = if let Some(c) = &w.crossing {
        crate::pack_c::campaign::sweep_crossing(doc, &w.metric, c).map(|(v, u)| (v, Some(u)))
    } else if let Some(e) = &w.ensemble {
        let read = |p: &str| resolve_metric(doc, p, &source);
        match (read(&e.mean), read(&e.std), read(&e.n)) {
            (Ok((m, _)), Ok((sd, _)), Ok((n, _))) => {
                out.ensemble = Some(EnsembleStats {
                    mean: m,
                    std: sd,
                    n,
                });
                resolve_metric(doc, &w.metric, &source)
            }
            (Err(x), _, _) | (_, Err(x), _) | (_, _, Err(x)) => Err(x),
        }
    } else {
        resolve_metric(doc, &w.metric, &source)
    };
    match resolved {
        Ok((v, u)) => {
            out.value = Some(v);
            out.unit = u;
        }
        Err(e) => {
            out.reason = Some(format!("not found: {e}"));
        }
    }
    out.limits = w.limits(out.unit.as_deref());

    // Against the baseline.
    let base_value = base_outcome.and_then(|o| o.value).or(base_metric);
    out.baseline = base_value;
    if let Some(v) = out.value {
        match base_value {
            None => out.status = "no-baseline".into(),
            Some(b) => {
                out.change = Some(v - b);
                out.rel_change = (b != 0.0).then(|| (v - b) / b.abs());
                if w.ensemble.is_some() {
                    match (base_outcome.and_then(|o| o.ensemble), out.ensemble) {
                        (Some(bs), Some(cs)) => {
                            match stat_gate::decide(bs, cs, direction, w.alpha()) {
                                Ok(sv) => {
                                    out.status = if sv.regressed() {
                                        "regressed"
                                    } else if sv.mean_improved {
                                        "improved"
                                    } else {
                                        "no-significant-change"
                                    }
                                    .into();
                                    out.reason = Some(format!(
                                    "mean: Welch t = {:.3}, df = {:.1}, p = {:.3e}{}; spread: F = {:.3} on ({:.0}, {:.0}) degrees of freedom, p = {:.3e}{}; alpha = {}",
                                    sv.t,
                                    sv.df,
                                    sv.p_mean,
                                    if sv.mean_regressed {
                                        " (significant, worse)"
                                    } else if sv.mean_improved {
                                        " (significant, better)"
                                    } else {
                                        ""
                                    },
                                    sv.f,
                                    sv.df_f.0,
                                    sv.df_f.1,
                                    sv.p_spread,
                                    if sv.spread_regressed { " (spread grew)" } else { "" },
                                    sv.alpha
                                ));
                                    out.stat = Some(sv);
                                }
                                Err(e) => {
                                    out.status = "not-found".into();
                                    out.reason = Some(format!("statistical gate: {e}"));
                                }
                            }
                        }
                        _ => {
                            out.status = "no-baseline".into();
                            out.reason = Some(
                                "the baseline carries no ensemble statistics for this watch".into(),
                            );
                        }
                    }
                } else {
                    let allowance = w
                        .tolerance
                        .clone()
                        .unwrap_or(crate::pack_c::watch::Tolerance {
                            rel: None,
                            abs: None,
                        })
                        .allowance(b, default_rel);
                    out.allowance = Some(allowance);
                    out.status = gate::judge_watch(direction, allowance, b, v).into();
                }
            }
        }
    }

    // The requirement, against the entry's own result, with `watched` bound.
    if let Some(req) = &w.requirement {
        out.requirement = Some(evaluate_requirement(
            req,
            doc,
            out.value,
            out.unit.as_deref(),
            &source,
        ));
    }
    out
}

fn evaluate_requirement(
    req: &str,
    doc: &serde_json::Value,
    value: Option<f64>,
    unit: Option<&str>,
    source: &str,
) -> RequirementOutcome {
    use crate::mbse::criterion;
    use crate::pack_c::watch::{DocUnits, WATCHED};
    let c = match criterion::parse(req) {
        Ok(c) => c,
        Err(e) => {
            return RequirementOutcome {
                criterion: req.to_string(),
                verdict: "not verified".into(),
                reason: Some(format!("the criterion does not parse: {e}")),
                evidence: vec![],
            }
        }
    };
    let mut bound = doc.clone();
    if let (Some(v), Some(obj)) = (value, bound.as_object_mut()) {
        if !obj.contains_key(WATCHED) {
            obj.insert(WATCHED.into(), serde_json::json!(v));
            if let Some(u) = unit {
                let units = obj
                    .entry("units")
                    .or_insert_with(|| serde_json::Value::Object(Default::default()));
                if let Some(m) = units.as_object_mut() {
                    m.insert(
                        WATCHED.into(),
                        serde_json::json!({"unit": u, "provenance": "computed"}),
                    );
                }
            }
        }
    }
    let ev = criterion::evaluate(
        &c,
        &bound,
        &DocUnits {
            doc: &bound,
            source,
        },
    );
    RequirementOutcome {
        criterion: req.to_string(),
        verdict: ev.verdict.as_str().to_string(),
        reason: ev.reason,
        evidence: ev.observations.iter().map(|o| o.detail.clone()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack_c::manifest::PackEntry;
    use std::collections::BTreeMap;

    /// A pack used purely to exercise the pure assembly path (never run).
    fn sample_pack() -> ScenarioPack {
        ScenarioPack {
            id: "resilience-baseline".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            entries: vec![PackEntry {
                path: "araim.toml".to_string(),
                label: Some("araim".to_string()),
            }],
            watches: vec![],
        }
    }

    fn metric_set(pairs: &[(&str, f64)]) -> MetricSet {
        MetricSet(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    fn ctx_with_stamp() -> CiContext {
        CiContext {
            generated_utc: Some("2026-01-01T00:00:00Z".to_string()),
            customer: Some("ACME".to_string()),
            thresholds: Thresholds::new(0.05),
        }
    }

    #[test]
    fn assemble_without_baseline_has_no_verdict_and_carries_fields() {
        let pack = sample_pack();
        let metrics = metric_set(&[("araim::fom.raim_availability", 0.9)]);
        let ctx = ctx_with_stamp();

        let report = assemble_report(
            &pack,
            metrics.clone(),
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );

        assert!(report.verdict.is_none());
        assert_eq!(report.pack_id, "resilience-baseline");
        assert_eq!(report.pack_version, "1.0.0");
        assert_eq!(report.pack_hash, pack.pack_hash());
        assert_eq!(report.metrics, metrics);
        assert_eq!(
            report.generated_utc.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(report.customer.as_deref(), Some("ACME"));
        assert_eq!(report.engine_version.as_deref(), Some("0.20.0"));
        assert_eq!(report.scenario_hashes, vec!["hash-a".to_string()]);
    }

    #[test]
    fn assemble_with_better_baseline_fails_the_gate() {
        let pack = sample_pack();
        // raim_availability is HigherIsBetter: baseline 0.9, current 0.7 => regression.
        let baseline_metrics = metric_set(&[("araim::fom.raim_availability", 0.9)]);
        let current_metrics = metric_set(&[("araim::fom.raim_availability", 0.7)]);
        let ctx = ctx_with_stamp();

        let baseline = assemble_report(
            &pack,
            baseline_metrics,
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );

        let report = assemble_report(
            &pack,
            current_metrics,
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            Some(&baseline),
            &ctx,
        );

        let verdict = report
            .verdict
            .expect("baseline supplied => verdict present");
        assert!(!verdict.passed);
    }

    #[test]
    fn assemble_with_equal_baseline_passes_the_gate() {
        let pack = sample_pack();
        let metrics = metric_set(&[("araim::fom.raim_availability", 0.9)]);
        let ctx = ctx_with_stamp();

        let baseline = assemble_report(
            &pack,
            metrics.clone(),
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );

        let report = assemble_report(
            &pack,
            metrics,
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            Some(&baseline),
            &ctx,
        );

        let verdict = report
            .verdict
            .expect("baseline supplied => verdict present");
        assert!(verdict.passed);
    }

    #[test]
    fn report_round_trips_and_to_json_is_deterministic() {
        let pack = sample_pack();
        let metrics = metric_set(&[
            ("araim::fom.raim_availability", 0.9),
            ("araim::fom.mean_hpl_m", 22.1),
        ]);
        let ctx = ctx_with_stamp();

        let baseline = assemble_report(
            &pack,
            metrics.clone(),
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );
        // Include a verdict so the round-trip exercises the gated branch too.
        let report = assemble_report(
            &pack,
            metrics,
            vec!["hash-a".to_string(), "hash-b".to_string()],
            Some("0.20.0".to_string()),
            Some(&baseline),
            &ctx,
        );

        let json = report.to_json();
        let parsed = CiReport::from_json(&json).expect("report JSON must parse back");
        assert_eq!(parsed, report);

        // Determinism: two serialisations are byte-identical.
        assert_eq!(report.to_json(), report.to_json());
    }
}
