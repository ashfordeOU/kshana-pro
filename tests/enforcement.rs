// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Jammer enforcement evidence pack (research 2026-11-20 gaps B0605) on a SYNTHETIC case: the
//! timeline and victim effects, cessation at the device-off time, the affected area from victim
//! receivers only (no emitter position in or out), a sealed capture's custody chain, and the
//! checklist derived from published cases with its citations.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana::sdr::Cf64;
use kshana::sigmf::{DataType, Meta, Recording};
use kshana_pro::enforcement::{self, Request, CHECKLIST, SOURCES};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b0605-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn setup(d: &Path) -> Request {
    let (csv, req) = enforcement::synth();
    std::fs::write(d.join("victims.csv"), csv).unwrap();
    std::fs::write(d.join("request.toml"), &req).unwrap();
    toml::from_str(&req).unwrap()
}

/// A sealed capture of a SYNTHETIC recording, with one handover.
fn custody(d: &Path, seed: &[u8; 32]) -> PathBuf {
    let samples: Vec<Cf64> = (0..20_000)
        .map(|k| Cf64::new(if k >= 10_000 { 0.5 } else { 0.01 }, 0.0))
        .collect();
    let rec = Recording {
        meta: Meta::new(DataType::Ci16Le, 100_000.0, 1_575_420_000.0, "SYNTHETIC"),
        samples,
    };
    let (m, data, _) = kshana::sigmf::write(&rec, 1.0).unwrap();
    std::fs::write(d.join("src.sigmf-meta"), m).unwrap();
    std::fs::write(d.join("src.sigmf-data"), data).unwrap();
    let q: kshana_pro::custody::Request = toml::from_str(
        "title = \"SYNTHETIC capture\"\nmeta = \"src.sigmf-meta\"\ndata = \"src.sigmf-data\"\n\
         pre_s = 0.05\npost_s = 0.05\ncalibration_id = \"CAL-SYNTH\"\ncollected_by = \"Officer A\"\n\
         location = \"synthetic monitoring van\"\nreason = \"test\"\n[trigger]\nkind = \"sample\"\nsample = 10000\n",
    )
    .unwrap();
    let out = d.join("capture");
    kshana_pro::custody::capture(&q, d, &out, seed, "2026-06-01T10:30:00Z").unwrap();
    kshana_pro::custody::handover(
        &out,
        "Officer A",
        "Evidence store",
        "storage",
        seed,
        "2026-06-01T12:00:00Z",
    )
    .unwrap();
    out
}

#[test]
fn pack_supplies_the_victim_side_of_a_case() {
    let d = tmp("pack");
    let mut q = setup(&d);
    let seed = [3u8; 32];
    custody(&d, &seed);
    q.custody_dir = Some("capture".into());
    q.custody_trust = vec![hex(&kshana_pro::license::public_key_for(&seed))];
    let p = enforcement::build(&q, &d).unwrap();
    assert_eq!(p.victims_affected, 4);
    assert_eq!(
        p.max_concurrent_victims, 3,
        "GBAS, tracker and one aircraft at once"
    );
    assert!(p
        .victims
        .iter()
        .all(|v| v.ceased_with_device_off == Some(true)));
    let gbas = p.victims.iter().find(|v| v.victim_id == "GBAS-1").unwrap();
    assert_eq!(gbas.episodes.len(), 2);
    assert_eq!(gbas.affected_s, 450.0 + 1120.0);
    // Affected area: victim cells only; the reference station's cell is unaffected.
    assert!(p
        .affected_area
        .iter()
        .any(|c| c.receivers_unaffected == 1 && c.victims_affected == 0));
    let json = serde_json::to_string(&p).unwrap();
    for k in ["emitter_lat", "source_estimate", "bearing_deg"] {
        assert!(!json.contains(k), "{k}");
    }
    // Custody chain verified and on the timeline.
    let c = p.custody.as_ref().unwrap();
    assert!(c.verification.passed(), "{:?}", c.verification.problems);
    assert!(p
        .timeline
        .iter()
        .any(|e| e.text.contains("Custody handover")));
    // Checklist: every item cited to a published source; C1-C7 supplied, C8-C11 the authority's.
    assert_eq!(p.checklist.len(), CHECKLIST.len());
    for c in &p.checklist {
        assert!(!c.sources.is_empty());
        for s in &c.sources {
            let key = s.split(' ').next().unwrap();
            assert!(SOURCES.iter().any(|(k, _)| *k == key), "{s}");
        }
        let want = if ["C8", "C9", "C10", "C11"].contains(&c.id) {
            "for-the-authority"
        } else {
            "supplied"
        };
        assert_eq!(c.status, want, "{} {}", c.id, c.detail);
    }
}

#[test]
fn emitter_inputs_are_refused_and_gaps_are_marked() {
    let d = tmp("guard");
    let q = setup(&d);
    std::fs::write(
        d.join("victims.csv"),
        "victim_id,kind,start_utc,end_utc,lat,lon,effect,source,emitter_lat\nV,rx,2026-06-01T10:00:00Z,2026-06-01T10:01:00Z,51,0,degraded,x,51.1\n",
    )
    .unwrap();
    let e = enforcement::build(&q, &d).unwrap_err();
    assert!(e.contains("no emitter position"), "{e}");
    let d = tmp("gaps");
    let mut q = setup(&d);
    q.device_off_utc = None;
    let p = enforcement::build(&q, &d).unwrap();
    let st = |id: &str| p.checklist.iter().find(|c| c.id == id).unwrap().status;
    assert_eq!(st("C4"), "missing");
    assert_eq!(st("C5"), "missing");
    assert_eq!(st("C6"), "partial");
}

#[test]
fn cli_signs_the_pack() {
    let d = tmp("cli");
    let home = d.join("home");
    let seed = [4u8; 32];
    std::fs::write(d.join("key.txt"), format!("private_key={}\n", hex(&seed))).unwrap();
    let run = |a: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(a)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (ok, m) = run(&["enforcement", "synth", d.to_str().unwrap()]);
    assert!(ok, "{m}");
    let out = d.join("pack");
    let (r, kf) = (d.join("request.toml"), d.join("key.txt"));
    let args = [
        "enforcement",
        "pack",
        r.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        kf.to_str().unwrap(),
        "--utc",
        "2026-11-20T00:00:00Z",
    ];
    assert!(!run(&args).0, "needs a licence");
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(ok && m.contains("victim(s) affected"), "{m}");
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
