// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Space-operations signed evidence packs (research 2026-11-20 gaps B2205) on SYNTHETIC inputs:
//! the encounter-plane probability against its closed form, the decision record against the
//! operator's own policy, identification against candidate TLEs on the open SGP4, the disposal
//! lifetime check, and the signed operator pack (licence needed; a changed byte fails).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::space_ops as so;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b2205-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn synth(d: &Path) {
    for (n, t) in so::synth().unwrap() {
        std::fs::write(d.join(n), t).unwrap();
    }
}

fn cdm(r: [f64; 3], s1: f64, s2: f64, hbr: f64) -> so::Cdm {
    so::Cdm {
        message_id: "T".into(),
        creation_utc: "2026-11-17T00:00:00Z".into(),
        tca_utc: "2026-11-18T00:00:00Z".into(),
        rel_pos_rtn_m: r,
        rel_vel_rtn_mps: [0.0, -14_000.0, 0.0],
        cov1_rtn_m2: [s1 * s1, 0.0, 1e6, 0.0, 0.0, s1 * s1],
        cov2_rtn_m2: [s2 * s2, 0.0, 1e6, 0.0, 0.0, s2 * s2],
        hbr_m: hbr,
        pc_reported: None,
    }
}

#[test]
fn encounter_plane_probability_matches_the_closed_forms() {
    // Head-on miss of zero, isotropic combined sigma s in the plane: Pc = 1 - exp(-R²/2s²).
    for (s1, s2, hbr) in [(30.0, 40.0, 20.0), (100.0, 0.0, 10.0), (10.0, 10.0, 30.0)] {
        let s2c: f64 = s1 * s1 + s2 * s2;
        let want = 1.0 - (-(hbr * hbr) / (2.0 * s2c)).exp();
        let got = so::pc_2d(&cdm([0.0; 3], s1, s2, hbr)).unwrap();
        assert!((got / want - 1.0).abs() < 1e-3, "{got} vs {want}");
    }
    // Small body far off-centre: Pc ~ (R²/2s²) exp(-d²/2s²).
    let (s, hbr, d) = (100.0f64, 1.0f64, 250.0f64);
    let want = hbr * hbr / (2.0 * s * s) * (-(d * d) / (2.0 * s * s)).exp();
    let got = so::pc_2d(&cdm([d, 0.0, 0.0], s, 0.0, hbr)).unwrap();
    assert!((got / want - 1.0).abs() < 1e-2, "{got} vs {want}");
    // The along-track miss (along the relative velocity) does not count.
    let a = so::pc_2d(&cdm([50.0, 0.0, 0.0], 60.0, 0.0, 10.0)).unwrap();
    let b = so::pc_2d(&cdm([50.0, 5_000.0, 0.0], 60.0, 0.0, 10.0)).unwrap();
    assert!((a / b - 1.0).abs() < 1e-9);
    // No relative velocity, or no hard body, is refused.
    let mut c = cdm([0.0; 3], 1.0, 1.0, 1.0);
    c.rel_vel_rtn_mps = [0.0; 3];
    assert!(so::pc_2d(&c).is_err());
    assert!(so::pc_2d(&cdm([0.0; 3], 1.0, 1.0, 0.0)).is_err());
}

#[test]
fn decision_record_is_checked_against_the_operators_own_policy() {
    let d = tmp("ca");
    synth(&d);
    let q: so::CaRequest =
        toml::from_str(&std::fs::read_to_string(d.join("ca.toml")).unwrap()).unwrap();
    let r = so::ca_record(&q).unwrap();
    assert_eq!(r.format, so::CA_FORMAT);
    assert_eq!(r.cdms.len(), 4);
    assert_eq!(r.basis_cdm.as_deref(), Some("CDM-SYNTH-3"));
    assert!(r.basis_pc.unwrap() >= q.policy.pc_threshold);
    assert!((r.lead_time_h - 22.0).abs() < 1e-9);
    assert!(r.post_maneuver_pc.unwrap() < 1e-20);
    assert!(r.policy_findings.is_empty(), "{:?}", r.policy_findings);
    // The probability timeline rises to the decision.
    let pcs: Vec<f64> = r.cdms.iter().take(3).map(|c| c.pc_computed).collect();
    assert!(pcs[0] < pcs[1] && pcs[1] < pcs[2]);

    // No manoeuvre above the threshold, decided late, against a mismatching reported value.
    let mut late = q.clone();
    late.decision.action = "no-maneuver".into();
    late.decision.utc = "2026-11-18T00:00:00Z".into();
    late.post_maneuver_cdm = None;
    late.cdms[2].pc_reported = Some(1e-6);
    late.cdms.retain(|c| c.message_id != "CDM-SYNTH-4");
    let r = so::ca_record(&late).unwrap();
    let all = r.policy_findings.join(" | ");
    assert!(all.contains("at or above the stated threshold"), "{all}");
    assert!(all.contains("later than the stated 12 h"), "{all}");
    assert!(all.contains("factor of ten"), "{all}");
    assert!(r.verdict.contains("3 point(s) for review"));

    // A manoeuvre below the threshold asks for the reason; unknown actions are refused.
    let mut low = q.clone();
    low.policy.pc_threshold = 0.5;
    let r = so::ca_record(&low).unwrap();
    assert!(r
        .policy_findings
        .iter()
        .any(|f| f.contains("below the stated threshold")));
    let mut bad = q.clone();
    bad.decision.action = "ram".into();
    assert!(so::ca_record(&bad).is_err());
    bad = q;
    bad.decision.rationale = " ".into();
    assert!(so::ca_record(&bad).is_err());
}

