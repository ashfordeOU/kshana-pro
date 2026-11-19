// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! # eLoran-Bench: a reference benchmark for eLoran receivers
//!
//! A fixed set of simulated scenarios with exact truth (`scenarios/bench-eloran/`), pass
//! bars **pre-registered by hash before any scoring**, and a scorer that takes a receiver's
//! [ELOF](crate::elof) observations and returns a sealed scorecard; scorecards enter a
//! leaderboard file.
//!
//! | file | content |
//! |---|---|
//! | `<scenario>.toml` | a scenario with a `[conformance]` table (receiver options, targets, interferers), as in the conformance library |
//! | `bars.toml` | `[[bars]]`: `scenario`, `metric`, `op`, `limit` (the conformance [`Criterion`] plus the scenario id) |
//! | `registration.toml` | benchmark id and version, registration time, SHA-256 of `bars.toml` and of every scenario file |
//!
//! [`Benchmark::load`] recomputes every hash and refuses to score when one differs, so a
//! bar (or a scenario) changed after registration is detected. The registration file's own
//! SHA-256 is written into every scorecard, and the leaderboard only accepts scorecards
//! of one registration.
//!
//! ## Metrics (per scenario, computed from ELOF against the simulator truth)
//!
//! The same definitions as the conformance library's software-receiver metrics
//! ([`crate::campaign::conformance::evaluate_software`]); ELOF observations are converted
//! to receiver measurements and scored with [`crate::rx::toa_errors`] and
//! [`crate::rx::clock_errors`]. Bare names are the worst over the scenario's targets.
//!
//! | metric | meaning |
//! |---|---|
//! | `tracked_fraction` | targets with ≥ 1 valid observation / targets |
//! | `toa_rms_ns`, `toa_bias_ns`, `toa_std_ns`, `toa_max_abs_ns` | ToA error against truth, matched by (chain, station, group) |
//! | `ecd_rms_us`, `snr_error_db` | ECD and SNR errors |
//! | `cycle_errors` | observations with \|ToA error\| > 2.5 µs (sum) |
//! | `cycle_slips` | observations flagged `cycle_slip` (sum) |
//! | `time_error_ns` | per-station `timing` records against truth (worst \|error\|); absent without `timing` records |
//! | `ldc_symbol_error_rate`, `ldc_symbols` | `ldc` records against the transmitted symbols (targets carrying data) |
//!
//! A bar whose metric the receiver did not report is NOT_EVALUATED, never a pass.
//!
//! ## Scorecard and seal
//!
//! A [`Scorecard`] holds the registration and bars hashes, the SHA-256 of each scenario
//! and of each submitted observation file, metrics, scored bars and verdicts, and a seal:
//! the SHA-256 of its canonical JSON with the seal empty ([`Scorecard::seal`],
//! [`Scorecard::verify_seal`]). The command line signs the scorecard directory with the
//! Ed25519 dossier signature (`kshana-pro dossier`), which adds who issued it.
//!
//! # Evidence label: MODELLED
//!
//! Stimuli and truth are simulated. The bars are **illustrative**: set by Kshana, not taken
//! from a standard. They were set after observing the in-house software receiver on these
//! fixed seeds, so for that receiver they are not a blind pre-registration; for any other
//! receiver they are fixed before its observations are scored. No live captures are in
//! v0.1 (partnered datasets are a later step).

