// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receiver trajectory export for GNSS simulators that import motion files.
//!
//! * [`NmeaTrajectoryExport`]: NMEA 0183 `GGA` + `RMC` sentence pairs (default 10 Hz), the
//!   format most commercial GNSS simulators accept for "NMEA motion" import.
//! * [`CsvTrajectoryExport`]: generic CSV `time_s,utc,lat_deg,lon_deg,height_m,heading_deg`
//!   for simulators with a configurable column import.
//!
//! The trajectory is sampled from the scenario's [`crate::traj::Trajectory`], so the GNSS
//! simulator flies exactly the receiver path the eLoran scene uses. Heights are the
//! scenario's ellipsoidal heights; in GGA they are written as altitude with a geoid
//! separation of 0 (configure the simulator accordingly). Time stamps are UTC from the
//! scenario's `start_utc`; a leap second inside a scenario is not represented.

use super::{TrajFormat, TrajectoryFile, UtcStamp};
use crate::scenario::Scenario;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One trajectory sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrajSample {
    /// Scene time (s).
    pub t_s: f64,
    pub utc: UtcStamp,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
    /// Heading (degrees clockwise from true north, 0..360).
    pub heading_deg: f64,
    pub speed_mps: f64,
}

/// Sample the scenario's receiver trajectory at `rate_hz` over the scene duration
/// (inclusive of both ends). Times are rounded to the nanosecond.
pub fn sample_trajectory(sc: &Scenario, rate_hz: f64) -> Result<Vec<TrajSample>, String> {
    if rate_hz.is_nan() || rate_hz <= 0.0 {
        return Err("rate must be positive".into());
    }
    sc.receiver.validate()?;
    let start = UtcStamp::parse(&sc.scene.start_utc)?;
    let n = (sc.scene.params.duration_s * rate_hz + 1e-9).floor() as usize + 1;
    Ok((0..n)
        .map(|k| {
            let t = k as f64 / rate_hz;
            let s = sc.receiver.state(t);
            TrajSample {
                t_s: t,
                utc: start.plus(t),
                lat_deg: s.pos.lat_deg,
                lon_deg: s.pos.lon_deg,
                h_m: s.pos.h_m,
                heading_deg: s.heading_rad.to_degrees().rem_euclid(360.0),
                speed_mps: s.speed_mps,
            }
        })
        .collect())
}

/// A trajectory export format.
pub trait TrajectoryExport {
    fn format(&self) -> TrajFormat;
    fn rate_hz(&self) -> f64;
    /// Render samples to text.
    fn render(&self, samples: &[TrajSample]) -> String;
    /// Sample `sc` and write the file.
    fn write(&self, sc: &Scenario, path: &Path) -> Result<TrajectoryFile, String> {
        let s = sample_trajectory(sc, self.rate_hz())?;
        std::fs::write(path, self.render(&s)).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(TrajectoryFile {
            path: path.to_path_buf(),
            format: self.format(),
            rate_hz: self.rate_hz(),
            samples: s.len(),
        })
    }
}

/// NMEA 0183 GGA + RMC export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NmeaTrajectoryExport {
    /// Output rate (Hz), default 10.
    pub rate_hz: f64,
    /// Talker ID, default "GP".
    pub talker: String,
}

impl Default for NmeaTrajectoryExport {
    fn default() -> Self {
        NmeaTrajectoryExport {
            rate_hz: 10.0,
            talker: "GP".into(),
        }
    }
}

/// NMEA checksum: XOR of the bytes between `$` and `*`.
pub fn nmea_checksum(body: &str) -> u8 {
    body.bytes().fold(0u8, |a, b| a ^ b)
}

fn sentence(body: &str) -> String {
    format!("${body}*{:02X}\r\n", nmea_checksum(body))
}

/// `ddmm.mmmmm,N` / `dddmm.mmmmm,E`.
fn nmea_angle(v: f64, deg_digits: usize, pos: char, neg: char) -> String {
    let m5 = (v.abs() * 60.0 * 1e5).round() as u64;
    let d = m5 / 6_000_000;
    let rem = m5 % 6_000_000;
    format!(
        "{d:0w$}{:02}.{:05},{}",
        rem / 100_000,
        rem % 100_000,
        if v < 0.0 { neg } else { pos },
        w = deg_digits
    )
}

fn nmea_time(u: UtcStamp) -> String {
    let (h, m, s, ns) = u.hms();
    format!("{h:02}{m:02}{s:02}.{:02}", ns / 10_000_000)
}

