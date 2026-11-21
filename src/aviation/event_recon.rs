// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Aviation and drone event reconstruction (research 2026-11-20 gaps, B0602)
//!
//! An investigator holding an aircraft's (or a drone's) flight data, the ADS-B it broadcast and
//! a portable tracker's log needs them on one time base and needs every disagreement between
//! GNSS, barometric and ADS-B altitude either explained or flagged. This module does that, and
//! only that: **alignment and comparison**. Nothing is fused into a navigation solution.
//!
//! * **Alignment**: each source's clock offset against the FDM/QAR record, found by least
//!   squares over a shared quantity (ADS-B barometric altitude against FDM barometric altitude
//!   over the whole record; the tracker's position against the FDM GNSS position in the
//!   opening calibration window).
//! * **Comparison**: per FDM row, GNSS-minus-barometric altitude from each GNSS-derived source
//!   (FDM GNSS altitude, ADS-B geometric altitude, tracker altitude) against what the
//!   atmosphere and the datums explain:
//!   - the flight's own GNSS-baro offset in the calibration window;
//!   - a non-standard temperature term, `ISA deviation / 288.15 × (h − h_cal)` (first order),
//!     when the request states the ISA deviation;
//!   - the geoid separation between ADS-B geometric height (ellipsoid) and MSL altitude, stated
//!     or estimated in the calibration window.
//! * **Explanation** per divergence episode: common to every GNSS-derived source while the
//!   barometric sources agree (consistent with GNSS interference), a single source
//!   (equipment, installation or recording), barometric sources disagreeing (altimeter
//!   setting, static system), or GNSS lost.
//! * An event timeline that also carries the FDM detector's events ([`crate::fdm`]), and a
//!   signed investigator pack.
//!
//! **MODELLED**: exercised on a SYNTHETIC King Air-type flight (twin turboprop climbing to
//! FL250 in ISA+15 with a GNSS altitude divergence in cruise; [`synth`]); no operator or
//! published event data were used. Where no truth exists the reconstruction says what the data
//! show, not what happened. Nothing here locates an emitter.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc, round, sha256_hex};
use crate::transport::{dist_m, Table};

/// Pro feature.
pub const FEATURE: &str = "pro-aviation-event-recon";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-aviation-event-reconstruction/1";
/// Method.
pub const METHOD_ID: &str = "kshana-pro/aviation-event-recon/v1";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED: alignment and comparison rules exercised on a synthetic \
King Air-type flight with known truth; not validated on operator or published event data.";
/// Scope statement.
pub const SCOPE: &str = "Alignment and comparison of recorded sources only: no source is \
fused into a navigation solution, and no output estimates an aircraft position or an \
interference source position.";

const T_ISA_K: f64 = 288.15;

/// Thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Opening window (s) taken as clean: GNSS-baro offset, datum, tracker alignment.
    pub calib_s: f64,
    /// Largest clock offset searched (s).
    pub align_max_s: f64,
    /// Altitude divergence (ft) beyond what is explained.
    pub alt_tol_ft: f64,
    /// Barometric sources agree within this (ft).
    pub baro_tol_ft: f64,
    /// Flagged rows closer than this (s) form one episode.
    pub merge_s: f64,
    /// Shortest episode kept (s).
    pub min_episode_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            calib_s: 300.0,
            align_max_s: 30.0,
            alt_tol_ft: 250.0,
            baro_tol_ft: 100.0,
            merge_s: 30.0,
            min_episode_s: 10.0,
        }
    }
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub aircraft_id: String,
    /// Where the data come from ("SYNTHETIC", or the occurrence reference).
    pub data_source: String,
    /// FDM/QAR CSV in the FDM detector's columns (`time,lat,lon,gnss_alt_ft,baro_alt_ft,...`).
    pub fdm: String,
    /// ADS-B CSV: `timestamp,lat,lon,alt_baro_ft` and optionally `alt_geom_ft,nic,nacp`
    /// (an `aircraft_id` column, if present, is filtered to [`Request::aircraft_id`]).
    #[serde(default)]
    pub adsb: Option<String>,
    /// Tracker CSV: `timestamp,lat,lon,alt_ft` (GNSS altitude, MSL).
    #[serde(default)]
    pub tracker: Option<String>,
    /// ISA temperature deviation (deg C) along the flight, if known.
    #[serde(default)]
    pub isa_dev_c: Option<f64>,
    /// Geoid height above the ellipsoid (ft), if known; else estimated in the calibration window.
    #[serde(default)]
    pub geoid_ft: Option<f64>,
    #[serde(default)]
    pub params: Params,
}

