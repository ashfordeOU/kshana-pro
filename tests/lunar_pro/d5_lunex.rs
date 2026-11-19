// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D5: LUNEX prototype (a proposal for the IGS RINEX Committee, not a standard). Part of the
// tests/lunar_pro.rs test binary.

use kshana_pro::lunar_pro::lunex::*;

/// A synthetic LuGRE-style CSV: GPS L1 C/A + L5 and Galileo E1 seen from the lunar surface
/// (ranges ≈ 380 000 km), one missing carrier phase, rows deliberately out of time order.
fn csv() -> String {
    let mut s = String::from("# SYNTHETIC LuGRE-style observations\ntime,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz\n");
    let sats = [("G05", 0.0), ("G13", 1.7e6), ("E11", -2.3e6)];
    for k in (0..6).rev() {
        let t = format!("2025-03-03T10:{:02}:00.000", k);
        for (i, (sv, off)) in sats.iter().enumerate() {
            let pr = 3.80e8 + off + 1234.567 * k as f64 + i as f64 * 0.001;
            let l1 = pr / 0.190293672798365;
            let dop = -650.25 + 3.5 * k as f64;
            let cn0 = 24.0 + i as f64;
            let sig = "1C";
            let carrier = if *sv == "G13" && k == 3 {
                String::new()
            } else {
                format!("{l1:.3}")
            };
            s.push_str(&format!(
                "{t},{sv},{sig},{pr:.3},{carrier},{dop:.3},{cn0:.3}\n"
            ));
            if sv.starts_with('G') {
                s.push_str(&format!(
                    "{t},{sv},5Q,{:.3},,{:.3},{:.3}\n",
                    pr + 2.5,
                    dop * 0.747,
                    cn0 - 3.0
                ));
            }
        }
    }
    s
}

fn opts() -> CsvOptions {
    CsvOptions {
        interval_s: Some(60.0),
        date: "20261119 000000 UTC".into(),
        approx_xyz: Some([1_234.5, -2_345.6, -1_736_000.25]),
        ..Default::default()
    }
}

fn csv_values() -> Vec<(String, String, String, [Option<f64>; 4])> {
    csv()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("time,"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let v = |i: usize| f[i].parse::<f64>().ok();
            (
                f[0].to_string(),
                f[1].to_string(),
                f[2].to_string(),
                [v(3), v(4), v(5), v(6)],
            )
        })
        .collect()
}

fn epoch_index(l: &Lunex, t: &str) -> usize {
    let min: u32 = t[14..16].parse().unwrap();
    l.epochs.iter().position(|e| e.time.minute == min).unwrap()
}

#[test]
fn d5_csv_to_lunex_round_trips_byte_for_byte_and_value_for_value() {
    let l = from_csv(&csv(), &opts()).unwrap();
    assert_eq!(l.epochs.len(), 6);
    // Epochs come out in time order whatever the CSV order.
    assert!(l
        .epochs
        .windows(2)
        .all(|w| secs(&w[1].time) > secs(&w[0].time)));
    let text = l.to_text();
    let back = Lunex::parse(&text).unwrap();
    assert_eq!(back.to_text(), text, "write(read(x)) == x");
    assert_eq!(back.header, l.header);
    for (t, sv, sig, v) in csv_values() {
        let e = epoch_index(&back, &t);
        let sat = back.epochs[e].sats.iter().find(|s| s.sat == sv).unwrap();
        let codes = &back
            .header
            .obs_types
            .iter()
            .find(|(c, _)| *c == sv.as_bytes()[0] as char)
            .unwrap()
            .1;
        for (k, p) in ["C", "L", "D", "S"].iter().enumerate() {
            let j = codes
                .iter()
                .position(|c| *c == format!("{p}{sig}"))
                .unwrap();
            match (v[k], sat.obs[j]) {
                (Some(a), Some(b)) => {
                    assert!((a - b.value).abs() <= 0.0005 + 1e-9, "{sv} {p}{sig}")
                }
                (None, None) => {}
                other => panic!("{sv} {p}{sig} at {t}: {other:?}"),
            }
        }
    }
}

#[test]
fn d5_the_open_engine_rinex_3_parser_reads_lunex_bodies() {
    let l = from_csv(&csv(), &opts()).unwrap();
    let text = l.to_text();
    let r = kshana::rinex_obs::parse_obs(&text).unwrap();
    assert_eq!(r.header.version, 3.05);
    assert_eq!(r.header.system, 'M');
    assert_eq!(r.epochs.len(), l.epochs.len());
    assert_eq!(
        r.header.approx_xyz,
        Some([1_234.5, -2_345.6, -1_736_000.25])
    );
    for (k, e) in l.epochs.iter().enumerate() {
        for s in &e.sats {
            let codes = &l
                .header
                .obs_types
                .iter()
                .find(|(c, _)| *c == s.sat.as_bytes()[0] as char)
                .unwrap()
                .1;
            for (code, ob) in codes.iter().zip(&s.obs) {
                assert_eq!(
                    r.observation(k, &s.sat, code),
                    ob.map(|o| (o.value * 1e3).round() / 1e3)
                );
            }
        }
    }
}

