// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Deployment integrity attestation and configuration drift (ground software)
//!
//! A ground segment runs software on many hosts, and a resilience case needs evidence that
//! what runs is what was released, or a record of when and where it stopped being so. This
//! module gives that evidence from hashes, and nothing more:
//!
//! * **baseline** ([`baseline_of_dir`], [`baseline_of_list`]): the SHA-256 of every file of a
//!   released deployment (a directory, or a `sha256sum`-style list), written as a manifest
//!   (`kshana-pro-deploy-baseline/1`) and signed as a pack;
//! * **check** ([`check`]): hash snapshots of deployed hosts, taken over time by the operator's
//!   own tooling (`sha256sum` output, or CSV `path,sha256`), compared with the **verified**
//!   baseline: per snapshot the files that match, are modified, missing or added, and the
//!   changes accepted by a stated change reference; per host a **drift timeline** (when each
//!   file first left the baseline, whether and when it came back); a signed report.
//!
//! The baseline's signature must verify against a trusted key or the check stops. Excluded
//! paths (logs, run files) are skipped on both sides.
//!
//! **Evidence only.** Nothing here connects to a host, scans a network, blocks, repairs or
//! detects intrusions: it compares hash lists the operator supplies. **Evidence label:
//! MODELLED**: a comparison of the supplied hash lists; whether a deployment is acceptable is
//! for its operator to decide.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{parse_utc, sha256_hex};
use crate::license::KEY_LEN;

/// Pro feature.
pub const FEATURE: &str = "pro-deploy-attestation";
/// Formats.
pub const BASELINE_FORMAT: &str = "kshana-pro-deploy-baseline/1";
pub const FORMAT: &str = "kshana-pro-deploy-attestation/1";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED";
/// The manifest file name inside a baseline pack.
pub const MANIFEST: &str = "manifest.json";

/// One file of a deployment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileHash {
    pub path: String,
    pub sha256: String,
}

/// A baseline manifest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Baseline {
    pub format: String,
    pub deployment: String,
    pub release: String,
    pub created_utc: String,
    pub source: String,
    pub exclude: Vec<String>,
    pub files: Vec<FileHash>,
}

/// One snapshot of a host in the request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRef {
    pub host: String,
    pub utc: String,
    pub file: String,
}

/// A change accepted against the baseline (by a stated change reference).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Accepted {
    pub path: String,
    pub sha256: String,
    pub reference: String,
    /// Only on this host; every host when absent.
    #[serde(default)]
    pub host: Option<String>,
}

/// The check request (`request.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// The signed baseline pack directory.
    pub baseline: String,
    /// Paths skipped on both sides, added to the baseline's own (`*` matches any run).
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(rename = "snapshot")]
    pub snapshots: Vec<SnapshotRef>,
    #[serde(default)]
    pub accepted: Vec<Accepted>,
}

/// One finding of one snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub path: String,
    /// `modified`, `missing`, `added` or `accepted`.
    pub kind: String,
    pub baseline_sha256: Option<String>,
    pub deployed_sha256: Option<String>,
    pub reference: Option<String>,
}

/// One snapshot compared with the baseline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotResult {
    pub host: String,
    pub utc: String,
    pub sha256: String,
    pub files: usize,
    pub matched: usize,
    pub modified: usize,
    pub missing: usize,
    pub added: usize,
    pub accepted: usize,
    /// `matches baseline` or `drifted`.
    pub state: String,
    pub findings: Vec<Finding>,
    /// Paths that newly left the baseline since this host's previous snapshot.
    pub new_drift: Vec<String>,
    /// Paths that came back to the baseline since this host's previous snapshot.
    pub reverted: Vec<String>,
}

/// One drift episode of one file on one host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub host: String,
    pub path: String,
    pub kind: String,
    pub first_seen_utc: String,
    pub last_seen_utc: String,
    /// The first later snapshot where the file was back at the baseline.
    pub reverted_utc: Option<String>,
    /// Open at the host's latest snapshot.
    pub open: bool,
    /// From first seen to reverted, or to the host's latest snapshot when open (days).
    pub duration_days: f64,
}

