// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic inputs for every `simlab` command (`kshana-pro simlab synth <dir>`). All
//! synthetic and deterministic: no real bench, simulator, lab or receiver.
//!
//! * `chain-v1.toml`, `chain-v2.toml`: one recorder chain registered twice; the second has
//!   a different LNA (higher noise figure) and an active AGC;
//! * `drive-good.toml`, `drive-bad.toml`: a 60 s GNSS I/Q stream (SigMF, decimated to 1 kHz
//!   so the file stays small) with IMU (free-running, 20 ppm), CAN (host clock, a gap) and
//!   camera streams; in the bad set the camera has no shared event and the CAN only one;
//! * `selftest-sim.toml`: two reference receivers that both read 1.5 dB high (a simulator
//!   power offset); `selftest-rig.toml`: a replay rig against station observations where one
//!   receiver loses a PRN and reads another 2 dB low (a receiver fault);
//! * `lab-a.toml`, `lab-b.toml`: two lab layouts (units, names, booleans differ) of the same
//!   spoofing tests, one scenario run from a different file;
//! * `tco.toml`: four illustrative options.

use std::path::Path;

/// Reference stream start (seconds on the GNSS clock).
pub const T_REF: f64 = 1000.0;
pub const DUR: f64 = 60.0;

fn w(dir: &Path, name: &str, text: &str) -> Result<(), String> {
    std::fs::write(dir.join(name), text).map_err(|e| format!("{}: {e}", dir.join(name).display()))
}

const CHAIN: &str = r#"format = "kshana-simlab-recorder-chain/1"
chain_id = "REC-A"
date = "@DATE@"
operator = "Synthetic operator"
sample_rate_hz = 20e6
signal_bandwidth_hz = 2.046e6
interval_days = 365
[[stage]]
kind = "lna"
name = "@LNA@"
gain_db = 28
noise_figure_db = @NF@
group_delay_ns = 12
group_delay_u_ns = 2
[[stage]]
kind = "cable"
name = "20 m LMR-400"
gain_db = -4.4
group_delay_ns = 78
group_delay_u_ns = 0.5
[[stage]]
kind = "filter"
name = "L1 SAW"
gain_db = -2.5
bandwidth_hz = 16e6
group_delay_ns = 45
group_delay_u_ns = 3
[[stage]]
kind = "amplifier"
name = "IF gain block"
gain_db = 20
noise_figure_db = 4.0
[[stage]]
kind = "agc"
mode = "@AGC@"
[[stage]]
kind = "adc"
name = "Recorder ADC"
bits = 4
[[stage]]
kind = "reference"
name = "Lab OCXO"
reference = "ocxo"
[[stage.phase_noise]]
offset_hz = 10
dbc_hz = -85
[[stage.phase_noise]]
offset_hz = 1000
dbc_hz = -120
"#;

fn chain(date: &str, lna: &str, nf: f64, agc: &str) -> String {
    CHAIN
        .replace("@DATE@", date)
        .replace("@LNA@", lna)
        .replace("@NF@", &nf.to_string())
        .replace("@AGC@", agc)
}

/// Deterministic small "noise" in ±1 (no RNG dependency).
fn wobble(i: usize) -> f64 {
    ((i as f64 * 12.9898).sin() * 43_758.545).fract()
}

/// Stream clock → reference clock models `(offset, drift)` per stream.
const IMU: (f64, f64) = (999.5, 20e-6);
const CAN: (f64, f64) = (1000.2, -30e-6);
const CAM: (f64, f64) = (1000.1, 0.0);

fn to_stream(t_ref: f64, m: (f64, f64)) -> f64 {
    (t_ref - m.0) / (1.0 + m.1)
}

fn times(rate: f64, dur: f64, skip: Option<(f64, f64)>, jitter: f64) -> String {
    let mut s = String::from("t_s\n");
    let n = (rate * dur) as usize;
    for i in 0..=n {
        let t = i as f64 / rate;
        if skip.is_some_and(|(a, b)| t >= a && t < b) {
            continue;
        }
        s.push_str(&format!("{:.6}\n", t + jitter * wobble(i)));
    }
    s
}