use crate::campaign::conformance::{
    overall, rx_config, score, simulate, targets, ConformanceFile, Criterion, Metrics, Scored,
    SimRun, Verdict,
};
use crate::elof::{self, convert, ElofFile};
use crate::rx::{self, RxInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Benchmark identifier.
pub const BENCHMARK: &str = "eLoran-Bench";
/// Scorecard format tag.
pub const SCORECARD_FORMAT: &str = "eloran-bench-scorecard";
/// Leaderboard format tag.
pub const LEADERBOARD_FORMAT: &str = "eloran-bench-leaderboard";
/// ToA error beyond which an observation counts as a cycle error (µs → ns).
pub const CYCLE_ERROR_NS: f64 = 2500.0;

/// The benchmark directory shipped with the crate.
pub fn bench_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scenarios/bench-eloran")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// A bar: a conformance criterion bound to a scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub scenario: String,
    #[serde(flatten)]
    pub criterion: Criterion,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BarsFile {
    pub benchmark: String,
    pub version: String,
    pub bars: Vec<Bar>,
}

/// One registered scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegisteredScenario {
    pub id: String,
    pub file: String,
    pub sha256: String,
}

/// The registration (pre-registration of bars and scenarios by hash).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Registration {
    pub benchmark: String,
    pub version: String,
    pub registered_utc: String,
    pub bars_file: String,
    pub bars_sha256: String,
    pub scenarios: Vec<RegisteredScenario>,
}

/// A scenario of the benchmark.
#[derive(Clone, Debug)]
pub struct BenchScenario {
    pub id: String,
    pub file: ConformanceFile,
    pub sha256: String,
    pub text: String,
}

impl BenchScenario {
    /// The scenario source without the `[conformance]` table.
    pub fn scenario_text(&self) -> Result<String, String> {
        self.file.scenario_text(None, &BTreeMap::new())
    }
    /// Simulate the stimulus (deterministic: fixed seeds).
    pub fn simulate(&self) -> Result<SimRun, String> {
        simulate(&self.scenario_text()?)
    }
}

/// A loaded, hash-verified benchmark.
#[derive(Clone, Debug)]
pub struct Benchmark {
    pub dir: PathBuf,
    pub registration: Registration,
    pub registration_sha256: String,
    pub bars: BarsFile,
    pub scenarios: Vec<BenchScenario>,
}

impl Benchmark {
    /// Load `dir` and verify the pre-registration: the bars file and every scenario file
    /// must match their registered SHA-256.
    pub fn load(dir: &Path) -> Result<Benchmark, String> {
        let rp = dir.join("registration.toml");
        let rtext = std::fs::read_to_string(&rp).map_err(|e| format!("{}: {e}", rp.display()))?;
        let reg: Registration =
            toml::from_str(&rtext).map_err(|e| format!("registration.toml: {e}"))?;
        if reg.benchmark != BENCHMARK {
            return Err(format!(
                "registration.toml: benchmark {:?} is not {BENCHMARK}",
                reg.benchmark
            ));
        }
        let bp = dir.join(&reg.bars_file);
        let btext = std::fs::read(&bp).map_err(|e| format!("{}: {e}", bp.display()))?;
        let bsha = sha256_hex(&btext);
        if bsha != reg.bars_sha256 {
            return Err(format!(
                "{}: bars do not match their pre-registered SHA-256 ({} registered, {} found): \
                 the bars were changed after registration",
                reg.bars_file, reg.bars_sha256, bsha
            ));
        }
        let bars: BarsFile = toml::from_str(
            std::str::from_utf8(&btext).map_err(|e| format!("{}: {e}", reg.bars_file))?,
        )
        .map_err(|e| format!("{}: {e}", reg.bars_file))?;
        if bars.benchmark != reg.benchmark || bars.version != reg.version {
            return Err("bars file and registration name different benchmarks".into());
        }
        let mut scenarios = Vec::new();
        for s in &reg.scenarios {
            let p = dir.join(&s.file);
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let h = sha256_hex(text.as_bytes());
            if h != s.sha256 {
                return Err(format!(
                    "{}: scenario does not match its pre-registered SHA-256: changed after \
                     registration",
                    s.file
                ));
            }
            let file = ConformanceFile::from_toml(&text).map_err(|e| format!("{}: {e}", s.file))?;
            if file.case.id != s.id {
                return Err(format!(
                    "{}: case id {} is not the registered {}",
                    s.file, file.case.id, s.id
                ));
            }
            scenarios.push(BenchScenario {
                id: s.id.clone(),
                file,
                sha256: h,
                text,
            });
        }
        for b in &bars.bars {
            if !scenarios.iter().any(|s| s.id == b.scenario) {
                return Err(format!("bar on unknown scenario {}", b.scenario));
            }
            if !b.criterion.illustrative {
                return Err(format!(
                    "bar {}:{} must be marked illustrative",
                    b.scenario,
                    b.criterion.key()
                ));
            }
        }
        Ok(Benchmark {
            dir: dir.to_path_buf(),
            registration_sha256: sha256_hex(rtext.as_bytes()),
            registration: reg,
            bars,
            scenarios,
        })
    }

