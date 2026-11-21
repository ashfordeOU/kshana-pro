// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Fuzz-style robustness tests for the parsers of user-supplied text and bytes (robustness
//! audit, 21 November 2026). No fuzzing crate is used: a seeded xorshift generator mutates
//! valid sample lines (inserting NaN, inf, huge numbers, multi-byte characters, empty fields,
//! stray separators) and also produces random text, and every parser is run under
//! `catch_unwind`. The property is that a parser returns a value or an error on any input and
//! never panics. The seed is fixed, so a failure reproduces; the failing input is printed.

use std::panic::{catch_unwind, AssertUnwindSafe};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const TOKENS: &[&str] = &[
    "NaN",
    "nan",
    "-nan",
    "inf",
    "-inf",
    "infinity",
    "1e309",
    "-1e309",
    "0",
    "-0",
    "",
    "99999999999999999999999",
    "-1",
    "1.5",
    ",",
    ";",
    ":",
    "T",
    "Z",
    "-",
    ".",
    "°",
    "é",
    "€",
    "\u{1F600}",
    " ",
    "\t",
    "\"",
    "'",
    "#",
    "\n",
    "\r\n",
    "60",
    "24",
    "23:59:60",
    "1970",
    "2026-02-31",
    "2026-13-01",
    "9999-12-31T23:59:59Z",
    "0000-01-01T00:00:00Z",
    "*",
    "$",
];

fn mutate(r: &mut Rng, seed: &str) -> String {
    let mut s: Vec<char> = seed.chars().collect();
    for _ in 0..1 + r.below(4) {
        let pos = r.below(s.len() + 1);
        match r.below(4) {
            0 => {
                let t = TOKENS[r.below(TOKENS.len())];
                for (k, c) in t.chars().enumerate() {
                    s.insert((pos + k).min(s.len()), c);
                }
            }
            1 if !s.is_empty() => {
                let end = (pos + 1 + r.below(6)).min(s.len());
                s.drain(pos.min(s.len())..end);
            }
            2 if !s.is_empty() => {
                // Replace one field of a comma-separated line by a token.
                let text: String = s.iter().collect();
                let mut f: Vec<String> = text.split(',').map(String::from).collect();
                let i = r.below(f.len());
                f[i] = TOKENS[r.below(TOKENS.len())].to_string();
                s = f.join(",").chars().collect();
            }
            _ => {
                let c = char::from_u32(r.below(0x2FF) as u32).unwrap_or('?');
                s.insert(pos.min(s.len()), c);
            }
        }
    }
    s.into_iter().collect()
}

fn random_text(r: &mut Rng) -> String {
    let n = r.below(60);
    (0..n)
        .map(|_| {
            if r.below(3) == 0 {
                TOKENS[r.below(TOKENS.len())].to_string()
            } else {
                char::from_u32(32 + r.below(95) as u32)
                    .unwrap_or('?')
                    .to_string()
            }
        })
        .collect()
}

/// Inputs for one parser family: the valid seeds, each mutated many times, random text, and the
/// seeds truncated at every byte that is a char boundary.
fn inputs(seeds: &[&str], n: usize, seed: u64) -> Vec<String> {
    let mut r = Rng(seed);
    let mut out: Vec<String> = vec![String::new(), "\n".into(), ",,,,".into()];
    for s in seeds {
        out.push(s.to_string());
        for k in (0..s.len()).filter(|&k| s.is_char_boundary(k)) {
            out.push(s[..k].to_string());
        }
        for _ in 0..n {
            out.push(mutate(&mut r, s));
        }
    }
    for _ in 0..n {
        out.push(random_text(&mut r));
    }
    out
}

/// Run `f` on every input; panics are collected and reported together.
fn never_panics(name: &str, seeds: &[&str], f: impl Fn(&str)) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut bad = Vec::new();
    for (i, s) in inputs(seeds, 300, 0x9E37_79B9_7F4A_7C15 ^ name.len() as u64)
        .iter()
        .enumerate()
    {
        if catch_unwind(AssertUnwindSafe(|| f(s))).is_err() {
            bad.push(format!("#{i}: {s:?}"));
        }
    }
    std::panic::set_hook(prev);
    assert!(
        bad.is_empty(),
        "{name}: {} input(s) panicked, first: {}",
        bad.len(),
        bad.iter().take(5).cloned().collect::<Vec<_>>().join(" | ")
    );
}

