// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-batch` pack — exposes the PR20 parallel Monte-Carlo runner
//! ([`crate::batch`]) as an out-of-tree [`ExternalPack`]. It fans one base scenario
//! across many seeds, runs them concurrently on the embedded open engine, and
//! aggregates a figure of merit (mean / std / min / max / p05 / p95).

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::batch::{run_monte_carlo, MonteCarloSpec};

/// The honesty label every `pro-batch` result carries, in the open engine's `label` form.
pub const BATCH_LABEL: &str = "MODELLED - statistics of one open-engine scenario run over a range of engine seeds: every sample is a simulation run of the open engine, never a measurement, and the statistics are only as good as the base scenario's own model.";

fn default_seed_start() -> u64 {
    1
}
fn default_max_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// A `pro-batch` scenario: a base scenario + a seed range + the metric to aggregate.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BatchScenario {
    pub title: String,
    /// The base scenario, inline as a TOML string (its `seed` is overwritten per run).
    pub base_scenario: String,
    /// Dot-path into each run's result JSON, e.g. `quantum.fom.timing_p95_ns`.
    pub metric: String,
    /// Number of Monte-Carlo seeds to run (>= 2).
    pub seed_count: u64,
    #[serde(default = "default_seed_start")]
    pub seed_start: u64,
    #[serde(default = "default_max_threads")]
    pub max_threads: usize,
}

impl BatchScenario {
    pub const KIND: &'static str = "pro-batch";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro scale enhancement (PR20): run one base scenario across many seeds in parallel on the open engine and aggregate a figure of merit (Monte-Carlo mean/std/min/max/p05/p95). Parallel result equals sequential by construction.",
            required_fields: &["title", "base_scenario", "metric", "seed_count"],
            optional_fields: &["seed_start", "max_threads"],
        }
    }
}

impl Scenario for BatchScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let end = self
            .seed_start
            .checked_add(self.seed_count)
            .ok_or_else(|| {
                KshanaError::InvalidInput(format!(
                    "seed_start ({}) + seed_count ({}) overflows u64",
                    self.seed_start, self.seed_count
                ))
            })?;
        let seeds: Vec<u64> = (self.seed_start..end).collect();
        let spec = MonteCarloSpec {
            base_scenario: self.base_scenario.clone(),
            seeds,
            metric: self.metric.clone(),
        };
        let r = run_monte_carlo(&spec, self.max_threads)?;

        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "title": self.title,
            "label": BATCH_LABEL,
            "metric": self.metric,
            "threads": self.max_threads,
            "stats": {
                "n_total": r.n_total,
                "n_ok": r.n_ok,
                "mean": r.mean,
                "std": r.std,
                "min": r.min,
                "max": r.max,
                "p05": r.p05,
                "p95": r.p95,
            },
            "samples": r.samples.iter().map(|s| serde_json::json!({
                "seed": s.seed,
                "value": s.value,
                "scenario_hash": s.scenario_hash,
                "error": s.error,
            })).collect::<Vec<_>>(),
        })
        .to_string();

        let summary = format!(
            "Kshana Pro batch | {} | metric '{}' over {} seeds ({} ok, {} threads) | mean {:.4} std {:.4} [p05 {:.4} .. p95 {:.4}]",
            self.title, self.metric, r.n_total, r.n_ok, self.max_threads, r.mean, r.std, r.p05, r.p95,
        );

        let svg = histogram_svg(&self.title, &self.metric, &r);
        Ok(RunOutput {
            json,
            svg,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for BatchScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        // Drive the open registry seam: add THIS pack's id→factory binding so the
        // superset `pro_registry()` can build it by id. Mirrors `kind_name`/`meta`.
        reg.register(crate::registry::pro_factory::<Self>(Self::KIND));
    }
}

