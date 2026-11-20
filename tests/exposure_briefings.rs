// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Exposure, route risk and briefings (research backlog E09: B0901 to B0905), on small
//! synthetic fixtures built here.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana::interference_map::grid::{CellId, Grid};
use kshana_pro::briefings::{
    self, aviation, brief, layers::synthetic_layer, regional, road, spillover, transit,
};
use kshana_pro::license::{self, Entitlement, Tier};

const SEED: [u8; 32] = [7u8; 32];
const G: Grid = Grid { cell_deg: 0.5 };

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-e09-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn w(dir: &Path, name: &str, text: &str) {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn c(lat: f64, lon: f64) -> CellId {
    G.cell_of(lat, lon)
}

/// Every cell of a box with one status, with exceptions.
fn box_cells(
    s: f64,
    wl: f64,
    n: f64,
    e: f64,
    status: &'static str,
    except: &[(CellId, &'static str)],
) -> Vec<(CellId, &'static str)> {
    let (a, b) = (G.cell_of(s, wl), G.cell_of(n - 1e-9, e - 1e-9));
    let mut v = vec![];
    for i in a.i..=b.i {
        for j in a.j..=b.j {
            let id = CellId { i, j };
            let st = except
                .iter()
                .find(|x| x.0 == id)
                .map(|x| x.1)
                .unwrap_or(status);
            v.push((id, st));
        }
    }
    v
}

fn layer(dir: &Path, name: &str, date: &str, kind: &str, cells: &[(CellId, &str)]) {
    w(
        dir,
        name,
        &synthetic_layer(date, kind, 0.5, cells).to_string(),
    );
}

#[allow(clippy::too_many_arguments)]
fn event(
    id: &str,
    kind: &str,
    lat: f64,
    lon: f64,
    r_m: f64,
    a: &str,
    b: &str,
    conf: f64,
    corr: bool,
) -> serde_json::Value {
    serde_json::json!({
        "format": "kshana-pnt-interference-event/1", "id": id, "start_utc": a, "end_utc": b,
        "systems": [{"system": "gnss", "band": "L1"}], "kind": kind,
        "location": {"lat_deg": lat, "lon_deg": lon, "radius_m": r_m},
        "observers": if corr { vec!["obs-a", "obs-b"] } else { vec!["obs-a"] },
        "confidence": conf, "basis": "synthetic", "corroborated": corr
    })
}

fn iso(t: f64) -> String {
    briefings::iso(t)
}

fn t0() -> f64 {
    briefings::utc("t", "2026-06-01T02:00:00Z").unwrap()
}

/// An eastbound AIS track along 26.25 N through the Hormuz box, 10-minute reports.
fn hormuz_track(id: &str, start: f64) -> String {
    let mut s = String::new();
    for k in 0..=54 {
        s.push_str(&format!(
            "{},{id},26.25,{:.4},15\n",
            iso(start + k as f64 * 600.0),
            55.0 + k as f64 * 0.052
        ));
    }
    s
}

fn hormuz_fixture(d: &Path) {
    let mut ais = String::from("timestamp,vessel_id,lat,lon,sog_kn\n");
    ais.push_str(&hormuz_track("244000001", t0()));
    // A second transit of the same vessel after a 12 h gap, back west.
    for k in 0..=6 {
        ais.push_str(&format!(
            "{},244000001,26.25,{:.3},15\n",
            iso(t0() + 54.0 * 600.0 + 12.0 * 3600.0 + k as f64 * 600.0),
            57.8 - k as f64 * 0.05
        ));
    }
    // A vessel in the Dover Strait, where no layer and no event falls.
    for k in 0..6 {
        ais.push_str(&format!(
            "{},235000002,51.0,{:.3},12\n",
            iso(t0() + k as f64 * 600.0),
            1.0 + k as f64 * 0.04
        ));
    }
    w(d, "ais.csv", &ais);
    let deg = c(26.25, 56.2);
    for (k, day) in ["2026-06-01", "2026-06-02", "2026-06-03"]
        .iter()
        .enumerate()
    {
        let mut cells = box_cells(25.5, 55.5, 27.0, 57.5, "not_anomalous", &[]);
        if k < 2 {
            for x in cells.iter_mut() {
                if x.0 == deg {
                    x.1 = "anomalous";
                }
            }
        }
        cells.extend(box_cells(50.5, 0.5, 51.5, 2.0, "not_anomalous", &[]));
        layer(d, &format!("layers/ais-{day}.geojson"), day, "ais", &cells);
    }
    let ev = serde_json::json!([
        event(
            "EV-1",
            "spoofing",
            26.3,
            57.0,
            20_000.0,
            "2026-06-01T00:00:00Z",
            "2026-06-01T23:00:00Z",
            0.8,
            true
        ),
        event(
            "EV-OLD",
            "jamming",
            12.6,
            43.3,
            10_000.0,
            "2026-01-01T00:00:00Z",
            "2026-01-01T02:00:00Z",
            0.5,
            false
        )
    ]);
    w(d, "events.json", &ev.to_string());
    w(
        d,
        "areas.toml",
        "[[area]]\nid = \"listed-test\"\nname = \"Synthetic listed area\"\nkind = \"listed\"\nsource = \"Synthetic listed area for tests\"\nbbox = [25.5, 56.0, 27.0, 58.0]\n",
    );
}

#[test]
fn transits_give_records_listed_area_records_and_a_fleet_view() {
    let d = tmp("transits");
    hormuz_fixture(&d);
    w(
        &d,
        "transits.toml",
        "title = \"Fleet Q2\"\ntracks = [\"ais.csv\"]\nevents = [\"events.json\"]\nlayers = [\"layers\"]\narea_file = \"areas.toml\"\n[names]\n\"244000001\" = \"MV Example\"\n",
    );
    let (q, src) = transit::TransitRequest::from_file(&d.join("transits.toml")).unwrap();
    let h = transit::build(&q).unwrap();
    assert_eq!(
        h.transits.len(),
        3,
        "two transits of one vessel, one of another"
    );
    let t = h
        .transits
        .iter()
        .find(|t| t.vessel_id == "244000001" && t.reports == 55)
        .unwrap();
    assert_eq!(t.vessel_name, "MV Example");
    assert!(t.areas.iter().any(|a| a.area_id == "hormuz"));
    assert!(t.areas.iter().any(|a| a.area_id == "listed-test"));
    assert!(t.degraded_minutes > 0.0 && t.event_minutes > 0.0);
    assert!(t.exposure_minutes > 0.0 && t.exposure_minutes < t.minutes);
    assert_eq!(t.events.len(), 1);
    assert_eq!(t.events[0].event_id, "EV-1");
    assert!(t.record_id.starts_with("T-") && t.record_id.len() == 18);
    let dover = h
        .transits
        .iter()
        .find(|t| t.vessel_id == "235000002")
        .unwrap();
    assert_eq!(dover.exposure_minutes, 0.0);
    assert!(dover.areas.iter().any(|a| a.area_id == "dover-strait"));
    // No layer for Dover on that day is wrong: there is one, observed, not degraded.
    assert_eq!(dover.not_observed_minutes, 0.0);
    // Listed-area records name the user's source.
    assert!(!h.listed_area_records.is_empty());
    assert!(h
        .listed_area_records
        .iter()
        .all(|l| l.area_source == "Synthetic listed area for tests"));
    assert_eq!(h.fleet.len(), 2);
    let v = h.fleet.iter().find(|v| v.vessel_id == "244000001").unwrap();
    assert_eq!(v.transits, 2);
    assert!(v.top_areas.contains(&"hormuz".to_string()));
    // Deterministic record ids.
    let h2 = transit::build(&q).unwrap();
    assert_eq!(h, h2);
    let out = d.join("out");
    transit::write_dir(&out, &h, &src).unwrap();
    for f in [
        "transit-history.json",
        "transits.csv",
        "listed-areas.csv",
        "fleet.html",
    ] {
        assert!(out.join(f).is_file(), "{f}");
    }
    let page = std::fs::read_to_string(out.join("fleet.html")).unwrap();
    assert!(page.contains("Decision support only") && page.contains("MODELLED"));
}

#[test]
fn briefing_for_three_chokepoints_states_confidence_window_and_provenance() {
    let d = tmp("brief");
    hormuz_fixture(&d);
    w(
        &d,
        "brief.toml",
        "title = \"Gulf and Red Sea passage\"\nareas = [\"hormuz\", \"bab-el-mandeb\", \"dover-strait\"]\nwindow_from_utc = \"2026-06-01T00:00:00Z\"\nwindow_to_utc = \"2026-06-04T00:00:00Z\"\nevents = [\"events.json\"]\nlayers = [\"layers\"]\nroute = [[26.25, 55.6], [26.25, 57.4]]\nroute_name = \"Hormuz eastbound\"\n",
    );
    let (q, src) = brief::BriefRequest::from_file(&d.join("brief.toml")).unwrap();
    let b = brief::build(&q).unwrap();
    assert_eq!(b.areas.len(), 3);
    let by = |id: &str| b.areas.iter().find(|a| a.area_id == id).unwrap();
    let h = by("hormuz");
    assert_eq!((h.events, h.events_corroborated), (1, 1));
    assert_eq!((h.degraded_days, h.observed_days, h.layer_days), (2, 3, 3));
    assert_eq!(h.persistent_cells, 1);
    assert_eq!(h.confidence.level, "high", "{:?}", h.confidence);
    // Nothing observed, nothing reported: insufficient, never "clear".
    let bm = by("bab-el-mandeb");
    assert_eq!(bm.confidence.level, "insufficient");
    assert!(bm.summary.contains("nothing can be said"));
    assert_eq!(by("dover-strait").degraded_days, 0);
    assert_eq!(by("dover-strait").confidence.level, "high");
    let r = b.route.as_ref().unwrap();
    assert_eq!(r.days.len(), 3);
    assert_eq!(r.days_with_degraded, 2);
    assert!(r.contributing_events.contains(&"EV-1".to_string()));
    assert_eq!(b.window.days_in_window, 3);
    assert_eq!(b.window.layer_days.len(), 3);
    // The old event is outside the window.
    assert_eq!(b.window.events_in_window, 1);
    // Provenance: every input with its hash, and the layer licence line.
    assert!(b
        .inputs
        .iter()
        .any(|i| i.role == "layer" && i.sha256.len() == 64));
    assert!(b.inputs.iter().any(|i| i.role == "events"));
    assert!(b.data_licences.iter().any(|l| l.contains("SYNTHETIC")));
    assert!(b.statement.contains("not an operational instruction"));
    let out = d.join("out");
    brief::write_dir(&out, &b, &src).unwrap();
    let pdf = std::fs::read(out.join("briefing.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let html = std::fs::read_to_string(out.join("briefing.html")).unwrap();
    for s in [
        "Strait of Hormuz",
        "Bab el-Mandeb",
        "Dover Strait",
        "sha256",
        "Data window",
    ] {
        assert!(html.contains(s), "{s}");
    }
    for bad in ["compliant", "certified", "conform"] {
        assert!(!html.to_lowercase().contains(bad), "{bad}");
    }
}

fn adsb(rows: &[(f64, &str, f64, f64, f64, u8)]) -> String {
    let mut s = String::from("timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp\n");
    for (t, id, la, lo, alt, nic) in rows {
        s.push_str(&format!("{},{id},{la},{lo},{alt},{nic},9\n", iso(*t)));
    }
    s
}

#[test]
fn tail_ledger_flags_the_exposed_tail_by_altitude_band() {
    let d = tmp("tails");
    let deg = c(69.75, 29.75);
    layer(
        &d,
        "l/adsb-2026-06-01.geojson",
        "2026-06-01",
        "adsb",
        &[(deg, "degraded"), (c(69.75, 30.25), "not_degraded")],
    );
    let mut rows = vec![];
    for k in 0..40 {
        // A HEMS-type flight at 1,500 ft inside the degraded cell, own NIC low on half.
        rows.push((
            t0() + k as f64 * 60.0,
            "4ca123",
            69.6,
            29.6 + k as f64 * 0.002,
            1500.0,
            if k % 2 == 0 { 3 } else { 8 },
        ));
        // A flight at 35,000 ft far away.
        rows.push((
            t0() + k as f64 * 60.0,
            "4ca999",
            60.0,
            10.0 + k as f64 * 0.01,
            35_000.0,
            8,
        ));
    }
    w(&d, "adsb.csv", &adsb(&rows));
    w(
        &d,
        "tails.toml",
        "title = \"Fleet 90 days\"\ntracks = [\"adsb.csv\"]\nlayers = [\"l\"]\nperiod_end_utc = \"2026-06-30T00:00:00Z\"\n[aliases]\n\"4ca123\" = \"OH-HMS\"\n",
    );
    let (q, src) = aviation::LedgerRequest::from_file(&d.join("tails.toml")).unwrap();
    let l = aviation::tail_ledger(&q).unwrap();
    assert_eq!(l.period_days, 90.0);
    assert_eq!(l.period_start_utc, "2026-04-01T00:00:00Z");
    let a = l.entries.iter().find(|e| e.tail == "OH-HMS").unwrap();
    assert_eq!(a.ids, vec!["4ca123".to_string()]);
    assert_eq!(a.flights, 1);
    assert!((a.exposure_minutes - 39.0).abs() < 1e-6, "{a:?}");
    assert!((a.exposure_minutes_by_band["low"] - 39.0).abs() < 1e-6);
    assert_eq!(a.exposure_minutes_by_band["high"], 0.0);
    assert!(a.own_nic_degraded_minutes >= 19.0);
    assert_eq!(a.flag, "review");
    let b = l.entries.iter().find(|e| e.tail == "4ca999").unwrap();
    assert_eq!((b.exposure_minutes, b.flag.as_str()), (0.0, "none"));
    let out = d.join("out");
    aviation::write_ledger(&out, &l, &src).unwrap();
    assert!(std::fs::read_to_string(out.join("tail-ledger.csv"))
        .unwrap()
        .contains("OH-HMS"));
}

#[test]
fn low_altitude_map_publishes_only_cells_with_enough_aircraft_and_no_ids() {
    let d = tmp("lowalt");
    let mut rows = vec![];
    for a in 0..6 {
        for k in 0..5 {
            rows.push((
                t0() + k as f64 * 30.0,
                ["a1", "a2", "a3", "a4", "a5", "a6"][a],
                69.6,
                29.6,
                400.0,
                2,
            ));
        }
    }
    for a in 0..3 {
        rows.push((t0(), ["b1", "b2", "b3"][a], 70.6, 25.6, 500.0, 2));
    }
    w(&d, "adsb.csv", &adsb(&rows));
    w(
        &d,
        "lowalt.toml",
        "title = \"Varanger low level\"\ntracks = [\"adsb.csv\"]\n",
    );
    let (q, _) = aviation::LowAltRequest::from_file(&d.join("lowalt.toml")).unwrap();
    let m = aviation::low_altitude_map(&q).unwrap();
    let f = m["features"].as_array().unwrap();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0]["properties"]["band"], "below");
    assert_eq!(f[0]["properties"]["status"], "degraded");
    let text = m.to_string();
    for id in ["\"a1\"", "\"b1\"", "a1,", "b2"] {
        assert!(!text.contains(id), "{id}");
    }
    let mut fine = q.clone();
    fine.cell_deg = 0.25;
    assert!(aviation::low_altitude_map(&fine).is_err());
}

#[test]
fn occ_alerts_state_level_confidence_and_latency() {
    let d = tmp("occ");
    let deg = c(59.25, 24.75);
    let mut cells = box_cells(58.5, 23.5, 60.0, 26.0, "not_degraded", &[(deg, "degraded")]);
    cells.sort();
    layer(
        &d,
        "l/adsb-2026-06-01.geojson",
        "2026-06-01",
        "adsb",
        &cells,
    );
    // A layer of a day that has not ended at as_of is never used.
    layer(
        &d,
        "l/adsb-2026-06-02.geojson",
        "2026-06-02",
        "adsb",
        &cells,
    );
    w(
        &d,
        "occ.toml",
        "title = \"OCC morning\"\nas_of_utc = \"2026-06-02T12:00:00Z\"\nlookback_days = 1.0\nlayers = [\"l\"]\n\
         [[flights]]\nid = \"AB101\"\ntail = \"ES-ABC\"\ndeparture_utc = \"2026-06-02T14:00:00Z\"\nwaypoints = [[59.25, 24.0, 30000.0], [59.25, 25.5, 30000.0]]\n\
         [[flights]]\nid = \"AB202\"\ndeparture_utc = \"2026-06-02T15:00:00Z\"\nwaypoints = [[58.6, 23.6, 30000.0], [58.6, 23.9, 30000.0]]\n\
         [[flights]]\nid = \"HEMS1\"\noperation = \"hems\"\ndeparture_utc = \"2026-06-02T16:00:00Z\"\nwaypoints = [[58.6, 23.6, 800.0], [58.9, 23.9, 800.0]]\n",
    );
    let (q, src) = aviation::OccRequest::from_file(&d.join("occ.toml")).unwrap();
    let a = aviation::occ(&q).unwrap();
    assert_eq!(a.alerts[0].flight_id, "AB101");
    assert_eq!(a.alerts[0].level, "advisory");
    assert_eq!(a.alerts[0].layer_days, 1);
    assert_eq!(a.alerts[0].data_latency_h, Some(12.0));
    let b = a.alerts.iter().find(|x| x.flight_id == "AB202").unwrap();
    assert_eq!(b.level, "information");
    let hems = a.alerts.iter().find(|x| x.flight_id == "HEMS1").unwrap();
    assert!(hems.low_altitude_km > 0.0);
    assert!(hems
        .confidence
        .reasons
        .iter()
        .any(|r| r.contains("low-altitude")));
    assert_ne!(hems.confidence.level, b.confidence.level);
    assert!(a.latency_note.contains("24 h"));
    let out = d.join("out");
    aviation::write_occ(&out, &a, &src).unwrap();
    assert!(std::fs::read(out.join("flight-briefing.pdf"))
        .unwrap()
        .starts_with(b"%PDF"));
}

#[test]
fn regional_dataset_rebuilds_byte_identically_and_separates_patterns() {
    let d = tmp("regional");
    // Korea west: two separate degraded clusters; one persists.
    let a = c(37.25, 124.75);
    let b = c(38.25, 126.25);
    for (k, day) in ["2026-06-01", "2026-06-02", "2026-06-03", "2026-06-04"]
        .iter()
        .enumerate()
    {
        let mut ex = vec![(a, "degraded")];
        if k == 1 {
            ex.push((b, "degraded"));
        }
        let cells = box_cells(37.0, 124.5, 38.5, 126.5, "not_degraded", &ex);
        layer(&d, &format!("l/adsb-{day}.geojson"), day, "adsb", &cells);
    }
    w(
        &d,
        "regional.toml",
        "dataset = \"korea-west-exposure\"\nversion = \"2026.06-r1\"\nregions = [\"korea-west\", \"delhi-tma\"]\nlayers = [\"l\"]\nsource_kind = \"adsb\"\n\
         [[airports]]\nid = \"RKSI\"\nname = \"Incheon\"\nlat = 37.46\nlon = 126.44\nradius_km = 100.0\n",
    );
    let (q, src) = regional::RegionalRequest::from_file(&d.join("regional.toml")).unwrap();
    let ds = regional::build(&q).unwrap();
    assert!(ds.evidence.starts_with("MODELLED"));
    assert_eq!(ds.dates.len(), 4);
    let kw = &ds.regions[0];
    assert_eq!(kw.patterns.len(), 2, "{:?}", kw.patterns);
    assert_eq!(kw.patterns[0].active_days, 4);
    assert_eq!(kw.class_counts["persistent"], 1);
    assert_eq!(kw.class_counts["recurrent"], 1);
    let cell = kw
        .cells
        .iter()
        .find(|x| x.cell_i == a.i && x.cell_j == a.j)
        .unwrap();
    assert_eq!(cell.states, vec!["degraded"; 4]);
    // Delhi: no layer cells: all not observed, no pattern.
    assert!(ds.regions[1].patterns.is_empty());
    assert!(ds.regions[1].cells.iter().all(|c| c.observed_days == 0));
    assert_eq!(ds.airports[0].patterns, vec!["korea-west/P002".to_string()]);
    let (o1, o2) = (d.join("o1"), d.join("o2"));
    regional::write_dir(&o1, &ds, &src).unwrap();
    regional::write_dir(&o2, &regional::build(&q).unwrap(), &src).unwrap();
    for f in [
        "dataset.json",
        "cells.csv",
        "METHOD.txt",
        "MANIFEST.sha256",
        "request.toml",
    ] {
        assert_eq!(
            std::fs::read(o1.join(f)).unwrap(),
            std::fs::read(o2.join(f)).unwrap(),
            "{f}"
        );
    }
    let mut mixed = q.clone();
    mixed.source_kind = "ais".into();
    assert!(regional::build(&mixed).is_err());
}

#[test]
fn road_index_enforces_the_privacy_rule_and_is_reproducible() {
    let d = tmp("road");
    let mut s = String::from("timestamp,device_id,lat,lon\n");
    for dev in 0..6 {
        for k in 0..3 {
            s.push_str(&format!(
                "{},TRK-SECRET-{dev},32.01,{:.3}\n",
                iso(t0() + (dev * 3 + k) as f64 * 3600.0),
                -109.95 + 0.01 * k as f64
            ));
        }
    }
    for dev in 0..2 {
        s.push_str(&format!("{},LONE-{dev},32.0,-109.3\n", iso(t0())));
    }
    s.push_str("bad,x,1,1\n");
    w(&d, "det.csv", &s);
    w(
        &d,
        "road.toml",
        "title = \"I-10 lane\"\ndetections = [\"det.csv\"]\nperiod_from_utc = \"2026-06-01T00:00:00Z\"\nperiod_to_utc = \"2026-06-08T00:00:00Z\"\n\
         [[corridors]]\nid = \"i10\"\nname = \"Synthetic corridor\"\nwaypoints = [[32.0, -110.0], [32.0, -109.0]]\n",
    );
    let (q, src) = road::RoadRequest::from_file(&d.join("road.toml")).unwrap();
    let (ix, heat) = road::build(&q).unwrap();
    assert_eq!(ix.detections_refused, 1);
    let s1 = &ix.segments[0];
    assert_eq!(s1.status, "published");
    assert_eq!(s1.detections, Some(18));
    assert!(s1.index.unwrap() > 0.0 && s1.index.unwrap() < 100.0);
    let lone = ix
        .segments
        .iter()
        .find(|x| x.from_km <= 65.0 && x.to_km >= 65.0)
        .unwrap();
    assert_eq!(lone.status, "withheld");
    assert_eq!(lone.detections, None);
    let all = format!("{}{}", serde_json::to_string(&ix).unwrap(), heat);
    for id in ["TRK-SECRET", "LONE-"] {
        assert!(!all.contains(id), "{id} leaked");
    }
    assert_eq!(heat["features"].as_array().unwrap().len(), 1);
    assert!(ix.privacy_rule.contains("never written"));
    let out = d.join("o");
    road::write_dir(&out, &ix, &heat, &src).unwrap();
    let (ix2, heat2) = road::build(&q).unwrap();
    assert_eq!((ix, heat), (ix2, heat2));
    let mut weak = q.clone();
    weak.min_distinct = 2;
    assert!(road::build(&weak).is_err());
    let mut fine = q.clone();
    fine.cell_deg = 0.1;
    assert!(road::build(&fine).is_err());
}

const NOTAMS: &str = "A0123/26 NOTAMN\n\
Q) ENOR/QWELW/IV/BO/W/000/400/6950N03000E080\n\
A) ENOR B) 2606010800 C) 2606031600 EST\n\
D) DAILY 0800-1600\n\
E) GNSS JAMMING EXERCISE. GPS SIGNALS MAY BE UNRELIABLE WI AREA 6930N 02900E - 7010N 02900E - 7010N 03100E - 6930N 03100E - 6930N 02900E.\n\
F) SFC G) FL400\n\
\n\
A0124/26 NOTAMN\n\
Q) EVRR/QWELW/IV/BO/W/000/200/5700N02400E030\n\
A) EVRR B) 2606100000 C) PERM\n\
E) GPS INTERFERENCE TESTING.\n";

#[test]
fn notams_become_geometry_with_harmonised_times() {
    let d = spillover::parse_notams(NOTAMS);
    assert!(d.refused.is_empty(), "{:?}", d.refused);
    assert_eq!(d.notams.len(), 2);
    let a = &d.notams[0];
    assert_eq!(a.id, "A0123/26");
    assert_eq!(a.kind, "gnss-interference");
    assert_eq!(a.geometry.kind, "polygon");
    assert_eq!(a.geometry.source, "E) item");
    assert_eq!(a.geometry.polygon.len(), 4);
    assert_eq!(a.start_utc, "2026-06-01T08:00:00Z");
    assert!(a.end_estimated && !a.permanent);
    assert_eq!(a.active_windows.len(), 3);
    assert_eq!(
        a.active_windows[2],
        [
            "2026-06-03T08:00:00Z".to_string(),
            "2026-06-03T16:00:00Z".to_string()
        ]
    );
    assert_eq!((a.lower_ft, a.upper_ft), (Some(0.0), Some(40_000.0)));
    let b = &d.notams[1];
    assert!(b.permanent && b.end_utc.is_none());
    assert_eq!(b.geometry.source, "Q) line");
    assert_eq!(b.geometry.kind, "circle");
    assert!((b.geometry.radius_km.unwrap() - 30.0 * 1.852).abs() < 1e-6);
    let g = spillover::notams_geojson(&d);
    assert_eq!(g["features"].as_array().unwrap().len(), 2);
    let bad = spillover::parse_notams("X1/26 NOTAMN\nQ) X/Y\nE) NO AREA\n");
    assert_eq!(bad.notams.len(), 0);
    assert_eq!(bad.refused.len(), 1);
}

#[test]
fn spillover_footprint_grows_with_altitude_lists_users_and_reports_agreement() {
    let d = tmp("spill");
    w(&d, "range.notam", NOTAMS.split("\n\n").next().unwrap());
    let day0 = briefings::utc("t", "2026-06-01T09:00:00Z").unwrap();
    let mut rows = vec![];
    for k in 0..10 {
        // In the window at cruise, about 200 km south of the area.
        rows.push((day0 + k as f64 * 60.0, "47a001", 67.8, 30.0, 35_000.0, 8));
        // Outside the window (night), same place.
        rows.push((
            day0 + 14.0 * 3600.0 + k as f64 * 60.0,
            "47a002",
            67.8,
            30.0,
            35_000.0,
            8,
        ));
        // Low and far: outside the low-altitude footprint.
        rows.push((day0 + k as f64 * 60.0, "47a003", 67.8, 30.0, 300.0, 8));
    }
    w(&d, "adsb.csv", &adsb(&rows));
    let inside = c(69.75, 30.25);
    let near = c(68.75, 30.25);
    let far = c(66.25, 20.25);
    layer(
        &d,
        "l/adsb-2026-06-01.geojson",
        "2026-06-01",
        "adsb",
        &[
            (inside, "degraded"),
            (near, "degraded"),
            (far, "degraded"),
            (c(69.25, 30.25), "not_degraded"),
        ],
    );
    w(
        &d,
        "spill.toml",
        "title = \"Range week\"\nnotams = [\"range.notam\"]\ntracks_adsb = [\"adsb.csv\"]\nlayers = [\"l\"]\n",
    );
    let (q, src) = spillover::FootprintRequest::from_file(&d.join("spill.toml")).unwrap();
    let (doc, geo) = spillover::footprint(&q).unwrap();
    let a = &doc.areas[0];
    let low = &a.altitudes[0];
    let high = a.altitudes.last().unwrap();
    assert_eq!(low.altitude_ft, 0.0);
    assert!(high.outer_range_km > low.outer_range_km);
    assert!(high.inner_cells + high.outer_cells > low.inner_cells + low.outer_cells);
    let ids: Vec<&str> = a.affected_users.iter().map(|u| u.id.as_str()).collect();
    assert_eq!(ids, vec!["47a001"], "{:?}", a.affected_users);
    assert_eq!(a.agreement.len(), 1);
    let ag = &a.agreement[0];
    assert_eq!(ag.degraded_inside, 2);
    assert_eq!(ag.footprint_observed_not_degraded, 1);
    assert!(ag.footprint_not_observed > 0);
    // Coarse outputs only: every footprint cell is 0.5 degree; nothing estimates a source.
    let text = geo.to_string() + &serde_json::to_string(&doc).unwrap();
    assert!(!text.contains("source_estimate") && !text.contains("emitter_lat"));
    for f in geo["features"].as_array().unwrap() {
        let ring = &f["geometry"]["coordinates"][0];
        let wdt = ring[1][0].as_f64().unwrap() - ring[0][0].as_f64().unwrap();
        assert!((wdt - 0.5).abs() < 1e-9);
    }
    let out = d.join("o");
    spillover::write_dir(&out, &doc, &geo, &src).unwrap();
    let mut fine = q.clone();
    fine.cell_deg = 0.25;
    assert!(spillover::footprint(&fine).is_err());
}

#[test]
fn features_are_registered_pro() {
    for f in briefings::FEATURES {
        assert_eq!(license::explicit_tier(f), Some(Tier::Pro), "{f}");
        assert!(license::pro_command_features().contains(f), "{f}");
    }
    let pro = Entitlement {
        licensee: "L".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&pro, briefings::FEATURE_ROUTE, 0).is_ok());
}

fn bin(args: &[&str], home: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn cli_brief_is_gated_signed_and_verifies_and_notam_is_free() {
    let d = tmp("cli");
    let home = d.join("home");
    hormuz_fixture(&d);
    w(
        &d,
        "brief.toml",
        "title = \"Chokepoints\"\nareas = [\"hormuz\", \"bab-el-mandeb\", \"dover-strait\"]\nwindow_from_utc = \"2026-06-01T00:00:00Z\"\nwindow_to_utc = \"2026-06-04T00:00:00Z\"\nevents = [\"events.json\"]\nlayers = [\"layers\"]\n",
    );
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let p = |x: &Path| x.to_str().unwrap().to_string();
    let out = d.join("out");
    let base = [
        "briefing".to_string(),
        "brief".into(),
        p(&d.join("brief.toml")),
        "--out".into(),
        p(&out),
        "--signer".into(),
        "Ops".into(),
        "--key-file".into(),
        p(&key),
        "--utc".into(),
        "2026-06-04T06:00:00Z".into(),
    ];
    let a: Vec<&str> = base.iter().map(String::as_str).collect();
    let (ok, o) = bin(&a, &home);
    assert!(!ok && o.contains("requires the 'pro' tier"), "{o}");
    let mut a2 = a.clone();
    a2.push("--dev");
    let (ok, o) = bin(&a2, &home);
    assert!(ok, "{o}");
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, o) = bin(&["briefing", "verify", &p(&out), "--trust", &pk], &home);
    assert!(ok, "{o}");
    w(&d, "n.notam", NOTAMS);
    let (ok, o) = bin(&["briefing", "notam", &p(&d.join("n.notam"))], &home);
    assert!(
        ok && o.contains("A0123/26") && o.contains("harmonised_criteria"),
        "{o}"
    );
    let (ok, o) = bin(&["briefing", "areas"], &home);
    assert!(ok && o.contains("bab-el-mandeb"), "{o}");
}
