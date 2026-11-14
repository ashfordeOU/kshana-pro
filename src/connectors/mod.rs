// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Enterprise integration (PR21) — connectors & CI hooks
//!
//! Adapters that move Kshana outputs into the formats enterprise space-ops tools
//! ingest, plus a CI gate. The open core emits the agency-standard CCSDS OEM/OMM and
//! SP3 (which GMAT and Orekit read) and, since 0.29, STK `.e` files itself
//! (`kshana::interop::stk`, the open `--export stk` of any scene). What is here:
//!
//!  * **STK `.e` from an `ephemeris` result** — the Pro command `kshana-pro export
//!    --format stk` writes one `.e` file straight from an open `ephemeris` result
//!    (J2000/GCRS, kilometres and seconds). It predates the open exporter and is kept
//!    for the scripts that call it; new work can use the open `--export stk`.
//!  * **CSV** — a universal flat export of the ephemeris state series (Excel,
//!    pandas, BI tools, custom GMAT/Orekit loaders).
//!  * **CI gate** — run any scenario, extract a metric, compare against a threshold,
//!    and return pass/fail for a pipeline exit code.

use kshana::api::KshanaError;

/// SpaceStream — the versioned, aggregated-metrics-only NAVEX→NAV-DT interface (A09).
// Arksa-exclusive (docs/PROTECTION.md): compiled only with the `arksa` feature.
#[cfg(feature = "arksa")]
pub mod spacestream;

/// The subset of an open `ephemeris` result sample the connectors need. Unknown
/// fields in the result JSON are ignored by serde, so this tracks only what we emit.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EphemSample {
    pub t_s: f64,
    pub jd_utc: f64,
    pub gcrs_r_m: [f64; 3],
    pub gcrs_v_m_s: [f64; 3],
    pub ecef_r_m: [f64; 3],
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_km: f64,
    pub speed_m_s: f64,
}

#[derive(serde::Deserialize)]
struct EphemResult {
    samples: Vec<EphemSample>,
}

