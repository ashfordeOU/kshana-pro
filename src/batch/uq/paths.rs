// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Paths into a scenario (to set an input) and into a result document (to read an
//! output).
//!
//! Both grammars are the open engine's own, so a path that works in an open
//! `campaign` or `sweep-nd` works here unchanged:
//!
//! * **Scenario key** (the open sweep's rule): dot-separated table names; a segment
//!   that is a whole number indexes an array of tables, so `fusion.system.1.sisre_m`
//!   is the second `[[fusion.system]]`'s `sisre_m`. Every segment must already exist:
//!   a mistyped key fails, it never creates a field the engine would ignore.
//! * **Result path** (the open campaign's rule): dot-separated field names; `name[i]`
//!   takes element `i` of an array, counting from the end when `i` is negative
//!   (`series[-1]` is the last row); `name[]` takes every element, which names a
//!   series rather than one value.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
enum Seg {
    Key(String),
    Each(String),
    Index(String, i64),
}

fn parse(path: &str) -> Result<Vec<Seg>, String> {
    if path.is_empty() {
        return Err("empty result path".into());
    }
    path.split('.')
        .map(|part| {
            if part.is_empty() {
                return Err(format!("result path `{path}` has an empty segment"));
            }
            if let Some(name) = part.strip_suffix("[]") {
                return Ok(Seg::Each(name.to_string()));
            }
            if let (Some(open), true) = (part.find('['), part.ends_with(']')) {
                let name = &part[..open];
                let idx: i64 = part[open + 1..part.len() - 1]
                    .parse()
                    .map_err(|_| format!("result path `{path}`: bad index in `{part}`"))?;
                return Ok(Seg::Index(name.to_string(), idx));
            }
            Ok(Seg::Key(part.to_string()))
        })
        .collect()
}

fn child<'a>(v: &'a Value, name: &str, path: &str) -> Result<&'a Value, String> {
    if name.is_empty() {
        return Ok(v);
    }
    v.get(name)
        .ok_or_else(|| format!("result path `{path}`: no field `{name}`"))
}

fn index<'a>(v: &'a Value, i: i64, path: &str) -> Result<&'a Value, String> {
    let a = v
        .as_array()
        .ok_or_else(|| format!("result path `{path}`: indexed value is not an array"))?;
    let n = a.len() as i64;
    let j = if i < 0 { n + i } else { i };
    if j < 0 || j >= n {
        return Err(format!(
            "result path `{path}`: index {i} out of range ({n})"
        ));
    }
    Ok(&a[j as usize])
}

fn leaf(v: &Value, path: &str) -> Result<f64, String> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Null => Ok(f64::NAN),
        _ => Err(format!(
            "result path `{path}`: value is not a number or a flag"
        )),
    }
}

fn walk<'a>(mut cur: &'a Value, segs: &[Seg], path: &str) -> Result<&'a Value, String> {
    for s in segs {
        cur = match s {
            Seg::Key(k) => child(cur, k, path)?,
            Seg::Index(k, i) => index(child(cur, k, path)?, *i, path)?,
            Seg::Each(_) => return Err(format!("result path `{path}` names rows, not one value")),
        };
    }
    Ok(cur)
}

/// Whether `path` names a series (it has a `[]` segment).
pub fn is_series(path: &str) -> bool {
    path.split('.').any(|p| p.ends_with("[]"))
}

/// The number at a scalar result path; `None` for a null or a non-finite value.
pub fn scalar(root: &Value, path: &str) -> Result<Option<f64>, String> {
    let segs = parse(path)?;
    if segs.iter().any(|s| matches!(s, Seg::Each(_))) {
        return Err(format!(
            "result path `{path}` must name one value, not rows (use `series` for a band)"
        ));
    }
    let v = leaf(walk(root, &segs, path)?, path)?;
    Ok(v.is_finite().then_some(v))
}