#[test]
fn time_parsers_never_panic() {
    let seeds = [
        "2026-11-21T12:34:56Z",
        "2026-11-21T12:34:56.789Z",
        "2016-12-31T23:59:60Z",
        "1767225600",
        "2026-11-21",
    ];
    never_panics("forensics parse_utc", &seeds, |s| {
        let _ = kshana_pro::forensics::inputs::parse_utc(s);
    });
    never_panics("forensics iso", &seeds, |s| {
        if let Some(t) = kshana_pro::forensics::inputs::parse_utc(s) {
            let _ = kshana_pro::forensics::inputs::iso(t);
        }
    });
    never_panics("agri parse_utc", &seeds, |s| {
        let _ = kshana_pro::agri::parse_utc(s);
    });
    never_panics("timebomb Label::parse_iso", &seeds, |s| {
        let _ = kshana_pro::timing::timebomb::Label::parse_iso(s);
    });
    never_panics("eloran AbsTime::from_utc_iso", &seeds, |s| {
        let _ = kshana_eloran::time::AbsTime::from_utc_iso(s);
    });
    never_panics("eloran UtcStamp::parse", &seeds, |s| {
        let _ = kshana_eloran::sync::UtcStamp::parse(s);
    });
    never_panics(
        "payments parse_rfc3339",
        &["2026-11-21T12:34:56+05:30"],
        |s| {
            let _ = kshana_pro::payments::provider::parse_rfc3339(s);
        },
    );
}

#[test]
fn report_text_parsers_never_panic() {
    use kshana_pro::ireports::text as t;
    let seeds = [
        "211234Z NOV 26",
        "5440N02520E",
        "54 40 12N 025 20 30E",
        "Q) EYVL/QRDCA/IV/BO/W/000/180/5440N02520E005",
        "5440N02520E005",
        "SFC-FL180",
        "2026-11-21 12:34",
    ];
    never_panics("ireports parse_utc", &seeds, |s| {
        let _ = t::parse_utc(s);
    });
    never_panics("ireports parse_datetime", &seeds, |s| {
        let _ = t::parse_datetime(s);
    });
    never_panics("ireports parse_coords", &seeds, |s| {
        let _ = t::parse_coords(s);
    });
    never_panics("ireports parse_q_position", &seeds, |s| {
        let _ = t::parse_q_position(s);
    });
    never_panics("ireports parse_altitude", &seeds, |s| {
        let _ = t::parse_altitude(s);
    });
    never_panics(
        "briefings parse_notam",
        &["A1234/26 NOTAMN\nQ) EYVL/QRDCA/IV/BO/W/000/180/5440N02520E005\nA) EYVL B) 2611210000 C) 2611220000\nE) GNSS INTERFERENCE"],
        |s| {
            let _ = kshana_pro::briefings::spillover::parse_notam(s, "X");
        },
    );
}

#[test]
fn csv_tables_never_panic() {
    let seeds = [
        "t,a,b\n1,2.5,3\n2,nan,4\n",
        "timestamp,x_km,y_km,z_km\n2026-11-21T00:00:00Z,1,2,3\n",
    ];
    never_panics("transport Table", &seeds, |s| {
        if let Ok(t) = kshana_pro::transport::Table::parse(s) {
            for r in 0..t.rows.len() {
                for c in 0..t.cols.len() {
                    let _ = t.num(r, Some(c));
                    let _ = t.text(r, Some(c));
                }
            }
        }
    });
    never_panics("simlab numeric_csv", &["1,2,3\n4,5,6\n"], |s| {
        let _ = kshana_pro::simlab::numeric_csv(s, 3);
    });
}

