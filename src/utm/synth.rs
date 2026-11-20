// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic inputs for every `utm` command (`kshana-pro utm synth <dir>`): five day maps around
//! a 6 km survey route (degraded cells on two days), Remote ID records (one consistent
//! identity, one cloned), a clean flight and a flight with a fix gap, spoofing flags and a
//! position jump. All synthetic; nothing here is a real operation or a real place's data.

use std::path::Path;

use kshana::interference_map::grid::Grid;

/// Route start `(lat, lon)`.
pub const START: (f64, f64) = (52.10, 5.10);
/// Route end: about 6 km east.
pub const END: (f64, f64) = (52.10, 5.188);
/// Map days; the route's first cell is degraded on the 2nd and 4th.
pub const DAYS: [&str; 5] = [
    "2026-06-01",
    "2026-06-02",
    "2026-06-03",
    "2026-06-04",
    "2026-06-05",
];
/// Flight start, Unix seconds (2026-06-03T10:00:00Z, a day the route was not degraded).
pub const T0: f64 = 1_780_480_800.0;

/// One day map at 0.05° cells: the cells along the route, the first degraded on `degraded`.
pub fn map(date: &str, degraded: bool) -> String {
    let g = Grid::new(0.05).expect("grid");
    let mut feats = Vec::new();
    // Start just inside the route so a start on a cell edge still marks the route's own cell.
    let mut lon = START.1 + 0.001;
    let mut seen = Vec::new();
    while lon <= END.1 + 1e-9 {
        let c = g.cell_of(START.0, lon);
        if !seen.contains(&(c.i, c.j)) {
            let first = seen.is_empty();
            seen.push((c.i, c.j));
            let status = if first && degraded {
                "degraded"
            } else {
                "not_degraded"
            };
            feats.push(serde_json::json!({"type": "Feature", "geometry": null,
                "properties": {"cell_i": c.i, "cell_j": c.j, "status": status}}));
        }
        lon += 0.005;
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "type": "FeatureCollection",
        "kshana_interference_map": {"date": date, "source_kind": "adsb", "grid": {"cell_deg": 0.05},
            "data": {"licence": "synthetic", "attribution": "synthetic"}},
        "features": feats
    }))
    .unwrap_or_default()
}

/// Remote ID records: `SURVEY-1` flies the route at 10 m/s, reported once a second;
/// `CLONE-7` is reported 40 km apart within half a second.
pub fn rid_csv() -> String {
    let mut s = String::from("t_utc,uas_id,lat,lon,alt_m,speed_m_s,op_lat,op_lon\n");
    let m_per_deg = 111_320.0 * START.0.to_radians().cos();
    for k in 0..60 {
        let lon = START.1 + 10.0 * k as f64 / m_per_deg;
        s += &format!(
            "{},SURVEY-1,{:.7},{:.7},60.0,10.0,{},{}\n",
            T0 + k as f64,
            START.0,
            lon,
            START.0,
            START.1
        );
    }
    for k in 0..4 {
        let (lat, lon) = if k % 2 == 0 {
            (52.0, 5.0)
        } else {
            (52.36, 5.0)
        };
        s += &format!("{},CLONE-7,{lat},{lon},80.0,12.0,,\n", T0 + 0.5 * k as f64);
    }
    s
}

/// Telemetry of the flight: 1 Hz along the route at 10 m/s for 600 s. With `faulty`, a
/// 20 s stretch without a fix, receiver spoofing flags for 5 s and one 400 m jump.
pub fn telemetry_csv(faulty: bool) -> String {
    let mut s = String::from("t_utc,lat,lon,fix_type,eph_m,jamming_state,spoofing_state\n");
    let m_per_deg = 111_320.0 * START.0.to_radians().cos();
    for k in 0..600 {
        let mut lon = START.1 + 10.0 * k as f64 / m_per_deg;
        let (mut fix, mut spoof) = (3, 1);
        if faulty {
            if (200..220).contains(&k) {
                fix = 1;
            }
            if (300..305).contains(&k) {
                spoof = 2;
            }
            if k >= 400 {
                lon += 400.0 / m_per_deg;
            }
        }
        s += &format!(
            "{},{:.7},{:.7},{fix},1.2,1,{spoof}\n",
            T0 + k as f64,
            START.0,
            lon
        );
    }
    s
}

/// A request naming the files [`write_all`] writes.
pub fn request(telemetry: &str, feed: bool) -> String {
    let mut s = format!(
        "title = \"Synthetic survey operation\"\n\
         maps = [{}]\nrid = \"rid.csv\"\ntelemetry = \"{telemetry}\"\n\n\
         [operation]\nid = \"SURVEY-1\"\nuas_class = \"multirotor\"\nspeed_m_s = 10.0\nheight_agl_m = 60.0\n\
         waypoints = [[{}, {}], [{}, {}]]\ncontingency_m = 50.0\nground_risk_buffer_m = 100.0\n",
        DAYS.iter()
            .map(|d| format!("\"map-{d}.geojson\""))
            .collect::<Vec<_>>()
            .join(", "),
        START.0,
        START.1,
        END.0,
        END.1
    );
    if feed {
        s += "\n[feed]\narea = [52.0, 5.0, 52.2, 5.3]\nvalid_hours = 24.0\n";
    }
    s
}

/// Write every synthetic input and the requests `sora.toml`, `feed.toml`, `rid.toml`,
/// `attest-clean.toml` and `attest-faulty.toml` into `dir`.
pub fn write_all(dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    for (k, d) in DAYS.iter().enumerate() {
        std::fs::write(
            dir.join(format!("map-{d}.geojson")),
            map(d, k == 1 || k == 3),
        )
        .map_err(io)?;
    }
    std::fs::write(dir.join("rid.csv"), rid_csv()).map_err(io)?;
    std::fs::write(dir.join("telemetry-clean.csv"), telemetry_csv(false)).map_err(io)?;
    std::fs::write(dir.join("telemetry-faulty.csv"), telemetry_csv(true)).map_err(io)?;
    for (name, tel, feed) in [
        ("sora.toml", "telemetry-clean.csv", false),
        ("feed.toml", "telemetry-clean.csv", true),
        ("rid.toml", "telemetry-clean.csv", false),
        ("attest-clean.toml", "telemetry-clean.csv", false),
        ("attest-faulty.toml", "telemetry-faulty.csv", false),
    ] {
        std::fs::write(dir.join(name), request(tel, feed)).map_err(io)?;
    }
    Ok(())
}
