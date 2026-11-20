// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! SYNTHETIC inputs with known truth for the second B2404 slice (tests and trying the
//! commands). Every value here is SYNTHETIC, chosen to exercise the code, not a mission or
//! measured value.
//!
//! * `plume.toml`: a 30 m descent at 1 m/s; a line of sight 1 m above the surface from a
//!   beacon 300 m out to a rover 300 m out on the far side (through the sheet), and a high
//!   one from a mast 500 m out to the lander top (above it).
//! * `traverse.toml` + `traverse.csv`: a one-hour traverse east at 0.5 m/s with truth; the
//!   reported sigma (1 m) is honest except from 1700 s to 2000 s, where the error is three
//!   times larger; a crater 30 m off the track; consumables end at 4000 s with 600 s reserve,
//!   so the walk-back margin turns negative near 2263 s.
//! * `rf.toml` + `rf.sigmf-meta` + `rf.sigmf-data`: 0.128 s of complex noise at 2.048 MS/s
//!   centred on 2 GHz with a converter harmonic (always on), a motor-drive harmonic present
//!   only while the activity log says the motor is on, and one spur from no declared source.

use std::path::Path;

use kshana::sdr::Cf64;
use kshana::sigmf::{DataType, Meta, Recording};

/// Sample rate of the synthetic recording (Hz).
pub const FS: f64 = 2_048_000.0;
/// Centre frequency (Hz).
pub const CENTRE: f64 = 2_000_000_000.0;
/// Samples in the recording.
pub const N: usize = 1 << 18;
/// The converter fundamental and its harmonic in the band.
pub const DCDC_HZ: f64 = 400_050.0;
/// The motor-drive fundamental.
pub const MOTOR_HZ: f64 = 25_003.0;
/// Offset of the spur from no declared source (Hz).
pub const STRAY_OFFSET_HZ: f64 = 333_000.0;

/// Offset from the centre of the harmonic of `f0` nearest the centre.
pub fn harmonic_offset(f0: f64) -> (u32, f64) {
    let n = (CENTRE / f0).round();
    (n as u32, n * f0 - CENTRE)
}

/// Motor on between these times (s).
pub const MOTOR_ON: [(f64, f64); 2] = [(0.032, 0.064), (0.096, 0.128)];

struct Rng(u64);

