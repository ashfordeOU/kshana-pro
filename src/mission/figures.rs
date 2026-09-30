// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The figures of one run, read from its result document.
//!
//! A figure is a number the run **computed**. The result document says which those are:
//! every entry of its `units` block carries a `provenance`, and only the entries with
//! `provenance = "computed"` are figures. Everything else numeric is listed for what it
//! is: an output of another class (`modelled`, `derived`, `closed-form`, ...), or an
//! input or constant the run was given (`input`, `modelled-input`, `spec`, `published`,
//! `constant`, `measured-or-input`). An echoed input is never shown as a figure.
//!
//! Nothing is recomputed. A column of numbers is summarised by its count, minimum and
//! maximum, and the chart columns are the result's own arrays.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::programme::ChartSpec;

/// A column is charted with at most this many points; a longer one is thinned by a
/// constant stride, and the chart says so.
pub const MAX_CHART_POINTS: usize = 2000;
/// An array of objects with at most this many elements is listed element by element
/// (`timeline.bands[3].min_cn0_dbhz`); a longer one is summarised as a column.
pub const MAX_EXPANDED_ELEMENTS: usize = 12;

/// One number of a result document.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FigureRow {
    /// The path of the value, with an index where it sits in a short array.
    pub path: String,
    pub value: f64,
    pub unit: Option<String>,
    /// The engine's own description of the field (the `note` of its units entry).
    pub note: Option<String>,
    /// The provenance class the units entry states.
    pub provenance: String,
    /// The `name` or `label` of the array element the value sits in, when it has one.
    pub element: Option<String>,
}

/// One column of numbers of a result document, summarised.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SeriesRow {
    /// The canonical path, `[]` for each array level.
    pub path: String,
    pub unit: Option<String>,
    pub note: Option<String>,
    pub provenance: String,
    /// How many numbers the column holds (nulls are not counted).
    pub count: usize,
    pub min: f64,
    pub max: f64,
}

/// Every number of one result document, sorted into what it is.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct RunFigures {
    /// The figures the sheet leads with: the ones the programme names, else all of
    /// `figures`.
    pub headline: Vec<FigureRow>,
    /// Every computed scalar.
    pub figures: Vec<FigureRow>,
    /// Scalar outputs of another provenance class, each with its class.
    pub other_outputs: Vec<FigureRow>,
    /// Inputs and constants the run was given and echoes.
    pub inputs: Vec<FigureRow>,
    /// Computed columns, summarised.
    pub series: Vec<SeriesRow>,
    /// Numeric fields with no entry in the result's `units` block. They are counted,
    /// not shown: without a provenance nothing says whether one is a figure.
    pub unlabelled_numeric_fields: usize,
    /// True when the result document has no `units` block with provenance at all.
    pub no_provenance: bool,
}

/// The provenance classes that are inputs or constants, not outputs of the run.
fn is_input_class(p: &str) -> bool {
    matches!(
        p,
        "input" | "modelled-input" | "spec" | "published" | "constant" | "measured-or-input"
    )
}

struct Leaf {
    concrete: String,
    canonical: String,
    value: f64,
    /// Number of array levels above the leaf.
    depth: usize,
    /// Length of the single enclosing array, when there is exactly one.
    len: usize,
    element: Option<String>,
    /// True when the leaf is itself an array element (a column of numbers).
    in_numeric_array: bool,
}

/// Where the walk is in the document.
struct At<'a> {
    concrete: String,
    canonical: String,
    depth: usize,
    len: usize,
    element: Option<&'a str>,
    in_array: bool,
}

