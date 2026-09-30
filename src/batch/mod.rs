// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Scale & performance — parallel batch / Monte-Carlo runner (PR20)
//!
//! A Pro enhancement that runs many open-engine scenarios **concurrently** and
//! aggregates a figure of merit — the "scale" upsell beyond a single sequential
//! workstation run. Built entirely on the open [`kshana::api::run_scenario`] (a pure
//! function), parallelised with the standard library's scoped threads.
//!
//! **Determinism guarantee:** each scenario is deterministic in `(scenario, seed,
//! version)`, so the aggregate is identical regardless of thread scheduling — the
//! parallel result equals the sequential result. That invariant is battle-tested.

pub mod uq;

use kshana::api::{run_scenario, KshanaError};

use crate::util::{dot_path_number, inject_seed, percentile};

/// One unit of work: a label and a full scenario TOML.
#[derive(Debug, Clone)]
pub struct BatchJob {
    pub label: String,
    pub scenario_toml: String,
}

/// The result of one job.
#[derive(Debug, Clone)]
pub struct BatchOutcome {
    pub label: String,
    pub scenario_hash: Option<String>,
    pub metric_value: Option<f64>,
    pub error: Option<String>,
}

/// Run `jobs` concurrently across up to `max_threads` OS threads (clamped to a sane
/// range and to the job count), extracting `metric` (a dot-path) from each result.
/// Output order matches input order, so results are reproducible.
pub fn run_batch(jobs: &[BatchJob], metric: &str, max_threads: usize) -> Vec<BatchOutcome> {
    if jobs.is_empty() {
        return Vec::new();
    }
    let n_threads = max_threads.clamp(1, jobs.len());
    // Contiguous chunking keeps the mapping from input index to output simple and
    // the merge order deterministic.
    let chunk_size = jobs.len().div_ceil(n_threads);

    let chunks: Vec<(usize, &[BatchJob])> = jobs.chunks(chunk_size).enumerate().collect();
    let mut by_chunk: Vec<Option<Vec<BatchOutcome>>> = vec![None; chunks.len()];

    std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|&(ci, chunk)| {
                scope.spawn(move || {
                    (
                        ci,
                        chunk.iter().map(|j| run_one(j, metric)).collect::<Vec<_>>(),
                    )
                })
            })
            .collect();
        for h in handles {
            let (ci, out) = h.join().expect("batch worker panicked");
            by_chunk[ci] = Some(out);
        }
    });

    by_chunk.into_iter().flatten().flatten().collect()
}

fn run_one(job: &BatchJob, metric: &str) -> BatchOutcome {
    let fail = |error: String| BatchOutcome {
        label: job.label.clone(),
        scenario_hash: None,
        metric_value: None,
        error: Some(error),
    };
    match run_scenario(&job.scenario_toml) {
        Ok(out) => match serde_json::from_str::<serde_json::Value>(&out.json) {
            Ok(json) => {
                let scenario_hash = json
                    .get("scenario_hash")
                    .and_then(|h| h.as_str())
                    .map(String::from);
                let metric_value = dot_path_number(&json, metric);
                BatchOutcome {
                    label: job.label.clone(),
                    scenario_hash,
                    metric_value,
                    error: if metric_value.is_none() {
                        Some(format!("metric '{metric}' not found or non-numeric"))
                    } else {
                        None
                    },
                }
            }
            // Distinct from a missing metric: the engine returned unparseable JSON.
            Err(e) => fail(format!("result JSON unparseable: {e}")),
        },
        Err(e) => fail(format!("run failed: {e}")),
    }
}

/// A Monte-Carlo request: one base scenario fanned across many seeds, aggregating a
/// figure of merit.
#[derive(Debug, Clone)]
pub struct MonteCarloSpec {
    pub base_scenario: String,
    pub seeds: Vec<u64>,
    pub metric: String,
}

/// One Monte-Carlo sample.
#[derive(Debug, Clone)]
pub struct McSample {
    pub seed: u64,
    pub value: Option<f64>,
    pub scenario_hash: Option<String>,
    pub error: Option<String>,
}

