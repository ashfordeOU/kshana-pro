// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Space weather for field work: Kp sources, the NOAA G scale, a scintillation advisor, a
//! PPP-versus-RTK advisor per field and hour, and a plain-language translation for growers.
//!
//! **Kp sources.** The GFZ file `Kp_ap_Ap_SN_F107_since_1932.txt` (Matzka et al. 2021,
//! doi:10.1029/2020SW002641; CC BY 4.0), NOAA SWPC `noaa-planetary-k-index.json` (objects or
//! the older array-of-arrays form) and the NOAA SWPC 3-day forecast text (its "NOAA Kp index
//! breakdown" table). Each value covers three hours from its time tag.
//!
//! **G scale.** The NOAA Space Weather Scale for geomagnetic storms: G1 at Kp 5, G2 at 6, G3 at
//! 7, G4 at 8 (including 9-), G5 at 9. Thirds are rounded to the nearest whole Kp, except that
//! 9- is G4 as the scale states.
//!
//! **Scintillation advisor (MODELLED rules).** The field's geomagnetic latitude comes from the
//! open engine's IGRF centred dipole (`kshana::igrf::geomagnetic_north_pole`) and its modified
//! dip latitude from the IGRF inclination (`kshana::igrf::magnetic_field`, Rawer's
//! `tan(modip) = I / sqrt(cos lat)`). Three regions:
//!
//! * *low latitude* (|modip| up to [`AdvisorParams::low_lat_modip_deg`]): post-sunset
//!   equatorial scintillation, high from 19:00 to 02:00 local solar time in the longitude
//!   sector's season when solar flux F10.7 is at or above [`AdvisorParams::f107_high`],
//!   moderate in that window otherwise (after the published climatology, e.g. Aarons 1993,
//!   Basu et al. 2002; simplified);
//! * *auroral and polar*: high when the field is poleward of an equatorward auroral boundary
//!   that moves with Kp, `boundary = oval_base_deg - oval_slope_deg x Kp` geomagnetic
//!   latitude (a linear simplification of the Feldstein-Starkov oval), moderate within
//!   [`AdvisorParams::oval_margin_deg`] of it;
//! * *storm-time mid latitude*: at Kp 7 or more and geomagnetic latitude 40 deg or more,
//!   ionospheric gradients that upset RTK are flagged high even outside the oval.
//!
//! The engine's Cornell scintillation model (`kshana::iq::channel::scint`) turns each level's
//! representative S4 into the share of time a signal fades deeper than the receiver's
//! tracking margin, to make "moderate" and "high" concrete.
//!
//! **PPP versus RTK (MODELLED).** RTK horizontal error (1 sigma) `a + b x baseline x m(Kp)`
//! (defaults 10 mm + 1 ppm, storm multipliers per G level), optionally with the differential
//! vertical TEC across the baseline from an IONEX map (engine `kshana::ionex`) reported as an
//! L1 delay difference; PPP error after convergence and a convergence time lengthened by
//! scintillation. Per hour the advisor says which method can meet the accuracy asked (95 %,
//! taken as 2 sigma) and a go / caution / no-go status for RTK-grade work.
//!
//! **What a hosted service would add.** Live regional TEC and rate-of-TEC maps from reference
//! station networks in place of the Kp proxy, thresholds calibrated against observed receiver
//! outcomes per region, and pushed alerts. None of that is here: these rules run locally on
//! public indices.

use serde::Serialize;

use super::{civil_from_days, days_from_civil, round, utc_text};

/// A three-hourly Kp series (unix seconds of the interval start, Kp).
#[derive(Clone, Debug, Default, Serialize)]
pub struct KpSeries {
    pub source: String,
    pub values: Vec<(i64, f64)>,
    /// Daily observed F10.7 (unix day start, sfu) when the source carries it.
    pub f107: Vec<(i64, f64)>,
}

