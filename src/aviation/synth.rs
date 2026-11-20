// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic inputs with known truth for the aviation analytics (tests and `aviation synth`).
//! Every place, aircraft and time here is invented.

use std::path::Path;

use crate::forensics::inputs::iso;

/// 2026-06-01T08:00:00Z.
pub const T0: f64 = 1_780_300_800.0;
/// The synthetic airport (GNSS-only: two RNP approaches).
pub const AIRPORT: (f64, f64, f64) = (42.07, 24.85, 600.0);
/// The terminal-area outage, seconds after [`T0`]: one hour.
pub const OUTAGE: (f64, f64) = (3_900.0, 7_500.0);
/// Arrival spacing (s) and count.
pub const ARRIVAL_EVERY_S: f64 = 300.0;
pub const ARRIVALS: usize = 48;
/// Arrivals (index) that go around: three inside the outage, one outside.
pub const GO_AROUNDS: [usize; 4] = [14, 16, 19, 33];
/// The oceanic box and the spoofed aircraft's not-supported span (s after its start).
pub const OCEANIC_BOX: [f64; 4] = [55.0, -30.0, 60.0, -20.0];
pub const OCEANIC_SPOOF: (f64, f64) = (600.0, 1_800.0);

const R: f64 = 6_371_000.0;

fn east(lat: f64, lon: f64, nm: f64) -> (f64, f64) {
    (
        lat,
        lon + (nm * 1852.0 / (R * lat.to_radians().cos())).to_degrees(),
    )
}

fn north(lat: f64, lon: f64, m: f64) -> (f64, f64) {
    (lat + (m / R).to_degrees(), lon)
}

fn row(s: &mut String, t: f64, id: &str, p: (f64, f64), alt: f64, nic: u8, nacp: u8) {
    s.push_str(&format!(
        "{},{id},{:.6},{:.6},{:.0},{nic},{nacp}\n",
        iso(t),
        p.0,
        p.1,
        alt
    ));
}

/// Terminal-area ADS-B: 48 arrivals from the east every 5 minutes, four departures, the
/// outage within 25 NM during [`OUTAGE`], four go-arounds, and three oceanic crossings in
/// [`OCEANIC_BOX`] of which one is spoofed (a 30 km jump, then no usable accuracy for 20
/// minutes).
pub fn approach_adsb() -> String {
    let (alat, alon, elev) = AIRPORT;
    let mut s = String::from("timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp\n");
    let jammed = |t: f64, d: f64| t >= T0 + OUTAGE.0 && t < T0 + OUTAGE.1 && d.abs() <= 25.0;
    let step = 250.0 / 3600.0 * 10.0; // NM per 10 s at 250 kt
    for i in 0..ARRIVALS {
        let id = format!("ARR{i:02}");
        let ga = GO_AROUNDS.contains(&i);
        let t_start = T0 + i as f64 * ARRIVAL_EVERY_S;
        let mut d: f64 = 40.0;
        let mut t = t_start;
        let mut alt: f64;
        loop {
            alt = elev + (d * 318.0f64).min(9_000.0);
            let (nic, nacp) = if jammed(t, d) { (0, 0) } else { (8, 9) };
            row(&mut s, t, &id, east(alat, alon, d), alt, nic, nacp);
            if ga && d <= 3.0 {
                break;
            }
            if d <= 0.0 {
                break;
            }
            d = (d - step).max(0.0);
            t += 10.0;
        }
        if ga {
            // Go around: climb at 1,500 ft/min straight ahead to 15 NM west.
            while d > -15.0 {
                d -= step;
                t += 10.0;
                alt += 250.0;
                let (nic, nacp) = if jammed(t, d) { (0, 0) } else { (8, 9) };
                row(&mut s, t, &id, east(alat, alon, d), alt, nic, nacp);
            }
        }
    }
    for k in 0..4 {
        let id = format!("DEP{k}");
        let mut t = T0 + 900.0 + k as f64 * 3_000.0;
        let (mut d, mut alt) = (0.0, elev);
        while d > -35.0 {
            let (nic, nacp) = if jammed(t, d) { (0, 0) } else { (8, 9) };
            row(&mut s, t, &id, east(alat, alon, d), alt, nic, nacp);
            d -= step;
            alt = (alt + 333.0).min(elev + 12_000.0);
            t += 10.0;
        }
    }
    for (k, lat) in [57.0f64, 58.0, 59.0].into_iter().enumerate() {
        let id = format!("OCN{k}");
        let spoofed = k == 1;
        let mut p: (f64, f64) = (lat, -20.1);
        for j in 0..120 {
            let ts = j as f64 * 30.0;
            let t = T0 + ts;
            p = (
                p.0,
                p.1 - (480.0 * 1852.0 / 3600.0 * 30.0 / (R * p.0.to_radians().cos())).to_degrees(),
            );
            let shown = if spoofed && ts >= OCEANIC_SPOOF.0 && ts < OCEANIC_SPOOF.1 {
                north(p.0, p.1, 30_000.0)
            } else {
                p
            };
            let bad = spoofed && ts >= OCEANIC_SPOOF.0 && ts < OCEANIC_SPOOF.1;
            let (nic, nacp) = if bad { (0, 0) } else { (7, 8) };
            row(&mut s, t, &id, shown, 37_000.0, nic, nacp);
        }
    }
    s
}