/// Per host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostRow {
    pub host: String,
    pub snapshots: usize,
    pub latest_utc: String,
    pub latest_state: String,
    pub open_findings: usize,
    pub first_drift_utc: Option<String>,
}

/// The report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub format: String,
    pub title: String,
    pub evidence: String,
    pub verdict: String,
    pub deployment: String,
    pub release: String,
    pub baseline_files: usize,
    pub baseline_manifest_sha256: String,
    pub baseline_signature: String,
    pub exclude: Vec<String>,
    pub hosts: Vec<HostRow>,
    pub snapshots: Vec<SnapshotResult>,
    pub episodes: Vec<Episode>,
    pub caveats: Vec<String>,
}

// ---------------------------------------------------------------------------------------

/// `*` matches any run of characters; everything else literally.
pub fn glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == s;
    }
    let mut pos = 0;
    for (i, p) in parts.iter().enumerate() {
        if i == 0 {
            if !s.starts_with(p) {
                return false;
            }
            pos = p.len();
        } else if i == parts.len() - 1 {
            return s.len() >= pos + p.len() && s[pos..].ends_with(p);
        } else if let Some(k) = s[pos..].find(p) {
            pos += k + p.len();
        } else {
            return false;
        }
    }
    true
}

fn norm(p: &str) -> String {
    let p = p.trim().replace('\\', "/");
    p.trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Parse a hash list: `sha256sum` output (`<hex>  <path>` or `<hex> *<path>`) or CSV with a
/// `path,sha256` header. Blank lines and `#` comments are skipped.
pub fn parse_hash_list(text: &str) -> Result<Vec<FileHash>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end_matches('\r')))
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .peekable();
    let mut out = Vec::new();
    let csv = lines
        .peek()
        .map(|(_, l)| {
            l.to_ascii_lowercase()
                .replace(' ', "")
                .starts_with("path,sha256")
        })
        .unwrap_or(false);
    if csv {
        lines.next();
    }
    for (n, l) in lines {
        let (h, p) = if csv {
            let (p, h) = l
                .split_once(',')
                .ok_or_else(|| format!("line {n}: expected path,sha256"))?;
            (h.trim().to_string(), p.to_string())
        } else {
            let (h, rest) = l
                .split_once(' ')
                .ok_or_else(|| format!("line {n}: expected '<sha256>  <path>'"))?;
            let rest = rest
                .strip_prefix(' ')
                .or_else(|| rest.strip_prefix('*'))
                .unwrap_or(rest);
            (h.to_string(), rest.to_string())
        };
        let h = h.to_ascii_lowercase();
        if !hex64(&h) {
            return Err(format!("line {n}: '{h}' is not a SHA-256"));
        }
        let p = norm(&p);
        if p.is_empty() {
            return Err(format!("line {n}: empty path"));
        }
        out.push(FileHash { path: p, sha256: h });
    }
    out.sort();
    for w in out.windows(2) {
        if w[0].path == w[1].path {
            return Err(format!("'{}' listed twice", w[0].path));
        }
    }
    Ok(out)
}