impl KpSeries {
    /// Parse any of the supported sources.
    pub fn parse(text: &str) -> Result<KpSeries, String> {
        let t = text.trim_start();
        let mut s = if t.starts_with('[') {
            parse_swpc_json(t)?
        } else if t.contains("NOAA Kp index breakdown") {
            parse_swpc_forecast(t)?
        } else {
            parse_gfz(t)?
        };
        s.values.sort_by_key(|v| v.0);
        s.values.dedup_by_key(|v| v.0);
        if s.values.is_empty() {
            return Err("no Kp values found".into());
        }
        Ok(s)
    }

    /// Kp in force at `t` (the value whose three-hour interval contains it).
    pub fn at(&self, t: i64) -> Option<f64> {
        let i = self.values.partition_point(|v| v.0 <= t);
        (i > 0)
            .then(|| self.values[i - 1])
            .filter(|v| t - v.0 < 3 * 3600)
            .map(|v| v.1)
    }

    /// Largest Kp over the intervals touching `[t0, t1]` (seconds, may be fractional).
    pub fn max_between(&self, t0: f64, t1: f64) -> Option<f64> {
        self.values
            .iter()
            .filter(|v| (v.0 as f64) <= t1 && (v.0 + 3 * 3600) as f64 > t0)
            .map(|v| v.1)
            .reduce(f64::max)
    }

    /// F10.7 on the day of `t`, or the nearest earlier day within 3 days.
    pub fn f107_at(&self, t: i64) -> Option<f64> {
        let i = self.f107.partition_point(|v| v.0 <= t);
        (i > 0)
            .then(|| self.f107[i - 1])
            .filter(|v| t - v.0 < 4 * 86_400)
            .map(|v| v.1)
    }
}

fn parse_gfz(text: &str) -> Result<KpSeries, String> {
    let mut s = KpSeries {
        source: "GFZ Kp (Matzka et al. 2021, CC BY 4.0)".into(),
        ..Default::default()
    };
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 15 {
            return Err(format!("GFZ Kp: short line '{l}'"));
        }
        let num = |i: usize| -> Result<f64, String> {
            f[i].parse::<f64>()
                .map_err(|_| format!("GFZ Kp: bad field '{}'", f[i]))
        };
        let day = days_from_civil(num(0)? as i64, num(1)? as i64, num(2)? as i64) * 86_400;
        for k in 0..8 {
            let kp = num(7 + k)?;
            if kp >= 0.0 {
                s.values.push((day + k as i64 * 3 * 3600, kp));
            }
        }
        if f.len() >= 27 {
            if let Ok(fx) = f[25].parse::<f64>() {
                if fx > 0.0 {
                    s.f107.push((day, fx));
                }
            }
        }
    }
    Ok(s)
}

fn parse_iso(t: &str) -> Result<i64, String> {
    super::parse_utc(&t.replace(' ', "T"))
}

