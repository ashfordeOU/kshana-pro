// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Converters between ELOF and the engine's own records.
//!
//! | from | to | function |
//! |---|---|---|
//! | [`Scenario`] | header (stations, epoch, scenario SHA-256, receiver, ASF model) | [`header_for_scenario`] |
//! | simulator truth ([`TruthRecord`], or the truth CSV) | `truth` records | [`truth_records`], [`from_truth`] |
//! | `truth` records | [`TruthRecord`] | [`to_truth`] |
//! | receiver [`Measurement`]s | `obs` records | [`obs_from_measurements`] |
//! | `obs` records | [`Measurement`]s (for [`crate::rx::toa_errors`] and the solvers) | [`to_measurements`] |
//! | receiver [`TimeSolution`] | `timing` records | [`timing_from_solution`] |
//! | `timing` records | [`TimeSolution`] (for [`crate::rx::clock_errors`]) | [`to_time_solution`] |
//! | receiver data channel ([`StationLdc`]) | `ldc` records | [`ldc_from_rx`] |
//! | imported logs ([`ObsRecord`]) | `obs` records, and back | [`obs_from_obs_records`], [`to_obs_records`] |
//! | a whole software-receiver run | an ELOF file | [`from_rx_output`] |
//!
//! Round trips are exact for every stored field except `toa_s`, which is rebuilt from the
//! nominal emission plus `toa_ns` (within 1 ps over a day).

use super::*;
use crate::chain::Chain;
use crate::field::importers::{flags, ObsRecord};
use crate::field::unix_s_from_abs;
use crate::ldc::build_modulator;
use crate::rx::{Measurement, RxOutput, StationClock, StationLdc, TimeSolution};
use crate::scenario::Scenario;
use crate::TruthRecord;

/// `Some(v)` for a finite value (JSON has no NaN or infinity: absent means not
/// available, or infinite for a noise-free SNR).
fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

/// Software tag written by these converters.
pub fn software() -> String {
    format!("kshana-eloran {}", env!("CARGO_PKG_VERSION"))
}

/// Station metadata of chains (with each chain's LDC scheme name when given).
pub fn stations_of(chains: &[Chain], ldc: &[Option<String>]) -> Vec<StationMeta> {
    let mut v = Vec::new();
    for (ci, ch) in chains.iter().enumerate() {
        for st in &ch.stations {
            v.push(StationMeta {
                chain: ch.name.clone(),
                gri_us: ch.gri_us,
                station: st.id.clone(),
                role: st.role,
                lat_deg: st.position.lat_deg,
                lon_deg: st.position.lon_deg,
                emission_delay_us: st.emission_delay_us,
                time_ref: st.time_ref.clone(),
                ldc_scheme: ldc.get(ci).cloned().flatten(),
            });
        }
    }
    v
}

/// LDC scheme name per chain of a scenario (`None`: no data channel).
pub fn ldc_schemes(sc: &Scenario) -> Vec<Option<String>> {
    sc.chains
        .iter()
        .map(|c| {
            c.ldc
                .as_ref()
                .and_then(|l| build_modulator(l, Some(&c.chain)).ok().flatten())
                .map(|m| m.c.name.clone())
        })
        .collect()
}

/// Header for observations of a scenario: stations, epoch (scene start), scenario SHA-256,
/// static receiver position, antenna type and ASF model name.
pub fn header_for_scenario(sc: &Scenario, name: &str, source: Source) -> Header {
    let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let mut h = Header::new(
        &sc.scene.start_utc,
        TimeScale::Scene,
        source,
        TimeReference {
            id: "scene".into(),
            kind: TimeRefKind::Scene,
            description: "simulator truth timeline (receiver clock = truth)".into(),
        },
        stations_of(&chains, &ldc_schemes(sc)),
    );
    h.scenarios.push(ScenarioRef {
        sha256: sc.hash(),
        name: name.into(),
        uri: None,
    });
    if let crate::traj::Trajectory::Static {
        lat_deg,
        lon_deg,
        h_m,
        ..
    } = &sc.receiver
    {
        h.receiver.position = Some(Position {
            lat_deg: *lat_deg,
            lon_deg: *lon_deg,
            alt_m: *h_m,
        });
    }
    let loops = sc
        .outputs
        .iter()
        .filter(|o| matches!(o, crate::antenna::OutputChannel::HLoop { .. }))
        .count();
    h.receiver.antenna.kind = match loops {
        0 => AntennaKind::EField,
        1 => AntennaKind::HLoop,
        _ => AntennaKind::CrossedLoop,
    };
    h.asf.model = serde_json::to_value(&sc.propagation)
        .ok()
        .and_then(|v| {
            v.get("ground")
                .and_then(|g| g.get("model"))
                .and_then(|m| m.as_str().map(String::from))
        })
        .unwrap_or_default();
    h.asf.source = "scenario propagation model".into();
    h
}