/// Hash every regular file under `dir` (relative paths with `/`). A symbolic link is recorded
/// by its target (`link:<target>` hashed), not followed.
pub fn hash_dir(dir: &Path) -> Result<Vec<FileHash>, String> {
    fn walk(root: &Path, d: &Path, out: &mut Vec<FileHash>) -> Result<(), String> {
        let rd = std::fs::read_dir(d).map_err(|e| format!("{}: {e}", d.display()))?;
        for ent in rd {
            let ent = ent.map_err(|e| e.to_string())?;
            let p = ent.path();
            let ft = ent.file_type().map_err(|e| e.to_string())?;
            let rel = norm(&p.strip_prefix(root).unwrap_or(&p).to_string_lossy());
            if ft.is_symlink() {
                let t = std::fs::read_link(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                out.push(FileHash {
                    path: rel,
                    sha256: sha256_hex(format!("link:{}", t.to_string_lossy()).as_bytes()),
                });
            } else if ft.is_dir() {
                walk(root, &p, out)?;
            } else if ft.is_file() {
                let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                out.push(FileHash {
                    path: rel,
                    sha256: sha256_hex(&b),
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn excluded(p: &str, ex: &[String]) -> bool {
    ex.iter().any(|g| glob(g, p))
}

/// A baseline from a hash list.
pub fn baseline_of_list(
    files: Vec<FileHash>,
    deployment: &str,
    release: &str,
    source: &str,
    exclude: &[String],
    utc: &str,
) -> Result<Baseline, String> {
    if deployment.trim().is_empty() {
        return Err("baseline: the deployment needs a name".into());
    }
    let files: Vec<FileHash> = files
        .into_iter()
        .filter(|f| !excluded(&f.path, exclude))
        .collect();
    if files.is_empty() {
        return Err("baseline: no file left after exclusions".into());
    }
    Ok(Baseline {
        format: BASELINE_FORMAT.into(),
        deployment: deployment.into(),
        release: release.into(),
        created_utc: utc.into(),
        source: source.into(),
        exclude: exclude.to_vec(),
        files,
    })
}

/// A baseline from a released deployment directory.
pub fn baseline_of_dir(
    dir: &Path,
    deployment: &str,
    release: &str,
    exclude: &[String],
    utc: &str,
) -> Result<Baseline, String> {
    baseline_of_list(
        hash_dir(dir)?,
        deployment,
        release,
        &format!("directory {}", dir.display()),
        exclude,
        utc,
    )
}

/// Write a baseline pack (manifest and a hash list, to be signed).
pub fn write_baseline(out: &Path, b: &Baseline) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out).map_err(io)?;
    std::fs::write(
        out.join(MANIFEST),
        serde_json::to_string_pretty(b).expect("serialises"),
    )
    .map_err(io)?;
    let list: String = b
        .files
        .iter()
        .map(|f| format!("{}  {}\n", f.sha256, f.path))
        .collect();
    std::fs::write(out.join("baseline.sha256"), list).map_err(io)?;
    Ok(())
}

/// Compare one snapshot with the baseline.
pub fn compare(
    base: &Baseline,
    snap: &[FileHash],
    host: &str,
    accepted: &[Accepted],
    exclude: &[String],
) -> (usize, Vec<Finding>) {
    let b: BTreeMap<&str, &str> = base
        .files
        .iter()
        .filter(|f| !excluded(&f.path, exclude))
        .map(|f| (f.path.as_str(), f.sha256.as_str()))
        .collect();
    let s: BTreeMap<&str, &str> = snap
        .iter()
        .filter(|f| !excluded(&f.path, exclude))
        .map(|f| (f.path.as_str(), f.sha256.as_str()))
        .collect();
    let ok = |p: &str, h: &str| {
        accepted
            .iter()
            .find(|a| {
                a.path == p
                    && a.sha256.eq_ignore_ascii_case(h)
                    && a.host.as_deref().is_none_or(|x| x == host)
            })
            .map(|a| a.reference.clone())
    };
    let mut matched = 0;
    let mut out = Vec::new();
    let paths: BTreeSet<&str> = b.keys().chain(s.keys()).copied().collect();
    for p in paths {
        let f = |kind: &str, r: Option<String>| Finding {
            path: p.into(),
            kind: kind.into(),
            baseline_sha256: b.get(p).map(|h| h.to_string()),
            deployed_sha256: s.get(p).map(|h| h.to_string()),
            reference: r,
        };
        match (b.get(p), s.get(p)) {
            (Some(x), Some(y)) if x == y => matched += 1,
            (Some(_), Some(y)) | (None, Some(y)) => match ok(p, y) {
                Some(r) => out.push(f("accepted", Some(r))),
                None => out.push(f(
                    if b.contains_key(p) {
                        "modified"
                    } else {
                        "added"
                    },
                    None,
                )),
            },
            (Some(_), None) => out.push(f("missing", None)),
            (None, None) => {}
        }
    }
    (matched, out)
}

fn days(a: &str, b: &str) -> f64 {
    match (parse_utc(a), parse_utc(b)) {
        (Some(x), Some(y)) => ((y - x) / 86_400.0).max(0.0),
        _ => 0.0,
    }
}

/// The report from a verified baseline and snapshots already read (`(ref, files, sha256)`).
pub fn report(
    title: &str,
    base: &Baseline,
    manifest_sha: &str,
    signature: &str,
    snaps: &[(SnapshotRef, Vec<FileHash>, String)],
    accepted: &[Accepted],
    extra_exclude: &[String],
) -> Result<Report, String> {
    let mut exclude = base.exclude.clone();
    exclude.extend(extra_exclude.iter().cloned());
    let mut order: Vec<usize> = (0..snaps.len()).collect();
    for (r, _, _) in snaps {
        parse_utc(&r.utc).ok_or_else(|| format!("snapshot {}: bad utc '{}'", r.host, r.utc))?;
    }
    order.sort_by(|&a, &b| {
        let (x, y) = (&snaps[a].0, &snaps[b].0);
        x.host.cmp(&y.host).then(
            parse_utc(&x.utc)
                .partial_cmp(&parse_utc(&y.utc))
                .expect("checked"),
        )
    });
    let mut results: Vec<SnapshotResult> = Vec::new();
    let mut episodes: Vec<Episode> = Vec::new();
    // Open episodes per (host, path): index into `episodes`.
    let mut open: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut prev_host = String::new();
    for &i in &order {
        let (r, files, sha) = &snaps[i];
        if r.host != prev_host {
            open.clear();
            prev_host = r.host.clone();
        }
        let (matched, findings) = compare(base, files, &r.host, accepted, &exclude);
        let drifted: BTreeMap<&str, &Finding> = findings
            .iter()
            .filter(|f| f.kind != "accepted")
            .map(|f| (f.path.as_str(), f))
            .collect();
        let mut new_drift = Vec::new();
        let mut reverted = Vec::new();
        for (p, f) in &drifted {
            let key = (r.host.clone(), p.to_string());
            match open.get(&key) {
                Some(&k) => episodes[k].last_seen_utc = r.utc.clone(),
                None => {
                    new_drift.push(p.to_string());
                    open.insert(key, episodes.len());
                    episodes.push(Episode {
                        host: r.host.clone(),
                        path: p.to_string(),
                        kind: f.kind.clone(),
                        first_seen_utc: r.utc.clone(),
                        last_seen_utc: r.utc.clone(),
                        reverted_utc: None,
                        open: true,
                        duration_days: 0.0,
                    });
                }
            }
        }
        let gone: Vec<(String, String)> = open
            .keys()
            .filter(|(h, p)| *h == r.host && !drifted.contains_key(p.as_str()))
            .cloned()
            .collect();
        for key in gone {
            let k = open.remove(&key).expect("present");
            episodes[k].reverted_utc = Some(r.utc.clone());
            episodes[k].open = false;
            episodes[k].duration_days = days(&episodes[k].first_seen_utc, &r.utc);
            reverted.push(key.1);
        }
        let count = |k: &str| findings.iter().filter(|f| f.kind == k).count();
        results.push(SnapshotResult {
            host: r.host.clone(),
            utc: r.utc.clone(),
            sha256: sha.clone(),
            files: files.len(),
            matched,
            modified: count("modified"),
            missing: count("missing"),
            added: count("added"),
            accepted: count("accepted"),
            state: if drifted.is_empty() {
                "matches baseline".into()
            } else {
                "drifted".into()
            },
            findings,
            new_drift,
            reverted,
        });
    }
    let mut hosts: Vec<HostRow> = Vec::new();
    for s in &results {
        match hosts.last_mut() {
            Some(h) if h.host == s.host => {
                h.snapshots += 1;
                h.latest_utc = s.utc.clone();
                h.latest_state = s.state.clone();
            }
            _ => hosts.push(HostRow {
                host: s.host.clone(),
                snapshots: 1,
                latest_utc: s.utc.clone(),
                latest_state: s.state.clone(),
                open_findings: 0,
                first_drift_utc: None,
            }),
        }
    }
    for h in &mut hosts {
        for e in episodes.iter_mut().filter(|e| e.host == h.host) {
            if e.open {
                e.duration_days = days(&e.first_seen_utc, &h.latest_utc);
                h.open_findings += 1;
            }
            let earlier = h
                .first_drift_utc
                .as_ref()
                .is_none_or(|f| parse_utc(&e.first_seen_utc) < parse_utc(f));
            if earlier {
                h.first_drift_utc = Some(e.first_seen_utc.clone());
            }
        }
    }
    let clean = hosts
        .iter()
        .filter(|h| h.latest_state == "matches baseline")
        .count();
    let open_n = episodes.iter().filter(|e| e.open).count();
    let verdict = format!(
        "{clean} of {} hosts match the signed baseline of {} {} at their latest snapshot; {open_n} drift findings open, {} reverted",
        hosts.len(),
        base.deployment,
        base.release,
        episodes.len() - open_n
    );
    Ok(Report {
        format: FORMAT.into(),
        title: title.into(),
        evidence: EVIDENCE.into(),
        verdict,
        deployment: base.deployment.clone(),
        release: base.release.clone(),
        baseline_files: base.files.len(),
        baseline_manifest_sha256: manifest_sha.into(),
        baseline_signature: signature.into(),
        exclude,
        hosts,
        snapshots: results,
        episodes,
        caveats: vec![
            "A comparison of hash lists supplied by the operator; nothing here connects to a host, scans, blocks or repairs.".into(),
            "A snapshot shows the files as hashed at that time by the operator's tooling; a change between snapshots that was reverted before the next one is not seen.".into(),
            "An accepted change is one the request names with a change reference; whether a change is acceptable is for the operator to decide.".into(),
        ],
    })
}

/// The input files (published name, bytes) of a check, for the pack.
pub type Inputs = Vec<(String, Vec<u8>)>;

fn resolve(base: &Path, p: &str) -> PathBuf {
    if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        base.join(p)
    }
}

/// Run a check: verify the baseline pack against `trust` (required), read every snapshot.
pub fn check(
    q: &Request,
    base: &Path,
    trust: &[[u8; KEY_LEN]],
) -> Result<(Report, Inputs), String> {
    if trust.is_empty() {
        return Err("the baseline's trusted key is needed (--trust)".into());
    }
    if q.snapshots.is_empty() {
        return Err("no [[snapshot]]".into());
    }
    let bdir = resolve(base, &q.baseline);
    let v = crate::signed_dossier::verify(&bdir, trust)
        .map_err(|e| format!("baseline {}: {e}", bdir.display()))?;
    if !v.problems.is_empty() {
        return Err(format!(
            "baseline {} does not verify: {}",
            bdir.display(),
            v.problems.join("; ")
        ));
    }
    let signature = format!(
        "verified (signer {}, key {}, signed {})",
        v.signer, v.key_id, v.utc
    );
    let mb = std::fs::read(bdir.join(MANIFEST))
        .map_err(|e| format!("{}: {e}", bdir.join(MANIFEST).display()))?;
    let b: Baseline = serde_json::from_slice(&mb).map_err(|e| format!("{MANIFEST}: {e}"))?;
    if b.format != BASELINE_FORMAT {
        return Err(format!(
            "{MANIFEST}: format '{}', expected {BASELINE_FORMAT}",
            b.format
        ));
    }
    let mut inputs: Inputs = vec![("baseline-manifest.json".into(), mb.clone())];
    let mut snaps = Vec::new();
    for (i, s) in q.snapshots.iter().enumerate() {
        let p = resolve(base, &s.file);
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let text = String::from_utf8(bytes.clone())
            .map_err(|_| format!("{}: not UTF-8 text", p.display()))?;
        let files = parse_hash_list(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        let name = Path::new(&s.file)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| s.file.clone());
        snaps.push((s.clone(), files, sha256_hex(&bytes)));
        inputs.push((format!("snapshots/{:02}-{name}", i + 1), bytes));
    }
    let r = report(
        &q.title,
        &b,
        &sha256_hex(&mb),
        &signature,
        &snaps,
        &q.accepted,
        &q.exclude,
    )?;
    Ok((r, inputs))
}

/// Write the pack (sign afterwards).
pub fn write(out: &Path, r: &Report, request_src: &str, inputs: &Inputs) -> Result<(), String> {
    crate::transport::write_pack(out, r, &html(r), "request.toml", request_src)?;
    for (name, b) in inputs {
        let p = out.join("inputs").join(name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, b).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

fn short(h: &Option<String>) -> String {
    h.as_deref()
        .map(|h| h[..12.min(h.len())].to_string())
        .unwrap_or_default()
}

/// The evidence page.
pub fn html(r: &Report) -> String {
    let intro = vec![
        format!(
            "Deployment {} release {}: {} files in the baseline (manifest {}), baseline signature {}.",
            r.deployment,
            r.release,
            r.baseline_files,
            &r.baseline_manifest_sha256[..12.min(r.baseline_manifest_sha256.len())],
            r.baseline_signature
        ),
        r.caveats.join(" "),
    ];
    let hosts = r
        .hosts
        .iter()
        .map(|h| {
            vec![
                h.host.clone(),
                h.snapshots.to_string(),
                h.latest_utc.clone(),
                h.latest_state.clone(),
                h.open_findings.to_string(),
                h.first_drift_utc.clone().unwrap_or_default(),
            ]
        })
        .collect();
    let timeline = r
        .snapshots
        .iter()
        .map(|s| {
            vec![
                s.host.clone(),
                s.utc.clone(),
                s.state.clone(),
                format!(
                    "{} / {} / {} / {} / {}",
                    s.matched, s.modified, s.missing, s.added, s.accepted
                ),
                s.new_drift.join(", "),
                s.reverted.join(", "),
            ]
        })
        .collect();
    let eps = r
        .episodes
        .iter()
        .map(|e| {
            vec![
                e.host.clone(),
                e.path.clone(),
                e.kind.clone(),
                e.first_seen_utc.clone(),
                e.reverted_utc.clone().unwrap_or_else(|| "open".into()),
                format!("{:.1}", e.duration_days),
            ]
        })
        .collect();
    let latest: Vec<Vec<String>> = r
        .hosts
        .iter()
        .filter_map(|h| r.snapshots.iter().rev().find(|s| s.host == h.host))
        .flat_map(|s| {
            s.findings.iter().map(move |f| {
                vec![
                    s.host.clone(),
                    f.path.clone(),
                    f.kind.clone(),
                    short(&f.baseline_sha256),
                    short(&f.deployed_sha256),
                    f.reference.clone().unwrap_or_default(),
                ]
            })
        })
        .collect();
    crate::transport::page(
        &r.title,
        &r.evidence,
        &r.verdict,
        &intro,
        &[
            (
                "Hosts".into(),
                vec![
                    "host",
                    "snapshots",
                    "latest",
                    "state",
                    "open findings",
                    "first drift",
                ],
                hosts,
            ),
            (
                "Drift timeline".into(),
                vec![
                    "host",
                    "snapshot",
                    "state",
                    "matched / modified / missing / added / accepted",
                    "new drift",
                    "reverted",
                ],
                timeline,
            ),
            (
                "Drift episodes".into(),
                vec!["host", "path", "kind", "first seen", "reverted", "days"],
                eps,
            ),
            (
                "Findings at the latest snapshot".into(),
                vec![
                    "host",
                    "path",
                    "kind",
                    "baseline",
                    "deployed",
                    "change reference",
                ],
                latest,
            ),
        ],
    )
}

// ---------------------------------------------------------------------------------------

/// SYNTHETIC: a released deployment (files) and three snapshots of two hosts, with the
/// request. Returns `(release files, snapshots (name, text), request.toml)`.
pub type Synth = (Vec<(String, String)>, Vec<(String, String)>, String);

/// See [`Synth`].
pub fn synth() -> Synth {
    let rel: Vec<(String, String)> = vec![
        (
            "bin/mcs-server".into(),
            "SYNTHETIC mission control server 4.2.0\n".into(),
        ),
        (
            "bin/tm-ingest".into(),
            "SYNTHETIC telemetry ingest 4.2.0\n".into(),
        ),
        (
            "etc/mcs.conf".into(),
            "port = 7000\nlog_level = info\n".into(),
        ),
        (
            "etc/stations.conf".into(),
            "station = SYN-GS1\nstation = SYN-GS2\n".into(),
        ),
        (
            "lib/libframe.so".into(),
            "SYNTHETIC frame library 1.9\n".into(),
        ),
        ("logs/README".into(), "logs go here\n".into()),
    ];
    let h = |t: &str| sha256_hex(t.as_bytes());
    let line = |p: &str, t: &str| format!("{}  ./{p}\n", h(t));
    let base_of = |skip: &[&str], over: &[(&str, &str)], extra: &[(&str, &str)]| {
        let mut s = String::new();
        for (p, t) in &rel {
            if skip.contains(&p.as_str()) {
                continue;
            }
            let t = over
                .iter()
                .find(|(q, _)| q == p)
                .map(|(_, t)| *t)
                .unwrap_or(t.as_str());
            s.push_str(&line(p, t));
        }
        for (p, t) in extra {
            s.push_str(&line(p, t));
        }
        s
    };
    let conf2 = "port = 7000\nlog_level = debug\n";
    let snaps = vec![
        (
            "gs-a-2026-11-01.sha256".into(),
            base_of(
                &[],
                &[("logs/README", "rotated\n")],
                &[("logs/mcs.log", "x\n")],
            ),
        ),
        ("gs-b-2026-11-01.sha256".into(), base_of(&[], &[], &[])),
        (
            "gs-a-2026-11-08.sha256".into(),
            base_of(
                &[],
                &[("etc/mcs.conf", conf2)],
                &[("bin/helper.sh", "#!/bin/sh\n")],
            ),
        ),
        (
            "gs-b-2026-11-08.sha256".into(),
            base_of(&["lib/libframe.so"], &[], &[]),
        ),
        (
            "gs-a-2026-11-15.sha256".into(),
            base_of(
                &[],
                &[
                    ("etc/mcs.conf", conf2),
                    ("etc/stations.conf", "station = SYN-GS1\n"),
                ],
                &[],
            ),
        ),
        ("gs-b-2026-11-15.sha256".into(), base_of(&[], &[], &[])),
    ];
    let req = format!(
        "title = \"SYNTHETIC ground segment: deployment drift\"\nbaseline = \"baseline-pack\"\nexclude = [\"logs/*\"]\n\n\
         [[snapshot]]\nhost = \"gs-a\"\nutc = \"2026-11-01T06:00:00Z\"\nfile = \"gs-a-2026-11-01.sha256\"\n\n\
         [[snapshot]]\nhost = \"gs-b\"\nutc = \"2026-11-01T06:00:00Z\"\nfile = \"gs-b-2026-11-01.sha256\"\n\n\
         [[snapshot]]\nhost = \"gs-a\"\nutc = \"2026-11-08T06:00:00Z\"\nfile = \"gs-a-2026-11-08.sha256\"\n\n\
         [[snapshot]]\nhost = \"gs-b\"\nutc = \"2026-11-08T06:00:00Z\"\nfile = \"gs-b-2026-11-08.sha256\"\n\n\
         [[snapshot]]\nhost = \"gs-a\"\nutc = \"2026-11-15T06:00:00Z\"\nfile = \"gs-a-2026-11-15.sha256\"\n\n\
         [[snapshot]]\nhost = \"gs-b\"\nutc = \"2026-11-15T06:00:00Z\"\nfile = \"gs-b-2026-11-15.sha256\"\n\n\
         [[accepted]]\npath = \"etc/stations.conf\"\nsha256 = \"{}\"\nreference = \"SYNTHETIC CR-0042 (GS2 withdrawn)\"\n",
        h("station = SYN-GS1\n")
    );
    (rel, snaps, req)
}
