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
        rel_pos_rtn_m: Some(r),
        rel_vel_rtn_mps: Some([0.0, -14_000.0, 0.0]),
        state1_m: None,
        state2_m: None,
        cov1_rtn_m2: [s1 * s1, 0.0, 1e6, 0.0, 0.0, s1 * s1],
        cov2_rtn_m2: [s2 * s2, 0.0, 1e6, 0.0, 0.0, s2 * s2],
        cov2_frame: Some("rtn1".into()),
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
    c.rel_vel_rtn_mps = Some([0.0; 3]);
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

    /// Frame audit: an OEM target's TIME_SYSTEM must be UTC (epochs are compared with UTC
    /// windows); a GPS-time OEM is refused instead of being shifted by 18 s.
    #[test]
    fn oem_time_system_must_be_utc() {
        let d = std::env::temp_dir().join(format!("kshana-oem-ts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let oem = |ts: &str| {
            let mut s = format!(
                "CCSDS_OEM_VERS = 2.0\nCREATION_DATE = 2026-11-16T00:00:00\nORIGINATOR = SYNTHETIC\n\nMETA_START\nOBJECT_NAME = SYN-OEM\nOBJECT_ID = SYN-OEM\nCENTER_NAME = EARTH\nREF_FRAME = TEME\nTIME_SYSTEM = {ts}\nSTART_TIME = 2026-11-16T12:00:00\nSTOP_TIME = 2026-11-16T12:03:00\nMETA_STOP\n\n"
            );
            for k in 0..4 {
                let a = k as f64 * 60.0 * 7.5 / 7000.0;
                s.push_str(&format!(
                    "2026-11-16T12:0{k}:00 {:.3} {:.3} 0.0 {:.6} {:.6} 0.0\n",
                    7000.0 * a.cos(),
                    7000.0 * a.sin(),
                    -7.5 * a.sin(),
                    7.5 * a.cos()
                ));
            }
            s
        };
        let mut q = req();
        q.targets.truncate(1);
        q.targets[0].tle_line1 = None;
        q.targets[0].tle_line2 = None;
        q.targets[0].oem = Some("t.oem".into());
        q.targets[0].oem_object = Some("SYN-OEM".into());
        std::fs::write(d.join("t.oem"), oem("GPS")).unwrap();
        let e = debris_obs::plan(&q, &d).unwrap_err();
        assert!(e.contains("TIME_SYSTEM GPS"), "{e}");
        std::fs::write(d.join("t.oem"), oem("UTC")).unwrap();
        if let Err(e) = debris_obs::plan(&q, &d) {
            assert!(!e.contains("TIME_SYSTEM"), "{e}");
        }
        let _ = std::fs::remove_dir_all(&d);
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
    }
}

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

/// Pc dedupe preparation: Pro `space_ops::pc_2d` against the open engine's
/// `orbit_data::pc::foster_2d` (open branch open/conjunction-pc, kshana #170), mirrored here
/// because the pinned engine release does not have it yet. Once it does, `pc_2d` calls it and
/// this mirror goes.
mod pc_dedupe {
    use kshana_pro::space_ops as so;

    type V3 = [f64; 3];
    type M3 = [[f64; 3]; 3];

