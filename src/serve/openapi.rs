// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The OpenAPI 3.1 document
//!
//! Generated at request time from [`super::api::ROUTES`], the same table the router
//! matches against, filtered the same way (the job routes appear only when the server
//! has a data directory). A route cannot be served without being documented, and a
//! documented path cannot be missing from the router.

use serde_json::{json, Map, Value};

use super::api::served_routes;
use super::ops::OPERATIONS;

/// The path parameters of a template, in order (`/jobs/{id}` → `["id"]`).
pub fn path_params(template: &str) -> Vec<&str> {
    template
        .split('/')
        .filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
        .collect()
}

fn param_doc(name: &str) -> Value {
    let (desc, schema) = match name {
        "id" => (
            "Job identifier: SHA-256 of the operation and the canonical request",
            json!({"type": "string", "pattern": "^[0-9a-f]{64}$"}),
        ),
        "name" => (
            "Artefact file name",
            json!({"type": "string", "pattern": "^[A-Za-z0-9_-][A-Za-z0-9._-]{0,127}$"}),
        ),
        "chain" => (
            "GRI database entry: an entry id, a GRI (e.g. 9960) or search text",
            json!({"type": "string", "maxLength": 128}),
        ),
        _ => ("", json!({"type": "string"})),
    };
    json!({"name": name, "in": "path", "required": true, "description": desc, "schema": schema})
}

/// The OpenAPI operationId of a route (`GET /jobs/{id}` → `get_jobs_id`).
pub fn operation_id(method: &str, template: &str) -> String {
    let mut s = method.to_ascii_lowercase();
    for seg in template.split('/').filter(|s| !s.is_empty()) {
        let seg = seg.trim_start_matches('{').trim_end_matches('}');
        s.push('_');
        s.push_str(&seg.replace(['.', '-'], "_"));
    }
    if template == "/" {
        s.push_str("_root");
    }
    s
}

/// Build the document for a server with or without a data directory and a token.
pub fn document(jobs_enabled: bool, token: bool) -> Value {
    let mut paths: Map<String, Value> = Map::new();
    for r in served_routes(jobs_enabled) {
        let mut op = Map::new();
        op.insert("operationId".into(), json!(operation_id(r.method, r.path)));
        op.insert("summary".into(), json!(r.summary));
        let mut params: Vec<Value> = path_params(r.path).into_iter().map(param_doc).collect();
        for (name, desc) in r.query {
            params.push(json!({"name": name, "in": "query", "required": false, "description": desc, "schema": {"type": "string"}}));
        }
        if !params.is_empty() {
            op.insert("parameters".into(), Value::Array(params));
        }
        if let Some((media, desc)) = r.request {
            let schema = if r.path == "/jobs" {
                json!({"$ref": "#/components/schemas/JobSubmission"})
            } else if media == "application/json" {
                json!({"type": "object"})
            } else {
                json!({"type": "string"})
            };
            op.insert(
                "requestBody".into(),
                json!({"required": true, "description": desc, "content": {media: {"schema": schema}}}),
            );
        }
        let mut responses = Map::new();
        for (code, media, desc) in r.responses {
            if *code == 401 && !token {
                continue;
            }
            let schema = match (*code, r.path) {
                (c, _) if c >= 400 => json!({"$ref": "#/components/schemas/Error"}),
                (200, "/jobs/{id}") => json!({"$ref": "#/components/schemas/JobStatus"}),
                (_, _) if media.starts_with("text/") => json!({"type": "string"}),
                _ => json!({}),
            };
            responses.insert(
                code.to_string(),
                json!({"description": desc, "content": {*media: {"schema": schema}}}),
            );
        }
        op.insert("responses".into(), Value::Object(responses));
        if token && r.path == "/health" {
            op.insert("security".into(), json!([]));
        }
        let entry = paths
            .entry(r.path.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        entry[r.method.to_ascii_lowercase()] = Value::Object(op);
    }

    let op_names: Vec<&str> = OPERATIONS.iter().map(|o| o.name).collect();
    let op_docs: Vec<Value> = OPERATIONS
        .iter()
        .map(|o| json!({"name": o.name, "summary": o.summary, "feature": o.feature}))
        .collect();
    let mut doc = json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Kshana Pro service",
            "version": crate::version(),
            "description": "On-premises job service of the Kshana Pro overlay: queued runs that survive a restart, stored artefacts and a hash-chained delivery ledger. Generated from the server's route table.",
        },
        "paths": paths,
        "components": {
            "schemas": {
                "Error": {"type": "object", "properties": {"error": {"type": "string"}}, "required": ["error"]},
                "JobSubmission": {
                    "type": "object",
                    "properties": {
                        "op": {"type": "string", "enum": op_names},
                        "request": {"type": "object"},
                    },
                    "required": ["op", "request"],
                    "additionalProperties": false,
                },
                "JobStatus": {
                    "type": "object",
                    "properties": {
                        "job_id": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                        "op": {"type": "string"},
                        "kind": {"type": ["string", "null"]},
                        "state": {"type": "string", "enum": ["queued", "running", "succeeded", "failed"]},
                        "seq": {"type": "integer"},
                        "submitted_unix": {"type": "integer"},
                        "submitted_utc": {"type": "string"},
                        "started_utc": {"type": ["string", "null"]},
                        "finished_utc": {"type": ["string", "null"]},
                        "attempts": {"type": "integer"},
                        "error": {"type": ["string", "null"]},
                        "result_sha256": {"type": ["string", "null"]},
                        "label": {"type": ["string", "null"]},
                        "artefacts": {"type": "array", "items": {"type": "object"}},
                    },
                    "required": ["job_id", "op", "state"],
                },
            },
        },
        "x-kshana-operations": op_docs,
    });
    if token {
        doc["components"]["securitySchemes"] =
            json!({"bearerAuth": {"type": "http", "scheme": "bearer"}});
        doc["security"] = json!([{"bearerAuth": []}]);
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_paths_equal_the_served_templates() {
        for jobs in [false, true] {
            let doc = document(jobs, false);
            let mut documented: Vec<(String, String)> = Vec::new();
            for (p, ops) in doc["paths"].as_object().unwrap() {
                for m in ops.as_object().unwrap().keys() {
                    documented.push((m.to_ascii_uppercase(), p.clone()));
                }
            }
            let mut served: Vec<(String, String)> = served_routes(jobs)
                .map(|r| (r.method.to_string(), r.path.to_string()))
                .collect();
            documented.sort();
            served.sort();
            assert_eq!(documented, served);
        }
    }

    #[test]
    fn token_adds_a_security_scheme_and_frees_health() {
        let doc = document(true, true);
        assert_eq!(
            doc["components"]["securitySchemes"]["bearerAuth"]["scheme"],
            "bearer"
        );
        assert_eq!(doc["paths"]["/health"]["get"]["security"], json!([]));
        assert!(document(true, false).get("security").is_none());
    }
}
