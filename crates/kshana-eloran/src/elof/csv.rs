// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The compact CSV profile of ELOF: observations only, for spreadsheets and receivers that
//! log rows.
//!
//! ```text
//! #ELOF-CSV 0.1
//! #header {"record":"header","format":"ELOF","version":"0.1",...}
//! t_s,chain,station,group,channel,toa_ns,ecd_us,snr_db,field_uv_m,phase_code,bearing_deg,flags
//! 0.0123,conf-A,M,12345678,,5123.25,0.1,24.5,812.0,A,,
//! ```
//!
//! Line 1 is the magic, line 2 the JSON header (exactly as in the JSON-lines profile, with
//! `truth_included` false), line 3 the fixed column line, then one `obs` row per line.
//! Empty cells are absent values; `flags` are joined with `|`. Numbers are written in the
//! shortest form that reads back to the same `f64`, so a CSV round trip is exact. Chain
//! and station ids may not contain `,`, `|`, `"` or line breaks. `truth`, `timing` and
//! `ldc` records have no CSV form: [`to_csv`] reports how many were left out.

use super::*;

/// First line of the CSV profile.
pub const MAGIC: &str = "#ELOF-CSV";
/// The column line.
pub const COLUMNS: &str =
    "t_s,chain,station,group,channel,toa_ns,ecd_us,snr_db,field_uv_m,phase_code,bearing_deg,flags";

/// Whether `text` is in the CSV profile.
pub fn is_csv(text: &str) -> bool {
    text.trim_start().starts_with(MAGIC)
}

fn bad(s: &str) -> bool {
    s.contains([',', '|', '"', '\n', '\r'])
}

/// Write the CSV profile; returns the text and the number of non-`obs` records left out.
pub fn to_csv(f: &ElofFile) -> Result<(String, usize), String> {
    let mut h = f.header.clone();
    h.truth_included = false;
    let mut header_json = serde_json::to_value(&h).map_err(|e| e.to_string())?;
    header_json
        .as_object_mut()
        .ok_or("header is not an object")?
        .insert("record".into(), "header".into());
    let mut s = format!(
        "{MAGIC} {VERSION}\n#header {}\n{COLUMNS}\n",
        serde_json::to_string(&header_json).map_err(|e| e.to_string())?
    );
    let o = |v: Option<f64>| v.map(|x| x.to_string()).unwrap_or_default();
    let mut left_out = 0;
    for r in &f.records {
        let Record::Obs(x) = r else {
            left_out += 1;
            continue;
        };
        if bad(&x.chain) || bad(&x.station) || x.flags.iter().any(|f| bad(f)) {
            return Err(format!(
                "CSV: id or flag of {}/{} contains a reserved character",
                x.chain, x.station
            ));
        }
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            x.t_s,
            x.chain,
            x.station,
            x.group,
            x.channel.map(|c| c.to_string()).unwrap_or_default(),
            x.toa_ns,
            o(x.ecd_us),
            o(x.snr_db),
            o(x.field_uv_m),
            match x.phase_code {
                Some(Parity::A) => "A",
                Some(Parity::B) => "B",
                None => "",
            },
            o(x.bearing_deg),
            x.flags.iter().cloned().collect::<Vec<_>>().join("|"),
        ));
    }
    Ok((s, left_out))
}

/// Read the CSV profile (structure; see [`validate`] for the semantic checks).
pub fn from_csv(text: &str) -> Result<ElofFile, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty());
    let (_, magic) = lines.next().ok_or("ELOF CSV: empty")?;
    let ver = magic
        .trim()
        .strip_prefix(MAGIC)
        .map(str::trim)
        .ok_or("line 1: not the ELOF CSV magic")?;
    match parse_version(ver) {
        Some((maj, min)) if maj == SUPPORTED.0 && min <= SUPPORTED.1 => {}
        _ => return Err(format!("line 1: ELOF CSV version {ver:?} is not supported")),
    }
    let (_, hl) = lines.next().ok_or("ELOF CSV: no header line")?;
    let hj = hl
        .strip_prefix("#header ")
        .ok_or("line 2: expected '#header {json}'")?;
    let file = ElofFile::from_jsonl(hj).map_err(|e| format!("line 2: {e}"))?;
    let (_, cols) = lines.next().ok_or("ELOF CSV: no column line")?;
    if cols.trim() != COLUMNS {
        return Err(format!("line 3: columns must be exactly {COLUMNS:?}"));
    }
    let mut records = Vec::new();
    for (i, l) in lines {
        let n = i + 1;
        let c: Vec<&str> = l.split(',').collect();
        if c.len() != 12 {
            return Err(format!("line {n}: {} cells, expected 12", c.len()));
        }
        let num = |k: usize, name: &str| -> Result<f64, String> {
            c[k].trim()
                .parse::<f64>()
                .map_err(|_| format!("line {n}: {name} {:?} is not a number", c[k]))
        };
        let opt = |k: usize, name: &str| -> Result<Option<f64>, String> {
            if c[k].trim().is_empty() {
                Ok(None)
            } else {
                num(k, name).map(Some)
            }
        };
        records.push(Record::Obs(Obs {
            t_s: num(0, "t_s")?,
            chain: c[1].to_string(),
            station: c[2].to_string(),
            group: c[3]
                .trim()
                .parse()
                .map_err(|_| format!("line {n}: group {:?} is not an integer", c[3]))?,
            channel: if c[4].trim().is_empty() {
                None
            } else {
                Some(
                    c[4].trim()
                        .parse()
                        .map_err(|_| format!("line {n}: channel {:?}", c[4]))?,
                )
            },
            toa_ns: num(5, "toa_ns")?,
            ecd_us: opt(6, "ecd_us")?,
            snr_db: opt(7, "snr_db")?,
            field_uv_m: opt(8, "field_uv_m")?,
            phase_code: match c[9].trim() {
                "" => None,
                "A" => Some(Parity::A),
                "B" => Some(Parity::B),
                x => return Err(format!("line {n}: phase_code {x:?} is not A or B")),
            },
            bearing_deg: opt(10, "bearing_deg")?,
            flags: c[11]
                .split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
        }));
    }
    Ok(ElofFile {
        header: file.header,
        records,
    })
}