fn meta<'a>(h: &'a Header, chain: &str, station: &str) -> Result<&'a StationMeta, String> {
    h.station(chain, station)
        .ok_or_else(|| format!("ELOF: {chain}/{station} is not declared in the header"))
}

/// Truth records of a simulator run (with the transmitted LDC symbol per group when the
/// scenario's chain carries a data channel).
pub fn truth_records(
    h: &Header,
    truth: &[TruthRecord],
    sc: Option<&Scenario>,
) -> Result<Vec<Record>, String> {
    let epoch = h.epoch()?;
    let mods: Vec<(String, Chain, Option<crate::ldc::modulator::LdcModulator>)> = sc
        .map(|sc| {
            sc.chains
                .iter()
                .map(|c| {
                    let m = c
                        .ldc
                        .as_ref()
                        .and_then(|l| build_modulator(l, Some(&c.chain)).ok().flatten());
                    (c.chain.name.clone(), c.chain.clone(), m)
                })
                .collect()
        })
        .unwrap_or_default();
    truth
        .iter()
        .map(|r| {
            let m = meta(h, &r.chain, &r.station)?;
            let ldc_symbol = mods
                .iter()
                .find(|x| x.0 == r.chain)
                .and_then(|(_, ch, md)| {
                    let si = ch.stations.iter().position(|s| s.id == r.station)?;
                    md.as_ref()?.symbol(si, ch.stations[si].role, r.group)
                });
            Ok(Record::Truth(Truth {
                t_s: r.toa_s,
                t_emit_s: r.t_emit_s,
                chain: r.chain.clone(),
                station: r.station.clone(),
                group: r.group,
                toa_ns: (r.toa_s - nominal_emission_s(m, r.group, epoch)) * 1e9,
                delay_ns: r.delay_ns,
                geom_ns: r.geom_ns,
                pf_ns: r.pf_ns,
                sf_ns: r.sf_ns,
                asf_ns: r.asf_ns,
                ecd_us: r.ecd_us,
                field_uv_m: r.field_uv_m,
                snr_db: finite(r.snr_db),
                sky_delay_us: r.sky_delay_us,
                sky_ratio_db: r.sky_ratio_db,
                time_ref_offset_ns: r.time_ref_offset_ns,
                emission_error_ns: r.emission_error_ns,
                rx_lat_deg: r.rx_lat_deg,
                rx_lon_deg: r.rx_lon_deg,
                rx_heading_deg: r.rx_heading_deg,
                bearing_deg: r.bearing_deg,
                distance_km: r.distance_km,
                ldc_symbol,
            }))
        })
        .collect()
}

/// A simulated-truth ELOF file of a scenario run.
pub fn from_truth(sc: &Scenario, name: &str, truth: &[TruthRecord]) -> Result<ElofFile, String> {
    let mut h = header_for_scenario(
        sc,
        name,
        Source {
            kind: SourceKind::Simulated,
            software: software(),
            description: "simulator truth".into(),
        },
    );
    h.truth_included = true;
    let records = truth_records(&h, truth, Some(sc))?;
    let mut f = ElofFile { header: h, records };
    f.sort();
    Ok(f)
}

/// `truth` records back to [`TruthRecord`]s.
pub fn to_truth(f: &ElofFile) -> Result<Vec<TruthRecord>, String> {
    let epoch = f.header.epoch()?;
    f.truth()
        .map(|x| {
            let m = meta(&f.header, &x.chain, &x.station)?;
            Ok(TruthRecord {
                t_emit_s: x.t_emit_s,
                chain: x.chain.clone(),
                station: x.station.clone(),
                group: x.group,
                toa_s: nominal_emission_s(m, x.group, epoch) + x.toa_ns * 1e-9,
                delay_ns: x.delay_ns,
                geom_ns: x.geom_ns,
                pf_ns: x.pf_ns,
                sf_ns: x.sf_ns,
                asf_ns: x.asf_ns,
                ecd_us: x.ecd_us,
                field_uv_m: x.field_uv_m,
                snr_db: x.snr_db.unwrap_or(f64::INFINITY),
                sky_delay_us: x.sky_delay_us,
                sky_ratio_db: x.sky_ratio_db,
                time_ref_offset_ns: x.time_ref_offset_ns,
                emission_error_ns: x.emission_error_ns,
                rx_lat_deg: x.rx_lat_deg,
                rx_lon_deg: x.rx_lon_deg,
                rx_heading_deg: x.rx_heading_deg,
                bearing_deg: x.bearing_deg,
                distance_km: x.distance_km,
            })
        })
        .collect()
}