/// The numbers along a series result path (exactly one `[]` segment); a null row value
/// is a not-a-number.
pub fn series(root: &Value, path: &str) -> Result<Vec<f64>, String> {
    let segs = parse(path)?;
    let each: Vec<usize> = segs
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s, Seg::Each(_)))
        .map(|(i, _)| i)
        .collect();
    let [pos] = each[..] else {
        return Err(format!(
            "series path `{path}` must have exactly one `[]` segment (it has {})",
            each.len()
        ));
    };
    let parent = walk(root, &segs[..pos], path)?;
    let Seg::Each(name) = &segs[pos] else {
        unreachable!("`pos` is the position of a `[]` segment")
    };
    let rows = child(parent, name, path)?
        .as_array()
        .ok_or_else(|| format!("result path `{path}`: `{name}` is not an array"))?;
    rows.iter()
        .map(|row| {
            let v = leaf(walk(row, &segs[pos + 1..], path)?, path)?;
            Ok(if v.is_finite() { v } else { f64::NAN })
        })
        .collect()
}

/// The units-block spelling of a result path: every index becomes `[]`.
pub fn canonical(path: &str) -> String {
    path.split('.')
        .map(|p| match (p.find('['), p.ends_with(']')) {
            (Some(open), true) => format!("{}[]", &p[..open]),
            _ => p.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// The unit a result document's own `units` block gives `path`.
pub fn unit_in_doc(doc: &Value, path: &str) -> Option<String> {
    let units = doc.get("units")?.as_object()?;
    let (_, entry) = kshana::field_schema::lookup(units, &canonical(path))?;
    entry.get("unit")?.as_str().map(str::to_string)
}

/// The value at a scenario key.
pub fn get_key<'a>(root: &'a toml::Value, key: &str) -> Result<&'a toml::Value, String> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("scenario key `{key}` is malformed"));
    }
    let mut cur = root;
    for part in parts {
        cur = if let Some(arr) = cur.as_array() {
            let i: usize = part.parse().map_err(|_| {
                format!("scenario key `{key}`: `{part}` follows an array, so it must be an index")
            })?;
            arr.get(i).ok_or_else(|| {
                format!(
                    "scenario key `{key}`: index {i} is past the {} entries of the array",
                    arr.len()
                )
            })?
        } else {
            cur.as_table()
                .ok_or_else(|| format!("scenario key `{key}`: `{part}`'s parent is not a table"))?
                .get(part)
                .ok_or_else(|| {
                    format!("scenario key `{key}`: no field `{part}` in the base scenario")
                })?
        };
    }
    Ok(cur)
}

