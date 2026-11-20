// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! B2001: the ground-interference envelope seen by a spacecraft GNSS receiver.
//!
//! The orbit is a two-line element set propagated with the open engine's SGP4
//! ([`kshana::sgp4::SgpOrbit`], TEME to ITRS). Interference **areas** are declared (centre,
//! radius, assumed EIRPs) or taken from the degraded cells of an open-engine interference map
//! (GeoJSON, `status` degraded or anomalous, or observatory `state` degraded). An area is a
//! region, never an emitter position: the model puts a transmitter at the point of the area
//! nearest the sub-satellite point (the worst case for the receiver) and says so.
//!
//! Per time step and area in radio view (spacecraft above the elevation mask seen from that
//! point): the receive-antenna gain toward the ground (the antenna pattern over the angle off
//! boresight), free-space loss, J/S and the effective C/N0 from the open engine's anti-jam
//! equation ([`kshana::jamming::effective_cn0_dbhz`]), and the Doppler offset of a ground
//! transmitter's carrier at the spacecraft (its range rate; a ground transmitter that emulates
//! signals for a static ground user is offset from the authentic signals by about this much).
//!
//! Outputs: passes per area, exposure per revolution, and the **test envelope** a receiver
//! team uses to set simulator test levels (percentiles of J/S per assumed EIRP, pass dwell,
//! Doppler offset and Doppler rate) plus the time above the receiver's tested J/S tolerance
//! when that is supplied. Everything is MODELLED; EIRPs and antenna gains are assumptions.

use std::path::Path;

use kshana::frames::{ecef_to_geodetic, geodetic_to_ecef, Geodetic};
use kshana::jamming::{
    effective_cn0_dbhz, free_space_path_loss_db, nominal_cn0_dbhz, CA_CHIP_RATE_HZ,
    DEFAULT_SIGNAL_POWER_DBW, DEFAULT_TEMP_K, DEFAULT_TRACKING_THRESHOLD_DBHZ, L1_HZ, Q_BROADBAND,
};
use kshana::jd2::Jd2;
use kshana::sgp4::{teme_to_itrs_matrix, SgpOrbit};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{quantile, round, sha256_hex, LABEL};

/// The scenario format.
pub const FORMAT: &str = "kshana-space-rx-exposure/1";
/// The result schema.
pub const SCHEMA: &str = "kshana-space-rx-exposure-result/1";
/// The most areas one run takes (map cells included).
pub const MAX_AREAS: usize = 2000;

const DEG: f64 = std::f64::consts::PI / 180.0;
const C: f64 = 299_792_458.0;
const R_EARTH_KM: f64 = 6371.0;

fn d_duration() -> f64 {
    24.0
}
fn d_step() -> f64 {
    10.0
}
fn d_eirp() -> Vec<f64> {
    vec![20.0, 40.0, 60.0]
}
fn d_boresight() -> String {
    "zenith".into()
}
fn d_gnss_gain() -> f64 {
    3.0
}
fn d_signal() -> f64 {
    DEFAULT_SIGNAL_POWER_DBW
}
fn d_temp() -> f64 {
    DEFAULT_TEMP_K
}
fn d_threshold() -> f64 {
    DEFAULT_TRACKING_THRESHOLD_DBHZ
}
fn d_q() -> f64 {
    Q_BROADBAND
}
fn d_carrier() -> f64 {
    L1_HZ
}
fn d_height() -> f64 {
    10.0
}

/// A generic zenith patch (off-boresight degrees, dBi). An assumption, stated in the result.
pub const DEFAULT_PATTERN: [[f64; 2]; 7] = [
    [0.0, 5.0],
    [30.0, 4.0],
    [60.0, 1.0],
    [90.0, -5.0],
    [120.0, -15.0],
    [150.0, -22.0],
    [180.0, -25.0],
];

