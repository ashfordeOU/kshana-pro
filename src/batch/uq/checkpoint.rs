// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The checkpoint file `runs.jsonl`: one JSON document per line.
//!
//! * Line 1 is the **header**: the format number, the study hash, the number of runs
//!   the study needs, and the overlay and engine versions that started it.
//! * Every further line is one **finished realisation**: its index, the input vector,
//!   the engine seed (or null), the scalar output values, the series values, the
//!   SHA-256 of the engine's result document, and the error when the run failed.
//!
//! Lines are appended in realisation order and each is written in one call, so a run
//! that is stopped leaves a valid prefix. If the process dies in the middle of a write,
//! the last line is cut short: [`load`] detects a last line that does not parse, drops
//! it, truncates the file back to the last whole line and reports it, and that
//! realisation is run again. A line that does not parse anywhere else means the file
//! was damaged or edited, and the load is refused.
//!
//! Numbers are written in the shortest form that reads back to the same 64-bit value,
//! so a resumed study computes from exactly the numbers an uninterrupted one holds.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use super::exec::RunRecord;

/// The checkpoint format number.
pub const FORMAT: u64 = 1;
/// The header key that identifies the file.
pub const HEADER_KEY: &str = "kshana_pro_uncertainty_checkpoint";

/// The header of a checkpoint file.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub study_hash: String,
    pub runs_total: usize,
    pub overlay_version: String,
    pub engine_version: String,
}

impl Header {
    /// The header as its line (no newline).
    pub fn to_line(&self) -> String {
        json!({
            HEADER_KEY: FORMAT,
            "study_hash": self.study_hash,
            "runs_total": self.runs_total,
            "overlay_version": self.overlay_version,
            "engine_version": self.engine_version,
        })
        .to_string()
    }

    fn from_line(line: &str) -> Result<Header, String> {
        let v: Value = serde_json::from_str(line)
            .map_err(|e| format!("the checkpoint header does not parse: {e}"))?;
        let format = v.get(HEADER_KEY).and_then(Value::as_u64).ok_or_else(|| {
            format!("the first line is not a checkpoint header (no `{HEADER_KEY}`)")
        })?;
        if format != FORMAT {
            return Err(format!(
                "checkpoint format {format} is not the format this build reads ({FORMAT})"
            ));
        }
        let text = |k: &str| -> Result<String, String> {
            v.get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("the checkpoint header has no `{k}`"))
        };
        Ok(Header {
            study_hash: text("study_hash")?,
            runs_total: v
                .get("runs_total")
                .and_then(Value::as_u64)
                .ok_or("the checkpoint header has no `runs_total`")?
                as usize,
            overlay_version: text("overlay_version")?,
            engine_version: text("engine_version")?,
        })
    }
}

fn number_or_null(v: f64) -> Value {
    if v.is_finite() {
        json!(v)
    } else {
        Value::Null
    }
}

/// One record as its line (no newline).
pub fn record_line(r: &RunRecord) -> String {
    json!({
        "index": r.index,
        "inputs": r.inputs,
        "seed": r.seed,
        "outputs": r.scalars,
        "series": r.series.iter().map(|s| {
            s.as_ref().map(|xs| xs.iter().map(|x| number_or_null(*x)).collect::<Vec<_>>())
        }).collect::<Vec<_>>(),
        "result_sha256": r.result_sha256,
        "error": r.error,
    })
    .to_string()
}

