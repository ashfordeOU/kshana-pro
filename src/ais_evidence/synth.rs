// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic AIS scenario with known truth (SYNTHETIC; for tests and trying the packs).
//!
//! Seven vessels over twelve hours from 2026-06-01T00:00Z, reports every ten minutes:
//!
//! | MMSI | truth |
//! |---|---|
//! | 211000001 | steams east, goes silent for 3.5 h, then meets 211000002 for 3.5 h at low speed; renamed in static data |
//! | 211000002 | waits at the meeting point |
//! | 211000003 | 'engaged in fishing' at 3 kn inside the closed zone `Z1` for 5 h |
//! | 211000004 | 'at anchor' while moving at 1.5 kn along the cable corridor `C1` for 2 h around the fault |
//! | 211000005 | AIS offset 2 km from its true track for 2 h; the radar (with declared MMSI) sees the true track |
//! | 211000006 | AIS-only target inside radar coverage, never seen by the radar |
//! | 211000007 | one MMSI on two transmitters 60 km apart for an hour |
//!
//! The radar also tracks `R9`, a target with no AIS.

use crate::forensics::inputs::iso;
use crate::transport::offset_deg;

/// Scenario start, unix seconds (2026-06-01T00:00:00Z).
pub const T0: f64 = 1_780_272_000.0;
const STEP: f64 = 600.0;
const KNM: f64 = 1852.0 / 3600.0;

/// The generated files.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub positions_csv: String,
    pub static_csv: String,
    pub radar_csv: String,
    pub consistency_toml: String,
    pub sanctions_toml: String,
    pub iuu_toml: String,
    pub subsea_toml: String,
}

struct W(String);

impl W {
    #[allow(clippy::too_many_arguments)]
    fn row(&mut self, t: f64, m: &str, lat: f64, lon: f64, sog: f64, nav: u8, src: &str) {
        self.0.push_str(&format!(
            "{},{m},{lat:.6},{lon:.6},{sog:.1},90.0,{nav},{src}\n",
            iso(t)
        ));
    }
}