fn walk(v: &Value, at: &At<'_>, out: &mut Vec<Leaf>) {
    match v {
        Value::Number(n) => {
            if let Some(x) = n.as_f64().filter(|x| x.is_finite()) {
                out.push(Leaf {
                    concrete: at.concrete.clone(),
                    canonical: at.canonical.clone(),
                    value: x,
                    depth: at.depth,
                    len: at.len,
                    element: at.element.map(str::to_string),
                    in_numeric_array: at.in_array,
                });
            }
        }
        Value::Array(a) => {
            for (i, e) in a.iter().enumerate() {
                let name = e
                    .get("name")
                    .or_else(|| e.get("label"))
                    .and_then(Value::as_str);
                walk(
                    e,
                    &At {
                        concrete: format!("{}[{i}]", at.concrete),
                        canonical: format!("{}[]", at.canonical),
                        depth: at.depth + 1,
                        len: a.len(),
                        element: name.or(at.element),
                        in_array: true,
                    },
                    out,
                );
            }
        }
        Value::Object(m) => {
            for (k, child) in m {
                if at.concrete.is_empty() && k == "units" {
                    continue;
                }
                let (concrete, canonical) = if at.concrete.is_empty() {
                    (k.clone(), k.clone())
                } else {
                    (
                        format!("{}.{k}", at.concrete),
                        format!("{}.{k}", at.canonical),
                    )
                };
                walk(
                    child,
                    &At {
                        concrete,
                        canonical,
                        depth: at.depth,
                        len: at.len,
                        element: at.element,
                        in_array: false,
                    },
                    out,
                );
            }
        }
        _ => {}
    }
}

struct Entry {
    unit: Option<String>,
    note: Option<String>,
    provenance: Option<String>,
}

fn entry_for(units: Option<&serde_json::Map<String, Value>>, canonical: &str) -> Option<Entry> {
    let (_, v) = kshana::field_schema::lookup(units?, canonical)?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some(Entry {
        unit: s("unit"),
        note: s("note"),
        provenance: s("provenance"),
    })
}

/// Sort every number of `result` into figures, other outputs, inputs and columns.
/// `named` are the figures the programme asks the sheet to lead with; each must be a
/// computed scalar of this result, and one that is not is refused with the reason.
pub fn figures_of(result: &Value, named: &[String]) -> Result<RunFigures, String> {
    let units = result.get("units").and_then(Value::as_object);
    let no_provenance = !units.is_some_and(|u| {
        u.values()
            .any(|e| e.get("provenance").and_then(Value::as_str).is_some())
    });
    let mut leaves = Vec::new();
    walk(
        result,
        &At {
            concrete: String::new(),
            canonical: String::new(),
            depth: 0,
            len: 0,
            element: None,
            in_array: false,
        },
        &mut leaves,
    );

    let mut out = RunFigures {
        no_provenance,
        ..Default::default()
    };
    let mut columns: BTreeMap<String, SeriesRow> = BTreeMap::new();
    for leaf in &leaves {
        let Some(e) = entry_for(units, &leaf.canonical) else {
            out.unlabelled_numeric_fields += 1;
            continue;
        };
        let Some(provenance) = e.provenance.clone() else {
            out.unlabelled_numeric_fields += 1;
            continue;
        };
        let scalar = leaf.depth == 0
            || (leaf.depth == 1 && !leaf.in_numeric_array && leaf.len <= MAX_EXPANDED_ELEMENTS);
        if scalar {
            let row = FigureRow {
                path: leaf.concrete.clone(),
                value: leaf.value,
                unit: e.unit,
                note: e.note,
                element: leaf.element.clone(),
                provenance: provenance.clone(),
            };
            if provenance == "computed" {
                out.figures.push(row);
            } else if is_input_class(&provenance) {
                out.inputs.push(row);
            } else {
                out.other_outputs.push(row);
            }
        } else if provenance == "computed" {
            columns
                .entry(leaf.canonical.clone())
                .and_modify(|s| {
                    s.count += 1;
                    s.min = s.min.min(leaf.value);
                    s.max = s.max.max(leaf.value);
                })
                .or_insert(SeriesRow {
                    path: leaf.canonical.clone(),
                    unit: e.unit,
                    note: e.note,
                    provenance,
                    count: 1,
                    min: leaf.value,
                    max: leaf.value,
                });
        }
    }
    out.series = columns.into_values().collect();

    if named.is_empty() {
        out.headline = out.figures.clone();
    } else {
        for n in named {
            if let Some(f) = out.figures.iter().find(|f| &f.path == n) {
                out.headline.push(f.clone());
                continue;
            }
            let other = out
                .inputs
                .iter()
                .chain(out.other_outputs.iter())
                .find(|f| &f.path == n);
            return Err(match other {
                Some(f) => format!(
                    "'{n}' is not a figure of this run: the result's units block gives it provenance `{}`, and only `computed` fields are figures",
                    f.provenance
                ),
                None => format!(
                    "'{n}' is not a computed scalar of this run's result (write an array element with its index, for example bands[3].value)"
                ),
            });
        }
    }
    Ok(out)
}

