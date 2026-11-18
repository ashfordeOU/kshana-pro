// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The eLoran/GNSS simulator routes
//!
//! | Route | What | Licence |
//! |---|---|---|
//! | `GET /eloran/example` | the example scenario (TOML) | free |
//! | `POST /eloran/validate` | body: scenario TOML → `{ok, scenario_sha256}` or 422 with the error | free |
//! | `POST /eloran/run` | body: scenario TOML → with a data directory, an `eloran-run` job (202, poll `/jobs/{id}`; its result is the run record); without one, a synchronous run of at most [`SYNC_MAX_SAMPLES`] samples (200, the run record) | `pro-eloran` |
//! | `POST /eloran/rx` | body: `{"run": <run id>, "scenario"?: <TOML>, "per_loop"?: bool, "cri_blanking"?: bool}` → the receiver report with the truth comparison | `pro-eloran` |
//! | `GET /eloran/gri` | the GRI/chain database (`region`, `active_on`, `search`) | free |
//! | `GET /eloran/gri/{chain}` | entries matching an id, a GRI or text, with their `[[chains]]` snippet | free |
//!
//! A run's identifier is the job identifier ([`super::jobs::tenant_job_id`] of the tenant,
//! `eloran-run` and the request), with or without a data directory. With one, the recording
//! is kept in the job's `inputs/` directory (it is binary, so it is not a served artefact);
//! the run record, the truth table and the SigMF metadata are the job's artefacts. Without
//! one, the files of a synchronous run are kept under [`sync_runs_dir`], one directory per
//! run with the owning tenant recorded; `POST /eloran/rx` finds a run in either place and
//! answers for another tenant's run exactly as for a run that does not exist.
//!
//! Hardware output (`kshana-pro eloran stream`, feature `pro-eloran-hw`) is never served over
//! HTTP: it stays on the command line, conducted use only.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::api::Service;
use super::auth::Principal;
use super::jobs::{is_job_id, JobState};
use super::ops::{OpError, OpOutput, Prepared, RunContext};
use super::HttpResponse;
use crate::eloran_service as es;
use crate::license::Entitlement;

/// The job operation behind `POST /eloran/run` with a data directory.
pub const OP: &str = "eloran-run";
/// The largest synchronous run (samples over every channel): 8 million, e.g. 2 s at 2 MHz on
/// two channels. A larger run needs the job service (`--data-dir`) or the command line.
pub const SYNC_MAX_SAMPLES: f64 = 8.0e6;
/// The largest recording `POST /eloran/rx` reads (bytes of `.sigmf-data`).
pub const RX_MAX_BYTES: u64 = 256 * 1024 * 1024;
/// Truth rows returned in a run record's preview.
pub const PREVIEW_ROWS: usize = 20;
/// The base name of a run's files in its directory.
const BASE: &str = "recording";

/// Where a server without a data directory keeps synchronous runs.
pub fn sync_runs_dir() -> PathBuf {
    std::env::temp_dir().join("kshana-pro-eloran-runs")
}

fn licensed(ent: &Entitlement, now: i64) -> Result<(), HttpResponse> {
    crate::license::check(ent, es::FEATURE, now)
        .map_err(|e| HttpResponse::error(403, &e.to_string()))
}

fn json_ok(status: u16, v: &Value) -> HttpResponse {
    HttpResponse::json(status, serde_json::to_string_pretty(v).unwrap_or_default())
}

/// `GET /eloran/example`.
pub fn example() -> HttpResponse {
    let mut r = HttpResponse::text(200, es::EXAMPLE);
    r.content_type = "text/plain; charset=utf-8";
    r
}

/// `POST /eloran/validate`.
pub fn validate(body: &str) -> HttpResponse {
    if body.trim().is_empty() {
        return HttpResponse::error(422, "empty request body (expected a scenario TOML)");
    }
    match es::validate(body) {
        Ok((sc, hash)) => json_ok(
            200,
            &json!({
                "ok": true,
                "scenario_sha256": hash,
                "duration_s": sc.scene.params.duration_s,
                "fs_hz": sc.scene.params.fs_hz,
                "channels": sc.outputs.len(),
                "chains": sc.chains.iter().map(|c| json!({"name": c.chain.name, "gri_us": c.chain.gri_us, "stations": c.chain.stations.len()})).collect::<Vec<_>>(),
                "samples_total": es::samples_total(&sc),
                "synchronous_limit": SYNC_MAX_SAMPLES,
            }),
        ),
        Err(e) => HttpResponse::json(422, json!({"ok": false, "error": e}).to_string()),
    }
}