/// A small histogram of the metric across seeds, with mean / p05 / p95 markers.
fn histogram_svg(title: &str, metric: &str, r: &crate::batch::McResult) -> String {
    let (w, h, ml, mr, mt, mb) = (560.0_f64, 320.0_f64, 50.0_f64, 18.0_f64, 56.0_f64, 44.0_f64);
    let values: Vec<f64> = r.samples.iter().filter_map(|s| s.value).collect();
    let bins = 12usize;
    let span = (r.max - r.min).max(f64::MIN_POSITIVE);
    let mut counts = vec![0usize; bins];
    for &v in &values {
        let mut b = (((v - r.min) / span) * bins as f64).floor() as isize;
        b = b.clamp(0, bins as isize - 1);
        counts[b as usize] += 1;
    }
    let cmax = counts.iter().cloned().max().unwrap_or(1).max(1) as f64;
    let plot_w = w - ml - mr;
    let plot_h = h - mt - mb;
    let bw = plot_w / bins as f64;

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"system-ui,sans-serif\" font-size=\"11\">"
    );
    s += "<rect width=\"100%\" height=\"100%\" fill=\"#0c0b08\"/>";
    s += &format!("<text x=\"{ml}\" y=\"24\" fill=\"#c084fc\" font-size=\"14\" font-weight=\"bold\">Kshana Pro batch — Monte-Carlo</text>");
    s += &format!(
        "<text x=\"{ml}\" y=\"40\" fill=\"#8c8273\">{} · {} ({} seeds)</text>",
        esc(title),
        esc(metric),
        r.n_ok
    );
    let base = mt + plot_h;
    for (i, &c) in counts.iter().enumerate() {
        let bh = (c as f64 / cmax) * plot_h;
        let x = ml + i as f64 * bw;
        s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#6f6450\"/>",
            x + 1.0,
            base - bh,
            (bw - 2.0).max(1.0),
            bh
        );
    }
    // Mean / p05 / p95 markers (mapped along x).
    let xof = |v: f64| ml + ((v - r.min) / span).clamp(0.0, 1.0) * plot_w;
    for (val, col, lab) in [
        (r.p05, "#60a5fa", "p05"),
        (r.mean, "#2dd4bf", "mean"),
        (r.p95, "#eab308", "p95"),
    ] {
        let x = xof(val);
        s += &format!("<line x1=\"{x:.1}\" y1=\"{mt}\" x2=\"{x:.1}\" y2=\"{base}\" stroke=\"{col}\" stroke-dasharray=\"3 3\"/>");
        s += &format!("<text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{col}\" font-size=\"10\">{lab}</text>", mt - 4.0);
    }
    s += &format!(
        "<line x1=\"{ml}\" y1=\"{base}\" x2=\"{}\" y2=\"{base}\" stroke=\"#342c21\"/>",
        w - mr
    );
    s += &format!(
        "<text x=\"{ml}\" y=\"{}\" fill=\"#8c8273\">{:.4}</text>",
        h - 14.0,
        r.min
    );
    s += &format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"#8c8273\">{:.4}</text>",
        w - mr,
        h - 14.0,
        r.max
    );
    s += "</svg>";
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock_base() -> String {
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
         provenance = \"batch test\"\n\
         y0 = 0.0\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n\
         [clock_classical]\n\
         id = \"c\"\n\
         provenance = \"batch test\"\n\
         y0 = 0.0\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n"
            .to_string()
    }

    #[test]
    fn batch_pack_runs_monte_carlo_and_aggregates() {
        let pack = BatchScenario {
            title: "clock timing-error MC".into(),
            base_scenario: clock_base(),
            metric: "quantum.fom.timing_p95_ns".into(),
            seed_count: 16,
            seed_start: 1,
            max_threads: 4,
        };
        let out = pack.run().expect("batch runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["stats"]["n_ok"], 16);
        assert!(v["stats"]["mean"].as_f64().unwrap() >= 0.0);
        assert!(out.svg.contains("<svg"));
        assert!(out.summary.contains("mean"));
    }

    #[test]
    fn batch_pack_rejects_seed_range_overflow() {
        // seed_start + seed_count overflowing u64 must error, not silently under-run.
        let pack = BatchScenario {
            title: "overflow".into(),
            base_scenario: clock_base(),
            metric: "quantum.fom.timing_p95_ns".into(),
            seed_count: 10,
            seed_start: u64::MAX - 3,
            max_threads: 2,
        };
        assert!(matches!(pack.run(), Err(KshanaError::InvalidInput(_))));
    }

    #[test]
    fn batch_pack_external_contract() {
        let pack = BatchScenario {
            title: "t".into(),
            base_scenario: clock_base(),
            metric: "quantum.fom.timing_p95_ns".into(),
            seed_count: 2,
            seed_start: 1,
            max_threads: 1,
        };
        assert_eq!(pack.kind_name(), "pro-batch");
        assert_eq!(pack.meta().name, "pro-batch");
    }
}