/// Receiver measurements → `obs` records (time tag = the measured arrival).
pub fn obs_from_measurements(h: &Header, meas: &[Measurement]) -> Result<Vec<Record>, String> {
    let epoch = h.epoch()?;
    meas.iter()
        .map(|m| {
            let st = meta(h, &m.chain, &m.station)?;
            let mut flags_ = BTreeSet::new();
            if m.cycle_slip {
                flags_.insert(flags::CYCLE_SLIP.to_string());
            }
            Ok(Record::Obs(Obs {
                t_s: m.toa_s,
                chain: m.chain.clone(),
                station: m.station.clone(),
                group: m.group,
                channel: m.channel,
                toa_ns: (m.toa_s - nominal_emission_s(st, m.group, epoch)) * 1e9,
                ecd_us: finite(m.ecd_us),
                snr_db: finite(m.snr_db),
                field_uv_m: finite(m.field_uv_m),
                phase_code: Some(Parity::of_group(m.group)),
                bearing_deg: m.bearing_deg,
                flags: flags_,
            }))
        })
        .collect()
}

/// `obs` records → receiver [`Measurement`]s against `chains` (records of stations not in
/// `chains` are skipped). Fields ELOF does not carry (`signal_rms`, `pulses`,
/// `samples`) are zero; the envelope ToA is rebuilt from the ECD.
pub fn to_measurements(f: &ElofFile, chains: &[Chain]) -> Result<Vec<Measurement>, String> {
    let epoch = f.header.epoch()?;
    let mut out = Vec::new();
    for o in f.obs() {
        if o.flags.contains(flags::INVALID) {
            continue;
        }
        let Some(ci) = chains.iter().position(|c| c.name == o.chain) else {
            continue;
        };
        let Some(si) = chains[ci].stations.iter().position(|s| s.id == o.station) else {
            continue;
        };
        let m = meta(&f.header, &o.chain, &o.station)?;
        let toa_s = nominal_emission_s(m, o.group, epoch) + o.toa_ns * 1e-9;
        let ecd = o.ecd_us.unwrap_or(0.0);
        out.push(Measurement {
            chain_index: ci,
            station_index: si,
            chain: o.chain.clone(),
            station: o.station.clone(),
            channel: o.channel,
            group: o.group,
            toa_s,
            envelope_toa_s: toa_s + ecd * 1e-6,
            ecd_us: ecd,
            snr_db: o.snr_db.unwrap_or(f64::NAN),
            signal_rms: 0.0,
            field_uv_m: o.field_uv_m.unwrap_or(f64::NAN),
            cycle_slip: o.flags.contains(flags::CYCLE_SLIP),
            bearing_deg: o.bearing_deg,
            pulses: 0,
            samples: 0,
        });
    }
    Ok(out)
}

/// A receiver time solution → `timing` records (one per station, one per time
/// reference), time-tagged `t_s`.
pub fn timing_from_solution(ts: &TimeSolution, t_s: f64) -> Vec<Record> {
    let mut v: Vec<Record> = ts
        .stations
        .iter()
        .map(|s| {
            Record::Timing(Timing {
                t_s,
                time_ref: s.time_ref.clone(),
                chain: Some(s.chain.clone()),
                station: Some(s.station.clone()),
                ref_minus_rx_ns: s.ref_minus_rx_ns,
                std_ns: Some(s.std_ns),
                n: Some(s.n),
            })
        })
        .collect();
    for (id, mean, n) in &ts.by_time_ref {
        v.push(Record::Timing(Timing {
            t_s,
            time_ref: id.clone(),
            chain: None,
            station: None,
            ref_minus_rx_ns: *mean,
            std_ns: None,
            n: Some(*n),
        }));
    }
    v
}

/// `timing` records → a [`TimeSolution`] (per-station records; per-reference records
/// fill `by_time_ref`). When a station has several records the last one is used.
pub fn to_time_solution(f: &ElofFile) -> TimeSolution {
    let mut stations: Vec<StationClock> = Vec::new();
    let mut by: Vec<(String, f64, usize)> = Vec::new();
    for t in f.timing() {
        match (&t.chain, &t.station) {
            (Some(c), Some(s)) => {
                stations.retain(|x| !(x.chain == *c && x.station == *s));
                stations.push(StationClock {
                    chain: c.clone(),
                    station: s.clone(),
                    time_ref: t.time_ref.clone(),
                    ref_minus_rx_ns: t.ref_minus_rx_ns,
                    std_ns: t.std_ns.unwrap_or(0.0),
                    n: t.n.unwrap_or(1),
                });
            }
            _ => {
                by.retain(|x| x.0 != t.time_ref);
                by.push((t.time_ref.clone(), t.ref_minus_rx_ns, t.n.unwrap_or(1)));
            }
        }
    }
    TimeSolution {
        stations,
        by_time_ref: by,
    }
}

