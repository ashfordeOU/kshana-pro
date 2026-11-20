// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Machine logs: a neutral CSV and NMEA 0183 `GGA`.
//!
//! **CSV** (`kshana-agri-log`): a header row, then one row per epoch. Recognised columns (any
//! order; unknown columns are ignored): `t_s` (seconds, any origin; required), `lat`, `lon`
//! (degrees; required), `h` (m), `fix` (`rtk_fixed`, `rtk_float`, `ppp`, `dgnss`, `sbas`,
//! `autonomous`, `dr`, `none`, or the NMEA quality digit), `sats`, `hdop`, `cn0` (mean C/N0 of
//! the satellites tracked, dB-Hz), `corr_age` (age of corrections, s), `steer` (1/0 or
//! true/false: autosteer engaged) and `xte` (the guidance system's own cross-track error, m).
//! Column-name aliases: `time_s`, `lat_deg`, `lon_deg`, `h_m`, `alt_m`, `cn0_dbhz`,
//! `corr_age_s`, `steer_engaged`, `xte_m`.
//!
//! **NMEA**: `$..GGA` sentences (any talker), checksum checked when present; time is seconds
//! of day carried across midnight. Fix quality: 0 none, 1 autonomous, 2 DGNSS, 4 RTK fixed,
//! 5 RTK float, 6 dead reckoning; 3, 7, 8 are read as none (not a usable position).

use serde::{Deserialize, Serialize};

/// Position fix quality, worst to best for machine guidance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    None,
    DeadReckoning,
    Autonomous,
    Sbas,
    Dgnss,
    Ppp,
    RtkFloat,
    RtkFixed,
}

impl Fix {
    /// Parse a fix name or NMEA quality digit.
    pub fn parse(s: &str) -> Result<Fix, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "rtk_fixed" | "fixed" | "rtk" | "4" => Fix::RtkFixed,
            "rtk_float" | "float" | "5" => Fix::RtkFloat,
            "ppp" => Fix::Ppp,
            "dgnss" | "dgps" | "2" => Fix::Dgnss,
            "sbas" => Fix::Sbas,
            "autonomous" | "single" | "gps" | "gnss" | "1" => Fix::Autonomous,
            "dr" | "dead_reckoning" | "6" => Fix::DeadReckoning,
            "none" | "invalid" | "no_fix" | "0" | "3" | "7" | "8" | "" => Fix::None,
            other => return Err(format!("unknown fix quality '{other}'")),
        })
    }

    /// The snake-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Fix::None => "none",
            Fix::DeadReckoning => "dr",
            Fix::Autonomous => "autonomous",
            Fix::Sbas => "sbas",
            Fix::Dgnss => "dgnss",
            Fix::Ppp => "ppp",
            Fix::RtkFloat => "rtk_float",
            Fix::RtkFixed => "rtk_fixed",
        }
    }

    /// True for a fix that carries a usable GNSS position.
    pub fn has_position(self) -> bool {
        !matches!(self, Fix::None | Fix::DeadReckoning)
    }
}

/// One epoch of a machine log.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Epoch {
    pub t_s: f64,
    pub lat: f64,
    pub lon: f64,
    pub h: Option<f64>,
    pub fix: Fix,
    pub sats: Option<u32>,
    pub hdop: Option<f64>,
    pub cn0: Option<f64>,
    pub corr_age: Option<f64>,
    pub steer: Option<bool>,
    pub xte: Option<f64>,
}

/// A parsed log, epochs in time order.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MachineLog {
    /// `csv` or `nmea`.
    pub format: String,
    pub epochs: Vec<Epoch>,
    /// Lines skipped (bad checksum, unparsable), with the reason.
    pub skipped: Vec<String>,
}