/// Decode a `%XX` / `+` query value.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 3 <= b.len()
                && b[i + 1].is_ascii_hexdigit()
                && b[i + 2].is_ascii_hexdigit() =>
            {
                let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push(hex(b[i + 1]) * 16 + hex(b[i + 2]));
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `GET /eloran/gri`.
pub fn gri_list(query: &str) -> HttpResponse {
    let (mut region, mut active, mut search) = (None, None, None);
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = percent_decode(v);
        match k {
            "region" => region = Some(v),
            "active_on" => active = Some(v),
            "search" => search = Some(v),
            _ => {
                return HttpResponse::error(
                    400,
                    &format!("unknown query parameter '{k}' (region, active_on, search)"),
                )
            }
        }
    }
    match es::gri_list(region.as_deref(), active.as_deref(), search.as_deref()) {
        Ok(v) => json_ok(200, &v),
        Err(e) => HttpResponse::error(500, &e),
    }
}

/// `GET /eloran/gri/{chain}`.
pub fn gri_show(target: &str) -> HttpResponse {
    let t = percent_decode(target);
    if t.trim().is_empty() || t.len() > 128 {
        return HttpResponse::error(
            400,
            "name a chain: an entry id, a GRI (e.g. 9960) or search text",
        );
    }
    match es::gri_show(t.trim()) {
        Ok(Some(v)) => json_ok(200, &v),
        Ok(None) => HttpResponse::error(404, "no chain of the GRI database matches"),
        Err(e) => HttpResponse::error(500, &e),
    }
}

/// The optional `format` query parameter of `POST /eloran/run`.
fn format_param(query: &str) -> Result<Option<String>, HttpResponse> {
    let mut format = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "format" if ["ri16", "rf32", "none"].contains(&v) => format = Some(v.to_string()),
            "format" => {
                return Err(HttpResponse::error(
                    400,
                    "format must be ri16, rf32 or none",
                ))
            }
            _ => {
                return Err(HttpResponse::error(
                    400,
                    &format!("unknown query parameter '{k}' (format)"),
                ))
            }
        }
    }
    Ok(format)
}

/// The job request of a run.
fn run_request(scenario: &str, format: Option<&str>) -> Value {
    match format {
        Some(f) => json!({"scenario": scenario, "format": f}),
        None => json!({"scenario": scenario}),
    }
}