#[test]
fn identification_and_disposal_checks() {
    let d = tmp("id");
    synth(&d);
    let cands: so::Candidates =
        toml::from_str(&std::fs::read_to_string(d.join("candidates.toml")).unwrap()).unwrap();
    let id = so::identify(
        &std::fs::read_to_string(d.join("ephemeris.csv")).unwrap(),
        &cands.candidate,
    )
    .unwrap();
    assert_eq!(id.best, "SYNTH-A");
    assert_eq!(id.fits[0].norad_id, "90001");
    assert!(id.fits[0].rms_km < 0.5, "{:?}", id.fits);
    assert!(id.margin > 50.0, "{}", id.margin);
    assert!(so::identify("timestamp,x_km,y_km,z_km\n", &cands.candidate).is_err());
    assert!(so::identify("timestamp,x_km,y_km,z_km\n", &cands.candidate[..1]).is_err());

    let q: so::DisposalRequest =
        toml::from_str(&std::fs::read_to_string(d.join("disposal.toml")).unwrap()).unwrap();
    let r = so::disposal(&q).unwrap();
    let y = r.lifetime_years.unwrap();
    assert!(y > 0.1 && y < q.plan_deadline_years, "{y}");
    assert_eq!(r.within_plan, Some(true));
    // Higher is longer; a heavy object high up does not decay within 200 years.
    let mut hi = q.clone();
    hi.altitude_km = 550.0;
    assert!(so::disposal(&hi).unwrap().lifetime_years.unwrap() > y);
    hi.altitude_km = 900.0;
    hi.mass_kg = 1000.0;
    let r = so::disposal(&hi).unwrap();
    assert_eq!(r.lifetime_years, None);
    assert_eq!(r.within_plan, Some(false));
    let mut bad = q;
    bad.plan_source = String::new();
    assert!(so::disposal(&bad).is_err());
}

#[test]
fn operator_pack_is_signed_and_needs_a_licence() {
    let d = tmp("cli");
    synth(&d);
    let home = d.join("home");
    let seed = [31u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={hex}\n")).unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
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
    };
    let out = d.join("pack");
    let (p, o, k) = (
        d.join("pack.toml"),
        out.to_str().unwrap().to_string(),
        key.to_str().unwrap().to_string(),
    );
    let p = p.to_str().unwrap();
    let (ok, m) = run(&[
        "space-ops",
        "pack",
        p,
        "--out",
        &o,
        "--signer",
        "T",
        "--key-file",
        &k,
    ]);
    assert!(!ok, "{m}");
    let (ok, m) = run(&[
        "space-ops",
        "pack",
        p,
        "--out",
        &o,
        "--signer",
        "T",
        "--key-file",
        &k,
        "--dev",
    ]);
    assert!(ok, "{m}");
    assert!(m.contains("0 open point(s)"), "{m}");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    assert_eq!(v["format"], so::PACK_FORMAT);
    assert!(v["framework_note"].as_str().unwrap().contains("to confirm"));
    for f in [
        "pack.toml",
        "ca-1.toml",
        "ephemeris.csv",
        "candidates.toml",
        "disposal.toml",
    ] {
        assert!(out.join("inputs").join(f).exists(), "{f}");
    }
    let html = std::fs::read_to_string(out.join("report.html"))
        .unwrap()
        .to_lowercase();
    for w in ["compliant", "conform", "certified"] {
        assert!(!html.contains(w), "{w}");
    }
    let pk = kshana_pro::license::public_key_for(&seed);
    assert!(kshana_pro::signed_dossier::verify(&out, &[pk])
        .unwrap()
        .passed());
    let mut b = std::fs::read(out.join("inputs").join("ca-1.toml")).unwrap();
    b[10] ^= 1;
    std::fs::write(out.join("inputs").join("ca-1.toml"), &b).unwrap();
    assert!(!kshana_pro::signed_dossier::verify(&out, &[pk])
        .map(|v| v.passed())
        .unwrap_or(false));
    // The single records run on their own.
    for (cmd, files) in [
        ("ca", vec!["ca.toml"]),
        ("identify", vec!["ephemeris.csv", "candidates.toml"]),
        ("disposal", vec!["disposal.toml"]),
    ] {
        let o = d.join(format!("out-{cmd}"));
        let mut a: Vec<String> = vec!["space-ops".into(), cmd.into()];
        a.extend(
            files
                .iter()
                .map(|f| d.join(f).to_str().unwrap().to_string()),
        );
        a.extend(
            [
                "--out",
                o.to_str().unwrap(),
                "--signer",
                "T",
                "--key-file",
                &k,
                "--dev",
            ]
            .map(String::from),
        );
        let a: Vec<&str> = a.iter().map(String::as_str).collect();
        let (ok, m) = run(&a);
        assert!(ok, "{cmd}: {m}");
        assert!(kshana_pro::signed_dossier::verify(&o, &[pk])
            .unwrap()
            .passed());
    }
}
// ---------------------------------------------------------------------------------------------
// Observation scheduling for space-debris sensors (planning only).