impl MachineLog {
    /// Parse a log, detecting NMEA (lines beginning `$`) or CSV.
    pub fn parse(text: &str) -> Result<MachineLog, String> {
        let first = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#'))
            .ok_or("empty log")?;
        let mut log = if first.starts_with('$') {
            parse_nmea(text)
        } else {
            parse_csv(text)?
        };
        log.epochs.sort_by(|a, b| a.t_s.total_cmp(&b.t_s));
        if log.epochs.is_empty() {
            return Err("log has no epochs".into());
        }
        Ok(log)
    }

    /// Duration (s) from the first to the last epoch.
    pub fn span_s(&self) -> f64 {
        match (self.epochs.first(), self.epochs.last()) {
            (Some(a), Some(b)) => b.t_s - a.t_s,
            _ => 0.0,
        }
    }

    /// Median epoch interval (s); 1 s when there are fewer than two epochs.
    pub fn interval_s(&self) -> f64 {
        let d: Vec<f64> = self
            .epochs
            .windows(2)
            .map(|w| w[1].t_s - w[0].t_s)
            .filter(|d| *d > 0.0)
            .collect();
        super::percentile(&d, 50.0).unwrap_or(1.0)
    }

    /// Whether any epoch records the steering state.
    pub fn has_steer(&self) -> bool {
        self.epochs.iter().any(|e| e.steer.is_some())
    }

    /// The neutral CSV text of the log (round-trips through [`MachineLog::parse`]).
    pub fn to_csv(&self) -> String {
        let mut s = String::from("t_s,lat,lon,h,fix,sats,hdop,cn0,corr_age,steer,xte\n");
        let o = |v: Option<f64>, d: usize| v.map(|x| format!("{x:.d$}")).unwrap_or_default();
        for e in &self.epochs {
            s.push_str(&format!(
                "{},{:.9},{:.9},{},{},{},{},{},{},{},{}\n",
                e.t_s,
                e.lat,
                e.lon,
                o(e.h, 3),
                e.fix.as_str(),
                e.sats.map(|x| x.to_string()).unwrap_or_default(),
                o(e.hdop, 2),
                o(e.cn0, 1),
                o(e.corr_age, 1),
                e.steer.map(|b| if b { "1" } else { "0" }).unwrap_or(""),
                o(e.xte, 4),
            ));
        }
        s
    }
}

fn col(header: &[String], names: &[&str]) -> Option<usize> {
    header.iter().position(|h| names.contains(&h.as_str()))
}

fn parse_csv(text: &str) -> Result<MachineLog, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines
        .next()
        .ok_or("empty log")?
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let need = |names: &[&str]| {
        col(&header, names).ok_or_else(|| format!("CSV log needs a '{}' column", names[0]))
    };
    let (ct, cla, clo) = (
        need(&["t_s", "time_s", "t"])?,
        need(&["lat", "lat_deg"])?,
        need(&["lon", "lon_deg"])?,
    );
    let ch = col(&header, &["h", "h_m", "alt_m", "height"]);
    let cf = col(&header, &["fix", "quality"]);
    let cs = col(&header, &["sats", "nsat", "satellites"]);
    let chd = col(&header, &["hdop"]);
    let cc = col(&header, &["cn0", "cn0_dbhz"]);
    let ca = col(&header, &["corr_age", "corr_age_s", "age"]);
    let cst = col(&header, &["steer", "steer_engaged", "engaged"]);
    let cx = col(&header, &["xte", "xte_m", "cross_track_m"]);
    let mut log = MachineLog {
        format: "csv".into(),
        ..Default::default()
    };
    for (i, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let get = |c: Option<usize>| c.and_then(|c| f.get(c)).filter(|v| !v.is_empty());
        let num = |c: Option<usize>| -> Result<Option<f64>, String> {
            get(c)
                .map(|v| {
                    v.parse::<f64>()
                        .map_err(|_| format!("row {}: '{v}' is not a number", i + 2))
                })
                .transpose()
        };
        let row = (|| -> Result<Epoch, String> {
            let t = num(Some(ct))?.ok_or("missing time")?;
            let lat = num(Some(cla))?.ok_or("missing lat")?;
            let lon = num(Some(clo))?.ok_or("missing lon")?;
            let fix = match get(cf) {
                Some(v) => Fix::parse(v)?,
                None => Fix::Autonomous,
            };
            let steer = match get(cst).map(|v| v.to_ascii_lowercase()) {
                None => None,
                Some(v) if ["1", "true", "yes", "on", "engaged"].contains(&v.as_str()) => {
                    Some(true)
                }
                Some(v) if ["0", "false", "no", "off", "disengaged"].contains(&v.as_str()) => {
                    Some(false)
                }
                Some(v) => return Err(format!("steer '{v}' is not 1/0")),
            };
            Ok(Epoch {
                t_s: t,
                lat,
                lon,
                h: num(ch)?,
                fix,
                sats: num(cs)?.map(|x| x.max(0.0) as u32),
                hdop: num(chd)?,
                cn0: num(cc)?,
                corr_age: num(ca)?,
                steer,
                xte: num(cx)?,
            })
        })();
        match row {
            Ok(e) => log.epochs.push(e),
            Err(e) => log.skipped.push(format!("row {}: {e}", i + 2)),
        }
    }
    Ok(log)
}

