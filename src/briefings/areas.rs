// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Named areas: chokepoints, sea and air regions, listed areas, declared test areas.
//!
//! The built-in areas ([`builtin`]) are coarse boxes on a 0.5° raster drawn for exposure
//! accounting only. They are **not** legal, listed or charted boundaries. Market listed areas
//! (for example war-risk listed areas) are published under their owners' terms, so they are
//! not shipped here: a user supplies them as an area file with `kind = "listed"` and the
//! source they took them from, and every record names that source.
//!
//! Area file (TOML):
//!
//! ```toml
//! [[area]]
//! id = "jwla-example"
//! name = "Example listed area"
//! kind = "listed"            # chokepoint | region | listed | custom | test-area
//! source = "Listed-area circular, as published on 2026-06-01"
//! polygon = [[12.0, 42.5], [13.5, 42.5], [13.5, 44.0], [12.0, 44.0]]   # [lat, lon]
//! # or: bbox = [south, west, north, east]
//! ```

use serde::{Deserialize, Serialize};

/// A named area: a simple polygon of `[lat, lon]` vertices (not closed; the last joins the
/// first).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub polygon: Vec<[f64; 2]>,
    /// Where the boundary comes from.
    pub source: String,
}

/// The source note of every built-in area.
pub const BUILTIN_SOURCE: &str = "Kshana Pro built-in box on a 0.5 degree raster, drawn for \
exposure accounting; not a legal, listed or charted boundary.";

const KINDS: &[&str] = &["chokepoint", "region", "listed", "custom", "test-area"];

fn bx(id: &str, name: &str, kind: &str, s: f64, w: f64, n: f64, e: f64) -> Area {
    Area {
        id: id.into(),
        name: name.into(),
        kind: kind.into(),
        polygon: vec![[s, w], [n, w], [n, e], [s, e]],
        source: BUILTIN_SOURCE.into(),
    }
}

