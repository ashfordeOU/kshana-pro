// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Fisheries VMS integrity indicators (E07 B0704)
//!
//! A vessel monitoring system (VMS) unit reports positions at a declared polling interval
//! through its own satellite link. Over the user's VMS export
//! (`timestamp,vessel_id,lat,lon[,sog_kn,cog_deg]`) and, where the user maps a VMS vessel to an
//! MMSI, the vessel's AIS, this module raises tamper and spoof **indicators**, each citing the
//! reports behind it:
//!
//! * **missed polls**: intervals longer than 1.5 declared intervals, with the polls missed;
//! * **frozen position**: three or more consecutive reports at an identical position while
//!   the reported or AIS speed says the vessel was moving;
//! * **jump**: an implied speed above the track engine's limit;
//! * **too-regular track**: six or more consecutive reports with identical speed and course
//!   lying on a straight line within one metre (what a replayed or generated track looks like,
//!   rarely what a fishing vessel does);
//! * **AIS disagreement**: the VMS position more than the stated tolerance from the AIS
//!   position interpolated to the same time.
//!
//! **MODELLED** indicators for review; not a finding of tampering.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{Params, Tracks, KN};
use crate::forensics::inputs::{iso, parse_utc};
use crate::transport::{dist_m, en_m, Table};

/// One VMS position.
#[derive(Clone, Debug, PartialEq)]
pub struct Vms {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    pub sog_kn: Option<f64>,
    pub cog_deg: Option<f64>,
}

/// Parse the VMS CSV into tracks by vessel id (sorted).
pub fn parse(text: &str) -> Result<BTreeMap<String, Vec<Vms>>, String> {
    let t = Table::parse(text)?;
    let (ct, cv, cla, clo) = (
        t.need("timestamp")?,
        t.need("vessel_id")?,
        t.need("lat")?,
        t.need("lon")?,
    );
    let (cs, cc) = (t.col("sog_kn"), t.col("cog_deg"));
    let mut out: BTreeMap<String, Vec<Vms>> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp '{ts}'", i + 2))?;
        let num = |c| {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty coordinate", i + 2))
        };
        out.entry(t.text(i, Some(cv)).to_string())
            .or_default()
            .push(Vms {
                t: tt,
                lat: num(cla)?,
                lon: num(clo)?,
                sog_kn: t.num(i, cs)?,
                cog_deg: t.num(i, cc)?,
            });
    }
    for v in out.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    Ok(out)
}

/// One indicator with the reports behind it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VmsIndicator {
    pub code: &'static str,
    pub text: String,
    pub observations: Vec<String>,
}

/// Indicators for one VMS vessel.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VmsResult {
    pub vessel_id: String,
    pub mmsi: Option<String>,
    pub reports: usize,
    pub missed_polls: usize,
    pub indicators: Vec<VmsIndicator>,
}