/// A source's clock alignment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Alignment {
    pub source: String,
    /// Seconds added to the source's time stamps to put them on the FDM time base.
    pub offset_s: f64,
    /// RMS residual of the shared quantity after alignment, and before.
    pub rms_after: f64,
    pub rms_before: f64,
    pub on: String,
    pub unit: String,
}

/// What a divergence episode is consistent with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Explanation {
    /// Every GNSS-derived altitude departs together while barometric sources agree.
    CommonGnssDivergence,
    /// One GNSS-derived source departs; the others do not.
    SingleSource,
    /// Barometric sources disagree with each other.
    BarometricDisagreement,
    /// The GNSS altitude was missing.
    GnssLoss,
}

/// One episode.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Episode {
    pub n: usize,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    pub onset_uncertainty_s: f64,
    pub baro_alt_ft_at_onset: Option<f64>,
    /// Source → largest unexplained GNSS-minus-baro residual (ft, signed).
    pub residual_ft: BTreeMap<String, f64>,
    /// Sources that diverged.
    pub diverging: Vec<String>,
    /// Largest ADS-B-baro minus FDM-baro (ft).
    pub baro_disagreement_ft: Option<f64>,
    /// Largest tracker-to-FDM position distance (m) in the episode.
    pub tracker_vs_fdm_m: Option<f64>,
    pub explanation: Explanation,
    pub text: String,
}

/// How much of the GNSS-baro difference the model explains over the flight.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Explained {
    pub calibration_offset_ft: f64,
    pub isa_term_ft_max: f64,
    pub datum_ft: Option<f64>,
    pub datum_estimated: bool,
    /// Largest |FDM GNSS-baro − calibration offset| outside episodes, with and without the ISA
    /// term: what the atmosphere accounted for.
    pub outside_episodes_raw_ft: f64,
    pub outside_episodes_residual_ft: f64,
    /// Largest altitude difference that the clock offsets alone would have caused (ft).
    pub misalignment_effect_ft: f64,
}

/// A timeline row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TimelineEntry {
    pub utc: String,
    pub source: String,
    pub text: String,
    #[serde(skip)]
    t: f64,
}

/// The investigator pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub evidence: &'static str,
    pub scope: &'static str,
    pub title: String,
    pub aircraft_id: String,
    pub data_source: String,
    pub params: Params,
    pub inputs: BTreeMap<String, String>,
    pub alignment: Vec<Alignment>,
    pub explained: Explained,
    pub episodes: Vec<Episode>,
    pub fdm_events: Vec<crate::fdm::FdmEvent>,
    pub timeline: Vec<TimelineEntry>,
    pub limits: Vec<String>,
    pub verdict: String,
}

type Series = Vec<(f64, f64)>;