fn nmea_checksum_ok(s: &str) -> Option<bool> {
    let body = s.strip_prefix('$')?;
    let (data, cs) = body.split_once('*')?;
    let want = u8::from_str_radix(cs.trim().get(..2)?, 16).ok()?;
    Some(data.bytes().fold(0u8, |a, b| a ^ b) == want)
}

fn nmea_coord(v: &str, hemi: &str, deg_digits: usize) -> Option<f64> {
    if v.len() < deg_digits + 2 {
        return None;
    }
    let d: f64 = v.get(..deg_digits)?.parse().ok()?;
    let m: f64 = v.get(deg_digits..)?.parse().ok()?;
    let x = d + m / 60.0;
    Some(match hemi {
        "S" | "W" => -x,
        _ => x,
    })
}

fn parse_nmea(text: &str) -> MachineLog {
    let mut log = MachineLog {
        format: "nmea".into(),
        ..Default::default()
    };
    let mut day_offset = 0.0;
    let mut last_sod: Option<f64> = None;
    for (i, l) in text.lines().map(str::trim).enumerate() {
        if !l.starts_with('$') || l.len() < 7 || &l[3..6] != "GGA" {
            continue;
        }
        if nmea_checksum_ok(l) == Some(false) {
            log.skipped.push(format!("line {}: bad checksum", i + 1));
            continue;
        }
        let data = l.split('*').next().unwrap_or(l);
        let f: Vec<&str> = data.split(',').collect();
        if f.len() < 10 {
            log.skipped.push(format!("line {}: short GGA", i + 1));
            continue;
        }
        let t = f[1];
        let sod = (|| -> Option<f64> {
            let h: f64 = t.get(0..2)?.parse().ok()?;
            let m: f64 = t.get(2..4)?.parse().ok()?;
            let s: f64 = t.get(4..)?.parse().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        })();
        let (Some(sod), Some(lat), Some(lon)) =
            (sod, nmea_coord(f[2], f[3], 2), nmea_coord(f[4], f[5], 3))
        else {
            log.skipped
                .push(format!("line {}: no time or position", i + 1));
            continue;
        };
        if let Some(p) = last_sod {
            if sod + 43_200.0 < p {
                day_offset += 86_400.0;
            }
        }
        last_sod = Some(sod);
        let fix = Fix::parse(f[6]).unwrap_or(Fix::None);
        let h = match (
            f[9].parse::<f64>().ok(),
            f.get(11).and_then(|g| g.parse::<f64>().ok()),
        ) {
            (Some(msl), Some(n)) => Some(msl + n),
            (Some(msl), None) => Some(msl),
            _ => None,
        };
        log.epochs.push(Epoch {
            t_s: sod + day_offset,
            lat,
            lon,
            h,
            fix,
            sats: f[7].parse().ok(),
            hdop: f[8].parse().ok(),
            cn0: None,
            corr_age: f.get(13).and_then(|a| a.parse().ok()),
            steer: None,
            xte: None,
        });
    }
    log
}

