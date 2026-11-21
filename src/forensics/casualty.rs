// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Maritime casualty reconstruction: grounding and collision (research 2026-11-20 gaps, B0601)
//!
//! After a grounding or a collision the investigator asks two questions the actor-vs-victim
//! pack ([`super::maritime`]) does not answer on its own: **where was the ship really, against
//! where its displays said it was**, and **did interference cause the casualty, or was it a
//! navigational error that would have happened anyway?**
//!
//! Inputs: the vessel's VDR NMEA export (GGA/RMC/GSV plus gyro heading HDT and log speed VHW),
//! AIS of the vessel and of the other ship in a collision, a radar log of ranges and true
//! bearings to charted, radar-conspicuous objects and to the other ship, and the casualty
//! (time, the charted danger for a grounding or the other ship for a collision).
//!
//! What it produces, in one signed pack:
//!
//! * **Displayed track**: the GNSS positions the VDR recorded (what ECDIS showed).
//! * **Reconstructed true track**: outside interference the displayed track; inside it, dead
//!   reckoning from the last clean fix on the ship's own gyro heading and log speed, with a
//!   1-sigma envelope that grows with time ([`Params`]). Radar range-and-bearing fixes on
//!   charted objects are shown beside it as **independent checks**. Nothing is fused, filtered
//!   or matched to a chart: the two reconstructions are compared, not combined.
//! * **Interference timeline**: the episodes of the actor-vs-victim method, which runs on the
//!   same data and whose pack is embedded under `interference/`.
//! * **Causation statement**: interference, navigational error, interference with a
//!   navigational factor, or undetermined, from rules fixed before the run, with the
//!   displayed-vs-reconstructed divergence at the casualty and its uncertainty, and a
//!   rule-based confidence ([`CONFIDENCE_RULE`]).
//!
//! For a collision the radar's relative track of the other ship gives the closest point of
//! approach without GNSS; a collision course visible on radar for long enough is a
//! navigational matter whatever the GNSS did.
//!
//! Wording: an engineering reconstruction under stated assumptions, not a finding of fact or
//! of blame; that is for the investigation body. **MODELLED**: exercised on synthetic
//! casualties with known truth ([`synth`]); no published casualty data were used. Nothing
//! here locates an emitter.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::receiver_trust::ingest::read_nmea;
use serde::{Deserialize, Serialize};

use super::inputs::{iso, parse_tracks, parse_utc, round, sha256_hex, PositionReport};
use super::maritime;
use crate::transport::{dist_m, en_m, offset_deg, Table};

/// Pro feature.
pub const FEATURE: &str = "pro-casualty-reconstruction";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-casualty-reconstruction/1";
/// Method.
pub const METHOD_ID: &str = "kshana-pro/casualty-reconstruction/v1";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED: rules exercised on synthetic grounding and collision \
cases with known truth; not validated against a labelled set of real casualties.";
/// What every pack says about itself.
pub const STATEMENT: &str = "An engineering reconstruction of the data supplied under \
assumptions and thresholds fixed before the run. The reconstructed track is dead reckoning on \
the ship's own gyro and log from the last clean GNSS fix; radar fixes are independent checks \
beside it; nothing is fused or chart-matched. Not a finding of fact or of blame, which is for \
the investigation body.";
/// Confidence rule.
pub const CONFIDENCE_RULE: &str = "Indicators combine as 1 − Π(1 − cᵢ): interference episode \
recorded in the look-back window 0.4; divergence at the casualty above k sigma 0.5; displayed \
track clear of the danger while the reconstructed track met it 0.5; radar disagreement with \
the displayed position 0.4; radar collision course for the warning time 0.6; displayed track \
itself met the danger 0.6. A ranking aid stated before the run, not a calibrated probability.";

const KN: f64 = 1852.0 / 3600.0;

/// Assumptions and thresholds, written into every pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Look-back window before the casualty (s).
    pub lookback_s: f64,
    /// 1-sigma error of the gyro heading (deg).
    pub heading_sigma_deg: f64,
    /// 1-sigma error of the log speed (kn).
    pub speed_sigma_kn: f64,
    /// 1-sigma unknown set and drift (kn).
    pub current_sigma_kn: f64,
    /// 1-sigma error of a clean GNSS fix (m).
    pub gnss_sigma_m: f64,
    /// 1-sigma radar range error (m) and bearing error (deg).
    pub radar_range_sigma_m: f64,
    pub radar_bearing_sigma_deg: f64,
    /// Significance multiplier on sigma.
    pub k_sigma: f64,
    /// Clearance from the danger (m) the displayed track must keep to count as "clear".
    pub safety_m: f64,
    /// CPA (m) below which the radar showed a collision course.
    pub cpa_alarm_m: f64,
    /// Warning time (s): a collision course or a radar disagreement seen this long before the
    /// casualty was available to the bridge.
    pub min_warning_s: f64,
    /// Sampling of the track table (s).
    pub sample_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            lookback_s: 1800.0,
            heading_sigma_deg: 1.0,
            speed_sigma_kn: 0.3,
            current_sigma_kn: 0.5,
            gnss_sigma_m: 10.0,
            radar_range_sigma_m: 30.0,
            radar_bearing_sigma_deg: 1.0,
            k_sigma: 2.0,
            safety_m: 200.0,
            cpa_alarm_m: 926.0,
            min_warning_s: 300.0,
            sample_s: 10.0,
        }
    }
}

/// Kind of casualty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CasualtyKind {
    Grounding,
    Collision,
}

/// A charted point (danger or radar-conspicuous object).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charted {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    /// Radius of the danger (m); 0 for a point object.
    #[serde(default)]
    pub radius_m: f64,
}

