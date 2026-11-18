// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Ionosphere from data for the `ionospheric` skywave model (MODELLED): a time series of
//! measured virtual reflection height and reflection-coefficient scaling (e.g. from an
//! ionosonde or an LF phase/amplitude monitor) and a space-weather index series (Kp,
//! F10.7) that modify the skywave height, attenuation and fading by a documented
//! heuristic.
//!
//! Enabled by an optional `data` table in `[propagation.sky]` (model `ionospheric`):
//!
//! ```toml
//! [propagation.sky.data]
//! ionosonde_file = "sonde.csv"        # time_utc,h_km,refl_scale   (either column optional)
//! space_weather_file = "kp_f107.csv"  # time_utc,kp,f107
//! # or inline:
//! # ionosonde = [{ time_utc = "2026-11-18T00:00:00Z", h_km = 88.0, refl_scale = 0.9 }]
//! # space_weather = [{ time_utc = "2026-11-18T00:00:00Z", kp = 6.0, f107 = 150.0 }]
//! [propagation.sky.data.heuristic]   # coefficients below (all optional)
//! ```
//!
//! Times are UTC (`YYYY-MM-DDTHH:MM:SSZ`), compared on the scene's TAI timeline (scene start
//! plus scene time). Ionosonde heights, reflection scales and F10.7 are interpolated
//! linearly in time; Kp (a 3-hour index) is held from its timestamp to the next. Before the
//! first and after the last record the end value is held. An empty field means "no value".
//!
//! # What the data change
//!
//! * A measured `h_km` **replaces** the model's (day/night interpolated) virtual height —
//!   the group height; the carrier reflects `phase_height_offset_km` lower as before.
//! * A measured `refl_scale` multiplies the reflection coefficient ‖R‖ (per hop) of the
//!   computed P.684-style field.
//!
//! # Space-weather heuristic (MODELLED; coefficients are engineering guesses, not fitted)
//!
//! With `s = (F10.7 − f107_ref)/100`, `e = max(Kp − kp_threshold, 0)`, `w` the day weight
//! of the path midpoint and `L` a smoothstep from 0 at geomagnetic latitude
//! `geomag_onset_deg` (50°) to 1 at `geomag_full_deg` (65°) (centred dipole, pole at
//! 80.65° N, 72.68° W, IGRF-13 epoch 2020):
//!
//! ```text
//! Δh      = w·dh_day_km_per_100sfu·s + dh_km_per_kp·e·L                 (km)
//! Δ‖R‖_dB = w·day_refl_db_per_100sfu·s + refl_db_per_kp·e·L             (per hop)
//! fading depth m = 1 + fading_depth_per_kp·e·L    (gain a → a^m, phase p → m·p)
//! ```
//!
//! Rationale (qualitative, standard ionospheric physics): higher solar EUV/X-ray flux
//! (F10.7) increases daytime D-region ionisation, lowering the daytime reflection height
//! and increasing absorption; geomagnetic storms (Kp) cause auroral/high-latitude
//! D-region absorption, a lower reflection height and deeper, faster fading (e.g. K.
//! Davies, *Ionospheric Radio*, ch. 6 and 9; ITU-R P.684 notes on temporal variability).
//! Defaults: `f107_ref` 70 sfu, `dh_day_km_per_100sfu` −3 km, `day_refl_db_per_100sfu`
//! −2 dB, `kp_threshold` 4, `dh_km_per_kp` −1 km, `refl_db_per_kp` −1.5 dB,
//! `fading_depth_per_kp` 0.15. Not validated against measured LF skywave statistics.
//! Stated overrides (`delay_us`, `ratio_db`) keep their stated values.

use crate::geo::Geo;
use crate::time::{AbsTime, PS_PER_S};
use serde::{Deserialize, Serialize};

/// One ionosonde/monitor record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IonosondeRecord {
    pub time_utc: String,
    #[serde(default)]
    pub h_km: Option<f64>,
    #[serde(default)]
    pub refl_scale: Option<f64>,
}

/// One space-weather record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpaceWeatherRecord {
    pub time_utc: String,
    #[serde(default)]
    pub kp: Option<f64>,
    #[serde(default)]
    pub f107: Option<f64>,
}

