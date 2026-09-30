// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The programme file: what a mission dossier is built from.
//!
//! ```toml
//! title = "Navigation resilience programme"
//! reference = "KP-MD-0001"
//! issue = "1"
//! classification = "Unclassified"
//! requirements = "requirements.csv"      # .csv, .reqif / .xml, or .sysml
//!
//! [[scenario]]
//! id = "RUN-LUNAR"
//! path = "lunar-relay-constellation.toml"
//! description = "Lunar relay constellation, one day"          # optional
//! figures = ["global.availability_pct"]                       # optional
//!
//! [[scenario.chart]]                                          # optional, repeatable
//! title = "Dilution of precision percentiles"
//! x = "timeline.t_s"
//! y = ["timeline.channels.time_error_ns.values"]
//!
//! [[link]]                                # optional: a requirement's own runs also link
//! requirement = "LUN-001"
//! runs = ["RUN-LUNAR"]
//!
//! [trade]                                 # optional: one `pro-trade-study` scenario
//! path = "trade.toml"
//!
//! [[sign_off]]
//! role = "Prepared by"
//! name = ""
//! ```
//!
//! Paths are relative to the programme file. Every file is read once, as bytes, and
//! those exact bytes are what is hashed, run and copied into the dossier.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::mbse::evidence::{sha256_hex, SourceFile};
use crate::mbse::requirements::Requirement;
use crate::mbse::trace::{import_requirements, SignOff};
use kshana::api::KshanaError;

/// The run identifier the trade study gets in the trace and the dossier.
pub const TRADE_RUN_ID: &str = "TRADE";

/// File names the build writes at the top of the dossier directory; the copy of the
/// requirements file may not take one of them.
pub const RESERVED_NAMES: [&str; 8] = [
    "trace.toml",
    "dossier.json",
    "dossier.html",
    "dossier.pdf",
    "evidence-pack.html",
    "evidence-pack.json",
    "manifest.json",
    "ledger.json",
];

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgrammeFile {
    title: String,
    #[serde(default)]
    reference: Option<String>,
    #[serde(default)]
    issue: Option<String>,
    #[serde(default)]
    classification: Option<String>,
    requirements: String,
    #[serde(rename = "scenario", default)]
    scenarios: Vec<ScenarioSpec>,
    #[serde(rename = "link", default)]
    links: Vec<LinkSpec>,
    #[serde(default)]
    trade: Option<TradeSpec>,
    #[serde(default)]
    sign_off: Vec<SignOff>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioSpec {
    id: String,
    path: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    figures: Vec<String>,
    #[serde(rename = "chart", default)]
    charts: Vec<ChartSpec>,
}

/// One chart of a run's sheet: the `y` columns of the result against its `x` column.
/// When `x` is a column of names the chart is drawn as bars, otherwise as lines.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChartSpec {
    pub title: String,
    pub x: String,
    pub y: Vec<String>,
}

/// A requirement-to-run link stated in the programme file.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinkSpec {
    pub requirement: String,
    pub runs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TradeSpec {
    path: String,
    #[serde(default)]
    description: Option<String>,
}

/// One scenario of the programme, with the exact bytes of its file.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: String,
    pub description: Option<String>,
    /// The file name without its directory, as it is copied into the dossier.
    pub file_name: String,
    /// The file name without `.toml`.
    pub stem: String,
    /// The path as the programme file spells it.
    pub path_display: String,
    pub bytes: Vec<u8>,
    pub src: String,
    pub sha256: String,
    /// The scenario's `kind` (the engine's default kind, `clock`, when it states none).
    pub kind: String,
    /// Figures the programme names for the sheet's headline; empty means every
    /// computed scalar.
    pub figures: Vec<String>,
    pub charts: Vec<ChartSpec>,
    /// True for the `[trade]` scenario.
    pub is_trade: bool,
}

impl Scenario {
    pub fn is_pro_kind(&self) -> bool {
        self.kind.starts_with(crate::registry::PRO_KIND_PREFIX)
    }
}

/// A loaded programme.
#[derive(Debug, Clone)]
pub struct Programme {
    pub title: String,
    pub reference: String,
    pub issue: String,
    pub classification: String,
    /// The programme file: its name and the SHA-256 of its bytes.
    pub file: SourceFile,
    pub requirements: Vec<Requirement>,
    /// The requirements file: its name and the SHA-256 of its bytes.
    pub requirements_file: SourceFile,
    pub requirements_bytes: Vec<u8>,
    /// Every scenario in programme order; the trade study, when present, is last.
    pub scenarios: Vec<Scenario>,
    pub links: Vec<LinkSpec>,
    pub sign_off: Vec<SignOff>,
    /// SHA-256 over the programme hash, the requirements hash and every scenario hash:
    /// one value that changes when any input changes.
    pub request_sha256: String,
}

fn file_name(p: &str) -> Result<String, KshanaError> {
    Path::new(p)
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| err(format!("'{p}' is not a file path")))
}