/// The request (`kshana-pro casualty run <request.toml>`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub vessel_id: String,
    pub casualty: CasualtyKind,
    pub casualty_utc: String,
    /// VDR NMEA export.
    pub nmea: String,
    /// AIS CSV (`timestamp,vessel_id,lat,lon[,sog_kn]`).
    #[serde(default)]
    pub ais: Option<String>,
    /// Radar log CSV (`timestamp,target,range_m,bearing_deg`, bearings true).
    #[serde(default)]
    pub radar: Option<String>,
    /// Grounding: the charted danger.
    #[serde(default)]
    pub hazard: Option<Charted>,
    /// Collision: the other ship (its AIS identifier and radar target name).
    #[serde(default)]
    pub other_vessel: Option<String>,
    /// Charted radar-conspicuous objects the radar log names.
    #[serde(default)]
    pub reference: Vec<Charted>,
    /// Where the inputs come from (e.g. "SYNTHETIC" or the case reference).
    pub data_source: String,
    #[serde(default)]
    pub params: Params,
}

/// Causation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Causation {
    /// Interference put the displayed position clear of the danger while the ship was not.
    Interference,
    /// Interference is recorded and contributed, and a navigational factor (an independent
    /// means showed the error or the risk, or the bridge ran without GNSS) is present.
    InterferenceWithNavigationalFactor,
    /// The displayed picture itself showed the danger or the collision course.
    NavigationalError,
    /// The data do not decide.
    Undetermined,
}

/// One row of the track table.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TrackRow {
    pub utc: String,
    pub displayed: Option<[f64; 2]>,
    pub reconstructed: Option<[f64; 2]>,
    pub sigma_m: f64,
    pub divergence_m: Option<f64>,
    /// `gnss` (clean fix) or `dead-reckoning`.
    pub source: String,
}

/// One radar fix on a charted object.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RadarFix {
    pub utc: String,
    pub reference: String,
    pub position: [f64; 2],
    pub sigma_m: f64,
    pub vs_displayed_m: Option<f64>,
    pub vs_reconstructed_m: Option<f64>,
    /// The displayed position disagrees with this fix beyond k sigma.
    pub displayed_disagrees: bool,
}

/// Radar closest point of approach of the other ship.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RadarCpa {
    pub utc: String,
    pub range_m: f64,
    pub cpa_m: f64,
    pub tcpa_s: f64,
}

/// One timeline row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TimelineEntry {
    pub utc: String,
    pub source: String,
    pub text: String,
    #[serde(skip)]
    t: f64,
}

/// Geometry at the casualty.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AtCasualty {
    pub displayed: Option<[f64; 2]>,
    pub reconstructed: Option<[f64; 2]>,
    pub reconstructed_sigma_m: f64,
    pub divergence_m: Option<f64>,
    pub divergence_sigma_m: f64,
    /// Grounding: distance of each position from the danger's edge (m).
    pub displayed_clearance_m: Option<f64>,
    pub reconstructed_clearance_m: Option<f64>,
    /// Collision: separation from the other ship's AIS position (m).
    pub displayed_separation_m: Option<f64>,
    pub reconstructed_separation_m: Option<f64>,
    /// Seconds without a GNSS fix before the casualty.
    pub no_fix_before_s: f64,
}

/// The causation statement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Statement {
    pub causation: Causation,
    pub confidence: f64,
    pub text: String,
    pub rationale: Vec<String>,
    pub uncertainty: Vec<String>,
}

/// The pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub evidence: &'static str,
    pub statement_of_method: &'static str,
    pub confidence_rule: &'static str,
    pub title: String,
    pub vessel_id: String,
    pub casualty: CasualtyKind,
    pub casualty_utc: String,
    pub data_source: String,
    pub params: Params,
    pub inputs: BTreeMap<String, String>,
    pub interference_episodes: Vec<maritime::Episode>,
    pub interference_conclusion: maritime::Conclusion,
    pub dead_reckoning_from_utc: Option<String>,
    pub at_casualty: AtCasualty,
    pub radar_fixes: Vec<RadarFix>,
    pub radar_cpa: Vec<RadarCpa>,
    pub tracks: Vec<TrackRow>,
    pub timeline: Vec<TimelineEntry>,
    pub causation: Statement,
    pub limits: Vec<String>,
    pub verdict: String,
}

#[derive(Clone, Debug)]
struct Epoch {
    t: f64,
    fix: Option<(f64, f64)>,
    hdg: Option<f64>,
    stw: Option<f64>,
}

fn epochs(text: &str) -> Result<Vec<Epoch>, String> {
    let tl = read_nmea(text)?;
    let mut out: Vec<Epoch> = tl
        .epochs
        .iter()
        .filter_map(|e| {
            let t = e.time_label.as_deref().and_then(parse_utc)?;
            let m = e.marine.clone().unwrap_or_default();
            let valid = m.fix_valid.unwrap_or(e.fix.is_some());
            Some(Epoch {
                t,
                fix: if valid {
                    e.fix.map(|f| (f.lat_deg, f.lon_deg))
                } else {
                    None
                },
                hdg: m.heading_deg,
                stw: m.stw_kn,
            })
        })
        .collect();
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok(out)
}

fn at_or_before<T>(v: &[(f64, T)], t: f64, tol: f64) -> Option<&T> {
    let i = v.partition_point(|x| x.0 <= t);
    i.checked_sub(1)
        .map(|k| &v[k])
        .filter(|x| t - x.0 <= tol)
        .map(|x| &x.1)
}

fn ais_at(v: &[PositionReport], t: f64) -> Option<(f64, f64)> {
    let i = v.partition_point(|r| r.t <= t);
    match (i.checked_sub(1).map(|k| &v[k]), v.get(i)) {
        (Some(a), Some(b)) if b.t > a.t && b.t - a.t <= 120.0 => {
            let f = (t - a.t) / (b.t - a.t);
            Some((a.lat + f * (b.lat - a.lat), a.lon + f * (b.lon - a.lon)))
        }
        (Some(a), _) if t - a.t <= 30.0 => Some((a.lat, a.lon)),
        _ => None,
    }
}

fn combine(cs: &[f64]) -> f64 {
    1.0 - cs.iter().fold(1.0, |a, c| a * (1.0 - c))
}

fn ll(p: (f64, f64)) -> [f64; 2] {
    [round(p.0, 6), round(p.1, 6)]
}

fn read(base: &Path, p: &str) -> Result<Vec<u8>, String> {
    let path = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        base.join(p)
    };
    std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Reconstruct a casualty. `base` resolves relative input paths.