/// Parse the `samples` series out of an open `ephemeris` result JSON. Errors if the
/// result is not an ephemeris run (the connectors are ephemeris-specific).
pub fn parse_ephem_samples(result_json: &str) -> Result<Vec<EphemSample>, KshanaError> {
    let r: EphemResult = serde_json::from_str(result_json).map_err(|_| {
        KshanaError::InvalidInput(
            "ephemeris export needs an `ephemeris` scenario result (no `samples` series found)"
                .into(),
        )
    })?;
    if r.samples.is_empty() {
        return Err(KshanaError::InvalidInput(
            "ephemeris result has no samples to export".into(),
        ));
    }
    Ok(r.samples)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Convert a Julian Date (UTC) to a Gregorian calendar UTC tuple
/// `(year, month, day, hour, minute, second)` via the Meeus algorithm.
pub fn jd_to_utc(jd: f64) -> (i64, u32, u32, u32, u32, f64) {
    // Round to the nearest millisecond first, then split. Integer-millisecond
    // arithmetic makes the seconds field structurally < 60 (no "60.000" rollover)
    // and lets a time that rounds up to the next midnight carry the date correctly.
    let total_ms = ((jd + 0.5) * 86_400_000.0).round();
    let z = (total_ms / 86_400_000.0).floor() as i64;
    let ms_of_day = (total_ms - z as f64 * 86_400_000.0) as i64;

    // Calendar date from the integer day z (Meeus, Astronomical Algorithms §7).
    let a = if z < 2_299_161 {
        z
    } else {
        let alpha = ((z as f64 - 1_867_216.25) / 36_524.25).floor() as i64;
        z + 1 + alpha - alpha / 4
    };
    let b = a + 1524;
    let c = ((b as f64 - 122.1) / 365.25).floor() as i64;
    let d = (365.25 * c as f64).floor() as i64;
    let ee = ((b - d) as f64 / 30.6001).floor() as i64;
    let day = ((b - d) - (30.6001 * ee as f64).floor() as i64) as u32;
    let month = (if ee < 14 { ee - 1 } else { ee - 13 }) as u32;
    let year = if month > 2 { c - 4716 } else { c - 4715 };

    // Time of day from integer milliseconds — seconds is always in [0, 60).
    let hour = (ms_of_day / 3_600_000) as u32;
    let minute = ((ms_of_day % 3_600_000) / 60_000) as u32;
    let second = (ms_of_day % 60_000) as f64 / 1000.0;
    (year, month, day, hour, minute, second)
}

/// Format a JD (UTC) as an STK UTCG epoch string, e.g. `1 Jan 2000 12:00:00.000`.
fn stk_utcg(jd: f64) -> String {
    let (y, mo, d, h, mi, s) = jd_to_utc(jd);
    let mon = MONTHS[(mo.clamp(1, 12) - 1) as usize];
    format!("{d} {mon} {y} {h:02}:{mi:02}:{s:06.3}")
}

/// Build an STK `.e` ephemeris (J2000/GCRS, metres & seconds) from the samples.
pub fn to_stk_ephemeris(samples: &[EphemSample]) -> Result<String, KshanaError> {
    if samples.is_empty() {
        return Err(KshanaError::InvalidInput("no samples to export".into()));
    }
    if !samples[0].jd_utc.is_finite() {
        return Err(KshanaError::InvalidInput(
            "first sample has a non-finite jd_utc (cannot form the STK ScenarioEpoch)".into(),
        ));
    }
    // STK `.e` distance/velocity default to KILOMETRES & km/s — emit km so STK does
    // not silently read metres as km (a 1000x position error).
    const M_TO_KM: f64 = 1.0e-3;
    // Lagrange interpolation needs M1+1 points; never request more than we have.
    let m1 = (samples.len() - 1).min(5);

    let mut s = String::new();
    s.push_str("stk.v.11.0\n\n");
    s.push_str("# Generated by Kshana Pro — J2000 (GCRS) ephemeris, kilometres & seconds\n");
    s.push_str("BEGIN Ephemeris\n\n");
    s.push_str(&format!("NumberOfEphemerisPoints {}\n", samples.len()));
    s.push_str(&format!(
        "ScenarioEpoch            {}\n",
        stk_utcg(samples[0].jd_utc)
    ));
    s.push_str("InterpolationMethod      Lagrange\n");
    s.push_str(&format!("InterpolationSamplesM1   {m1}\n"));
    s.push_str("CentralBody              Earth\n");
    s.push_str("CoordinateSystem         J2000\n");
    s.push_str("DistanceUnit             Kilometers\n\n");
    s.push_str("EphemerisTimePosVel\n\n");
    // Times are seconds past ScenarioEpoch; the first sample is the epoch (t=0).
    let t0 = samples[0].t_s;
    for p in samples {
        s.push_str(&format!(
            "{:.6} {:.9} {:.9} {:.9} {:.12} {:.12} {:.12}\n",
            p.t_s - t0,
            p.gcrs_r_m[0] * M_TO_KM,
            p.gcrs_r_m[1] * M_TO_KM,
            p.gcrs_r_m[2] * M_TO_KM,
            p.gcrs_v_m_s[0] * M_TO_KM,
            p.gcrs_v_m_s[1] * M_TO_KM,
            p.gcrs_v_m_s[2] * M_TO_KM,
        ));
    }
    s.push_str("\nEND Ephemeris\n");
    Ok(s)
}

/// Build a universal CSV of the ephemeris state series.
pub fn to_csv(samples: &[EphemSample]) -> Result<String, KshanaError> {
    if samples.is_empty() {
        return Err(KshanaError::InvalidInput("no samples to export".into()));
    }
    let mut s = String::new();
    s.push_str("t_s,jd_utc,gcrs_x_m,gcrs_y_m,gcrs_z_m,gcrs_vx_m_s,gcrs_vy_m_s,gcrs_vz_m_s,ecef_x_m,ecef_y_m,ecef_z_m,lat_deg,lon_deg,alt_km,speed_m_s\n");
    for p in samples {
        s.push_str(&format!(
            "{:.6},{:.9},{:.6},{:.6},{:.6},{:.9},{:.9},{:.9},{:.6},{:.6},{:.6},{:.9},{:.9},{:.6},{:.6}\n",
            p.t_s, p.jd_utc,
            p.gcrs_r_m[0], p.gcrs_r_m[1], p.gcrs_r_m[2],
            p.gcrs_v_m_s[0], p.gcrs_v_m_s[1], p.gcrs_v_m_s[2],
            p.ecef_r_m[0], p.ecef_r_m[1], p.ecef_r_m[2],
            p.lat_deg, p.lon_deg, p.alt_km, p.speed_m_s,
        ));
    }
    Ok(s)
}

/// A CI-gate comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl GateOp {
    pub fn parse(s: &str) -> Result<GateOp, KshanaError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "lt" | "<" => Ok(GateOp::Lt),
            "le" | "<=" => Ok(GateOp::Le),
            "gt" | ">" => Ok(GateOp::Gt),
            "ge" | ">=" => Ok(GateOp::Ge),
            other => Err(KshanaError::InvalidInput(format!(
                "unknown gate op '{other}' (expected lt|le|gt|ge)"
            ))),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            GateOp::Lt => "<",
            GateOp::Le => "<=",
            GateOp::Gt => ">",
            GateOp::Ge => ">=",
        }
    }
    /// Whether `value <op> threshold` holds. A non-finite value never passes.
    pub fn passes(self, value: f64, threshold: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        match self {
            GateOp::Lt => value < threshold,
            GateOp::Le => value <= threshold,
            GateOp::Gt => value > threshold,
            GateOp::Ge => value >= threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t: f64, jd: f64) -> EphemSample {
        EphemSample {
            t_s: t,
            jd_utc: jd,
            gcrs_r_m: [1.0e6, 2.0e6, 3.0e6],
            gcrs_v_m_s: [10.0, 20.0, 30.0],
            ecef_r_m: [4.0e6, 5.0e6, 6.0e6],
            lat_deg: 45.0,
            lon_deg: -120.0,
            alt_km: 400.0,
            speed_m_s: 7600.0,
        }
    }

    #[test]
    fn jd_to_utc_matches_j2000_epoch() {
        // JD 2451545.0 = 2000-01-01 12:00:00 UTC (the J2000 epoch).
        let (y, mo, d, h, mi, s) = jd_to_utc(2_451_545.0);
        assert_eq!((y, mo, d, h, mi), (2000, 1, 1, 12, 0));
        assert!(s.abs() < 1e-3, "seconds ~0, got {s}");
    }

    #[test]
    fn jd_to_utc_matches_unix_epoch() {
        // JD 2440587.5 = 1970-01-01 00:00:00 UTC.
        let (y, mo, d, h, mi, _s) = jd_to_utc(2_440_587.5);
        assert_eq!((y, mo, d, h, mi), (1970, 1, 1, 0, 0));
    }

    #[test]
    fn stk_ephemeris_has_required_structure() {
        let s = [sample(0.0, 2_451_545.0), sample(60.0, 2_451_545.000694)];
        let e = to_stk_ephemeris(&s).unwrap();
        assert!(e.starts_with("stk.v.11.0"));
        assert!(e.contains("NumberOfEphemerisPoints 2"));
        assert!(e.contains("CoordinateSystem         J2000"));
        assert!(e.contains("DistanceUnit             Kilometers"));
        assert!(e.contains("ScenarioEpoch            1 Jan 2000 12:00:00.000"));
        assert!(e.contains("EphemerisTimePosVel"));
        assert!(e.trim_end().ends_with("END Ephemeris"));
        // 2 samples → 1-degree interpolation (never request more points than exist).
        assert!(e.contains("InterpolationSamplesM1   1"));
        // First data row at t=0; position is in KILOMETRES (1e6 m → 1000 km).
        assert!(
            e.contains("0.000000 1000.000000000 2000.000000000 3000.000000000"),
            "positions must be in km: {e}"
        );
    }

    #[test]
    fn stk_rejects_nonfinite_epoch() {
        let s = [sample(0.0, f64::NAN)];
        assert!(to_stk_ephemeris(&s).is_err());
    }

    #[test]
    fn jd_to_utc_does_not_emit_60_seconds() {
        // A JD whose time-of-day is a hair under midnight must round to the next day
        // at 00:00:00, never "...:60.000".
        let (_, _, _, h, mi, s) = jd_to_utc(2_451_545.0 - 0.5 + 0.999_999_99); // ~23:59:59.999
        assert!(s < 60.0, "seconds must be < 60, got {s}");
        // Either it stayed at 23:59:xx or carried to 00:00 — both have s < 60.
        assert!(h < 24 && mi < 60);
    }

    #[test]
    fn csv_has_header_and_one_row_per_sample() {
        let s = [sample(0.0, 2_451_545.0), sample(60.0, 2_451_545.000694)];
        let csv = to_csv(&s).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines[0].starts_with("t_s,jd_utc,gcrs_x_m"));
        assert_eq!(lines.len(), 3); // header + 2 rows
        assert_eq!(lines[1].matches(',').count(), 14); // 15 columns
    }

    #[test]
    fn parse_ephem_samples_rejects_non_ephemeris() {
        assert!(parse_ephem_samples("{\"foo\":1}").is_err());
        assert!(parse_ephem_samples("{\"samples\":[]}").is_err());
    }

    #[test]
    fn gate_ops_evaluate_correctly() {
        assert!(GateOp::Lt.passes(1.0, 2.0));
        assert!(!GateOp::Lt.passes(2.0, 2.0));
        assert!(GateOp::Le.passes(2.0, 2.0));
        assert!(GateOp::Gt.passes(3.0, 2.0));
        assert!(GateOp::Ge.passes(2.0, 2.0));
        assert!(!GateOp::Ge.passes(1.0, 2.0));
        assert!(!GateOp::Lt.passes(f64::NAN, 2.0), "NaN never passes");
        assert_eq!(GateOp::parse("<=").unwrap(), GateOp::Le);
        assert!(GateOp::parse("bogus").is_err());
    }

    #[test]
    fn open_core_has_its_own_stk_exporter() {
        // F8d: the note above says the open engine writes STK `.e` itself. If the open
        // exporter is ever removed, this stops compiling and the note must change.
        let f: fn(&str) -> String = kshana::interop::stk::file_part;
        assert!(!f("sat-1").is_empty());
    }
}