/// A run identifier becomes a directory name, so it is restricted to letters, digits,
/// `-`, `_` and `.`, and may not start with a dot.
fn check_id(id: &str) -> Result<(), KshanaError> {
    let ok = !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(err(format!(
            "scenario id '{id}' is not usable as a directory name: use letters, digits, '-', '_' and '.'"
        )))
    }
}

/// The first value of a scenario that is not a finite number, as `key = value`.
///
/// TOML allows `nan` and `inf`, and the engine takes them: a threshold of `nan` is never
/// crossed, so the run reports a figure no real threshold would give, and the result
/// echoes the input as `null`, which no sheet shows. A verdict read from such a run is
/// not evidence, so the programme is refused before anything runs. A scenario text
/// embedded in a string named `scenario` (a variant of a trade study) is checked too.
fn non_finite_value(v: &toml::Value, path: &str) -> Option<String> {
    match v {
        toml::Value::Float(x) if !x.is_finite() => Some(format!("{path} = {x}")),
        toml::Value::Array(a) => a
            .iter()
            .enumerate()
            .find_map(|(i, e)| non_finite_value(e, &format!("{path}[{i}]"))),
        toml::Value::Table(t) => t.iter().find_map(|(k, e)| {
            let p = if path.is_empty() {
                k.clone()
            } else {
                format!("{path}.{k}")
            };
            match e {
                toml::Value::String(s) if k == "scenario" => toml::from_str::<toml::Value>(s)
                    .ok()
                    .and_then(|inner| non_finite_value(&inner, ""))
                    .map(|found| format!("{p}: {found}")),
                _ => non_finite_value(e, &p),
            }
        }),
        _ => None,
    }
}

fn read(path: &Path, what: &str) -> Result<Vec<u8>, KshanaError> {
    std::fs::read(path).map_err(|e| err(format!("could not read {what} {}: {e}", path.display())))
}

fn load_scenario(
    base: &Path,
    id: &str,
    path: &str,
    description: Option<String>,
    figures: Vec<String>,
    charts: Vec<ChartSpec>,
    is_trade: bool,
) -> Result<Scenario, KshanaError> {
    check_id(id)?;
    let file_name = file_name(path)?;
    let Some(stem) = file_name.strip_suffix(".toml").map(str::to_string) else {
        return Err(err(format!(
            "scenario '{id}': '{path}' is not a .toml scenario file"
        )));
    };
    if stem.is_empty() {
        return Err(err(format!("scenario '{id}': '{path}' has no file name")));
    }
    let bytes = read(&base.join(path), &format!("the scenario file of '{id}',"))?;
    let src = String::from_utf8(bytes.clone())
        .map_err(|_| err(format!("scenario '{id}': {path} is not UTF-8")))?;
    let parsed = toml::from_str::<toml::Value>(&src)
        .map_err(|e| err(format!("scenario '{id}': {path} is not TOML: {e}")))?;
    if let Some(found) = non_finite_value(&parsed, "") {
        return Err(err(format!(
            "scenario '{id}': {path} holds a value that is not a finite number ({found}); a run on such an input is not evidence, so nothing was run"
        )));
    }
    let kind = crate::registry::peek_kind(&src).unwrap_or_else(|| "clock".to_string());
    for c in &charts {
        if c.y.is_empty() {
            return Err(err(format!(
                "scenario '{id}': chart '{}' names no y path",
                c.title
            )));
        }
    }
    Ok(Scenario {
        id: id.to_string(),
        description,
        file_name,
        stem,
        path_display: path.to_string(),
        sha256: sha256_hex(&bytes),
        bytes,
        src,
        kind,
        figures,
        charts,
        is_trade,
    })
}