fn parse_swpc_json(text: &str) -> Result<KpSeries, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("SWPC Kp JSON: {e}"))?;
    let rows = v.as_array().ok_or("SWPC Kp JSON: not an array")?;
    let mut s = KpSeries {
        source: "NOAA SWPC planetary K index".into(),
        ..Default::default()
    };
    for (i, r) in rows.iter().enumerate() {
        let (t, kp) = if let Some(o) = r.as_object() {
            (
                o.get("time_tag").and_then(|x| x.as_str()).map(String::from),
                o.get("Kp")
                    .or(o.get("kp_index"))
                    .or(o.get("kp"))
                    .and_then(|x| {
                        x.as_f64()
                            .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
                    }),
            )
        } else if let Some(a) = r.as_array() {
            if i == 0 && a.first().and_then(|x| x.as_str()) == Some("time_tag") {
                continue;
            }
            (
                a.first().and_then(|x| x.as_str()).map(String::from),
                a.get(1).and_then(|x| {
                    x.as_f64()
                        .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
                }),
            )
        } else {
            (None, None)
        };
        match (t, kp) {
            (Some(t), Some(kp)) => s.values.push((parse_iso(&t)?, kp)),
            _ => return Err(format!("SWPC Kp JSON: row {i} has no time_tag/Kp")),
        }
    }
    Ok(s)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn parse_swpc_forecast(text: &str) -> Result<KpSeries, String> {
    let mut s = KpSeries {
        source: "NOAA SWPC 3-day forecast (Kp breakdown)".into(),
        ..Default::default()
    };
    let lines: Vec<&str> = text.lines().collect();
    let hi = lines
        .iter()
        .position(|l| l.contains("NOAA Kp index breakdown"))
        .ok_or("forecast: no Kp breakdown")?;
    // The year of the first column: the issue year when the product states it (a forecast
    // issued on 31 December spans into the next year), else the last four-digit number on
    // the breakdown title line.
    let issued = lines.iter().find_map(|l| {
        l.strip_prefix(":Issued:")?
            .split_whitespace()
            .next()?
            .parse::<i64>()
            .ok()
    });
    let year: i64 = match issued {
        Some(y) => y,
        None => lines[hi]
            .split_whitespace()
            .rev()
            .find_map(|w| w.parse::<i64>().ok().filter(|y| *y > 1900))
            .ok_or("forecast: no year on the breakdown line")?,
    };
    let head = lines[hi + 1..]
        .iter()
        .find(|l| !l.trim().is_empty())
        .ok_or("forecast: no day header")?;
    let toks: Vec<&str> = head.split_whitespace().collect();
    let mut days = Vec::new();
    for pair in toks.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        let m = MONTHS
            .iter()
            .position(|x| *x == pair[0])
            .ok_or_else(|| format!("forecast: month '{}'", pair[0]))? as i64
            + 1;
        let d: i64 = pair[1]
            .parse()
            .map_err(|_| format!("forecast: day '{}'", pair[1]))?;
        days.push(days_from_civil(year, m, d) * 86_400);
    }
    // A forecast spanning New Year: later columns roll into the next year.
    for i in 1..days.len() {
        if days[i] < days[i - 1] {
            let (y, m, d) = civil_from_days(days[i] / 86_400);
            days[i] = days_from_civil(y + 1, m, d) * 86_400;
        }
    }
    for l in &lines[hi + 2..] {
        let l = l.trim();
        if l.len() < 7 || !l.contains("UT") {
            if s.values.len() >= 8 * days.len() {
                break;
            }
            continue;
        }
        let h: i64 = l[..2].parse().map_err(|_| format!("forecast row '{l}'"))?;
        let vals: Vec<f64> = l[7..]
            .split_whitespace()
            .filter(|w| !w.starts_with('('))
            .filter_map(|w| w.parse().ok())
            .collect();
        for (k, v) in vals.iter().enumerate().take(days.len()) {
            s.values.push((days[k] + h * 3600, *v));
        }
    }
    Ok(s)
}

/// NOAA G level (0 below the scale) of a Kp value.
pub fn g_level(kp: f64) -> u8 {
    if kp >= 9.0 - 1e-6 {
        return 5;
    }
    match kp.round() as i64 {
        k if k >= 8 => 4,
        7 => 3,
        6 => 2,
        5 => 1,
        _ => 0,
    }
}

/// Geomagnetic (centred-dipole) latitude (deg) of a site in `year`.
pub fn geomagnetic_lat_deg(lat: f64, lon: f64, year: f64) -> f64 {
    let (pl, po) = kshana::igrf::geomagnetic_north_pole(year);
    let (a, b) = (lat.to_radians(), pl.to_radians());
    (a.sin() * b.sin() + a.cos() * b.cos() * (lon - po).to_radians().cos())
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees()
}

/// Modified dip latitude (deg): `atan(I / sqrt(cos lat))`, `I` the IGRF inclination.
pub fn modip_deg(lat: f64, lon: f64, year: f64) -> f64 {
    let i = kshana::igrf::magnetic_field(lat, lon, 0.0, year)
        .inclination_deg
        .to_radians();
    (i / lat.to_radians().cos().max(1e-6).sqrt())
        .atan()
        .to_degrees()
}