/// An NMEA `GGA` sentence for an epoch (used to write examples and tests).
pub fn gga(e: &Epoch) -> String {
    let sod = e.t_s.rem_euclid(86_400.0);
    let (h, m, s) = (
        (sod / 3600.0).floor(),
        ((sod % 3600.0) / 60.0).floor(),
        sod % 60.0,
    );
    let ll = |v: f64, deg: usize| {
        let a = v.abs();
        let d = a.floor();
        format!("{:0deg$}{:010.7}", d as u32, (a - d) * 60.0)
    };
    let q = match e.fix {
        Fix::None => 0,
        Fix::Autonomous | Fix::Sbas | Fix::Ppp => 1,
        Fix::Dgnss => 2,
        Fix::RtkFixed => 4,
        Fix::RtkFloat => 5,
        Fix::DeadReckoning => 6,
    };
    let body = format!(
        "GPGGA,{:02}{:02}{:05.2},{},{},{},{},{},{:02},{:.1},{:.3},M,0.000,M,{},",
        h as u32,
        m as u32,
        s,
        ll(e.lat, 2),
        if e.lat < 0.0 { "S" } else { "N" },
        ll(e.lon, 3),
        if e.lon < 0.0 { "W" } else { "E" },
        q,
        e.sats.unwrap_or(0),
        e.hdop.unwrap_or(0.9),
        e.h.unwrap_or(0.0),
        e.corr_age.map(|a| format!("{a:.1}")).unwrap_or_default(),
    );
    let cs = body.bytes().fold(0u8, |a, b| a ^ b);
    format!("${body}*{cs:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_and_nmea() {
        let csv = "t_s,lat,lon,fix,sats,steer,xte\n0,49.9,-97.1,rtk_fixed,18,1,0.01\n1,49.90001,-97.1,5,17,0,\n";
        let l = MachineLog::parse(csv).unwrap();
        assert_eq!(l.epochs.len(), 2);
        assert_eq!(l.epochs[1].fix, Fix::RtkFloat);
        assert_eq!(l.epochs[1].steer, Some(false));
        assert_eq!(l.epochs[1].xte, None);
        let again = MachineLog::parse(&l.to_csv()).unwrap();
        assert_eq!(again.epochs, l.epochs);
        let s = gga(&l.epochs[0]);
        assert_eq!(nmea_checksum_ok(&s), Some(true));
        let n = MachineLog::parse(&format!("{s}\n{}\n", gga(&l.epochs[1]))).unwrap();
        assert_eq!(n.format, "nmea");
        assert_eq!(n.epochs[0].fix, Fix::RtkFixed);
        assert!((n.epochs[0].lat - 49.9).abs() < 1e-8 && (n.epochs[0].lon + 97.1).abs() < 1e-8);
        let bad = s.replace("*", "0*");
        assert_eq!(
            MachineLog::parse(&format!("{bad}\n{s}\n"))
                .unwrap()
                .skipped
                .len(),
            1
        );
    }

    #[test]
    fn nmea_midnight_rollover() {
        let mk = |t: f64| Epoch {
            t_s: t,
            lat: 1.0,
            lon: 2.0,
            h: None,
            fix: Fix::Autonomous,
            sats: Some(9),
            hdop: None,
            cn0: None,
            corr_age: None,
            steer: None,
            xte: None,
        };
        let l = MachineLog::parse(&format!("{}\n{}\n", gga(&mk(86_399.0)), gga(&mk(86_401.0))))
            .unwrap();
        assert!((l.epochs[1].t_s - 86_401.0).abs() < 1e-6, "{:?}", l.epochs);
    }
}