/// An open interference map (v1 GeoJSON) for the outage day with the airport's cell degraded.
pub fn approach_map() -> String {
    let g = kshana::interference_map::grid::Grid::new(0.5).expect("grid");
    let c = g.cell_of(AIRPORT.0, AIRPORT.1);
    let n = g.cell_of(AIRPORT.0 + 0.5, AIRPORT.1);
    serde_json::to_string_pretty(&serde_json::json!({
        "type": "FeatureCollection",
        "kshana_interference_map": {
            "date": "2026-06-01", "source_kind": "adsb", "grid": {"cell_deg": 0.5},
            "data": {"licence": "synthetic", "attribution": "synthetic"}
        },
        "features": [
            {"type": "Feature", "geometry": null, "properties": {"cell_i": c.i, "cell_j": c.j, "status": "degraded"}},
            {"type": "Feature", "geometry": null, "properties": {"cell_i": n.i, "cell_j": n.j, "status": "not_degraded"}}
        ]
    }))
    .unwrap_or_default()
}

/// The approach request matching [`approach_adsb`] and [`approach_map`].
pub fn approach_request() -> String {
    let (alat, alon, elev) = AIRPORT;
    let faf = east(alat, alon, 5.0);
    let o = OCEANIC_BOX;
    format!(
        "title = \"Synthetic one-hour terminal outage at a GNSS-only airport\"\n\
         adsb = [\"adsb.csv\"]\nmaps = [\"map-2026-06-01.geojson\"]\n\n\
         [airport]\nid = \"ZZAP\"\nlat = {alat}\nlon = {alon}\nelevation_ft = {elev}\n\n\
         [[procedures]]\nid = \"RNP 27\"\nkind = \"rnp\"\nrunway = \"27\"\nfaf = [{:.6}, {:.6}]\nthreshold = [{alat}, {alon}]\n\n\
         [[procedures]]\nid = \"RNP 09\"\nkind = \"lpv\"\nrunway = \"09\"\n\n\
         [capacity]\nnominal_arrivals_per_hour = 12\n\n\
         [oceanic]\nname = \"Synthetic oceanic area\"\nbox = [{}, {}, {}, {}]\nrnp_nm = 4.0\n",
        faf.0, faf.1, o[0], o[1], o[2], o[3]
    )
}

/// One flight-data export in the [`crate::fdm`] format, cruising at FL350, 1 Hz.
#[derive(Clone, Debug)]
pub struct FdmFlight {
    pub t0: f64,
    pub secs: usize,
    pub start: (f64, f64),
    pub heading: f64,
    /// Spoofing window (s after `t0`): position 60 km north, GNSS altitude +2,000 ft, GNSS
    /// ground speed +120 kt.
    pub spoof: Option<(f64, f64)>,
    /// Jamming window (s after `t0`): no position, NIC/NACp 0.
    pub jam: Option<(f64, f64)>,
    /// GNSS UTC − recorder time throughout (s).
    pub clock_offset_s: f64,
    /// Added from the spoofing start to the end of the flight (s): a clock that stays wrong.
    pub clock_step_from_spoof_s: f64,
}

impl FdmFlight {
    pub fn new(t0: f64) -> Self {
        FdmFlight {
            t0,
            secs: 2_400,
            start: (58.0, 22.0),
            heading: 90.0,
            spoof: None,
            jam: None,
            clock_offset_s: 0.0,
            clock_step_from_spoof_s: 0.0,
        }
    }