pub fn run(q: &Request, base: &Path) -> Result<(Report, maritime::EvidencePack), String> {
    let p = &q.params;
    if q.data_source.trim().is_empty() {
        return Err("data_source must say where the inputs come from".into());
    }
    if !(p.k_sigma > 0.0 && p.lookback_s > 0.0 && p.sample_s > 0.0) {
        return Err("k_sigma, lookback_s and sample_s must be positive".into());
    }
    let tc = parse_utc(&q.casualty_utc)
        .ok_or_else(|| format!("casualty_utc '{}' is not a UTC time", q.casualty_utc))?;
    match q.casualty {
        CasualtyKind::Grounding if q.hazard.is_none() => {
            return Err("a grounding needs [hazard] (the charted danger)".into())
        }
        CasualtyKind::Collision if q.other_vessel.is_none() => {
            return Err("a collision needs other_vessel".into())
        }
        _ => {}
    }
    let mut inputs = BTreeMap::new();
    let nmea_b = read(base, &q.nmea)?;
    inputs.insert(q.nmea.clone(), sha256_hex(&nmea_b));
    let nmea = String::from_utf8_lossy(&nmea_b).to_string();
    let ais_text = match &q.ais {
        Some(a) => {
            let b = read(base, a)?;
            inputs.insert(a.clone(), sha256_hex(&b));
            Some(String::from_utf8_lossy(&b).to_string())
        }
        None => None,
    };
    let radar = match &q.radar {
        Some(r) => {
            let b = read(base, r)?;
            inputs.insert(r.clone(), sha256_hex(&b));
            Some(Table::parse(&String::from_utf8_lossy(&b)).map_err(|e| format!("{r}: {e}"))?)
        }
        None => None,
    };

    // Interference: the actor-vs-victim method on the same data.
    let mut files = vec![(q.nmea.clone(), nmea_b.clone())];
    if let (Some(a), Some(t)) = (&q.ais, &ais_text) {
        files.push((a.clone(), t.clone().into_bytes()));
    }
    let mi = maritime::MaritimeInputs {
        title: format!("{}: interference", q.title),
        vessel_id: q.vessel_id.clone(),
        nmea: nmea.clone(),
        ais: ais_text.clone(),
        station: vec![],
        files,
        params: maritime::Params::default(),
    };
    let mpack = maritime::analyse(&mi)?;
    let w0 = tc - p.lookback_s;
    let rel: Vec<&maritime::Episode> = mpack
        .episodes
        .iter()
        .filter(|e| {
            let (s, x) = (
                parse_utc(&e.start_utc).unwrap_or(f64::NAN),
                parse_utc(&e.end_utc).unwrap_or(f64::NAN),
            );
            s <= tc && x >= w0
        })
        .collect();
    let interference = !rel.is_empty();
    let (ep_start, ep_end) = rel.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, e| {
        (
            a.0.min(parse_utc(&e.onset.earliest_utc).unwrap_or(f64::INFINITY)),
            a.1.max(parse_utc(&e.end.latest_utc).unwrap_or(f64::NEG_INFINITY)),
        )
    });

    // Displayed and reconstructed tracks.
    let ep = epochs(&nmea)?;
    if ep.len() < 10 {
        return Err("the NMEA log has fewer than 10 timed epochs".into());
    }
    let disp: Vec<(f64, (f64, f64))> = ep.iter().filter_map(|e| e.fix.map(|f| (e.t, f))).collect();
    let anchor = if interference {
        disp.iter().rev().find(|(t, _)| *t <= ep_start).copied()
    } else {
        None
    };
    if interference && anchor.is_none() {
        return Err(
            "interference starts before the first clean GNSS fix: no anchor for dead reckoning"
                .into(),
        );
    }
    let sv = (p.speed_sigma_kn.powi(2) + p.current_sigma_kn.powi(2)).sqrt() * KN;
    let sh = p.heading_sigma_deg.to_radians();
    // (t, position, sigma, from dead reckoning)
    let mut rec: Vec<(f64, (f64, f64), f64, bool)> = vec![];
    // (t, east, north, accumulated 1-sigma drift): the errors are biases over the window
    // (current, log calibration, gyro error), so they add linearly with time.
    let mut dr: Option<(f64, f64, f64, f64)> = None;
    let (mut hdg, mut stw) = (None::<f64>, None::<f64>);
    for e in &ep {
        hdg = e.hdg.or(hdg);
        stw = e.stw.or(stw);
        let in_dr = match anchor {
            Some((ta, _)) => e.t >= ta && e.t <= ep_end.max(ta),
            None => false,
        };
        if in_dr {
            let (ta, a) = anchor.unwrap();
            let st = dr.get_or_insert((ta, 0.0, 0.0, 0.0));
            let dt = e.t - st.0;
            if dt > 0.0 {
                let (Some(h), Some(v)) = (hdg, stw) else {
                    return Err(format!(
                        "no gyro heading (HDT) or log speed (VHW) at {}: dead reckoning needs both",
                        iso(e.t)
                    ));
                };
                let vm = v * KN;
                st.1 += vm * h.to_radians().sin() * dt;
                st.2 += vm * h.to_radians().cos() * dt;
                st.3 += (sv.powi(2) + (vm * sh).powi(2)).sqrt() * dt;
                st.0 = e.t;
            }
            let pos = offset_deg(a.0, a.1, st.1, st.2);
            rec.push((e.t, pos, p.gnss_sigma_m.hypot(st.3), true));
        } else if let Some(f) = e.fix {
            rec.push((e.t, f, p.gnss_sigma_m, false));
        }
    }
    let rec_at = |t: f64| -> Option<((f64, f64), f64, bool)> {
        let i = rec.partition_point(|r| r.0 <= t);
        i.checked_sub(1)
            .map(|k| rec[k])
            .filter(|r| t - r.0 <= 30.0)
            .map(|r| (r.1, r.2, r.3))
    };
    let disp_at = |t: f64| at_or_before(&disp, t, 30.0).copied();

    let mut tracks = vec![];
    let t_first = ep[0].t.max(w0);
    let mut t = t_first;
    while t <= tc + 1e-6 {
        let d = disp_at(t);
        let r = rec_at(t);
        tracks.push(TrackRow {
            utc: iso(t),
            displayed: d.map(ll),
            reconstructed: r.map(|x| ll(x.0)),
            sigma_m: round(r.map(|x| x.1).unwrap_or(f64::NAN), 1),
            divergence_m: match (d, r) {
                (Some(a), Some(b)) => Some(round(dist_m(a.0, a.1, b.0 .0, b.0 .1), 1)),
                _ => None,
            },
            source: if r.map(|x| x.2).unwrap_or(false) {
                "dead-reckoning".into()
            } else {
                "gnss".into()
            },
        });
        t += p.sample_s;
    }

    // Radar: fixes on charted objects; the other ship's relative track.
    let mut radar_fixes = vec![];
    let mut rel_obs: Vec<(f64, f64, f64)> = vec![]; // (t, east, north) of the other ship
    if let Some(tb) = &radar {
        let (ct, cg, cr, cb) = (
            tb.need("timestamp")?,
            tb.need("target")?,
            tb.need("range_m")?,
            tb.need("bearing_deg")?,
        );
        for i in 0..tb.rows.len() {
            let Some(tt) = parse_utc(tb.text(i, Some(ct))) else {
                return Err(format!("radar row {}: bad timestamp", i + 2));
            };
            let (Some(r), Some(b)) = (tb.num(i, Some(cr))?, tb.num(i, Some(cb))?) else {
                continue;
            };
            if tt < w0 || tt > tc {
                continue;
            }
            let tgt = tb.text(i, Some(cg));
            // The charted reference and the GNSS track must be on one datum (WGS-84), and the
            // radar fix is the radar antenna's position: the offset to the GNSS antenna (lever
            // arm, tens of metres on a large ship) is not applied and is part of the tolerance
            // only through gnss_sigma_m.
            let (e, n) = (r * b.to_radians().sin(), r * b.to_radians().cos());
            if let Some(rf) = q.reference.iter().find(|x| x.name == tgt) {
                let pos = offset_deg(rf.lat, rf.lon, -e, -n);
                let sg = (p.radar_range_sigma_m.powi(2)
                    + (r * p.radar_bearing_sigma_deg.to_radians()).powi(2))
                .sqrt();
                let vd = disp_at(tt).map(|d| dist_m(d.0, d.1, pos.0, pos.1));
                let vr = rec_at(tt).map(|d| dist_m(d.0 .0, d.0 .1, pos.0, pos.1));
                let tol = p.k_sigma * (sg.powi(2) + p.gnss_sigma_m.powi(2)).sqrt();
                radar_fixes.push(RadarFix {
                    utc: iso(tt),
                    reference: tgt.to_string(),
                    position: ll(pos),
                    sigma_m: round(sg, 1),
                    vs_displayed_m: vd.map(|x| round(x, 1)),
                    vs_reconstructed_m: vr.map(|x| round(x, 1)),
                    displayed_disagrees: vd.map(|x| x > tol).unwrap_or(false),
                });
            } else if q.other_vessel.as_deref() == Some(tgt) {
                rel_obs.push((tt, e, n));
            }
        }
    }
    rel_obs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut radar_cpa = vec![];
    for (k, o) in rel_obs.iter().enumerate() {
        // The earliest observation 60 to 180 s back: a two-point relative velocity.
        let Some(prev) = rel_obs[..k]
            .iter()
            .find(|x| (60.0..=180.0).contains(&(o.0 - x.0)))
        else {
            continue;
        };
        let dt = o.0 - prev.0;
        let (vx, vy) = ((o.1 - prev.1) / dt, (o.2 - prev.2) / dt);
        let v2 = vx * vx + vy * vy;
        let tcpa = if v2 > 1e-9 {
            (-(o.1 * vx + o.2 * vy) / v2).max(0.0)
        } else {
            0.0
        };
        let cpa = ((o.1 + vx * tcpa).powi(2) + (o.2 + vy * tcpa).powi(2)).sqrt();
        radar_cpa.push(RadarCpa {
            utc: iso(o.0),
            range_m: round((o.1 * o.1 + o.2 * o.2).sqrt(), 1),
            cpa_m: round(cpa, 1),
            tcpa_s: round(tcpa, 1),
        });
    }
    // Continuous radar collision course up to the casualty.
    let radar_warning_s = {
        let mut first: Option<f64> = None;
        for c in radar_cpa.iter().rev() {
            if c.cpa_m < p.cpa_alarm_m {
                first = parse_utc(&c.utc);
            } else {
                break;
            }
        }
        first.map(|f| tc - f).unwrap_or(0.0)
    };
    let first_radar_disagree = radar_fixes
        .iter()
        .find(|f| f.displayed_disagrees)
        .and_then(|f| parse_utc(&f.utc));

    // At the casualty.
    let dc = disp_at(tc);
    let rc = rec_at(tc);
    let sig_c = rc.map(|x| x.1).unwrap_or(p.gnss_sigma_m);
    let sig_d = (sig_c.powi(2) + p.gnss_sigma_m.powi(2)).sqrt();
    let div = match (dc, rc) {
        (Some(a), Some(b)) => Some(dist_m(a.0, a.1, b.0 .0, b.0 .1)),
        _ => None,
    };
    let no_fix_before = match disp.iter().rev().find(|x| x.0 <= tc) {
        Some((t, _)) => tc - t,
        None => p.lookback_s,
    };
    let ais = match &ais_text {
        Some(t) => Some(parse_tracks(t, "vessel_id").map_err(|e| e.to_string())?),
        None => None,
    };
    let other_at = |t: f64| {
        let id = q.other_vessel.as_ref()?;
        ais_at(ais.as_ref()?.by_id.get(id)?, t)
    };
    let clear = |x: (f64, f64)| {
        q.hazard
            .as_ref()
            .map(|h| dist_m(x.0, x.1, h.lat, h.lon) - h.radius_m)
    };
    let sep = |x: (f64, f64)| other_at(tc).map(|o| dist_m(x.0, x.1, o.0, o.1));
    let at = AtCasualty {
        displayed: dc.map(ll),
        reconstructed: rc.map(|x| ll(x.0)),
        reconstructed_sigma_m: round(sig_c, 1),
        divergence_m: div.map(|x| round(x, 1)),
        divergence_sigma_m: round(sig_d, 1),
        displayed_clearance_m: dc.and_then(clear).map(|x| round(x, 1)),
        reconstructed_clearance_m: rc.and_then(|x| clear(x.0)).map(|x| round(x, 1)),
        displayed_separation_m: dc.and_then(sep).map(|x| round(x, 1)),
        reconstructed_separation_m: rc.and_then(|x| sep(x.0)).map(|x| round(x, 1)),
        no_fix_before_s: round(no_fix_before, 1),
    };

    // Causation.
    let k = p.k_sigma;
    let mut why = vec![];
    let mut cs = vec![];
    if interference {
        cs.push(0.4);
        why.push(format!(
            "{} interference episode(s) of the actor-vs-victim method fall in the {:.0} s before the casualty ({}).",
            rel.len(),
            p.lookback_s,
            rel.iter()
                .map(|e| format!("{:?} {} to {}", e.event_type, e.start_utc, e.end_utc))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    } else {
        why.push(format!(
            "No interference episode in the {:.0} s before the casualty.",
            p.lookback_s
        ));
    }
    let div_sig = div.map(|d| d > k * sig_d).unwrap_or(false);
    if let Some(d) = div {
        why.push(format!(
            "Displayed vs reconstructed position at the casualty: {d:.0} m (1 sigma {sig_d:.0} m; {} k = {k}).",
            if div_sig { "above" } else { "within" }
        ));
        if div_sig {
            cs.push(0.5);
        }
    }
    if let Some(f) = first_radar_disagree {
        cs.push(0.4);
        why.push(format!(
            "Radar fixes on charted objects disagreed with the displayed position from {}, {:.0} s before the casualty.",
            iso(f),
            tc - f
        ));
    }
    let radar_warned = first_radar_disagree
        .map(|f| tc - f >= p.min_warning_s)
        .unwrap_or(false);
    let (causation, text) = match q.casualty {
        CasualtyKind::Grounding => {
            let dcl = at.displayed_clearance_m;
            let rmet = rc
                .and_then(|x| clear(x.0))
                .map(|c| c <= k * sig_c)
                .unwrap_or(false);
            if let Some(c) = at.reconstructed_clearance_m {
                why.push(format!(
                    "Reconstructed position at the casualty is {c:.0} m from the danger's edge (1 sigma {sig_c:.0} m)."
                ));
            }
            if let Some(c) = dcl {
                why.push(format!(
                    "Displayed position at the casualty is {c:.0} m from the danger's edge (safety distance {:.0} m).",
                    p.safety_m
                ));
            }
            if !interference {
                if dcl.map(|c| c <= p.safety_m).unwrap_or(false) {
                    cs.push(0.6);
                    (Causation::NavigationalError, "The displayed track itself ran onto the charted danger with no interference recorded: consistent with a navigational error.".to_string())
                } else {
                    (Causation::Undetermined, "No interference recorded and the displayed track did not meet the danger: the data do not explain the grounding.".to_string())
                }
            } else if dc.is_none() {
                if rmet {
                    cs.push(0.5);
                }
                (Causation::InterferenceWithNavigationalFactor, format!(
                    "The ship had no GNSS position for {no_fix_before:.0} s before the casualty and the dead-reckoned track {} the danger: the loss of GNSS is recorded; how the ship was navigated without it is the navigational factor.",
                    if rmet { "met" } else { "passed near" }
                ))
            } else if div_sig && dcl.map(|c| c > p.safety_m).unwrap_or(false) && rmet {
                cs.push(0.5);
                if radar_warned {
                    (Causation::InterferenceWithNavigationalFactor, format!(
                        "Interference displaced the displayed position {:.0} m from the reconstructed track, which showed the ship clear of the danger while it was not; radar fixes showed the error {:.0} s before the casualty, an independent means available to the bridge.",
                        div.unwrap_or(0.0),
                        tc - first_radar_disagree.unwrap_or(tc)
                    ))
                } else {
                    (Causation::Interference, format!(
                        "Interference displaced the displayed position {:.0} m from the reconstructed track: the display showed the ship clear of the danger while the reconstructed track met it. Consistent with interference as the cause.",
                        div.unwrap_or(0.0)
                    ))
                }
            } else if !div_sig && dcl.map(|c| c <= p.safety_m).unwrap_or(false) {
                cs.push(0.6);
                (Causation::NavigationalError, "Interference is recorded but the displayed position agreed with the reconstructed one within its uncertainty and itself ran onto the danger: consistent with a navigational error.".to_string())
            } else {
                (Causation::Undetermined, "Interference is recorded but the divergence or the clearances do not decide whether it caused the grounding.".to_string())
            }
        }
        CasualtyKind::Collision => {
            if let (Some(a), Some(b)) = (at.displayed_separation_m, at.reconstructed_separation_m) {
                why.push(format!("Separation from the other ship's AIS position at the casualty: displayed {a:.0} m, reconstructed {b:.0} m."));
            }
            if radar_warning_s > 0.0 {
                why.push(format!(
                    "The radar's relative track of the other ship (independent of GNSS) showed a CPA below {:.0} m for {radar_warning_s:.0} s before the casualty.",
                    p.cpa_alarm_m
                ));
            }
            let radar_course = radar_warning_s >= p.min_warning_s;
            if radar_course {
                cs.push(0.6);
            }
            let disp_safe = at
                .displayed_separation_m
                .map(|s| s > p.cpa_alarm_m)
                .unwrap_or(false);
            let rec_met = at
                .reconstructed_separation_m
                .map(|s| s <= k * sig_d)
                .unwrap_or(false);
            match (interference, radar_course) {
                (false, true) => (Causation::NavigationalError, format!(
                    "No interference recorded; the radar showed a collision course for {radar_warning_s:.0} s before the collision: consistent with a navigational error."
                )),
                (true, true) => (Causation::InterferenceWithNavigationalFactor, format!(
                    "Interference is recorded, and the radar, independent of GNSS, showed a collision course for {radar_warning_s:.0} s before the collision: the navigational factor is present whatever the GNSS showed."
                )),
                (true, false) if div_sig && disp_safe && rec_met => {
                    cs.push(0.5);
                    (Causation::Interference, "Interference displaced the displayed position so that the other ship appeared clear while the reconstructed track met it, and no radar collision course is recorded for the warning time: consistent with interference as the cause.".to_string())
                }
                (false, false) if !disp_safe && at.displayed_separation_m.is_some() => {
                    cs.push(0.6);
                    (Causation::NavigationalError, "No interference recorded and the displayed picture showed the ships meeting: consistent with a navigational error.".to_string())
                }
                _ => (Causation::Undetermined, "The data do not decide between interference and a navigational error.".to_string()),
            }
        }
    };
    let mut uncertainty = vec![format!(
        "Reconstructed position 1 sigma at the casualty {sig_c:.0} m: dead reckoning with heading {:.1} deg, log {:.1} kn and unknown current {:.1} kn (1 sigma), growing linearly with time since the anchor fix.",
        p.heading_sigma_deg, p.speed_sigma_kn, p.current_sigma_kn
    )];
    for e in &rel {
        uncertainty.push(format!(
            "Episode {} onset between {} and {} ({:.0} s); end between {} and {}.",
            e.n,
            e.onset.earliest_utc,
            e.onset.latest_utc,
            e.onset.uncertainty_s,
            e.end.earliest_utc,
            e.end.latest_utc
        ));
    }
    uncertainty.push("An unmodelled current larger than its stated sigma biases the dead-reckoned track; radar fixes, where present, bound it independently.".into());
    let confidence = round(combine(&cs), 2);

    // Timeline.
    let mut timeline: Vec<TimelineEntry> = vec![];
    let mut push = |t: f64, s: &str, x: String| {
        timeline.push(TimelineEntry {
            utc: iso(t),
            source: s.into(),
            text: x,
            t,
        })
    };
    if let Some((ta, _)) = anchor {
        push(
            ta,
            "VDR NMEA",
            "Last clean GNSS fix: dead reckoning starts".into(),
        );
    }
    for e in &rel {
        if let Some(s) = parse_utc(&e.start_utc) {
            push(
                s,
                "interference",
                format!("{:?} episode {} starts", e.event_type, e.n),
            );
        }
        if let Some(x) = parse_utc(&e.end_utc) {
            push(
                x,
                "interference",
                format!("{:?} episode {} ends", e.event_type, e.n),
            );
        }
    }
    if let Some(f) = first_radar_disagree {
        push(
            f,
            "radar",
            "Radar fix disagrees with the displayed position".into(),
        );
    }
    if radar_warning_s > 0.0 {
        push(
            tc - radar_warning_s,
            "radar",
            format!(
                "Radar CPA of {} below {:.0} m",
                q.other_vessel.clone().unwrap_or_default(),
                p.cpa_alarm_m
            ),
        );
    }
    push(tc, "casualty", format!("{:?}", q.casualty));
    timeline.sort_by(|a, b| a.t.total_cmp(&b.t));

    let verdict = format!(
        "{:?}: {:?} (confidence {:.2}, MODELLED)",
        q.casualty, causation, confidence
    );
    Ok((
        Report {
            format: FORMAT,
            method: METHOD_ID,
            evidence: EVIDENCE,
            statement_of_method: STATEMENT,
            confidence_rule: CONFIDENCE_RULE,
            title: q.title.clone(),
            vessel_id: q.vessel_id.clone(),
            casualty: q.casualty,
            casualty_utc: iso(tc),
            data_source: q.data_source.clone(),
            params: p.clone(),
            inputs,
            interference_episodes: rel.into_iter().cloned().collect(),
            interference_conclusion: mpack.verdict.conclusion,
            dead_reckoning_from_utc: anchor.map(|a| iso(a.0)),
            at_casualty: at,
            radar_fixes,
            radar_cpa,
            tracks,
            timeline,
            causation: Statement {
                causation,
                confidence,
                text,
                rationale: why,
                uncertainty,
            },
            limits: vec![
                "Dead reckoning assumes the gyro and log were sound; an interference that also affected them is outside this method.".into(),
                "The other ship's AIS position is GNSS-derived and may itself have been affected; the radar relative track is not.".into(),
                "Radar fixes need the charted position of the object; a misidentified object gives a false fix.".into(),
                "Thresholds are pre-registered (params); the method is MODELLED on synthetic cases.".into(),
                "Nothing here estimates where an interference source was.".into(),
            ],
            verdict,
        },
        mpack,
    ))
}

/// HTML page.
pub fn html(r: &Report) -> String {
    let f = |x: Option<f64>| x.map(|v| format!("{v:.0}")).unwrap_or_else(|| "-".into());
    let p = |x: &Option<[f64; 2]>| {
        x.map(|v| format!("{:.5}, {:.5}", v[0], v[1]))
            .unwrap_or_else(|| "-".into())
    };
    let a = &r.at_casualty;
    let mut intro = vec![
        r.causation.text.clone(),
        r.statement_of_method.to_string(),
        format!("Data source: {}. {}", r.data_source, r.evidence),
    ];
    intro.extend(r.causation.rationale.iter().cloned());
    crate::transport::page(
        &r.title,
        r.evidence,
        &r.verdict,
        &intro,
        &[
            (
                "At the casualty".into(),
                vec!["quantity", "value"],
                vec![
                    vec!["displayed".into(), p(&a.displayed)],
                    vec!["reconstructed".into(), p(&a.reconstructed)],
                    vec!["divergence (m)".into(), f(a.divergence_m)],
                    vec![
                        "divergence 1 sigma (m)".into(),
                        format!("{:.0}", a.divergence_sigma_m),
                    ],
                    vec!["displayed clearance (m)".into(), f(a.displayed_clearance_m)],
                    vec![
                        "reconstructed clearance (m)".into(),
                        f(a.reconstructed_clearance_m),
                    ],
                    vec![
                        "displayed separation (m)".into(),
                        f(a.displayed_separation_m),
                    ],
                    vec![
                        "reconstructed separation (m)".into(),
                        f(a.reconstructed_separation_m),
                    ],
                    vec![
                        "no GNSS fix before (s)".into(),
                        format!("{:.0}", a.no_fix_before_s),
                    ],
                ],
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
                "Radar fixes (independent checks)".into(),
                vec![
                    "UTC",
                    "object",
                    "fix",
                    "sigma (m)",
                    "vs displayed (m)",
                    "vs reconstructed (m)",
                ],
                r.radar_fixes
                    .iter()
                    .map(|x| {
                        vec![
                            x.utc.clone(),
                            x.reference.clone(),
                            p(&Some(x.position)),
                            format!("{:.0}", x.sigma_m),
                            f(x.vs_displayed_m),
                            f(x.vs_reconstructed_m),
                        ]
                    })
                    .collect(),
            ),
            (
                "Uncertainty".into(),
                vec!["statement"],
                r.causation
                    .uncertainty
                    .iter()
                    .map(|u| vec![u.clone()])
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                r.limits.iter().map(|u| vec![u.clone()]).collect(),
            ),
        ],
    )
}

/// The track table as CSV.
pub fn tracks_csv(r: &Report) -> String {
    let mut s = String::from(
        "utc,displayed_lat,displayed_lon,reconstructed_lat,reconstructed_lon,sigma_m,divergence_m,source\n",
    );
    let o = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
    for t in &r.tracks {
        s += &format!(
            "{},{},{},{},{},{},{},{}\n",
            t.utc,
            o(t.displayed.map(|v| v[0])),
            o(t.displayed.map(|v| v[1])),
            o(t.reconstructed.map(|v| v[0])),
            o(t.reconstructed.map(|v| v[1])),
            if t.sigma_m.is_finite() {
                t.sigma_m.to_string()
            } else {
                String::new()
            },
            o(t.divergence_m),
            t.source
        );
    }
    s
}

/// Write the unsigned pack: `report.json`, `report.html`, `tracks.csv`, the request and input
/// files under `inputs/`, and the embedded actor-vs-victim pack under `interference/`.
pub fn write(
    out: &Path,
    r: &Report,
    m: &maritime::EvidencePack,
    q: &Request,
    request_src: &str,
    base: &Path,
) -> Result<(), String> {
    crate::transport::write_pack(out, r, &html(r), "request.toml", request_src)?;
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::write(out.join("tracks.csv"), tracks_csv(r)).map_err(io)?;
    for f in [Some(&q.nmea), q.ais.as_ref(), q.radar.as_ref()]
        .into_iter()
        .flatten()
    {
        let name = Path::new(f)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| f.clone());
        std::fs::write(out.join("inputs").join(name), read(base, f)?).map_err(io)?;
    }
    let dir = out.join("interference");
    std::fs::create_dir_all(&dir).map_err(io)?;
    std::fs::write(
        dir.join(maritime::RESULT_FILE),
        serde_json::to_string_pretty(m).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(io)?;
    Ok(())
}

/// SYNTHETIC casualties with known truth.
pub mod synth {
    use super::KN;
    use crate::forensics::inputs::iso;
    use crate::transport::offset_deg;

    /// The cases.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Case {
        /// Spoofing drags the displayed position 800 m north; the true track meets the shoal.
        GroundingSpoofed,
        /// As above, with radar fixes on a lighthouse that show the error.
        GroundingSpoofedRadar,
        /// Jamming: no GNSS fix for the last ten minutes before the grounding.
        GroundingJammed,
        /// No interference: the ship steams onto the shoal on its displayed track.
        GroundingNavError,
        /// No interference: a crossing ship on a collision course visible on radar.
        CollisionNavError,
    }

    impl Case {
        /// Parse a case name.
        pub fn parse(s: &str) -> Option<Case> {
            Some(match s {
                "grounding-spoofed" => Case::GroundingSpoofed,
                "grounding-spoofed-radar" => Case::GroundingSpoofedRadar,
                "grounding-jammed" => Case::GroundingJammed,
                "grounding-nav-error" => Case::GroundingNavError,
                "collision-nav-error" => Case::CollisionNavError,
                _ => return None,
            })
        }
    }

    /// Start (2026-06-01T08:00:00Z).
    pub const T0: f64 = 1_780_300_800.0;
    /// Casualty time (s after T0).
    pub const TC: usize = 1500;
    /// Interference starts (s after T0).
    pub const EV: f64 = 900.0;
    const START: (f64, f64) = (50.10, -1.20);
    const SOG: f64 = 12.0;

    fn own(s: f64) -> (f64, f64) {
        offset_deg(START.0, START.1, SOG * KN * s, 0.0)
    }

    struct Rng(u64);
    impl Rng {
        fn n(&mut self) -> f64 {
            let mut u = || {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                (self.0 >> 11) as f64 / (1u64 << 53) as f64
            };
            (u() + u() + u() + u() - 2.0) * 1.732
        }
    }

    fn cs(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, b| a ^ b);
        format!("${body}*{x:02X}\n")
    }

    fn hms(t: f64) -> String {
        let s = (t - (t / 86_400.0).floor() * 86_400.0) as i64;
        format!("{:02}{:02}{:02}.00", s / 3600, s / 60 % 60, s % 60)
    }

    fn nll(lat: f64, lon: f64) -> String {
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

    /// A generated case: file name to contents, and the request TOML.
    pub struct Scenario {
        pub files: Vec<(String, String)>,
        pub request: String,
    }

    /// The charted shoal: centred 150 m beyond the true position at the casualty.
    pub fn shoal() -> (f64, f64) {
        offset_deg(START.0, START.1, SOG * KN * TC as f64 + 150.0, 0.0)
    }

    /// The charted lighthouse: 2.5 km north of the shoal.
    pub fn lighthouse() -> (f64, f64) {
        let s = shoal();
        offset_deg(s.0, s.1, 0.0, 2500.0)
    }

    /// Generate a case.
    pub fn make(case: Case) -> Scenario {
        use Case::*;
        let mut r = Rng(0x2545_f491_4f6c_dd1d);
        let prns = [2u32, 5, 7, 9, 13, 15, 18, 21, 30];
        let base = [46.0, 41.0, 38.0, 44.0, 35.0, 48.0, 40.0, 37.0, 43.0];
        let mut nmea = String::new();
        let mut shown: Vec<Option<(f64, f64)>> = vec![];
        for k in 0..=TC {
            let s = k as f64;
            let truth = own(s);
            let t = iso(T0 + s);
            let date = format!("{}{}{}", &t[8..10], &t[5..7], &t[2..4]);
            let ev = s >= EV;
            let (mut pos, mut valid) = (Some(truth), true);
            let (mut sog, mut cog) = (SOG, 90.0);
            let mut cn0: Vec<Option<f64>> = base.iter().map(|b| Some(b + 0.7 * r.n())).collect();
            match case {
                GroundingSpoofed | GroundingSpoofedRadar if ev => {
                    let north = (2.0 * (s - EV)).min(800.0);
                    let vn = if north < 800.0 { 2.0 } else { 0.0 };
                    pos = Some(offset_deg(truth.0, truth.1, 0.0, north));
                    let ve = SOG * KN;
                    sog = (ve * ve + vn * vn).sqrt() / KN;
                    cog = ve.atan2(vn).to_degrees();
                    for c in cn0.iter_mut() {
                        *c = Some(50.0 + 0.3 * r.n());
                    }
                }
                GroundingJammed if ev => {
                    let depth = ((s - EV) / 60.0).min(1.0) * 22.0;
                    for c in cn0.iter_mut() {
                        *c = c.map(|v| v - depth).filter(|v| *v >= 28.0);
                    }
                    if cn0.iter().filter(|c| c.is_some()).count() < 4 {
                        valid = false;
                    }
                }
                _ => {}
            }
            if !valid {
                pos = None;
            }
            shown.push(pos);
            let (la, lo) = pos.unwrap_or(truth);
            let lls = if valid { nll(la, lo) } else { ",,,".into() };
            nmea += &cs(&format!(
                "GPRMC,{},{},{},{:.1},{:.1},{date},,,{}",
                hms(T0 + s),
                if valid { "A" } else { "V" },
                lls,
                if valid { sog } else { 0.0 },
                if valid { cog } else { 0.0 },
                if valid { "A" } else { "N" }
            ));
            let used: Vec<u32> = prns
                .iter()
                .zip(&cn0)
                .filter(|(_, c)| c.is_some())
                .map(|(p, _)| *p)
                .collect();
            nmea += &cs(&format!(
                "GPGGA,{},{},{},{:02},{:.1},12.0,M,47.0,M,,",
                hms(T0 + s),
                lls,
                if valid { 1 } else { 0 },
                used.len(),
                if valid { 0.9 } else { 99.9 }
            ));
            let tot = prns.len().div_ceil(4);
            for (g, ch) in prns.chunks(4).enumerate() {
                let mut b = format!("GPGSV,{tot},{},{:02}", g + 1, prns.len());
                for (j, p) in ch.iter().enumerate() {
                    let i = g * 4 + j;
                    let snr = cn0[i]
                        .map(|v| format!("{:.0}", v.round()))
                        .unwrap_or_default();
                    b += &format!(",{p:02},{:02},{:03},{snr}", 20 + 7 * i, (40 * i) % 360);
                }
                nmea += &cs(&b);
            }
            nmea += &cs(&format!("HEHDT,{:.1},T", 90.0 + 0.3 * r.n()));
            nmea += &cs(&format!("VWVHW,90.0,T,,M,{:.1},N,,K", SOG + 0.1 * r.n()));
        }
        let other = |s: f64| {
            // Crossing from the south at 10 kn, meeting the own ship at the casualty.
            let meet = own(TC as f64);
            offset_deg(meet.0, meet.1, 0.0, -10.0 * KN * (TC as f64 - s))
        };
        let mut ais = String::from("timestamp,vessel_id,lat,lon,sog_kn\n");
        for k in (0..=TC).step_by(10) {
            if let Some((la, lo)) = shown[k] {
                ais += &format!("{},SUBJ,{la:.6},{lo:.6},{SOG:.1}\n", iso(T0 + k as f64));
            }
            if case == CollisionNavError {
                let (la, lo) = other(k as f64);
                ais += &format!("{},OTHER,{la:.6},{lo:.6},10.0\n", iso(T0 + k as f64));
            }
        }
        let mut radar = String::from("timestamp,target,range_m,bearing_deg\n");
        let lh = lighthouse();
        for k in (0..=TC).step_by(30) {
            let s = k as f64;
            let tr = own(s);
            let mut obs = |name: &str, p: (f64, f64)| {
                let (e, n) = super::en_m(tr.0, tr.1, p.0, p.1);
                let rng = (e * e + n * n).sqrt() + 10.0 * r.n();
                let brg = (e.atan2(n).to_degrees() + 0.3 * r.n()).rem_euclid(360.0);
                radar += &format!("{},{name},{rng:.0},{brg:.1}\n", iso(T0 + s));
            };
            obs("LT-A", lh);
            if case == CollisionNavError {
                obs("OTHER", other(s));
            }
        }
        let tc = iso(T0 + TC as f64);
        let sh = shoal();
        let mut files = vec![("vdr.nmea".to_string(), nmea), ("ais.csv".to_string(), ais)];
        let with_radar = matches!(
            case,
            GroundingSpoofedRadar | GroundingNavError | CollisionNavError | GroundingJammed
        );
        if with_radar {
            files.push(("radar.csv".into(), radar));
        }
        let mut req = format!(
            "title = \"SYNTHETIC casualty: {case:?}\"\nvessel_id = \"SUBJ\"\ncasualty_utc = \"{tc}\"\nnmea = \"vdr.nmea\"\nais = \"ais.csv\"\ndata_source = \"SYNTHETIC (kshana-pro casualty synth)\"\n"
        );
        if with_radar {
            req += "radar = \"radar.csv\"\n";
        }
        if case == CollisionNavError {
            req += "casualty = \"collision\"\nother_vessel = \"OTHER\"\n";
        } else {
            req += &format!(
                "casualty = \"grounding\"\n[hazard]\nname = \"SYNTHETIC shoal\"\nlat = {:.7}\nlon = {:.7}\nradius_m = 150.0\n",
                sh.0, sh.1
            );
        }
        req += &format!(
            "[[reference]]\nname = \"LT-A\"\nlat = {:.7}\nlon = {:.7}\n",
            lh.0, lh.1
        );
        Scenario {
            files,
            request: req,
        }
    }
}