    pub fn scenario(&self, id: &str) -> Option<&BenchScenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }

    /// Bars of one scenario.
    pub fn bars_of(&self, id: &str) -> Vec<Criterion> {
        self.bars
            .bars
            .iter()
            .filter(|b| b.scenario == id)
            .map(|b| b.criterion.clone())
            .collect()
    }
}

/// Write `registration.toml` for the bars and scenario files in `dir` (the benchmark
/// maintainer's step, done once before any scoring).
pub fn register(
    dir: &Path,
    version: &str,
    utc: &str,
    scenarios: &[(&str, &str)],
) -> Result<Registration, String> {
    let bars = std::fs::read(dir.join("bars.toml")).map_err(|e| format!("bars.toml: {e}"))?;
    let mut reg = Registration {
        benchmark: BENCHMARK.into(),
        version: version.into(),
        registered_utc: utc.into(),
        bars_file: "bars.toml".into(),
        bars_sha256: sha256_hex(&bars),
        scenarios: vec![],
    };
    for (id, file) in scenarios {
        let t = std::fs::read(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
        reg.scenarios.push(RegisteredScenario {
            id: id.to_string(),
            file: file.to_string(),
            sha256: sha256_hex(&t),
        });
    }
    let text = toml::to_string(&reg).map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("registration.toml"),
        format!(
            "# eLoran-Bench pre-registration: SHA-256 of the bars and scenarios, written before\n\
             # any receiver is scored. Do not edit by hand.\n{text}"
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(reg)
}

fn worst(v: impl Iterator<Item = f64>) -> f64 {
    v.filter(|x| x.is_finite())
        .fold(f64::NAN, |a, b| if a.is_nan() { b } else { a.max(b) })
}

/// Metrics of a receiver's ELOF observations on one scenario run.
pub fn metrics(sc: &BenchScenario, run: &SimRun, obs: &ElofFile) -> Result<Metrics, String> {
    let want = run.scenario.hash();
    if !obs.header.scenarios.iter().any(|s| s.sha256 == want) {
        return Err(format!(
            "{}: the observations do not reference this scenario (SHA-256 {want})",
            sc.id
        ));
    }
    let chains = run.chains();
    let tg = targets(&sc.file.case, &chains)?;
    let meas = convert::to_measurements(obs, &chains)?;
    let mut m = Metrics::new();
    let n = tg.len() as f64;
    m.insert(
        "tracked_fraction".into(),
        tg.iter()
            .filter(|t| {
                meas.iter()
                    .any(|x| x.chain_index == t.0 && x.station_index == t.1)
            })
            .count() as f64
            / n,
    );
    let errs = rx::toa_errors(&meas, &run.truth);
    let (mut cyc, mut slips) = (0.0, 0.0);
    for (ci, si, key) in &tg {
        let (cn, sn) = (&chains[*ci].name, &chains[*ci].stations[*si].id);
        let es: Vec<&rx::StationErrors> = errs
            .iter()
            .filter(|e| &e.chain == cn && &e.station == sn && e.n > 0)
            .collect();
        if let Some(e) = es.first() {
            m.insert(format!("toa_rms_ns@{key}"), e.toa_rms_ns);
            m.insert(format!("toa_bias_ns@{key}"), e.toa_mean_ns.abs());
            m.insert(format!("toa_std_ns@{key}"), e.toa_std_ns);
            m.insert(format!("toa_max_abs_ns@{key}"), e.toa_max_abs_ns);
            m.insert(format!("ecd_rms_us@{key}"), e.ecd_rms_err_us);
            m.insert(
                format!("snr_error_db@{key}"),
                (e.snr_meas_db - e.snr_truth_db).abs(),
            );
            cyc += es.iter().map(|e| e.cycle_errors as f64).sum::<f64>();
        }
        slips += meas
            .iter()
            .filter(|x| x.chain_index == *ci && x.station_index == *si && x.cycle_slip)
            .count() as f64;
    }
    for name in [
        "toa_rms_ns",
        "toa_bias_ns",
        "toa_std_ns",
        "toa_max_abs_ns",
        "ecd_rms_us",
        "snr_error_db",
    ] {
        let w = worst(
            tg.iter()
                .filter_map(|t| m.get(&format!("{name}@{}", t.2)).copied()),
        );
        m.insert(name.into(), w);
    }
    m.insert("cycle_errors".into(), cyc);
    m.insert("cycle_slips".into(), slips);
    if obs.timing().any(|t| t.station.is_some()) {
        let ts = convert::to_time_solution(obs);
        let ce = rx::clock_errors(&ts, &run.truth);
        let w = worst(
            ce.iter()
                .filter(|c| {
                    tg.iter().any(|t| {
                        chains[t.0].name == c.chain && chains[t.0].stations[t.1].id == c.station
                    })
                })
                .map(|c| c.error_ns.abs()),
        );
        m.insert("time_error_ns".into(), w);
    }
    // data channel: decided symbols against the transmitted ones
    let mods: Vec<Option<crate::ldc::modulator::LdcModulator>> = run
        .scenario
        .chains
        .iter()
        .map(|c| {
            c.ldc.as_ref().and_then(|l| {
                crate::ldc::build_modulator(l, Some(&c.chain))
                    .ok()
                    .flatten()
            })
        })
        .collect();
    if mods.iter().any(Option::is_some) {
        let (mut total, mut wrong) = (0usize, 0usize);
        for l in obs.ldc() {
            let Some(t) = tg
                .iter()
                .find(|t| chains[t.0].name == l.chain && chains[t.0].stations[t.1].id == l.station)
            else {
                continue;
            };
            let Some(md) = &mods[t.0] else { continue };
            let Some(truth) = md.symbol(t.1, chains[t.0].stations[t.1].role, l.group) else {
                continue;
            };
            total += 1;
            wrong += (l.missing || l.symbol != truth) as usize;
        }
        m.insert("ldc_symbols".into(), total as f64);
        if total > 0 {
            m.insert("ldc_symbol_error_rate".into(), wrong as f64 / total as f64);
        }
    }
    Ok(m)
}

/// The in-house software receiver's observations on a scenario run, as ELOF (with the
/// time solution at the scenario's receiver position for static receivers).
pub fn software_receiver_observations(
    sc: &BenchScenario,
    run: &SimRun,
) -> Result<ElofFile, String> {
    let chains = run.chains();
    let cfg = rx_config(&sc.file.case, &run.scenario)?;
    let input = RxInput::new(
        run.channels.clone(),
        run.scenario.scene.params.fs_hz,
        run.start,
    );
    let (_, out) = rx::process(&input, &chains, cfg)?;
    let ts = match &run.scenario.receiver {
        crate::traj::Trajectory::Static { .. } => Some(rx::time_solution(
            &out.measurements,
            &chains,
            run.start,
            run.scenario.receiver.state(0.0).pos,
            &rx::Corrections::default(),
        )),
        _ => None,
    };
    convert::from_rx_output(&run.scenario, &sc.id, &out, ts.as_ref(), None)
}

/// Result of one scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub id: String,
    pub scenario_sha256: String,
    /// SHA-256 of the submitted observations (canonical ELOF JSON lines); empty when none.
    pub observations_sha256: String,
    pub metrics: Metrics,
    pub scored: Vec<Scored>,
    pub verdict: Verdict,
    #[serde(default)]
    pub error: Option<String>,
}

/// A scorecard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub format: String,
    pub benchmark: String,
    pub benchmark_version: String,
    pub registration_sha256: String,
    pub bars_sha256: String,
    pub receiver: String,
    /// Receiver make/model from the ELOF headers (first file).
    pub receiver_model: String,
    pub results: Vec<ScenarioResult>,
    /// Bars passed / failed / not evaluated over all scenarios.
    pub pass: usize,
    pub fail: usize,
    pub not_evaluated: usize,
    /// Scenarios whose verdict is PASS.
    pub scenarios_passed: usize,
    pub evidence_label: String,
    /// SHA-256 of the canonical JSON with this field empty.
    pub seal_sha256: String,
}