    pub fn csv(&self) -> String {
        let mut s = String::from("time,lat,lon,gnss_alt_ft,baro_alt_ft,gs_kt,track_deg,ins_gs_kt,nic,nacp,hfom_m,gnss_status,gnss_utc\n");
        let v = 450.0 * 1852.0 / 3600.0;
        let (mut n, mut e) = (0.0f64, 0.0f64);
        let inw = |w: Option<(f64, f64)>, t: f64| w.is_some_and(|(a, b)| t >= a && t < b);
        for k in 0..self.secs {
            let t = k as f64;
            n += v * self.heading.to_radians().cos();
            e += v * self.heading.to_radians().sin();
            let lat = self.start.0 + (n / R).to_degrees();
            let lon = self.start.1 + (e / (R * self.start.0.to_radians().cos())).to_degrees();
            let mut pos = Some((lat, lon));
            let (mut galt, mut gs, mut nic, mut nacp, mut hfom, mut st) =
                (35_350.0, 450.0, 8, 10, 15.0, "1");
            let mut clock = self.clock_offset_s;
            if let Some((a, _)) = self.spoof {
                if t >= a {
                    clock += self.clock_step_from_spoof_s;
                }
            }
            if inw(self.spoof, t) {
                pos = Some(north(lat, lon, 60_000.0));
                galt += 2_000.0;
                gs += 120.0;
            }
            if inw(self.jam, t) {
                pos = None;
                nic = 0;
                nacp = 0;
                hfom = 9_999.0;
                st = "0";
            }
            let (la, lo) = pos
                .map(|p| (format!("{:.6}", p.0), format!("{:.6}", p.1)))
                .unwrap_or_default();
            s += &format!(
                "{},{la},{lo},{galt:.0},35000,{gs:.1},{:.1},450.0,{nic},{nacp},{hfom:.0},{st},{}\n",
                iso(self.t0 + t),
                self.heading,
                iso(self.t0 + t + clock)
            );
        }
        s
    }
}

/// Three consecutive flights of one aircraft: F1 spoofed with a clock that stays 30 s wrong,
/// F2 (next day) with the clock still wrong and a TAWS alert with no interference, a reset,
/// then F3 clean. Returns `(flights, alerts_csv, resets_csv)`.
pub fn residual_case() -> (Vec<(String, String)>, String, String) {
    let mut f1 = FdmFlight::new(T0);
    f1.spoof = Some((600.0, 900.0));
    f1.clock_step_from_spoof_s = 30.0;
    let mut f2 = FdmFlight::new(T0 + 86_400.0);
    f2.clock_offset_s = 30.0;
    let f3 = FdmFlight::new(T0 + 3.0 * 86_400.0);
    let alerts = format!(
        "time,flight,alert\n{},F2,TERRAIN TERRAIN PULL UP\n",
        iso(T0 + 86_400.0 + 1_500.0)
    );
    let resets = format!(
        "reset_utc,action\n{},GNSS receiver power cycle\n",
        iso(T0 + 2.0 * 86_400.0)
    );
    (
        vec![
            ("F1".into(), f1.csv()),
            ("F2".into(), f2.csv()),
            ("F3".into(), f3.csv()),
        ],
        alerts,
        resets,
    )
}

/// A jammed flight that keeps its degradation long after leaving the area box returned with
/// it (`[57.9, 21.9, 58.1, 22.5]`; the aircraft leaves it about 130 s after the start).
pub fn area_exit_case() -> (String, [f64; 4]) {
    let mut f = FdmFlight::new(T0);
    f.secs = 1_200;
    f.jam = Some((60.0, 600.0));
    (f.csv(), [57.9, 21.9, 58.1, 22.5])
}

/// The nuisance-alert fixture (counts worked by hand in `tests/aviation_analytics.rs`).
pub const NUISANCE_ALERTS: &str = "time,subject,route,phase,alert,disposition
2026-06-01T10:05:00Z,A1,R1,cruise,TERRAIN TERRAIN PULL UP,nuisance
2026-06-01T10:07:00Z,A1,R1,cruise,GPS POSITION LOST,
2026-06-01T10:20:00Z,A1,R1,descent,CLOCK FAIL,
2026-06-01T12:00:00Z,A1,R1,approach,TERRAIN,genuine
2026-06-01T10:10:00Z,A2,R1,cruise,TERRAIN PULL UP,
2026-06-01T11:00:00Z,A2,R1,cruise,CABIN PRESSURE,
2026-06-01T14:00:00Z,A3,R2,cruise,GPS POSITION LOST,
2026-06-01T14:30:00Z,A3,R2,approach,SINK RATE,valid
2026-06-01T15:00:00Z,A4,R2,cruise,TERRAIN,false
";
pub const NUISANCE_INTERVALS: &str = "subject,start_utc,end_utc
A1,2026-06-01T10:00:00Z,2026-06-01T10:15:00Z
A2,2026-06-01T10:00:00Z,2026-06-01T10:12:00Z
";
pub const NUISANCE_HOURS: &str = "route,crew_hours\nR1,20\nR2,30\n";

