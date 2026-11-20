// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic inputs with known faults for every `lunar-evidence` command (`synth <dir>`). Each
//! request file names the fault it carries, so the expected findings are known in advance.

use std::fmt::Write as _;
use std::path::Path;

use kshana::lunar_afs::codes::{afs_i_from_delay, afs_q_primary};
use kshana::lunar_afs::frame::crc24_bits;
use rand::{Rng, SeedableRng};

use super::afs::bits_to_hex;
use super::surface::pa_from_me;
use crate::lunar_pro::budget::std_normal;

type R = rand::rngs::StdRng;

/// URA fixture: nodes N1-N3 have SISE at 0.8 URA; N4 understates its URA (SISE at 1.6 URA).
pub fn ura_csv() -> String {
    let mut rng = R::seed_from_u64(24_01);
    let mut s = String::from("time_s,node,sise_m,ura_m\n");
    for k in 0..2880 {
        for (node, f) in [("N1", 0.8), ("N2", 0.8), ("N3", 0.8), ("N4", 1.6)] {
            let ura = 4.0;
            let _ = writeln!(
                s,
                "{},{node},{:.4},{ura}",
                k * 60,
                f * ura * std_normal(&mut rng)
            );
        }
    }
    s
}

/// Time-consistency fixture: five nodes, 2 ns noise and a shared wander; N3 steps +25 ns at
/// 40 000 s; N5 ramps 0.002 ns/s from 60 000 s.
pub fn time_csv() -> String {
    let mut rng = R::seed_from_u64(24_02);
    let mut s = String::from("time_s,node,clock_ns\n");
    let mut common = 0.0;
    for k in 0..2880 {
        let t = k as f64 * 30.0;
        common += 0.5 * std_normal(&mut rng);
        for n in 1..=5 {
            let mut x = common + 2.0 * std_normal(&mut rng);
            if n == 3 && t >= 40_000.0 {
                x += 25.0;
            }
            if n == 5 && t >= 60_000.0 {
                x += 0.002 * (t - 60_000.0);
            }
            let _ = writeln!(s, "{t},N{n},{x:.4}");
        }
    }
    s
}

/// AFS fixtures: (codes, frames, assignments). PRN 3's Q code has one chip flipped; the second
/// I code is not a Gold code; node 1 has 3 CRC failures and one TOI slip; node 5 reuses node
/// 2's AFS-Q PRN.
pub fn afs_files() -> (String, String, String) {
    let mut codes = String::from("prn,channel,hex\n");
    for prn in 1..=3u16 {
        let mut q = afs_q_primary(prn).expect("PRN in range");
        if prn == 3 {
            q[100] ^= 1;
        }
        let _ = writeln!(codes, "{prn},q,{}", bits_to_hex(&q));
    }
    let i = afs_i_from_delay(5);
    let _ = writeln!(codes, "1,i,{}", bits_to_hex(&i));
    let mut bad = i.clone();
    bad[7] ^= 1;
    let _ = writeln!(codes, "2,i,{}", bits_to_hex(&bad));
    let mut rng = R::seed_from_u64(24_03);
    let mut frames = String::from("time_s,node,toi,bits,hex\n");
    for k in 0..200u32 {
        let data: Vec<u8> = (0..1176).map(|_| rng.gen_range(0..2u8)).collect();
        let mut bits = data.clone();
        bits.extend_from_slice(&crc24_bits(&data));
        if [17, 90, 151].contains(&k) {
            bits[40] ^= 1;
        }
        let toi = if k >= 120 { (k + 1) % 100 } else { k % 100 };
        let _ = writeln!(
            frames,
            "{},1,{toi},{},{}",
            k * 12,
            bits.len(),
            bits_to_hex(&bits)
        );
    }
    let mut asg = String::from("node,i_prn,q_prn,secondary,tertiary_prn,tertiary_phase\n");
    for n in 1..=4 {
        let _ = writeln!(asg, "{n},{n},{n},{},{n},0", (n - 1) % 4);
    }
    asg.push_str("5,5,2,0,5,0\n");
    (codes, frames, asg)
}

/// A white-FM phase record with `sigma_y(1 s)` and a small linear frequency drift.
pub fn clock_csv(sigma_y_1s: f64, seed: u64) -> String {
    let mut rng = R::seed_from_u64(seed);
    let tau0: f64 = 10.0;
    let sy = sigma_y_1s / tau0.sqrt();
    let mut s = String::from("time_s,phase_s\n");
    let mut x = 0.0;
    for k in 0..8640 {
        let t = k as f64 * tau0;
        let _ = writeln!(s, "{t},{:.6e}", x + 0.5 * 1e-17 * t * t);
        x += sy * std_normal(&mut rng) * tau0;
    }
    s
}

/// Station-delay sessions around 1250.0 ns.
pub fn sessions_csv() -> String {
    let v = [
        1249.8, 1250.3, 1250.1, 1249.7, 1250.2, 1250.0, 1249.9, 1250.4,
    ];
    let mut s = String::from("session,delay_ns\n");
    for (i, d) in v.iter().enumerate() {
        let _ = writeln!(s, "S{},{d}", i + 1);
    }
    s
}