fn interp(s: &Series, t: f64) -> Option<f64> {
    let i = s.partition_point(|x| x.0 <= t);
    let a = s.get(i.checked_sub(1)?)?;
    if (t - a.0).abs() < 1e-9 {
        return Some(a.1);
    }
    let b = s.get(i)?;
    if b.0 - a.0 > 10.0 {
        return None;
    }
    Some(a.1 + (t - a.0) / (b.0 - a.0) * (b.1 - a.1))
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn rms(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt()
}

/// Least-squares clock offset (whole seconds in ±`max_s`) of a source with `n` samples:
/// `resid(i, off)` is sample `i`'s residual against the reference when `off` is added to the
/// source's time stamps. Returns (offset, RMS after, RMS at zero offset).
fn align<F: Fn(usize, f64) -> Option<f64>>(
    n: usize,
    max_s: f64,
    resid: F,
) -> Option<(f64, f64, f64)> {
    let score = |off: f64| {
        let v: Vec<f64> = (0..n).filter_map(|i| resid(i, off)).collect();
        (v.len() * 2 >= n && !v.is_empty()).then(|| rms(&v))
    };
    let before = score(0.0)?;
    let mut best = (0.0, before);
    let mut o = -max_s;
    while o <= max_s + 1e-9 {
        if let Some(s) = score(o) {
            if s < best.1 - 1e-9 {
                best = (o, s);
            }
        }
        o += 1.0;
    }
    Some((best.0, best.1, before))
}

struct Fdm {
    t: Vec<f64>,
    pos: Vec<Option<(f64, f64)>>,
    gnss: Vec<Option<f64>>,
    baro: Vec<Option<f64>>,
}

fn time_of(s: &str) -> Option<f64> {
    parse_utc(s).or_else(|| s.parse::<f64>().ok())
}

fn read_fdm(text: &str) -> Result<Fdm, String> {
    let tb = Table::parse(text)?;
    let col = |names: &[&str]| {
        names
            .iter()
            .find_map(|n| tb.cols.iter().position(|c| c.eq_ignore_ascii_case(n)))
    };
    let ct = col(&["time", "timestamp", "utc", "t"]).ok_or("FDM: missing column 'time'")?;
    let (cla, clo) = (col(&["lat", "latitude"]), col(&["lon", "longitude"]));
    let cg = col(&["gnss_alt_ft", "gps_alt_ft", "geo_alt_ft"]).ok_or("FDM: missing gnss_alt_ft")?;
    let cb = col(&["baro_alt_ft", "pressure_alt_ft", "alt_baro_ft"])
        .ok_or("FDM: missing baro_alt_ft")?;
    let mut f = Fdm {
        t: vec![],
        pos: vec![],
        gnss: vec![],
        baro: vec![],
    };
    for i in 0..tb.rows.len() {
        let Some(t) = time_of(tb.text(i, Some(ct))) else {
            return Err(format!("FDM row {}: bad time", i + 2));
        };
        f.t.push(t);
        f.pos.push(match (tb.num(i, cla)?, tb.num(i, clo)?) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        });
        f.gnss.push(tb.num(i, Some(cg))?);
        f.baro.push(tb.num(i, Some(cb))?);
    }
    if f.t.windows(2).any(|w| w[1] < w[0]) {
        return Err("FDM rows are not in time order".into());
    }
    if f.t.len() < 30 {
        return Err("FDM: fewer than 30 rows".into());
    }
    Ok(f)
}

struct Other {
    pos: Vec<(f64, (f64, f64))>,
    alt: Series,
    geom: Series,
}