mod debris_obs {
    use kshana_pro::debris_obs::{self, Request};

    fn req() -> Request {
        toml::from_str(&debris_obs::synth()).unwrap()
    }

    fn plan(q: &Request) -> debris_obs::Plan {
        debris_obs::plan(q, std::path::Path::new(".")).unwrap()
    }

    fn t(s: &str) -> f64 {
        kshana_pro::forensics::inputs::parse_utc(s).unwrap()
    }

    #[test]
    fn windows_schedule_and_covariance_gain() {
        let q = req();
        let p = plan(&q);
        assert_eq!(p.windows.len(), 9);
        assert_eq!(p.schedule.len(), 6);
        // The low-inclination object never rises into the radar's sector at 50 deg N.
        let d4 = p
            .targets
            .iter()
            .find(|x| x.target == "SYNTH-DEB-4")
            .unwrap();
        assert_eq!(d4.scheduled, 0);
        assert!(d4
            .reason_unscheduled
            .as_deref()
            .unwrap()
            .contains("no visibility window"));
        // Every track lies inside a window of its target and clears the others by the slew time.
        for tr in &p.schedule {
            assert!(p.windows.iter().any(|w| w.target == tr.target
                && t(&w.start_utc) <= t(&tr.start_utc)
                && t(&tr.end_utc) <= t(&w.end_utc)));
            assert!(
                tr.posterior_pos_sigma_m < tr.prior_pos_sigma_m / 5.0,
                "{tr:?}"
            );
            assert_eq!(tr.measurements, 39, "13 epochs x (az, el, range)");
        }
        let mut iv: Vec<(f64, f64)> = p
            .schedule
            .iter()
            .map(|x| (t(&x.start_utc), t(&x.end_utc)))
            .collect();
        iv.sort_by(|a, b| a.0.total_cmp(&b.0));
        for w in iv.windows(2) {
            assert!(w[1].0 >= w[0].1 + q.sensor.slew_s - 1e-6);
        }
        // Noisier measurements give less improvement.
        let mut noisy = q.clone();
        noisy.sensor.sigma_angle_arcsec *= 10.0;
        noisy.sensor.sigma_range_m = Some(150.0);
        let n = plan(&noisy);
        assert!(n.schedule[0].posterior_pos_sigma_m > p.schedule[0].posterior_pos_sigma_m);
        assert!(p.evidence.starts_with("MODELLED") && p.evidence.contains("Planning only"));
    }

    #[test]
    fn optical_needs_darkness_and_requests_are_checked() {
        let mut q = req();
        q.sensor.kind = debris_obs::SensorKind::Optical;
        q.sensor.az_min_deg = None;
        q.sensor.az_max_deg = None;
        let p = plan(&q);
        // At 50 deg N, 7 deg E in November the Sun is 12 deg below the horizon only between
        // about 17:30 and 05:30 UTC: no optical window around local noon.
        for w in &p.windows {
            let h = (t(&w.start_utc) / 3600.0).rem_euclid(24.0);
            assert!(!(8.0..15.0).contains(&h), "{w:?}");
        }
        let mut bad = req();
        bad.sensor.sigma_range_m = None;
        assert!(
            debris_obs::plan(&bad, std::path::Path::new(".")).is_err(),
            "radar needs range noise"
        );
        let mut bad = req();
        bad.targets[0].tle_line1 = None;
        assert!(debris_obs::plan(&bad, std::path::Path::new(".")).is_err());
    }
}