/// Radiometric residuals: P1 clean; P2 a 4 m step and two outliers; P3 a ramp and a 20 min gap.
pub fn radiometric_files() -> (String, String) {
    let mut rng = R::seed_from_u64(24_05);
    let mut s = String::from("time_s,pass,kind,residual,sigma\n");
    let mut plan = String::from("pass,start_s,end_s\n");
    for (p, start) in [("P1", 0.0), ("P2", 20_000.0), ("P3", 40_000.0)] {
        let _ = writeln!(plan, "{p},{start},{}", start + 3600.0);
        for k in 0..360 {
            let dt = k as f64 * 10.0;
            if p == "P3" && (1200.0..2400.0).contains(&dt) {
                continue;
            }
            let mut r = 0.5 * std_normal(&mut rng);
            if p == "P2" && dt >= 1800.0 {
                r += 4.0;
            }
            if p == "P2" && (k == 40 || k == 300) {
                r += 6.0;
            }
            if p == "P3" {
                r += 0.002 * dt;
            }
            let _ = writeln!(s, "{},{p},range_m,{r:.4},0.5", start + dt);
        }
    }
    plan.push_str("P4,60000,63600\n");
    (s, plan)
}

/// FDIR plan and results. The rangefinder case misses 10 of 300 (FAIL), the dust case has only
/// 20 runs (insufficient), the altimeter case and the nominal case pass.
pub fn fdir_files() -> (String, String) {
    let plan = "id = \"synthetic-descent-fdir-v1\"\nregistered_utc = \"2026-11-01T00:00:00Z\"\n\
min_detection = 0.99\nmax_false_alarm = 0.01\n\n\
[[case]]\nid = \"alt-dropout\"\nfault = \"altimeter dropout at 200 m\"\nexpect_detect = true\nmax_latency_s = 0.5\nmin_runs = 100\n\n\
[[case]]\nid = \"lrf-false-reject\"\nfault = \"rangefinder valid returns rejected below 1 km\"\nexpect_detect = true\nmax_latency_s = 1.0\nmin_runs = 100\n\n\
[[case]]\nid = \"dust-dropout\"\nfault = \"lidar loss in plume below 30 m\"\nexpect_detect = true\nmax_latency_s = 0.5\nmin_runs = 100\n\n\
[[case]]\nid = \"nominal\"\nfault = \"none\"\nexpect_detect = false\nmin_runs = 300\n"
        .to_string();
    let mut r = String::from("case,run,detected,latency_s\n");
    for k in 0..400 {
        let _ = writeln!(r, "alt-dropout,{k},1,{:.2}", 0.1 + 0.0005 * k as f64);
    }
    for k in 0..300 {
        let miss = k % 30 == 7;
        let _ = writeln!(
            r,
            "lrf-false-reject,{k},{},{}",
            u8::from(!miss),
            if miss { "" } else { "0.4" }
        );
    }
    for k in 0..20 {
        let _ = writeln!(r, "dust-dropout,{k},1,0.3");
    }
    for k in 0..500 {
        let _ = writeln!(r, "nominal,{k},0,");
    }
    r.push_str("extra-case,0,1,0.1\n");
    (plan, r)
}

/// Control points around a south-polar site: reference in PA, map in ME (declared PA) when
/// `me_map`, else the map is PA with a 2 m offset.
pub fn datum_csv(me_map: bool) -> String {
    let mut rng = R::seed_from_u64(24_07);
    let rm = 1_737_400.0;
    let r = pa_from_me(2_451_545.0);
    let mut s = String::from("id,map_x,map_y,map_z,ref_x,ref_y,ref_z,sigma_m\n");
    for i in 0..30 {
        let lat = (-85.5f64 + 0.1 * (i % 6) as f64).to_radians();
        let lon = (31.0f64 + 0.5 * (i / 6) as f64).to_radians();
        let p = [
            rm * lat.cos() * lon.cos(),
            rm * lat.cos() * lon.sin(),
            rm * lat.sin(),
        ];
        let mut m = if me_map {
            [0, 1, 2].map(|k| r[0][k] * p[0] + r[1][k] * p[1] + r[2][k] * p[2])
        } else {
            [p[0] + 2.0, p[1], p[2]]
        };
        for v in &mut m {
            *v += std_normal(&mut rng);
        }
        let _ = writeln!(
            s,
            "C{i},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},1.0",
            m[0], m[1], m[2], p[0], p[1], p[2]
        );
    }
    s
}

/// An 81 x 81 slope grid at 5 m: 4 degrees background, a crater rim at 18 degrees east of
/// 120 m, and a 50-degree block field at (40, -30) within 15 m.
pub fn slopes_csv() -> String {
    let mut s = String::from("x_m,y_m,slope_deg\n");
    for j in -40..=40 {
        for i in -40..=40 {
            let (x, y) = (i as f64 * 5.0, j as f64 * 5.0);
            let mut v = 4.0;
            if x > 120.0 {
                v = 18.0;
            }
            if ((x - 40.0).powi(2) + (y + 30.0).powi(2)).sqrt() < 15.0 {
                v = 50.0;
            }
            let _ = writeln!(s, "{x},{y},{v}");
        }
    }
    s
}