    // ---- Mirror of the open foster_2d (encounter frame, principal axes, closed-form inner
    // integral, adaptive Simpson with x = R sin t). Kept line for line with the open code.
    fn sub(a: V3, b: V3) -> V3 {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn dot(a: V3, b: V3) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    fn cross(a: V3, b: V3) -> V3 {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn scale(a: V3, s: f64) -> V3 {
        [a[0] * s, a[1] * s, a[2] * s]
    }
    fn norm(a: V3) -> f64 {
        dot(a, a).sqrt()
    }
    fn quad(m: &M3, a: V3, b: V3) -> f64 {
        let mb = [dot(m[0], b), dot(m[1], b), dot(m[2], b)];
        dot(a, mb)
    }

    pub fn foster_2d(r1: V3, v1: V3, c1: &M3, r2: V3, v2: V3, c2: &M3, hbr_m: f64) -> f64 {
        let r = sub(r2, r1);
        let v = sub(v2, v1);
        let speed = norm(v);
        let mut c = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                c[i][j] = 0.5 * (c1[i][j] + c1[j][i]) + 0.5 * (c2[i][j] + c2[j][i]);
            }
        }
        let z = scale(v, 1.0 / speed);
        let perp = sub(r, scale(z, dot(r, z)));
        let x = if norm(perp) > 1e-12 * norm(r).max(1.0) {
            scale(perp, 1.0 / norm(perp))
        } else {
            let t = if z[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let p = cross(z, t);
            scale(p, 1.0 / norm(p))
        };
        let y = cross(z, x);
        let (cxx, cxy, cyy) = (quad(&c, x, x), quad(&c, x, y), quad(&c, y, y));
        let miss_plane = dot(r, x);
        let tr = cxx + cyy;
        let disc = (((cxx - cyy) / 2.0).powi(2) + cxy * cxy).sqrt();
        let (l1, l2) = (tr / 2.0 + disc, tr / 2.0 - disc);
        assert!(l2 > 0.0);
        let th = 0.5 * (2.0 * cxy).atan2(cxx - cyy);
        let (s, co) = th.sin_cos();
        integrate(
            hbr_m,
            l1.sqrt(),
            l2.sqrt(),
            miss_plane * co,
            -miss_plane * s,
        )
    }

    fn integrate(r: f64, sx: f64, sy: f64, mx: f64, my: f64) -> f64 {
        let inv_sqrt_2pi = 1.0 / (2.0 * std::f64::consts::PI).sqrt();
        let g = |t: f64| {
            let (st, ct) = t.sin_cos();
            let x = r * st;
            let h = (r * ct).max(0.0);
            let px = inv_sqrt_2pi / sx * (-0.5 * ((x - mx) / sx).powi(2)).exp();
            let s2 = std::f64::consts::SQRT_2 * sy;
            let inner = 0.5 * (libm::erf((my + h) / s2) - libm::erf((my - h) / s2));
            px * inner * r * ct
        };
        let (a, b) = (-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
        let n = 64;
        let w = (b - a) / n as f64;
        let mut total = 0.0;
        for k in 0..n {
            let (l, u) = (a + k as f64 * w, a + (k + 1) as f64 * w);
            let (fl, fm, fu) = (g(l), g(0.5 * (l + u)), g(u));
            let whole = (u - l) / 6.0 * (fl + 4.0 * fm + fu);
            total += simpson(&g, l, u, fl, fm, fu, whole, 1e-13, 40);
        }
        total.clamp(0.0, 1.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn simpson(
        g: &dyn Fn(f64) -> f64,
        a: f64,
        b: f64,
        fa: f64,
        fm: f64,
        fb: f64,
        whole: f64,
        tol: f64,
        depth: u32,
    ) -> f64 {
        let m = 0.5 * (a + b);
        let (lm, rm) = (0.5 * (a + m), 0.5 * (m + b));
        let (flm, frm) = (g(lm), g(rm));
        let left = (m - a) / 6.0 * (fa + 4.0 * flm + fm);
        let right = (b - m) / 6.0 * (fm + 4.0 * frm + fb);
        let delta = left + right - whole;
        if depth == 0 || delta.abs() <= 15.0 * tol.max(1e-10 * (left + right).abs()) {
            return left + right + delta / 15.0;
        }
        simpson(g, a, m, fa, flm, fm, left, tol / 2.0, depth - 1)
            + simpson(g, m, b, fm, frm, fb, right, tol / 2.0, depth - 1)
    }

    // ---- The comparison.

    fn cov(l: [f64; 6]) -> M3 {
        [[l[0], l[1], l[3]], [l[1], l[2], l[4]], [l[3], l[4], l[5]]]
    }

    /// The open function on a Pro CDM: object 1 at the origin at rest, object 2 at the relative
    /// state, both covariances taken in the same (object 1 RTN) frame, as `pc_2d` does.
    fn open_on(c: &so::Cdm) -> f64 {
        foster_2d(
            [0.0; 3],
            [0.0; 3],
            &cov(c.cov1_rtn_m2),
            c.rel_pos_rtn_m.unwrap(),
            c.rel_vel_rtn_mps.unwrap(),
            &cov(c.cov2_rtn_m2),
            c.hbr_m,
        )
    }

    fn cases() -> Vec<(String, so::Cdm)> {
        let base = |id: &str, r: V3, v: V3, c1: [f64; 6], c2: [f64; 6], hbr: f64| {
            (
                id.to_string(),
                so::Cdm {
                    message_id: id.into(),
                    creation_utc: "2026-11-17T00:00:00Z".into(),
                    tca_utc: "2026-11-18T00:00:00Z".into(),
                    rel_pos_rtn_m: Some(r),
                    rel_vel_rtn_mps: Some(v),
                    state1_m: None,
                    state2_m: None,
                    cov1_rtn_m2: c1,
                    cov2_rtn_m2: c2,
                    cov2_frame: Some("rtn1".into()),
                    hbr_m: hbr,
                    pc_reported: None,
                },
            )
        };
        let head_on = [0.0, -14_000.0, 0.0];
        let mut v = vec![
            base(
                "zero-miss-iso",
                [0.0; 3],
                head_on,
                [900.0, 0.0, 1e6, 0.0, 0.0, 900.0],
                [1600.0, 0.0, 1e6, 0.0, 0.0, 1600.0],
                20.0,
            ),
            base(
                "far-small-body",
                [250.0, 0.0, 0.0],
                head_on,
                [1e4, 0.0, 1e6, 0.0, 0.0, 1e4],
                [0.0; 6],
                1.0,
            ),
            base(
                "big-body",
                [40.0, 0.0, 10.0],
                head_on,
                [100.0, 0.0, 1e6, 0.0, 0.0, 100.0],
                [100.0, 0.0, 1e6, 0.0, 0.0, 100.0],
                30.0,
            ),
            base(
                "correlated-oblique",
                [120.0, 300.0, -80.0],
                [50.0, -9_000.0, 4_000.0],
                [2500.0, 800.0, 250_000.0, -300.0, 1500.0, 900.0],
                [40_000.0, -2000.0, 4e6, 500.0, -800.0, 10_000.0],
                12.0,
            ),
            base(
                "elongated",
                [5.0, 0.0, 400.0],
                head_on,
                [25.0, 0.0, 1e6, 0.0, 0.0, 250_000.0],
                [0.0; 6],
                10.0,
            ),
        ];
        // The Pro CDM fixtures (the decision-record synth).
        let files = so::synth().unwrap();
        let ca = &files.iter().find(|(n, _)| *n == "ca.toml").unwrap().1;
        let q: so::CaRequest = toml::from_str(ca).unwrap();
        for c in q.cdms {
            v.push((c.message_id.clone(), c));
        }
        v
    }

    /// The mirror reproduces the closed forms to 1e-9 (so it, not Pro's grid, is the reference).
    #[test]
    fn open_mirror_matches_the_closed_forms() {
        let head_on = [0.0, -14_000.0, 0.0];
        for (s1, s2, hbr) in [(30.0, 40.0, 20.0), (100.0, 0.0, 10.0), (10.0, 10.0, 30.0)] {
            let s2c: f64 = s1 * s1 + s2 * s2;
            let want = 1.0 - (-(hbr * hbr) / (2.0 * s2c)).exp();
            let c1 = [[s1 * s1, 0.0, 0.0], [0.0, 1e6, 0.0], [0.0, 0.0, s1 * s1]];
            let c2 = [[s2 * s2, 0.0, 0.0], [0.0, 1e6, 0.0], [0.0, 0.0, s2 * s2]];
            let got = foster_2d([0.0; 3], [0.0; 3], &c1, [0.0; 3], head_on, &c2, hbr);
            assert!((got / want - 1.0).abs() < 1e-9, "{got} vs {want}");
        }
    }

    /// Pro `pc_2d` against the open `foster_2d` on the same inputs, the Pro CDM fixtures
    /// included: agreement to 1e-9 relative now that Pro integrates as the open routine does
    /// (before pro/pc-frame-fix, Pro's fixed polar grid agreed only to about 1e-5).
    #[test]
    fn pro_pc_2d_against_the_open_foster_2d() {
        for (id, c) in cases() {
            let pro = so::pc_2d(&c).unwrap();
            let open = open_on(&c);
            let rel = (pro / open - 1.0).abs();
            assert!(
                open > 0.0 && rel < 1e-9,
                "{id}: pro {pro} open {open} rel {rel}"
            );
        }
    }

    // ---- Mirror of the open CDM path (from_cdm: each object's RTN covariance rotated from its
    // own RTN frame, covariance::from_rtn with cdm::rtn_axes).
    fn rtn_axes(s: &[f64; 6]) -> M3 {
        let r = [s[0], s[1], s[2]];
        let v = [s[3], s[4], s[5]];
        let rr = scale(r, 1.0 / norm(r));
        let h = cross(r, v);
        let n = scale(h, 1.0 / norm(h));
        [rr, cross(n, rr), n]
    }
    fn from_rtn(s: &[f64; 6], c: &M3) -> M3 {
        let a = rtn_axes(s);
        let mut out = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    for l in 0..3 {
                        out[i][j] += a[k][i] * c[k][l] * a[l][j];
                    }
                }
            }
        }
        out
    }
    fn open_from_states(s1: &[f64; 6], c1: [f64; 6], s2: &[f64; 6], c2: [f64; 6], hbr: f64) -> f64 {
        foster_2d(
            [s1[0], s1[1], s1[2]],
            [s1[3], s1[4], s1[5]],
            &from_rtn(s1, &cov(c1)),
            [s2[0], s2[1], s2[2]],
            [s2[3], s2[4], s2[5]],
            &from_rtn(s2, &cov(c2)),
            hbr,
        )
    }

    /// Two objects whose RTN frames differ (orbit planes 70 degrees apart, object 2 faster and
    /// slightly eccentric), each covariance in its own RTN frame, long along track. The fixed code (object states) matches the mirrored
    /// open CDM path; the old convention (relative state in object 1's RTN, object 2's covariance
    /// summed unrotated) is measurably wrong.
    #[test]
    fn object_2_covariance_is_rotated_from_its_own_rtn_frame() {
        let mu: f64 = 3.986004418e14;
        let a: f64 = 7_000e3;
        let vc = (mu / a).sqrt();
        let inc: f64 = 70f64.to_radians();
        let s1 = [a, 0.0, 0.0, 0.0, vc, 0.0];
        // Object 2 crosses 120 m radially and 80 m cross-track off object 1 at TCA.
        // Object 2 is on a faster, slightly eccentric orbit (a radial velocity and 8 % more speed), so
        // the relative velocity does not bisect the two along-track axes.
        let s2 = [
            a + 120.0,
            0.0,
            80.0,
            300.0,
            1.08 * vc * inc.cos(),
            1.08 * vc * inc.sin(),
        ];
        let c1 = [400.0, 0.0, 250_000.0, 0.0, 0.0, 900.0];
        let c2 = [2_500.0, 0.0, 4_000_000.0, 0.0, 0.0, 1_600.0];
        let hbr = 15.0;
        let want = open_from_states(&s1, c1, &s2, c2, hbr);
        let new = so::Cdm {
            message_id: "X".into(),
            creation_utc: "2026-11-17T00:00:00Z".into(),
            tca_utc: "2026-11-18T00:00:00Z".into(),
            rel_pos_rtn_m: None,
            rel_vel_rtn_mps: None,
            state1_m: Some(s1),
            state2_m: Some(s2),
            cov1_rtn_m2: c1,
            cov2_rtn_m2: c2,
            cov2_frame: None,
            hbr_m: hbr,
            pc_reported: None,
        };
        let got = so::pc_2d(&new).unwrap();
        assert!((got / want - 1.0).abs() < 1e-9, "{got} vs {want}");
        // The old convention: the relative state in object 1's RTN axes, object 2's covariance
        // (in its own RTN) summed as if it were in object 1's.
        let ax = rtn_axes(&s1);
        let to1 = |w: V3| [dot(ax[0], w), dot(ax[1], w), dot(ax[2], w)];
        let r = to1([s2[0] - s1[0], s2[1] - s1[1], s2[2] - s1[2]]);
        let v = to1([s2[3] - s1[3], s2[4] - s1[4], s2[5] - s1[5]]);
        let mut old = new.clone();
        old.state1_m = None;
        old.state2_m = None;
        old.rel_pos_rtn_m = Some(r);
        old.rel_vel_rtn_mps = Some(v);
        old.cov2_frame = Some("rtn1".into());
        let wrong = so::pc_2d(&old).unwrap();
        let err = (wrong / want - 1.0).abs();
        eprintln!("rotated {want:.6e}  unrotated {wrong:.6e}  relative error {err:.2}");
        assert!(err > 0.1, "{wrong} vs {want}");
        // The relative form with the covariance already rotated into object 1's RTN agrees.
        let c2_in_1 = {
            let g = from_rtn(&s2, &cov(c2));
            let mut m = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    for k in 0..3 {
                        for l in 0..3 {
                            m[i][j] += ax[i][k] * g[k][l] * ax[j][l];
                        }
                    }
                }
            }
            [m[0][0], m[1][0], m[1][1], m[2][0], m[2][1], m[2][2]]
        };
        let mut right = old.clone();
        right.cov2_rtn_m2 = c2_in_1;
        let ok = so::pc_2d(&right).unwrap();
        assert!((ok / want - 1.0).abs() < 1e-9, "{ok} vs {want}");
        // Mixed or undeclared inputs.
        let mut mixed = new.clone();
        mixed.rel_pos_rtn_m = Some(r);
        assert!(so::pc_2d(&mixed).is_err());
        let mut bad = old.clone();
        bad.cov2_frame = Some("rtn2".into());
        assert!(so::pc_2d(&bad)
            .unwrap_err()
            .contains("give both object states"));
    }

    /// A record whose relative-state CDMs do not declare the frame of object 2's covariance is
    /// flagged for review; the synth (which declares it) is not.
    #[test]
    fn undeclared_covariance_frame_is_flagged_in_the_record() {
        let files = so::synth().unwrap();
        let ca = &files.iter().find(|(n, _)| *n == "ca.toml").unwrap().1;
        let q: so::CaRequest = toml::from_str(ca).unwrap();
        let r = so::ca_record(&q).unwrap();
        assert!(
            r.policy_findings.iter().all(|f| !f.contains("cov2_frame")),
            "{:?}",
            r.policy_findings
        );
        assert!(r.cdms[0].covariance_frames.contains("(declared)"));
        let q: so::CaRequest = toml::from_str(&ca.replace("cov2_frame = \"rtn1\"\n", "")).unwrap();
        let r = so::ca_record(&q).unwrap();
        assert!(
            r.policy_findings.iter().any(|f| f.contains("cov2_frame")),
            "{:?}",
            r.policy_findings
        );
        assert!(r.cdms[0].covariance_frames.contains("ASSUMED"));
    }
