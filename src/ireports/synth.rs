// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic observation data with known truth, matching the sample report forms in
//! `tests/data/interference-reports/` (all synthetic; no real aircraft, vessel or report).
//!
//! * **Punjab, 12 Nov 2026** (aircraft): jamming over the area from 05:38 to 06:00 UTC, and
//!   (within 100 km of 31.70 N 74.80 E) and spoofing of VT-ABC (ICAO 800ABC) from 05:44 to 06:00 (position pushed 60 km north). After
//!   it, VT-ABC's GNSS stays 2.2 km off its inertial position (a receiver that needs a reset);
//!   VT-XYZ (800DEF) recovers fully. ADS-B of five aircraft; VT-PQR (800AAA) flies clean near
//!   Delhi at 07:10, when its crew reports jamming (a report the data contradicts).
//! * **Gulf of Finland, 14 Jun 2026** (vessel IMO 9387421, MMSI 230123000): GNSS spoofing to a
//!   point ashore from 04:12 to 04:41 UTC (NMEA: C/N0 raised and uniform, position jump; AIS
//!   follows it, and so does neighbour 230555000), then from 05:25 to 05:33 the vessel's AIS
//!   track alone is moved 37 km south while its receiver is clean (AIS-layer manipulation).

use std::path::Path;

use super::text::iso;

const KN: f64 = 0.514_444;

/// 2026-11-12T05:00:00Z.
pub const PUNJAB_T0: f64 = 1_794_459_600.0;
/// 2026-06-14T03:30:00Z.
pub const GULF_T0: f64 = 1_781_407_800.0;

fn dest(lat: f64, lon: f64, north_m: f64, east_m: f64) -> (f64, f64) {
    let r = 6_371_000.0;
    (
        lat + (north_m / r).to_degrees(),
        lon + (east_m / (r * lat.to_radians().cos())).to_degrees(),
    )
}

/// Position on a straight track through `(lat, lon)` at `t_pass`, heading `hdg` degrees.
fn along(lat: f64, lon: f64, t_pass: f64, hdg: f64, kn: f64, t: f64) -> (f64, f64) {
    let d = (t - t_pass) * kn * KN;
    dest(
        lat,
        lon,
        d * hdg.to_radians().cos(),
        d * hdg.to_radians().sin(),
    )
}

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn n(&mut self) -> f64 {
        (self.u() + self.u() + self.u() + self.u() - 2.0) * 1.732
    }
}

struct Aircraft {
    icao: &'static str,
    pass: (f64, f64),
    t_pass: f64,
    hdg: f64,
    kn: f64,
    alt_ft: f64,
    t0: f64,
    t1: f64,
}

fn punjab_fleet() -> Vec<Aircraft> {
    let t = |h: f64, m: f64| PUNJAB_T0 + h * 3600.0 + m * 60.0;
    vec![
        // VT-ABC, AI-117: spoofed.
        Aircraft {
            icao: "800ABC",
            pass: (31.70, 74.80),
            t_pass: t(0.0, 42.0),
            hdg: 300.0,
            kn: 460.0,
            alt_ft: 35_000.0,
            t0: t(0.0, 20.0),
            t1: t(1.0, 20.0),
        },
        // VT-XYZ, 6E-2231: jammed, recovers.
        Aircraft {
            icao: "800DEF",
            pass: (31.65, 74.95),
            t_pass: t(0.0, 50.0),
            hdg: 120.0,
            kn: 440.0,
            alt_ft: 31_000.0,
            t0: t(0.0, 25.0),
            t1: t(1.0, 15.0),
        },
        Aircraft {
            icao: "800BBB",
            pass: (31.80, 74.60),
            t_pass: t(0.0, 48.0),
            hdg: 90.0,
            kn: 420.0,
            alt_ft: 37_000.0,
            t0: t(0.0, 30.0),
            t1: t(1.0, 10.0),
        },
        Aircraft {
            icao: "800CCC",
            pass: (31.55, 74.70),
            t_pass: t(0.0, 52.0),
            hdg: 200.0,
            kn: 400.0,
            alt_ft: 29_000.0,
            t0: t(0.0, 30.0),
            t1: t(1.0, 10.0),
        },
        // VT-PQR near Delhi, clean.
        Aircraft {
            icao: "800AAA",
            pass: (28.57, 77.10),
            t_pass: t(2.0, 10.0),
            hdg: 270.0,
            kn: 250.0,
            alt_ft: 12_000.0,
            t0: t(1.0, 50.0),
            t1: t(2.0, 30.0),
        },
    ]
}