/// Risk level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Moderate,
    High,
}

/// Status for RTK-grade work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Status {
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "caution")]
    Caution,
    #[serde(rename = "no-go")]
    NoGo,
}

/// Advisor parameters (named, MODELLED defaults).
#[derive(Clone, Debug, Serialize)]
pub struct AdvisorParams {
    pub low_lat_modip_deg: f64,
    pub f107_high: f64,
    pub oval_base_deg: f64,
    pub oval_slope_deg: f64,
    pub oval_margin_deg: f64,
    /// Representative S4 for low / moderate / high.
    pub s4: [f64; 3],
    pub tau0_s: f64,
    /// Receiver tracking margin (dB): a fade deeper than this risks loss of lock.
    pub margin_db: f64,
    /// RTK 1-sigma horizontal error: constant (m) and per km of baseline (m/km).
    pub rtk_a_m: f64,
    pub rtk_b_m_per_km: f64,
    /// Multipliers of the baseline term for G0..G5.
    pub storm_mult: [f64; 6],
    pub ppp_sigma_m: f64,
    pub ppp_convergence_min: f64,
    /// Required horizontal accuracy (m, 95 %).
    pub accuracy_m: f64,
}

impl Default for AdvisorParams {
    fn default() -> Self {
        AdvisorParams {
            low_lat_modip_deg: 30.0,
            f107_high: 150.0,
            oval_base_deg: 68.0,
            oval_slope_deg: 2.0,
            oval_margin_deg: 3.0,
            s4: [0.1, 0.4, 0.8],
            tau0_s: 1.0,
            margin_db: 10.0,
            rtk_a_m: 0.01,
            rtk_b_m_per_km: 0.001,
            storm_mult: [1.0, 2.0, 3.0, 6.0, 10.0, 20.0],
            ppp_sigma_m: 0.04,
            ppp_convergence_min: 20.0,
            accuracy_m: 0.05,
        }
    }
}

/// Where the field is and what serves it.
#[derive(Clone, Debug, Serialize)]
pub struct Site {
    pub lat: f64,
    pub lon: f64,
    /// RTK baseline to the base or nearest network station (km).
    pub baseline_km: f64,
    pub base: Option<(f64, f64)>,
    pub rtk_available: bool,
    pub ppp_available: bool,
}

/// One hour of advice.
#[derive(Clone, Debug, Serialize)]
pub struct HourAdvice {
    pub utc: String,
    pub local_solar_hour: f64,
    pub kp: Option<f64>,
    pub g: u8,
    pub scintillation: Risk,
    pub drivers: Vec<String>,
    pub rtk_sigma_m: Option<f64>,
    pub ppp_sigma_m: Option<f64>,
    pub ppp_convergence_min: Option<f64>,
    /// L1 delay difference across the baseline from the IONEX map (m), when given.
    pub iono_l1_diff_m: Option<f64>,
    /// `rtk`, `ppp`, `rtk_or_ppp` or `none`.
    pub method: String,
    pub status: Status,
}

/// The advisory.
#[derive(Clone, Debug, Serialize)]
pub struct Advisory {
    pub label: &'static str,
    pub site: Site,
    pub geomagnetic_lat_deg: f64,
    pub modip_deg: f64,
    pub region: String,
    pub kp_source: String,
    pub params: AdvisorParams,
    /// Share of time faded deeper than the margin, per level (engine Cornell model).
    pub fade_fraction: [f64; 3],
    pub hours: Vec<HourAdvice>,
    pub notes: Vec<String>,
}