/// Aggregate Monte-Carlo statistics over the metric.
#[derive(Debug, Clone)]
pub struct McResult {
    pub n_total: usize,
    pub n_ok: usize,
    pub mean: f64,
    pub std: f64,
    pub min: f64,
    pub max: f64,
    pub p05: f64,
    pub p95: f64,
    pub samples: Vec<McSample>,
}

/// Run a Monte-Carlo campaign: inject each seed into the base scenario, run all in
/// parallel, and aggregate the metric. Deterministic in the set of seeds (the result
/// does not depend on thread scheduling).
pub fn run_monte_carlo(spec: &MonteCarloSpec, max_threads: usize) -> Result<McResult, KshanaError> {
    if spec.seeds.len() < 2 {
        return Err(KshanaError::InvalidInput(
            "Monte-Carlo needs >= 2 seeds".into(),
        ));
    }
    if spec.metric.trim().is_empty() {
        return Err(KshanaError::InvalidInput(
            "Monte-Carlo needs a non-empty `metric` dot-path".into(),
        ));
    }

    // Pre-inject seeds (fail fast on a malformed base scenario).
    let jobs: Vec<BatchJob> = spec
        .seeds
        .iter()
        .map(|&seed| {
            inject_seed(&spec.base_scenario, seed).map(|toml| BatchJob {
                label: format!("seed={seed}"),
                scenario_toml: toml,
            })
        })
        .collect::<Result<_, _>>()?;

    let outcomes = run_batch(&jobs, &spec.metric, max_threads);

    let samples: Vec<McSample> = spec
        .seeds
        .iter()
        .zip(outcomes.iter())
        .map(|(&seed, o)| McSample {
            seed,
            value: o.metric_value,
            scenario_hash: o.scenario_hash.clone(),
            error: o.error.clone(),
        })
        .collect();

    let values: Vec<f64> = samples.iter().filter_map(|s| s.value).collect();
    if values.is_empty() {
        return Err(KshanaError::NonConvergence(format!(
            "no Monte-Carlo run produced metric '{}'",
            spec.metric
        )));
    }

    let n_ok = values.len();
    let mean = values.iter().sum::<f64>() / n_ok as f64;
    // Sample standard deviation (n-1); 0 for a single sample.
    let std = if n_ok > 1 {
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n_ok as f64 - 1.0)).sqrt()
    } else {
        0.0
    };
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    Ok(McResult {
        n_total: spec.seeds.len(),
        n_ok,
        mean,
        std,
        min,
        max,
        p05: percentile(&values, 0.05).expect("values non-empty (guarded above)"),
        p95: percentile(&values, 0.95).expect("values non-empty (guarded above)"),
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal real open scenario; n_samples is a clean deterministic metric and
    // varies with duration but NOT with seed (ephemeris is noise-free), which lets
    // us check determinism precisely.
    fn ephem(duration_s: f64) -> String {
        format!(
            "kind = \"ephemeris\"\n\
             tle = \"\"\"\n\
             1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
             2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
             \"\"\"\n\
             step_s = 30.0\n\
             duration_s = {duration_s}\n"
        )
    }

    // A clock scenario IS seed-sensitive (stochastic), so Monte-Carlo over seeds
    // produces a real spread — the right shape to test aggregation.
    fn clock() -> String {
        "kind = \"clock\"\n\
         seed = 1\n\
         threshold_ns = 20.0\n\
         [time]\n\
         step_s = 10.0\n\
         duration_s = 3600.0\n\
         [gnss]\n\
         windows = [\n\
         { t0 = 0.0, t1 = 600.0, state = \"nominal\" },\n\
         { t0 = 600.0, t1 = 3600.0, state = \"denied\" },\n\
         ]\n\
         [clock_quantum]\n\
         id = \"q\"\n\
         provenance = \"mc test\"\n\
         y0 = 0.0\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n\
         [clock_classical]\n\
         id = \"c\"\n\
         provenance = \"mc test\"\n\
         y0 = 0.0\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n"
            .to_string()
    }

    #[test]
    fn batch_preserves_input_order() {
        let jobs: Vec<BatchJob> = [60.0, 120.0, 180.0, 240.0]
            .iter()
            .enumerate()
            .map(|(i, &d)| BatchJob {
                label: format!("job{i}"),
                scenario_toml: ephem(d),
            })
            .collect();
        let out = run_batch(&jobs, "n_samples", 4);
        let labels: Vec<&str> = out.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, vec!["job0", "job1", "job2", "job3"]);
        // n_samples must increase with duration.
        let vals: Vec<f64> = out.iter().map(|o| o.metric_value.unwrap()).collect();
        assert!(
            vals.windows(2).all(|w| w[1] >= w[0]),
            "monotonic in duration: {vals:?}"
        );
    }

    #[test]
    fn parallel_equals_sequential() {
        // THE core correctness invariant: thread count must not change results.
        let spec = MonteCarloSpec {
            base_scenario: clock(),
            seeds: (1..=12).collect(),
            metric: "quantum.fom.timing_p95_ns".into(),
        };
        let seq = run_monte_carlo(&spec, 1).unwrap();
        let par = run_monte_carlo(&spec, 8).unwrap();
        assert_eq!(seq.n_ok, par.n_ok);
        assert_eq!(
            seq.mean.to_bits(),
            par.mean.to_bits(),
            "mean must be bit-identical"
        );
        assert_eq!(
            seq.std.to_bits(),
            par.std.to_bits(),
            "std must be bit-identical"
        );
        assert_eq!(seq.p95.to_bits(), par.p95.to_bits());
        // Per-seed samples must match in order and value.
        for (a, b) in seq.samples.iter().zip(par.samples.iter()) {
            assert_eq!(a.seed, b.seed);
            assert_eq!(a.value.map(f64::to_bits), b.value.map(f64::to_bits));
        }
    }

    #[test]
    fn monte_carlo_aggregates_a_real_spread() {
        let spec = MonteCarloSpec {
            base_scenario: clock(),
            seeds: (1..=20).collect(),
            metric: "quantum.fom.timing_p95_ns".into(),
        };
        let r = run_monte_carlo(&spec, 4).unwrap();
        assert_eq!(r.n_total, 20);
        assert_eq!(r.n_ok, 20);
        assert!(r.min <= r.mean && r.mean <= r.max);
        assert!(r.p05 <= r.p95);
        assert!(r.std >= 0.0);
        // A stochastic clock over 20 seeds should show some spread.
        assert!(
            r.max > r.min,
            "expected a non-degenerate spread across seeds"
        );
    }

    #[test]
    fn monte_carlo_rejects_too_few_seeds() {
        let spec = MonteCarloSpec {
            base_scenario: clock(),
            seeds: vec![1],
            metric: "quantum.fom.timing_p95_ns".into(),
        };
        assert!(matches!(
            run_monte_carlo(&spec, 4),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn monte_carlo_errors_when_metric_never_found() {
        let spec = MonteCarloSpec {
            base_scenario: clock(),
            seeds: (1..=4).collect(),
            metric: "no.such.metric".into(),
        };
        assert!(matches!(
            run_monte_carlo(&spec, 4),
            Err(KshanaError::NonConvergence(_))
        ));
    }

    #[test]
    fn batch_captures_failures_without_aborting() {
        let jobs = vec![
            BatchJob {
                label: "ok".into(),
                scenario_toml: ephem(120.0),
            },
            BatchJob {
                label: "bad".into(),
                scenario_toml: "kind = \"ephemeris\"\nstep_s = 30.0\n".into(),
            },
        ];
        let out = run_batch(&jobs, "n_samples", 2);
        assert!(out[0].metric_value.is_some());
        assert!(out[1].metric_value.is_none() && out[1].error.is_some());
    }
}