/// Write every fixture and request file under `dir`.
pub fn write_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (codes, frames, asg) = afs_files();
    let (radio, plan_passes) = radiometric_files();
    let (plan, results) = fdir_files();
    let sha = crate::forensics::inputs::sha256_hex(plan.as_bytes());
    let files: Vec<(&str, String)> = vec![
        ("ura.csv", ura_csv()),
        ("ura.toml", "# N4 understates its URA.\n[ura]\ndata = \"ura.csv\"\ndecorrelation_s = 300\nsource = \"synthetic\"\n".into()),
        ("clock-residuals.csv", time_csv()),
        ("time-consistency.toml", "# N3 steps at 40000 s; N5 ramps from 60000 s.\n[time_consistency]\ndata = \"clock-residuals.csv\"\n".into()),
        ("afs-codes.csv", codes),
        ("afs-frames.csv", frames),
        ("afs-assignments.csv", asg),
        ("afs-rx.toml", "# PRN 3 Q chip error, PRN 2 I not Gold, 3 CRC errors, 1 TOI slip, node 5 reuses Q PRN 2.\n[afs_rx]\nreceiver = \"synthetic receiver\"\ncodes = \"afs-codes.csv\"\nframes = \"afs-frames.csv\"\nassignments = \"afs-assignments.csv\"\nmax_frame_error_rate = 0.05\n".into()),
        ("clock-phase.csv", clock_csv(1e-11, 24_04)),
        ("clock.toml", "# White FM at the rafs class level.\n[clock]\ndata = \"clock-phase.csv\"\nclock = \"synthetic Rb\"\nspec = \"rafs\"\n".into()),
        ("delay-sessions.csv", sessions_csv()),
        ("station-delay.toml", "# The previous calibration is 1.8 ns away: En > 1.\n[station_delay]\nstation = \"GS-SYN\"\nsessions = \"delay-sessions.csv\"\nprevious_ns = 1251.8\nprevious_expanded_ns = 0.6\n\n[[station_delay.terms]]\nname = \"cable\"\nu_ns = 0.2\nsource = \"cable delay measurement\"\n\n[[station_delay.terms]]\nname = \"reference-plane\"\nu_ns = 0.5\ndist = \"rectangular\"\nsource = \"reference point definition\"\n\n[[station_delay.terms]]\nname = \"temperature\"\nu_ns = 0.15\nsource = \"temperature coefficient over the session range\"\n".into()),
        ("radiometric.csv", radio),
        ("passes.csv", plan_passes),
        ("radiometric.toml", "# P1 clean; P2 step and outliers; P3 ramp and gap; P4 lost.\n[radiometric]\ndata = \"radiometric.csv\"\nplanned = \"passes.csv\"\n".into()),
        ("fdir-plan.toml", plan),
        ("fdir-results.csv", results),
        ("fdir.toml", format!("# lrf-false-reject fails; dust-dropout has too few runs.\n[fdir]\nplan = \"fdir-plan.toml\"\nregistered_sha256 = \"{sha}\"\nresults = \"fdir-results.csv\"\n")),
        ("config-reviewed.toml", "[expected]\n\"lrf.enabled\" = \"true\"\n\"alt.arm\" = \"armed\"\n\"nav.sw_version\" = \"4.2.1\"\n\"dem.version\" = \"site-dem-v3\"\n\"dem.frame\" = \"PA\"\n".into()),
        ("config-loaded.csv", "key,value\nlrf.enabled,false\nalt.arm,armed\nnav.sw_version,4.2.1\ndem.version,site-dem-v3\ndem.frame,PA\ndebug.patch,applied\n".into()),
        ("config.toml", "# The rangefinder enable flag is off; an unreviewed patch is loaded.\n[config]\nvehicle = \"synthetic lander\"\nexpected = \"config-reviewed.toml\"\nloaded = \"config-loaded.csv\"\n".into()),
        ("datum-me.csv", datum_csv(true)),
        ("datum.toml", "# The map is mean-Earth but declared principal-axis.\n[datum]\npoints = \"datum-me.csv\"\nmap = \"synthetic site map\"\nmap_frame = \"PA\"\nref_frame = \"PA\"\n".into()),
        ("datum-ok.csv", datum_csv(false)),
        ("datum-ok.toml", "# A PA map with a 2 m offset.\n[datum]\npoints = \"datum-ok.csv\"\nmap = \"synthetic site map\"\nmap_frame = \"PA\"\nref_frame = \"PA\"\n".into()),
        ("slopes.csv", slopes_csv()),
        ("tipover.toml", "# A block field 50 m from the target.\n[tipover]\nslopes = \"slopes.csv\"\ndispersion_m = [30.0, 30.0]\nslope_sigma_deg = 2.0\nfootprint_radius_m = 2.0\ncg_height_m = 1.5\ndynamic_margin_deg = 5.0\ndesign_slope_deg = 12.0\n".into()),
    ];
    for (name, text) in files {
        std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(())
}