/// Replace the value at a scenario key that already exists.
pub fn set_key(root: &mut toml::Value, key: &str, value: toml::Value) -> Result<(), String> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("scenario key `{key}` is malformed"));
    }
    let mut cur = root;
    for part in &parts[..parts.len() - 1] {
        if cur.is_array() {
            let i: usize = part.parse().map_err(|_| {
                format!("scenario key `{key}`: `{part}` follows an array, so it must be an index")
            })?;
            let arr = cur.as_array_mut().ok_or("unreachable")?;
            let n = arr.len();
            cur = arr.get_mut(i).ok_or_else(|| {
                format!("scenario key `{key}`: index {i} is past the {n} entries of the array")
            })?;
            continue;
        }
        let tbl = cur
            .as_table_mut()
            .ok_or_else(|| format!("scenario key `{key}`: `{part}`'s parent is not a table"))?;
        cur = tbl.get_mut(*part).ok_or_else(|| {
            format!("scenario key `{key}`: no field `{part}` in the base scenario")
        })?;
    }
    let last = parts[parts.len() - 1];
    let tbl = cur
        .as_table_mut()
        .ok_or_else(|| format!("scenario key `{key}`: parent of `{last}` is not a table"))?;
    let slot = tbl
        .get_mut(last)
        .ok_or_else(|| format!("scenario key `{key}`: no field `{last}` in the base scenario"))?;
    *slot = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> Value {
        json!({
            "fom": {"rms": 3.5, "ok": true, "gap": null, "name": "x"},
            "series": [
                {"t": 0.0, "e": 1.0},
                {"t": 10.0, "e": null},
                {"t": 20.0, "e": -4.0}
            ],
            "cases": [{"v": 7.0}, {"v": 8.0}],
            "units": {"series[].e": {"unit": "ns", "provenance": "computed"},
                      "fom.rms": {"unit": "m", "provenance": "computed"}}
        })
    }

    #[test]
    fn scalar_paths_index_from_either_end() {
        let d = doc();
        assert_eq!(scalar(&d, "fom.rms"), Ok(Some(3.5)));
        assert_eq!(scalar(&d, "fom.ok"), Ok(Some(1.0)));
        assert_eq!(scalar(&d, "fom.gap"), Ok(None));
        assert_eq!(scalar(&d, "series[-1].e"), Ok(Some(-4.0)));
        assert_eq!(scalar(&d, "series[0].t"), Ok(Some(0.0)));
        assert_eq!(scalar(&d, "cases[1].v"), Ok(Some(8.0)));
        assert!(scalar(&d, "fom.nope").is_err());
        assert!(scalar(&d, "fom.name").is_err());
        assert!(scalar(&d, "series[3].e").is_err());
        assert!(scalar(&d, "series[-4].e").is_err());
        assert!(scalar(&d, "series[].e").is_err());
        assert!(scalar(&d, "").is_err());
    }

    #[test]
    fn series_paths_take_every_row() {
        let d = doc();
        let e = series(&d, "series[].e").expect("a series");
        assert_eq!(e.len(), 3);
        assert_eq!((e[0], e[2]), (1.0, -4.0));
        assert!(e[1].is_nan());
        assert_eq!(series(&d, "series[].t"), Ok(vec![0.0, 10.0, 20.0]));
        assert!(series(&d, "fom.rms").is_err());
        assert!(series(&d, "series[].nope").is_err());
        assert!(is_series("series[].e") && !is_series("series[-1].e"));
    }

    #[test]
    fn units_are_read_through_the_engine_grammar() {
        let d = doc();
        assert_eq!(canonical("series[-1].e"), "series[].e");
        assert_eq!(unit_in_doc(&d, "series[-1].e").as_deref(), Some("ns"));
        assert_eq!(unit_in_doc(&d, "fom.rms").as_deref(), Some("m"));
        assert_eq!(unit_in_doc(&d, "cases[0].v"), None);
    }

    #[test]
    fn scenario_keys_must_exist_and_may_index_arrays_of_tables() {
        let mut scn: toml::Value = toml::from_str(
            "a = 1.0\n[t]\nb = 2.0\n[[sys]]\nname = \"x\"\ns = 0.5\n[[sys]]\nname = \"y\"\ns = 0.7\n",
        )
        .expect("valid");
        assert_eq!(get_key(&scn, "t.b").expect("there").as_float(), Some(2.0));
        assert_eq!(
            get_key(&scn, "sys.1.s").expect("there").as_float(),
            Some(0.7)
        );
        set_key(&mut scn, "sys.1.s", toml::Value::Float(0.9)).expect("set");
        set_key(&mut scn, "a", toml::Value::Float(3.0)).expect("set");
        assert_eq!(
            get_key(&scn, "sys.1.s").expect("there").as_float(),
            Some(0.9)
        );
        assert_eq!(
            get_key(&scn, "sys.0.s").expect("there").as_float(),
            Some(0.5)
        );
        assert_eq!(get_key(&scn, "a").expect("there").as_float(), Some(3.0));
        for bad in ["t.c", "nope", "sys.2.s", "sys.x.s", "a.b", "t..b"] {
            assert!(get_key(&scn, bad).is_err(), "get {bad}");
            assert!(
                set_key(&mut scn, bad, toml::Value::Float(1.0)).is_err(),
                "set {bad}"
            );
        }
    }
}