#[test]
fn timing_log_parsers_never_panic() {
    use kshana_pro::timing::selfcheck::{read_source, SourceSpec};
    let spec = |format: &str| SourceSpec {
        name: "s".into(),
        kind: None,
        format: format.into(),
        path: "x".into(),
        u_s: 1e-6,
        disciplining: false,
        sign: None,
        time_offset_s: 0.0,
        select: None,
    };
    never_panics(
        "selfcheck csv",
        &["1767225600,1e-6,1\n1767225601,2e-6,1\n"],
        |s| {
            let _ = read_source(&spec("csv"), s);
        },
    );
    never_panics(
        "selfcheck ntp-peerstats",
        &["61000 3600.000 127.127.1.0 9014 0.000001 0.000200 0.000010 0.000005\n"],
        |s| {
            let _ = read_source(&spec("ntp-peerstats"), s);
        },
    );
    never_panics(
        "selfcheck ptp4l",
        &["ptp4l[1234.567]: master offset -12 s2 freq +100 path delay 500\n"],
        |s| {
            let _ = read_source(&spec("ptp4l"), s);
        },
    );
    never_panics(
        "station read_passes",
        &["pass_id,satellite,scheduled_aos,scheduled_los,actual_aos,bytes_delivered\np1,A,2026-11-01T10:00:00Z,2026-11-01T10:10:00Z,2026-11-01T10:00:05Z,990\n"],
        |s| {
            let _ = kshana_pro::timing::station::read_passes(s);
        },
    );
    never_panics(
        "chain read_circular_t",
        &["mjd,utc_minus_utck_ns,u_ns\n61334,1.0,2.0\n"],
        |s| {
            let _ = kshana_pro::timing::chain::read_circular_t(s);
        },
    );
    never_panics("recovery read_csv", &["t_s,te_ns\n0,1\n1,2\n"], |s| {
        let _ = kshana_pro::timing::recovery::read_csv(s, 0.0);
    });
    never_panics(
        "timebomb read_nmea",
        &["$GPZDA,235959.00,30,06,2027,00,00*6B\n$GPRMC,235959.00,A,5440.000,N,02520.000,E,0.0,0.0,300627,,,A*6A\n"],
        |s| {
            let _ = kshana_pro::timing::timebomb::read_nmea(s);
        },
    );
    never_panics(
        "timebomb read_csv",
        &["lab_s,utc,valid\n1,2027-06-30T23:59:59Z,1\n"],
        |s| {
            let _ = kshana_pro::timing::timebomb::read_csv(s);
        },
    );
    never_panics(
        "leo_timing parse_te",
        &["t_s,te_ns,valid\n0,12.5,1\n1,13.0,1\n"],
        |s| {
            let _ = kshana_pro::leo_timing::parse_te(s);
        },
    );
    never_panics(
        "altpnt time_iface parse_log",
        &["t_s,offset_ns,valid\n0,1,1\n"],
        |s| {
            let _ = kshana_pro::altpnt::time_iface::parse_log(s);
        },
    );
}

#[test]
fn navigation_and_track_parsers_never_panic() {
    never_panics("navaids parse_track", &["0,54.0,25.0,3000,5\n"], |s| {
        let _ = kshana_pro::navaids::inspection::parse_track(s);
    });
    never_panics("navaids parse_measurements", &["0,DME1,dme,12.5\n"], |s| {
        let _ = kshana_pro::navaids::inspection::parse_measurements(s);
    });
    never_panics(
        "interface_drift parse_records",
        &["timestamp,stream,message_type,subtype,gnss_ids,prn,fields\n2026-09-01T00:00:00Z,s,4073,1,G|R|E|J,193,vendor_id=5\n"],
        |s| {
            let _ = kshana_pro::interface_drift::parse_records(s);
        },
    );
    never_panics(
        "integrity parse_ais",
        &["mmsi,t,lat,lon\n123456789,1767225600,54.0,25.0\n"],
        |s| {
            let _ = kshana_pro::integrity::parse_ais(s);
        },
    );
    never_panics(
        "integrity parse_rf",
        &["id,t,cn0\n1,1767225600,45\n"],
        |s| {
            let _ = kshana_pro::integrity::parse_rf(s);
        },
    );
    never_panics(
        "multi_source wwrns parse_fixes",
        &["t_s,source,lat_deg,lon_deg,sigma_m\n0,gnss,54.0,25.0,5\n"],
        |s| {
            let _ = kshana_pro::multi_source::wwrns::parse_fixes(s);
        },
    );
    never_panics(
        "lunar_surface traverse parse_log",
        &["t_s,east_m,north_m\n0,1,2\n1,2,3\n"],
        |s| {
            let _ = kshana_pro::lunar_surface::traverse::parse_log(s);
        },
    );
    never_panics(
        "space_rx see parse_trace",
        &["t_s,prn,cn0_dbhz\n0,1,45\n"],
        |s| {
            let _ = kshana_pro::space_rx::see::parse_trace(s);
        },
    );
    never_panics("space_rx obc parse_log", &["key=value\nfix=3\n"], |s| {
        let _ = kshana_pro::space_rx::obc::parse_log(s);
    });
    never_panics(
        "utm parse_rid",
        &["t_s,id,lat,lon,alt_m\n0,UA1,54.0,25.0,100\n"],
        |s| {
            let _ = kshana_pro::utm::parse_rid(s);
        },
    );
    never_panics("area parse_states", &["0,ok\n10,degraded,note\n"], |s| {
        let _ = kshana_pro::area::parse_states("st", s);
    });
    never_panics("area parse_feed", &["0,60,jamming,0.9,54.0,25.0\n"], |s| {
        let _ = kshana_pro::area::parse_feed("f", s, 0.5);
    });
    never_panics(
        "alerts parse_alerts",
        &["id,t,kind\na1,1767225600,jam\n"],
        |s| {
            let _ = kshana_pro::alerts::parse_alerts(s);
        },
    );
    never_panics(
        "aviation read_intervals",
        &["id,start,end\nx,0,10\n"],
        |s| {
            let _ = kshana_pro::aviation::nuisance::read_intervals(s);
        },
    );
    never_panics(
        "briefings parse_areas",
        &["name,lat,lon,radius_km\nA,54.0,25.0,50\n"],
        |s| {
            let _ = kshana_pro::briefings::areas::parse_areas(s);
        },
    );
}