fn drive(good: bool) -> String {
    let ev = |name: &str, tr: f64, with: &[(&str, (f64, f64))]| {
        let mut at = format!("\"gnss-iq\" = {tr}");
        for (k, m) in with {
            at.push_str(&format!(", {k} = {:.6}", to_stream(tr, *m)));
        }
        format!("[[event]]\nname = \"{name}\"\nat = {{ {at} }}\n")
    };
    let mut s = format!(
        "format = \"kshana-simlab-replay-set/1\"\nid = \"drive-{}\"\nreference = \"gnss-iq\"\ntolerance_ms = 1.0\n\
[[stream]]\nname = \"gnss-iq\"\nkind = \"iq\"\nclock = \"gnss\"\nsigmf = \"drive.sigmf-meta\"\nstart_s = {T_REF}\n\
[[stream]]\nname = \"imu\"\nkind = \"imu\"\nclock = \"free-running\"\ntimestamps = \"imu.csv\"\n\
[[stream]]\nname = \"can\"\nkind = \"can\"\nclock = \"host\"\ntimestamps = \"can.csv\"\n\
[[stream]]\nname = \"camera\"\nkind = \"camera\"\nclock = \"free-running\"\ntimestamps = \"camera.csv\"\n",
        if good { "good" } else { "bad" }
    );
    if good {
        s.push_str(&ev(
            "tap-1",
            1010.0,
            &[("imu", IMU), ("can", CAN), ("camera", CAM)],
        ));
        s.push_str(&ev(
            "tap-2",
            1030.0,
            &[("imu", IMU), ("can", CAN), ("camera", CAM)],
        ));
        s.push_str(&ev(
            "tap-3",
            1050.0,
            &[("imu", IMU), ("can", CAN), ("camera", CAM)],
        ));
    } else {
        s.push_str(&ev("tap-1", 1010.0, &[("imu", IMU), ("can", CAN)]));
        s.push_str(&ev("tap-2", 1030.0, &[("imu", IMU)]));
        s.push_str(&ev("tap-3", 1050.0, &[("imu", IMU)]));
    }
    s
}

const SIGMF_RATE: f64 = 1000.0;

fn obs(prns: &[(&str, f64)], n: usize, wob: f64) -> String {
    let mut s = String::from("t_s,prn,cn0_dbhz\n");
    for i in 0..n {
        for (k, (p, c)) in prns.iter().enumerate() {
            s.push_str(&format!("{i},{p},{:.2}\n", c + wob * wobble(i * 7 + k)));
        }
    }
    s
}

fn fixes(at: [f64; 3], n: usize, m: f64) -> String {
    let mut s = String::from("t_s,lat_deg,lon_deg,h_m\n");
    for i in 0..n {
        let dn = m * wobble(i) / 111_195.0;
        let de = m * wobble(i + 99) / (111_195.0 * at[0].to_radians().cos());
        s.push_str(&format!(
            "{i},{:.9},{:.9},{:.2}\n",
            at[0] + dn,
            at[1] + de,
            at[2] + m * wobble(i + 7)
        ));
    }
    s
}

fn pps(mean: f64, n: usize) -> String {
    let mut s = String::from("t_s,offset_ns\n");
    for i in 0..n {
        s.push_str(&format!("{i},{:.2}\n", mean + 3.0 * wobble(i + 3)));
    }
    s
}

const POS: [f64; 3] = [52.0, 5.0, 10.0];
const PRNS: [(&str, f64); 6] = [
    ("G01", 45.0),
    ("G07", 44.0),
    ("G12", 46.0),
    ("G19", 43.0),
    ("E05", 45.0),
    ("E11", 44.0),
];

fn shift(d: f64, drop: Option<&str>, low: Option<(&str, f64)>) -> Vec<(&'static str, f64)> {
    PRNS.iter()
        .filter(|(p, _)| Some(*p) != drop)
        .map(|(p, c)| {
            let l = low.filter(|x| x.0 == *p).map(|x| x.1).unwrap_or(0.0);
            (*p, c + d + l)
        })
        .collect()
}

const LAB_A: &str = r#"format = "kshana-simlab-lab-results/1"
lab = "Lab A (synthetic)"
service = "spoofing"
report_id = "LA-2026-014"
date = "2026-11-10"
test_object = "Receiver X, firmware 1.2 (synthetic)"
results = "lab-a.csv"
[setup]
rf_path = "conducted"
simulator = "Lab A simulator"
reference = "gpsdo"
calibration_due = "2027-03-01"
[scenario_ids]
"SP-03 sync takeover" = "spoof-sync-takeover"
"SP-07 meaconing 2s" = "spoof-meaconing"
[scenario_sha256]
spoof-sync-takeover = "1111111111111111111111111111111111111111111111111111111111111111"
spoof-meaconing = "2222222222222222222222222222222222222222222222222222222222222222"
[columns]
"Scenario" = { field = "scenario" }
"Detected" = { metric = "detected" }
"Captured" = { metric = "captured" }
"TTD [ms]" = { metric = "time_to_detect_s", unit = "ms" }
"U TTD [ms]" = { uncertainty_of = "time_to_detect_s", unit = "ms" }
"Max pos err [m]" = { metric = "position_error_max_m", unit = "m" }
"U pos [m]" = { uncertainty_of = "position_error_max_m", unit = "m" }
"Max time err [us]" = { metric = "time_error_max_ns", unit = "us" }
"#;