/// Build the scenario.
pub fn make() -> Scenario {
    let mut p = W("timestamp,mmsi,lat,lon,sog_kn,cog_deg,nav_status,source\n".into());
    let mut radar = String::from("timestamp,track_id,lat,lon,mmsi\n");
    let n = (12.0 * 3600.0 / STEP) as usize;
    let meet = (35.0, 20.3);
    for k in 0..=n {
        let t = T0 + k as f64 * STEP;
        let h = (t - T0) / 3600.0;
        // 1: east at 12 kn from (35.0, 19.9) for 3 h, silent 3-6.5 h, then at the meeting point.
        if h <= 3.0 {
            let (la, lo) = offset_deg(35.0, 19.9, 12.0 * KNM * (t - T0) - 60_000.0, 0.0);
            p.row(t, "211000001", la, lo, 12.0, 0, "terrestrial");
        } else if (6.5..=10.0).contains(&h) {
            let (la, lo) = offset_deg(meet.0, meet.1, 150.0, 0.0);
            p.row(t, "211000001", la, lo, 0.4, 0, "satellite");
        } else if h > 10.0 {
            let (la, lo) = offset_deg(
                meet.0,
                meet.1,
                150.0 + 10.0 * KNM * (t - T0 - 36_000.0),
                0.0,
            );
            p.row(t, "211000001", la, lo, 10.0, 0, "satellite");
        }
        // 2: waits at the meeting point.
        p.row(t, "211000002", meet.0, meet.1, 0.3, 0, "satellite");
        // 3: fishing inside Z1 (36.0..36.2, 21.0..21.2) from h 2 to h 7, transiting otherwise.
        let (la, lo) = offset_deg(35.9, 21.1, 0.0, 3.0 * KNM * (t - T0));
        let (st, nav) = if (2.0..=7.0).contains(&h) {
            (3.0, 7)
        } else {
            (3.0, 0)
        };
        p.row(t, "211000003", la, lo, st, nav, "terrestrial");
        // 4: along cable C1 (34.5, 19.5)-(34.5, 20.5); at anchor while moving 7-9 h.
        let (la, lo, sog, nav) = if (7.0..=9.0).contains(&h) {
            let (a, b) = offset_deg(34.5, 20.05, 1.5 * KNM * (t - T0 - 7.0 * 3600.0), 200.0);
            (a, b, 1.5, 1)
        } else if h < 7.0 {
            let (a, b) = offset_deg(
                34.5,
                20.05,
                0.0,
                200.0 - 8.0 * KNM * (T0 + 7.0 * 3600.0 - t),
            );
            (a, b, 8.0, 0)
        } else {
            let (a, b) = offset_deg(
                34.5,
                20.05,
                1.5 * KNM * 7200.0,
                200.0 + 8.0 * KNM * (t - T0 - 9.0 * 3600.0),
            );
            (a, b, 8.0, 0)
        };
        p.row(t, "211000004", la, lo, sog, nav, "terrestrial");
        // 5: true track north at 10 kn from (33.8, 21.0); AIS offset 2 km east 4-6 h.
        let (tla, tlo) = offset_deg(33.8, 21.0, 0.0, 10.0 * KNM * (t - T0));
        let (ala, alo) = if (4.0..=6.0).contains(&h) {
            offset_deg(tla, tlo, 2000.0, 0.0)
        } else {
            (tla, tlo)
        };
        p.row(t, "211000005", ala, alo, 10.0, 0, "terrestrial");
        radar.push_str(&format!("{},V5,{tla:.6},{tlo:.6},211000005\n", iso(t)));
        // 6: AIS-only target inside radar coverage.
        let (la, lo) = offset_deg(33.9, 21.2, 0.0, 6.0 * KNM * (t - T0));
        p.row(t, "211000006", la, lo, 6.0, 0, "terrestrial");
        // R9: radar target without AIS.
        let (la, lo) = offset_deg(34.2, 20.85, 5.0 * KNM * (t - T0), 0.0);
        radar.push_str(&format!("{},R9,{la:.6},{lo:.6},\n", iso(t)));
        // 7: two transmitters on one MMSI for an hour (h 5-6), alternating.
        if (5.0..=6.0).contains(&h) {
            let far = k % 2 == 1;
            let (la, lo) = if far { (36.5, 19.0) } else { (36.5, 19.7) };
            p.row(t, "211000007", la, lo, 0.0, 5, "terrestrial");
        }
    }
    let static_csv = format!(
        "timestamp,mmsi,imo,name,callsign,flag,ship_type\n\
         {},211000001,9000001,EXAMPLE TANKER ONE,XX0001,ZZ,80\n\
         {},211000001,9000001,EXAMPLE TANKER RENAMED,XX0001,ZZ,80\n\
         {},211000002,9000002,EXAMPLE TANKER TWO,XX0002,ZZ,80\n\
         {},211000006,9000001,EXAMPLE PHANTOM,XX0006,ZZ,70\n",
        iso(T0),
        iso(T0 + 7.0 * 3600.0),
        iso(T0),
        iso(T0)
    );
    let consistency_toml = "title = \"SYNTHETIC cross-source consistency\"\n\
ais = \"positions.csv\"\n\
ais_sigma_m = 30\n\
\n\
[[sources]]\n\
name = \"coastal-radar\"\n\
kind = \"radar\"\n\
file = \"radar.csv\"\n\
sigma_m = 150\n\
coverage = { name = \"radar-coverage\", kind = \"coverage\", polygon = [[33.7, 20.8], [34.4, 20.8], [34.4, 21.4], [33.7, 21.4]] }\n"
        .to_string();
    let sanctions_toml = "title = \"SYNTHETIC dark-activity review\"\n\
ais = \"positions.csv\"\n\
static = \"static.csv\"\n\
vessels = [\"211000001\", \"211000007\"]\n"
        .to_string();
    let iuu_toml = "title = \"SYNTHETIC fishing-activity review\"\n\
ais = \"positions.csv\"\n\
vessels = [\"211000003\"]\n\
\n\
[[zones]]\n\
name = \"Z1\"\n\
kind = \"closed-area\"\n\
polygon = [[36.0, 21.0], [36.2, 21.0], [36.2, 21.2], [36.0, 21.2]]\n"
        .to_string();
    let subsea_toml = format!(
        "title = \"SYNTHETIC subsea-asset review\"\n\
ais = \"positions.csv\"\n\
\n\
[[assets]]\n\
name = \"C1\"\n\
kind = \"telecom-cable\"\n\
buffer_m = 1000\n\
path = [[34.5, 19.5], [34.5, 20.5]]\n\
\n\
[incident]\n\
utc = \"{}\"\n\
lat = 34.5\n\
lon = 20.08\n\
window_h = 6\n\
radius_m = 5000\n",
        iso(T0 + 8.5 * 3600.0)
    );
    Scenario {
        positions_csv: p.0,
        static_csv,
        radar_csv: radar,
        consistency_toml,
        sanctions_toml,
        iuu_toml,
        subsea_toml,
    }
}

/// Write the scenario into `dir`.
pub fn write(dir: &std::path::Path) -> Result<(), String> {
    let s = make();
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    for (n, c) in [
        ("positions.csv", &s.positions_csv),
        ("static.csv", &s.static_csv),
        ("radar.csv", &s.radar_csv),
        ("consistency.toml", &s.consistency_toml),
        ("sanctions.toml", &s.sanctions_toml),
        ("iuu.toml", &s.iuu_toml),
        ("subsea.toml", &s.subsea_toml),
    ] {
        std::fs::write(dir.join(n), c).map_err(io)?;
    }
    Ok(())
}