/// The fleet burst: `n` aircraft cruising through one 0.5° cell, all affected for two
/// minutes from `T0 + 3600` (integrity loss, or position jumps when `spoof`), plus `quiet`
/// aircraft elsewhere. Returns the ADS-B CSV.
pub fn burst_adsb(n: usize, spoof: bool, cell_center: (f64, f64)) -> String {
    let mut s = String::from("timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp\n");
    for k in 0..n {
        let id = format!("FLT{k:02}");
        let lat = cell_center.0 - 0.2 + 0.4 * k as f64 / n.max(1) as f64;
        for j in 0..120 {
            let ts = 3_000.0 + j as f64 * 10.0;
            let lon = cell_center.1 - 0.6 + j as f64 * 0.01;
            let hit = (3_600.0..3_720.0).contains(&ts);
            let (p, nic, nacp) = match (hit, spoof) {
                (true, false) => ((lat, lon), 0, 0),
                (true, true) => (north(lat, lon, 40_000.0), 8, 9),
                _ => ((lat, lon), 8, 9),
            };
            row(&mut s, T0 + ts, &id, p, 33_000.0, nic, nacp);
        }
    }
    s
}

/// Write every synthetic case under `dir`: `approach/`, `residual/`, `nuisance/`, `areas/`,
/// each with a request file ready for the matching `kshana-pro aviation` command.
pub fn write_all(dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    let w = |p: &str, t: &str| -> Result<(), String> {
        let f = dir.join(p);
        std::fs::create_dir_all(f.parent().unwrap_or(dir)).map_err(io)?;
        std::fs::write(&f, t).map_err(|e| format!("{}: {e}", f.display()))
    };
    w("approach/adsb.csv", &approach_adsb())?;
    w("approach/map-2026-06-01.geojson", &approach_map())?;
    w("approach/request.toml", &approach_request())?;
    let (flights, alerts, resets) = residual_case();
    let mut req = String::from("title = \"Synthetic residual effects\"\naircraft = \"SYN-01\"\nalerts = \"alerts.csv\"\nresets = \"resets.csv\"\n");
    for (id, csv) in &flights {
        w(&format!("residual/{id}.csv"), csv)?;
        req += &format!("\n[[flights]]\nid = \"{id}\"\ncsv = \"{id}.csv\"\n");
    }
    w("residual/alerts.csv", &alerts)?;
    w("residual/resets.csv", &resets)?;
    w("residual/request.toml", &req)?;
    w("nuisance/alerts.csv", NUISANCE_ALERTS)?;
    w("nuisance/intervals.csv", NUISANCE_INTERVALS)?;
    w("nuisance/hours.csv", NUISANCE_HOURS)?;
    w(
        "nuisance/request.toml",
        "title = \"Synthetic nuisance alerts\"\nalerts = [\"alerts.csv\"]\nexposure = [\"intervals.csv\"]\nhours = \"hours.csv\"\n",
    )?;
    let mut fleet = burst_adsb(8, true, (32.75, -97.25));
    fleet += burst_adsb(6, false, (45.25, 10.25))
        .lines()
        .skip(1)
        .map(|l| l.replace(",FLT", ",JAM") + "\n")
        .collect::<String>()
        .as_str();
    w("areas/fleet.csv", &fleet)?;
    w(
        "areas/request.toml",
        &format!(
            "title = \"Synthetic fleet bursts\"\nadsb = [\"fleet.csv\"]\n\n\
             [[advisories]]\nname = \"Synthetic advisory\"\ncenter = [45.25, 10.25]\nradius_nm = 40\nstart_utc = \"{}\"\nend_utc = \"{}\"\n",
            iso(T0),
            iso(T0 + 86_400.0)
        ),
    )?;
    Ok(())
}
