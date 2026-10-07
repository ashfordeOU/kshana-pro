//! Load a campaign output directory and check it before anything is rendered.
//!
//! Checks, in order:
//! 1. `campaign.json` parses, and its schema is one this overlay reads;
//! 2. every `cells/*.json` parses, its schema is known, its file name equals its `key`,
//!    and its key re-derives from its own inputs (LAB-CAMPAIGN.md §2, "Cells");
//! 3. every cell belongs to the campaign (its key is listed, its recording, chain and
//!    design are the campaign's, its run/scoring hashes match), and every cell has the
//!    campaign's engine version;
//! 4. every `modelled` block carries the `MODELLED` label;
//! 5. `DIGEST` matches the cells actually present.
//!
//! A cell the campaign lists but the directory lacks makes the campaign *incomplete*,
//! which is reported, not hidden. Anything else that fails is an error.

use super::hash::{cell_key, digest, sha256_hex};
use super::model::{
    CampaignCell, CampaignManifest, DataClass, CAMPAIGN_SCHEMAS, CELL_SCHEMAS, MODELLED_LABEL,
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
    MissingDigest,
    DigestMismatch {
        stated: String,
        computed: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(m) => write!(f, "{m}"),
            LoadError::Parse { file, msg } => write!(f, "{file}: {msg}"),
            LoadError::UnknownSchema { file, schema } => {
                write!(f, "{file}: schema {schema:?} is not one this overlay reads")
            }
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
            LoadError::MissingDigest => write!(f, "DIGEST is missing"),
            LoadError::DigestMismatch { stated, computed } => {
                write!(
                    f,
                    "DIGEST mismatch: file says {stated}, cells give {computed}"
                )
            }
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
    /// The verified DIGEST.
    pub digest: String,
}

impl LoadedCampaign {
    /// True when every listed cell is present.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }

    /// The campaign's declared data class, if it states one.
    pub fn data_class(&self) -> Option<DataClass> {
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

/// Load and check `dir` (the engine's campaign `--out` directory).
pub fn load(dir: &Path) -> Result<LoadedCampaign, LoadError> {
    let manifest_text = read(&dir.join("campaign.json"))?;
    let manifest: CampaignManifest = parse("campaign.json", &manifest_text)?;
    if !CAMPAIGN_SCHEMAS.contains(&manifest.schema.as_str()) {
        return Err(LoadError::UnknownSchema {
            file: "campaign.json".into(),
            schema: manifest.schema.clone(),
        });
    }

    let listed: BTreeSet<&str> = manifest.cells.iter().map(String::as_str).collect();
    let recordings: BTreeMap<&str, (&str, &str)> = manifest
        .recordings
        .iter()
        .map(|r| {
            (
                r.id.as_str(),
                (r.sha256.as_str(), r.conditions_hash.as_str()),
            )
        })
        .collect();
    let frontends: BTreeMap<&str, &str> = manifest
        .frontends
        .iter()
        .map(|f| (f.name.as_str(), f.hash.as_str()))
        .collect();
    let designs: BTreeMap<&str, &str> = manifest
        .designs
        .iter()
        .map(|d| (d.name.as_str(), d.hash.as_str()))
        .collect();

    let cells_dir = dir.join("cells");
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(&cells_dir) {
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
        let text = read(path)?;
        let cell: CampaignCell = parse(&file, &text)?;
        if !CELL_SCHEMAS.contains(&cell.schema.as_str()) {
            return Err(LoadError::UnknownSchema {
                file,
                schema: cell.schema,
            });
        }
        let stem = path.file_stem().unwrap().to_string_lossy();
        if stem != cell.key {
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
        if !listed.contains(cell.key.as_str()) {
            return Err(LoadError::NotInCampaign {
                file,
                detail: "key not listed".into(),
            });
        }
        if cell.engine_version != manifest.engine_version {
            return Err(LoadError::EngineVersion {
                file,
                found: cell.engine_version,
                expected: manifest.engine_version.clone(),
            });
        }
        let rec_ok = recordings.get(cell.recording.id.as_str())
            == Some(&(
                cell.recording.sha256.as_str(),
                cell.recording.conditions_hash.as_str(),
            ));
        let fe_ok =
            frontends.get(cell.frontend.name.as_str()) == Some(&cell.frontend.hash.as_str());
        let de_ok = designs.get(cell.design.name.as_str()) == Some(&cell.design.hash.as_str());
        let run_ok =
            cell.run_hash == manifest.run_hash && cell.scoring_hash == manifest.scoring_hash;
        if !(rec_ok && fe_ok && de_ok && run_ok) {
            let what = [
                (!rec_ok).then_some("recording"),
                (!fe_ok).then_some("front-end chain"),
                (!de_ok).then_some("loop design"),
                (!run_ok).then_some("run/scoring hash"),
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
        let file_sha = sha256_hex(text.as_bytes());
        by_key.insert(cell.key.clone(), (cell, file_sha));
    }

    let stated = match std::fs::read_to_string(dir.join("DIGEST")) {
        Ok(t) => t.trim().to_owned(),
        Err(_) => return Err(LoadError::MissingDigest),
    };
    let computed = digest(
        by_key
            .iter()
            .map(|(k, (_, sha))| (k.as_str(), sha.as_str())),
    );
    if stated != computed {
        return Err(LoadError::DigestMismatch { stated, computed });
    }

    let mut cells = Vec::with_capacity(by_key.len());
    let mut missing = Vec::new();
    for key in &manifest.cells {
        match by_key.remove(key) {
            Some((cell, _)) => cells.push(cell),
            None => missing.push(key.clone()),
        }
    }

    Ok(LoadedCampaign {
        dir: dir.to_path_buf(),
        manifest,
        cells,
        missing,
        digest: computed,
    })
}