impl Scorecard {
    fn digest(&self) -> String {
        let mut c = self.clone();
        c.seal_sha256 = String::new();
        sha256_hex(&serde_json::to_vec(&c).expect("serialisable"))
    }
    /// Set the seal.
    pub fn seal(&mut self) {
        self.seal_sha256 = self.digest();
    }
    /// The seal matches the content.
    pub fn verify_seal(&self) -> bool {
        !self.seal_sha256.is_empty() && self.seal_sha256 == self.digest()
    }
    /// Fraction of bars passed (NOT_EVALUATED counts as not passed).
    pub fn score(&self) -> f64 {
        let n = self.pass + self.fail + self.not_evaluated;
        if n == 0 {
            0.0
        } else {
            self.pass as f64 / n as f64
        }
    }
}

/// Score a receiver: `obs` maps scenario id → its ELOF observations (missing scenarios
/// score NOT_EVALUATED on every bar). Every scenario is simulated again from its
/// registered file, so truth never comes from the submission.
pub fn score_receiver(
    b: &Benchmark,
    receiver: &str,
    obs: &BTreeMap<String, ElofFile>,
) -> Result<Scorecard, String> {
    for k in obs.keys() {
        if b.scenario(k).is_none() {
            return Err(format!("observations for unknown scenario {k}"));
        }
    }
    let mut results = Vec::new();
    for sc in &b.scenarios {
        let bars = b.bars_of(&sc.id);
        let (metrics, err, osha) = match obs.get(&sc.id) {
            None => (
                Metrics::new(),
                Some("no observations submitted".to_string()),
                String::new(),
            ),
            Some(f) => {
                let osha = sha256_hex(f.to_jsonl()?.as_bytes());
                let rep = elof::validate(f);
                if !rep.ok() {
                    (
                        Metrics::new(),
                        Some(format!("invalid ELOF: {}", rep.summary())),
                        osha,
                    )
                } else {
                    match sc.simulate().and_then(|run| metrics(sc, &run, f)) {
                        Ok(m) => (m, None, osha),
                        Err(e) => (Metrics::new(), Some(e), osha),
                    }
                }
            }
        };
        let scored = score(&bars, "", &metrics);
        results.push(ScenarioResult {
            id: sc.id.clone(),
            scenario_sha256: sc.sha256.clone(),
            observations_sha256: osha,
            verdict: overall(&scored),
            metrics,
            scored,
            error: err,
        });
    }
    let count = |v: Verdict| {
        results
            .iter()
            .flat_map(|r| &r.scored)
            .filter(|s| s.verdict == v)
            .count()
    };
    let model = obs
        .values()
        .next()
        .map(|f| {
            format!("{} {}", f.header.receiver.make, f.header.receiver.model)
                .trim()
                .to_string()
        })
        .unwrap_or_default();
    let mut card = Scorecard {
        format: SCORECARD_FORMAT.into(),
        benchmark: b.registration.benchmark.clone(),
        benchmark_version: b.registration.version.clone(),
        registration_sha256: b.registration_sha256.clone(),
        bars_sha256: b.registration.bars_sha256.clone(),
        receiver: receiver.into(),
        receiver_model: model,
        pass: count(Verdict::Pass),
        fail: count(Verdict::Fail),
        not_evaluated: count(Verdict::NotEvaluated),
        scenarios_passed: results
            .iter()
            .filter(|r| r.verdict == Verdict::Pass)
            .count(),
        results,
        evidence_label:
            "MODELLED: simulated stimuli with exact truth; bars ILLUSTRATIVE (not from \
                         a standard)"
                .into(),
        seal_sha256: String::new(),
    };
    card.seal();
    Ok(card)
}