/// Heuristic coefficients (see the module documentation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Heuristic {
    pub f107_ref: f64,
    pub dh_day_km_per_100sfu: f64,
    pub day_refl_db_per_100sfu: f64,
    pub kp_threshold: f64,
    pub dh_km_per_kp: f64,
    pub refl_db_per_kp: f64,
    pub fading_depth_per_kp: f64,
    pub geomag_onset_deg: f64,
    pub geomag_full_deg: f64,
}

impl Default for Heuristic {
    fn default() -> Self {
        Heuristic {
            f107_ref: 70.0,
            dh_day_km_per_100sfu: -3.0,
            day_refl_db_per_100sfu: -2.0,
            kp_threshold: 4.0,
            dh_km_per_kp: -1.0,
            refl_db_per_kp: -1.5,
            fading_depth_per_kp: 0.15,
            geomag_onset_deg: 50.0,
            geomag_full_deg: 65.0,
        }
    }
}

/// `[propagation.sky.data]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct IonoDataConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ionosonde_file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ionosonde: Vec<IonosondeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_weather_file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub space_weather: Vec<SpaceWeatherRecord>,
    #[serde(default)]
    pub heuristic: Heuristic,
}

/// Modifiers of the skywave at one instant and path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IonoModifiers {
    /// Measured virtual height (m), replacing the model's.
    pub h_override_m: Option<f64>,
    /// Height change (m), added after any override.
    pub dh_m: f64,
    /// Factor on ‖R‖ per hop.
    pub refl_scale: f64,
    /// Fading depth exponent (1 = unchanged).
    pub fading_depth: f64,
}

impl Default for IonoModifiers {
    fn default() -> Self {
        IonoModifiers {
            h_override_m: None,
            dh_m: 0.0,
            refl_scale: 1.0,
            fading_depth: 1.0,
        }
    }
}

impl IonoModifiers {
    /// The virtual height (m) given the model's `h_m`.
    pub fn height(&self, h_m: f64) -> f64 {
        (self.h_override_m.unwrap_or(h_m) + self.dh_m).clamp(40_000.0, 400_000.0)
    }
}

/// A time series of `(TAI seconds since 1958, value)`, sorted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Series(pub Vec<(f64, f64)>);

impl Series {
    /// Linear interpolation, ends held; `None` if empty.
    pub fn linear(&self, t: f64) -> Option<f64> {
        let v = &self.0;
        let first = v.first()?;
        if t <= first.0 {
            return Some(first.1);
        }
        let last = v[v.len() - 1];
        if t >= last.0 {
            return Some(last.1);
        }
        let k = v.partition_point(|p| p.0 <= t);
        let (a, b) = (v[k - 1], v[k]);
        Some(a.1 + (b.1 - a.1) * (t - a.0) / (b.0 - a.0))
    }
    /// Value held from each timestamp to the next; the first value before it.
    pub fn hold(&self, t: f64) -> Option<f64> {
        let v = &self.0;
        let first = v.first()?;
        let k = v.partition_point(|p| p.0 <= t);
        Some(if k == 0 { first.1 } else { v[k - 1].1 })
    }
}

/// Seconds of TAI since 1958 of an absolute instant.
pub fn abs_seconds(t: AbsTime) -> f64 {
    t.0 as f64 / PS_PER_S as f64
}

/// Built ionospheric data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IonoData {
    pub h_km: Series,
    pub refl_scale: Series,
    pub kp: Series,
    pub f107: Series,
    pub heuristic: Heuristic,
}

fn parse_time(s: &str) -> Result<f64, String> {
    Ok(abs_seconds(AbsTime::from_utc_iso(s)?))
}

/// Parse a CSV with a header naming `time_utc` and value columns; returns per requested
/// column the series of present values.
fn parse_csv(text: &str, cols: &[&str]) -> Result<Vec<Series>, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines
        .next()
        .ok_or("empty CSV")?
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let it = header
        .iter()
        .position(|h| h == "time_utc" || h == "time")
        .ok_or("CSV: no time_utc column")?;
    let idx: Vec<Option<usize>> = cols
        .iter()
        .map(|c| header.iter().position(|h| h == c))
        .collect();
    let mut out = vec![Series::default(); cols.len()];
    for (n, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let t = parse_time(f.get(it).copied().unwrap_or(""))
            .map_err(|e| format!("CSV row {}: {e}", n + 1))?;
        for (k, i) in idx.iter().enumerate() {
            if let Some(s) = i.and_then(|i| f.get(i)).filter(|s| !s.is_empty()) {
                let v: f64 = s
                    .parse()
                    .map_err(|e| format!("CSV row {}: {}: {e}", n + 1, cols[k]))?;
                out[k].0.push((t, v));
            }
        }
    }
    Ok(out)
}