/// Share of samples with intensity below `-margin_db`, for S4 and tau0, from the engine's
/// Cornell model (deterministic seed; 20,000 samples over 32 satellites).
pub fn fade_fraction(s4: f64, tau0_s: f64, margin_db: f64) -> f64 {
    use kshana::iq::channel::scint::{ScintParams, Scintillation};
    let mut m = Scintillation::new(ScintParams::new(s4, tau0_s), 0xA6_2024);
    let thr = 10f64.powf(-margin_db / 10.0);
    let (mut n, mut k) = (0usize, 0usize);
    for sat in 1..=32u32 {
        for smp in m.series(sat, tau0_s / 2.0, 625) {
            n += 1;
            if smp.amplitude * smp.amplitude < thr {
                k += 1;
            }
        }
    }
    k as f64 / n as f64
}

fn in_season(lon: f64, month: i64) -> bool {
    // American sector: September to March; elsewhere the equinox months.
    if (-130.0..-30.0).contains(&lon) {
        !(4..=8).contains(&month)
    } else {
        matches!(month, 2..=4 | 9..=11)
    }
}

/// An optional IONEX map for the differential TEC.
pub struct Ionex<'a> {
    pub file: &'a kshana::ionex::IonexFile,
}

impl Ionex<'_> {
    fn unix_of_first_day(&self) -> i64 {
        let e = &self.file.header.first_epoch;
        days_from_civil(e.year as i64, e.month as i64, e.day as i64) * 86_400
    }

    fn vtec(&self, t: i64, lat: f64, lon: f64) -> Option<f64> {
        let ts = (t - self.unix_of_first_day()) as f64;
        self.file
            .vtec_at(
                ts,
                lat,
                lon,
                kshana::ionex::IonexTimeInterp::RotatedConsecutive,
            )
            .filter(|v| v.is_finite())
    }
}