fn record_from_line(line: &str) -> Result<RunRecord, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let obj = v.as_object().ok_or("not a JSON object")?;
    let get = |k: &str| obj.get(k).ok_or_else(|| format!("no `{k}`"));
    let floats = |k: &str| -> Result<Vec<f64>, String> {
        get(k)?
            .as_array()
            .ok_or_else(|| format!("`{k}` is not an array"))?
            .iter()
            .map(|x| {
                x.as_f64()
                    .ok_or_else(|| format!("`{k}` holds a non-number"))
            })
            .collect()
    };
    let optional_text = |k: &str| -> Result<Option<String>, String> {
        match get(k)? {
            Value::Null => Ok(None),
            Value::String(s) => Ok(Some(s.clone())),
            _ => Err(format!("`{k}` is neither text nor null")),
        }
    };
    let scalars = get("outputs")?
        .as_array()
        .ok_or("`outputs` is not an array")?
        .iter()
        .map(|x| match x {
            Value::Null => Ok(None),
            other => other
                .as_f64()
                .map(Some)
                .ok_or_else(|| "`outputs` holds a non-number".to_string()),
        })
        .collect::<Result<Vec<_>, String>>()?;
    let series = get("series")?
        .as_array()
        .ok_or("`series` is not an array")?
        .iter()
        .map(|s| match s {
            Value::Null => Ok(None),
            Value::Array(xs) => xs
                .iter()
                .map(|x| match x {
                    Value::Null => Ok(f64::NAN),
                    other => other
                        .as_f64()
                        .ok_or_else(|| "`series` holds a non-number".to_string()),
                })
                .collect::<Result<Vec<_>, String>>()
                .map(Some),
            _ => Err("`series` holds a non-array".to_string()),
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(RunRecord {
        index: get("index")?
            .as_u64()
            .ok_or("`index` is not a whole number")? as usize,
        inputs: floats("inputs")?,
        seed: match get("seed")? {
            Value::Null => None,
            other => Some(other.as_u64().ok_or("`seed` is not a whole number")?),
        },
        scalars,
        series,
        result_sha256: optional_text("result_sha256")?,
        error: optional_text("error")?,
    })
}

/// What [`load`] found.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub header: Header,
    /// Finished realisations by index.
    pub records: BTreeMap<usize, RunRecord>,
    /// The index-less description of a cut-short last line that was dropped, if any.
    pub dropped_truncated_line: Option<String>,
}

/// Read a checkpoint file, check it belongs to the study with `study_hash`, and repair
/// a cut-short last line by truncating the file back to the last whole line.
pub fn load(path: &Path, study_hash: &str, runs_total: usize) -> Result<Loaded, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let ends_with_newline = text.ends_with('\n');
    let lines: Vec<&str> = text.split('\n').collect();
    // `split` leaves one empty piece after a final newline.
    let lines: &[&str] = if ends_with_newline {
        &lines[..lines.len() - 1]
    } else {
        &lines[..]
    };
    let Some(first) = lines.first() else {
        return Err(format!("{} is empty", path.display()));
    };
    let header = Header::from_line(first)?;
    if header.study_hash != study_hash {
        return Err(format!(
            "refused: {} was written for a different study (its study hash is {}, this \
             study's is {}). The study file changed after the checkpoint was started. \
             Restore the study file, or start again in a new output directory",
            path.display(),
            header.study_hash,
            study_hash
        ));
    }
    if header.runs_total != runs_total {
        return Err(format!(
            "refused: {} records a study of {} runs, this study needs {runs_total}",
            path.display(),
            header.runs_total
        ));
    }

    let mut records = BTreeMap::new();
    let mut good_bytes = first.len() + 1;
    let mut dropped = None;
    for (at, line) in lines.iter().enumerate().skip(1) {
        let last = at + 1 == lines.len();
        match record_from_line(line) {
            Ok(r) => {
                if r.index >= runs_total {
                    return Err(format!(
                        "{} line {}: realisation index {} is outside the study's {runs_total} runs",
                        path.display(),
                        at + 1,
                        r.index
                    ));
                }
                if records.insert(r.index, r).is_some() {
                    return Err(format!(
                        "{} line {}: a realisation is recorded twice",
                        path.display(),
                        at + 1
                    ));
                }
                good_bytes += line.len() + 1;
            }
            Err(e) if last => {
                dropped = Some(format!(
                    "line {} of {} was cut short ({e}); it was dropped and its realisation \
                     is run again",
                    at + 1,
                    path.display()
                ));
            }
            Err(e) => {
                return Err(format!(
                    "{} line {} does not parse ({e}) and it is not the last line: the \
                     checkpoint is damaged",
                    path.display(),
                    at + 1
                ));
            }
        }
    }

    // Put the file back to whole lines only, each ending in a newline.
    let intact = dropped.is_none() && ends_with_newline;
    if !intact {
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| format!("could not repair {}: {e}", path.display()))?;
        let keep = if dropped.is_some() {
            good_bytes
        } else {
            bytes.len()
        };
        file.set_len(keep.min(bytes.len()) as u64)
            .map_err(|e| format!("could not repair {}: {e}", path.display()))?;
        drop(file);
        if dropped.is_none() {
            // Every line parsed but the last has no newline: add it.
            let mut f = OpenOptions::new()
                .append(true)
                .open(path)
                .map_err(|e| format!("could not repair {}: {e}", path.display()))?;
            f.write_all(b"\n")
                .map_err(|e| format!("could not repair {}: {e}", path.display()))?;
        }
    }
    Ok(Loaded {
        header,
        records,
        dropped_truncated_line: dropped,
    })
}