const LAB_A_CSV: &str = "Scenario,Detected,Captured,TTD [ms],U TTD [ms],Max pos err [m],U pos [m],Max time err [us],Operator\n\
SP-03 sync takeover,yes,no,2400,300,35.0,4.0,0.42,JD\n\
SP-07 meaconing 2s,yes,yes,5100,500,610.0,20.0,2000,JD\n";

const LAB_B: &str = r#"format = "kshana-simlab-lab-results/1"
lab = "Lab B (synthetic)"
service = "spoofing"
report_id = "B/26/0091"
date = "2026-11-12"
test_object = "Receiver X, firmware 1.2 (synthetic)"
results = "lab-b.csv"
[setup]
rf_path = "conducted"
simulator = "Lab B simulator"
reference = "rubidium"
calibration_due = "2026-12-31"
[scenario_ids]
"sync_takeover" = "spoof-sync-takeover"
"meaconing" = "spoof-meaconing"
[scenario_sha256]
spoof-sync-takeover = "1111111111111111111111111111111111111111111111111111111111111111"
spoof-meaconing = "3333333333333333333333333333333333333333333333333333333333333333"
[columns]
"test" = { field = "scenario" }
"flag_raised" = { metric = "detected" }
"pulled_off" = { metric = "captured" }
"t_detect_s" = { metric = "time_to_detect_s", unit = "s" }
"t_detect_U_s" = { uncertainty_of = "time_to_detect_s", unit = "s" }
"hpe_max_km" = { metric = "position_error_max_m", unit = "km" }
"hpe_max_U_km" = { uncertainty_of = "position_error_max_m", unit = "km" }
"js_lol_db" = { metric = "jsr_at_loss_of_lock_db", unit = "dB" }
"#;

const LAB_B_CSV: &str =
    "test,flag_raised,pulled_off,t_detect_s,t_detect_U_s,hpe_max_km,hpe_max_U_km,js_lol_db\n\
sync_takeover,1,0,2.9,0.2,0.0380,0.003,\n\
meaconing,1,1,4.8,0.4,0.598,0.025,24.5\n";

const TCO: &str = r#"format = "kshana-simlab-tco/1"
currency = "USD"
years = 5
discount_rate = 0.06
campaigns_per_year = 4
notes = [
  "ILLUSTRATIVE INPUTS. Replace every figure with your own quotes.",
  "Annual simulator maintenance of 61k to 250k USD and a 44k USD record-and-replay unit are figures quoted in public research notes; the other figures are placeholders.",
]
[[option]]
name = "Buy a new multi-constellation simulator"
purchase = 300000
annual_maintenance = 61000
annual_staff = 15000
per_campaign = 2000
residual_value = 30000
[[option]]
name = "Keep the legacy simulator in service"
annual_maintenance = 90000
annual_staff = 20000
per_campaign = 3000
[[option]]
name = "Record-and-replay unit plus software scenarios"
purchase = 44000
annual_maintenance = 5000
annual_staff = 15000
per_campaign = 4000
[[option]]
name = "Test as a service"
per_campaign = 40000
"#;