/// Advise for `hours` hours from `t0` (unix seconds, rounded down to the hour).
pub fn advise(
    site: &Site,
    kp: &KpSeries,
    f107_override: Option<f64>,
    t0: i64,
    hours: usize,
    p: &AdvisorParams,
    ionex: Option<&Ionex>,
) -> Advisory {
    let t0 = t0 - t0.rem_euclid(3600);
    let (y, _, _) = civil_from_days(t0.div_euclid(86_400));
    let year = y as f64 + 0.5;
    let mlat = geomagnetic_lat_deg(site.lat, site.lon, year);
    let modip = modip_deg(site.lat, site.lon, year);
    let low = modip.abs() <= p.low_lat_modip_deg;
    let region = if low {
        "low latitude (equatorial scintillation)"
    } else if mlat.abs() >= 55.0 {
        "high latitude (auroral)"
    } else {
        "mid latitude"
    };
    let fades = [
        fade_fraction(p.s4[0], p.tau0_s, p.margin_db),
        fade_fraction(p.s4[1], p.tau0_s, p.margin_db),
        fade_fraction(p.s4[2], p.tau0_s, p.margin_db),
    ];
    let mut notes = vec![
        "Rules and thresholds are MODELLED defaults (named parameters); they are a planning aid, not a forecast of any receiver's behaviour.".to_string(),
    ];
    let mut out = Vec::new();
    let mut missing = 0;
    for h in 0..hours {
        let t = t0 + h as i64 * 3600;
        let kpv = kp.at(t);
        if kpv.is_none() {
            missing += 1;
        }
        let g = kpv.map(g_level).unwrap_or(0);
        let (_, month, _) = civil_from_days(t.div_euclid(86_400));
        let lst = ((t.rem_euclid(86_400)) as f64 / 3600.0 + site.lon / 15.0).rem_euclid(24.0);
        let mut drivers = Vec::new();
        let mut risk = Risk::Low;
        if low {
            let night = !(2.0..19.0).contains(&lst);
            if night {
                let f107 = f107_override.or(kp.f107_at(t));
                let season = in_season(site.lon, month);
                let hi_flux = f107.is_some_and(|f| f >= p.f107_high);
                risk = if season && hi_flux {
                    Risk::High
                } else {
                    Risk::Moderate
                };
                drivers.push(format!(
                    "post-sunset equatorial window ({lst:.1} h local solar time){}{}",
                    if season {
                        ", in season"
                    } else {
                        ", out of season"
                    },
                    match f107 {
                        Some(f) => format!(", F10.7 {f:.0}"),
                        None => ", F10.7 unknown".into(),
                    }
                ));
            }
        }
        if let Some(k) = kpv {
            let boundary = p.oval_base_deg - p.oval_slope_deg * k;
            if mlat.abs() >= boundary {
                risk = risk.max(Risk::High);
                drivers.push(format!(
                    "inside the auroral boundary ({boundary:.0} deg geomagnetic at Kp {k:.2})"
                ));
            } else if mlat.abs() >= boundary - p.oval_margin_deg {
                risk = risk.max(Risk::Moderate);
                drivers.push(format!(
                    "near the auroral boundary ({boundary:.0} deg geomagnetic at Kp {k:.2})"
                ));
            }
            if k >= 6.67 && mlat.abs() >= 40.0 {
                risk = risk.max(Risk::High);
                drivers.push(format!("storm-time ionospheric gradients (G{g})"));
            }
        }
        // RTK and PPP.
        let mut iono = None;
        if let (Some(ix), Some(b)) = (ionex, site.base) {
            if let (Some(a), Some(c)) = (ix.vtec(t, site.lat, site.lon), ix.vtec(t, b.0, b.1)) {
                // 40.3 TEC / f^2, TEC in 1e16 el/m^2, f = 1575.42 MHz.
                iono = Some(round(40.3e16 * (a - c).abs() / 1575.42e6f64.powi(2), 4));
            }
        }
        let rtk = site.rtk_available.then(|| {
            let mult = p.storm_mult[g as usize];
            let s =
                p.rtk_a_m + p.rtk_b_m_per_km * site.baseline_km * mult + iono.unwrap_or(0.0) / 2.0;
            round(s, 4)
        });
        let (ppp, conv) = if site.ppp_available {
            match risk {
                Risk::Low => (Some(p.ppp_sigma_m), Some(p.ppp_convergence_min)),
                Risk::Moderate => (Some(p.ppp_sigma_m * 1.5), Some(p.ppp_convergence_min * 2.0)),
                Risk::High => (None, None),
            }
        } else {
            (None, None)
        };
        let rtk_ok = rtk.is_some_and(|s| 2.0 * s <= p.accuracy_m) && risk != Risk::High;
        let ppp_ok = ppp.is_some_and(|s| 2.0 * s <= p.accuracy_m);
        let method = match (rtk_ok, ppp_ok) {
            (true, true) => "rtk_or_ppp",
            (true, false) => "rtk",
            (false, true) => "ppp",
            (false, false) => "none",
        };
        let status = if method == "none" || risk == Risk::High {
            Status::NoGo
        } else if risk == Risk::Moderate || g >= 1 {
            Status::Caution
        } else {
            Status::Go
        };
        out.push(HourAdvice {
            utc: utc_text(t),
            local_solar_hour: round(lst, 2),
            kp: kpv,
            g,
            scintillation: risk,
            drivers,
            rtk_sigma_m: rtk,
            ppp_sigma_m: ppp,
            ppp_convergence_min: conv,
            iono_l1_diff_m: iono,
            method: method.into(),
            status,
        });
    }
    if missing > 0 {
        notes.push(format!(
            "{missing} hour(s) had no Kp value in the series: geomagnetic rules were not applied there."
        ));
    }
    Advisory {
        label: super::LABEL,
        site: site.clone(),
        geomagnetic_lat_deg: round(mlat, 2),
        modip_deg: round(modip, 2),
        region: region.into(),
        kp_source: kp.source.clone(),
        params: p.clone(),
        fade_fraction: [round(fades[0], 5), round(fades[1], 5), round(fades[2], 5)],
        hours: out,
        notes,
    }
}

/// "from 15:00 to 24:00"-style ranges of whole hours.
fn hour_ranges(h: &[u32]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < h.len() {
        let mut j = i;
        while j + 1 < h.len() && h[j + 1] == h[j] + 1 {
            j += 1;
        }
        out.push(format!("from {:02}:00 to {:02}:00", h[i], h[j] + 1));
        i = j + 1;
    }
    out
}