fn sort(s: &mut Series) {
    s.0.sort_by(|a, b| a.0.total_cmp(&b.0));
}

impl IonoData {
    /// Build from the configuration (reads the files).
    pub fn build(cfg: &IonoDataConfig) -> Result<IonoData, String> {
        let mut d = IonoData {
            heuristic: cfg.heuristic.clone(),
            ..Default::default()
        };
        if let Some(f) = &cfg.ionosonde_file {
            let t = std::fs::read_to_string(f).map_err(|e| format!("ionosonde {f}: {e}"))?;
            let s = parse_csv(&t, &["h_km", "refl_scale"])
                .map_err(|e| format!("ionosonde {f}: {e}"))?;
            d.h_km.0.extend(s[0].0.iter().copied());
            d.refl_scale.0.extend(s[1].0.iter().copied());
        }
        for r in &cfg.ionosonde {
            let t = parse_time(&r.time_utc)?;
            if let Some(h) = r.h_km {
                d.h_km.0.push((t, h));
            }
            if let Some(s) = r.refl_scale {
                d.refl_scale.0.push((t, s));
            }
        }
        if let Some(f) = &cfg.space_weather_file {
            let t = std::fs::read_to_string(f).map_err(|e| format!("space weather {f}: {e}"))?;
            let s =
                parse_csv(&t, &["kp", "f107"]).map_err(|e| format!("space weather {f}: {e}"))?;
            d.kp.0.extend(s[0].0.iter().copied());
            d.f107.0.extend(s[1].0.iter().copied());
        }
        for r in &cfg.space_weather {
            let t = parse_time(&r.time_utc)?;
            if let Some(k) = r.kp {
                d.kp.0.push((t, k));
            }
            if let Some(f) = r.f107 {
                d.f107.0.push((t, f));
            }
        }
        for s in [&mut d.h_km, &mut d.refl_scale, &mut d.kp, &mut d.f107] {
            sort(s);
        }
        if d.h_km.0.iter().any(|p| !(40.0..=400.0).contains(&p.1)) {
            return Err("ionosonde: h_km outside 40–400 km".into());
        }
        if d.refl_scale.0.iter().any(|p| !(p.1 >= 0.0 && p.1 <= 10.0)) {
            return Err("ionosonde: refl_scale outside 0–10".into());
        }
        if d.kp.0.iter().any(|p| !(0.0..=9.0).contains(&p.1)) {
            return Err("space weather: Kp outside 0–9".into());
        }
        if d.f107.0.iter().any(|p| !(p.1 > 0.0 && p.1 < 1000.0)) {
            return Err("space weather: F10.7 outside 0–1000 sfu".into());
        }
        let h = &d.heuristic;
        if !(h.geomag_full_deg > h.geomag_onset_deg) {
            return Err("iono data: geomag_full_deg must exceed geomag_onset_deg".into());
        }
        Ok(d)
    }

    /// Modifiers at absolute time `t_abs_s` (TAI s since 1958) for a path whose midpoint
    /// is `mid`, with day weight `w_day`.
    pub fn modifiers(&self, t_abs_s: f64, w_day: f64, mid: Geo) -> IonoModifiers {
        let h = &self.heuristic;
        let s = self
            .f107
            .linear(t_abs_s)
            .map(|f| (f - h.f107_ref) / 100.0)
            .unwrap_or(0.0);
        let e = self
            .kp
            .hold(t_abs_s)
            .map(|k| (k - h.kp_threshold).max(0.0))
            .unwrap_or(0.0);
        let gm = geomagnetic_lat_deg(mid).abs();
        let u =
            ((gm - h.geomag_onset_deg) / (h.geomag_full_deg - h.geomag_onset_deg)).clamp(0.0, 1.0);
        let l = u * u * (3.0 - 2.0 * u);
        let dh_km = w_day * h.dh_day_km_per_100sfu * s + h.dh_km_per_kp * e * l;
        let db = w_day * h.day_refl_db_per_100sfu * s + h.refl_db_per_kp * e * l;
        let measured = self.refl_scale.linear(t_abs_s).unwrap_or(1.0);
        IonoModifiers {
            h_override_m: self.h_km.linear(t_abs_s).map(|k| k * 1000.0),
            dh_m: dh_km * 1000.0,
            refl_scale: measured * 10f64.powf(db / 20.0),
            fading_depth: (1.0 + h.fading_depth_per_kp * e * l).max(0.0),
        }
    }
}