impl TrajectoryExport for NmeaTrajectoryExport {
    fn format(&self) -> TrajFormat {
        TrajFormat::Nmea
    }
    fn rate_hz(&self) -> f64 {
        self.rate_hz
    }
    fn render(&self, samples: &[TrajSample]) -> String {
        let mut out = String::new();
        for s in samples {
            let t = nmea_time(s.utc);
            let lat = nmea_angle(s.lat_deg, 2, 'N', 'S');
            let lon = nmea_angle(s.lon_deg, 3, 'E', 'W');
            let gga = format!(
                "{}GGA,{t},{lat},{lon},1,12,0.8,{:.2},M,0.00,M,,",
                self.talker, s.h_m
            );
            out.push_str(&sentence(&gga));
            let (y, mo, d) = s.utc.ymd();
            let rmc = format!(
                "{}RMC,{t},A,{lat},{lon},{:.2},{:.2},{d:02}{mo:02}{:02},,,A",
                self.talker,
                s.speed_mps * 3600.0 / 1852.0,
                s.heading_deg,
                y % 100
            );
            out.push_str(&sentence(&rmc));
        }
        out
    }
}

/// Generic CSV export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsvTrajectoryExport {
    pub rate_hz: f64,
}

impl Default for CsvTrajectoryExport {
    fn default() -> Self {
        CsvTrajectoryExport { rate_hz: 10.0 }
    }
}

impl TrajectoryExport for CsvTrajectoryExport {
    fn format(&self) -> TrajFormat {
        TrajFormat::Csv
    }
    fn rate_hz(&self) -> f64 {
        self.rate_hz
    }
    fn render(&self, samples: &[TrajSample]) -> String {
        let mut out = String::from("time_s,utc,lat_deg,lon_deg,height_m,heading_deg\n");
        for s in samples {
            out.push_str(&format!(
                "{:.3},{},{:.9},{:.9},{:.3},{:.3}\n",
                s.t_s,
                s.utc.iso(),
                s.lat_deg,
                s.lon_deg,
                s.h_m,
                s.heading_deg
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SC: &str = r#"
[scene]
start_utc = "2026-11-18T12:00:00Z"
duration_s = 2.0
fs_hz = 1000000
[receiver]
type = "waypoints"
points = [
  { t_s = 0.0, lat_deg = 55.5, lon_deg = -3.25, h_m = 10.0 },
  { t_s = 100.0, lat_deg = 55.51, lon_deg = -3.25, h_m = 10.0 },
]
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 0.0 }
"#;

    #[test]
    fn nmea_export_has_valid_checksums_and_fields() {
        let sc = Scenario::from_toml(SC).unwrap();
        let txt = NmeaTrajectoryExport::default().render(&sample_trajectory(&sc, 10.0).unwrap());
        let lines: Vec<&str> = txt.lines().collect();
        assert_eq!(lines.len(), 2 * 21);
        for l in &lines {
            let (body, cs) = l.trim_start_matches('$').split_once('*').unwrap();
            assert_eq!(u8::from_str_radix(cs, 16).unwrap(), nmea_checksum(body));
        }
        assert!(
            lines[0].starts_with("$GPGGA,120000.00,5530.00000,N,00315.00000,W,1,12,0.8,10.00,M"),
            "{}",
            lines[0]
        );
        assert!(
            lines[1].starts_with("$GPRMC,120000.00,A,5530.00000,N,00315.00000,W,"),
            "{}",
            lines[1]
        );
        assert!(lines[1].contains(",181126,"), "{}", lines[1]);
        assert!(lines[3].starts_with("$GPRMC,120000.10,"));
        // speed ≈ 11.13 m/s ≈ 21.6 kn, heading north
        let f: Vec<&str> = lines[3].split(',').collect();
        assert!((f[7].parse::<f64>().unwrap() - 21.63).abs() < 0.1);
        assert!(f[8].parse::<f64>().unwrap() < 0.01 || f[8].parse::<f64>().unwrap() > 359.99);
    }

    #[test]
    fn csv_export_writes_file() {
        let sc = Scenario::from_toml(SC).unwrap();
        let p = std::env::temp_dir().join(format!("kshana-eloran-traj-{}.csv", std::process::id()));
        let f = CsvTrajectoryExport::default().write(&sc, &p).unwrap();
        assert_eq!(f.samples, 21);
        let txt = std::fs::read_to_string(&p).unwrap();
        let rows: Vec<&str> = txt.lines().collect();
        assert_eq!(rows[0], "time_s,utc,lat_deg,lon_deg,height_m,heading_deg");
        assert!(rows[21]
            .starts_with("2.000,2026-11-18T12:00:02.000000000Z,55.500200000,-3.250000000,10.000,"));
    }

    #[test]
    fn nmea_angle_rounding_never_shows_60_minutes() {
        assert_eq!(nmea_angle(55.9999999999, 2, 'N', 'S'), "5600.00000,N");
        assert_eq!(nmea_angle(-0.5, 3, 'E', 'W'), "00030.00000,W");
    }
}