#[test]
fn binary_readers_never_panic() {
    // UBX: random bytes and a valid frame with bytes flipped.
    let mut r = Rng(0xDEAD_BEEF_CAFE_F00D);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut bad = 0;
    let frame: Vec<u8> = vec![0xB5, 0x62, 0x01, 0x21, 0x14, 0x00]
        .into_iter()
        .chain(std::iter::repeat_n(0u8, 20))
        .chain([0x36, 0x8D])
        .collect();
    for _ in 0..2000 {
        let mut b: Vec<u8> = if r.below(2) == 0 {
            (0..r.below(200)).map(|_| r.next() as u8).collect()
        } else {
            frame.repeat(1 + r.below(3))
        };
        for _ in 0..r.below(6) {
            if !b.is_empty() {
                let i = r.below(b.len());
                b[i] = r.next() as u8;
            }
        }
        if r.below(3) == 0 && !b.is_empty() {
            let cut = r.below(b.len());
            b.truncate(cut);
        }
        if catch_unwind(AssertUnwindSafe(|| {
            let _ = kshana_pro::timing::timebomb::read_ubx(&b);
        }))
        .is_err()
        {
            bad += 1;
        }
    }
    std::panic::set_hook(prev);
    assert_eq!(bad, 0, "read_ubx panicked on {bad} inputs");
}

/// The specific gaps the robustness audit closed: a non-ASCII Q-line position is "not a
/// position" (it used to panic on a char boundary), a non-numeric PRN or subtype in decoded
/// interface records is an error (it used to become "none" and hide the unknown-PRN alarm), and
/// a non-finite device time in a timebomb CSV is a skipped row (NaN used to become 1970).
#[test]
fn audited_gaps_stay_closed() {
    assert_eq!(
        kshana_pro::ireports::text::parse_q_position("5440N0252°E100"),
        None
    );
    assert!(kshana_pro::ireports::text::parse_q_position("5440N02520E005").is_some());
    let head = "timestamp,stream,message_type,subtype,gnss_ids,prn,fields\n";
    let rec = |prn: &str, sub: &str| format!("{head}2026-09-01T00:00:00Z,s,4073,{sub},G,{prn},\n");
    assert!(kshana_pro::interface_drift::parse_records(&rec("193", "1")).is_ok());
    assert!(kshana_pro::interface_drift::parse_records(&rec("", "")).is_ok());
    let e = kshana_pro::interface_drift::parse_records(&rec("G33", "1")).unwrap_err();
    assert!(e.contains("prn 'G33' is not a number"), "{e}");
    assert!(kshana_pro::interface_drift::parse_records(&rec("193", "x")).is_err());
    let (recs, skipped) =
        kshana_pro::timing::timebomb::read_csv("lab_s,utc\n1,1798761599\n2,NaN\n3,-inf\n4,1e300\n");
    assert_eq!((recs.len(), skipped), (1, 3));
}