/// Decided data-channel symbols → `ldc` records (one per group).
pub fn ldc_from_rx(h: &Header, ldc: &[StationLdc]) -> Result<Vec<Record>, String> {
    let epoch = h.epoch()?;
    let mut v = Vec::new();
    for l in ldc {
        let m = meta(h, &l.chain, &l.station)?;
        for (k, sym) in l.symbols.iter().enumerate() {
            let g = l.first_group + k as i64;
            v.push(Record::Ldc(Ldc {
                t_s: nominal_emission_s(m, g, epoch),
                chain: l.chain.clone(),
                station: l.station.clone(),
                group: g,
                scheme: l.scheme.clone(),
                symbol: *sym,
                missing: false,
            }));
        }
    }
    Ok(v)
}

/// A whole software-receiver run as ELOF: observations, the time solution at the
/// scenario's receiver position (if given) and decoded data-channel symbols; with
/// `truth`, the truth records too.
pub fn from_rx_output(
    sc: &Scenario,
    name: &str,
    out: &RxOutput,
    time_solution: Option<&TimeSolution>,
    truth: Option<&[TruthRecord]>,
) -> Result<ElofFile, String> {
    let mut h = header_for_scenario(
        sc,
        name,
        Source {
            kind: SourceKind::SoftwareReceiver,
            software: software(),
            description: "Kshana software eLoran receiver on simulated samples".into(),
        },
    );
    h.receiver.id = "kshana-software-rx".into();
    h.receiver.make = "Kshana".into();
    h.receiver.model = "software receiver".into();
    h.truth_included = truth.is_some();
    let mut records = obs_from_measurements(&h, &out.measurements)?;
    records.extend(ldc_from_rx(&h, &out.ldc)?);
    if let Some(ts) = time_solution {
        let t_end = out.measurements.iter().map(|m| m.toa_s).fold(0.0, f64::max);
        records.extend(timing_from_solution(ts, t_end));
    }
    if let Some(t) = truth {
        records.extend(truth_records(&h, t, Some(sc))?);
    }
    let mut f = ElofFile { header: h, records };
    f.sort();
    Ok(f)
}

/// Imported receiver-log records → `obs` records. `ObsRecord::toa_us` is relative to the
/// chain master's nominal emission (the importers' convention); the group is recovered
/// from the time tag. Records without time, chain/station or ToA are skipped (counted in
/// the second return value).
pub fn obs_from_obs_records(
    h: &Header,
    recs: &[ObsRecord],
) -> Result<(Vec<Record>, usize), String> {
    let epoch = h.epoch()?;
    let mut out = Vec::new();
    let mut skipped = 0;
    for r in recs {
        let (Some(t_unix), Some(c), Some(s), Some(toa_us)) =
            (r.t_unix_s, &r.chain, &r.station, r.toa_us)
        else {
            skipped += 1;
            continue;
        };
        let Some(m) = h.station(c, s) else {
            skipped += 1;
            continue;
        };
        let t_s = t_unix - unix_s_from_abs(epoch);
        // arrival (s since epoch) − ToA re master emission → master emission; group from it
        let master_emit_s = t_s - toa_us * 1e-6;
        let gri_s = m.gri_us as f64 * 1e-6;
        let loran_s = epoch.0 as f64 * 1e-12 + master_emit_s;
        let group = (loran_s / gri_s).round() as i64;
        out.push(Record::Obs(Obs {
            t_s,
            chain: c.clone(),
            station: s.clone(),
            group,
            channel: None,
            toa_ns: toa_us * 1e3 - m.emission_delay_us * 1e3,
            ecd_us: r.ecd_us,
            snr_db: r.snr_db,
            field_uv_m: None,
            phase_code: None,
            bearing_deg: None,
            flags: r.flags.clone(),
        }));
    }
    Ok((out, skipped))
}

/// `obs` records → importer [`ObsRecord`]s (ToA re the master's nominal emission, so the
/// campaign's external-receiver scoring applies unchanged).
pub fn to_obs_records(f: &ElofFile) -> Result<Vec<ObsRecord>, String> {
    let epoch = f.header.epoch()?;
    let t0 = unix_s_from_abs(epoch);
    f.obs()
        .map(|o| {
            let m = meta(&f.header, &o.chain, &o.station)?;
            Ok(ObsRecord {
                source: format!("elof:{}", f.header.receiver.id),
                t_unix_s: Some(t0 + o.t_s),
                chain: Some(o.chain.clone()),
                station: Some(o.station.clone()),
                toa_us: Some((o.toa_ns + m.emission_delay_us * 1e3) * 1e-3),
                ecd_us: o.ecd_us,
                snr_db: o.snr_db,
                flags: o.flags.clone(),
                ..Default::default()
            })
        })
        .collect()
}