/// The receiver as the model sees it. Every default is stated in the result.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receiver {
    pub name: String,
    /// `zenith` (the usual navigation antenna) or `nadir`.
    #[serde(default = "d_boresight")]
    pub boresight: String,
    /// Gain over the angle off boresight, `[[deg, dBi], ...]`, increasing angles from 0 to 180.
    #[serde(default)]
    pub pattern: Vec<[f64; 2]>,
    /// Gain toward the GNSS satellites tracked, dBi.
    #[serde(default = "d_gnss_gain")]
    pub gnss_gain_dbi: f64,
    /// Received GNSS signal power at an isotropic antenna, dBW.
    #[serde(default = "d_signal")]
    pub signal_power_dbw: f64,
    #[serde(default = "d_temp")]
    pub temp_k: f64,
    /// The receiver's tracking threshold, dB-Hz.
    #[serde(default = "d_threshold")]
    pub tracking_threshold_dbhz: f64,
    /// Spectral-separation coefficient of the interference (2 = broadband, 1 = CW).
    #[serde(default = "d_q")]
    pub q: f64,
    #[serde(default = "d_carrier")]
    pub carrier_hz: f64,
    /// The J/S the receiver was tested to (for example from its campaign scorecard), dB.
    #[serde(default)]
    pub tested_js_db: Option<f64>,
    #[serde(default)]
    pub tested_js_source: String,
}

/// A declared interference area.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaSpec {
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub radius_km: f64,
    /// Assumed EIRPs for this area (dBW); the scenario's list when absent.
    #[serde(default)]
    pub eirp_dbw: Option<Vec<f64>>,
    #[serde(default = "d_height")]
    pub height_m: f64,
    /// Where the area comes from (a report, a map, an assumption).
    #[serde(default)]
    pub source: String,
}

/// The scenario (`kshana-space-rx-exposure/1`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub format: String,
    pub name: String,
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    pub start_utc: String,
    #[serde(default = "d_duration")]
    pub duration_h: f64,
    #[serde(default = "d_step")]
    pub step_s: f64,
    /// The two lines of the element set.
    pub tle: Vec<String>,
    #[serde(default)]
    pub elevation_mask_deg: f64,
    /// Assumed EIRPs (dBW) for areas without their own and for map cells.
    #[serde(default = "d_eirp")]
    pub eirp_dbw: Vec<f64>,
    pub receiver: Receiver,
    #[serde(default)]
    pub area: Vec<AreaSpec>,
    /// Open-engine interference maps (GeoJSON), relative to the scenario file.
    #[serde(default)]
    pub map: Vec<String>,
}

impl Scenario {
    pub fn parse(src: &str) -> Result<Self, String> {
        let s: Scenario = toml::from_str(src).map_err(|e| format!("scenario: {e}"))?;
        if s.format != FORMAT {
            return Err(format!("scenario: format must be \"{FORMAT}\""));
        }
        if s.tle.len() != 2 {
            return Err("scenario: tle must hold the two lines of one element set".into());
        }
        if !(s.step_s >= 1.0 && s.step_s <= 600.0) {
            return Err("scenario: step_s must be in 1..=600".into());
        }
        if !(s.duration_h > 0.0 && s.duration_h <= 24.0 * 31.0) {
            return Err("scenario: duration_h must be in (0, 744]".into());
        }
        if s.eirp_dbw.is_empty() {
            return Err("scenario: eirp_dbw needs at least one value".into());
        }
        let b = s.receiver.boresight.as_str();
        if b != "zenith" && b != "nadir" {
            return Err("receiver.boresight must be zenith or nadir".into());
        }
        let p = &s.receiver.pattern;
        if !p.is_empty()
            && (p.len() < 2
                || p.windows(2).any(|w| w[1][0] <= w[0][0])
                || p[0][0] < 0.0
                || p[p.len() - 1][0] > 180.0)
        {
            return Err("receiver.pattern: angles must increase within 0..=180".into());
        }
        for a in &s.area {
            if !(a.lat_deg.abs() <= 90.0 && a.lon_deg.abs() <= 180.0 && a.radius_km >= 0.0) {
                return Err(format!("area {}: bad centre or radius", a.name));
            }
        }
        Ok(s)
    }
}

