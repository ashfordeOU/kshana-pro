// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Correction-decoder interface-change regression (research 2026-11-20 gaps B1604) on SYNTHETIC
//! profiles and header records. The fixtures are shaped like a correction stream adding a
//! constellation to its mask (the MADOCA-PPP BeiDou-addition case) and a new transmitting
//! satellite (the QZSS seven-satellite transition); every value here is SYNTHETIC, not taken from
//! IS-QZSS-L6.

use std::process::Command;

use kshana_pro::interface_drift::{self as id, Profile};

fn profile() -> Profile {
    toml::from_str(
        r#"format = "kshana-pro-interface-profile/1"
stream = "synth-l6e-ppp"
version = "v1"
source = "SYNTHETIC test profile (not from any specification)"
message_types = [4073]
subtypes = [1, 2, 3, 4, 7]
gnss_ids = ["G", "R", "E", "J"]
prns = [193, 194, 195, 196, 199]
[fields]
vendor_id = ["5"]
"#,
    )
    .unwrap()
}

fn records(after: &str) -> String {
    let mut s = String::from("timestamp,stream,message_type,subtype,gnss_ids,prn,fields\n");
    for d in 1..=30 {
        s.push_str(&format!(
            "2026-09-{d:02}T00:00:00Z,synth-l6e-ppp,4073,1,G|R|E|J,193,vendor_id=5\n"
        ));
    }
    s.push_str(after);
    s
}

#[test]
fn constellation_addition_raises_a_drift_alarm_at_its_first_appearance() {
    let p = profile();
    let clean = id::check(&p, &id::parse_records(&records("")).unwrap()).unwrap();
    assert!(clean.alarms.is_empty(), "{clean:?}");
    let after = "2026-10-01T00:00:00Z,synth-l6e-ppp,4073,1,G|R|E|J,193,vendor_id=5\n\
                 2026-10-02T00:00:00Z,synth-l6e-ppp,4073,1,G|R|E|C|J,193,vendor_id=5\n\
                 2026-10-03T00:00:00Z,synth-l6e-ppp,4073,1,G|R|E|C|J,193,vendor_id=5\n\
                 2026-10-03T00:00:00Z,other-stream,1,,C,,\n";
    let d = id::check(&p, &id::parse_records(&records(after)).unwrap()).unwrap();
    assert_eq!(d.alarms.len(), 1);
    assert_eq!(d.alarms[0].element, "GNSS id C");
    assert_eq!(d.alarms[0].first_utc, "2026-10-02T00:00:00Z");
    assert_eq!(d.alarms[0].occurrences, 2);
    assert_eq!(d.other_streams, 1);
    assert!(d.verdict.starts_with("INTERFACE DRIFT"));
    // A new transmitting satellite and an unknown subtype.
    let after = "2026-11-01T00:00:00Z,synth-l6e-ppp,4073,9,G,200,vendor_id=5\n";
    let d = id::check(&p, &id::parse_records(&records(after)).unwrap()).unwrap();
    let els: Vec<&str> = d.alarms.iter().map(|a| a.element.as_str()).collect();
    assert!(
        els.contains(&"transmitting PRN 200") && els.contains(&"subtype 9"),
        "{els:?}"
    );
}

#[test]
fn vectors_separate_a_strict_decoder_from_a_silent_one() {
    let set = id::vectors(&profile()).unwrap();
    let flags = set.vectors.iter().filter(|v| v.expect == "flag").count();
    assert!(flags >= 5 && set.vectors.len() > flags);
    assert!(set
        .vectors
        .iter()
        .any(|v| v.expect == "flag" && v.why.contains("GNSS id C")));
    let strict: String = std::iter::once("id,result".to_string())
        .chain(set.vectors.iter().map(|v| format!("{},{}", v.id, v.expect)))
        .collect::<Vec<_>>()
        .join("\n");
    let s = id::score(&set, &strict).unwrap();
    assert_eq!(s.correct, s.vectors);
    // A decoder that accepts everything drops what it does not know without a word.
    let silent: String = std::iter::once("id,result".to_string())
        .chain(set.vectors.iter().map(|v| format!("{},accept", v.id)))
        .collect::<Vec<_>>()
        .join("\n");
    let s = id::score(&set, &silent).unwrap();
    assert_eq!(s.silently_accepted.len(), flags);
    // Profiles must name their source; the template is all placeholders and is refused.
    let t: Profile = toml::from_str(&id::template("x")).unwrap();
    assert!(t.validate().is_err());
}

#[test]
fn cli_template_is_free_and_check_alarms() {
    let d = std::env::temp_dir().join(format!("b1604-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let home = d.join("home");
    let run = |a: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(a)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let (ok, out) = run(&["iface", "template", "s"]);
    assert!(ok && out.contains("PLACEHOLDER"));
    std::fs::write(d.join("p.toml"), toml::to_string(&profile()).unwrap()).unwrap();
    std::fs::write(
        d.join("r.csv"),
        records("2026-10-02T00:00:00Z,synth-l6e-ppp,4073,1,G|C,193,vendor_id=5\n"),
    )
    .unwrap();
    let (p, r) = (d.join("p.toml"), d.join("r.csv"));
    let (ok, _) = run(&["iface", "check", p.to_str().unwrap(), r.to_str().unwrap()]);
    assert!(!ok, "needs a licence");
    let (ok, out) = run(&[
        "iface",
        "check",
        p.to_str().unwrap(),
        r.to_str().unwrap(),
        "--dev",
    ]);
    assert!(!ok, "a drift alarm exits non-zero");
    assert!(out.contains("GNSS id C"), "{out}");
}