/// One leaderboard entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: usize,
    pub receiver: String,
    pub receiver_model: String,
    pub submitted_utc: String,
    pub scorecard_seal_sha256: String,
    pub score: f64,
    pub pass: usize,
    pub fail: usize,
    pub not_evaluated: usize,
    pub scenarios_passed: usize,
    /// Worst ToA RMS over scenarios (ns), the tie-breaker.
    pub worst_toa_rms_ns: Option<f64>,
}

/// The leaderboard file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leaderboard {
    pub format: String,
    pub benchmark: String,
    pub benchmark_version: String,
    pub registration_sha256: String,
    pub bars_sha256: String,
    pub entries: Vec<LeaderboardEntry>,
}

impl Leaderboard {
    pub fn new(b: &Benchmark) -> Leaderboard {
        Leaderboard {
            format: LEADERBOARD_FORMAT.into(),
            benchmark: b.registration.benchmark.clone(),
            benchmark_version: b.registration.version.clone(),
            registration_sha256: b.registration_sha256.clone(),
            bars_sha256: b.registration.bars_sha256.clone(),
            entries: vec![],
        }
    }

    pub fn from_json(text: &str) -> Result<Leaderboard, String> {
        let l: Leaderboard = serde_json::from_str(text).map_err(|e| format!("leaderboard: {e}"))?;
        if l.format != LEADERBOARD_FORMAT {
            return Err(format!("leaderboard: format {:?}", l.format));
        }
        Ok(l)
    }