/// `POST /eloran/run`.
pub fn run(svc: &Service, who: &Principal, body: &str, query: &str, now: i64) -> HttpResponse {
    if let Err(r) = licensed(&svc.entitlement, now) {
        return r;
    }
    let format = match format_param(query) {
        Ok(f) => f,
        Err(r) => return r,
    };
    if body.trim().is_empty() {
        return HttpResponse::error(422, "empty request body (expected a scenario TOML)");
    }
    let sc = match es::validate(body) {
        Ok((sc, _)) => sc,
        Err(e) => return HttpResponse::error(422, &e),
    };
    let request = run_request(body, format.as_deref());
    if let Some(j) = svc.jobs.as_deref() {
        let submission = json!({"op": OP, "request": request}).to_string();
        let r = super::api::submit(svc, j, who, &submission, now);
        if r.status / 100 != 2 {
            return r;
        }
        let mut v: Value = serde_json::from_str(&r.body).unwrap_or_default();
        let id = v["job_id"].as_str().unwrap_or_default().to_string();
        v["mode"] = json!("job");
        v["run_id"] = json!(id);
        v["links"]["result"] = json!(format!("/jobs/{id}/result"));
        v["links"]["events"] = json!(format!("/jobs/{id}/events"));
        v["links"]["truth"] = json!(format!("/jobs/{id}/artefacts/truth.csv"));
        return json_ok(r.status, &v);
    }
    let total = es::samples_total(&sc);
    if total > SYNC_MAX_SAMPLES {
        return HttpResponse::error(
            422,
            &format!(
                "this run has {total:.0} samples; a synchronous run is limited to {SYNC_MAX_SAMPLES:.0}: shorten it, start the service with --data-dir to run it as a job, or use `kshana-pro eloran run`"
            ),
        );
    }
    let tenant = super::api::submit_tenant(who);
    let id = super::jobs::tenant_job_id(&tenant, OP, &request);
    let dir = sync_runs_dir().join(&id);
    // Always run (a synchronous run is small, and a kept directory may come from an older
    // engine); the finished directory replaces any earlier one of the same run.
    {
        let tmp = sync_runs_dir().join(format!(
            ".{id}.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let res = (|| -> Result<(), String> {
            std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
            std::fs::write(tmp.join("tenant"), &tenant).map_err(|e| e.to_string())?;
            std::fs::write(tmp.join("scenario.toml"), body).map_err(|e| e.to_string())?;
            es::run_to_files(
                &sc,
                "request body",
                &tmp.join(BASE),
                format.as_deref(),
                &svc.entitlement.licensee,
            )?;
            if dir.exists() {
                let _ = std::fs::remove_dir_all(&dir);
            }
            std::fs::rename(&tmp, &dir).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&tmp);
        if let Err(e) = res {
            return HttpResponse::error(500, &format!("the run failed: {e}"));
        }
    }
    match sync_record(&dir, &id) {
        Ok(v) => json_ok(200, &v),
        Err(e) => HttpResponse::error(500, &e),
    }
}

/// The run record of a synchronous run, with its identifier, files and truth preview.
fn sync_record(dir: &Path, id: &str) -> Result<Value, String> {
    let base = dir.join(BASE);
    let mut rec: Value = serde_json::from_str(
        &std::fs::read_to_string(es::with_suffix(&base, "run.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let truth = std::fs::read_to_string(es::with_suffix(&base, "truth.csv")).unwrap_or_default();
    // The files are rewritten as the run directory holds them now (it was renamed into place).
    let samples = base.with_extension("sigmf-data");
    let has_samples = samples.is_file();
    rec["files"] = json!({
        "samples": has_samples.then(|| samples.clone()),
        "metadata": has_samples.then(|| base.with_extension("sigmf-meta")),
        "truth": es::with_suffix(&base, "truth.csv"),
        "run": es::with_suffix(&base, "run.json"),
    });
    rec["run_id"] = json!(id);
    std::fs::write(
        es::with_suffix(&base, "run.json"),
        serde_json::to_string_pretty(&rec).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    rec["mode"] = json!("sync");
    rec["artefacts"] = json!({
        "samples": has_samples.then(|| format!("{BASE}.sigmf-data")),
        "metadata": has_samples.then(|| format!("{BASE}.sigmf-meta")),
        "truth": format!("{BASE}.truth.csv"),
        "run": format!("{BASE}.run.json"),
        "samples_sha256": if has_samples { std::fs::read(&samples).ok().map(|b| super::jobs::sha256_hex(&b)) } else { None },
    });
    rec["truth_preview"] = es::truth_preview(&truth, PREVIEW_ROWS);
    Ok(rec)
}

/// Where a run's files are, for `POST /eloran/rx`: `(recording base, truth table, stored
/// scenario)`, or the response refusing it.
fn locate(
    svc: &Service,
    who: &Principal,
    id: &str,
) -> Result<(PathBuf, PathBuf, String), HttpResponse> {
    if !is_job_id(id) {
        return Err(HttpResponse::error(
            400,
            "run is a run identifier: 64 lower-case hexadecimal characters",
        ));
    }
    let missing = || HttpResponse::error(404, "no such run");
    if let Some(j) = svc.jobs.as_deref() {
        let s = match j.store.status(id) {
            Ok(Some(s)) if who.sees(&s.tenant) && s.op == OP => s,
            Ok(_) => return Err(missing()),
            Err(e) => {
                return Err(HttpResponse::error(
                    500,
                    &format!("cannot read the run: {e}"),
                ))
            }
        };
        if s.state != JobState::Succeeded {
            return Err(HttpResponse::json(
                409,
                json!({"error": "the run has not succeeded", "job_id": s.job_id, "state": s.state.as_str(), "job_error": s.error}).to_string(),
            ));
        }
        let (_, req) = j
            .store
            .request(id)
            .map_err(|e| HttpResponse::error(500, &e.to_string()))?;
        let base = j.store.job_dir(id).join("inputs").join(BASE);
        let scenario = req["scenario"].as_str().unwrap_or_default().to_string();
        return Ok((base.clone(), es::with_suffix(&base, "truth.csv"), scenario));
    }
    let dir = sync_runs_dir().join(id);
    match std::fs::read_to_string(dir.join("tenant")) {
        Ok(t) if who.sees(t.trim()) => {}
        _ => return Err(missing()),
    }
    let base = dir.join(BASE);
    let scenario = std::fs::read_to_string(dir.join("scenario.toml")).unwrap_or_default();
    Ok((base.clone(), es::with_suffix(&base, "truth.csv"), scenario))
}

/// `POST /eloran/rx`.
pub fn rx(svc: &Service, who: &Principal, body: &str, now: i64) -> HttpResponse {
    if let Err(r) = licensed(&svc.entitlement, now) {
        return r;
    }
    let v: Value = match serde_json::from_str(body) {
        Ok(v @ Value::Object(_)) => v,
        Ok(_) => {
            return HttpResponse::error(
                422,
                "the body is a JSON object {\"run\": <run id>, \"scenario\"?: <TOML>}",
            )
        }
        Err(e) => return HttpResponse::error(400, &format!("malformed JSON: {e}")),
    };
    if let Some(k) = v.as_object().and_then(|o| {
        o.keys().find(|k| {
            !["run", "scenario", "per_loop", "cri_blanking", "truth"].contains(&k.as_str())
        })
    }) {
        return HttpResponse::error(
            422,
            &format!("unknown field '{k}' (run, scenario, per_loop, cri_blanking, truth)"),
        );
    }
    let Some(id) = v["run"].as_str() else {
        return HttpResponse::error(
            422,
            "run is required: the run_id of a stored run (POST /eloran/run)",
        );
    };
    let flag = |k: &str, d: bool| v.get(k).and_then(Value::as_bool).unwrap_or(d);
    for k in ["per_loop", "cri_blanking", "truth"] {
        if v.get(k).is_some_and(|x| !x.is_boolean()) {
            return HttpResponse::error(422, &format!("{k} must be true or false"));
        }
    }
    let (base, truth_path, stored) = match locate(svc, who, id) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let text = match v.get("scenario") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        None | Some(Value::Null) => stored,
        Some(_) => return HttpResponse::error(422, "scenario must be a TOML string"),
    };
    let sc = match es::validate(&text) {
        Ok((sc, _)) => sc,
        Err(e) => return HttpResponse::error(422, &e),
    };
    let data = base.with_extension("sigmf-data");
    match std::fs::metadata(&data) {
        Ok(m) if m.len() > RX_MAX_BYTES => {
            return HttpResponse::error(
                422,
                &format!("the recording has {} bytes; the service reads at most {RX_MAX_BYTES}: use `kshana-pro eloran rx`", m.len()),
            )
        }
        Ok(_) => {}
        Err(_) => return HttpResponse::error(409, "this run has no recording (it was a truth-only run, format none)"),
    }
    let truth = if flag("truth", true) {
        match es::read_truth(&truth_path) {
            Ok(t) => Some(t),
            Err(e) => {
                return HttpResponse::error(500, &format!("cannot read the run's truth: {e}"))
            }
        }
    } else {
        None
    };
    let opts = es::RxOptions {
        per_loop: flag("per_loop", false),
        cri_blanking: flag("cri_blanking", true),
    };
    match es::receive(&sc, &base, &format!("run {id}"), truth.as_deref(), opts) {
        Ok(out) => {
            let mut rep = out.report;
            rep["run_id"] = json!(id);
            json_ok(200, &rep)
        }
        Err(e) => HttpResponse::error(422, &format!("the receiver failed: {e}")),
    }
}

// ─── the job operation ──────────────────────────────────────────────────────────

fn op_scenario(req: &Value) -> Result<(&str, Option<&str>), OpError> {
    let src = match req.get("scenario") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.as_str(),
        _ => {
            return Err(OpError::Invalid(
                "request.scenario is required: a scenario TOML".into(),
            ))
        }
    };
    let format = match req.get("format") {
        None | Some(Value::Null) => None,
        Some(Value::String(f)) if ["ri16", "rf32", "none"].contains(&f.as_str()) => {
            Some(f.as_str())
        }
        Some(_) => {
            return Err(OpError::Invalid(
                "request.format must be ri16, rf32 or none".into(),
            ))
        }
    };
    if let Some(k) = req.as_object().and_then(|o| {
        o.keys()
            .find(|k| !["scenario", "format"].contains(&k.as_str()))
    }) {
        return Err(OpError::Invalid(format!(
            "unknown field request.{k} (scenario, format)"
        )));
    }
    Ok((src, format))
}

/// `prepare` of the `eloran-run` operation: the licence, then the scenario.
pub fn prepare_op(req: &Value, ent: &Entitlement, now: i64) -> Result<Prepared, OpError> {
    crate::license::check(ent, es::FEATURE, now).map_err(|e| OpError::Forbidden(e.to_string()))?;
    let (src, _) = op_scenario(req)?;
    es::validate(src).map_err(OpError::Invalid)?;
    Ok(Prepared {
        kind: Some(es::FEATURE.into()),
    })
}

/// `run` of the `eloran-run` operation: the recording in the job's `inputs/`, the run record,
/// truth table and SigMF metadata as artefacts.
pub fn run_op(req: &Value, ctx: &RunContext<'_>) -> Result<OpOutput, String> {
    let to_text = |e: OpError| match e {
        OpError::Invalid(m) | OpError::Forbidden(m) => m,
    };
    prepare_op(req, ctx.entitlement, ctx.now_unix).map_err(to_text)?;
    let (src, format) = op_scenario(req).map_err(to_text)?;
    let (sc, _) = es::validate(src)?;
    std::fs::create_dir_all(ctx.work_dir)
        .map_err(|e| format!("cannot create the run directory: {e}"))?;
    let out = es::run_to_files(
        &sc,
        "request.scenario",
        &ctx.work_dir.join(BASE),
        format,
        &ctx.entitlement.licensee,
    )?;
    let mut result = out.record.clone();
    let mut artefacts = vec![
        (
            "run.json".to_string(),
            serde_json::to_string_pretty(&out.record).unwrap_or_default(),
        ),
        ("truth.csv".to_string(), out.truth_csv.clone()),
    ];
    if let Some(m) = &out.meta {
        let meta = std::fs::read_to_string(m).map_err(|e| format!("{}: {e}", m.display()))?;
        artefacts.push(("recording.sigmf-meta".to_string(), meta));
    }
    result["mode"] = json!("job");
    result["artefacts"] = json!({
        "samples": out.samples.as_ref().map(|_| "inputs/recording.sigmf-data (kept with the job, read by POST /eloran/rx)"),
        "metadata": out.meta.as_ref().map(|_| "recording.sigmf-meta"),
        "truth": "truth.csv",
        "run": "run.json",
        "samples_sha256": out.samples.as_ref().and_then(|p| std::fs::read(p).ok()).map(|b| super::jobs::sha256_hex(&b)),
    });
    result["truth_preview"] = es::truth_preview(&out.truth_csv, PREVIEW_ROWS);
    Ok(OpOutput {
        result: serde_json::to_string_pretty(&result).unwrap_or_default(),
        artefacts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_values_are_decoded() {
        assert_eq!(percent_decode("North%20Atlantic+x"), "North Atlantic x");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%41"), "A");
    }

    #[test]
    fn run_format_parameter() {
        assert_eq!(
            format_param("format=none").unwrap().as_deref(),
            Some("none")
        );
        assert!(format_param("format=wav").is_err());
        assert!(format_param("x=1").is_err());
        assert_eq!(format_param("").unwrap(), None);
    }
}
