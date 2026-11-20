// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synthetic inputs for `leo-timing`: one time-error log per environment, an authentication
//! log and a plan naming them. Every log is marked `synthetic = true`, so a scorecard made
//! from them is MODELLED.

use std::path::Path;

/// A deterministic pseudo-random sequence (xorshift), uniform in [-1, 1).
fn noise(seed: u64) -> impl FnMut() -> f64 {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

/// A time-error log: `n` one-second samples, a random walk of step `walk_ns` plus white noise
/// `white_ns`, invalid for `gap` samples starting at `gap_at`.
pub fn te_log(
    seed: u64,
    n: usize,
    walk_ns: f64,
    white_ns: f64,
    gap_at: usize,
    gap: usize,
) -> String {
    let mut r = noise(seed);
    let mut w = 0.0;
    let mut out = String::from("t_s,te_ns,valid\n");
    for k in 0..n {
        w += walk_ns * r();
        let valid = !(gap_at..gap_at + gap).contains(&k);
        out += &format!("{k},{:.3},{}\n", w + white_ns * r(), u8::from(valid));
    }
    out
}

/// An authentication log with one message per `period_s`, latencies under `window_s`, and
/// `replays` old messages of which the receiver accepted `accepted_replays`.
pub fn auth_log(
    n: usize,
    period_s: f64,
    window_s: f64,
    replays: usize,
    accepted_replays: usize,
) -> String {
    let mut r = noise(7);
    let mut out = String::from("rx_s,tx_s,accepted\n");
    for k in 0..n {
        let tx = k as f64 * period_s;
        let lat = 0.2 * window_s * (1.0 + r()) * 0.5;
        out += &format!("{:.3},{tx:.3},1\n", tx + lat);
    }
    for k in 0..replays {
        let tx = k as f64 * period_s;
        let rx = tx + 3.0 * window_s + k as f64;
        out += &format!("{rx:.3},{tx:.3},{}\n", u8::from(k < accepted_replays));
    }
    out
}

/// The synthetic plan.
pub const PLAN: &str = r#"receiver = "synthetic-leo-timing-receiver"
service = "synthetic-leo-timing-service"
replay_window_s = 30.0

[thresholds]
max_abs_te_ns = 1000.0
availability_min = 0.95

[[claim]]
id = "indoor-p95"
metric = "p95_abs_te_ns"
environment = "indoor"
value = 500.0
source = "synthetic example claim"

[[claim]]
id = "open-availability"
metric = "availability"
environment = "open"
value = 0.999
source = "synthetic example claim"

[[log]]
file = "open.csv"
environment = "open"
synthetic = true

[[log]]
file = "indoor.csv"
environment = "indoor"
synthetic = true

[replay]
file = "auth.csv"
window_s = 30.0
synthetic = true
"#;

/// Write the plan and its logs under `dir`.
pub fn write_all(dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(dir.join("open.csv"), te_log(1, 3600, 0.5, 5.0, 0, 0)).map_err(io)?;
    std::fs::write(
        dir.join("indoor.csv"),
        te_log(2, 3600, 3.0, 30.0, 1200, 300),
    )
    .map_err(io)?;
    std::fs::write(dir.join("auth.csv"), auth_log(600, 6.0, 30.0, 5, 1)).map_err(io)?;
    std::fs::write(dir.join("plan.toml"), PLAN).map_err(io)?;
    Ok(())
}
