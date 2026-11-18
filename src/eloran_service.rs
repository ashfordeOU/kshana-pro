// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The eLoran/GNSS simulator as a service
//!
//! The shared logic behind the service's `/eloran/...` routes ([`crate::serve`]) and the
//! Desktop Pro "eLoran" page, written against the public API of the `kshana-eloran` crate in
//! the same way as `kshana-pro eloran` (`src/cli/eloran_cmd.rs`), so the command line can call
//! it too:
//!
//! * [`validate`]: parse and build a scenario, its SHA-256 (no licence);
//! * [`run_to_files`]: run a scenario to `<base>.sigmf-data/-meta`, `<base>.truth.csv` and
//!   `<base>.run.json`, returning the run record (feature `pro-eloran`, checked by the caller);
//! * [`receive`]: the software receiver on a recording, with the truth comparison when the
//!   truth is given (feature `pro-eloran`);
//! * [`gri_list`] / [`gri_show`]: the preloaded GRI/chain database (no licence);
//! * [`truth_preview`]: the first rows of a truth table, for a page.
//!
//! Hardware output (`eloran stream`, feature `pro-eloran-hw`) is deliberately not here: it
//! stays on the command line, conducted use only (cables or antenna simulators, never over
//! the air), and is never reachable over HTTP.

use std::path::{Path, PathBuf};

use kshana_eloran::io::{Format, SigmfWriter};
use kshana_eloran::scene::{truth_csv, NullSink, RunStats, SampleSink};
use kshana_eloran::{Scenario, TruthRecord};
use serde_json::{json, Value};

/// Licence feature of the simulator (Enterprise).
pub const FEATURE: &str = "pro-eloran";

/// The example scenario (two chains, two UTC(k), crossed loops, moving receiver).
pub const EXAMPLE: &str =
    include_str!("../crates/kshana-eloran/scenarios/synthetic-two-chain.toml");

/// Parse and build a scenario; returns it with its SHA-256.
pub fn validate(text: &str) -> Result<(Scenario, String), String> {
    let sc = Scenario::from_toml(text)?;
    sc.build()?;
    let hash = sc.hash();
    Ok((sc, hash))
}

/// Samples a run produces over every output channel (duration × rate × channels).
pub fn samples_total(sc: &Scenario) -> f64 {
    sc.scene.params.duration_s.max(0.0) * sc.scene.params.fs_hz.max(0.0) * sc.outputs.len() as f64
}

/// `<base>.<suffix>` (the base may itself contain dots).
pub fn with_suffix(base: &Path, suffix: &str) -> PathBuf {
    let mut s = base.as_os_str().to_os_string();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

/// What [`run_to_files`] wrote.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The run record, as written to `<base>.run.json`.
    pub record: Value,
    pub stats: RunStats,
    /// `<base>.sigmf-data` and `<base>.sigmf-meta` (`None` for a truth-only run).
    pub samples: Option<PathBuf>,
    pub meta: Option<PathBuf>,
    pub truth: PathBuf,
    pub run_json: PathBuf,
    /// The truth table (CSV text, as written).
    pub truth_csv: String,
}