/// Check one VMS track.
pub fn check(
    id: &str,
    v: &[Vms],
    interval_s: f64,
    mmsi: Option<&str>,
    ais: &Tracks,
    disagreement_m: f64,
    p: &Params,
) -> VmsResult {
    let mut inds = Vec::new();
    // Missed polls.
    let mut missed = 0;
    let mut obs = Vec::new();
    for w in v.windows(2) {
        let dt = w[1].t - w[0].t;
        if dt > 1.5 * interval_s {
            let m = (dt / interval_s).round() as usize - 1;
            missed += m;
            obs.push(format!(
                "{} to {}: {m} poll(s) missed",
                iso(w[0].t),
                iso(w[1].t)
            ));
        }
    }
    if missed > 0 {
        inds.push(VmsIndicator {
            code: "missed-polls",
            text: format!("{missed} poll(s) missed at a declared interval of {interval_s:.0} s"),
            observations: obs,
        });
    }
    let ais_at = |t: f64| mmsi.and_then(|m| ais.at(m, t, p.interp_max_s));
    // Frozen position.
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1].lat == v[i].lat && v[j + 1].lon == v[i].lon {
            j += 1;
        }
        if j - i + 1 >= 3 {
            let moving = (i..=j).any(|k| {
                v[k].sog_kn.is_some_and(|s| s >= 1.0)
                    || ais_at(v[k].t).is_some_and(|(_, _, s)| s.is_some_and(|s| s >= 1.0))
            });
            if moving {
                inds.push(VmsIndicator {
                    code: "frozen-position",
                    text: format!(
                        "{} consecutive reports at {:.5},{:.5} while the reported or AIS speed shows movement",
                        j - i + 1,
                        v[i].lat,
                        v[i].lon
                    ),
                    observations: (i..=j).map(|k| iso(v[k].t)).collect(),
                });
            }
        }
        i = j + 1;
    }
    // Jumps.
    let jumps: Vec<String> = v
        .windows(2)
        .filter_map(|w| {
            let s =
                dist_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon) / (w[1].t - w[0].t).max(1.0) / KN;
            (s > p.max_speed_kn).then(|| format!("{}: {s:.0} kn implied", iso(w[1].t)))
        })
        .collect();
    if !jumps.is_empty() {
        inds.push(VmsIndicator {
            code: "jump",
            text: format!(
                "{} implied-speed jump(s) above {} kn",
                jumps.len(),
                p.max_speed_kn
            ),
            observations: jumps,
        });
    }
    // Too-regular runs.
    let mut k = 0;
    while k + 6 <= v.len() {
        let same = |a: &Vms, b: &Vms| {
            a.sog_kn.is_some()
                && a.sog_kn == b.sog_kn
                && a.cog_deg.is_some()
                && a.cog_deg == b.cog_deg
        };
        let mut e = k;
        while e + 1 < v.len() && same(&v[k], &v[e + 1]) {
            e += 1;
        }
        if e - k + 1 >= 6 {
            let (x1, y1) = en_m(v[k].lat, v[k].lon, v[e].lat, v[e].lon);
            let len = x1.hypot(y1);
            let off = (k..=e)
                .map(|q| {
                    let (x, y) = en_m(v[k].lat, v[k].lon, v[q].lat, v[q].lon);
                    if len > 0.0 {
                        (x * y1 - y * x1).abs() / len
                    } else {
                        x.hypot(y)
                    }
                })
                .fold(0.0, f64::max);
            if off < 1.0 && len > 0.0 {
                inds.push(VmsIndicator {
                    code: "too-regular-track",
                    text: format!(
                        "{} consecutive reports with identical speed and course on a straight line (largest offset {off:.2} m)",
                        e - k + 1
                    ),
                    observations: (k..=e).map(|q| iso(v[q].t)).collect(),
                });
            }
            k = e + 1;
        } else {
            k += 1;
        }
    }
    // AIS disagreement.
    if mmsi.is_some() {
        let d: Vec<String> = v
            .iter()
            .filter_map(|x| {
                let (la, lo, _) = ais_at(x.t)?;
                let dd = dist_m(x.lat, x.lon, la, lo);
                (dd > disagreement_m)
                    .then(|| format!("{}: {:.1} km from AIS", iso(x.t), dd / 1000.0))
            })
            .collect();
        if !d.is_empty() {
            inds.push(VmsIndicator {
                code: "ais-disagreement",
                text: format!(
                    "{} VMS report(s) more than {:.1} km from the AIS position at the same time",
                    d.len(),
                    disagreement_m / 1000.0
                ),
                observations: d,
            });
        }
    }
    VmsResult {
        vessel_id: id.into(),
        mmsi: mmsi.map(str::to_string),
        reports: v.len(),
        missed_polls: missed,
        indicators: inds,
    }
}

/// `(t, lat, lon)` points of a VMS track, for gap classification.
pub fn points(v: &[Vms]) -> Vec<(f64, f64, f64)> {
    v.iter().map(|x| (x.t, x.lat, x.lon)).collect()
}
