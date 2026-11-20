// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic inputs for every `altpnt` command (`kshana-pro altpnt synth <dir>`), with known
//! answers:
//!
//! * `architecture.toml`: five sources on a shared power bus (a single point of failure), a
//!   shared roof antenna, four failure modes (one tested to the field, one twin-only with a
//!   common-cause fallback, one with no detection and no test, one with signed lab evidence in
//!   `evidence/fm-leo-hil/`, signed with the synthetic key [`EVIDENCE_SEED`]);
//! * `labels.toml`: four claims (twin-optimistic and not met in flight, validated, twin-only,
//!   unsupported);
//! * `campaign.toml`: three runs along a 1 km straight line (nominal field; GNSS-denied field
//!   with 10 missing epochs, 50 misleading epochs and 30 of them unalerted; GNSS-denied twin)
//!   and four claims; `campaign-refused.toml` names a military signal and is refused;
//! * `time-iface.toml`: an hour of a resilient-time output (locked, a 20 min holdover that
//!   overstates its declared accuracy, a 60 s recovery, a 5 s gap).
//!
//! Nothing here is a real system, place or recording.

use std::fmt::Write as _;
use std::path::Path;

/// The synthetic lab's signing seed (synthetic evidence only; never a real key).
pub const EVIDENCE_SEED: [u8; 32] = [9u8; 32];
/// Route start `(lat, lon)`, degrees.
pub const START: (f64, f64) = (59.0, 24.0);
/// Speed along the route, m/s (east).
pub const SPEED: f64 = 10.0;

/// Metres per degree of latitude used to place synthetic offsets (spherical, R = 6 371 008.8 m).
pub fn m_per_deg_lat() -> f64 {
    6_371_008.8 * std::f64::consts::PI / 180.0
}

fn east(i: usize) -> f64 {
    START.1 + (SPEED * i as f64) / (m_per_deg_lat() * START.0.to_radians().cos())
}

pub fn architecture() -> &'static str {
    r#"title = "Synthetic A-PNT architecture"
system = "SYN-APNT-1"

[[source]]
id = "gnss"
kind = "gnss"
depends_on = ["ant-roof", "pwr-a"]

[[source]]
id = "leo"
kind = "leo"
depends_on = ["ant-roof", "pwr-a"]

[[source]]
id = "eloran"
kind = "eloran"
depends_on = ["ant-hfield", "pwr-a"]

[[source]]
id = "clock"
kind = "clock"
depends_on = ["pwr-a"]

[[source]]
id = "inertial"
kind = "inertial"
depends_on = ["pwr-a"]

[[failure_mode]]
id = "FM-GNSS-RFI"
source = "gnss"
mode = "Wideband interference: loss of fix"
effect = "no GNSS position or time"
detection = "C/N0 and AGC monitor"
mitigation = "carry on with the fallbacks"
fallback = ["leo", "eloran", "clock"]

[[failure_mode]]
id = "FM-ANT"
resource = "ant-roof"
mode = "Roof antenna or cable damage"
effect = "GNSS and LEO lost together"
detection = "antenna current monitor"
mitigation = "carry on with the fallbacks"
fallback = ["leo", "eloran"]

[[failure_mode]]
id = "FM-ELORAN"
source = "eloran"
mode = "Loss of the eLoran station"
effect = "no eLoran"
mitigation = "carry on with GNSS"
fallback = ["gnss"]

[[failure_mode]]
id = "FM-LEO"
source = "leo"
mode = "LEO service outage"
effect = "no LEO ranging"
detection = "service status message"
mitigation = "carry on with GNSS"
fallback = ["gnss", "eloran"]

[[test]]
id = "T-01"
failure_mode = "FM-GNSS-RFI"
environment = "twin"
outcome = "pass"

[[test]]
id = "T-02"
failure_mode = "FM-GNSS-RFI"
environment = "hil"
outcome = "pass"

[[test]]
id = "T-03"
failure_mode = "FM-GNSS-RFI"
environment = "field"
outcome = "pass"

[[test]]
id = "T-04"
failure_mode = "FM-ANT"
environment = "twin"
outcome = "pass"

[[test]]
id = "T-05"
failure_mode = "FM-LEO"
environment = "hil"
outcome = "pass"
evidence = "evidence/fm-leo-hil"
"#
}

pub fn labels() -> &'static str {
    r#"title = "Synthetic claims"
system = "SYN-APNT-1"

[[claim]]
id = "C1"
metric = "horizontal error 95 %, GNSS denied"
unit = "m"
better = "lower"
claimed = 50.0
stated_in = "synthetic datasheet"

[[claim]]
id = "C2"
metric = "availability, nominal"
unit = "fraction"
better = "higher"
claimed = 0.99
tolerance = 0.01

[[claim]]
id = "C3"
metric = "time error after 24 h holdover"
unit = "ns"
better = "lower"
claimed = 1000.0

[[claim]]
id = "C4"
metric = "time to alert"
unit = "s"
better = "lower"
claimed = 6.0

[[result]]
claim = "C1"
environment = "twin"
value = 20.0

[[result]]
claim = "C1"
environment = "flight"
value = 70.0

[[result]]
claim = "C1"
environment = "flight"
value = 60.0

[[result]]
claim = "C2"
environment = "simulator"
value = 0.995

[[result]]
claim = "C2"
environment = "field"
value = 0.993

[[result]]
claim = "C3"
environment = "twin"
value = 400.0
"#
}