/// A column of a result document, for a chart.
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    Numbers(Vec<Option<f64>>),
    Labels(Vec<String>),
}

impl Column {
    fn len(&self) -> usize {
        match self {
            Column::Numbers(v) => v.len(),
            Column::Labels(v) => v.len(),
        }
    }
}

/// Read the column at `path`, following the engine's path grammar: a segment is a name,
/// optionally followed by `[]` (every element) or `[i]` (one element); an array met on
/// the way is walked element by element whether or not `[]` is written. Returns the
/// column and its canonical path (`[]` for each array level).
pub fn column(doc: &Value, path: &str) -> Result<(Column, String), String> {
    if path.trim().is_empty() {
        return Err("the path is empty".into());
    }
    let mut cur: Vec<&Value> = vec![doc];
    let mut canon: Vec<String> = Vec::new();
    for seg in path.split('.') {
        let (name, index) = match seg.find('[') {
            Some(i) if seg.ends_with(']') => (&seg[..i], Some(&seg[i + 1..seg.len() - 1])),
            Some(_) => return Err(format!("'{seg}' in '{path}' is not a path segment")),
            None => (seg, None),
        };
        if name.is_empty() {
            return Err(format!("'{path}' has an empty segment"));
        }
        let mut next: Vec<&Value> = Vec::new();
        let mut seg_canon = name.to_string();
        let mut found = false;
        for v in &cur {
            let Some(x) = v.get(name) else { continue };
            found = true;
            match (x, index) {
                (Value::Array(a), Some(ix)) if !ix.is_empty() => {
                    let i: usize = ix
                        .parse()
                        .map_err(|_| format!("'{seg}' in '{path}': the index is not a number"))?;
                    let e = a.get(i).ok_or_else(|| {
                        format!("'{seg}' in '{path}': the array has {} elements", a.len())
                    })?;
                    seg_canon = format!("{name}[]");
                    next.push(e);
                }
                (Value::Array(a), _) => {
                    seg_canon = format!("{name}[]");
                    next.extend(a.iter());
                }
                (_, Some(ix)) if !ix.is_empty() => {
                    return Err(format!("'{seg}' in '{path}': '{name}' is not an array"));
                }
                (other, _) => next.push(other),
            }
        }
        if !found {
            return Err(format!(
                "field '{path}' is not in the result document (no '{name}')"
            ));
        }
        canon.push(seg_canon);
        cur = next;
    }
    // A path that ends on rows of a table (an array of arrays, or one row picked by its
    // index) is read as the numbers in those rows.
    let mut canonical = canon.join(".");
    while !cur.is_empty() && cur.iter().all(|v| v.is_array()) {
        cur = cur
            .iter()
            .flat_map(|v| v.as_array().into_iter().flatten())
            .collect();
        canonical.push_str("[]");
    }
    if cur.iter().all(|v| v.is_string()) && !cur.is_empty() {
        return Ok((
            Column::Labels(
                cur.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect(),
            ),
            canonical,
        ));
    }
    let mut nums = Vec::with_capacity(cur.len());
    for v in &cur {
        match v {
            Value::Number(n) => nums.push(n.as_f64().filter(|x| x.is_finite())),
            Value::Null => nums.push(None),
            Value::Bool(b) => nums.push(Some(if *b { 1.0 } else { 0.0 })),
            _ => {
                return Err(format!(
                    "field '{path}' is not a column of numbers or of names"
                ))
            }
        }
    }
    Ok((Column::Numbers(nums), canonical))
}