/// Write every synthetic input under `dir`.
pub fn write_all(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    w(
        dir,
        "chain-v1.toml",
        &chain("2026-05-04", "Active antenna LNA s/n 101", 1.8, "fixed"),
    )?;
    w(
        dir,
        "chain-v2.toml",
        &chain("2026-11-18", "Active antenna LNA s/n 207", 3.5, "active"),
    )?;
    // Replay sets.
    w(dir, "drive-good.toml", &drive(true))?;
    w(dir, "drive-bad.toml", &drive(false))?;
    let meta = serde_json::json!({
        "global": {"core:datatype": "ci8", "core:sample_rate": SIGMF_RATE, "core:version": "1.0.0",
                   "core:description": "SYNTHETIC zeros standing in for a decimated GNSS I/Q recording"},
        "captures": [{"core:sample_start": 0, "core:datetime": "1970-01-01T00:16:40Z"}],
        "annotations": []
    });
    w(
        dir,
        "drive.sigmf-meta",
        &serde_json::to_string_pretty(&meta).unwrap(),
    )?;
    std::fs::write(
        dir.join("drive.sigmf-data"),
        vec![0u8; (SIGMF_RATE * DUR) as usize * 2],
    )
    .map_err(|e| e.to_string())?;
    w(dir, "imu.csv", &times(100.0, DUR, None, 0.0))?;
    w(dir, "can.csv", &times(10.0, DUR, Some((20.0, 22.0)), 0.0))?;
    w(dir, "camera.csv", &times(30.0, DUR, None, 0.0002))?;
    // Self-tests.
    let n = 60;
    w(
        dir,
        "sim-ref-a-obs.csv",
        &obs(&shift(1.5, None, None), n, 0.3),
    )?;
    w(
        dir,
        "sim-ref-b-obs.csv",
        &obs(&shift(1.4, None, None), n, 0.3),
    )?;
    w(dir, "sim-ref-a-fix.csv", &fixes(POS, n, 1.0))?;
    w(dir, "sim-ref-b-fix.csv", &fixes(POS, n, 1.2))?;
    w(dir, "sim-ref-a-pps.csv", &pps(4.0, n))?;
    w(dir, "sim-ref-b-pps.csv", &pps(-6.0, n))?;
    let rx = |name: &str, p: &str| {
        format!(
            "[[receiver]]\nname = \"{name}\"\nobservations = \"{p}-obs.csv\"\nfixes = \"{p}-fix.csv\"\npps = \"{p}-pps.csv\"\n"
        )
    };
    let tol = "[tolerance]\npower_db = 1.0\nspread_db = 1.0\nposition_m = 3.0\ntime_ns = 20.0\n";
    let mut by_prn = String::from("[expected.cn0_by_prn]\n");
    for (p, c) in PRNS {
        by_prn.push_str(&format!("{p} = {c}\n"));
    }
    w(
        dir,
        "selftest-sim.toml",
        &format!(
            "format = \"kshana-simlab-selftest/1\"\nid = \"SIM-A self-test (synthetic)\"\nmode = \"simulator\"\nsource = \"Lab simulator A (synthetic)\"\n[expected]\nposition = [{}, {}, {}]\npps_offset_ns = 0.0\n{by_prn}{tol}{}{}",
            POS[0], POS[1], POS[2],
            rx("REF-A", "sim-ref-a"),
            rx("REF-B", "sim-ref-b")
        ),
    )?;
    w(
        dir,
        "station-obs.csv",
        &obs(&shift(0.0, None, None), n, 0.5),
    )?;
    w(
        dir,
        "rig-ref-a-obs.csv",
        &obs(&shift(0.2, None, None), n, 0.3),
    )?;
    w(
        dir,
        "rig-ref-b-obs.csv",
        &obs(&shift(0.1, Some("G07"), Some(("G12", -2.0))), n, 0.3),
    )?;
    w(dir, "rig-ref-a-fix.csv", &fixes(POS, n, 1.0))?;
    w(dir, "rig-ref-b-fix.csv", &fixes(POS, n, 1.1))?;
    w(dir, "rig-ref-a-pps.csv", &pps(2.0, n))?;
    w(dir, "rig-ref-b-pps.csv", &pps(1.0, n))?;
    w(
        dir,
        "selftest-rig.toml",
        &format!(
            "format = \"kshana-simlab-selftest/1\"\nid = \"Replay rig check (synthetic)\"\nmode = \"replay-rig\"\nsource = \"AWG/SDR replay rig (synthetic)\"\n[expected]\nobservations = \"station-obs.csv\"\nposition = [{}, {}, {}]\npps_offset_ns = 0.0\n{tol}{}{}",
            POS[0], POS[1], POS[2],
            rx("REF-A", "rig-ref-a"),
            rx("REF-B", "rig-ref-b")
        ),
    )?;
    // Labs.
    w(dir, "lab-a.toml", LAB_A)?;
    w(dir, "lab-a.csv", LAB_A_CSV)?;
    w(dir, "lab-b.toml", LAB_B)?;
    w(dir, "lab-b.csv", LAB_B_CSV)?;
    w(dir, "tco.toml", TCO)?;
    Ok(())
}