/// An area as flown over.
#[derive(Clone, Debug, Serialize)]
pub struct Area {
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub radius_km: f64,
    pub height_m: f64,
    pub eirp_dbw: Vec<f64>,
    pub source: String,
}

/// The areas of a scenario: declared ones, then degraded cells of the maps.
pub fn areas(s: &Scenario, base: &Path) -> Result<Vec<Area>, String> {
    let mut out: Vec<Area> = s
        .area
        .iter()
        .map(|a| Area {
            name: a.name.clone(),
            lat_deg: a.lat_deg,
            lon_deg: a.lon_deg,
            radius_km: a.radius_km,
            height_m: a.height_m,
            eirp_dbw: a.eirp_dbw.clone().unwrap_or_else(|| s.eirp_dbw.clone()),
            source: if a.source.is_empty() {
                "declared".into()
            } else {
                a.source.clone()
            },
        })
        .collect();
    for m in &s.map {
        let path = base.join(m);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.extend(map_cells(&text, m, &s.eirp_dbw)?);
    }
    if out.is_empty() {
        return Err("scenario: no areas (declare [[area]] or give a map)".into());
    }
    if out.len() > MAX_AREAS {
        return Err(format!("{} areas; at most {MAX_AREAS}", out.len()));
    }
    Ok(out)
}

/// Degraded cells of an open-engine interference map (GeoJSON polygons) as areas: centre of
/// the cell's bounding box, radius its half-diagonal.
pub fn map_cells(text: &str, label: &str, eirp: &[f64]) -> Result<Vec<Area>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("{label}: {e}"))?;
    let feats = v["features"]
        .as_array()
        .ok_or_else(|| format!("{label}: not a GeoJSON FeatureCollection"))?;
    let mut out = Vec::new();
    for (i, f) in feats.iter().enumerate() {
        let p = &f["properties"];
        let status = p["status"].as_str().or(p["state"].as_str()).unwrap_or("");
        if status != "degraded" && status != "anomalous" {
            continue;
        }
        let Some(ring) = f["geometry"]["coordinates"][0].as_array() else {
            continue;
        };
        let pts: Vec<(f64, f64)> = ring
            .iter()
            .filter_map(|c| Some((c[0].as_f64()?, c[1].as_f64()?)))
            .collect();
        if pts.is_empty() {
            continue;
        }
        let (mut w, mut e, mut s, mut n) = (180.0f64, -180.0f64, 90.0f64, -90.0f64);
        for (lon, lat) in &pts {
            w = w.min(*lon);
            e = e.max(*lon);
            s = s.min(*lat);
            n = n.max(*lat);
        }
        let (clat, clon) = ((s + n) / 2.0, (w + e) / 2.0);
        let dy = (n - s) / 2.0 * 111.2;
        let dx = (e - w) / 2.0 * 111.2 * (clat * DEG).cos();
        out.push(Area {
            name: format!("{label}#{i}"),
            lat_deg: clat,
            lon_deg: clon,
            radius_km: (dx * dx + dy * dy).sqrt(),
            height_m: d_height(),
            eirp_dbw: eirp.to_vec(),
            source: format!("map cell ({status}) of {label}"),
        });
    }
    Ok(out)
}

/// `YYYY-MM-DDTHH:MM:SS[.s]Z` to a UTC two-part Julian date.
pub fn parse_utc(s: &str) -> Result<Jd2, String> {
    let bad = || format!("time '{s}': expected YYYY-MM-DDTHH:MM:SSZ");
    let t = s.strip_suffix('Z').ok_or_else(bad)?;
    let (d, h) = t.split_once('T').ok_or_else(bad)?;
    let dp: Vec<&str> = d.split('-').collect();
    let hp: Vec<&str> = h.split(':').collect();
    if dp.len() != 3 || hp.len() != 3 {
        return Err(bad());
    }
    let n = |x: &str| x.parse::<u32>().map_err(|_| bad());
    Jd2::from_utc_calendar(
        dp[0].parse::<i32>().map_err(|_| bad())?,
        n(dp[1])?,
        n(dp[2])?,
        n(hp[0])?,
        n(hp[1])?,
        hp[2].parse::<f64>().map_err(|_| bad())?,
    )
}