/// Appends records to a checkpoint file.
pub struct Writer {
    file: File,
    path: String,
}

impl Writer {
    /// Start a new checkpoint file with its header. Fails if the file exists.
    pub fn create(path: &Path, header: &Header) -> Result<Writer, String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("could not create {}: {e}", path.display()))?;
        file.write_all(format!("{}\n", header.to_line()).as_bytes())
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        Ok(Writer {
            file,
            path: path.display().to_string(),
        })
    }

    /// Continue an existing checkpoint file.
    pub fn append(path: &Path) -> Result<Writer, String> {
        let file = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| format!("could not open {}: {e}", path.display()))?;
        Ok(Writer {
            file,
            path: path.display().to_string(),
        })
    }

    /// Append one finished realisation, as one write.
    pub fn write(&mut self, record: &RunRecord) -> Result<(), String> {
        self.file
            .write_all(format!("{}\n", record_line(record)).as_bytes())
            .and_then(|()| self.file.flush())
            .map_err(|e| format!("could not write {}: {e}", self.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        // Under the build's own target directory, never the system temporary folder.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/tmp")
            .join(format!("uq-checkpoint-unit-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        dir.join("runs.jsonl")
    }

    fn header() -> Header {
        Header {
            study_hash: "abc123".into(),
            runs_total: 5,
            overlay_version: "0.1.0".into(),
            engine_version: "0.28.0".into(),
        }
    }

    fn record(index: usize) -> RunRecord {
        RunRecord {
            index,
            // Values with no short decimal form: they must read back bit for bit.
            inputs: vec![0.1 + 0.2, 1.0 / 3.0 * index as f64],
            seed: index.is_multiple_of(2).then_some(42 + index as u64),
            scalars: vec![Some(92.124_475_034_844_01), None],
            series: vec![Some(vec![1.5e-20, f64::NAN, -3.0]), None],
            result_sha256: Some("00ff".into()),
            error: (index == 3).then(|| "range_km must be finite and positive".to_string()),
        }
    }

    /// Records compare equal except that a not-a-number never equals itself.
    fn same(a: &RunRecord, b: &RunRecord) -> bool {
        let bits = |r: &RunRecord| -> Vec<Option<Vec<u64>>> {
            r.series
                .iter()
                .map(|s| {
                    s.as_ref()
                        .map(|xs| xs.iter().map(|x| x.to_bits()).collect())
                })
                .collect()
        };
        a.index == b.index
            && a.inputs
                .iter()
                .map(|x| x.to_bits())
                .eq(b.inputs.iter().map(|x| x.to_bits()))
            && a.seed == b.seed
            && a.scalars == b.scalars
            && bits(a) == bits(b)
            && a.result_sha256 == b.result_sha256
            && a.error == b.error
    }

    #[test]
    fn records_read_back_bit_for_bit() {
        let path = scratch("roundtrip");
        let mut w = Writer::create(&path, &header()).expect("created");
        for i in 0..4 {
            w.write(&record(i)).expect("written");
        }
        drop(w);
        let loaded = load(&path, "abc123", 5).expect("loads");
        assert_eq!(loaded.header, header());
        assert_eq!(loaded.records.len(), 4);
        assert!(loaded.dropped_truncated_line.is_none());
        for i in 0..4 {
            assert!(same(&loaded.records[&i], &record(i)), "record {i}");
        }
        // Creating over an existing checkpoint is refused.
        assert!(Writer::create(&path, &header()).is_err());
    }

    #[test]
    fn a_different_study_hash_or_size_is_refused() {
        let path = scratch("hash");
        drop(Writer::create(&path, &header()).expect("created"));
        let err = load(&path, "zzz999", 5).expect_err("refused");
        assert!(err.contains("refused") && err.contains("abc123") && err.contains("zzz999"));
        let err = load(&path, "abc123", 6).expect_err("refused");
        assert!(err.contains("refused"), "{err}");
    }

    #[test]
    fn a_cut_short_last_line_is_dropped_and_the_file_truncated() {
        let path = scratch("truncated");
        let mut w = Writer::create(&path, &header()).expect("created");
        for i in 0..3 {
            w.write(&record(i)).expect("written");
        }
        drop(w);
        let whole = std::fs::read(&path).expect("read");
        // Cut the file in the middle of the last record.
        std::fs::write(&path, &whole[..whole.len() - 25]).expect("cut");
        let loaded = load(&path, "abc123", 5).expect("loads with a repair");
        assert_eq!(
            loaded.records.keys().copied().collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(loaded
            .dropped_truncated_line
            .as_deref()
            .expect("reported")
            .contains("cut short"));
        // The file now ends after record 1; appending record 2 restores it exactly.
        let mut w = Writer::append(&path).expect("reopened");
        w.write(&record(2)).expect("written");
        drop(w);
        assert_eq!(std::fs::read(&path).expect("read"), whole);
    }

    #[test]
    fn a_whole_last_line_without_its_newline_is_kept() {
        let path = scratch("no-newline");
        let mut w = Writer::create(&path, &header()).expect("created");
        w.write(&record(0)).expect("written");
        drop(w);
        let whole = std::fs::read(&path).expect("read");
        std::fs::write(&path, &whole[..whole.len() - 1]).expect("cut the newline");
        let loaded = load(&path, "abc123", 5).expect("loads");
        assert_eq!(loaded.records.len(), 1);
        assert!(loaded.dropped_truncated_line.is_none());
        assert_eq!(std::fs::read(&path).expect("read"), whole);
    }

    #[test]
    fn damage_before_the_last_line_is_refused() {
        let path = scratch("damaged");
        let mut w = Writer::create(&path, &header()).expect("created");
        for i in 0..3 {
            w.write(&record(i)).expect("written");
        }
        drop(w);
        let text = std::fs::read_to_string(&path).expect("read");
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        lines[2].truncate(20);
        std::fs::write(&path, lines.join("\n") + "\n").expect("damage");
        let err = load(&path, "abc123", 5).expect_err("refused");
        assert!(err.contains("damaged"), "{err}");

        // A duplicated realisation and an index past the end are refused too.
        let twice = format!(
            "{}\n{}\n{}\n",
            header().to_line(),
            record_line(&record(1)),
            record_line(&record(1))
        );
        std::fs::write(&path, twice).expect("write");
        assert!(load(&path, "abc123", 5)
            .expect_err("refused")
            .contains("twice"));
        let past = format!("{}\n{}\n", header().to_line(), record_line(&record(9)));
        std::fs::write(&path, past).expect("write");
        assert!(load(&path, "abc123", 5)
            .expect_err("refused")
            .contains("outside"));
    }
}