/// Run a scenario to files under `base`, as `kshana-pro eloran run` does. `scenario_label`
/// is what the run record names as the scenario (a path or a label); `format` overrides the
/// scenario's `[file] format` (`ri16`, `rf32` or `none`). The caller checks the licence.
pub fn run_to_files(
    sc: &Scenario,
    scenario_label: &str,
    base: &Path,
    format: Option<&str>,
    licensee: &str,
) -> Result<RunOutcome, String> {
    let fmt_s = format
        .map(str::to_string)
        .unwrap_or_else(|| sc.file.format.clone());
    let mut scene = sc.build()?;
    if let Some(dir) = base.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let hash = sc.hash();
    let extra = json!({
        "scenario_sha256": hash,
        "output_scale_uv_m": sc.scene.params.output_scale_uv_m,
        "outputs": sc.outputs,
        "start_utc": sc.scene.start_utc,
        "engine": format!("kshana-eloran {}", env!("CARGO_PKG_VERSION")),
    });
    let mut sink: Box<dyn SampleSink> = if fmt_s == "none" {
        Box::new(NullSink::default())
    } else {
        let fmt = Format::parse(&fmt_s)?;
        Box::new(SigmfWriter::create(
            base,
            fmt,
            sc.outputs.len(),
            sc.scene.params.fs_hz,
            extra,
        )?)
    };
    let mut truth = Vec::new();
    let stats = scene.run(sink.as_mut(), &mut truth)?;
    drop(sink);
    let truth_path = with_suffix(base, "truth.csv");
    let truth_text = truth_csv(&truth);
    std::fs::write(&truth_path, &truth_text)
        .map_err(|e| format!("{}: {e}", truth_path.display()))?;
    let (samples, meta) = if fmt_s == "none" {
        (None, None)
    } else {
        (
            Some(base.with_extension("sigmf-data")),
            Some(base.with_extension("sigmf-meta")),
        )
    };
    let record = json!({
        "format": "kshana-eloran-run",
        "version": 1,
        "scenario": scenario_label,
        "scenario_sha256": hash,
        "licensee": licensee,
        "stats": stats,
        "evidence_label": "MODELLED",
        "files": {
            "samples": samples,
            "metadata": meta,
            "truth": truth_path,
        },
    });
    let run_path = with_suffix(base, "run.json");
    std::fs::write(
        &run_path,
        serde_json::to_string_pretty(&record).unwrap_or_default(),
    )
    .map_err(|e| format!("{}: {e}", run_path.display()))?;
    Ok(RunOutcome {
        record,
        stats,
        samples,
        meta,
        truth: truth_path,
        run_json: run_path,
        truth_csv: truth_text,
    })
}

/// The first `rows` data rows of a truth table: `{columns, rows, total}` (cells as text).
pub fn truth_preview(csv: &str, rows: usize) -> Value {
    let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
    let columns: Vec<&str> = lines
        .next()
        .map(|h| h.split(',').collect())
        .unwrap_or_default();
    let body: Vec<&str> = lines.collect();
    let shown: Vec<Vec<&str>> = body
        .iter()
        .take(rows)
        .map(|l| l.split(',').collect())
        .collect();
    json!({"columns": columns, "rows": shown, "total": body.len()})
}

/// Receiver options (the command line's `--per-loop` and `--no-cri-blanking`).
#[derive(Debug, Clone, Copy)]
pub struct RxOptions {
    /// Crossed loops processed per loop instead of beam-steered.
    pub per_loop: bool,
    /// Blank cross-rate interference (default on).
    pub cri_blanking: bool,
}

impl Default for RxOptions {
    fn default() -> Self {
        RxOptions {
            per_loop: false,
            cri_blanking: true,
        }
    }
}

/// What [`receive`] produced: the report (`kshana-eloran-rx`) and the measurements as CSV.
#[derive(Debug, Clone)]
pub struct RxOutcome {
    pub report: Value,
    pub measurements_csv: String,
}

