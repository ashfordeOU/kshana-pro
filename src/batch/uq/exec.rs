// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Parallel evaluation of a design, merged in realisation order.
//!
//! Worker threads (standard-library scoped threads) take the next pending realisation
//! from a shared counter and send the finished record back. The calling thread hands
//! records to the sink **strictly in realisation order**, holding a record back until
//! every earlier one has arrived. The checkpoint file and every statistic therefore
//! see the same sequence whatever the thread count, which is what makes the result
//! byte-identical for one thread and for many.
//!
//! A realisation that fails is a record with an `error`, never a missing record. A
//! panic inside an evaluation is caught and recorded the same way.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;

/// What one evaluation returns.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Evaluation {
    /// One value per scalar output; `None` where the run gave no number there.
    pub scalars: Vec<Option<f64>>,
    /// One series per band output; `None` where the run gave no usable series.
    pub series: Vec<Option<Vec<f64>>>,
    /// SHA-256 of the run's result document.
    pub result_sha256: Option<String>,
    /// Why the run failed, when it did.
    pub error: Option<String>,
}

impl Evaluation {
    /// A failed evaluation.
    pub fn failed(error: impl Into<String>) -> Evaluation {
        Evaluation {
            error: Some(error.into()),
            ..Evaluation::default()
        }
    }
}

/// Something that turns an input vector (and, when the study varies it, an engine
/// seed) into output values. The open engine is one; a closure is another.
pub trait Evaluator: Sync {
    /// Evaluate one realisation.
    fn evaluate(&self, inputs: &[f64], seed: Option<u64>) -> Evaluation;
}

/// An [`Evaluator`] over a plain function of the inputs returning the scalar outputs.
pub struct FnEvaluator<F>(pub F);

impl<F> Evaluator for FnEvaluator<F>
where
    F: Fn(&[f64]) -> Result<Vec<f64>, String> + Sync,
{
    fn evaluate(&self, inputs: &[f64], _seed: Option<u64>) -> Evaluation {
        match (self.0)(inputs) {
            Ok(values) => Evaluation {
                scalars: values
                    .into_iter()
                    .map(|v| v.is_finite().then_some(v))
                    .collect(),
                ..Evaluation::default()
            },
            Err(e) => Evaluation::failed(e),
        }
    }
}

/// One realisation to evaluate.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub index: usize,
    pub inputs: Vec<f64>,
    pub seed: Option<u64>,
}

/// One finished realisation: the job and what it returned.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRecord {
    pub index: usize,
    pub inputs: Vec<f64>,
    pub seed: Option<u64>,
    pub scalars: Vec<Option<f64>>,
    pub series: Vec<Option<Vec<f64>>>,
    pub result_sha256: Option<String>,
    pub error: Option<String>,
}

impl RunRecord {
    fn new(job: &Job, e: Evaluation) -> RunRecord {
        RunRecord {
            index: job.index,
            inputs: job.inputs.clone(),
            seed: job.seed,
            scalars: e.scalars,
            series: e.series,
            result_sha256: e.result_sha256,
            error: e.error,
        }
    }
}

fn evaluate_caught(evaluator: &dyn Evaluator, job: &Job) -> RunRecord {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        evaluator.evaluate(&job.inputs, job.seed)
    }));
    let evaluation = match outcome {
        Ok(e) => e,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "no message".to_string());
            Evaluation::failed(format!("the run panicked: {msg}"))
        }
    };
    RunRecord::new(job, evaluation)
}

/// Evaluate `jobs` on up to `threads` threads and pass each record to `sink` in the
/// order of `jobs`. A sink error stops the run and is returned.
pub fn run_ordered(
    jobs: &[Job],
    evaluator: &dyn Evaluator,
    threads: usize,
    sink: &mut dyn FnMut(RunRecord) -> Result<(), String>,
) -> Result<(), String> {
    if jobs.is_empty() {
        return Ok(());
    }
    let n_threads = threads.clamp(1, jobs.len());
    if n_threads == 1 {
        for job in jobs {
            sink(evaluate_caught(evaluator, job))?;
        }
        return Ok(());
    }
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let (tx, rx) = mpsc::channel::<(usize, RunRecord)>();
    let mut result = Ok(());
    std::thread::scope(|scope| {
        for _ in 0..n_threads {
            let tx = tx.clone();
            let (next, stop) = (&next, &stop);
            scope.spawn(move || loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let at = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(at) else { break };
                if tx.send((at, evaluate_caught(evaluator, job))).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        let mut held: BTreeMap<usize, RunRecord> = BTreeMap::new();
        let mut want = 0usize;
        for (at, record) in rx {
            held.insert(at, record);
            while let Some(record) = held.remove(&want) {
                want += 1;
                if result.is_ok() {
                    if let Err(e) = sink(record) {
                        result = Err(e);
                        stop.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jobs(n: usize) -> Vec<Job> {
        (0..n)
            .map(|i| Job {
                index: 100 + i,
                inputs: vec![i as f64],
                seed: None,
            })
            .collect()
    }

    /// Slow for early jobs and fast for late ones, so threads finish out of order.
    fn uneven() -> FnEvaluator<impl Fn(&[f64]) -> Result<Vec<f64>, String> + Sync> {
        FnEvaluator(|x: &[f64]| {
            let i = x[0] as u64;
            std::thread::sleep(std::time::Duration::from_millis(if i < 4 { 12 } else { 0 }));
            if i == 7 {
                return Err("seven fails".to_string());
            }
            if i == 9 {
                panic!("nine panics");
            }
            Ok(vec![x[0] * x[0], f64::NAN])
        })
    }

    fn collect(threads: usize) -> Vec<RunRecord> {
        let mut out = Vec::new();
        run_ordered(&jobs(24), &uneven(), threads, &mut |r| {
            out.push(r);
            Ok(())
        })
        .expect("runs");
        out
    }

    #[test]
    fn records_arrive_in_job_order_for_any_thread_count() {
        let one = collect(1);
        assert_eq!(one.len(), 24);
        assert!(one.iter().enumerate().all(|(i, r)| r.index == 100 + i));
        for threads in [2, 4, 9, 64] {
            assert_eq!(collect(threads), one, "threads = {threads}");
        }
    }

    #[test]
    fn a_failure_or_a_panic_is_a_record_not_a_gap() {
        let out = collect(4);
        assert_eq!(out[7].error.as_deref(), Some("seven fails"));
        assert!(out[7].scalars.is_empty());
        let panic = out[9].error.as_deref().expect("recorded");
        assert!(panic.contains("panicked") && panic.contains("nine panics"));
        // A value that is not a number is recorded as missing.
        assert_eq!(out[3].scalars, vec![Some(9.0), None]);
    }

    #[test]
    fn a_sink_error_stops_the_run_and_is_returned() {
        let mut seen = 0usize;
        let err = run_ordered(&jobs(24), &uneven(), 4, &mut |_| {
            seen += 1;
            if seen == 5 {
                Err("disk full".to_string())
            } else {
                Ok(())
            }
        })
        .expect_err("the sink failed");
        assert_eq!(err, "disk full");
        assert_eq!(seen, 5);
    }
}