/// Geomagnetic latitude (deg) of `p` for a centred dipole with its north pole at 80.65° N,
/// 72.68° W (IGRF-13, 2020).
pub fn geomagnetic_lat_deg(p: Geo) -> f64 {
    let (pl, po) = (80.65f64.to_radians(), (-72.68f64).to_radians());
    let (la, lo) = (p.lat_deg.to_radians(), p.lon_deg.to_radians());
    (la.sin() * pl.sin() + la.cos() * pl.cos() * (lo - po).cos())
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees()
}

#[cfg(test)]
mod tests {
    use super::super::sky::{Daylight, IonoConfig, Ionospheric};
    use super::super::sky_fading::FadingConfig;
    use super::super::PropagationModel;
    use super::*;
    use crate::traj::RxState;

    fn t(s: &str) -> f64 {
        parse_time(s).unwrap()
    }

    #[test]
    fn series_interpolation_and_csv() {
        let s = Series(vec![(0.0, 1.0), (10.0, 3.0)]);
        assert_eq!(s.linear(-5.0), Some(1.0));
        assert_eq!(s.linear(5.0), Some(2.0));
        assert_eq!(s.linear(50.0), Some(3.0));
        assert_eq!(s.hold(9.9), Some(1.0));
        assert_eq!(s.hold(10.0), Some(3.0));
        assert_eq!(Series::default().linear(0.0), None);
        let csv =
            "time_utc,h_km,refl_scale\n2026-11-18T00:00:00Z,85,\n2026-11-18T01:00:00Z,95,0.5\n";
        let v = parse_csv(csv, &["h_km", "refl_scale"]).unwrap();
        assert_eq!(v[0].0.len(), 2);
        assert_eq!(v[1].0.len(), 1);
        assert!(parse_csv("h_km\n1\n", &["h_km"]).is_err());
        assert!((geomagnetic_lat_deg(Geo::new(80.65, -72.68)) - 90.0).abs() < 1e-6);
        assert!(geomagnetic_lat_deg(Geo::new(0.0, 0.0)).abs() < 12.0);
    }

    #[test]
    fn heuristic_modifiers() {
        let d = IonoData::build(&IonoDataConfig {
            space_weather: vec![
                SpaceWeatherRecord {
                    time_utc: "2026-11-18T00:00:00Z".into(),
                    kp: Some(7.0),
                    f107: Some(170.0),
                },
                SpaceWeatherRecord {
                    time_utc: "2026-11-18T03:00:00Z".into(),
                    kp: Some(2.0),
                    f107: Some(70.0),
                },
            ],
            ..Default::default()
        })
        .unwrap();
        let polar = Geo::new(70.0, 20.0); // geomagnetic ≈ 67°
        let equat = Geo::new(0.0, 20.0);
        let m = d.modifiers(t("2026-11-18T01:00:00Z"), 1.0, polar);
        // Kp held at 7 (e = 3, L = 1), F10.7 interpolated to 136.7 (s = 0.667)
        let s = (170.0 - (100.0 / 3.0) - 70.0) / 100.0;
        assert!(
            (m.dh_m - 1000.0 * (-3.0 * s - 3.0)).abs() < 1e-6,
            "{}",
            m.dh_m
        );
        let db = -2.0 * s - 4.5;
        assert!((20.0 * m.refl_scale.log10() - db).abs() < 1e-9);
        assert!((m.fading_depth - 1.45).abs() < 1e-12);
        // low latitude: no Kp effect; night: no F10.7 effect
        let q = d.modifiers(t("2026-11-18T01:00:00Z"), 0.0, equat);
        assert_eq!((q.dh_m, q.refl_scale, q.fading_depth), (0.0, 1.0, 1.0));
        // quiet later
        let r = d.modifiers(t("2026-11-18T04:00:00Z"), 1.0, polar);
        assert!(r.dh_m.abs() < 1e-9 && (r.refl_scale - 1.0).abs() < 1e-12);
        assert!(IonoData::build(&IonoDataConfig {
            space_weather: vec![SpaceWeatherRecord {
                time_utc: "2026-11-18T00:00:00Z".into(),
                kp: Some(12.0),
                f107: None
            }],
            ..Default::default()
        })
        .is_err());
    }