/// Frame audit: the identification ephemeris must be TEME on UTC; declared otherwise it is
/// refused, undeclared it is assumed and said.
#[test]
fn identification_ephemeris_frame_is_declared_or_assumed() {
    let files = so::synth().unwrap();
    let get = |n: &str| files.iter().find(|(f, _)| *f == n).unwrap().1.clone();
    #[derive(serde::Deserialize)]
    struct C {
        candidate: Vec<so::Candidate>,
    }
    let eph = get("ephemeris.csv");
    let c: C = toml::from_str(&get("candidates.toml")).unwrap();
    let r = so::identify(&eph, &c.candidate).unwrap();
    assert_eq!(r.ephemeris_frame, "TEME on UTC (declared)");
    let bare: String = eph
        .lines()
        .map(|l| l.rsplitn(3, ',').nth(2).unwrap().to_string() + "\n")
        .collect();
    let r2 = so::identify(&bare, &c.candidate).unwrap();
    assert!(r2.ephemeris_frame.contains("ASSUMED"));
    assert_eq!(r2.best, r.best);
    assert!(
        so::identify(&eph.replace(",TEME,", ",EME2000,"), &c.candidate)
            .unwrap_err()
            .contains("must be TEME")
    );
    assert!(so::identify(&eph.replace(",UTC\n", ",GPS\n"), &c.candidate)
        .unwrap_err()
        .contains("must be UTC"));
}