    /// Add (or replace) a receiver's entry. The scorecard's seal must hold and it must be
    /// scored against this leaderboard's registration and bars.
    pub fn add(&mut self, card: &Scorecard, utc: &str) -> Result<usize, String> {
        if !card.verify_seal() {
            return Err("scorecard seal does not match its content (modified)".into());
        }
        if card.registration_sha256 != self.registration_sha256
            || card.bars_sha256 != self.bars_sha256
        {
            return Err("scorecard was scored against another registration or bar set".into());
        }
        let worst_toa = worst(
            card.results
                .iter()
                .filter_map(|r| r.metrics.get("toa_rms_ns").copied()),
        );
        self.entries.retain(|e| e.receiver != card.receiver);
        self.entries.push(LeaderboardEntry {
            rank: 0,
            receiver: card.receiver.clone(),
            receiver_model: card.receiver_model.clone(),
            submitted_utc: utc.into(),
            scorecard_seal_sha256: card.seal_sha256.clone(),
            score: card.score(),
            pass: card.pass,
            fail: card.fail,
            not_evaluated: card.not_evaluated,
            scenarios_passed: card.scenarios_passed,
            worst_toa_rms_ns: worst_toa.is_finite().then_some(worst_toa),
        });
        self.entries.sort_by(|a, b| {
            b.score.total_cmp(&a.score).then(
                a.worst_toa_rms_ns
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&b.worst_toa_rms_ns.unwrap_or(f64::INFINITY)),
            )
        });
        for (i, e) in self.entries.iter_mut().enumerate() {
            e.rank = i + 1;
        }
        Ok(self
            .entries
            .iter()
            .find(|e| e.receiver == card.receiver)
            .map(|e| e.rank)
            .unwrap_or(0))
    }
}

#[cfg(test)]
mod tests;