impl Programme {
    /// Load a programme file and every file it names. Nothing is run.
    pub fn load(path: &Path) -> Result<Programme, KshanaError> {
        let bytes = read(path, "the programme file")?;
        let src = String::from_utf8(bytes.clone())
            .map_err(|_| err(format!("{} is not UTF-8", path.display())))?;
        let pf: ProgrammeFile = toml::from_str(&src)
            .map_err(|e| err(format!("{}: invalid programme file: {e}", path.display())))?;
        let base: PathBuf = path.parent().map(Path::to_path_buf).unwrap_or_default();

        if pf.title.trim().is_empty() {
            return Err(err("the programme has an empty title"));
        }
        if pf.scenarios.is_empty() {
            return Err(err("the programme names no [[scenario]]"));
        }

        let req_path = base.join(&pf.requirements);
        let requirements_bytes = read(&req_path, "the requirements file")?;
        let requirements = import_requirements(&req_path).map_err(|e| {
            err(format!(
                "requirements file {}: {e}",
                req_path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default()
            ))
        })?;
        if requirements.is_empty() {
            return Err(err(format!(
                "requirements file {} holds no requirements",
                pf.requirements
            )));
        }

        let mut scenarios: Vec<Scenario> = Vec::new();
        for s in pf.scenarios {
            // A run identifier becomes a directory name, and a file system that ignores
            // letter case (the default on macOS and Windows) holds `RUN-A` and `run-a`
            // in one directory: the second run would overwrite the first one's files and
            // a verdict would be read from the wrong run. So identifiers must differ in
            // more than case, from each other and from the trade study's.
            if s.id.eq_ignore_ascii_case(TRADE_RUN_ID) {
                return Err(err(format!(
                    "scenario id '{}' is reserved for the [trade] study ('{TRADE_RUN_ID}', in any letter case)",
                    s.id
                )));
            }
            if let Some(x) = scenarios.iter().find(|x| x.id.eq_ignore_ascii_case(&s.id)) {
                return Err(err(if x.id == s.id {
                    format!("scenario id '{}' appears twice", s.id)
                } else {
                    format!(
                        "scenario ids '{}' and '{}' differ only in letter case; each becomes a directory name, and on a file system that ignores case one run would overwrite the other",
                        x.id, s.id
                    )
                }));
            }
            scenarios.push(load_scenario(
                &base,
                &s.id,
                &s.path,
                s.description,
                s.figures,
                s.charts,
                false,
            )?);
        }
        if let Some(t) = pf.trade {
            let sc = load_scenario(
                &base,
                TRADE_RUN_ID,
                &t.path,
                t.description,
                vec![],
                vec![],
                true,
            )?;
            if sc.kind != "pro-trade-study" {
                return Err(err(format!(
                    "[trade] {} is of kind '{}'; it must be a pro-trade-study scenario",
                    t.path, sc.kind
                )));
            }
            scenarios.push(sc);
        }

        // Every link must name a requirement and a run that exist, before anything runs.
        let known_run = |r: &str| scenarios.iter().any(|s| s.id == r);
        for r in &requirements {
            for run in &r.runs {
                if !known_run(run) {
                    return Err(err(format!(
                        "requirement '{}' is linked to run '{run}', which the programme does not define",
                        r.id
                    )));
                }
            }
        }
        for l in &pf.links {
            if !requirements.iter().any(|r| r.id == l.requirement) {
                return Err(err(format!(
                    "[[link]] names requirement '{}', which is not in {}",
                    l.requirement, pf.requirements
                )));
            }
            for run in &l.runs {
                if !known_run(run) {
                    return Err(err(format!(
                        "requirement '{}' is linked to run '{run}', which the programme does not define",
                        l.requirement
                    )));
                }
            }
        }

        let file = SourceFile {
            path: file_name(&path.to_string_lossy())?,
            sha256: sha256_hex(&bytes),
        };
        let requirements_file = SourceFile {
            path: file_name(&pf.requirements)?,
            sha256: sha256_hex(&requirements_bytes),
        };
        if RESERVED_NAMES.contains(&requirements_file.path.as_str()) {
            return Err(err(format!(
                "the requirements file may not be named {}: the dossier writes a file of that name",
                requirements_file.path
            )));
        }
        let mut canon = format!(
            "programme {}\nrequirements {}\n",
            file.sha256, requirements_file.sha256
        );
        for s in &scenarios {
            canon.push_str(&format!("scenario {} {}\n", s.id, s.sha256));
        }
        Ok(Programme {
            title: pf.title,
            reference: pf.reference.unwrap_or_default(),
            issue: pf.issue.unwrap_or_else(|| "1".to_string()),
            classification: pf
                .classification
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| crate::report::DEFAULT_CLASSIFICATION.to_string()),
            file,
            requirements,
            requirements_file,
            requirements_bytes,
            scenarios,
            links: pf.links,
            sign_off: pf.sign_off,
            request_sha256: sha256_hex(canon.as_bytes()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_ids_that_are_not_directory_names_are_refused() {
        assert!(check_id("RUN-LEO_1.a").is_ok());
        for bad in ["", "..", ".hidden", "a/b", "a b", "a\\b"] {
            assert!(check_id(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn a_value_that_is_not_a_finite_number_is_found_and_named() {
        let find = |src: &str| non_finite_value(&toml::from_str::<toml::Value>(src).unwrap(), "");
        assert_eq!(find("a = 1.0\nb = \"nan\"\n[c]\nd = [1.0, 2.0]\n"), None);
        assert_eq!(
            find("threshold_ns = nan\n").as_deref(),
            Some("threshold_ns = NaN")
        );
        assert_eq!(
            find("[c]\nd = [1.0, -inf]\n").as_deref(),
            Some("c.d[1] = -inf")
        );
        assert_eq!(
            find("[[shell]]\nalt_km = 1.0\n[[shell]]\nalt_km = inf\n").as_deref(),
            Some("shell[1].alt_km = inf")
        );
        // A scenario embedded in a trade variant is read too; another string is not.
        assert_eq!(
            find("[[variant]]\nlabel = \"x\"\nscenario = \"\"\"\nmask_deg = nan\n\"\"\"\n")
                .as_deref(),
            Some("variant[0].scenario: mask_deg = NaN")
        );
        assert_eq!(find("note = \"\"\"\nmask_deg = nan\n\"\"\"\n"), None);
    }
}