impl Rng {
    fn u(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    /// A standard normal pair (Box-Muller).
    fn g2(&mut self) -> (f64, f64) {
        let (u1, u2) = (self.u(), self.u());
        let m = (-2.0 * u1.ln()).sqrt();
        let a = std::f64::consts::TAU * u2;
        (m * a.cos(), m * a.sin())
    }
}

/// The synthetic I/Q samples: unit-variance complex noise plus three tones of power 0.01.
pub fn iq() -> Vec<Cf64> {
    let mut g = Rng(0x5eed_1a2b);
    let (_, f_dcdc) = harmonic_offset(DCDC_HZ);
    let (_, f_motor) = harmonic_offset(MOTOR_HZ);
    let amp = 0.1;
    (0..N)
        .map(|k| {
            let t = k as f64 / FS;
            let (a, b) = g.g2();
            let mut re = a / std::f64::consts::SQRT_2;
            let mut im = b / std::f64::consts::SQRT_2;
            let mut tone = |f: f64| {
                let ph = std::f64::consts::TAU * f * t;
                re += amp * ph.cos();
                im += amp * ph.sin();
            };
            tone(f_dcdc);
            tone(STRAY_OFFSET_HZ);
            if MOTOR_ON.iter().any(|(s, e)| t >= *s && t < *e) {
                tone(f_motor);
            }
            Cf64::new(re, im)
        })
        .collect()
}

/// The traverse log CSV.
pub fn traverse_csv() -> String {
    let mut g = Rng(0x7a7e);
    let mut s = String::from("t_s,east_m,north_m,sigma_m,true_east_m,true_north_m\n");
    for t in 0..=3600 {
        let t = t as f64;
        let (te, tn) = (0.5 * t, 0.0);
        let scale = if (1700.0..=2000.0).contains(&t) {
            3.0
        } else {
            1.0
        };
        let (a, b) = g.g2();
        s.push_str(&format!(
            "{t},{:.4},{:.4},1.0,{te:.4},{tn:.4}\n",
            te + scale * a,
            tn + scale * b
        ));
    }
    s
}

const PLUME: &str = "title = \"SYNTHETIC plume visibility\"\n\
optical_threshold_db = 3.0\n\
\n\
[descent]\n\
start_height_m = 30.0\n\
rate_mps = 1.0\n\
hover_s = 2.0\n\
touchdown_height_m = 2.0\n\
step_s = 0.5\n\
\n\
[ejecta]\n\
mdot0_kg_s = 100.0\n\
scale_height_m = 5.0\n\
onset_height_m = 20.0\n\
particle_radius_m = 10e-6\n\
particle_density_kg_m3 = 3100.0\n\
speed_mps = 500.0\n\
angle_deg = 2.0\n\
base_thickness_m = 0.5\n\
r_min_m = 5.0\n\
r_max_m = 2000.0\n\
\n\
[rf]\n\
frequency_hz = 2.0e9\n\
permittivity_real = 3.0\n\
permittivity_imag = 0.03\n\
\n\
[[sights]]\n\
name = \"beacon-to-rover\"\n\
from = [300.0, 0.0, 1.0]\n\
to = [-300.0, 0.0, 1.0]\n\
\n\
[[sights]]\n\
name = \"beacon-high\"\n\
from = [500.0, 0.0, 40.0]\n\
to = [0.0, 0.0, 5.0]\n";

const TRAVERSE: &str = "title = \"SYNTHETIC traverse safety\"\n\
log = \"traverse.csv\"\n\
pl_k = 5.0\n\
alert_limit_m = 10.0\n\
decorrelation_s = 10.0\n\
\n\
[[hazards]]\n\
name = \"crater-A\"\n\
east_m = 900.0\n\
north_m = 30.0\n\
radius_m = 20.0\n\
\n\
[walkback]\n\
base = [0.0, 0.0]\n\
speed_mps = 1.0\n\
consumables_end_s = 4000.0\n\
reserve_s = 600.0\n";

fn rf_toml() -> String {
    format!(
        "title = \"SYNTHETIC self-interference\"\n\
meta = \"rf.sigmf-meta\"\n\
data = \"rf.sigmf-data\"\n\
nfft = 4096\n\
pfa = 1e-3\n\
signal_band_hz = [{lo:.1}, {hi:.1}]\n\
activity = \"activity.csv\"\n\
block_s = 0.016\n\
\n\
[[candidates]]\n\
name = \"dcdc-converter\"\n\
fundamental_hz = {DCDC_HZ:.1}\n\
tolerance_hz = 100.0\n\
\n\
[[candidates]]\n\
name = \"motor-drive\"\n\
fundamental_hz = {MOTOR_HZ:.1}\n\
tolerance_hz = 100.0\n",
        lo = CENTRE - 500_000.0,
        hi = CENTRE + 500_000.0
    )
}

fn activity_csv() -> String {
    let mut s = String::from("t_s,source,state\n0.0,motor-drive,off\n0.0,dcdc-converter,on\n");
    for (a, b) in MOTOR_ON {
        s.push_str(&format!("{a},motor-drive,on\n{b},motor-drive,off\n"));
    }
    s
}

/// Write every synthetic input into `dir`.
pub fn write(dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(dir.join("plume.toml"), PLUME).map_err(io)?;
    std::fs::write(dir.join("traverse.toml"), TRAVERSE).map_err(io)?;
    std::fs::write(dir.join("traverse.csv"), traverse_csv()).map_err(io)?;
    std::fs::write(dir.join("rf.toml"), rf_toml()).map_err(io)?;
    std::fs::write(dir.join("activity.csv"), activity_csv()).map_err(io)?;
    let rec = Recording {
        meta: Meta::new(
            DataType::Cf32Le,
            FS,
            CENTRE,
            "SYNTHETIC self-interference recording",
        ),
        samples: iq(),
    };
    let (meta, data, _) = kshana::sigmf::write(&rec, 1.0)?;
    std::fs::write(dir.join("rf.sigmf-meta"), meta).map_err(io)?;
    std::fs::write(dir.join("rf.sigmf-data"), data).map_err(io)?;
    Ok(())
}
