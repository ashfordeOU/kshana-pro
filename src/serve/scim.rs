// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # SCIM 2.0 user and group provisioning (roadmap 0.42 F8, Enterprise)
//!
//! An organisation's identity provider provisions the people who may use the service
//! (RFC 7643 core schema, RFC 7644 protocol, the subset identity providers use):
//!
//! * `GET/POST /scim/v2/Users`, `GET/PUT/PATCH/DELETE /scim/v2/Users/{id}`; a list takes
//!   `filter=userName eq "<name>"`. PATCH applies `replace` of `active`, `displayName` or
//!   `userName` (the forms identity providers send to deactivate a user).
//! * `GET/POST /scim/v2/Groups`, `GET/PATCH/DELETE /scim/v2/Groups/{id}`; PATCH `add` and
//!   `remove` of `members` and `replace` of `displayName`.
//! * `GET /scim/v2/ServiceProviderConfig`.
//!
//! Stored per tenant under `<data-dir>/scim/<tenant>/{users,groups}/<id>.json`; the caller's
//! tenant is the only one it reads or writes (an operator key of every tenant has none and
//! is refused). Every route needs the `admin` role and the Enterprise feature `pro-scim`.
//!
//! **Effect on sign-in** ([`admit`]): once a tenant has any provisioned user, an OIDC
//! principal of that tenant is served only when its id is the `userName` of an **active**
//! user, and its role is the highest role its groups grant (`kshana-admin`,
//! `kshana-submitter`, `kshana-reader`; no such group: `reader`). A user deactivated or
//! deleted in the identity provider is refused at its next request, whatever its token
//! says. API keys are not people and are not affected.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::auth::{is_name, Principal, Role, Source, ALL_TENANTS};
use super::HttpResponse;

/// The Enterprise feature that serves these routes.
pub const FEATURE: &str = "pro-scim";
const USER_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:User";
const GROUP_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:Group";
const LIST_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
const ERROR_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:Error";
const PATCH_SCHEMA: &str = "urn:ietf:params:scim:api:messages:2.0:PatchOp";
const MEDIA: &str = "application/scim+json";

/// The group names that grant a role.
pub const ROLE_GROUPS: &[(&str, Role)] = &[
    ("kshana-admin", Role::Admin),
    ("kshana-submitter", Role::Submitter),
    ("kshana-reader", Role::Reader),
];

fn dir(data_dir: &Path, tenant: &str, what: &str) -> PathBuf {
    data_dir.join("scim").join(tenant).join(what)
}

fn scim(status: u16, v: &Value) -> HttpResponse {
    HttpResponse {
        status,
        content_type: MEDIA,
        body: serde_json::to_string_pretty(v).unwrap_or_default() + "\n",
        headers: Vec::new(),
    }
}

/// A SCIM error response.
pub fn error(status: u16, detail: &str) -> HttpResponse {
    scim(
        status,
        &json!({"schemas": [ERROR_SCHEMA], "status": status.to_string(), "detail": detail}),
    )
}

fn new_id(seed: &str, now: i64) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update(now.to_le_bytes());
    let nonce: [u8; 16] = rand::random();
    h.update(nonce);
    hex::encode(&h.finalize()[..16])
}

fn read_all(d: &Path) -> Vec<Value> {
    let mut out: Vec<Value> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|b| serde_json::from_slice(&b).ok())
        .collect();
    out.sort_by(|a: &Value, b: &Value| a["id"].as_str().cmp(&b["id"].as_str()));
    out
}

fn read_one(d: &Path, id: &str) -> Option<Value> {
    if !is_name(id) {
        return None;
    }
    serde_json::from_slice(&std::fs::read(d.join(format!("{id}.json"))).ok()?).ok()
}

fn write_one(d: &Path, v: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(d)?;
    let id = v["id"].as_str().unwrap_or_default();
    super::jobs::write_atomic(
        &d.join(format!("{id}.json")),
        (serde_json::to_string_pretty(v).unwrap_or_default() + "\n").as_bytes(),
    )
}

fn stamp(now: i64) -> String {
    super::jobs::utc_stamp(now)
}