    fn station() -> crate::chain::Station {
        crate::chain::Station {
            id: "M".into(),
            name: "M".into(),
            role: crate::signal::Role::Master,
            position: Geo::new(62.0, 5.0),
            power_kw: 400.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        }
    }

    #[test]
    fn data_drive_the_ionospheric_model() {
        let start = AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap();
        let rx = RxState {
            t_s: 0.0,
            pos: Geo::new(68.0, 15.0),
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        let base = IonoConfig {
            daylight: Daylight::Night,
            ..Default::default()
        };
        let plain = Ionospheric::with_start(base.clone(), Some(start))
            .unwrap()
            .paths(&station(), &rx)[0];
        // no data: identical to before
        let empty = IonoConfig {
            data: Some(IonoDataConfig::default()),
            ..base.clone()
        };
        let e = Ionospheric::with_start(empty, Some(start))
            .unwrap()
            .paths(&station(), &rx)[0];
        assert_eq!(e, plain);
        // measured height 100 km replaces the night 90 km → later skywave; half ‖R‖ → −6 dB
        let dir = std::env::temp_dir().join(format!("kshana-iono-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("sonde.csv");
        std::fs::write(
            &f,
            "time_utc,h_km,refl_scale\n2026-11-17T00:00:00Z,100,0.5\n",
        )
        .unwrap();
        let c = IonoConfig {
            data: Some(IonoDataConfig {
                ionosonde_file: Some(f.to_string_lossy().into()),
                ..Default::default()
            }),
            ..base.clone()
        };
        let p = Ionospheric::with_start(c, Some(start))
            .unwrap()
            .paths(&station(), &rx)[0];
        assert!(p.delay > plain.delay);
        let h100 = Ionospheric::with_start(
            IonoConfig {
                h_night_km: 100.0,
                ..base.clone()
            },
            Some(start),
        )
        .unwrap()
        .paths(&station(), &rx)[0];
        assert_eq!(p.delay, h100.delay);
        assert!((20.0 * (p.field_uv_m / h100.field_uv_m).log10() + 6.0206).abs() < 1e-3);
        // a storm deepens the fading (high-latitude path) and lowers the field
        let fad = Some(FadingConfig {
            seed: 3,
            ..Default::default()
        });
        let quiet = IonoConfig {
            fading: fad.clone(),
            ..base.clone()
        };
        let storm = IonoConfig {
            fading: fad,
            data: Some(IonoDataConfig {
                space_weather: vec![SpaceWeatherRecord {
                    time_utc: "2026-11-18T00:00:00Z".into(),
                    kp: Some(8.0),
                    f107: None,
                }],
                ..Default::default()
            }),
            ..base.clone()
        };
        let mq = Ionospheric::with_start(quiet, Some(start)).unwrap();
        let ms = Ionospheric::with_start(storm, Some(start)).unwrap();
        let spread = |m: &Ionospheric| {
            let v: Vec<f64> = (0..200)
                .map(|k| {
                    m.paths(
                        &station(),
                        &RxState {
                            t_s: k as f64 * 5.0,
                            ..rx
                        },
                    )[0]
                    .field_uv_m
                    .ln()
                })
                .collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
        };
        assert!(spread(&ms) > 1.3 * spread(&mq));
        let sp = ms.paths(&station(), &rx)[0];
        let qp = mq.paths(&station(), &rx)[0];
        assert!(sp.delay < qp.delay, "storm lowers the reflection height");
        // the TOML form parses
        let s: super::super::sky::SkyConfig = toml::from_str(
            "model = \"ionospheric\"\ndaylight = \"night\"\n[data]\nspace_weather = [{ time_utc = \"2026-11-18T00:00:00Z\", kp = 5.0 }]\n[data.heuristic]\nrefl_db_per_kp = -3.0\n",
        )
        .unwrap();
        let ctx = super::super::BuildContext::new(start, 1.0, 0);
        assert!(super::super::sky::build(&s, &ctx).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
