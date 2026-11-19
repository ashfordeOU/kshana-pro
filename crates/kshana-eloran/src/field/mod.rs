// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Field data (stream D-field): unattended LF recording stations and receiver log
//! importers.
//!
//! * [`station`]: an unattended recorder loop over any [`crate::hw::Device`] receive side —
//!   segmented SigMF recordings with UTC/PPS timestamps, automatic chain detection per
//!   segment with the software receiver against the GRI database, health checks,
//!   rotation/retention, an upload manifest (SHA-256 per file) and a resumable state file.
//! * [`importers`]: receiver logs (generic CSV, NMEA-0183, proprietary field-split
//!   sentences, time-interval-counter 1PPS CSV) into one observation record
//!   ([`importers::ObsRecord`]), and comparisons with simulator truth and with the software
//!   receiver.
//!
//! Evidence: everything here is MODELLED and tested against synthetic data only (a mock
//! receive device fed by a simulated scene, synthetic logs). Nothing has been run at a
//! field site or against a real receiver's logs; see each module's "Needs" section.

pub mod importers;
pub mod station;

use crate::time::{civil_from_days, days_from_civil, tai_minus_utc_s, AbsTime, PS_PER_S};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Nanoseconds per second.
const NS: i128 = 1_000_000_000;

/// [`AbsTime`] (TAI since 1958) of a UTC instant given as POSIX nanoseconds (seconds since
/// 1970-01-01T00:00:00 UTC without leap seconds, the usual device/host clock scale).
pub fn abs_from_unix_ns(ns: i128) -> AbsTime {
    let s = ns.div_euclid(NS);
    let frac_ns = ns.rem_euclid(NS);
    let days = s.div_euclid(86_400) as i64;
    let (y, m, d) = civil_from_days(days);
    let leap = tai_minus_utc_s(y, m, d) as i128;
    let d1958 = days_from_civil(1958, 1, 1) as i128;
    let tai = s - d1958 * 86_400 + leap;
    AbsTime(tai * PS_PER_S as i128 + frac_ns * 1_000)
}

/// POSIX nanoseconds (UTC) of an [`AbsTime`] (inverse of [`abs_from_unix_ns`]; an instant
/// inside a leap second reads as the following second).
pub fn unix_ns_from_abs(t: AbsTime) -> i128 {
    let ps = PS_PER_S as i128;
    let tai_s = t.0.div_euclid(ps);
    let frac_ns = t.0.rem_euclid(ps) / 1_000;
    let d1958 = days_from_civil(1958, 1, 1) as i128;
    let leap_of = |s: i128| {
        let (y, m, d) = civil_from_days((s.div_euclid(86_400) + d1958) as i64);
        tai_minus_utc_s(y, m, d) as i128
    };
    let utc_s = tai_s - leap_of(tai_s - leap_of(tai_s));
    (utc_s + d1958 * 86_400) * NS + frac_ns
}

/// POSIX seconds (UTC, fractional) of an [`AbsTime`].
pub fn unix_s_from_abs(t: AbsTime) -> f64 {
    let ns = unix_ns_from_abs(t);
    ns.div_euclid(NS) as f64 + ns.rem_euclid(NS) as f64 * 1e-9
}

/// ISO 8601 UTC string `YYYY-MM-DDTHH:MM:SS.ffffffZ` (microseconds) of POSIX nanoseconds.
pub fn iso_from_unix_ns(ns: i128) -> String {
    let s = ns.div_euclid(NS);
    let us = ns.rem_euclid(NS) / 1_000;
    let (y, m, d) = civil_from_days(s.div_euclid(86_400) as i64);
    let sod = s.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{us:06}Z",
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    )
}

/// POSIX nanoseconds of an ISO 8601 UTC string `YYYY-MM-DDTHH:MM:SS[.fff]Z`.
pub fn unix_ns_from_iso(s: &str) -> Result<i128, String> {
    Ok(unix_ns_from_abs(AbsTime::from_utc_iso(s)?))
}

/// POSIX seconds of a UTC calendar date and time of day.
pub fn unix_s_from_civil(y: i64, m: i64, d: i64, sod: f64) -> f64 {
    days_from_civil(y, m, d) as f64 * 86_400.0 + sod
}

/// Lower-case hex SHA-256 of a byte slice.
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Lower-case hex SHA-256 of a file (streamed).
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Write a file atomically (temporary file in the same directory, then rename).
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp-write");
    std::fs::write(&tmp, data).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_abs_round_trip_and_leap_seconds() {
        // 2026-11-18T00:00:00Z
        let ns = unix_ns_from_iso("2026-11-18T00:00:00Z").unwrap();
        assert_eq!(ns, 1_794_960_000 * NS);
        let t = abs_from_unix_ns(ns + 123_456_789);
        assert_eq!(
            t,
            AbsTime::from_utc_iso("2026-11-18T00:00:00.123456789Z").unwrap()
        );
        assert_eq!(unix_ns_from_abs(t), ns + 123_456_789);
        assert_eq!(
            iso_from_unix_ns(ns + 123_456_789),
            "2026-11-18T00:00:00.123456Z"
        );
        // across the 2017-01-01 leap second: one UTC second apart is two TAI seconds
        let a = abs_from_unix_ns(unix_ns_from_iso("2016-12-31T23:59:59Z").unwrap());
        let b = abs_from_unix_ns(unix_ns_from_iso("2017-01-01T00:00:00Z").unwrap());
        assert_eq!(b.0 - a.0, 2 * PS_PER_S as i128);
        assert_eq!(unix_s_from_civil(1970, 1, 2, 1.5), 86_401.5);
    }

    #[test]
    fn sha256_of_known_input() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