// ---------------------------------------------------------------------------------------------
// Disposal-plan sensitivity: lifetime against solar activity and ballistic coefficient.

mod disposal_sensitivity {
    use kshana_pro::disposal_sensitivity::{self, Request};

    fn req() -> Request {
        toml::from_str(&disposal_sensitivity::synth()).unwrap()
    }

    #[test]
    fn lifetime_moves_with_solar_activity_and_ballistic_coefficient() {
        let s = disposal_sensitivity::study(&req()).unwrap();
        assert_eq!(s.total, 15, "3 bands x 3 B + 2 cycle starts x 3 B");
        let y = |a: &str, b: &str| {
            s.cases
                .iter()
                .find(|c| c.activity == a && c.bc == b)
                .unwrap()
                .lifetime_years
                .unwrap()
        };
        // Higher activity and larger B shorten the lifetime, monotonically.
        for b in ["low", "nominal", "high"] {
            assert!(
                y("band low", b) > y("band moderate", b)
                    && y("band moderate", b) > y("band high", b)
            );
        }
        for a in [
            "band low",
            "band moderate",
            "band high",
            "cycle from minimum",
            "cycle from maximum",
        ] {
            assert!(
                y(a, "low") > y(a, "nominal") && y(a, "nominal") > y(a, "high"),
                "{a}"
            );
        }
        // Drag lifetime scales as 1/B: elasticity about -1. Solar activity moves it by an order of magnitude.
        assert!(
            (s.elasticity_bc.unwrap() + 1.0).abs() < 0.05,
            "{:?}",
            s.elasticity_bc
        );
        assert!(y("band low", "nominal") / y("band high", "nominal") > 10.0);
        // Starting at solar maximum decays faster than starting at minimum.
        assert!(y("cycle from maximum", "nominal") < y("cycle from minimum", "nominal"));
        // The plan's deadline splits the cases: some within, some beyond.
        assert!(s.within_plan > 0 && s.within_plan < s.total);
        assert!(s.us76_nominal_years.is_some());
        let j = serde_json::to_string(&s).unwrap().to_lowercase();
        for w in ["complian", "conform", "certif"] {
            assert!(!j.contains(w), "{w}");
        }
    }

    #[test]
    fn requests_are_checked() {
        let mut q = req();
        q.plan_source = " ".into();
        assert!(disposal_sensitivity::study(&q).is_err());
        let mut q = req();
        q.bc_uncertainty = 1.5;
        assert!(disposal_sensitivity::study(&q).is_err());
        let mut q = req();
        q.altitude_km = 100.0;
        assert!(disposal_sensitivity::study(&q).is_err());

// ---------------------------------------------------------------------------------------------
// Cislunar catalogue build-up: tracklet association and initial orbit determination.

mod cislunar_catalogue {
    use kshana_pro::cislunar_catalogue as cc;

    #[test]
    fn tracklets_associate_to_their_objects_and_iod_covers_truth() {
        let q = cc::synth();
        assert_eq!(q.tracklets.len(), 9);
        let r = cc::run(&q).unwrap();
        assert_eq!(r.objects.len(), 3);
        assert!(r.unassociated.is_empty());
        assert_eq!(r.purity, Some(1.0));
        assert_eq!(r.completeness, Some(1.0));
        // Every link joins tracklets of one truth object.
        let label = |id: &str| {
            q.tracklets
                .iter()
                .find(|t| t.id == id)
                .unwrap()
                .truth_object
                .clone()
        };
        for l in &r.links {
            assert_eq!(label(&l.from), label(&l.to), "{l:?}");
            assert!(l.residual <= q.gate);
        }
        for o in &r.objects {
            assert_eq!(o.tracklets.len(), 3);
            assert!(o.rms_arcsec.unwrap() < 2.0, "{o:?}");
            assert_eq!(o.within_3_sigma, Some(true), "{o:?}");
            assert!(o.pos_sigma_km.unwrap() > 0.0);
        }
        assert!(r.evidence.starts_with("MODELLED"));
    }

    #[test]
    fn a_tight_gate_leaves_tracklets_unassociated() {
        let mut q = cc::synth();
        q.gate = 0.01;
        let r = cc::run(&q).unwrap();
        assert!(r.objects.is_empty());
        assert_eq!(r.unassociated.len(), 9);
        let mut q = cc::synth();
        q.data_source = " ".into();
        assert!(cc::run(&q).is_err());
    }
}