/// One run's output CSV: `kind` is nominal, denied or twin.
pub fn output_csv(kind: &str) -> String {
    let mut s = String::from("t_s,lat_deg,lon_deg,alt_m,hpl_m,alert,time_error_ns\n");
    for i in 0..100usize {
        let x = i as f64;
        let (err, hpl, alert) = match kind {
            "nominal" => (2.0 + (i % 5) as f64, 20.0, false),
            "denied" => {
                if (40..50).contains(&i) {
                    continue;
                }
                (1.0 + 0.5 * x, 10.0 + 0.3 * x, i >= 80)
            }
            _ => (1.0 + 0.1 * x, 10.0 + 0.3 * x, false),
        };
        let lat = START.0 + err / m_per_deg_lat();
        let _ = writeln!(
            s,
            "{x},{lat:.9},{:.9},{:.2},{hpl:.2},{},{}",
            east(i),
            30.0 + err / 2.0,
            u8::from(alert),
            10 + i % 3
        );
    }
    s
}

/// The truth CSV: 100 s at 1 Hz, east at [`SPEED`].
pub fn truth_csv() -> String {
    let mut s = String::from("t_s,lat_deg,lon_deg,alt_m\n");
    for i in 0..100usize {
        let _ = writeln!(s, "{i},{:.9},{:.9},30.00", START.0, east(i));
    }
    s
}

pub fn campaign(refused: bool) -> String {
    let signals = if refused {
        r#"["GPS L1 C/A", "Galileo PRS"]"#
    } else {
        r#"["GPS L1 C/A", "eLoran 100 kHz"]"#
    };
    format!(
        r#"title = "Synthetic civil A-PNT campaign"
system = "SYN-APNT-1"
end_use = "civil maritime and aviation resilience trials"
match_window_s = 0.5

[[dataset]]
id = "D1"
classification = "unclassified"
release = "public"
signals = {signals}
owner = "synthetic"

[[run]]
id = "R1"
dataset = "D1"
condition = "nominal"
environment = "field"
output = "runs/r1-output.csv"
truth = "runs/truth.csv"

[[run]]
id = "R2"
dataset = "D1"
condition = "gnss-denied"
environment = "field"
output = "runs/r2-output.csv"
truth = "runs/truth.csv"

[[run]]
id = "R3"
dataset = "D1"
condition = "gnss-denied"
environment = "twin"
output = "runs/r3-output.csv"
truth = "runs/truth.csv"

[[claim]]
id = "K1"
condition = "gnss-denied"
metric = "h95_m"
max = 30.0

[[claim]]
id = "K2"
condition = "nominal"
metric = "availability"
min = 0.99

[[claim]]
id = "K3"
condition = "gnss-denied"
metric = "hmi_rate"
max = 0.001

[[claim]]
id = "K4"
condition = "gnss-jammed"
metric = "h95_m"
max = 100.0
"#
    )
}

/// An hour of a resilient-time output, 1 Hz.
pub fn time_log() -> String {
    let mut s = String::from("t_s,offset_ns,state,declared_accuracy_ns\n");
    for i in 0..3600usize {
        let x = i as f64;
        let (off, st, decl) = match i {
            0..=1199 => (20.0 * (x / 50.0).sin(), "locked", 50.0),
            1200..=2399 => (20.0 + 0.5 * (x - 1200.0), "holdover", 100.0),
            2400..=2459 => (620.0 - 10.0 * (x - 2400.0), "recovering", 1000.0),
            3000..=3004 => continue,
            _ => (20.0 * (x / 50.0).sin(), "locked", 50.0),
        };
        let _ = writeln!(s, "{x},{off:.3},{st},{decl}");
    }
    s
}

pub fn time_request() -> &'static str {
    r#"title = "Synthetic resilient-time output"
service = "SYN-RT-1 civil output"
interface = "PTP"
reference = "synthetic independent reference"
log = "time-log.csv"

[limits]
locked_ns = 100.0
holdover_profile = [[0.0, 100.0], [600.0, 400.0], [1200.0, 700.0]]
max_recovery_s = 300.0
max_gap_s = 2.0
mtie_mask = [[1.0, 50.0], [10.0, 80.0], [100.0, 100.0]]
"#
}

/// Write every synthetic input under `dir`.
pub fn write_all(dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("runs")).map_err(io)?;
    let ev = dir.join("evidence/fm-leo-hil");
    std::fs::create_dir_all(&ev).map_err(io)?;
    std::fs::write(
        ev.join("result.txt"),
        "FM-LEO HIL test, synthetic: outage injected at 120 s, detected at 121 s, outputs carried on.\n",
    )
    .map_err(io)?;
    crate::signed_dossier::sign(&ev, &EVIDENCE_SEED, "synthetic-lab", "2026-11-20T00:00:00Z")
        .map_err(|e| e.to_string())?;
    std::fs::write(dir.join("architecture.toml"), architecture()).map_err(io)?;
    std::fs::write(dir.join("labels.toml"), labels()).map_err(io)?;
    std::fs::write(dir.join("runs/truth.csv"), truth_csv()).map_err(io)?;
    for (f, k) in [("r1", "nominal"), ("r2", "denied"), ("r3", "twin")] {
        std::fs::write(dir.join(format!("runs/{f}-output.csv")), output_csv(k)).map_err(io)?;
    }
    std::fs::write(dir.join("campaign.toml"), campaign(false)).map_err(io)?;
    std::fs::write(dir.join("campaign-refused.toml"), campaign(true)).map_err(io)?;
    std::fs::write(dir.join("time-log.csv"), time_log()).map_err(io)?;
    std::fs::write(dir.join("time-iface.toml"), time_request()).map_err(io)?;
    Ok(())
}
