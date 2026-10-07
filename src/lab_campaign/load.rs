//! Load a campaign output directory and check it before anything is rendered.
//!
//! Checks, in order:
//! 1. `campaign.json` parses and its schema is one this overlay reads;
//! 2. every planned cell's key, chain and design resolve inside the campaign;
//! 3. every `cells/*.json` parses, its schema is known, its file name equals its `key`, its
//!    key re-derives from its own inputs, it is a planned cell, its recording, chain, design,
//!    run/scoring hashes and data class are the plan's, its engine version is the campaign's,
//!    and its lock source is one this overlay can describe;
//! 4. every `modelled` block carries the `MODELLED` label, and every epoch file a cell names
//!    is present with the size and SHA-256 the cell states;
//! 5. `DIGEST` matches the cells present (the engine writes it only for a complete campaign;
//!    a complete campaign without one is refused);
//! 6. `scorecard.json` (the engine's verdicts against the client's bars) belongs to this
//!    campaign: same name, data class, engine and digest, and every row names a present cell.
//!
//! A cell the campaign lists but the directory lacks makes the campaign *incomplete*, which is
//! reported, not hidden. Anything else that fails is an error.

use super::hash::{cell_key, digest, sha256_hex};
use super::model::{
    CampaignCell, CampaignManifest, DataClass, Scorecard, CELL_SCHEMAS, LOCK_SOURCES,
    MODELLED_LABEL, PLAN_SCHEMAS, SCORECARD_SCHEMAS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// Why a campaign directory was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    Io(String),
    Parse {
        file: String,
        msg: String,
    },
    UnknownSchema {
        file: String,
        schema: String,
    },
    BadPlan(String),
    KeyMismatch {
        file: String,
        detail: String,
    },
    NotInCampaign {
        file: String,
        detail: String,
    },
    EngineVersion {
        file: String,
        found: String,
        expected: String,
    },
    ModelledLabel {
        file: String,
        found: String,
    },
    UnknownLockSource {
        file: String,
        found: String,
    },
    Epochs {
        file: String,
        detail: String,
    },
    MissingDigest,
    DigestMismatch {
        stated: String,
        computed: String,
    },
    MissingScorecard,
    ScorecardMismatch(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(m) => write!(f, "{m}"),
            LoadError::Parse { file, msg } => write!(f, "{file}: {msg}"),
            LoadError::UnknownSchema { file, schema } => {
                write!(f, "{file}: schema {schema:?} is not one this overlay reads")
            }
            LoadError::BadPlan(m) => write!(f, "campaign.json: {m}"),
            LoadError::KeyMismatch { file, detail } => write!(f, "{file}: cell key {detail}"),
            LoadError::NotInCampaign { file, detail } => {
                write!(f, "{file}: not part of this campaign ({detail})")
            }
            LoadError::EngineVersion {
                file,
                found,
                expected,
            } => {
                write!(f, "{file}: engine {found}, campaign {expected}")
            }
            LoadError::ModelledLabel { file, found } => {
                write!(
                    f,
                    "{file}: reference curve labelled {found:?}, must be {MODELLED_LABEL:?}"
                )
            }
            LoadError::UnknownLockSource { file, found } => write!(
                f,
                "{file}: lock source {found:?} is not one this overlay reads ({})",
                LOCK_SOURCES.join(", ")
            ),
            LoadError::Epochs { file, detail } => write!(f, "{file}: epoch output {detail}"),
            LoadError::MissingDigest => write!(f, "the campaign is complete but DIGEST is missing"),
            LoadError::DigestMismatch { stated, computed } => {
                write!(
                    f,
                    "DIGEST mismatch: file says {stated}, cells give {computed}"
                )
            }
            LoadError::MissingScorecard => write!(
                f,
                "scorecard.json is missing; run `kshana iq campaign report <dir>` first"
            ),
            LoadError::ScorecardMismatch(m) => write!(f, "scorecard.json: {m}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// A campaign directory that passed every check.
#[derive(Debug, Clone)]
pub struct LoadedCampaign {
    pub dir: PathBuf,
    pub manifest: CampaignManifest,
    /// Cells present, in the campaign's own order.
    pub cells: Vec<CampaignCell>,
    /// Keys the campaign lists that have no cell file yet.
    pub missing: Vec<String>,
    /// The verified DIGEST of the cells present (recomputed; the engine writes it to disk only
    /// for a complete campaign).
    pub digest: String,
    /// The engine's verdicts against the client's bars.
    pub scorecard: Scorecard,
}

impl LoadedCampaign {
    /// True when every listed cell is present.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }

    /// The campaign's declared data class (every cell carries the same one).
    pub fn data_class(&self) -> DataClass {
        self.manifest.data_class
    }
}

fn read(path: &Path) -> Result<String, LoadError> {
    std::fs::read_to_string(path).map_err(|e| LoadError::Io(format!("{}: {e}", path.display())))
}

fn parse<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, LoadError> {
    serde_json::from_str(text).map_err(|e| LoadError::Parse {
        file: file.to_owned(),
        msg: e.to_string(),
    })
}

/// What a planned cell must contain, resolved from the plan's indices.
struct Expected<'a> {
    recording: (&'a str, &'a str, &'a str),
    frontend: (&'a str, &'a str),
    design: (&'a str, &'a str),
}

/// Check one epoch file a cell names (`rel`, relative to `dir`): present, inside the output
/// directory, of the stated size (when given) and SHA-256.
fn check_epoch_file(
    dir: &Path,
    cell_file: &str,
    rel: &str,
    sha256: &str,
    bytes: Option<u64>,
) -> Result<(), LoadError> {
    let err = |detail: String| LoadError::Epochs {
        file: cell_file.to_owned(),
        detail,
    };
    let p = Path::new(rel);
    if p.is_absolute()
        || p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(err(format!("{rel:?} is outside the output directory")));
    }
    let data = std::fs::read(dir.join(p)).map_err(|e| err(format!("{rel}: {e}")))?;
    if let Some(n) = bytes {
        if data.len() as u64 != n {
            return Err(err(format!(
                "{rel}: {} bytes, the cell states {n}",
                data.len()
            )));
        }
    }
    let got = sha256_hex(&data);
    if got != sha256 {
        return Err(err(format!(
            "{rel}: SHA-256 {got}, the cell states {sha256}"
        )));
    }
    Ok(())
}

/// Load and check `dir` (the engine's campaign `--out` directory).
pub fn load(dir: &Path) -> Result<LoadedCampaign, LoadError> {
    let manifest: CampaignManifest = parse("campaign.json", &read(&dir.join("campaign.json"))?)?;
    if !PLAN_SCHEMAS.contains(&manifest.schema.as_str()) {
        return Err(LoadError::UnknownSchema {
            file: "campaign.json".into(),
            schema: manifest.schema.clone(),
        });
    }

    let mut expected: BTreeMap<&str, Expected> = BTreeMap::new();
    for pc in &manifest.cells {
        let r = manifest.recordings.get(pc.recording).ok_or_else(|| {
            LoadError::BadPlan(format!("cell {} names recording {}", pc.key, pc.recording))
        })?;
        let fe = manifest
            .frontend_name(pc.frontend)
            .zip(manifest.frontend_hash(pc.frontend))
            .ok_or_else(|| {
                LoadError::BadPlan(format!("cell {} names front end {}", pc.key, pc.frontend))
            })?;
        let de = manifest
            .design_name(pc.design)
            .zip(manifest.design_hash(pc.design))
            .ok_or_else(|| {
                LoadError::BadPlan(format!("cell {} names design {}", pc.key, pc.design))
            })?;
        expected.insert(
            pc.key.as_str(),
            Expected {
                recording: (r.id.as_str(), r.sha256.as_str(), r.conditions_hash.as_str()),
                frontend: fe,
                design: de,
            },
        );
    }

    let mut entries: Vec<PathBuf> = match std::fs::read_dir(dir.join("cells")) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
            .collect(),
        Err(_) => Vec::new(),
    };
    entries.sort();

    let mut by_key: BTreeMap<String, (CampaignCell, String)> = BTreeMap::new();
    for path in &entries {
        let file = format!("cells/{}", path.file_name().unwrap().to_string_lossy());
        let bytes = std::fs::read(path).map_err(|e| LoadError::Io(format!("{file}: {e}")))?;
        let text = String::from_utf8_lossy(&bytes);
        let cell: CampaignCell = parse(&file, &text)?;
        if !CELL_SCHEMAS.contains(&cell.schema.as_str()) {
            return Err(LoadError::UnknownSchema {
                file,
                schema: cell.schema,
            });
        }
        if path.file_stem().unwrap().to_string_lossy() != cell.key {
            return Err(LoadError::KeyMismatch {
                file,
                detail: format!("{:?} differs from the file name", cell.key),
            });
        }
        let derived = cell_key(&cell);
        if derived != cell.key {
            return Err(LoadError::KeyMismatch {
                file,
                detail: format!(
                    "{} does not re-derive from its inputs ({derived})",
                    cell.key
                ),
            });
        }
        let Some(exp) = expected.get(cell.key.as_str()) else {
            return Err(LoadError::NotInCampaign {
                file,
                detail: "key not planned".into(),
            });
        };
        if cell.engine_version != manifest.engine_version {
            return Err(LoadError::EngineVersion {
                file,
                found: cell.engine_version,
                expected: manifest.engine_version.clone(),
            });
        }
        let rec_ok = (
            cell.recording.id.as_str(),
            cell.recording.sha256.as_str(),
            cell.recording.conditions_hash.as_str(),
        ) == exp.recording;
        let fe_ok = (cell.frontend.name.as_str(), cell.frontend.hash.as_str()) == exp.frontend;
        let de_ok = (cell.design.name.as_str(), cell.design.hash.as_str()) == exp.design;
        let run_ok =
            cell.run_hash == manifest.run_hash && cell.scoring_hash == manifest.scoring_hash;
        let class_ok = cell.data_class == manifest.data_class;
        if !(rec_ok && fe_ok && de_ok && run_ok && class_ok) {
            let what = [
                (!rec_ok).then_some("recording"),
                (!fe_ok).then_some("front-end chain"),
                (!de_ok).then_some("loop design"),
                (!run_ok).then_some("run/scoring hash"),
                (!class_ok).then_some("data class"),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
            return Err(LoadError::NotInCampaign {
                file,
                detail: format!("{what} differs"),
            });
        }
        if !LOCK_SOURCES.contains(&cell.lock_source.as_str()) {
            return Err(LoadError::UnknownLockSource {
                file,
                found: cell.lock_source,
            });
        }
        if let Some(ep) = &cell.epochs {
            check_epoch_file(dir, &file, &ep.path, &ep.sha256, Some(ep.bytes))?;
            check_epoch_file(dir, &file, &ep.events_path, &ep.events_sha256, None)?;
        }
        for sat in &cell.satellites {
            for ev in &sat.events {
                if let Some(m) = &ev.modelled {
                    if m.label != MODELLED_LABEL {
                        return Err(LoadError::ModelledLabel {
                            file,
                            found: m.label.clone(),
                        });
                    }
                }
            }
        }
        by_key.insert(cell.key.clone(), (cell, sha256_hex(&bytes)));
    }

    let computed = digest(
        by_key
            .iter()
            .map(|(k, (_, sha))| (k.as_str(), sha.as_str())),
    );
    let complete = by_key.len() == manifest.cells.len();
    match std::fs::read_to_string(dir.join("DIGEST")) {
        Ok(t) if t.trim() != computed => {
            return Err(LoadError::DigestMismatch {
                stated: t.trim().to_owned(),
                computed,
            })
        }
        Ok(_) => {}
        Err(_) if complete => return Err(LoadError::MissingDigest),
        Err(_) => {}
    }

    let scorecard: Scorecard = match std::fs::read_to_string(dir.join("scorecard.json")) {
        Ok(t) => parse("scorecard.json", &t)?,
        Err(_) => return Err(LoadError::MissingScorecard),
    };
    if !SCORECARD_SCHEMAS.contains(&scorecard.schema.as_str()) {
        return Err(LoadError::UnknownSchema {
            file: "scorecard.json".into(),
            schema: scorecard.schema.clone(),
        });
    }
    if scorecard.campaign != manifest.name
        || scorecard.engine_version != manifest.engine_version
        || scorecard.data_class != manifest.data_class
    {
        return Err(LoadError::ScorecardMismatch(
            "built for another campaign, engine or data class".into(),
        ));
    }
    if scorecard.cells_total != manifest.cells.len()
        || scorecard.cells_present != by_key.len()
        || scorecard.complete != complete
    {
        return Err(LoadError::ScorecardMismatch(format!(
            "it says {} of {} cells present (complete: {}), the directory has {} of {} (complete: {complete})",
            scorecard.cells_present,
            scorecard.cells_total,
            scorecard.complete,
            by_key.len(),
            manifest.cells.len()
        )));
    }
    if complete && scorecard.digest.as_deref() != Some(computed.as_str()) {
        return Err(LoadError::ScorecardMismatch(
            "its digest is not the cells' (rebuild it with `kshana iq campaign report`)".into(),
        ));
    }
    let present: BTreeSet<&str> = by_key.keys().map(String::as_str).collect();
    if let Some(r) = scorecard
        .rows
        .iter()
        .find(|r| !present.contains(r.cell.as_str()))
    {
        return Err(LoadError::ScorecardMismatch(format!(
            "row names absent cell {}",
            r.cell
        )));
    }

    let mut cells = Vec::with_capacity(by_key.len());
    let mut missing = Vec::new();
    for pc in &manifest.cells {
        match by_key.remove(&pc.key) {
            Some((cell, _)) => cells.push(cell),
            None => missing.push(pc.key.clone()),
        }
    }

    Ok(LoadedCampaign {
        dir: dir.to_path_buf(),
        manifest,
        cells,
        missing,
        digest: computed,
        scorecard,
    })
}