/// The provisioned users of `tenant`.
pub fn users(data_dir: &Path, tenant: &str) -> Vec<Value> {
    read_all(&dir(data_dir, tenant, "users"))
}

/// The groups of `tenant`.
pub fn groups(data_dir: &Path, tenant: &str) -> Vec<Value> {
    read_all(&dir(data_dir, tenant, "groups"))
}

/// The role `user_id`'s groups grant, if any.
fn role_of(groups: &[Value], user_id: &str) -> Option<Role> {
    let mut best: Option<Role> = None;
    for g in groups {
        let Some(role) = ROLE_GROUPS
            .iter()
            .find(|(n, _)| g["displayName"].as_str() == Some(n))
            .map(|(_, r)| *r)
        else {
            continue;
        };
        let member = g["members"]
            .as_array()
            .is_some_and(|m| m.iter().any(|x| x["value"].as_str() == Some(user_id)));
        if member && best.is_none_or(|b| role > b) {
            best = Some(role);
        }
    }
    best
}

/// The principal a request is served as, given the provisioned directory of its tenant
/// (see the module text). `Err` carries the reason for a 401.
pub fn admit(data_dir: &Path, p: Principal) -> Result<Principal, String> {
    if p.source != Source::Oidc || p.tenant == ALL_TENANTS || !is_name(&p.tenant) {
        return Ok(p);
    }
    let all = users(data_dir, &p.tenant);
    if all.is_empty() {
        return Ok(p);
    }
    let Some(u) = all.iter().find(|u| u["userName"].as_str() == Some(&p.id)) else {
        return Err("this user is not provisioned for the tenant".into());
    };
    if u["active"] != true {
        return Err("this user is deactivated".into());
    }
    let id = u["id"].as_str().unwrap_or_default();
    let role = role_of(&groups(data_dir, &p.tenant), id).unwrap_or(Role::Reader);
    Ok(Principal { role, ..p })
}

fn user_doc(id: &str, body: &Value, created: &str, now: i64) -> Result<Value, String> {
    let name = body["userName"]
        .as_str()
        .filter(|n| !n.trim().is_empty() && n.len() <= 256)
        .ok_or("userName is required (1 to 256 characters)")?;
    let mut u = json!({
        "schemas": [USER_SCHEMA],
        "id": id,
        "userName": name,
        "active": body.get("active").and_then(Value::as_bool).unwrap_or(true),
        "meta": {"resourceType": "User", "created": created, "lastModified": stamp(now),
                 "location": format!("/scim/v2/Users/{id}")},
    });
    for k in ["displayName", "externalId", "emails", "name"] {
        if let Some(v) = body.get(k) {
            u[k] = v.clone();
        }
    }
    Ok(u)
}

fn group_doc(id: &str, body: &Value, created: &str, now: i64) -> Result<Value, String> {
    let name = body["displayName"]
        .as_str()
        .filter(|n| !n.trim().is_empty() && n.len() <= 256)
        .ok_or("displayName is required (1 to 256 characters)")?;
    let members: Vec<Value> = body["members"]
        .as_array()
        .map(|m| {
            m.iter()
                .filter_map(|x| x["value"].as_str())
                .map(|v| json!({"value": v}))
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "schemas": [GROUP_SCHEMA],
        "id": id,
        "displayName": name,
        "members": members,
        "meta": {"resourceType": "Group", "created": created, "lastModified": stamp(now),
                 "location": format!("/scim/v2/Groups/{id}")},
    }))
}

fn list(items: Vec<Value>) -> HttpResponse {
    scim(
        200,
        &json!({"schemas": [LIST_SCHEMA], "totalResults": items.len(), "startIndex": 1,
                "itemsPerPage": items.len(), "Resources": items}),
    )
}