/// The software receiver on the recording at `recording` (a SigMF base), as `kshana-pro
/// eloran rx` does: the scenario gives the chains, the start and the antennas. With `truth`,
/// the report carries the error statistics against the simulator truth. `recording_label`
/// is what the report names as the recording. The caller checks the licence.
pub fn receive(
    sc: &Scenario,
    recording: &Path,
    recording_label: &str,
    truth: Option<&[TruthRecord]>,
    opts: RxOptions,
) -> Result<RxOutcome, String> {
    use kshana_eloran::rx::{self, solve, truth as rxt, Antenna, LoopMode, RxConfig, RxInput};
    sc.build()?;
    let start = kshana_eloran::time::AbsTime::from_utc_iso(&sc.scene.start_utc)?;
    let rec = kshana_eloran::io::read_sigmf(recording, None)?;
    let chains: Vec<_> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let mut cfg = RxConfig {
        output_scale_uv_m: sc.scene.params.output_scale_uv_m,
        ..Default::default()
    };
    let loops: Vec<usize> = sc
        .outputs
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, kshana_eloran::antenna::OutputChannel::HLoop { .. }))
        .map(|(i, _)| i)
        .collect();
    cfg.antenna = if loops.len() >= 2 {
        let mode = if opts.per_loop {
            LoopMode::PerLoop
        } else {
            LoopMode::BeamSteer
        };
        Antenna::CrossedLoop {
            a: loops[0],
            b: loops[1],
            mode,
        }
    } else {
        Antenna::EField { channel: 0 }
    };
    if !opts.cri_blanking {
        cfg.cri.blank = false;
    }
    let input = RxInput::from_recording(&rec, start);
    let (_rx, output) = rx::process(&input, &chains, cfg.clone())?;
    let mut csv = String::from("chain,station,channel,group,toa_s,envelope_toa_s,ecd_us,snr_db,field_uv_m,cycle_slip,bearing_deg,pulses\n");
    for m in &output.measurements {
        csv.push_str(&format!(
            "{},{},{},{},{:.12},{:.12},{:.4},{:.2},{:.4},{},{},{}\n",
            m.chain,
            m.station,
            m.channel.map(|c| c.to_string()).unwrap_or_default(),
            m.group,
            m.toa_s,
            m.envelope_toa_s,
            m.ecd_us,
            m.snr_db,
            m.field_uv_m,
            m.cycle_slip,
            m.bearing_deg.map(|b| format!("{b:.3}")).unwrap_or_default(),
            m.pulses
        ));
    }
    let corr = cfg.corrections.clone();
    let rx_pos = sc.receiver.state(0.0).pos;
    let ts = solve::time_solution(&output.measurements, &chains, start, rx_pos, &corr);
    let pos = solve::position_solution(
        &output.measurements,
        &chains,
        start,
        &corr,
        &solve::PositionConfig::default(),
    );
    let mut report = json!({
        "format": "kshana-eloran-rx",
        "version": 1,
        "scenario_sha256": sc.hash(),
        "recording": recording_label,
        "stations_tracked": output.tracks.len(),
        "measurements": output.measurements.len(),
        "time_solution": ts.stations.iter().map(|s| json!({
            "chain": s.chain, "station": s.station, "time_ref": s.time_ref,
            "ref_minus_rx_ns": s.ref_minus_rx_ns, "std_ns": s.std_ns, "n": s.n })).collect::<Vec<_>>(),
        "time_by_reference": ts.by_time_ref.iter().map(|(r, v, n)| json!({"time_ref": r, "ref_minus_rx_ns": v, "stations": n})).collect::<Vec<_>>(),
        "position_solution": match &pos {
            Ok(p) => json!({
                "lat_deg": p.position.lat_deg, "lon_deg": p.position.lon_deg, "hdop": p.hdop,
                "clock_bias_ns": p.clock_bias_ns, "residuals_ns": p.residuals_ns, "converged": p.converged }),
            Err(e) => json!({ "error": e }),
        },
        "evidence_label": "MODELLED",
    });
    if let Some(truth) = truth {
        let errs = rxt::toa_errors(&output.measurements, truth);
        report["truth_comparison"] = json!({
            "stations": errs.iter().map(|e| json!({
                "chain": e.chain, "station": e.station, "channel": e.channel, "n": e.n,
                "toa_mean_ns": e.toa_mean_ns, "toa_std_ns": e.toa_std_ns, "toa_rms_ns": e.toa_rms_ns,
                "toa_max_abs_ns": e.toa_max_abs_ns, "ecd_rms_err_us": e.ecd_rms_err_us,
                "cycle_errors": e.cycle_errors })).collect::<Vec<_>>(),
            "position_error_m": pos.as_ref().ok().and_then(|p| rxt::position_error_m(p.position, truth)),
        });
    }
    Ok(RxOutcome {
        report,
        measurements_csv: csv,
    })
}

/// Read a truth table written by [`run_to_files`].
pub fn read_truth(path: &Path) -> Result<Vec<TruthRecord>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    kshana_eloran::scene::truth_from_csv(&text)
}

/// One chain of the database as a list row: identity, status, confidence and the station
/// positions (for a map).
fn gri_row(c: &kshana_eloran::gridb::DbChain) -> Value {
    json!({
        "id": c.id,
        "gri": c.gri_us / 10,
        "gri_us": c.gri_us,
        "name": c.name,
        "region": c.region,
        "countries": c.countries,
        "system": c.system,
        "status": c.status,
        "confidence": c.confidence,
        "complete": c.is_complete(),
        "operational_from": c.operational_from,
        "operational_to": c.operational_to,
        "time_ref": c.time_ref,
        "stations": c.stations.iter().map(|s| json!({
            "id": s.id, "name": s.name, "role": s.role,
            "lat_deg": s.position.as_ref().map(|p| p.lat_deg),
            "lon_deg": s.position.as_ref().map(|p| p.lon_deg),
            "confidence": s.confidence,
        })).collect::<Vec<_>>(),
    })
}