const JAM_CENTRE: (f64, f64) = (31.70, 74.80);

fn jammed(lat: f64, lon: f64, t: f64) -> bool {
    let d = kshana::interference_map::grid::haversine_m(lat, lon, JAM_CENTRE.0, JAM_CENTRE.1);
    d <= 100_000.0 && (PUNJAB_T0 + 38.0 * 60.0..PUNJAB_T0 + 60.0 * 60.0).contains(&t)
}

fn abc_spoofed(t: f64) -> bool {
    (PUNJAB_T0 + 44.0 * 60.0..PUNJAB_T0 + 60.0 * 60.0).contains(&t)
}

/// ADS-B CSV of the Punjab case (`timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp`).
pub fn punjab_adsb() -> String {
    let mut rows: Vec<(f64, String)> = vec![];
    for a in punjab_fleet() {
        let mut t = a.t0;
        while t <= a.t1 {
            let (mut la, mut lo) = along(a.pass.0, a.pass.1, a.t_pass, a.hdg, a.kn, t);
            let (mut nic, mut nacp) = (8, 10);
            if jammed(la, lo, t) {
                (nic, nacp) = (0, 0);
            }
            if a.icao == "800ABC" && abc_spoofed(t) {
                (la, lo) = dest(la, lo, 60_000.0, 0.0);
                (nic, nacp) = (6, 7);
            }
            rows.push((
                t,
                format!(
                    "{},{},{la:.5},{lo:.5},{:.0},{nic},{nacp}",
                    iso(t),
                    a.icao,
                    a.alt_ft
                ),
            ));
            t += 10.0;
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut s = String::from("timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp\n");
    for (_, r) in rows {
        s += &r;
        s.push('\n');
    }
    s
}

/// The aircraft's GNSS position export and its inertial (IRS) reference track, every 5 s.
/// `which` is `abc` (spoofed, stays 2.2 km off afterwards) or `xyz` (jammed, recovers).
pub fn punjab_gnss_and_irs(which: &str) -> (String, String) {
    let fleet = punjab_fleet();
    let a = fleet
        .iter()
        .find(|a| a.icao == if which == "abc" { "800ABC" } else { "800DEF" })
        .expect("fleet");
    let mut r = Rng(if which == "abc" { 7 } else { 11 });
    let mut gnss = String::from("timestamp,lat,lon,cn0_dbhz,sats\n");
    let mut irs = String::from("timestamp,lat,lon\n");
    let (t0, t1) = (PUNJAB_T0, PUNJAB_T0 + 100.0 * 60.0);
    let mut t = t0;
    while t <= t1 {
        let (la, lo) = along(a.pass.0, a.pass.1, a.t_pass, a.hdg, a.kn, t);
        irs += &format!("{},{la:.6},{lo:.6}\n", iso(t));
        let (mut cn0, mut sats, mut fix) = (45.0 + 0.8 * r.n(), 11, Some((la, lo)));
        if jammed(la, lo, t) {
            cn0 = 27.0 + r.n();
            sats = 2;
            fix = None;
        }
        if which == "abc" {
            if abc_spoofed(t) {
                cn0 = 50.0 + 0.2 * r.n();
                sats = 11;
                fix = Some(dest(la, lo, 60_000.0, 0.0));
            } else if t >= PUNJAB_T0 + 60.0 * 60.0 {
                // Signals normal again, the solution still 2.2 km off.
                fix = Some(dest(la, lo, 2_200.0, 300.0));
            }
        }
        match fix {
            Some((x, y)) => gnss += &format!("{},{x:.6},{y:.6},{cn0:.1},{sats}\n", iso(t)),
            None => gnss += &format!("{},,,{cn0:.1},{sats}\n", iso(t)),
        }
        t += 5.0;
    }
    (gnss, irs)
}

/// The vessel's own track (12 kn, east).
fn vessel(t: f64) -> (f64, f64) {
    along(59.3333, 23.80, GULF_T0, 90.0, 12.0, t)
}

/// The spoofer's false position (ashore).
pub const FALSE_POS: (f64, f64) = (59.4133, 24.8333);

fn gulf_spoof(t: f64) -> bool {
    (GULF_T0 + 42.0 * 60.0..GULF_T0 + 71.0 * 60.0).contains(&t)
}

fn ais_manipulated(t: f64) -> bool {
    (GULF_T0 + 115.0 * 60.0..GULF_T0 + 123.0 * 60.0).contains(&t)
}

fn cs(body: &str) -> String {
    let x = body.bytes().fold(0u8, |a, b| a ^ b);
    format!("${body}*{x:02X}\n")
}

fn hms(t: f64) -> String {
    let s = (t - (t / 86_400.0).floor() * 86_400.0) as i64;
    format!("{:02}{:02}{:02}.00", s / 3600, s / 60 % 60, s % 60)
}

fn ll(lat: f64, lon: f64) -> String {
    let f = |v: f64, w: usize| {
        let a = v.abs();
        let d = a.floor();
        format!("{:0w$}{:07.4}", d as i64, (a - d) * 60.0, w = w)
    };
    format!(
        "{},{},{},{}",
        f(lat, 2),
        if lat >= 0.0 { "N" } else { "S" },
        f(lon, 3),
        if lon >= 0.0 { "E" } else { "W" }
    )
}

/// The vessel's NMEA 0183 log (RMC, GGA, GSV every 2 s, 03:30 to 06:00 UTC).
pub fn gulf_nmea() -> String {
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    let prns = [2u32, 5, 7, 9, 13, 15, 18, 21, 30];
    let base = [46.0, 41.0, 38.0, 44.0, 35.0, 48.0, 40.0, 37.0, 43.0];
    let mut s = String::new();
    let mut t = GULF_T0;
    while t < GULF_T0 + 150.0 * 60.0 {
        let tl = iso(t);
        let date = format!("{}{}{}", &tl[8..10], &tl[5..7], &tl[2..4]);
        let spoof = gulf_spoof(t);
        let (la, lo) = if spoof {
            dest(FALSE_POS.0, FALSE_POS.1, 2.0 * r.n(), 2.0 * r.n())
        } else {
            vessel(t)
        };
        let cn0: Vec<f64> = base
            .iter()
            .map(|b| {
                if spoof {
                    50.0 + 0.3 * r.n()
                } else {
                    b + 0.7 * r.n()
                }
            })
            .collect();
        let (sog, cog) = if spoof {
            (0.1, 0.0)
        } else {
            (12.0 + 0.1 * r.n(), 90.0)
        };
        s += &cs(&format!(
            "GPRMC,{},A,{},{sog:.1},{cog:.1},{date},,,A",
            hms(t),
            ll(la, lo)
        ));
        s += &cs(&format!(
            "GPGGA,{},{},1,{:02},0.9,12.0,M,18.0,M,,",
            hms(t),
            ll(la, lo),
            prns.len()
        ));
        let tot = prns.len().div_ceil(4);
        for (g, ch) in prns.chunks(4).enumerate() {
            let mut b = format!("GPGSV,{tot},{},{:02}", g + 1, prns.len());
            for (j, p) in ch.iter().enumerate() {
                let i = g * 4 + j;
                b += &format!(
                    ",{p:02},{:02},{:03},{:.0}",
                    20 + 7 * i,
                    (40 * i) % 360,
                    cn0[i].round()
                );
            }
            s += &cs(&b);
        }
        t += 2.0;
    }
    s
}

/// AIS CSV of the Gulf case (`timestamp,vessel_id,lat,lon,sog_kn`), every 10 s.
pub fn gulf_ais() -> String {
    let mut rows: Vec<(f64, String)> = vec![];
    let mut t = GULF_T0;
    while t < GULF_T0 + 150.0 * 60.0 {
        // The subject: its GNSS position, or a manipulated AIS position.
        let (mut la, mut lo) = if gulf_spoof(t) { FALSE_POS } else { vessel(t) };
        if ais_manipulated(t) {
            (la, lo) = dest(la, lo, -37_000.0, 0.0);
        }
        rows.push((t, format!("{},230123000,{la:.5},{lo:.5},12.0", iso(t))));
        // Neighbour 230555000, westbound, spoofed with the same false position.
        let (la, lo) = if gulf_spoof(t) {
            FALSE_POS
        } else {
            along(59.38, 24.30, GULF_T0, 270.0, 10.0, t)
        };
        rows.push((
            t + 3.0,
            format!("{},230555000,{la:.5},{lo:.5},10.0", iso(t + 3.0)),
        ));
        // Neighbour 230777000, further south, clean.
        let (la, lo) = along(59.10, 24.00, GULF_T0, 60.0, 14.0, t);
        rows.push((
            t + 6.0,
            format!("{},230777000,{la:.5},{lo:.5},14.0", iso(t + 6.0)),
        ));
        t += 10.0;
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut s = String::from("timestamp,vessel_id,lat,lon,sog_kn\n");
    for (_, r) in rows {
        s += &r;
        s.push('\n');
    }
    s
}

/// Write every synthetic file, and the request files that use them, into `dir`. The
/// requests expect `reports.json` (from `kshana-pro reports normalise`) and the episode
/// directory `episodes/` (from `kshana-pro reports episodes`) in the same folder.
pub fn write_all(dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (abc_g, abc_i) = punjab_gnss_and_irs("abc");
    let (xyz_g, xyz_i) = punjab_gnss_and_irs("xyz");
    let files: Vec<(&str, String)> = vec![
        ("adsb_punjab.csv", punjab_adsb()),
        ("vt-abc_gnss.csv", abc_g),
        ("vt-abc_irs.csv", abc_i),
        ("vt-xyz_gnss.csv", xyz_g),
        ("vt-xyz_irs.csv", xyz_i),
        ("vessel_230123000.nmea", gulf_nmea()),
        ("ais_gulf.csv", gulf_ais()),
        (
            "verify.toml",
            "# Cross-check every report in reports.json against the synthetic observations.\n\
             reports = \"reports.json\"\n\n\
             [[receiver]]\nlabel = \"VT-ABC GNSS (FMS export)\"\npath = \"vt-abc_gnss.csv\"\nformat = \"csv\"\nrole = \"subject\"\nsubject_id = \"VT-ABC\"\n\n\
             [[receiver]]\nlabel = \"MV Nordic Tern bridge GNSS\"\npath = \"vessel_230123000.nmea\"\nformat = \"nmea\"\nrole = \"subject\"\nsubject_id = \"9387421\"\nmax_speed_kn = 40\n\n\
             [[adsb]]\npath = \"adsb_punjab.csv\"\n\n\
             [[ais]]\npath = \"ais_gulf.csv\"\n\n\
             [aliases]\n\"800ABC\" = \"VT-ABC\"\n\"800DEF\" = \"VT-XYZ\"\n\"800AAA\" = \"VT-PQR\"\n"
                .into(),
        ),
        (
            "ledger.toml",
            "title = \"Synthetic fleet, November 2026\"\nepisodes = [\"episodes\"]\nresets = \"resets.csv\"\n\n\
             [aliases]\n\"800ABC\" = \"VT-ABC\"\n\"800DEF\" = \"VT-XYZ\"\n\"800AAA\" = \"VT-PQR\"\n\"230123000\" = \"9387421\"\n"
                .into(),
        ),
        (
            "resets.csv",
            "subject_id,reset_utc,action\nVT-ABC,2026-11-01T10:00:00Z,MMR power cycle (scheduled check)\nVT-XYZ,2026-11-12T08:30:00Z,GNSS receiver reset after arrival\n".into(),
        ),
        (
            "reset-abc.toml",
            "subject = \"VT-ABC\"\nepisode_end_utc = \"2026-11-12T06:00:00Z\"\nreference_track = \"vt-abc_irs.csv\"\nreference_label = \"IRS position\"\n\n\
             [receiver]\npath = \"vt-abc_gnss.csv\"\nformat = \"csv\"\n"
                .into(),
        ),
        (
            "reset-xyz.toml",
            "subject = \"VT-XYZ\"\nepisode_end_utc = \"2026-11-12T06:00:00Z\"\nreference_track = \"vt-xyz_irs.csv\"\nreference_label = \"IRS position\"\n\n\
             [receiver]\npath = \"vt-xyz_gnss.csv\"\nformat = \"csv\"\n"
                .into(),
        ),
    ];
    let mut written = vec![];
    for (n, body) in files {
        let p = dir.join(n);
        std::fs::write(&p, body).map_err(|e| format!("{}: {e}", p.display()))?;
        written.push(p.display().to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs_are_the_documented_times() {
        assert_eq!(iso(PUNJAB_T0), "2026-11-12T05:00:00Z");
        assert_eq!(iso(GULF_T0), "2026-06-14T03:30:00Z");
    }
}