/// `userName eq "x"` → `x`.
fn filter_user_name(query: &str) -> Result<Option<String>, String> {
    for (k, v) in query.split('&').filter_map(|kv| kv.split_once('=')) {
        if k != "filter" {
            continue;
        }
        let v = percent_decode(v);
        let rest = v
            .trim()
            .strip_prefix("userName eq ")
            .ok_or("only the filter userName eq \"<name>\" is supported")?;
        return Ok(Some(rest.trim().trim_matches('"').to_string()));
    }
    Ok(None)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(x) => {
                    out.push(x);
                    i += 2;
                }
                Err(_) => out.push(b'%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Apply one PATCH request's operations to `doc` (a user or a group).
fn patch(doc: &mut Value, body: &Value, group: bool) -> Result<(), String> {
    if !body["schemas"]
        .as_array()
        .is_some_and(|s| s.iter().any(|x| x == PATCH_SCHEMA))
    {
        return Err(format!("a PATCH body names the schema {PATCH_SCHEMA}"));
    }
    let ops = body["Operations"]
        .as_array()
        .ok_or("Operations is required")?;
    for op in ops {
        let kind = op["op"].as_str().unwrap_or("").to_ascii_lowercase();
        let path = op["path"].as_str().unwrap_or("");
        let value = &op["value"];
        match (kind.as_str(), path, group) {
            ("replace", "", _) => {
                let obj = value
                    .as_object()
                    .ok_or("a replace without a path takes an object")?;
                for (k, v) in obj {
                    let allowed = if group {
                        ["displayName"].contains(&k.as_str())
                    } else {
                        ["active", "displayName", "userName"].contains(&k.as_str())
                    };
                    if !allowed {
                        return Err(format!("{k} cannot be replaced"));
                    }
                    doc[k] = v.clone();
                }
            }
            ("replace", "active", false) => {
                doc["active"] = json!(value
                    .as_bool()
                    .or_else(|| value.as_str().map(|s| s.eq_ignore_ascii_case("true")))
                    .ok_or("active is true or false")?)
            }
            ("replace", "displayName" | "userName", _) => {
                doc[path] = json!(value.as_str().ok_or("a text value")?)
            }
            ("add", "members", true) => {
                let mut m = doc["members"].as_array().cloned().unwrap_or_default();
                for v in value.as_array().ok_or("members to add are a list")? {
                    let Some(id) = v["value"].as_str() else {
                        continue;
                    };
                    if !m.iter().any(|x| x["value"] == id) {
                        m.push(json!({"value": id}));
                    }
                }
                doc["members"] = json!(m);
            }
            ("remove", p, true) if p == "members" || p.starts_with("members[") => {
                let gone: Vec<String> = if let Some(inner) = p
                    .strip_prefix("members[value eq ")
                    .and_then(|r| r.strip_suffix(']'))
                {
                    vec![inner.trim_matches('"').to_string()]
                } else if let Some(a) = value.as_array() {
                    a.iter()
                        .filter_map(|v| v["value"].as_str().map(str::to_string))
                        .collect()
                } else {
                    // `remove members` without a value removes every member (RFC 7644 3.5.2.2).
                    doc["members"]
                        .as_array()
                        .map(|m| {
                            m.iter()
                                .filter_map(|v| v["value"].as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let m: Vec<Value> = doc["members"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|x| !gone.iter().any(|g| x["value"] == g.as_str()))
                    .collect();
                doc["members"] = json!(m);
            }
            _ => return Err(format!("unsupported operation {kind} {path}")),
        }
    }
    Ok(())
}

/// Serve one SCIM request. `rest` is the path after `/scim/v2` (`/Users`, `/Users/<id>`, ...).
pub fn handle(
    data_dir: &Path,
    who: &Principal,
    method: &str,
    rest: &str,
    query: &str,
    body: &str,
    now: i64,
) -> HttpResponse {
    if who.tenant == ALL_TENANTS || !is_name(&who.tenant) {
        return error(
            403,
            "SCIM provisions one tenant: use an admin credential of that tenant",
        );
    }
    let parsed = || -> Result<Value, HttpResponse> {
        serde_json::from_str(body).map_err(|e| error(400, &format!("malformed JSON: {e}")))
    };
    let segs: Vec<&str> = rest.trim_start_matches('/').split('/').collect();
    let t = who.tenant.as_str();
    match (method, segs.as_slice()) {
        ("GET", ["ServiceProviderConfig"]) => scim(
            200,
            &json!({
                "schemas": ["urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig"],
                "patch": {"supported": true}, "bulk": {"supported": false, "maxOperations": 0, "maxPayloadSize": 0},
                "filter": {"supported": true, "maxResults": 1000}, "changePassword": {"supported": false},
                "sort": {"supported": false}, "etag": {"supported": false},
                "authenticationSchemes": [{"type": "oauthbearertoken", "name": "Bearer token",
                    "description": "An admin API key or OIDC token of the tenant"}],
            }),
        ),
        ("GET", ["Users"]) => match filter_user_name(query) {
            Ok(f) => list(
                users(data_dir, t)
                    .into_iter()
                    .filter(|u| f.as_deref().is_none_or(|n| u["userName"] == n))
                    .collect(),
            ),
            Err(e) => error(400, &e),
        },
        ("GET", ["Groups"]) => list(groups(data_dir, t)),
        ("POST", ["Users"]) => {
            let b = match parsed() {
                Ok(b) => b,
                Err(r) => return r,
            };
            let name = b["userName"].as_str().unwrap_or_default();
            if users(data_dir, t).iter().any(|u| u["userName"] == name) {
                return error(409, "a user with this userName exists");
            }
            let id = new_id(&format!("{t}/{name}"), now);
            match user_doc(&id, &b, &stamp(now), now) {
                Ok(u) => match write_one(&dir(data_dir, t, "users"), &u) {
                    Ok(()) => scim(201, &u),
                    Err(e) => error(500, &e.to_string()),
                },
                Err(e) => error(400, &e),
            }
        }
        ("POST", ["Groups"]) => {
            let b = match parsed() {
                Ok(b) => b,
                Err(r) => return r,
            };
            let id = new_id(&format!("{t}/g/{}", b["displayName"]), now);
            match group_doc(&id, &b, &stamp(now), now) {
                Ok(g) => match write_one(&dir(data_dir, t, "groups"), &g) {
                    Ok(()) => scim(201, &g),
                    Err(e) => error(500, &e.to_string()),
                },
                Err(e) => error(400, &e),
            }
        }
        (m, [kind @ ("Users" | "Groups"), id]) => {
            let group = *kind == "Groups";
            let d = dir(data_dir, t, if group { "groups" } else { "users" });
            let Some(mut doc) = read_one(&d, id) else {
                return error(404, "no such resource in this tenant");
            };
            match m {
                "GET" => scim(200, &doc),
                "DELETE" => match std::fs::remove_file(d.join(format!("{id}.json"))) {
                    Ok(()) => HttpResponse {
                        status: 204,
                        content_type: MEDIA,
                        body: String::new(),
                        headers: Vec::new(),
                    },
                    Err(e) => error(500, &e.to_string()),
                },
                "PUT" if !group => {
                    let b = match parsed() {
                        Ok(b) => b,
                        Err(r) => return r,
                    };
                    let created = doc["meta"]["created"].as_str().unwrap_or("").to_string();
                    match user_doc(id, &b, &created, now) {
                        Ok(u) => match write_one(&d, &u) {
                            Ok(()) => scim(200, &u),
                            Err(e) => error(500, &e.to_string()),
                        },
                        Err(e) => error(400, &e),
                    }
                }
                "PATCH" => {
                    let b = match parsed() {
                        Ok(b) => b,
                        Err(r) => return r,
                    };
                    if let Err(e) = patch(&mut doc, &b, group) {
                        return error(400, &e);
                    }
                    doc["meta"]["lastModified"] = json!(stamp(now));
                    match write_one(&d, &doc) {
                        Ok(()) => scim(200, &doc),
                        Err(e) => error(500, &e.to_string()),
                    }
                }
                _ => error(405, "method not allowed on this resource"),
            }
        }
        _ => error(404, "no such SCIM endpoint"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_user_name_filter() {
        assert_eq!(
            filter_user_name("filter=userName%20eq%20%22ana%40example.org%22").unwrap(),
            Some("ana@example.org".into())
        );
        assert_eq!(filter_user_name("count=10").unwrap(), None);
        assert!(filter_user_name("filter=emails%20co%20%22x%22").is_err());
    }
}
