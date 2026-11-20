// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Road, rail, automotive, eCall and 3GPP evidence (research 2026-11-20 gaps E30: B3001-B3005)
//! on SYNTHETIC records built here: telematics streams with a jamming episode and a spoofed
//! drag, on-board-unit test runs, eCall laboratory results with NMEA logs, emergency-call
//! records, PL logs by triggering condition, a decoded CAM log, gNB layouts, integrity
//! vectors and virtual-balise passages; signed packs fed to the EN 50129, ISO 26262 and
//! 2017/79 pre-assessment catalogues.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::transport::{self, auto, ecall, integrity, nr, rail, telematics, Table};

const T0: f64 = 1_795_089_600.0;
const SALT: &str = "operator-salt-2026";

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e30-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn noise(i: u64) -> f64 {
    let x = i
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

fn key(d: &Path) -> (PathBuf, String) {
    let seed = [21u8; 32];
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let k = d.join("key.txt");
    std::fs::write(&k, format!("private_key={}\n", hex(&seed))).unwrap();
    (k, hex(&kshana_pro::license::public_key_for(&seed)))
}

fn run(home: &Path, args: &[&str]) -> (bool, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
        .output()
        .unwrap();
    (
        o.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

fn json(p: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A 1 Hz drive north at 20 m/s; `jam` and `drag` windows in seconds from the start.
fn stream(
    vehicle: &str,
    n: usize,
    jam: Option<(usize, usize)>,
    drag: Option<(usize, usize)>,
) -> String {
    let mut s = String::from(
        "t,vehicle,lat,lon,speed_mps,wheel_mps,cn0_dbhz,ignition,truth_lat,truth_lon\n",
    );
    for i in 0..n {
        let (tl, tn) = transport::offset_deg(52.5, 13.4, 0.0, 20.0 * i as f64);
        let mut fix = Some((tl, tn));
        let mut speed = 20.0 + 0.2 * noise(i as u64);
        let mut cn0 = 42.0 + noise(i as u64 + 7);
        if let Some((a, b)) = jam {
            if (a..b).contains(&i) {
                fix = None;
                cn0 = 24.0;
            }
        }
        if let Some((a, b)) = drag {
            if (a..b).contains(&i) {
                // The spoofed track drifts east at 3 m/s and reports the wrong speed.
                let k = (i - a) as f64;
                fix = Some(transport::offset_deg(tl, tn, 3.0 * k, 0.0));
                speed = 26.0;
                cn0 = 49.0;
            }
        }
        let (la, lo) = fix
            .map(|(a, b)| (format!("{a:.8}"), format!("{b:.8}")))
            .unwrap_or_default();
        s.push_str(&format!(
            "{},{vehicle},{la},{lo},{},{:.2},{cn0:.1},1,{tl:.8},{tn:.8}\n",
            T0 + i as f64,
            if fix.is_some() {
                format!("{speed:.2}")
            } else {
                String::new()
            },
            20.0
        ));
    }
    s
}

#[test]
fn road_check_finds_jamming_and_spoofing_and_pseudonymises() {
    let d = tmp("road");
    let mut csv = stream("TRUCK-0042", 1800, Some((300, 800)), None);
    csv.push_str(
        stream("VAN-7", 1800, None, Some((900, 1200)))
            .lines()
            .skip(1)
            .map(|l| format!("{l}\n"))
            .collect::<String>()
            .as_str(),
    );
    std::fs::write(d.join("stream.csv"), csv).unwrap();
    std::fs::write(
        d.join("check.toml"),
        format!(
            "title = \"Fleet week 47 (synthetic)\"\nsalt = \"{SALT}\"\nstream = \"stream.csv\"\n"
        ),
    )
    .unwrap();
    let req: telematics::CheckRequest =
        toml::from_str(&std::fs::read_to_string(d.join("check.toml")).unwrap()).unwrap();
    let r = telematics::check(&req, &d).unwrap();
    assert_eq!(r.vehicles.len(), 2);
    let all: Vec<&telematics::Episode> = r.vehicles.iter().flat_map(|v| &v.episodes).collect();
    let jam: Vec<_> = all
        .iter()
        .filter(|e| e.verdict == "jamming indicated")
        .collect();
    let spoof: Vec<_> = all
        .iter()
        .filter(|e| e.verdict == "spoofing indicated")
        .collect();
    assert_eq!(jam.len(), 1, "{:#?}", all);
    assert_eq!(jam[0].class.as_deref(), Some("travels-with-vehicle"));
    assert!((jam[0].features.wheel_km - 10.0).abs() < 0.5);
    assert_eq!(spoof.len(), 1, "{:#?}", all);
    assert!(spoof[0].kinds.contains(&"speed_mismatch".to_string()));
    assert!(spoof[0].kinds.contains(&"cn0_rise".to_string()));
    // Trips: the jammed truck's trip is untrusted, the clean parts trusted.
    assert!(r
        .vehicles
        .iter()
        .any(|v| v.trips.iter().any(|t| t.trust == "untrusted")));
    // Privacy: no raw id, no salt, cells only.
    let text = transport::rounded_json(&r);
    assert!(!text.contains("TRUCK-0042") && !text.contains("VAN-7") && !text.contains(SALT));
    assert!(!text.contains("52.500"), "no precise coordinates");
    assert_eq!(r.vehicles[0].vehicle.len(), 12);
    // The privacy rule refuses a short salt and a fine grid.
    let weak = telematics::CheckRequest {
        salt: "x".into(),
        ..req.clone()
    };
    assert!(telematics::check(&weak, &d).is_err());
    let fine = telematics::CheckRequest {
        cell_deg: 0.001,
        ..req
    };
    assert!(telematics::check(&fine, &d).is_err());

    // CLI: Pro feature refused without a licence; runs with --dev.
    let home = d.join("home");
    let p = d.join("check.toml");
    let (ok, out) = run(&home, &["road", "check", p.to_str().unwrap()]);
    assert!(!ok, "{out}");
    let (ok, out) = run(
        &home,
        &[
            "road",
            "check",
            p.to_str().unwrap(),
            "--dev",
            "--json",
            d.join("r.json").to_str().unwrap(),
            "--html",
            d.join("r.html").to_str().unwrap(),
        ],
    );
    assert!(ok, "{out}");
    assert_eq!(json(&d.join("r.json"))["format"], telematics::FORMAT);
    let (ok, out) = run(&home, &["road", "library"]);
    assert!(ok && out.contains("travels-with-vehicle"), "{out}");
}

#[test]
fn obu_evaluation_signed_and_pre_assessed() {
    let d = tmp("obu");
    std::fs::write(d.join("clean.csv"), stream("OBU", 3600, None, None)).unwrap();
    std::fs::write(
        d.join("jam.csv"),
        stream("OBU", 1800, Some((600, 1200)), None),
    )
    .unwrap();
    std::fs::write(
        d.join("drag.csv"),
        stream("OBU", 1800, None, Some((600, 1000))),
    )
    .unwrap();
    // A unit that does not notice a slow spoof: no speed, no C/N0 columns.
    let quiet: String = stream("OBU", 1800, None, Some((600, 1800)))
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            format!("{},{},{},{},{},{}\n", f[0], f[2], f[3], f[7], f[8], f[9])
        })
        .collect();
    std::fs::write(d.join("quiet.csv"), quiet).unwrap();
    let req = format!(
        r#"title = "OBU-X GNSS attack runs (synthetic)"
unit = "OBU-X 2.1"
salt = "{SALT}"
[[runs]]
name = "clean"
stream = "clean.csv"
attack = "none"
[[runs]]
name = "jam"
stream = "jam.csv"
attack = "jamming"
start = {}
end = {}
[[runs]]
name = "drag"
stream = "drag.csv"
attack = "spoofing-drag"
start = {}
end = {}
[[runs]]
name = "drag-no-sensors"
stream = "quiet.csv"
attack = "spoofing-drag"
start = {}
end = {}
"#,
        T0 + 600.0,
        T0 + 1200.0,
        T0 + 600.0,
        T0 + 1000.0,
        T0 + 600.0,
        T0 + 1799.0
    );
    std::fs::write(d.join("obu.toml"), &req).unwrap();
    let r: telematics::ObuRequest = toml::from_str(&req).unwrap();
    let rep = telematics::evaluate_obu(&r, &d, "2026-11-20T00:00:00Z").unwrap();
    assert_eq!(rep.attack_runs, 3);
    assert_eq!(rep.detected, 2, "{}", rep.verdict);
    let jam = &rep.runs[1];
    assert!(jam.unreported_share.unwrap() > 0.3, "{jam:?}");
    assert_eq!(rep.runs[0].false_episodes, 0);
    assert!(rep.runs[0].unreported_share.unwrap().abs() < 0.01);
    assert_eq!(rep.runs[3].detected, Some(false));

    let (k, pubhex) = key(&d);
    let home = d.join("home");
    let out = d.join("pack");
    let (ok, o) = run(
        &home,
        &[
            "road",
            "obu-eval",
            d.join("obu.toml").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "Lab",
            "--key-file",
            k.to_str().unwrap(),
            "--utc",
            "2026-11-20T00:00:00Z",
            "--dev",
        ],
    );
    assert!(ok, "{o}");
    let (ok, o) = run(
        &home,
        &[
            "dossier",
            "verify",
            out.to_str().unwrap(),
            "--trust",
            &pubhex,
        ],
    );
    assert!(ok, "{o}");
    assert_eq!(
        json(&out.join("report.json"))["format"],
        telematics::OBU_FORMAT
    );
    let text = std::fs::read_to_string(out.join("report.json")).unwrap();
    assert!(!text.contains("certif") && !text.contains("complian"));
}

fn nmea_log(n: usize, with_vtg: bool) -> String {
    let mk = |body: String| {
        let cs = body.bytes().fold(0u8, |a, b| a ^ b);
        format!("${body}*{cs:02X}\n")
    };
    let mut s = String::new();
    for i in 0..n {
        let (h, m, sec) = (12, i / 60, i % 60);
        let t = format!("{h:02}{m:02}{sec:02}.00");
        s.push_str(&mk(format!(
            "GNRMC,{t},A,5200.0000,N,01300.0000,E,0.0,0.0,201126,,,D"
        )));
        s.push_str(&mk(format!(
            "GNGGA,{t},5200.0000,N,01300.0000,E,2,14,0.8,40.0,M,40.0,M,,"
        )));
        if with_vtg {
            s.push_str(&mk("GNVTG,0.0,T,,M,0.0,N,0.0,K,D".into()));
        }
        s.push_str(&mk("GNGSA,A,3,01,02,03,04,05,06,,,,,,,1.5,0.8,1.2".into()));
        s.push_str(&mk(
            "GPGSV,1,1,04,01,40,083,46,02,17,308,41,03,07,344,39,04,22,228,45".into(),
        ));
    }
    s
}

#[test]
fn ecall_suite_scoring_and_mapping() {
    assert!(ecall::mapping_problems().is_empty());
    let d = tmp("ecall");
    let mut res = String::from("title = \"AECS-1 GNSS runs (synthetic)\"\nunit = \"AECS-1\"\nlaboratory = \"Synthetic lab\"\ntrusted_error_m = 50.0\n[extension_limits]\n\"ks-trusted-error\" = 0.01\n");
    for (si, id) in ["S1", "S2", "S3"].iter().enumerate() {
        std::fs::write(d.join(format!("{id}.nmea")), nmea_log(120, si != 2)).unwrap();
        let mut err = String::from("err_h_m\n");
        let mut shadow = String::from("err_h_m\n");
        let mut trust = String::from("err_h_m,trusted\n");
        for i in 0..3600u64 {
            err.push_str(&format!(
                "{:.2}\n",
                4.0 + 3.0 * noise(i + 100 * si as u64).abs()
            ));
            // S2 is poor in the shadow test: p95 about 45 m.
            let base = if si == 1 { 30.0 } else { 12.0 };
            shadow.push_str(&format!("{:.2}\n", base + 15.0 * noise(i + 7).abs()));
            let spoofed = (1000..1300).contains(&i);
            trust.push_str(&format!(
                "{:.2},{}\n",
                if spoofed { 400.0 } else { 5.0 },
                u8::from(!spoofed || si == 0)
            ));
        }
        std::fs::write(d.join(format!("{id}-static.csv")), &err).unwrap();
        std::fs::write(d.join(format!("{id}-shadow.csv")), &shadow).unwrap();
        std::fs::write(d.join(format!("{id}-trust.csv")), &trust).unwrap();
        let ttff: Vec<String> = (0..10).map(|k| format!("{:.1}", 32.0 + k as f64)).collect();
        // S3 ran the -140 dBm test only 5 times.
        let n140 = if si == 2 { 5 } else { 10 };
        let ttff140: Vec<String> = (0..n140)
            .map(|k| format!("{:.1}", 150.0 + k as f64))
            .collect();
        res.push_str(&format!(
            "[[sample]]\nid = \"{id}\"\nnmea = \"{id}.nmea\"\n[sample.errors]\n\"static-combined\" = \"{id}-static.csv\"\n\"static-galileo\" = \"{id}-static.csv\"\n\"static-gps\" = \"{id}-static.csv\"\n\"dynamic\" = \"{id}-static.csv\"\nshadow = \"{id}-shadow.csv\"\n\"ks-trusted-error\" = \"{id}-trust.csv\"\n[sample.values]\n\"ttff-130\" = [{}]\n\"ttff-140\" = [{}]\nreacq = [{}]\n\"sens-cold-144\" = [1800.0]\n\"sens-track-155\" = [600.0]\n\"sens-reacq-150\" = [42.0]\n",
            ttff.join(", "),
            ttff140.join(", "),
            (0..10).map(|k| format!("{:.1}", 8.0 + k as f64 * 0.5)).collect::<Vec<_>>().join(", ")
        ));
    }
    std::fs::write(d.join("results.toml"), &res).unwrap();
    let r: ecall::Results = toml::from_str(&res).unwrap();
    let rep = ecall::score(&r, &d, "2026-11-20T00:00:00Z").unwrap();
    let st = |id: &str| {
        rep.tests
            .iter()
            .find(|t| t.id == id)
            .unwrap()
            .status
            .clone()
    };
    assert_eq!(st("static-combined"), "within the stated limit");
    assert!(st("nmea").starts_with("beyond"), "{}", st("nmea"));
    assert!(st("shadow").starts_with("beyond"), "{}", st("shadow"));
    assert!(
        st("ttff-140").starts_with("not assessed: fewer than 10 repeats for S3"),
        "{}",
        st("ttff-140")
    );
    assert_eq!(st("ttff-130"), "within the stated limit");
    assert_eq!(st("sens-track-155"), "within the stated limit");
    assert_eq!(st("reacq"), "within the stated limit");
    assert!(
        st("ks-trusted-error").starts_with("beyond"),
        "S1 kept trusting a spoofed position"
    );
    assert!(st("ks-jam-recovery").starts_with("not assessed"));
    let wgs = rep.requirements.iter().find(|q| q.para == "1.2.4").unwrap();
    assert_eq!(wgs.status, "not tested by Kshana");
    assert!(rep.verdict.contains("Pre-assessment, not a type-approval"));
    let text = transport::rounded_json(&rep).to_lowercase();
    assert!(!text.contains("compliant") && !text.contains("certified"));

    // Signed through the CLI, then pre-assessed against the 2017/79 catalogue.
    let (k, pubhex) = key(&d);
    let home = d.join("home");
    let pack = d.join("pack");
    let (ok, o) = run(
        &home,
        &[
            "ecall",
            "score",
            d.join("results.toml").to_str().unwrap(),
            "--out",
            pack.to_str().unwrap(),
            "--signer",
            "Lab",
            "--key-file",
            k.to_str().unwrap(),
            "--utc",
            "2026-11-20T00:00:00Z",
            "--dev",
        ],
    );
    assert!(ok, "{o}");
    let pa = d.join("pa");
    let (ok, o) = run(
        &home,
        &[
            "regpack",
            "assess",
            "eu-2017-79-ecall-gnss",
            "--evidence",
            pack.to_str().unwrap(),
            "--out",
            pa.to_str().unwrap(),
            "--signer",
            "Lab",
            "--key-file",
            k.to_str().unwrap(),
            "--trust",
            &pubhex,
            "--utc",
            "2026-11-20T00:00:00Z",
            "--dev",
        ],
    );
    assert!(ok, "{o}");
    let p = json(&pa.join("pre-assessment.json"));
    let rows = p["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 11);
    let row = |id: &str| {
        rows.iter().find(|r| r["id"] == id).unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(row("ANNEXVI-1.2.5"), "evidence supplied");
    assert_ne!(row("ANNEXVI-1.2.4"), "evidence supplied");

    // Free listings.
    let (ok, o) = run(&home, &["ecall", "mapping"]);
    assert!(ok && o.contains("1.2.9") && o.contains("R144"), "{o}");
    let (ok, o) = run(&home, &["ecall", "suite", "--json"]);
    assert!(ok && o.contains("sens-track-155"), "{o}");
}

#[test]
fn emergency_location_duty_and_battery() {
    let d = tmp("loc");
    let mut s = String::from("id,source,radius_m,confidence,err_m,interference\n");
    for i in 0..400u64 {
        let jam = i % 4 == 0;
        // GNSS: honest without interference (about 92 % within at 68 %... deliberately
        // conservative), optimistic under interference.
        let (src, rad, err) = if i % 2 == 0 {
            (
                "gnss",
                10.0,
                if jam {
                    40.0 + 20.0 * noise(i).abs()
                } else {
                    6.0 * noise(i).abs() + 2.0
                },
            )
        } else {
            ("wifi", 50.0, 30.0 + 40.0 * noise(i).abs())
        };
        s.push_str(&format!(
            "call{i},{src},{rad},68,{err:.1},{}\n",
            u8::from(jam)
        ));
    }
    std::fs::write(d.join("calls.csv"), &s).unwrap();
    let r = ecall::location(&Table::parse(&s).unwrap(), "PSAP Q4 (synthetic)").unwrap();
    let g = |src: &str, i: &str| {
        r.groups
            .iter()
            .find(|g| g.source == src && g.interference == i)
            .unwrap()
    };
    assert_eq!(
        g("gnss", "interference flagged").reliability,
        "over-confident"
    );
    assert_eq!(
        g("gnss", "no interference flag").reliability,
        "under-confident"
    );
    let home = d.join("home");
    let (ok, o) = run(
        &home,
        &[
            "ecall",
            "location",
            d.join("calls.csv").to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok && !o.contains("call17"), "{o}");

    let mut log = String::from("t,lat,lon\n");
    for c in 0..30 {
        for k in 0..4 {
            log.push_str(&format!("{},52.0,13.0\n", T0 + c as f64 * 600.0 + k as f64));
        }
    }
    std::fs::write(d.join("tracker.csv"), log).unwrap();
    let (ok, o) = run(
        &home,
        &["ecall", "duty", d.join("tracker.csv").to_str().unwrap()],
    );
    assert!(ok && o.contains("duty-cycled (regular)"), "{o}");
    std::fs::write(
        d.join("battery.toml"),
        "capacity_wh = 7.4\nactive_w = 0.12\nsleep_w = 0.00002\nintervals_s = [60.0, 600.0, 21600.0]\nassisted_ttff_s = 5.0\nassist_j = 0.5\n",
    )
    .unwrap();
    let (ok, o) = run(
        &home,
        &[
            "ecall",
            "battery",
            d.join("battery.toml").to_str().unwrap(),
            "--json",
        ],
    );
    assert!(ok && o.contains("assisted"), "{o}");
}

fn pl_csv(d: &Path) {
    let mut s = String::from("t,err_m,pl_m,condition\n");
    let conds = ["open-sky", "urban-canyon", "tunnel-exit", "jamming"];
    for i in 0..40_000u64 {
        let c = conds[(i / 10_000) as usize];
        let mut err = 1.0 + 1.5 * noise(i).abs();
        let pl = 6.0;
        if c == "urban-canyon" && i % 2500 == 0 {
            err = 7.5; // MI
        }
        if c == "tunnel-exit" && i == 25_000 {
            err = 30.0; // HMI against AL 25
        }
        s.push_str(&format!("{},{err:.3},{pl},{c}\n", T0 + i as f64));
    }
    std::fs::write(d.join("pl.csv"), s).unwrap();
}

#[test]
fn pl_validation_v2x_and_injection() {
    let d = tmp("auto");
    pl_csv(&d);
    std::fs::write(
        d.join("pl.toml"),
        "title = \"ADAS GNSS PL (synthetic)\"\ndata = \"pl.csv\"\nal_m = 25.0\ndecorrelation_s = 10.0\ntarget_ir = 5e-3\n",
    )
    .unwrap();
    let req: auto::PlRequest =
        toml::from_str(&std::fs::read_to_string(d.join("pl.toml")).unwrap()).unwrap();
    let r = auto::validate_pl(&req, &d).unwrap();
    let c = |id: &str| r.conditions.iter().find(|c| c.id == id).unwrap();
    assert_eq!(c("open-sky").status, "supports the target");
    assert_eq!(c("urban-canyon").status, "MI observed");
    assert_eq!(c("tunnel-exit").stats.as_ref().unwrap().hmi, 1);
    assert_eq!(c("spoofing").status, "no data");
    assert!((r.coverage - 4.0 / auto::CATALOGUE.len() as f64).abs() < 1e-9);
    assert_eq!(r.overall.mi, 4);

    // V2X: honest ellipse outside the attack, misleading inside, detected after 12 s.
    let mut cam = String::from("t,gen_time,lat,lon,truth_lat,truth_lon,conf_major_m,conf_minor_m,conf_orient_deg,spoof_flag\n");
    for i in 0..1200u64 {
        let t = T0 + i as f64;
        let (tl, tn) = transport::offset_deg(48.1, 11.6, 15.0 * i as f64, 0.0);
        let attacked = (600..900).contains(&i);
        let (e, n) = if attacked {
            (2.0 * (i - 600) as f64, 0.0)
        } else {
            (1.5 * noise(i), 1.5 * noise(i + 3))
        };
        let (la, lo) = transport::offset_deg(tl, tn, e, n);
        let gen = if attacked { t + 2.0 } else { t + 0.05 };
        let flag = u8::from(attacked && i >= 612);
        cam.push_str(&format!(
            "{t},{gen},{la:.8},{lo:.8},{tl:.8},{tn:.8},4.0,4.0,0,{flag}\n"
        ));
    }
    std::fs::write(d.join("cam.csv"), cam).unwrap();
    std::fs::write(
        d.join("v2x.toml"),
        format!("title = \"OBU-V2X (synthetic)\"\nunit = \"OBU-V2X\"\nlog = \"cam.csv\"\ntime_tol_s = 0.5\ndetect_within_s = 10.0\n[[attack]]\nkind = \"drag\"\nstart = {}\nend = {}\n", T0 + 600.0, T0 + 899.0),
    )
    .unwrap();
    let vr: auto::V2xRequest =
        toml::from_str(&std::fs::read_to_string(d.join("v2x.toml")).unwrap()).unwrap();
    let v = auto::v2x(&vr, &d).unwrap();
    assert_eq!(v.checks[0].status, "consistent with 95 % confidence");
    assert!(v.checks[1].value.unwrap() > 0.9);
    assert_eq!(v.checks[2].status, "time misled during an attack");
    assert_eq!(v.checks[3].status, "every attack flagged, some late");
    assert!((v.checks[3].value.unwrap() - 12.0).abs() < 1e-9);

    // Signed PL pack and V2X pack, pre-assessed against the ISO 26262/SOTIF catalogue.
    let (k, pubhex) = key(&d);
    let home = d.join("home");
    let mut dirs = Vec::new();
    for (cmd, f) in [("pl", "pl.toml"), ("v2x", "v2x.toml")] {
        let out = d.join(format!("pack-{cmd}"));
        let (ok, o) = run(
            &home,
            &[
                "auto",
                cmd,
                d.join(f).to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--signer",
                "OEM",
                "--key-file",
                k.to_str().unwrap(),
                "--utc",
                "2026-11-20T00:00:00Z",
                "--dev",
            ],
        );
        assert!(ok, "{o}");
        dirs.push(out.to_str().unwrap().to_string());
    }
    let pa = d.join("pa");
    let (ok, o) = run(
        &home,
        &[
            "regpack",
            "assess",
            "iso26262-gnss-seooc",
            "--evidence",
            &dirs.join(","),
            "--out",
            pa.to_str().unwrap(),
            "--signer",
            "OEM",
            "--key-file",
            k.to_str().unwrap(),
            "--trust",
            &pubhex,
            "--utc",
            "2026-11-20T00:00:00Z",
            "--dev",
        ],
    );
    assert!(ok, "{o}");
    let p = json(&pa.join("pre-assessment.json"));
    let row = |id: &str| {
        p["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(row("SOTIF-VAL"), "evidence supplied");
    assert_eq!(row("V2X-POS"), "evidence supplied");
    assert_ne!(row("CORR"), "evidence supplied");

    // Fault injection on a recorded track.
    let mut rec = String::from("t,lat,lon,pl_m\n");
    for i in 0..100 {
        let (la, lo) = transport::offset_deg(48.1, 11.6, 0.0, 10.0 * i as f64);
        rec.push_str(&format!("{},{la:.9},{lo:.9},5.0\n", T0 + i as f64));
    }
    std::fs::write(d.join("rec.csv"), rec).unwrap();
    std::fs::write(
        d.join("inject.toml"),
        format!("input = \"rec.csv\"\nseed = 3\n[[fault]]\nkind = \"ramp\"\nstart = {}\nrate_mps = 1.0\nbearing_deg = 90.0\n[[fault]]\nkind = \"pl_scale\"\nstart = {}\nend = {}\nfactor = 0.5\n[[fault]]\nkind = \"dropout\"\nstart = {}\nend = {}\n", T0 + 50.0, T0 + 50.0, T0 + 70.0, T0 + 90.0, T0 + 95.0),
    )
    .unwrap();
    let outcsv = d.join("faulted.csv");
    let (ok, o) = run(
        &home,
        &[
            "auto",
            "inject",
            d.join("inject.toml").to_str().unwrap(),
            "--csv",
            outcsv.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok && o.contains("50 epoch(s) faulted"), "{o}");
    let f = Table::read(&outcsv).unwrap();
    let last = f.rows.len() - 1;
    assert!((f.num(last, f.col("offset_e_m")).unwrap().unwrap() - 49.0).abs() < 0.01);
    assert_eq!(f.num(60, f.col("pl_m")).unwrap(), Some(2.5));
    assert_eq!(f.num(92, f.col("lat")).unwrap(), None);
    let bad: auto::InjectRequest =
        toml::from_str("input = \"rec.csv\"\n[[fault]]\nkind = \"emit\"\nstart = 0.0\n").unwrap();
    assert!(auto::inject(&bad, &d).is_err());
}

#[test]
fn nr_tdd_prs_and_vectors() {
    let d = tmp("nr");
    let tdd = r#"title = "gNB-17 (synthetic)"
numerology = 1
guard_symbols = 2
interferer_km = 10.0
[holdover]
site = "gNB-17"
period_h = 24
initial_te_s = 0.0
u_initial_te_s = 5e-8
[holdover.oscillator]
class = "ocxo"
"#;
    let tr: nr::TddRequest = toml::from_str(tdd).unwrap();
    let r = nr::tdd(&tr, "2026-11-20T00:00:00Z").unwrap();
    assert!((r.guard_period_s - 71.43e-6).abs() < 1e-7);
    assert!((r.interference_te_s - (71.43e-6 - 10_000.0 / nr::C) / 2.0).abs() < 1e-7);
    let b = r
        .budget_exceeded_after_s
        .expect("an OCXO passes ±1.5 µs within a day");
    if let Some(i) = r.interference_after_s {
        assert!(i > b, "interference after the budget");
    }
    assert!(b > 3600.0 && b < 86_400.0, "{b}");
    // A TCXO loses the budget sooner.
    let mut tc = tr.clone();
    tc.holdover.oscillator.class = Some("tcxo".into());
    let r2 = nr::tdd(&tc, "2026-11-20T00:00:00Z").unwrap();
    assert!(r2.budget_exceeded_after_s.unwrap() < b);
    assert!(r2.interference_after_s.unwrap() > r2.budget_exceeded_after_s.unwrap());

    let prs = |bw: f64, sync: f64| -> f64 {
        let req = nr::PrsRequest {
            title: "t".into(),
            bandwidth_mhz: bw,
            symbols: 12,
            sync_sigma_s: sync,
            extra_sigma_m: 0.0,
            gnb: (0..4)
                .map(|k| {
                    let a = k as f64 * std::f64::consts::FRAC_PI_2 + 0.3;
                    nr::Gnb {
                        name: format!("g{k}"),
                        east_m: 500.0 * a.cos(),
                        north_m: 500.0 * a.sin(),
                        snr_db: 0.0,
                        sync_sigma_s: None,
                    }
                })
                .collect(),
            users: vec![[10.0, 20.0]],
        };
        nr::prs(&req).unwrap().points[0].two_drms_m.unwrap()
    };
    assert!(prs(20.0, 0.0) > 4.0 * prs(100.0, 0.0));
    // 50 ns of gNB time error dominates a 100 MHz PRS.
    assert!(prs(100.0, 50e-9) > 10.0 * prs(100.0, 0.0));

    // Vectors and a device that reports a tighter PL on two cases.
    let home = d.join("home");
    let vf = d.join("vectors.json");
    let (ok, o) = run(&home, &["nr", "vectors", "--out", vf.to_str().unwrap()]);
    assert!(ok, "{o}");
    let set: nr::VectorSet = serde_json::from_str(&std::fs::read_to_string(&vf).unwrap()).unwrap();
    let mut dev = String::from("case,hpl_m,available\n");
    for (i, c) in set.cases.iter().enumerate() {
        let h = if i < 2 {
            c.expected_hpl_m * 0.8
        } else {
            c.expected_hpl_m + 0.004
        };
        dev.push_str(&format!(
            "{},{h:.4},{}\n",
            c.id,
            u8::from(c.expected_available)
        ));
    }
    std::fs::write(d.join("dev.csv"), dev).unwrap();
    let cr = nr::check_vectors(&set, &Table::read(&d.join("dev.csv")).unwrap()).unwrap();
    assert_eq!(cr.agree, set.cases.len() - 2);
    assert_eq!(cr.cases[0].result, "optimistic HPL");
    let (ok, o) = run(
        &home,
        &[
            "nr",
            "check",
            vf.to_str().unwrap(),
            d.join("dev.csv").to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok && o.contains("2 optimistic"), "{o}");
    assert!(set.basis.contains("Not the 3GPP conformance cases"));
}

#[test]
fn rail_thr_balise_and_sil4_template() {
    let d = tmp("rail");
    std::fs::write(
        d.join("thr.toml"),
        r#"title = "VB position function, line 7 (synthetic)"
thr_per_hour = 1e-9
alert_limit_m = 25.0
sigma_m = 2.5
samples_per_hour = 120.0
fault_per_hour = 1e-4
fault_p_md = 1e-6
[jamming]
rate_per_hour = 0.02
p_undetected = 1e-4
error_sigma_m = 40.0
"#,
    )
    .unwrap();
    let req: rail::ThrRequest =
        toml::from_str(&std::fs::read_to_string(d.join("thr.toml")).unwrap()).unwrap();
    let r = rail::thr(&req, "2026-11-20T00:00:00Z").unwrap();
    assert!(!r.within, "{}", r.verdict);
    assert!(
        r.verdict.contains("largest contribution: jamming"),
        "{}",
        r.verdict
    );
    // Better jamming detection brings it within.
    let mut better = req.clone();
    better.jamming.as_mut().unwrap().p_undetected = 1e-8;
    assert!(rail::thr(&better, "2026-11-20T00:00:00Z").unwrap().within);

    // Passages: errors ~ N(0.4, 1.0) m from timing at 30 m/s, claimed sigma 1.0 (bounds) .
    let mut p = String::from("balise,truth_time,reported_time,speed_mps,sigma_m\n");
    let n = 2000;
    for i in 0..n {
        let z = transport::norm_quantile((i as f64 + 0.5) / n as f64);
        let e = 0.4 + z * 0.8;
        p.push_str(&format!(
            "VB{},{:.3},{:.6},30.0,1.0\n",
            i % 40,
            T0 + i as f64 * 60.0,
            T0 + i as f64 * 60.0 + e / 30.0
        ));
    }
    std::fs::write(d.join("passages.csv"), &p).unwrap();
    std::fs::write(
        d.join("balise.toml"),
        "title = \"VB line 7\"\npassages = \"passages.csv\"\nalert_limit_m = 5.0\n",
    )
    .unwrap();
    let br: rail::BaliseRequest =
        toml::from_str("title = \"VB\"\npassages = \"passages.csv\"\nalert_limit_m = 5.0\n")
            .unwrap();
    let b = rail::balise(&br, &d, "2026-11-20T00:00:00Z").unwrap();
    assert_eq!(b.passages, n);
    assert_eq!(b.balises, 40);
    assert!((b.mean_m - 0.4).abs() < 0.01, "{}", b.mean_m);
    assert!(b.overbound.as_ref().unwrap().bounded, "{:?}", b.overbound);
    assert_eq!(b.exceedances, Some(0));

    let (k, pubhex) = key(&d);
    let home = d.join("home");
    let mut dirs = Vec::new();
    for (cmd, f) in [("thr", "thr.toml"), ("balise", "balise.toml")] {
        let out = d.join(format!("pack-{cmd}"));
        let (ok, o) = run(
            &home,
            &[
                "rail",
                cmd,
                d.join(f).to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--signer",
                "Supplier",
                "--key-file",
                k.to_str().unwrap(),
                "--utc",
                "2026-11-20T00:00:00Z",
                "--dev",
            ],
        );
        assert!(ok, "{o}");
        dirs.push(out.to_str().unwrap().to_string());
    }
    let pa = d.join("pa");
    let (ok, o) = run(
        &home,
        &[
            "regpack",
            "assess",
            "rail-gnss-sil4",
            "--evidence",
            &dirs.join(","),
            "--out",
            pa.to_str().unwrap(),
            "--signer",
            "Supplier",
            "--key-file",
            k.to_str().unwrap(),
            "--trust",
            &pubhex,
            "--utc",
            "2026-11-20T00:00:00Z",
            "--dev",
        ],
    );
    assert!(ok, "{o}");
    let pj = json(&pa.join("pre-assessment.json"));
    let row = |id: &str| {
        pj["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap()["status"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(row("RAIL-THR"), "evidence supplied");
    assert_eq!(row("RAIL-VB"), "evidence supplied");
    assert_ne!(row("RAIL-SRAC"), "evidence supplied");
    assert_ne!(row("RAIL-INDEP"), "evidence supplied");
}

#[test]
fn overbound_and_stanford_helpers() {
    let z: Vec<f64> = (0..5000)
        .map(|i| transport::norm_quantile((i as f64 + 0.5) / 5000.0) * 2.0)
        .collect();
    let o = integrity::overbound(&z);
    assert!(!o.bounded);
    assert!(
        (o.inflation_needed - 2.0).abs() < 0.5,
        "{}",
        o.inflation_needed
    );
    assert!(integrity::samples_needed(1e-7).unwrap() > 3.6e7);
}