/// The built-in chokepoints and regions.
pub fn builtin() -> Vec<Area> {
    vec![
        bx(
            "hormuz",
            "Strait of Hormuz",
            "chokepoint",
            25.5,
            55.5,
            27.0,
            57.5,
        ),
        bx(
            "bab-el-mandeb",
            "Bab el-Mandeb",
            "chokepoint",
            12.0,
            42.5,
            13.5,
            44.0,
        ),
        bx(
            "dover-strait",
            "Dover Strait",
            "chokepoint",
            50.5,
            0.5,
            51.5,
            2.0,
        ),
        bx(
            "bosphorus",
            "Bosphorus approaches",
            "chokepoint",
            40.5,
            28.5,
            41.5,
            29.5,
        ),
        bx(
            "suez",
            "Suez Canal and approaches",
            "chokepoint",
            29.5,
            32.0,
            31.5,
            33.0,
        ),
        bx(
            "gibraltar",
            "Strait of Gibraltar",
            "chokepoint",
            35.5,
            -6.0,
            36.5,
            -5.0,
        ),
        bx(
            "singapore-strait",
            "Singapore Strait",
            "chokepoint",
            1.0,
            103.5,
            1.5,
            104.5,
        ),
        bx(
            "panama-canal",
            "Panama Canal approaches",
            "chokepoint",
            8.5,
            -80.0,
            9.5,
            -79.5,
        ),
        bx(
            "danish-straits",
            "Danish straits (Oresund and Belts)",
            "chokepoint",
            54.5,
            10.5,
            56.5,
            13.0,
        ),
        bx("baltic", "Baltic Sea", "region", 53.5, 9.5, 66.0, 30.5),
        bx("gulf", "The Gulf", "region", 23.5, 47.5, 30.5, 57.0),
        bx(
            "red-sea-jeddah",
            "Red Sea off Jeddah",
            "region",
            20.5,
            38.0,
            22.5,
            40.0,
        ),
        bx(
            "black-sea-nw",
            "North-western Black Sea and Danube mouths",
            "region",
            43.5,
            28.5,
            46.5,
            31.5,
        ),
        bx(
            "eastern-med",
            "Eastern Mediterranean",
            "region",
            31.0,
            32.0,
            37.0,
            36.5,
        ),
        bx(
            "caribbean",
            "Caribbean Sea",
            "region",
            9.0,
            -85.0,
            23.5,
            -59.5,
        ),
        bx(
            "canary-islands",
            "Canary Islands",
            "region",
            27.5,
            -18.5,
            29.5,
            -13.0,
        ),
        bx(
            "india-nw-border",
            "India north-western border area",
            "region",
            28.0,
            72.0,
            33.0,
            76.0,
        ),
        bx(
            "delhi-tma",
            "Delhi terminal area",
            "region",
            28.0,
            76.5,
            29.0,
            77.5,
        ),
        bx(
            "korea-west",
            "Korea western coast and Yellow Sea border area",
            "region",
            37.0,
            124.5,
            38.5,
            126.5,
        ),
        bx(
            "korea-dmz",
            "Korea demilitarised-zone corridor",
            "region",
            37.5,
            126.0,
            38.5,
            129.0,
        ),
        bx("finnmark", "Finnmark", "region", 69.0, 22.0, 71.5, 31.5),
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AreaSpec {
    id: String,
    name: String,
    #[serde(default = "custom")]
    kind: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    polygon: Option<Vec<[f64; 2]>>,
    #[serde(default)]
    bbox: Option<[f64; 4]>,
}

fn custom() -> String {
    "custom".into()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AreaFile {
    #[serde(default)]
    area: Vec<AreaSpec>,
}

/// Parse an area file (TOML); every area is checked ([`Area::problems`]).
pub fn parse_areas(text: &str) -> Result<Vec<Area>, String> {
    let f: AreaFile = toml::from_str(text).map_err(|e| format!("invalid area file: {e}"))?;
    let mut out = vec![];
    for a in f.area {
        let polygon = match (a.polygon, a.bbox) {
            (Some(p), None) => p,
            (None, Some([s, w, n, e])) => vec![[s, w], [n, w], [n, e], [s, e]],
            _ => {
                return Err(format!(
                    "area {}: give exactly one of polygon or bbox",
                    a.id
                ))
            }
        };
        let area = Area {
            id: a.id,
            name: a.name,
            kind: a.kind,
            polygon,
            source: if a.source.trim().is_empty() {
                "user-supplied; no source stated".into()
            } else {
                a.source
            },
        };
        let p = area.problems();
        if !p.is_empty() {
            return Err(format!("area {}: {}", area.id, p.join("; ")));
        }
        out.push(area);
    }
    Ok(out)
}

/// The built-in areas (unless `builtin_off`) followed by the file's; a file area replaces a
/// built-in one of the same id.
pub fn catalogue(file_areas: Vec<Area>, builtin_off: bool) -> Vec<Area> {
    let mut v: Vec<Area> = if builtin_off { vec![] } else { builtin() };
    for a in file_areas {
        v.retain(|b| b.id != a.id);
        v.push(a);
    }
    v
}

/// Pick areas by id, in the order given.
pub fn select(all: &[Area], ids: &[String]) -> Result<Vec<Area>, String> {
    ids.iter()
        .map(|id| {
            all.iter()
                .find(|a| &a.id == id)
                .cloned()
                .ok_or_else(|| format!("no area '{id}' (built-in ids: kshana-pro briefing areas)"))
        })
        .collect()
}

impl Area {
    /// What is wrong with the area.
    pub fn problems(&self) -> Vec<String> {
        let mut p = vec![];
        if self.id.trim().is_empty() {
            p.push("id is empty".into());
        }
        if !KINDS.contains(&self.kind.as_str()) {
            p.push(format!("kind must be one of {}", KINDS.join(", ")));
        }
        if self.polygon.len() < 3 {
            p.push("a polygon needs at least three vertices".into());
        }
        if self
            .polygon
            .iter()
            .any(|v| !(-90.0..=90.0).contains(&v[0]) || !(-180.0..=180.0).contains(&v[1]))
        {
            p.push("a vertex is out of range".into());
        }
        p
    }

    /// `(south, west, north, east)`.
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut b = (90.0_f64, 180.0_f64, -90.0_f64, -180.0_f64);
        for v in &self.polygon {
            b.0 = b.0.min(v[0]);
            b.1 = b.1.min(v[1]);
            b.2 = b.2.max(v[0]);
            b.3 = b.3.max(v[1]);
        }
        b
    }

    /// Whether `(lat, lon)` lies inside (ray casting; points on an edge may go either way).
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        point_in_polygon(&self.polygon, lat, lon)
    }

    /// The cells of a grid whose centre lies inside the area.
    pub fn cells(
        &self,
        grid: &kshana::interference_map::grid::Grid,
    ) -> Vec<kshana::interference_map::grid::CellId> {
        let (s, w, n, e) = self.bbox();
        let (a, b) = (grid.cell_of(s, w), grid.cell_of(n, e));
        let mut out = vec![];
        for i in a.i..=b.i {
            for j in a.j..=b.j {
                let c = kshana::interference_map::grid::CellId { i, j };
                let (la, lo) = grid.center(c);
                if self.contains(la, lo) {
                    out.push(c);
                }
            }
        }
        out
    }
}

/// Ray-casting point in polygon over `[lat, lon]` vertices.
pub fn point_in_polygon(poly: &[[f64; 2]], lat: f64, lon: f64) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (yi, xi) = (poly[i][0], poly[i][1]);
        let (yj, xj) = (poly[j][0], poly[j][1]);
        if (yi > lat) != (yj > lat) && lon < (xj - xi) * (lat - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The built-in catalogue as an area file (TOML), for `kshana-pro briefing areas`.
pub fn builtin_toml() -> String {
    let mut s = String::from(
        "# Kshana Pro built-in areas: coarse boxes on a 0.5 degree raster for exposure accounting.\n\
         # Not legal, listed or charted boundaries. Copy and edit, or add your own with kind = \"listed\".\n",
    );
    for a in builtin() {
        let (so, w, n, e) = a.bbox();
        s.push_str(&format!(
            "\n[[area]]\nid = \"{}\"\nname = \"{}\"\nkind = \"{}\"\nbbox = [{so:.1}, {w:.1}, {n:.1}, {e:.1}]\n",
            a.id, a.name, a.kind
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_boxes_are_coarse_and_valid() {
        for a in builtin() {
            assert!(a.problems().is_empty(), "{}", a.id);
            let (s, w, n, e) = a.bbox();
            for v in [s, w, n, e] {
                assert_eq!(
                    (v * 2.0).fract(),
                    0.0,
                    "{} is on the 0.5 degree raster",
                    a.id
                );
            }
            assert!(n - s >= 0.5 && e - w >= 0.5, "{}", a.id);
        }
        let t = builtin_toml();
        assert_eq!(parse_areas(&t).unwrap().len(), builtin().len());
    }

    #[test]
    fn polygon_contains_and_file_overrides() {
        let a = &builtin()[0];
        assert!(a.contains(26.5, 56.5));
        assert!(!a.contains(24.0, 56.5));
        let f = parse_areas(
            "[[area]]\nid = \"hormuz\"\nname = \"Mine\"\nkind = \"listed\"\nsource = \"own\"\nbbox = [25.0, 55.0, 27.0, 58.0]\n",
        )
        .unwrap();
        let c = catalogue(f, false);
        assert_eq!(c.iter().filter(|a| a.id == "hormuz").count(), 1);
        assert_eq!(select(&c, &["hormuz".into()]).unwrap()[0].kind, "listed");
        assert!(parse_areas(
            "[[area]]\nid = \"x\"\nname = \"x\"\nkind = \"nope\"\nbbox = [0.0, 0.0, 1.0, 1.0]\n"
        )
        .is_err());
    }
}