/// Plain-language text for growers, a paragraph per UTC day.
pub fn translate(a: &Advisory) -> String {
    let mut s = format!(
        "Space weather and your GPS guidance at {:.3}, {:.3} ({}, geomagnetic latitude {:.0} deg).\n\
         Source: {}. This is a planning aid ({}), not a guarantee: check your own receiver's status.\n",
        a.site.lat, a.site.lon, a.region, a.geomagnetic_lat_deg, a.kp_source, a.label
    );
    let mut day = String::new();
    let mut rows: Vec<&HourAdvice> = Vec::new();
    let flush = |day: &str, rows: &[&HourAdvice], s: &mut String| {
        if rows.is_empty() {
            return;
        }
        let gmax = rows.iter().map(|r| r.g).max().unwrap_or(0);
        let nogo = hour_ranges(
            &rows
                .iter()
                .filter(|r| r.status == Status::NoGo)
                .filter_map(|r| r.utc[11..13].parse::<u32>().ok())
                .collect::<Vec<_>>(),
        );
        let caution = rows.iter().filter(|r| r.status == Status::Caution).count();
        let storm = match gmax {
            0 => "no geomagnetic storm in the data".to_string(),
            1 => "a minor geomagnetic storm (G1)".to_string(),
            2 => "a moderate geomagnetic storm (G2)".to_string(),
            3 => "a strong geomagnetic storm (G3)".to_string(),
            4 => "a severe geomagnetic storm (G4)".to_string(),
            _ => "an extreme geomagnetic storm (G5)".to_string(),
        };
        s.push_str(&format!("\n{day}: {storm}. "));
        if nogo.is_empty() && caution == 0 {
            s.push_str("RTK autosteer should work normally.\n");
        } else if nogo.is_empty() {
            s.push_str(&format!(
                "{caution} hour(s) with some risk: watch for RTK dropping to float; keep a margin on planting and spraying lines.\n"
            ));
        } else {
            s.push_str(&format!(
                "Expect RTK-grade guidance to be unreliable {} UTC: lines may be off by more than your tolerance or steering may drop out. Consider doing work that does not need centimetre lines then, and re-check your guidance lines afterwards.\n",
                nogo.join(", ")
            ));
        }
    };
    for r in &a.hours {
        let d = r.utc[..10].to_string();
        if d != day {
            flush(&day, &rows, &mut s);
            rows.clear();
            day = d;
        }
        rows.push(r);
    }
    flush(&day, &rows, &mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g_scale() {
        assert_eq!(g_level(4.333), 0);
        assert_eq!(g_level(4.667), 1);
        assert_eq!(g_level(6.0), 2);
        assert_eq!(g_level(7.0), 3);
        assert_eq!(g_level(7.667), 4);
        assert_eq!(g_level(8.667), 4);
        assert_eq!(g_level(9.0), 5);
    }

    #[test]
    fn geomagnetic_latitudes_are_plausible() {
        // Winnipeg is near 58 deg geomagnetic; central Brazil lies close to the dip equator,
        // so its modified dip latitude is small (inside the low-latitude band).
        let w = geomagnetic_lat_deg(49.9, -97.1, 2024.4);
        assert!((55.0..62.0).contains(&w), "{w}");
        let m = modip_deg(-12.0, -55.0, 2024.4);
        assert!(m.abs() < 25.0, "{m}");
        let t = modip_deg(60.0, 25.0, 2024.4);
        assert!(t > 60.0, "{t}");
    }

    #[test]
    fn fades_grow_with_s4() {
        let a = fade_fraction(0.1, 1.0, 10.0);
        let b = fade_fraction(0.4, 1.0, 10.0);
        let c = fade_fraction(0.8, 1.0, 10.0);
        assert!(a < b && b < c, "{a} {b} {c}");
        assert!(a < 1e-3);
    }
}