#[test]
fn d5_validator_accepts_a_clean_file_with_lunar_tags_and_afs_observables() {
    let mut text = from_csv(&csv(), &opts()).unwrap().to_text();
    let v = validate(&text);
    assert!(v.valid, "{:#?}", v.findings);
    assert_eq!(v.epochs, 6);
    assert!(v.status.contains("not a standard"));
    // A TL-tagged lunar-orbit file with LunaNet AFS observables (system L, band 9).
    let afs = "time,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz\n\
               2027-06-01T00:00:00.0,L01,9P,12345678.901,102938475.125,-1500.5,38.0\n\
               2027-06-01T00:00:10.0,L01,9P,12345700.101,102938650.250,-1500.0,38.5\n\
               2027-06-01T00:00:10.0,L02,9P,22345700.101,,900.0,31.5\n";
    let o = CsvOptions {
        platform: "ORBIT".into(),
        frame: "MCI_ICRF".into(),
        time_tag: "TL".into(),
        lunar_scale: Some(("TL".into(), Some(2_821_020.123_4), "LTL1".into())),
        interval_s: Some(10.0),
        ..Default::default()
    };
    let l = from_csv(afs, &o).unwrap();
    text = l.to_text();
    let v = validate(&text);
    assert!(v.valid, "{:#?}", v.findings);
    let back = Lunex::parse(&text).unwrap();
    assert_eq!(back.header.time_tag, "TL");
    assert_eq!(
        back.header.lunar_scale,
        Some(("TL".into(), Some(2_821_020.123_4), "LTL1".into()))
    );
    assert_eq!(
        back.header.obs_types,
        vec![(
            'L',
            vec!["C9P".to_string(), "L9P".into(), "D9P".into(), "S9P".into()]
        )]
    );
}

fn find(text: &str, code: &str) -> bool {
    validate(text).findings.iter().any(|f| f.code == code)
}

#[test]
fn d5_validator_catches_injected_faults() {
    let clean = from_csv(&csv(), &opts()).unwrap().to_text();
    let lines: Vec<String> = clean.lines().map(String::from).collect();
    let join = |v: &[String]| v.join("\n") + "\n";
    let replace_label = |label: &str, content: &str| -> String {
        let mut v = lines.clone();
        let i = v.iter().position(|l| l.ends_with(label)).unwrap();
        v[i] = format!("{content:<60}{label}");
        join(&v)
    };
    // E001: no LUNEX record.
    let no_lunex: Vec<String> = lines
        .iter()
        .filter(|l| !l.ends_with("LUNEX VERSION / TYPE"))
        .cloned()
        .collect();
    assert!(find(&join(&no_lunex), "E001"));
    assert!(!validate(&join(&no_lunex)).valid);
    // E002: an unknown time tag.
    let t0 = lines
        .iter()
        .find(|l| l.ends_with("TIME OF FIRST OBS"))
        .unwrap()[..48]
        .to_string();
    assert!(find(
        &replace_label("TIME OF FIRST OBS", &format!("{t0}XYZ")),
        "E002"
    ));
    // E003: a lunar tag with no LUNAR TIME SCALE record.
    assert!(find(
        &replace_label("TIME OF FIRST OBS", &format!("{t0}TL")),
        "E003"
    ));
    // E004: an unknown frame; E011 an unknown platform.
    assert!(find(&replace_label("LUNAR REF FRAME", "MOON_XYZ"), "E004"));
    assert!(find(&replace_label("LUNAR PLATFORM", "BALLOON"), "E011"));
    // E007: epochs out of order (swap the first two epoch blocks' times).
    let mut v = lines.clone();
    let idx: Vec<usize> = v
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with('>'))
        .map(|(i, _)| i)
        .collect();
    let (a, b) = (v[idx[0]].clone(), v[idx[1]].clone());
    v[idx[0]] = b.clone();
    v[idx[1]] = a;
    let swapped = join(&v);
    assert!(find(&swapped, "E007") && find(&swapped, "E010"));
    // E009: a wrong satellite count.
    let mut v = lines.clone();
    v[idx[2]] = format!("{} 9", &v[idx[2]][..32]);
    assert!(find(&join(&v), "E009"));
    // E008: an implausible pseudorange for Earth GNSS seen from the Moon.
    let mut v = lines.clone();
    let rec = idx[0] + 1;
    v[rec] = format!("{}{:>14.3}{}", &v[rec][..3], 1_000.0, &v[rec][17..]);
    assert!(find(&join(&v), "E008"));
    // E006: a satellite of an undeclared system.
    let mut v = lines.clone();
    v[idx[0]] = format!("{}{:>3}", &v[idx[0]][..32], 4);
    v.insert(idx[0] + 1, format!("R07{:>14.3}", 3.9e8));
    assert!(find(&join(&v), "E006"));
    // W101: spacing not a multiple of INTERVAL is a warning, not an error.
    let w = replace_label("INTERVAL", &format!("{:>10.3}", 45.0));
    assert!(find(&w, "W101") && validate(&w).valid);
}

#[test]
fn d5_converter_refuses_bad_csv() {
    let o = CsvOptions::default();
    assert!(from_csv("", &o).is_err());
    assert!(from_csv("2025-03-03T10:00:00,G05,1C,3.8e8,,\n", &o).is_err());
    assert!(from_csv("2025-03-03T10:00:00,GPS5,1C,3.8e8,,,\n", &o).is_err());
    assert!(from_csv("2025-03-03T10:00:00,G05,L1CA,3.8e8,,,\n", &o).is_err());
    assert!(from_csv("2025-03-03 10:00:00,G05,1C,3.8e8,,,\n", &o).is_err());
    assert!(from_csv("2025-03-03T10:00:00,G05,1C,abc,,,\n", &o).is_err());
    let dup = "2025-03-03T10:00:00,G05,1C,3.8e8,,,\n2025-03-03T10:00:00,G05,1C,3.8e8,,,\n";
    assert!(from_csv(dup, &o).is_err());
}
