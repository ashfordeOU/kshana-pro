// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Timing scorecards under spoofing scenarios (roadmap 0.42, T4). Development inputs are
//! synthetic clocks written here; the public sample uses examples/timing-fleet.

use std::path::{Path, PathBuf};

use kshana_pro::timing::model::{synth_clock, SplitMix};
use kshana_pro::timing::scorecard::{self, TimingScorecardSpec};

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("t4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ticc(path: &Path, seed: u64, q_wf: f64, q_rw: f64) {
    let mut rng = SplitMix::new(seed);
    let x = synth_clock(&mut rng, 6 * 3600, 1.0, q_wf, q_rw, 0.0, 1e-12, 1e-11);
    let mut s = String::from("# SYNTHETIC test clock\n");
    for (i, v) in x.iter().enumerate() {
        s.push_str(&format!("{:.12} chA\n", 1000.0 + i as f64 + v));
    }
    std::fs::write(path, s).unwrap();
}

/// A synthetic fleet: two rubidium-like grandmasters at one site, one downstream boundary clock.
pub fn dev_fleet(d: &Path) -> PathBuf {
    ticc(&d.join("gm-a.ticc"), 7, 1e-24, 1e-32);
    ticc(&d.join("gm-b.ticc"), 8, 1e-24, 1e-32);
    std::fs::write(
        d.join("bc.ptp4l.log"),
        (0..400)
            .map(|i| format!("ptp4l[{}.000]: master offset {:>10} s2 freq   +2100 path delay       512\n", 50000 + 16 * i, (i % 7) as i64 * 10 - 30))
            .collect::<String>(),
    )
    .unwrap();
    std::fs::write(
        d.join("fleet.toml"),
        "kind = \"pro-timing-holdover\"\ntitle = \"Dev fleet\"\nwindow_h = 4\nhorizon_h = 12\n\
         [[limits]]\nname = \"1 us\"\nvalue_s = 1e-6\n[[limits]]\nname = \"100 us\"\nvalue_s = 100e-6\n\
         [[clocks]]\nid = \"gm-a\"\nrole = \"grandmaster\"\n[clocks.source]\nformat = \"ticc\"\npath = \"gm-a.ticc\"\nchannel = \"chA\"\n\
         [[clocks]]\nid = \"gm-b\"\nrole = \"grandmaster\"\n[clocks.source]\nformat = \"ticc\"\npath = \"gm-b.ticc\"\nchannel = \"chA\"\n\
         [[clocks]]\nid = \"bc\"\nrole = \"boundary\"\nclass = \"ocxo\"\n[clocks.source]\nformat = \"ptp4l\"\npath = \"bc.ptp4l.log\"\n",
    )
    .unwrap();
    d.join("fleet.toml")
}

#[test]
fn explore_dev() {
    let d = tmp("dev");
    dev_fleet(&d);
    let src = "kind = \"pro-timing-scorecard\"\ntitle = \"Dev\"\nfleet = \"fleet.toml\"\ngrandmasters = [\"gm-a\"]\n\
               scenarios = [\"jammertest-time-spoofing\", \"texbat-ds3-matched-power-time-push\", \"baltic-timing-holdover-under-jamming\"]\n\
               [[pairs]]\na = \"gm-a\"\nb = \"gm-b\"\n[[network]]\nclock = \"bc\"\nupstream = \"gm-a\"\n";
    std::fs::write(d.join("req.toml"), src).unwrap();
    let (spec, src) = TimingScorecardSpec::from_file(&d.join("req.toml")).unwrap();
    let s = scorecard::score(&spec, &src).unwrap();
    println!("{}", scorecard::csv(&s));
    for r in &s.results { println!("{} {} {:?} {:?} env {:e} x {:?} {:?}", r.scenario, r.detected_by, r.detection_s, r.offset_at_detection_s, r.envelope_s, r.cross_check_threshold_s, r.levels); for c in &r.clocks { println!("   {} {:?} {:?}", c.clock, c.holdover_warning_s, c.levels); } }
}
