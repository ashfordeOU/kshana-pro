// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! V2: the incident forensics detection bars of
//! `docs/evidence/v2-forensics-detection/2026-11-15_PREREGISTRATION.md`, scored once on the
//! pre-registered synthetic labelled set (S: 48 receiver cases from the open NMEA synthesiser;
//! T: 40 AIS and ADS-B cases). The public-event part (P) is empty at registration and is not
//! scored, so E1 stays unclaimed whatever this run shows.
//!
//! Written after the pre-registration was committed (e5a3369b). With `KSHANA_PRO_V2_WRITE=1`
//! the run writes `results.json` beside it; otherwise the run must reproduce the committed
//! file exactly (every case's outcome and every bar, misses included).

use std::path::{Path, PathBuf};

use kshana_pro::forensics::detect::{analyse, ForensicReport};
use kshana_pro::forensics::inputs::{iso, parse_utc};
use kshana_pro::forensics::IncidentScenario;
use serde_json::{json, Value};

const SEED: u64 = 20261115;
const RESULTS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/v2-forensics-detection/results.json"
);

/// SplitMix64, as pre-registered.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// U(a, b).
    fn u(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }
    /// UI(a, b), inclusive.
    fn ui(&mut self, a: i64, b: i64) -> i64 {
        a + (self.next() % ((b - a + 1) as u64)) as i64
    }
    /// Standard normal (Box-Muller).
    fn normal(&mut self) -> f64 {
        let u1 = self.u(f64::MIN_POSITIVE, 1.0);
        let u2 = self.u(0.0, 1.0);
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

fn tmp() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("v2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn indicated(r: &ForensicReport, cat: &str) -> bool {
    match cat {
        "jamming" => r.verdict.jamming.status == "indicated",
        _ => r.verdict.spoofing.status == "indicated",
    }
}

/// The onset (s from the stream start) of the earliest finding of `cat`.
fn onset(r: &ForensicReport, cat: &str, t0: f64) -> Option<f64> {
    r.findings
        .iter()
        .filter(|f| f.category == cat || f.category == "jamming-and-spoofing")
        .filter_map(|f| parse_utc(&f.onset.latest_utc))
        .map(|t| t - t0)
        .reduce(f64::min)
}

const START_S: f64 = 1_780_272_000.0; // 2026-06-01T00:00:00Z

/// Section 2.1: one receiver case. Returns its record.
fn receiver_case(i: usize, rng: &mut Rng, dir: &Path) -> Value {
    let class = ["jamming", "spoofing", "clean", "confounder"][i % 4];
    let t0 = START_S + 3.0 * 3600.0 * i as f64;
    let lat = rng.u(50.0, 62.0);
    let lon = rng.u(-5.0, 25.0);
    let kn = rng.u(6.0, 20.0);
    let wlat = lat + rng.u(0.1, 0.3);
    let wlon = lon + rng.u(0.2, 0.8);
    let even = (i / 4) % 2 == 0;
    let (event, start_s, ramp_s, params) = match class {
        "jamming" => {
            let (s, d, r, drop) = (rng.ui(250, 500), rng.ui(240, 600), rng.ui(30, 120), rng.u(20.0, 40.0));
            (
                format!("[[event]]\nkind = \"jamming\"\nlabel = \"V2\"\nstart_s = {s}\nduration_s = {d}\nramp_s = {r}\nrecovery_s = 60\ncn0_drop_db = {drop:.3}\nspread_db = 6\n"),
                s, r,
                json!({"kind": "jamming", "start_s": s, "duration_s": d, "ramp_s": r, "cn0_drop_db": r3(drop)}),
            )
        }
        "spoofing" if even => {
            let (s, d, r) = (rng.ui(250, 500), rng.ui(500, 800), rng.ui(120, 400));
            let (off, brg, cn0) = (rng.u(500.0, 2500.0), rng.u(0.0, 360.0), rng.u(44.0, 48.0));
            (
                format!("[[event]]\nkind = \"drag-off\"\nlabel = \"V2\"\nstart_s = {s}\nduration_s = {d}\nramp_s = {r}\nrecovery_s = 0\nfinal_offset_m = {off:.3}\nbearing_deg = {brg:.3}\ncounterfeit_cn0_dbhz = {cn0:.3}\n"),
                s, r,
                json!({"kind": "drag-off", "start_s": s, "duration_s": d, "ramp_s": r, "final_offset_m": r3(off), "bearing_deg": r3(brg), "counterfeit_cn0_dbhz": r3(cn0)}),
            )
        }
        "spoofing" => {
            let (s, d, r, off) = (rng.ui(250, 500), rng.ui(400, 700), rng.ui(60, 180), rng.u(5.0, 60.0));
            (
                format!("[[event]]\nkind = \"time-spoof\"\nlabel = \"V2\"\nstart_s = {s}\nduration_s = {d}\nramp_s = {r}\nrecovery_s = 0\noffset_s = {off:.3}\n"),
                s, r,
                json!({"kind": "time-spoof", "start_s": s, "duration_s": d, "ramp_s": r, "offset_s": r3(off)}),
            )
        }
        "clean" => (String::new(), 0, 0, json!({"kind": "none"})),
        _ => {
            let (s, d) = (rng.ui(250, 500), rng.ui(300, 660));
            let preset = if even { "fjord" } else { "container-terminal" };
            (
                format!("[[event]]\nkind = \"natural-degradation\"\nlabel = \"V2\"\nstart_s = {s}\nduration_s = {d}\nramp_s = 120\nrecovery_s = 120\nskyline = {{ preset = \"{preset}\" }}\n"),
                s, 120,
                json!({"kind": "natural-degradation", "start_s": s, "duration_s": d, "preset": preset}),
            )
        }
    };
    let scn = format!(
        "[scenario]\nname = \"v2-{i}\"\ndescription = \"V2 labelled case (synthetic)\"\nseed = {}\n\
         start_utc = \"{}\"\nduration_s = 1200\nrate_hz = 1\n\
         [vessel]\nlat_deg = {lat:.5}\nlon_deg = {lon:.5}\nspeed_kn = {kn:.3}\n\
         [[waypoint]]\nlat_deg = {wlat:.5}\nlon_deg = {wlon:.5}\nspeed_kn = {kn:.3}\n\
         [environment]\ncurrent_speed_kn = 0.5\ncurrent_set_deg = 180\nmagnetic_variation_deg = 2\n{event}",
        1000 + i,
        iso(t0)
    );
    let g = kshana::nmea_synth::generate_from_toml(&scn, None).unwrap();
    let case = dir.join(format!("s{i}"));
    std::fs::create_dir_all(&case).unwrap();
    std::fs::write(case.join("log.nmea"), g.nmea_text()).unwrap();
    let inc = format!(
        "kind = \"pro-incident-forensics\"\n[incident]\nid = \"V2-S-{i}\"\ntitle = \"V2 case {i}\"\n\
         start_utc = \"{}\"\nend_utc = \"{}\"\n\
         [[receiver]]\nlabel = \"GNSS\"\npath = \"log.nmea\"\nformat = \"nmea\"\nplatform = \"vessel\"\n",
        iso(t0 + 200.0),
        iso(t0 + 1200.0)
    );
    std::fs::write(case.join("incident.toml"), inc).unwrap();
    let r = analyse(&IncidentScenario::from_file(&case.join("incident.toml")).unwrap(), None).unwrap();
    let _ = std::fs::remove_dir_all(&case);
    let jam = indicated(&r, "jamming");
    let spf = indicated(&r, "spoofing");
    let high = r.findings.iter().any(|f| f.confidence.level == "high");
    let mut rec = json!({
        "case": format!("S{i:02}"), "class": class, "event": params,
        "start": {"lat": r3(lat), "lon": r3(lon), "speed_kn": r3(kn)},
        "jamming_indicated": jam, "spoofing_indicated": spf, "high_confidence": high,
    });
    if class == "jamming" || class == "spoofing" {
        let o = onset(&r, class, t0);
        rec["onset_s"] = json!(o.map(|x| x.round()));
        rec["onset_ok"] = json!(o.is_some_and(|x| x >= start_s as f64 && x <= (start_s + ramp_s) as f64 + 60.0));
    }
    rec
}

/// Section 2.2, AIS: one case. Returns its record.
fn ais_case(j: usize, rng: &mut Rng, dir: &Path) -> Value {
    let t0 = 1_782_864_000.0 + 2.0 * 3600.0 * j as f64; // 2026-07-01T00:00:00Z
    let positive = j < 10;
    let mut rows = vec!["timestamp,vessel_id,lat,lon,sog_kn".to_string()];
    let mut detail = json!({});
    let m_per_deg = 111_320.0;
    for v in 0..6 {
        let (lat0, lon0) = (rng.u(54.0, 58.0), rng.u(3.0, 12.0));
        let (kn, hdg) = (rng.u(8.0, 16.0), rng.u(0.0, 360.0).to_radians());
        let target = positive && v == 0;
        let (jump_at, jump_km, radius_km) = if target && j % 2 == 0 {
            (rng.ui(20, 40), rng.u(2.0, 20.0), 0.0)
        } else if target {
            (0, 0.0, rng.u(0.5, 2.0))
        } else {
            (0, 0.0, 0.0)
        };
        if target {
            detail = if j % 2 == 0 {
                json!({"kind": "jump", "from_report": jump_at, "km": r3(jump_km)})
            } else {
                json!({"kind": "circle", "radius_km": r3(radius_km)})
            };
        }
        for k in 0..60 {
            let t = 60.0 * k as f64;
            let (mut n, mut e, mut sog) = (kn * 0.514_444 * t * hdg.cos(), kn * 0.514_444 * t * hdg.sin(), kn);
            if target && j % 2 == 1 {
                let w = 2.0 * std::f64::consts::PI * t / 3000.0;
                n = radius_km * 1000.0 * w.sin();
                e = radius_km * 1000.0 * (1.0 - w.cos());
                sog = 2.0 * std::f64::consts::PI * radius_km * 1000.0 / 3000.0 / 0.514_444;
            }
            if target && j % 2 == 0 && k >= jump_at {
                n += jump_km * 1000.0;
            }
            n += 10.0 * rng.normal();
            e += 10.0 * rng.normal();
            let lat = lat0 + n / m_per_deg;
            let lon = lon0 + e / (m_per_deg * lat0.to_radians().cos());
            rows.push(format!("{},V{v},{lat:.6},{lon:.6},{sog:.1}", iso(t0 + t)));
        }
    }
    let r = track_incident(dir, &format!("a{j}"), "ais", &rows, t0, t0 + 3600.0);
    json!({"case": format!("A{j:02}"), "positive": positive, "spoof": detail,
           "spoofing_indicated": indicated(&r, "spoofing"), "jamming_indicated": indicated(&r, "jamming")})
}

/// Section 2.2, ADS-B: one case. Returns its record.
fn adsb_case(k: usize, rng: &mut Rng, dir: &Path) -> Value {
    let t0 = 1_785_542_400.0 + 2.0 * 3600.0 * k as f64; // 2026-08-01T00:00:00Z
    let positive = k < 10;
    let (hit, from, len) = if positive {
        (rng.ui(1, 4), rng.ui(10, 20), rng.ui(6, 15))
    } else {
        (0, 0, 0)
    };
    let mut rows = vec!["timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp,source_type".to_string()];
    for a in 0..6 {
        let (lat, lon0) = (rng.u(50.0, 60.0), rng.u(0.0, 20.0));
        for n in 0..40 {
            let bad = (a as i64) < hit && n >= from && n < from + len;
            let (nic, nacp) = if bad { (0, 0) } else { (8, 9) };
            rows.push(format!(
                "{},AC{a},{lat:.4},{:.4},35000,{nic},{nacp},adsb_icao",
                iso(t0 + 30.0 * n as f64),
                lon0 + 0.045 * n as f64
            ));
        }
    }
    let r = track_incident(dir, &format!("b{k}"), "adsb", &rows, t0, t0 + 1200.0);
    json!({"case": format!("B{k:02}"), "positive": positive,
           "drop": {"aircraft": hit, "from_report": from, "reports": len},
           "jamming_indicated": indicated(&r, "jamming"), "spoofing_indicated": indicated(&r, "spoofing")})
}

fn track_incident(dir: &Path, name: &str, kind: &str, rows: &[String], t0: f64, t1: f64) -> ForensicReport {
    let case = dir.join(name);
    std::fs::create_dir_all(&case).unwrap();
    std::fs::write(case.join("tracks.csv"), rows.join("\n") + "\n").unwrap();
    let inc = format!(
        "kind = \"pro-incident-forensics\"\n[incident]\nid = \"V2-{name}\"\ntitle = \"V2 {name}\"\nsubject = \"area\"\n\
         start_utc = \"{}\"\nend_utc = \"{}\"\n[[{kind}]]\nlabel = \"feed\"\npath = \"tracks.csv\"\n",
        iso(t0),
        iso(t1 + 1.0)
    );
    std::fs::write(case.join("incident.toml"), inc).unwrap();
    let r = analyse(&IncidentScenario::from_file(&case.join("incident.toml")).unwrap(), None).unwrap();
    let _ = std::fs::remove_dir_all(&case);
    r
}

fn count(v: &[Value], f: impl Fn(&Value) -> bool) -> usize {
    v.iter().filter(|x| f(x)).count()
}

fn bar(pass: bool, n: usize, of: usize, rule: &str) -> Value {
    json!({"pass": pass, "count": n, "of": of, "rule": rule})
}

#[test]
fn v2_labelled_set_scored_once_against_the_preregistered_bars() {
    let dir = tmp();
    let mut rng = Rng(SEED);
    let s: Vec<Value> = (0..48).map(|i| receiver_case(i, &mut rng, &dir)).collect();
    let a: Vec<Value> = (0..20).map(|j| ais_case(j, &mut rng, &dir)).collect();
    let b: Vec<Value> = (0..20).map(|k| adsb_case(k, &mut rng, &dir)).collect();
    let _ = std::fs::remove_dir_all(&dir);

    let cls = |c: &'static str| move |x: &Value| x["class"] == c;
    let own = |x: &Value| match x["class"].as_str() {
        Some("jamming") => x["jamming_indicated"] == true,
        Some("spoofing") => x["spoofing_indicated"] == true,
        _ => false,
    };
    let pos: Vec<Value> = s.iter().filter(|x| cls("jamming")(x) || cls("spoofing")(x)).cloned().collect();
    let neg: Vec<Value> = s.iter().filter(|x| cls("clean")(x) || cls("confounder")(x)).cloned().collect();
    let b1 = count(&pos, own);
    let b1j = count(&s, |x| cls("jamming")(x) && own(x));
    let b1s = count(&s, |x| cls("spoofing")(x) && own(x));
    let b2 = count(&neg, |x| x["jamming_indicated"] == true || x["spoofing_indicated"] == true);
    let b3 = count(&pos, |x| match x["class"].as_str() {
        Some("jamming") => x["spoofing_indicated"] == true,
        _ => x["jamming_indicated"] == true,
    });
    let detected: Vec<&Value> = pos.iter().filter(|x| own(x)).collect();
    let b4 = detected.iter().filter(|x| x["onset_ok"] == true).count();
    let b4_pass = !detected.is_empty() && b4 * 10 >= detected.len() * 9;
    let b5 = count(&s, |x| x["high_confidence"] == true);
    let a_tp = count(&a, |x| x["positive"] == true && x["spoofing_indicated"] == true);
    let a_fp = count(&a, |x| x["positive"] == false && x["spoofing_indicated"] == true);
    let b_tp = count(&b, |x| x["positive"] == true && x["jamming_indicated"] == true);
    let b_fp = count(&b, |x| x["positive"] == false && x["jamming_indicated"] == true);

    let bars = json!({
        "B1": bar(b1 >= 22, b1, 24, "injected category indicated in >= 22 of 24 positives"),
        "B1j": bar(b1j >= 10, b1j, 12, "jamming indicated in >= 10 of 12 jamming cases"),
        "B1s": bar(b1s >= 10, b1s, 12, "spoofing indicated in >= 10 of 12 spoofing cases"),
        "B2": bar(b2 <= 1, b2, 24, "jamming or spoofing indicated in <= 1 of 24 negatives"),
        "B3": bar(b3 <= 2, b3, 24, "the other category indicated in <= 2 of 24 positives"),
        "B4": bar(b4_pass, b4, detected.len(), "onset in [start, start + ramp + 60 s] in >= 90 % of detected positives"),
        "B5": bar(b5 == 0, b5, 48, "no high-confidence finding in any receiver-only case"),
        "B6_tp": bar(a_tp >= 9, a_tp, 10, "AIS: spoofing indicated in >= 9 of 10 positives"),
        "B6_fp": bar(a_fp <= 1, a_fp, 10, "AIS: spoofing indicated in <= 1 of 10 negatives"),
        "B7_tp": bar(b_tp >= 9, b_tp, 10, "ADS-B: jamming indicated in >= 9 of 10 positives"),
        "B7_fp": bar(b_fp <= 1, b_fp, 10, "ADS-B: jamming indicated in <= 1 of 10 negatives"),
    });
    let all_pass = bars.as_object().unwrap().values().all(|b| b["pass"] == true);
    let doc = json!({
        "format": "kshana-pro-v2-results/1",
        "preregistration": "2026-11-15_PREREGISTRATION.md (commit e5a3369b)",
        "method": "kshana-pro/incident-forensics/v1",
        "data_class": "SYNTHETIC",
        "seed": SEED,
        "public_event_part": "not scored: empty at registration (no openly licensed public-event data)",
        "synthetic_bars_all_pass": all_pass,
        "e1_claimed": false,
        "bars": bars,
        "receiver_cases": s,
        "ais_cases": a,
        "adsb_cases": b,
    });
    let text = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    println!("{}", serde_json::to_string_pretty(&doc["bars"]).unwrap());
    if std::env::var_os("KSHANA_PRO_V2_WRITE").is_some() {
        std::fs::write(RESULTS, &text).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(RESULTS).expect("results.json is committed after the one scoring run");
    assert_eq!(committed, text, "the V2 run no longer reproduces the committed results");
    // E1 is never claimed from this run: the public-event part is empty.
    assert_eq!(doc["e1_claimed"], false);
}