/// One drawn series of a chart.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChartSeries {
    pub path: String,
    pub unit: Option<String>,
    pub note: Option<String>,
    pub values: Vec<Option<f64>>,
}

/// One chart of a run's sheet: the result's own columns, ready to draw.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChartData {
    pub title: String,
    /// `line` when `x` is a column of numbers, `bar` when it is a column of names.
    pub style: String,
    pub x_path: String,
    pub x_unit: Option<String>,
    /// The x values of a line chart.
    pub x: Vec<Option<f64>>,
    /// The category names of a bar chart.
    pub labels: Vec<String>,
    pub series: Vec<ChartSeries>,
    /// Number of points in the result's columns.
    pub points: usize,
    /// Every `stride`-th point is kept when the columns are longer than
    /// [`MAX_CHART_POINTS`]; 1 means every point is drawn.
    pub stride: usize,
}

fn thin<T: Clone>(v: &[T], stride: usize) -> Vec<T> {
    v.iter().step_by(stride).cloned().collect()
}

/// Build one chart from the result document. A path that is missing, not a column, or of
/// a different length than `x` is an error naming the path.
pub fn chart_of(result: &Value, spec: &ChartSpec) -> Result<ChartData, String> {
    let units = result.get("units").and_then(Value::as_object);
    let (xc, x_canon) = column(result, &spec.x).map_err(|e| format!("x: {e}"))?;
    let n = xc.len();
    if n == 0 {
        return Err(format!("x: '{}' is an empty column", spec.x));
    }
    let stride = n.div_ceil(MAX_CHART_POINTS).max(1);
    let mut series = Vec::new();
    for y in &spec.y {
        let (yc, y_canon) = column(result, y).map_err(|e| format!("y: {e}"))?;
        let Column::Numbers(vals) = yc else {
            return Err(format!("y: '{y}' is a column of names, not numbers"));
        };
        if vals.len() != n {
            return Err(format!(
                "y: '{y}' has {} values but x '{}' has {n}",
                vals.len(),
                spec.x
            ));
        }
        let e = entry_for(units, &y_canon);
        series.push(ChartSeries {
            path: y.clone(),
            unit: e.as_ref().and_then(|e| e.unit.clone()),
            note: e.and_then(|e| e.note),
            values: thin(&vals, stride),
        });
    }
    let x_unit = entry_for(units, &x_canon).and_then(|e| e.unit);
    let (style, x, labels) = match xc {
        Column::Numbers(v) => ("line", thin(&v, stride), vec![]),
        Column::Labels(l) => ("bar", vec![], thin(&l, stride)),
    };
    Ok(ChartData {
        title: spec.title.clone(),
        style: style.to_string(),
        x_path: spec.x.clone(),
        x_unit,
        x,
        labels,
        series,
        points: n,
        stride,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> Value {
        json!({
            "body": { "mu": 4902.8 },
            "inputs": { "mask_deg": 10.0 },
            "global": { "availability_pct": 21.3, "pdop": { "median": 5.2 } },
            "bands": [
                { "name": "a", "min_cn0": 3.2, "t": [0.0, 1.0] },
                { "name": "b", "min_cn0": 44.1, "t": [0.0, 1.0] }
            ],
            "t_s": [0.0, 10.0, 20.0],
            "err": [1.0, null, 3.0],
            "modelled_out": 7.5,
            "stray": 1.0,
            "units": {
                "body.mu": { "unit": "km^3/s^2", "provenance": "constant" },
                "inputs.mask_deg": { "unit": "deg", "provenance": "input" },
                "global.availability_pct": { "unit": "%", "provenance": "computed" },
                "global.pdop.median": { "unit": "1", "provenance": "computed" },
                "bands[].min_cn0": { "unit": "dB-Hz", "provenance": "computed" },
                "bands[].t[]": { "unit": "s", "provenance": "computed" },
                "t_s[]": { "unit": "s", "provenance": "computed" },
                "err[]": { "unit": "ns", "provenance": "computed" },
                "modelled_out": { "unit": "min", "provenance": "modelled" }
            }
        })
    }

    #[test]
    fn only_computed_scalars_are_figures() {
        let f = figures_of(&doc(), &[]).unwrap();
        let paths: Vec<&str> = f.figures.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "bands[0].min_cn0",
                "bands[1].min_cn0",
                "global.availability_pct",
                "global.pdop.median"
            ]
        );
        assert_eq!(f.figures[1].element.as_deref(), Some("b"));
        assert_eq!(
            f.inputs.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            vec!["body.mu", "inputs.mask_deg"]
        );
        assert_eq!(f.other_outputs.len(), 1);
        assert_eq!(f.other_outputs[0].provenance, "modelled");
        assert_eq!(f.unlabelled_numeric_fields, 1);
        let err = f.series.iter().find(|s| s.path == "err[]").unwrap();
        assert_eq!((err.count, err.min, err.max), (2, 1.0, 3.0));
        assert!(f.series.iter().any(|s| s.path == "bands[].t[]"));
    }

    #[test]
    fn a_named_input_is_refused_as_a_figure() {
        let e = figures_of(&doc(), &["inputs.mask_deg".into()]).unwrap_err();
        assert!(e.contains("provenance `input`"), "{e}");
        assert!(figures_of(&doc(), &["nope".into()]).is_err());
        let ok = figures_of(&doc(), &["global.availability_pct".into()]).unwrap();
        assert_eq!(ok.headline.len(), 1);
        assert_eq!(ok.figures.len(), 4);
    }

    #[test]
    fn a_result_without_provenance_has_no_figures() {
        let f = figures_of(&json!({"a": 1.0, "units": {"a": {"unit": "m"}}}), &[]).unwrap();
        assert!(f.no_provenance);
        assert!(f.figures.is_empty());
        assert_eq!(f.unlabelled_numeric_fields, 1);
    }

    #[test]
    fn columns_follow_the_path_grammar() {
        let d = doc();
        assert_eq!(
            column(&d, "err").unwrap(),
            (
                Column::Numbers(vec![Some(1.0), None, Some(3.0)]),
                "err[]".to_string()
            )
        );
        assert_eq!(
            column(&d, "bands[].name").unwrap().0,
            Column::Labels(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            column(&d, "bands[1].min_cn0").unwrap(),
            (
                Column::Numbers(vec![Some(44.1)]),
                "bands[].min_cn0".to_string()
            )
        );
        assert_eq!(
            column(&d, "bands[1].t").unwrap(),
            (
                Column::Numbers(vec![Some(0.0), Some(1.0)]),
                "bands[].t[]".to_string()
            )
        );
        assert!(column(&d, "bands[9].min_cn0").is_err());
        assert!(column(&d, "missing.path").is_err());
    }

    #[test]
    fn a_chart_needs_columns_of_one_length() {
        let d = doc();
        let ok = chart_of(
            &d,
            &ChartSpec {
                title: "t".into(),
                x: "t_s".into(),
                y: vec!["err".into()],
            },
        )
        .unwrap();
        assert_eq!(ok.style, "line");
        assert_eq!(ok.series[0].unit.as_deref(), Some("ns"));
        assert_eq!(ok.x_unit.as_deref(), Some("s"));
        let bar = chart_of(
            &d,
            &ChartSpec {
                title: "t".into(),
                x: "bands[].name".into(),
                y: vec!["bands[].min_cn0".into()],
            },
        )
        .unwrap();
        assert_eq!(bar.style, "bar");
        assert_eq!(bar.labels, vec!["a", "b"]);
        let e = chart_of(
            &d,
            &ChartSpec {
                title: "t".into(),
                x: "t_s".into(),
                y: vec!["bands[].min_cn0".into()],
            },
        )
        .unwrap_err();
        assert!(e.contains("has 2 values"), "{e}");
    }
}