/// Gain of `pattern` at `deg` off boresight (linear interpolation, clamped).
pub fn gain_at(pattern: &[[f64; 2]], deg: f64) -> f64 {
    let p: &[[f64; 2]] = if pattern.is_empty() {
        &DEFAULT_PATTERN
    } else {
        pattern
    };
    if deg <= p[0][0] {
        return p[0][1];
    }
    for w in p.windows(2) {
        if deg <= w[1][0] {
            let f = (deg - w[0][0]) / (w[1][0] - w[0][0]);
            return w[0][1] + f * (w[1][1] - w[0][1]);
        }
    }
    p[p.len() - 1][1]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn unit_ll(lat: f64, lon: f64) -> [f64; 3] {
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// The point of the area's disc nearest the sub-satellite point (lat, lon in radians).
fn nearest_point(a: &Area, sub_lat: f64, sub_lon: f64) -> (f64, f64) {
    let uc = unit_ll(a.lat_deg * DEG, a.lon_deg * DEG);
    let us = unit_ll(sub_lat, sub_lon);
    let th = dot(uc, us).clamp(-1.0, 1.0).acos();
    let ra = a.radius_km / R_EARTH_KM;
    let u = if th <= ra {
        us
    } else {
        let f = ra / th;
        let (s0, s1) = (((1.0 - f) * th).sin(), (f * th).sin());
        let st = th.sin().max(1e-12);
        let v = [
            (s0 * uc[0] + s1 * us[0]) / st,
            (s0 * uc[1] + s1 * us[1]) / st,
            (s0 * uc[2] + s1 * us[2]) / st,
        ];
        let n = norm(v);
        [v[0] / n, v[1] / n, v[2] / n]
    };
    (u[2].clamp(-1.0, 1.0).asin(), u[1].atan2(u[0]))
}

/// One sample of an area in view.
#[derive(Clone, Copy, Debug)]
struct Sample {
    step: usize,
    elev_deg: f64,
    range_km: f64,
    off_boresight_deg: f64,
    gain_dbi: f64,
    doppler_hz: f64,
}

/// A pass of one area through radio view.
#[derive(Clone, Debug, Serialize)]
pub struct Pass {
    pub area: String,
    pub start_utc_offset_s: f64,
    pub dwell_s: f64,
    pub max_elevation_deg: f64,
    pub min_range_km: f64,
    /// The smallest angle off the antenna boresight toward the area, degrees.
    pub min_off_boresight_deg: f64,
    /// Per assumed EIRP: worst J/S (dB), lowest effective C/N0 (dB-Hz), time below the
    /// tracking threshold (s), time above the tested J/S (s, when supplied).
    pub per_eirp: Vec<PassEirp>,
    pub doppler_min_hz: f64,
    pub doppler_max_hz: f64,
    pub max_abs_doppler_rate_hz_s: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PassEirp {
    pub eirp_dbw: f64,
    pub max_js_db: f64,
    pub min_cn0_eff_dbhz: f64,
    pub below_threshold_s: f64,
    pub above_tested_js_s: Option<f64>,
}

/// Run a scenario; `base` resolves map paths.
pub fn run(s: &Scenario, base: &Path) -> Result<Value, String> {
    let areas = areas(s, base)?;
    let orbit = SgpOrbit::from_tle(&s.tle[0], &s.tle[1]).map_err(|e| format!("tle: {e}"))?;
    let t0 = parse_utc(&s.start_utc)?;
    let n = ((s.duration_h * 3600.0) / s.step_s).floor() as usize + 1;
    let rx = &s.receiver;
    let cn0_nom = nominal_cn0_dbhz(rx.signal_power_dbw, rx.gnss_gain_dbi, rx.temp_k);
    let s_dbw = rx.signal_power_dbw + rx.gnss_gain_dbi;
    // Per area: the samples in view; per step: whether any area is in view.
    let mut seen: Vec<Vec<Sample>> = vec![Vec::new(); areas.len()];
    let mut any_view = vec![false; n];
    let mut sat_alt_km = Vec::with_capacity(n);
    for (k, view) in any_view.iter_mut().enumerate() {
        let t = t0.add_seconds(k as f64 * s.step_s);
        let (r, v) = orbit.itrs_state_with(t, &teme_to_itrs_matrix(t))?;
        let g = ecef_to_geodetic(r);
        sat_alt_km.push(g.alt_m / 1e3);
        let r_hat = {
            let m = norm(r);
            [r[0] / m, r[1] / m, r[2] / m]
        };
        let bore = if rx.boresight == "nadir" {
            [-r_hat[0], -r_hat[1], -r_hat[2]]
        } else {
            r_hat
        };
        for (i, a) in areas.iter().enumerate() {
            let (plat, plon) = nearest_point(a, g.lat_rad, g.lon_rad);
            let gp = geodetic_to_ecef(Geodetic {
                lat_rad: plat,
                lon_rad: plon,
                alt_m: a.height_m,
            });
            let los = sub(r, gp);
            let d = norm(los);
            let up = unit_ll(plat, plon);
            let elev = (dot(los, up) / d).clamp(-1.0, 1.0).asin() / DEG;
            if elev < s.elevation_mask_deg {
                continue;
            }
            *view = true;
            let to_ground = [-los[0] / d, -los[1] / d, -los[2] / d];
            let off = dot(bore, to_ground).clamp(-1.0, 1.0).acos() / DEG;
            let range_rate = dot(los, v) / d;
            seen[i].push(Sample {
                step: k,
                elev_deg: elev,
                range_km: d / 1e3,
                off_boresight_deg: off,
                gain_dbi: gain_at(&rx.pattern, off),
                doppler_hz: -range_rate / C * rx.carrier_hz,
            });
        }
    }
    let js_of = |eirp: f64, smp: &Sample| -> f64 {
        eirp - free_space_path_loss_db(smp.range_km * 1e3, rx.carrier_hz) + smp.gain_dbi - s_dbw
    };
    let cn0_of = |js: f64| effective_cn0_dbhz(cn0_nom, js, rx.q, CA_CHIP_RATE_HZ);

    // Passes.
    let mut passes = Vec::new();
    let mut js_all: Vec<Vec<f64>> = vec![Vec::new(); s.eirp_dbw.len()];
    let mut lost_steps: Vec<Vec<bool>> = vec![vec![false; n]; s.eirp_dbw.len()];
    let mut doppler_all = Vec::new();
    let mut rate_all = Vec::new();
    for (i, a) in areas.iter().enumerate() {
        let smp = &seen[i];
        let mut j = 0;
        while j < smp.len() {
            let mut e = j;
            while e + 1 < smp.len() && smp[e + 1].step == smp[e].step + 1 {
                e += 1;
            }
            let p = &smp[j..=e];
            let mut rates = Vec::new();
            for w in p.windows(2) {
                rates.push((w[1].doppler_hz - w[0].doppler_hz) / s.step_s);
            }
            let max_rate = rates.iter().fold(0.0f64, |m, x| m.max(x.abs()));
            rate_all.extend(rates.iter().map(|x| x.abs()));
            doppler_all.extend(p.iter().map(|x| x.doppler_hz.abs()));
            let mut per_eirp = Vec::new();
            for e_dbw in &a.eirp_dbw {
                let js: Vec<f64> = p.iter().map(|x| js_of(*e_dbw, x)).collect();
                if let Some(ix) = s.eirp_dbw.iter().position(|x| x == e_dbw) {
                    js_all[ix].extend(js.iter().copied());
                    for (x, jv) in p.iter().zip(&js) {
                        if cn0_of(*jv) < rx.tracking_threshold_dbhz {
                            lost_steps[ix][x.step] = true;
                        }
                    }
                }
                let below = js
                    .iter()
                    .filter(|jv| cn0_of(**jv) < rx.tracking_threshold_dbhz)
                    .count() as f64
                    * s.step_s;
                per_eirp.push(PassEirp {
                    eirp_dbw: *e_dbw,
                    max_js_db: round(js.iter().fold(f64::MIN, |m, x| m.max(*x)), 2),
                    min_cn0_eff_dbhz: round(
                        js.iter().map(|x| cn0_of(*x)).fold(f64::MAX, f64::min),
                        2,
                    ),
                    below_threshold_s: below,
                    above_tested_js_s: rx
                        .tested_js_db
                        .map(|t| js.iter().filter(|x| **x > t).count() as f64 * s.step_s),
                });
            }
            passes.push(Pass {
                area: a.name.clone(),
                start_utc_offset_s: p[0].step as f64 * s.step_s,
                dwell_s: p.len() as f64 * s.step_s,
                max_elevation_deg: round(p.iter().fold(-90.0f64, |m, x| m.max(x.elev_deg)), 2),
                min_range_km: round(p.iter().fold(f64::MAX, |m, x| m.min(x.range_km)), 1),
                min_off_boresight_deg: round(
                    p.iter().fold(180.0f64, |m, x| m.min(x.off_boresight_deg)),
                    1,
                ),
                per_eirp,
                doppler_min_hz: round(p.iter().fold(f64::MAX, |m, x| m.min(x.doppler_hz)), 1),
                doppler_max_hz: round(p.iter().fold(f64::MIN, |m, x| m.max(x.doppler_hz)), 1),
                max_abs_doppler_rate_hz_s: round(max_rate, 2),
            });
            j = e + 1;
        }
    }
    passes.sort_by(|a, b| {
        a.start_utc_offset_s
            .total_cmp(&b.start_utc_offset_s)
            .then(a.area.cmp(&b.area))
    });

    // Per revolution.
    let period = orbit.period_s();
    let steps_per_rev = (period / s.step_s).round().max(1.0) as usize;
    let mut revs = Vec::new();
    for (r, chunk) in any_view.chunks(steps_per_rev).enumerate() {
        let lo = r * steps_per_rev;
        let frac = chunk.iter().filter(|x| **x).count() as f64 / chunk.len() as f64;
        let lost: Vec<Value> = s
            .eirp_dbw
            .iter()
            .enumerate()
            .map(|(ix, e)| {
                let c = lost_steps[ix][lo..lo + chunk.len()]
                    .iter()
                    .filter(|x| **x)
                    .count();
                json!({"eirp_dbw": e, "below_threshold_fraction": round(c as f64 / chunk.len() as f64, 4)})
            })
            .collect();
        revs.push(json!({
            "revolution": r + 1,
            "start_utc_offset_s": lo as f64 * s.step_s,
            "in_view_fraction": round(frac, 4),
            "per_eirp": lost,
        }));
    }

    // Test envelope.
    let q3 = |v: &[f64]| -> Value {
        match (quantile(v, 0.5), quantile(v, 0.95), quantile(v, 1.0)) {
            (Some(a), Some(b), Some(c)) => {
                json!({"p50": round(a, 2), "p95": round(b, 2), "max": round(c, 2)})
            }
            _ => Value::Null,
        }
    };
    let dwell: Vec<f64> = passes.iter().map(|p| p.dwell_s).collect();
    let js_env: Vec<Value> = s
        .eirp_dbw
        .iter()
        .enumerate()
        .map(|(ix, e)| {
            let above = rx.tested_js_db.map(|t| {
                js_all[ix].iter().filter(|x| **x > t).count() as f64 * s.step_s
            });
            json!({
                "eirp_dbw": e,
                "js_db": q3(&js_all[ix]),
                "time_below_threshold_s": lost_steps[ix].iter().filter(|x| **x).count() as f64 * s.step_s,
                "time_above_tested_js_s": above,
            })
        })
        .collect();
    let test_points: Vec<Value> = s
        .eirp_dbw
        .iter()
        .enumerate()
        .filter_map(|(ix, e)| {
            let js95 = quantile(&js_all[ix], 0.95)?;
            let jsmax = quantile(&js_all[ix], 1.0)?;
            Some(json!({
                "id": format!("eirp{}-p95", e),
                "eirp_dbw": e,
                "js_db": round(js95, 1),
                "js_max_db": round(jsmax, 1),
                "dwell_s": round(quantile(&dwell, 0.95).unwrap_or(0.0), 0),
                "doppler_offset_hz": round(quantile(&doppler_all, 0.95).unwrap_or(0.0), 0),
                "doppler_rate_hz_s": round(quantile(&rate_all, 0.95).unwrap_or(0.0), 1),
                "beyond_tested": rx.tested_js_db.map(|t| jsmax > t),
            }))
        })
        .collect();
    let in_view = any_view.iter().filter(|x| **x).count() as f64 / n as f64;
    let area_rows: Vec<Value> = areas
        .iter()
        .enumerate()
        .map(|(i, a)| {
            json!({
                "name": a.name, "lat_deg": a.lat_deg, "lon_deg": a.lon_deg,
                "radius_km": round(a.radius_km, 1), "eirp_dbw": a.eirp_dbw, "source": a.source,
                "time_in_view_s": seen[i].len() as f64 * s.step_s,
                "passes": passes.iter().filter(|p| p.area == a.name).count(),
            })
        })
        .collect();
    let input_hash = sha256_hex(toml::to_string(s).unwrap_or_default().as_bytes());
    Ok(json!({
        "schema": SCHEMA,
        "label": LABEL,
        "evidence": "MODELLED",
        "scenario": s.name,
        "input_sha256": input_hash,
        "start_utc": s.start_utc,
        "duration_h": s.duration_h,
        "step_s": s.step_s,
        "orbit": {
            "tle": s.tle,
            "propagator": "open engine SGP4 (WGS-72), TEME to ITRS",
            "period_s": round(period, 1),
            "altitude_km": q3(&sat_alt_km),
        },
        "receiver": {
            "name": rx.name,
            "boresight": rx.boresight,
            "pattern": if rx.pattern.is_empty() { json!({"assumed": "generic zenith patch", "points": DEFAULT_PATTERN}) } else { json!({"supplied": rx.pattern}) },
            "gnss_gain_dbi": rx.gnss_gain_dbi,
            "signal_power_dbw": rx.signal_power_dbw,
            "temp_k": rx.temp_k,
            "nominal_cn0_dbhz": round(cn0_nom, 2),
            "tracking_threshold_dbhz": rx.tracking_threshold_dbhz,
            "q": rx.q,
            "tested_js_db": rx.tested_js_db,
            "tested_js_source": rx.tested_js_source,
        },
        "assumptions": [
            "Areas are regions, not emitter positions; the transmitter is put at the point of the area nearest the sub-satellite point (worst case for the receiver).",
            "EIRPs are assumptions, several values; free-space propagation; no atmospheric or polarisation loss.",
            "Doppler offsets are those of a static ground transmitter at the spacecraft, from its range rate; signal-parameter level only.",
            "Effective C/N0 from the open engine's anti-jam equation with the stated Q and the GPS C/A chip rate.",
        ],
        "areas": area_rows,
        "in_view_fraction": round(in_view, 4),
        "passes": passes,
        "revolutions": revs,
        "envelope": {
            "per_eirp": js_env,
            "dwell_s": q3(&dwell),
            "abs_doppler_offset_hz": q3(&doppler_all),
            "abs_doppler_rate_hz_s": q3(&rate_all),
        },
        "test_points": test_points,
    }))
}

/// The passes as CSV.
pub fn passes_csv(doc: &Value) -> String {
    let mut out = String::from(
        "area,start_offset_s,dwell_s,max_elev_deg,min_range_km,eirp_dbw,max_js_db,min_cn0_eff_dbhz,below_threshold_s,above_tested_js_s,doppler_min_hz,doppler_max_hz,max_abs_doppler_rate_hz_s\n",
    );
    for p in doc["passes"].as_array().into_iter().flatten() {
        for e in p["per_eirp"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                p["area"].as_str().unwrap_or("").replace(',', ";"),
                p["start_utc_offset_s"],
                p["dwell_s"],
                p["max_elevation_deg"],
                p["min_range_km"],
                e["eirp_dbw"],
                e["max_js_db"],
                e["min_cn0_eff_dbhz"],
                e["below_threshold_s"],
                if e["above_tested_js_s"].is_null() {
                    String::new()
                } else {
                    e["above_tested_js_s"].to_string()
                },
                p["doppler_min_hz"],
                p["doppler_max_hz"],
                p["max_abs_doppler_rate_hz_s"],
            ));
        }
    }
    out
}

/// A short Markdown summary.
pub fn summary_md(doc: &Value) -> String {
    let mut m = format!(
        "# Ground-interference envelope: {}\n\n{}\n\nOrbit period {} s; {} passes over {} areas; in view of an area {:.1}% of the time.\n\n",
        doc["scenario"].as_str().unwrap_or(""),
        LABEL,
        doc["orbit"]["period_s"],
        doc["passes"].as_array().map_or(0, |v| v.len()),
        doc["areas"].as_array().map_or(0, |v| v.len()),
        doc["in_view_fraction"].as_f64().unwrap_or(0.0) * 100.0
    );
    m.push_str("| EIRP (dBW) | J/S p50 | J/S p95 | J/S max | time below threshold (s) | time above tested J/S (s) |\n|---|---|---|---|---|---|\n");
    for e in doc["envelope"]["per_eirp"].as_array().into_iter().flatten() {
        m.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            e["eirp_dbw"],
            e["js_db"]["p50"],
            e["js_db"]["p95"],
            e["js_db"]["max"],
            e["time_below_threshold_s"],
            if e["time_above_tested_js_s"].is_null() {
                "not assessed (no tested J/S)".to_string()
            } else {
                e["time_above_tested_js_s"].to_string()
            }
        ));
    }
    let env = &doc["envelope"];
    m.push_str(&format!(
        "\nPass dwell p95 {} s; |Doppler offset| p95 {} Hz, max {} Hz; |Doppler rate| p95 {} Hz/s.\n\n",
        env["dwell_s"]["p95"],
        env["abs_doppler_offset_hz"]["p95"],
        env["abs_doppler_offset_hz"]["max"],
        env["abs_doppler_rate_hz_s"]["p95"]
    ));
    m.push_str("Test points for simulator runs (signal parameters only):\n\n");
    for t in doc["test_points"].as_array().into_iter().flatten() {
        m.push_str(&format!(
            "- {}: J/S {} dB (max {}), dwell {} s, Doppler offset {} Hz, rate {} Hz/s{}\n",
            t["id"].as_str().unwrap_or(""),
            t["js_db"],
            t["js_max_db"],
            t["dwell_s"],
            t["doppler_offset_hz"],
            t["doppler_rate_hz_s"],
            match t["beyond_tested"].as_bool() {
                Some(true) => " — beyond the J/S the receiver was tested to",
                Some(false) => " — within the J/S the receiver was tested to",
                None => "",
            }
        ));
    }
    m
}

/// The SYNTHETIC sample exposure scenario.
pub const SAMPLE: &str = r#"format = "kshana-space-rx-exposure/1"
name = "SYNTHETIC sample: 510 km sun-synchronous orbit"
start_utc = "2026-11-01T00:00:00Z"
duration_h = 24
step_s = 10
tle = [
  "1 99999U 26001A   26305.00000000  .00000000  00000-0  10000-3 0  9994",
  "2 99999  97.6000  30.0000 0001000  90.0000 270.0000 15.20000000    10",
]
eirp_dbw = [20.0, 40.0, 60.0]

[receiver]
name = "example zenith-patch receiver"
tested_js_db = 30
tested_js_source = "sample campaign interference runs (SYNTHETIC)"

[[area]]
name = "eastern-mediterranean"
lat_deg = 34.5
lon_deg = 33.5
radius_km = 250
source = "illustrative region of public interference reports; not an emitter position"

[[area]]
name = "baltic"
lat_deg = 59.5
lon_deg = 24.0
radius_km = 200
source = "illustrative region of public interference reports; not an emitter position"
"#;