fn read_other(text: &str, id: Option<&str>, alt_col: &str) -> Result<Other, String> {
    let tb = Table::parse(text)?;
    let (ct, cla, clo, ca) = (
        tb.need("timestamp")?,
        tb.need("lat")?,
        tb.need("lon")?,
        tb.need(alt_col)?,
    );
    let (cid, cg) = (tb.col("aircraft_id"), tb.col("alt_geom_ft"));
    let mut o = Other {
        pos: vec![],
        alt: vec![],
        geom: vec![],
    };
    for i in 0..tb.rows.len() {
        if let (Some(want), Some(_)) = (id, cid) {
            if tb.text(i, cid) != want {
                continue;
            }
        }
        let t = time_of(tb.text(i, Some(ct))).ok_or(format!("row {}: bad timestamp", i + 2))?;
        if let (Some(a), Some(b)) = (tb.num(i, Some(cla))?, tb.num(i, Some(clo))?) {
            o.pos.push((t, (a, b)));
        }
        if let Some(a) = tb.num(i, Some(ca))? {
            o.alt.push((t, a));
        }
        if let Some(g) = tb.num(i, cg)? {
            o.geom.push((t, g));
        }
    }
    for s in [&mut o.alt, &mut o.geom] {
        s.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    o.pos.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(o)
}

fn pos_at(v: &[(f64, (f64, f64))], t: f64) -> Option<(f64, f64)> {
    let la: Series = v.iter().map(|x| (x.0, x.1 .0)).collect();
    let lo: Series = v.iter().map(|x| (x.0, x.1 .1)).collect();
    Some((interp(&la, t)?, interp(&lo, t)?))
}

fn shift(s: &Series, off: f64) -> Series {
    s.iter().map(|x| (x.0 + off, x.1)).collect()
}

/// Run the reconstruction. `base` resolves relative paths.
// The FDM columns are parallel vectors indexed together.
#[allow(clippy::needless_range_loop)]
pub fn run(q: &Request, base: &Path) -> Result<(Report, String), String> {
    let p = &q.params;
    if q.data_source.trim().is_empty() {
        return Err("data_source must say where the data come from".into());
    }
    let mut inputs = BTreeMap::new();
    let rd = |f: &str| crate::timing::grid::read_text(base, f);
    let fdm_text = rd(&q.fdm)?;
    inputs.insert(q.fdm.clone(), sha256_hex(fdm_text.as_bytes()));
    let f = read_fdm(&fdm_text)?;
    let adsb = match &q.adsb {
        Some(a) => {
            let t = rd(a)?;
            inputs.insert(a.clone(), sha256_hex(t.as_bytes()));
            Some(
                read_other(&t, Some(&q.aircraft_id), "alt_baro_ft")
                    .map_err(|e| format!("{a}: {e}"))?,
            )
        }
        None => None,
    };
    let trk = match &q.tracker {
        Some(a) => {
            let t = rd(a)?;
            inputs.insert(a.clone(), sha256_hex(t.as_bytes()));
            Some(read_other(&t, None, "alt_ft").map_err(|e| format!("{a}: {e}"))?)
        }
        None => None,
    };
    let fdm_rep = crate::fdm::analyse(&q.aircraft_id, &fdm_text, &crate::fdm::Params::default())?;

    let t0 = f.t[0];
    let cal_end = t0 + p.calib_s;
    let cal: Vec<usize> = (0..f.t.len()).filter(|&i| f.t[i] < cal_end).collect();
    let baro_s: Series =
        f.t.iter()
            .zip(&f.baro)
            .filter_map(|(t, b)| b.map(|b| (*t, b)))
            .collect();
    let fpos: Vec<(f64, (f64, f64))> =
        f.t.iter()
            .zip(&f.pos)
            .filter_map(|(t, p)| p.map(|p| (*t, p)))
            .collect();

    // Alignment.
    let mut alignment = vec![];
    let mut adsb_off = 0.0;
    if let Some(a) = &adsb {
        if let Some((o, after, before)) = align(a.alt.len(), p.align_max_s, |i, off| {
            interp(&baro_s, a.alt[i].0 + off).map(|b| a.alt[i].1 - b)
        }) {
            adsb_off = o;
            alignment.push(Alignment {
                source: "adsb".into(),
                offset_s: o,
                rms_after: round(after, 1),
                rms_before: round(before, 1),
                on: "barometric altitude against FDM barometric altitude, whole record".into(),
                unit: "ft".into(),
            });
        }
    }
    let mut trk_off = 0.0;
    if let Some(k) = &trk {
        let idx: Vec<usize> = (0..k.pos.len())
            .filter(|&i| k.pos[i].0 < cal_end + p.align_max_s)
            .collect();
        if let Some((o, after, before)) = align(idx.len(), p.align_max_s, |i, off| {
            let (t, tp) = k.pos[idx[i]];
            let fp = pos_at(&fpos, t + off)?;
            Some(dist_m(fp.0, fp.1, tp.0, tp.1))
        }) {
            trk_off = o;
            alignment.push(Alignment {
                source: "tracker".into(),
                offset_s: o,
                rms_after: round(after, 1),
                rms_before: round(before, 1),
                on: "position against FDM GNSS position, calibration window".into(),
                unit: "m".into(),
            });
        }
    }
    let ab = adsb.as_ref().map(|a| shift(&a.alt, adsb_off));
    let ag = adsb
        .as_ref()
        .map(|a| shift(&a.geom, adsb_off))
        .filter(|s| !s.is_empty());
    let ta = trk.as_ref().map(|k| shift(&k.alt, trk_off));
    let tp: Option<Vec<(f64, (f64, f64))>> = trk
        .as_ref()
        .map(|k| k.pos.iter().map(|x| (x.0 + trk_off, x.1)).collect());

    // Explained part: calibration offset, ISA term, datum.
    let c0 = median(
        cal.iter()
            .filter_map(|&i| Some(f.gnss[i]? - f.baro[i]?))
            .collect(),
    )
    .ok_or("no row in the calibration window has both GNSS and barometric altitude")?;
    let h_cal = median(cal.iter().filter_map(|&i| f.baro[i]).collect()).unwrap_or(0.0);
    let k_isa = q.isa_dev_c.map(|d| d / T_ISA_K).unwrap_or(0.0);
    let expected = |h: f64| c0 + k_isa * (h - h_cal);
    let (datum, datum_est) = match (q.geoid_ft, &ag) {
        (Some(n), _) => (Some(n), false),
        (None, Some(g)) => (
            median(
                cal.iter()
                    .filter_map(|&i| Some(interp(g, f.t[i])? - f.gnss[i]?))
                    .collect(),
            ),
            true,
        ),
        _ => (None, false),
    };

    // Per-row residuals.
    #[derive(Default, Clone)]
    struct R {
        fdm: Option<f64>,
        adsb: Option<f64>,
        trk: Option<f64>,
        baro_dis: Option<f64>,
        trk_m: Option<f64>,
        lost: bool,
    }
    let mut rows = vec![R::default(); f.t.len()];
    let (mut out_raw, mut out_res, mut isa_max, mut mis) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..f.t.len() {
        let t = f.t[i];
        let Some(b) = f.baro[i] else { continue };
        let e = expected(b);
        isa_max = isa_max.max((k_isa * (b - h_cal)).abs());
        let r = &mut rows[i];
        r.fdm = f.gnss[i].map(|g| g - b - e);
        r.lost = f.gnss[i].is_none() && t >= cal_end;
        if let (Some(g), Some(n)) = (&ag, datum) {
            r.adsb = interp(g, t).map(|x| x - n - b - e);
        }
        if let Some(a) = &ta {
            r.trk = interp(a, t).map(|x| x - b - e);
        }
        if let Some(a) = &ab {
            r.baro_dis = interp(a, t).map(|x| x - b);
            // What the clock offset alone would have done to the comparison.
            if let (Some(x0), Some(x1)) = (interp(&adsb.as_ref().unwrap().alt, t), interp(a, t)) {
                mis = mis.max((x0 - x1).abs());
            }
        }
        if let (Some(v), Some(fp)) = (&tp, f.pos[i]) {
            r.trk_m = pos_at(v, t).map(|x| dist_m(fp.0, fp.1, x.0, x.1));
        }
    }
    let flagged = |r: &R| {
        r.lost
            || [r.fdm, r.adsb, r.trk]
                .iter()
                .flatten()
                .any(|x| x.abs() > p.alt_tol_ft)
            || r.baro_dis.map(|x| x.abs() > p.baro_tol_ft).unwrap_or(false)
    };
    let mut raw: Vec<(usize, usize)> = vec![];
    for i in 0..f.t.len() {
        if f.t[i] < cal_end || !flagged(&rows[i]) {
            continue;
        }
        match raw.last_mut() {
            Some(x) if f.t[i] - f.t[x.1] <= p.merge_s => x.1 = i,
            _ => raw.push((i, i)),
        }
    }
    raw.retain(|(a, b)| f.t[*b] - f.t[*a] + 1.0 >= p.min_episode_s);
    // What the model explained away: rows after calibration and clear of every episode (by
    // the merge window, so an episode's ramp is not counted as explained).
    for i in 0..f.t.len() {
        let t = f.t[i];
        let near = raw
            .iter()
            .any(|(a, b)| t >= f.t[*a] - p.merge_s && t <= f.t[*b] + p.merge_s);
        if let (Some(g), Some(b), false) = (f.gnss[i], f.baro[i], t < cal_end || near) {
            let raw_d = g - b - c0;
            out_raw = out_raw.max(raw_d.abs());
            out_res = out_res.max((raw_d - k_isa * (b - h_cal)).abs());
        }
    }
    let mut episodes = vec![];
    for (n, (a, b)) in raw.iter().enumerate() {
        let mut res: BTreeMap<String, f64> = BTreeMap::new();
        let mut upd = |k: &str, v: Option<f64>| {
            if let Some(v) = v {
                let e = res.entry(k.to_string()).or_insert(0.0);
                if v.abs() > e.abs() {
                    *e = round(v, 0);
                }
            }
        };
        let (mut bd, mut tm, mut lost) = (None::<f64>, None::<f64>, 0usize);
        for r in &rows[*a..=*b] {
            upd("fdm_gnss", r.fdm);
            upd("adsb_geometric", r.adsb);
            upd("tracker", r.trk);
            if let Some(x) = r.baro_dis {
                if bd.map(|y| x.abs() > y.abs()).unwrap_or(true) {
                    bd = Some(x);
                }
            }
            if let Some(x) = r.trk_m {
                tm = Some(tm.unwrap_or(0.0).max(x));
            }
            lost += r.lost as usize;
        }
        let present: Vec<String> = res.keys().cloned().collect();
        let diverging: Vec<String> = res
            .iter()
            .filter(|(_, v)| v.abs() > p.alt_tol_ft)
            .map(|(k, _)| k.clone())
            .collect();
        let baro_ok = bd.map(|x| x.abs() <= p.baro_tol_ft).unwrap_or(true);
        let len = b - a + 1;
        let same_sign = {
            let s: Vec<f64> = diverging.iter().map(|k| res[k].signum()).collect();
            s.windows(2).all(|w| w[0] == w[1])
        };
        let (explanation, text) = if lost * 2 >= len {
            (Explanation::GnssLoss, "The FDM GNSS altitude was missing for most of the episode: consistent with loss of GNSS (jamming-like); the barometric sources carry the altitude.".to_string())
        } else if !baro_ok {
            (Explanation::BarometricDisagreement, format!("ADS-B and FDM barometric altitudes disagree by up to {:.0} ft: an altimeter-setting, static-system or recording matter, not shown to be a GNSS effect.", bd.unwrap_or(0.0)))
        } else if diverging.len() >= 2 && diverging.len() == present.len() && same_sign {
            (Explanation::CommonGnssDivergence, format!(
                "Every GNSS-derived altitude ({}) departs from barometric altitude in the same sense, beyond what the calibration offset{} explain, while the barometric sources agree: consistent with GNSS interference affecting the receivers on board, not with the atmosphere or a datum.",
                diverging.join(", "),
                if k_isa != 0.0 { ", ISA term and datum" } else { " and datum" }
            ))
        } else {
            (Explanation::SingleSource, format!(
                "Only {} departs beyond the tolerance while {} do not: an equipment, installation or recording matter of that source.",
                diverging.join(", "),
                if present.len() > diverging.len() { "the other GNSS-derived sources" } else { "no other GNSS-derived source is present and" }
            ))
        };
        episodes.push(Episode {
            n: n + 1,
            start_utc: iso(f.t[*a]),
            end_utc: iso(f.t[*b]),
            duration_s: round(f.t[*b] - f.t[*a] + 1.0, 1),
            onset_uncertainty_s: round(if *a > 0 { f.t[*a] - f.t[a - 1] } else { 0.0 }, 1),
            baro_alt_ft_at_onset: f.baro[*a].map(|x| round(x, 0)),
            residual_ft: res,
            diverging,
            baro_disagreement_ft: bd.map(|x| round(x, 0)),
            tracker_vs_fdm_m: tm.map(|x| round(x, 0)),
            explanation,
            text,
        });
    }

    let mut timeline: Vec<TimelineEntry> = vec![];
    let mut push = |t: f64, s: &str, x: String| {
        timeline.push(TimelineEntry {
            utc: iso(t),
            source: s.into(),
            text: x,
            t,
        })
    };
    push(t0, "fdm", "Record starts; calibration window opens".into());
    for a in &alignment {
        push(
            t0,
            a.source.as_str(),
            format!(
                "Clock offset {:+.0} s applied (RMS {} → {} {})",
                a.offset_s, a.rms_before, a.rms_after, a.unit
            ),
        );
    }
    for e in &fdm_rep.events {
        if let (Some(t), Some(x)) = (parse_utc(&e.start_utc), parse_utc(&e.end_utc)) {
            let overlaps = episodes.iter().any(|p| {
                parse_utc(&p.start_utc).unwrap_or(f64::INFINITY) <= x
                    && parse_utc(&p.end_utc).unwrap_or(f64::NEG_INFINITY) >= t
            });
            push(
                t,
                "fdm detector",
                format!(
                    "{:?} event {} starts{}",
                    e.kind,
                    e.n,
                    if overlaps {
                        ""
                    } else {
                        ": no unexplained divergence here (accounted for by the calibration offset, ISA term, datum or clock alignment)"
                    }
                ),
            );
        }
        if let Some(t) = parse_utc(&e.end_utc) {
            push(
                t,
                "fdm detector",
                format!("{:?} event {} ends", e.kind, e.n),
            );
        }
    }
    for e in &episodes {
        push(
            parse_utc(&e.start_utc).unwrap_or(t0),
            "comparison",
            format!("Episode {} starts: {:?}", e.n, e.explanation),
        );
        push(
            parse_utc(&e.end_utc).unwrap_or(t0),
            "comparison",
            format!("Episode {} ends", e.n),
        );
    }
    timeline.sort_by(|a, b| a.t.total_cmp(&b.t));
    let verdict = if episodes.is_empty() {
        "No unexplained GNSS, barometric or ADS-B altitude divergence (MODELLED)".to_string()
    } else {
        format!(
            "{} unexplained divergence episode(s): {} (MODELLED)",
            episodes.len(),
            episodes
                .iter()
                .map(|e| format!("{:?}", e.explanation))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Ok((
        Report {
            format: FORMAT,
            method: METHOD_ID,
            evidence: EVIDENCE,
            scope: SCOPE,
            title: q.title.clone(),
            aircraft_id: q.aircraft_id.clone(),
            data_source: q.data_source.clone(),
            params: p.clone(),
            inputs,
            alignment,
            explained: Explained {
                calibration_offset_ft: round(c0, 0),
                isa_term_ft_max: round(isa_max, 0),
                datum_ft: datum.map(|x| round(x, 0)),
                datum_estimated: datum_est,
                outside_episodes_raw_ft: round(out_raw, 0),
                outside_episodes_residual_ft: round(out_res, 0),
                misalignment_effect_ft: round(mis, 0),
            },
            episodes,
            fdm_events: fdm_rep.events,
            timeline,
            limits: vec![
                "The ISA term is first order (temperature deviation uniform with height); a strong inversion or a QNH error is not modelled.".into(),
                "Clock offsets are whole seconds and constant over the record; drift is not modelled.".into(),
                "The calibration window is taken as clean; interference inside it biases every offset.".into(),
                "ADS-B barometric altitude is quantised (25 or 100 ft).".into(),
                "Alignment and comparison only: no source is fused into a navigation solution.".into(),
            ],
            verdict,
        },
        fdm_text,
    ))
}

/// HTML page.
pub fn html(r: &Report) -> String {
    let mut intro = vec![
        r.scope.to_string(),
        format!("Data source: {}. {}", r.data_source, r.evidence),
    ];
    intro.push(format!(
        "Explained: calibration GNSS-baro offset {:.0} ft; ISA term up to {:.0} ft; datum {}; outside the episodes the GNSS-baro change was up to {:.0} ft, {:.0} ft after the ISA term; clock offsets alone would have caused up to {:.0} ft.",
        r.explained.calibration_offset_ft,
        r.explained.isa_term_ft_max,
        r.explained.datum_ft.map(|x| format!("{x:.0} ft{}", if r.explained.datum_estimated { " (estimated)" } else { "" })).unwrap_or_else(|| "not used".into()),
        r.explained.outside_episodes_raw_ft,
        r.explained.outside_episodes_residual_ft,
        r.explained.misalignment_effect_ft
    ));
    crate::transport::page(
        &r.title,
        r.evidence,
        &r.verdict,
        &intro,
        &[
            (
                "Alignment".into(),
                vec!["source", "offset (s)", "RMS before", "RMS after", "on"],
                r.alignment
                    .iter()
                    .map(|a| {
                        vec![
                            a.source.clone(),
                            format!("{:+.0}", a.offset_s),
                            format!("{} {}", a.rms_before, a.unit),
                            format!("{} {}", a.rms_after, a.unit),
                            a.on.clone(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Divergence episodes".into(),
                vec!["#", "from", "to", "residuals (ft)", "explanation"],
                r.episodes
                    .iter()
                    .map(|e| {
                        vec![
                            e.n.to_string(),
                            e.start_utc.clone(),
                            e.end_utc.clone(),
                            e.residual_ft
                                .iter()
                                .map(|(k, v)| format!("{k} {v:+.0}"))
                                .collect::<Vec<_>>()
                                .join(", "),
                            e.text.clone(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Timeline".into(),
                vec!["UTC", "source", "event"],
                r.timeline
                    .iter()
                    .map(|e| vec![e.utc.clone(), e.source.clone(), e.text.clone()])
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                r.limits.iter().map(|l| vec![l.clone()]).collect(),
            ),
        ],
    )
}

/// SYNTHETIC King Air-type flight.
pub mod synth {
    use crate::forensics::inputs::iso;
    use crate::transport::offset_deg;

    /// Start time (2026-06-01T09:00:00Z).
    pub const T0: f64 = 1_780_304_400.0;
    /// Divergence window (s after T0) and its peak (ft above the true GNSS altitude).
    pub const EV: (f64, f64) = (2400.0, 3300.0);
    pub const PEAK_FT: f64 = 2000.0;
    /// Clock offsets of the ADS-B and tracker records (s: source time minus FDM time).
    pub const ADSB_CLOCK_S: f64 = 2.0;
    pub const TRACKER_CLOCK_S: f64 = -5.0;
    /// ISA deviation and geoid height used.
    pub const ISA_DEV_C: f64 = 15.0;
    pub const GEOID_FT: f64 = 150.0;
    const DUR: usize = 4200;

    fn baro(s: f64) -> f64 {
        // Climb at 1500 ft/min from 1000 ft to FL250, then cruise.
        (1000.0 + 25.0 * s).min(25_000.0)
    }

    /// The flight: (fdm.csv, adsb.csv, tracker.csv, request.toml).
    pub fn make() -> (String, String, String, String) {
        let mut fdm =
            String::from("time,lat,lon,gnss_alt_ft,baro_alt_ft,gs_kt,track_deg,ins_gs_kt\n");
        let mut adsb =
            String::from("timestamp,aircraft_id,lat,lon,alt_baro_ft,alt_geom_ft,nic,nacp\n");
        let mut trk = String::from("timestamp,lat,lon,alt_ft\n");
        let (lat0, lon0, gs) = (47.0, 8.0, 260.0);
        let v = gs * 1852.0 / 3600.0;
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut n = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let spoof = |s: f64| {
            if s < EV.0 || s >= EV.1 {
                0.0
            } else {
                // Ramps to the peak in 120 s, holds, steps back at the end.
                PEAK_FT * ((s - EV.0) / 120.0).min(1.0)
            }
        };
        for k in 0..DUR {
            let s = k as f64;
            let b = baro(s);
            let h_cal = 1000.0 + 25.0 * 150.0; // median baro over the first 300 s
                                               // True MSL altitude in ISA+15: the GNSS altitude reads above the pressure altitude.
            let truth_msl = b + 120.0 + ISA_DEV_C / 288.15 * (b - h_cal);
            let g = truth_msl + spoof(s);
            let (la, lo) = offset_deg(lat0, lon0, v * s, 0.0);
            let (sla, slo) = if spoof(s) > 0.0 {
                offset_deg(la, lo, 0.0, 15.0 * (s - EV.0).min(400.0))
            } else {
                (la, lo)
            };
            fdm += &format!(
                "{},{sla:.6},{slo:.6},{:.0},{b:.0},{gs:.1},90.0,{gs:.1}\n",
                iso(T0 + s),
                g + 10.0 * n()
            );
            if k % 2 == 0 {
                let ts = T0 + s + ADSB_CLOCK_S;
                adsb += &format!(
                    "{},KA350,{sla:.6},{slo:.6},{:.0},{:.0},8,9\n",
                    iso(ts),
                    (b / 25.0).round() * 25.0,
                    g + GEOID_FT + 15.0 * n()
                );
            }
            if k % 5 == 0 {
                trk += &format!(
                    "{},{sla:.6},{slo:.6},{:.0}\n",
                    iso(T0 + s + TRACKER_CLOCK_S),
                    g + 20.0 * n()
                );
            }
        }
        let req = format!(
            "title = \"SYNTHETIC King Air-type altitude divergence\"\naircraft_id = \"KA350\"\ndata_source = \"SYNTHETIC (kshana-pro aviation-recon synth); King Air-type profile, no operator or published data\"\nfdm = \"fdm.csv\"\nadsb = \"adsb.csv\"\ntracker = \"tracker.csv\"\nisa_dev_c = {ISA_DEV_C}\n"
        );
        (fdm, adsb, trk, req)
    }
}