/// The GRI database, filtered like `kshana-pro eloran gri list` (`region`, `active_on`
/// YYYY-MM-DD, free-text `search`): `{format, updated, count, chains: [...]}`.
pub fn gri_list(
    region: Option<&str>,
    active_on: Option<&str>,
    search: Option<&str>,
) -> Result<Value, String> {
    use kshana_eloran::gridb::{DbChain, GriDb};
    let db = GriDb::embedded()?;
    let mut v: Vec<&DbChain> = db.chains.iter().collect();
    if let Some(r) = region.filter(|r| !r.is_empty()) {
        let keep: Vec<String> = db.by_region(r).iter().map(|c| c.id.clone()).collect();
        v.retain(|c| keep.contains(&c.id));
    }
    if let Some(d) = active_on.filter(|d| !d.is_empty()) {
        v.retain(|c| c.active_on(d));
    }
    if let Some(q) = search.filter(|q| !q.trim().is_empty()) {
        let keep: Vec<String> = db.search(q).iter().map(|c| c.id.clone()).collect();
        v.retain(|c| keep.contains(&c.id));
    }
    Ok(json!({
        "format": db.format,
        "version": db.version,
        "updated": db.updated,
        "count": v.len(),
        "chains": v.iter().map(|c| gri_row(c)).collect::<Vec<_>>(),
    }))
}

/// The chains matching `target` (an entry id, a GRI such as `9960`, or search text), each
/// with its full entry and its `[[chains]]` scenario snippet: `Ok(None)` when none matches.
pub fn gri_show(target: &str) -> Result<Option<Value>, String> {
    use kshana_eloran::gridb::{self, DbChain, GriDb};
    let db = GriDb::embedded()?;
    let found: Vec<&DbChain> = if let Some(c) = db.get(target) {
        vec![c]
    } else {
        match target.parse::<u32>() {
            Ok(g) => db.by_gri(g),
            Err(_) => db.search(target),
        }
    };
    if found.is_empty() {
        return Ok(None);
    }
    let chains: Vec<Value> = found
        .iter()
        .map(|c| {
            let (snippet, snippet_error) = match gridb::scenario_snippet_for(c, &Default::default())
            {
                Ok(s) => (Some(s), None),
                Err(e) => (None, Some(e)),
            };
            json!({
                "summary": gri_row(c),
                "entry": c,
                "snippet": snippet,
                "snippet_error": snippet_error,
            })
        })
        .collect();
    Ok(Some(
        json!({"query": target, "count": chains.len(), "chains": chains}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_validates() {
        let (_, h) = validate(EXAMPLE).unwrap();
        assert_eq!(h.len(), 64);
        assert!(validate("[scene]\n").is_err());
    }

    #[test]
    fn preview_keeps_the_header_and_counts_rows() {
        let p = truth_preview("a,b\n1,2\n3,4\n5,6\n", 2);
        assert_eq!(p["columns"], json!(["a", "b"]));
        assert_eq!(p["rows"], json!([["1", "2"], ["3", "4"]]));
        assert_eq!(p["total"], 3);
    }

    #[test]
    fn gri_database_lists_and_shows_with_a_snippet() {
        let all = gri_list(None, None, None).unwrap();
        assert!(all["count"].as_u64().unwrap() >= 30);
        let first = all["chains"][0]["id"].as_str().unwrap().to_string();
        let shown = gri_show(&first).unwrap().unwrap();
        assert_eq!(shown["chains"][0]["summary"]["id"], json!(first));
        assert!(gri_show("no-such-chain-anywhere").unwrap().is_none());
        let complete = all["chains"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["complete"] == true)
            .unwrap();
        let s = gri_show(complete["id"].as_str().unwrap()).unwrap().unwrap();
        assert!(s["chains"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("[[chains]]"));
    }
}
