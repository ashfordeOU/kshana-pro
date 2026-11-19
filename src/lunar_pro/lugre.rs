// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D6. LuGRE C/N0 reproduction harness, pre-registered (gap 14)
//!
//! LuGRE (NASA/ASI, Firefly Blue Ghost Mission 1, March 2025) tracked GPS and Galileo from the
//! lunar surface, and its received C/N0 was **7–12 dB below the planning simulation**, with no
//! correlation to altitude, temperature or mission phase (research note 03 §1.4, source [21]:
//! NAVIGATION doi:10.33012/navi.756; open data on Zenodo doi:10.5281/zenodo.16411686). This
//! module is a harness for an independent, pre-registered reproduction of that comparison:
//!
//! 1. **Link-budget model** ([`Model::predict`]): for each GNSS satellite and epoch, the
//!    transmit EIRP at the off-boresight (off-nadir) angle toward the receiver, read from an
//!    input side-lobe table; the free-space loss; the receive antenna gain at the off-boresight
//!    angle of the satellite, from an input table; and the system noise temperature from the
//!    antenna temperature, the loss ahead of the LNA and the LNA noise figure,
//!    `T_sys = T_A + (L−1)·T_0 + L·(F−1)·T_0` (T_0 = 290 K), giving
//!    `C/N0 = EIRP(θ) + G_R(z) − FSL + 228.6 − 10·log10(T_sys) − L_impl`. The link equation
//!    itself is the open engine's [`kshana::earth_gnss_lunar::measured_link`] (Montenbruck et al.
//!    2023, Eq. 4), called with the EIRP table as the transmit pattern and zero transmit power.
//!    Geometry: GNSS positions from an SP3 file (the engine's parser and its 11-point
//!    interpolator, in its TEME frame), the Moon from the engine's series
//!    ([`kshana::ephem::moon_icrf`], ≈0.05° from JPL Horizons), rotated to the mean equator of
//!    date ([`kshana::precession::gcrs_to_mod`]); a surface receiver is placed in the Moon PA
//!    frame ([`kshana::lunar_frame::icrf_to_moon_pa`]). Earth occultation (with an optional
//!    atmosphere mask height) and Moon occultation remove links.
//! 2. **Pre-registration** ([`PreRegistration`]): a TOML file, written *before any data is
//!    seen*, that fixes the model file by SHA-256 (the model file in turn fixes every pattern
//!    table and trajectory by SHA-256), the signals, the exclusion rules, the statistic, the
//!    coverage factor, the bootstrap seed and the bars. Its own SHA-256 is published (a commit,
//!    a registry) before the data are opened; the scorer refuses to run unless the file it is
//!    given has that hash, the model file has the registered hash, and `data_seen = false`. This
//!    follows the repository's existing pattern (`pnt_study`: `pre_registration` +
//!    `pre_registration_sha256`, checked before scoring).
//! 3. **Scorer** ([`score`]): matches observed C/N0 (LuGRE-style CSV, read with the D5 LUNEX
//!    converter [`super::lunex::from_csv`]) to predictions, forms residuals
//!    `observed − predicted`, clusters them by satellite and signal (a pass is one cluster, so
//!    correlated samples do not inflate the precision), and reports the bias with a Type A
//!    uncertainty (cluster standard error, plus a percentile cluster bootstrap) and a Type B
//!    uncertainty (the model's own budget: pattern tables, receive gain, noise temperature, noise
//!    figure, implementation loss and the geometric angle error times the pattern slope), against
//!    the published 7–12 dB shortfall.
//!
//! **Input schema (needs confirmation against the Zenodo record).** The observation CSV is the
//! D5 LuGRE-style CSV `time,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz`, time in
//! GPS time `YYYY-MM-DDTHH:MM:SS[.f]`. It is modelled on the kind of fields the LuGRE open
//! dataset carries; it is **not** that dataset's exact schema, which has not been checked here,
//! and the real data have **not** been downloaded (they are large: raw IQ batches). A converter
//! from the Zenodo files to this CSV is the first step when the data are taken up.
//!
//! **Evidence label: MODELLED.** The harness is checked on synthetic observations with an
//! injected bias (recovery, coverage, tamper refusal). No LuGRE observation has been scored, so
//! nothing here says anything yet about the real 7–12 dB gap. The synthetic example's patterns
//! are invented shapes, not ACE or Lockheed measurements.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::earth_gnss_lunar::{measured_link, segment_blocked_by_sphere, GainTable};
use kshana::orbit::R_EARTH_EQUATORIAL_M;
use kshana::rinex::EpochUtc;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::budget::std_normal;
use super::{Budget, Term, TermKind};

/// The statement every output of this module carries.
pub const STATUS: &str = "MODELLED harness (Kshana Pro D6): checked on synthetic observations \
    with an injected bias; no LuGRE observation has been scored; the input schema needs \
    confirmation against the Zenodo record";
/// The published shortfall (predicted − observed C/N0), dB.
pub const PUBLISHED_SHORTFALL_DB: [f64; 2] = [7.0, 12.0];
/// Its source.
pub const PUBLISHED_SOURCE: &str = "LuGRE results, NAVIGATION doi:10.33012/navi.756 \
    (C/N0 7-12 dB below the planning simulation); open data doi:10.5281/zenodo.16411686";
/// Reference noise temperature (K).
pub const T0_K: f64 = 290.0;
/// Mean lunar radius (m), IAU.
pub const R_MOON_M: f64 = 1_737_400.0;
/// The pre-registration format tag.
pub const PREREG_FORMAT: &str = "kshana-lugre-prereg/1";
/// GPS − TT offset (s): TT = GPS + 19 s + 32.184 s.
const GPS_TO_TT_S: f64 = 51.184;

type Vec3 = [f64; 3];

/// SHA-256 of bytes, lowercase hex.
pub fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Carrier frequency (Hz) of a band name.
pub fn band_hz(band: &str) -> Result<f64, String> {
    match band.to_ascii_uppercase().as_str() {
        "L1" | "E1" | "B1C" => Ok(1_575.42e6),
        "L2" => Ok(1_227.60e6),
        "L5" | "E5A" | "B2A" => Ok(1_176.45e6),
        "E5B" => Ok(1_207.14e6),
        o => Err(format!("unknown band '{o}' (L1, L2, L5, E1, E5a, E5b)")),
    }
}

/// System noise temperature (K) at the antenna terminals: `T_A + (L−1)·T_0 + L·(F−1)·T_0`,
/// with `L` the loss ahead of the LNA and `F` the LNA noise factor.
pub fn system_temperature_k(antenna_temp_k: f64, lna_nf_db: f64, pre_lna_loss_db: f64) -> f64 {
    let l = 10f64.powf(pre_lna_loss_db / 10.0);
    let f = 10f64.powf(lna_nf_db / 10.0);
    antenna_temp_k + (l - 1.0) * T0_K + l * (f - 1.0) * T0_K
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Read a two-column table `angle_deg,<value>` (header and `#` comments allowed).
pub fn parse_table(text: &str) -> Result<GainTable, String> {
    let mut pts = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("angle") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() != 2 {
            return Err(format!("line {}: expected angle_deg,value", i + 1));
        }
        let a: f64 = f[0]
            .parse()
            .map_err(|_| format!("line {}: bad angle '{}'", i + 1, f[0]))?;
        let v: f64 = f[1]
            .parse()
            .map_err(|_| format!("line {}: bad value '{}'", i + 1, f[1]))?;
        if !a.is_finite() || !v.is_finite() {
            return Err(format!("line {}: not finite", i + 1));
        }
        pts.push((a, v));
    }
    GainTable::new(&pts).ok_or_else(|| "table needs two or more strictly increasing angles".into())
}

// ------------------------------------------------------------------------------ model file

/// The receiver's location.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Site {
    /// A fixed point on the Moon, selenographic in the PA frame, on the mean sphere + height.
    Surface {
        lat_deg: f64,
        lon_deg: f64,
        #[serde(default)]
        height_m: f64,
    },
    /// A trajectory table `time,x_m,y_m,z_m` (GPS time; geocentric GCRS/ICRF), linearly
    /// interpolated, never extrapolated.
    Trajectory { file: String, sha256: String },
}

/// The receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverConfig {
    pub site: Site,
    /// `earth` (boresight toward the geocentre, as LuGRE's high-gain antenna) or `zenith`
    /// (selenocentric radial).
    pub boresight: String,
    /// Receive gain table `angle_deg,gain_dbi`, and its SHA-256.
    pub antenna_pattern: String,
    pub antenna_pattern_sha256: String,
    #[serde(default)]
    pub antenna_gain_u_db: f64,
    pub antenna_temp_k: f64,
    #[serde(default)]
    pub antenna_temp_u_k: f64,
    pub lna_nf_db: f64,
    #[serde(default)]
    pub lna_nf_u_db: f64,
    #[serde(default)]
    pub pre_lna_loss_db: f64,
    #[serde(default)]
    pub implementation_loss_db: f64,
    #[serde(default)]
    pub implementation_loss_u_db: f64,
    /// Height above the Earth's equatorial radius below which a ray counts as blocked (m).
    #[serde(default)]
    pub earth_mask_altitude_m: f64,
    /// Standard uncertainty of the transmit off-boresight angle from the geometry (deg).
    #[serde(default)]
    pub ephemeris_angle_u_deg: f64,
}

/// One transmit EIRP table: `key` is a satellite (`G05`), a block named in `blocks`, or a
/// system letter (`G`); `band` its band; `file` the table `angle_deg,eirp_dbw`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransmitPattern {
    pub key: String,
    pub band: String,
    pub file: String,
    pub sha256: String,
    /// Standard uncertainty of the table (dB), common to every link that uses it.
    #[serde(default)]
    pub u_db: f64,
}

/// The model file (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    pub title: String,
    pub receiver: ReceiverConfig,
    pub transmit: Vec<TransmitPattern>,
    /// `"G:1C" = "L1"`: observed signal (system letter : RINEX signal) to band.
    pub signals: BTreeMap<String, String>,
    /// Satellite to block name (optional).
    #[serde(default)]
    pub blocks: BTreeMap<String, String>,
}

/// A loaded model: every table read and its hash checked.
#[derive(Clone, Debug)]
pub struct Model {
    pub config: ModelConfig,
    pub sha256: String,
    tx: Vec<(TransmitPattern, GainTable)>,
    rx: GainTable,
    traj: Vec<(f64, Vec3)>,
}

fn read_checked(base: &Path, file: &str, sha: &str) -> Result<String, String> {
    let p = base.join(file);
    let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let got = sha256_hex(&b);
    if !got.eq_ignore_ascii_case(sha) {
        return Err(format!(
            "{file}: SHA-256 {got} does not match the model's {sha} (the table changed)"
        ));
    }
    String::from_utf8(b).map_err(|_| format!("{file}: not UTF-8"))
}

/// Seconds since J2000 (same day/second split as the D5 converter).
fn secs(t: &EpochUtc) -> f64 {
    super::lunex::secs(t)
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.f]`.
pub fn parse_epoch(s: &str) -> Result<EpochUtc, String> {
    let bad = || format!("bad time '{s}' (YYYY-MM-DDTHH:MM:SS[.f])");
    let (d, t) = s
        .trim()
        .trim_end_matches('Z')
        .split_once('T')
        .ok_or_else(bad)?;
    let dp: Vec<&str> = d.split('-').collect();
    let tp: Vec<&str> = t.split(':').collect();
    if dp.len() != 3 || tp.len() != 3 {
        return Err(bad());
    }
    Ok(EpochUtc {
        year: dp[0].parse().map_err(|_| bad())?,
        month: dp[1].parse().map_err(|_| bad())?,
        day: dp[2].parse().map_err(|_| bad())?,
        hour: tp[0].parse().map_err(|_| bad())?,
        minute: tp[1].parse().map_err(|_| bad())?,
        second: tp[2].parse().map_err(|_| bad())?,
    })
}

/// Format an epoch as `YYYY-MM-DDTHH:MM:SS.fff`.
pub fn fmt_epoch(t: &EpochUtc) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:06.3}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

/// The inverse of the D5 `secs` (seconds since 2000-01-01T12:00:00 of the same scale).
pub fn epoch_from_secs(s: f64) -> EpochUtc {
    // Seconds since 2000-01-01T00:00:00, split into whole days and seconds of day.
    let m = s + 43_200.0;
    let day = (m / 86_400.0).floor();
    let mut sod = m - day * 86_400.0;
    // Round to the microsecond so labels do not read 59.999999.
    sod = (sod * 1e6).round() / 1e6;
    let (day, sod) = if sod >= 86_400.0 {
        (day + 1.0, sod - 86_400.0)
    } else {
        (day, sod)
    };
    // The civil date of that day's noon (robust against the JD's half-day origin).
    let c = kshana::timescales::civil_from_jd(kshana::timescales::JD_J2000 + day);
    let h = (sod / 3600.0).floor();
    let mi = ((sod - h * 3600.0) / 60.0).floor();
    EpochUtc {
        year: c.year,
        month: c.month,
        day: c.day,
        hour: h as u32,
        minute: mi as u32,
        second: sod - h * 3600.0 - mi * 60.0,
    }
}

/// The geometry at one epoch, in the mean equator and equinox of date (≈ the SP3 TEME frame to
/// the nutation level, ≈20″): receiver, boresight, Moon centre.
#[derive(Clone, Copy, Debug)]
pub struct RxGeometry {
    pub rx: Vec3,
    pub boresight: Vec3,
    pub moon: Vec3,
}

/// One predicted link.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Prediction {
    pub time: String,
    pub sat: String,
    pub signal: String,
    pub band: String,
    pub pattern: String,
    pub theta_tx_deg: f64,
    pub z_rx_deg: f64,
    pub range_m: f64,
    pub fsl_db: f64,
    pub eirp_dbw: Option<f64>,
    pub rx_gain_dbi: Option<f64>,
    pub occulted_earth: bool,
    pub occulted_moon: bool,
    /// `None`: occulted, or an angle outside a table.
    pub cn0_dbhz: Option<f64>,
    /// d EIRP / d θ at the angle (dB/deg), for the geometric term of the budget.
    pub slope_db_per_deg: f64,
}

impl Model {
    /// Load a model file; every referenced table must match its SHA-256.
    pub fn load(path: &Path) -> Result<Model, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Model::from_bytes(&bytes, &base)
    }

    /// Load from the model file's bytes, resolving tables against `base`.
    pub fn from_bytes(bytes: &[u8], base: &Path) -> Result<Model, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "model: not UTF-8".to_string())?;
        let config: ModelConfig = toml::from_str(text).map_err(|e| format!("model: {e}"))?;
        let r = &config.receiver;
        if r.boresight != "earth" && r.boresight != "zenith" {
            return Err(format!(
                "receiver.boresight '{}' is not earth or zenith",
                r.boresight
            ));
        }
        for (n, v) in [
            ("antenna_temp_k", r.antenna_temp_k),
            ("lna_nf_db", r.lna_nf_db),
            ("pre_lna_loss_db", r.pre_lna_loss_db),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("receiver.{n} must be finite and >= 0"));
            }
        }
        let rx = parse_table(&read_checked(
            base,
            &r.antenna_pattern,
            &r.antenna_pattern_sha256,
        )?)
        .map_err(|e| format!("{}: {e}", r.antenna_pattern))?;
        let mut tx = Vec::new();
        for p in &config.transmit {
            band_hz(&p.band)?;
            let t = parse_table(&read_checked(base, &p.file, &p.sha256)?)
                .map_err(|e| format!("{}: {e}", p.file))?;
            tx.push((p.clone(), t));
        }
        for (k, b) in &config.signals {
            band_hz(b)?;
            if k.len() < 3 || k.as_bytes()[1] != b':' {
                return Err(format!("signals: key '{k}' is not <system>:<signal>"));
            }
        }
        let mut traj = Vec::new();
        if let Site::Trajectory { file, sha256 } = &r.site {
            let text = read_checked(base, file, sha256)?;
            for (i, l) in text.lines().enumerate() {
                let l = l.trim();
                if l.is_empty() || l.starts_with('#') || l.starts_with("time") {
                    continue;
                }
                let f: Vec<&str> = l.split(',').map(str::trim).collect();
                if f.len() != 4 {
                    return Err(format!("{file} line {}: expected time,x_m,y_m,z_m", i + 1));
                }
                let t = secs(&parse_epoch(f[0])?);
                let mut v = [0.0; 3];
                for k in 0..3 {
                    v[k] = f[k + 1]
                        .parse()
                        .map_err(|_| format!("{file} line {}: bad number", i + 1))?;
                }
                traj.push((t, v));
            }
            if traj.len() < 2 || traj.windows(2).any(|w| w[1].0 <= w[0].0) {
                return Err(format!("{file}: needs two or more increasing epochs"));
            }
        }
        Ok(Model {
            config,
            sha256: sha256_hex(bytes),
            tx,
            rx,
            traj,
        })
    }

    /// The system noise temperature (K) of the configured receiver.
    pub fn system_temperature_k(&self) -> f64 {
        let r = &self.config.receiver;
        system_temperature_k(r.antenna_temp_k, r.lna_nf_db, r.pre_lna_loss_db)
    }

    /// `10·log10(T_sys) + L_impl` (dB), the noise term of the link equation.
    pub fn noise_factor_db(&self) -> f64 {
        10.0 * self.system_temperature_k().log10() + self.config.receiver.implementation_loss_db
    }

    /// The band of an observed signal (`G`, `1C`), if configured.
    pub fn band_of(&self, sat: &str, signal: &str) -> Option<&str> {
        let sys = &sat[..1];
        self.config
            .signals
            .get(&format!("{sys}:{signal}"))
            .map(String::as_str)
    }

    /// The transmit table for a satellite and band: by satellite, then block, then system.
    pub fn pattern_for(&self, sat: &str, band: &str) -> Option<(&TransmitPattern, &GainTable)> {
        let block = self.config.blocks.get(sat).map(String::as_str);
        let sys = &sat[..1];
        for key in [Some(sat), block, Some(sys)].into_iter().flatten() {
            if let Some((p, t)) = self
                .tx
                .iter()
                .find(|(p, _)| p.key == key && p.band.eq_ignore_ascii_case(band))
            {
                return Some((p, t));
            }
        }
        None
    }

    /// The receiver geometry at a GPS-time epoch, or `None` outside a trajectory table.
    pub fn geometry(&self, t_gps: &EpochUtc) -> Option<RxGeometry> {
        let s = secs(t_gps);
        let jd_tt = kshana::timescales::JD_J2000 + (s + GPS_TO_TT_S) / 86_400.0;
        let moon_icrf = kshana::ephem::moon_icrf(jd_tt);
        let to_mod = |v: Vec3| kshana::precession::gcrs_to_mod(v, jd_tt);
        let (rx_icrf, radial_icrf) = match &self.config.receiver.site {
            Site::Surface {
                lat_deg,
                lon_deg,
                height_m,
            } => {
                let r = R_MOON_M + height_m;
                let (la, lo) = (lat_deg.to_radians(), lon_deg.to_radians());
                let pa = [
                    r * la.cos() * lo.cos(),
                    r * la.cos() * lo.sin(),
                    r * la.sin(),
                ];
                let m = kshana::lunar_frame::icrf_to_moon_pa(jd_tt);
                let off = kshana::precession::mat_vec(&kshana::precession::transpose(&m), pa);
                (add(moon_icrf, off), off)
            }
            Site::Trajectory { .. } => {
                let tr = &self.traj;
                if s < tr[0].0 || s > tr[tr.len() - 1].0 {
                    return None;
                }
                let k = tr.partition_point(|x| x.0 <= s).clamp(1, tr.len() - 1);
                let (t0, a) = tr[k - 1];
                let (t1, b) = tr[k];
                let f = (s - t0) / (t1 - t0);
                let p = [
                    a[0] + f * (b[0] - a[0]),
                    a[1] + f * (b[1] - a[1]),
                    a[2] + f * (b[2] - a[2]),
                ];
                (p, sub(p, moon_icrf))
            }
        };
        let rx = to_mod(rx_icrf);
        let boresight = if self.config.receiver.boresight == "earth" {
            [-rx[0], -rx[1], -rx[2]]
        } else {
            to_mod(radial_icrf)
        };
        Some(RxGeometry {
            rx,
            boresight,
            moon: to_mod(moon_icrf),
        })
    }

    /// Predict one link from a transmitter position (same frame as `g`).
    pub fn predict_link(
        &self,
        g: &RxGeometry,
        tx: Vec3,
        sat: &str,
        signal: &str,
        time: &str,
    ) -> Option<Prediction> {
        let band = self.band_of(sat, signal)?.to_string();
        let (pat, table) = self.pattern_for(sat, &band)?;
        let f = band_hz(&band).ok()?;
        let r = &self.config.receiver;
        let l = measured_link(
            tx,
            0.0,
            table,
            g.rx,
            g.boresight,
            &self.rx,
            f,
            self.noise_factor_db(),
            R_EARTH_EQUATORIAL_M + r.earth_mask_altitude_m,
        )?;
        // Moon occultation (10 m tolerance so a surface receiver does not occult itself).
        let occulted_moon = segment_blocked_by_sphere(
            sub(tx, g.moon),
            sub(g.rx, g.moon),
            R_MOON_M
                + match r.site {
                    Site::Surface { height_m, .. } => height_m.min(0.0),
                    _ => 0.0,
                }
                - 10.0,
        );
        let eirp = table.gain_at(l.theta_tx_deg);
        let grx = self.rx.gain_at(l.z_rx_deg);
        let slope = match (
            table.gain_at(l.theta_tx_deg + 0.05),
            table.gain_at(l.theta_tx_deg - 0.05),
        ) {
            (Some(a), Some(b)) => (a - b) / 0.1,
            _ => 0.0,
        };
        Some(Prediction {
            time: time.to_string(),
            sat: sat.to_string(),
            signal: signal.to_string(),
            band,
            pattern: format!("{}/{}", pat.key, pat.band),
            theta_tx_deg: l.theta_tx_deg,
            z_rx_deg: l.z_rx_deg,
            range_m: l.range_m,
            fsl_db: l.free_space_loss_db,
            eirp_dbw: eirp,
            rx_gain_dbi: grx,
            occulted_earth: l.occulted,
            occulted_moon,
            cn0_dbhz: if l.occulted || occulted_moon {
                None
            } else {
                l.cn0_dbhz
            },
            slope_db_per_deg: slope,
        })
    }

    /// Predict a link at a GPS-time epoch with satellite positions from `src`.
    pub fn predict(
        &self,
        src: &dyn GnssSource,
        t_gps: &EpochUtc,
        sat: &str,
        signal: &str,
    ) -> Option<Prediction> {
        let g = self.geometry(t_gps)?;
        let tx = src.position(sat, t_gps)?;
        self.predict_link(&g, tx, sat, signal, &fmt_epoch(t_gps))
    }
}

// ------------------------------------------------------------------------------ GNSS source

/// A source of GNSS satellite positions in the mean-of-date / TEME frame (m).
pub trait GnssSource {
    fn position(&self, sat: &str, t_gps: &EpochUtc) -> Option<Vec3>;
    fn satellites(&self) -> Vec<String>;
}

/// SP3 precise orbits through the open engine's parser and interpolator.
pub struct Sp3Source {
    start: EpochUtc,
    end_s: f64,
    interps: BTreeMap<String, kshana::sp3::Sp3Interpolator>,
    pub sha256: String,
}

impl Sp3Source {
    pub fn parse(text: &str) -> Result<Sp3Source, String> {
        let f = kshana::sp3::parse_sp3(text)?;
        let start = f.header.start;
        let end_s = f
            .epochs
            .last()
            .map(|e| secs(&e.time) - secs(&start))
            .ok_or("SP3: no epochs")?;
        let interps = f
            .observed_satellites()
            .into_iter()
            .filter_map(|s| f.interpolator(&s).map(|i| (s, i)))
            .collect();
        Ok(Sp3Source {
            start,
            end_s,
            interps,
            sha256: sha256_hex(text.as_bytes()),
        })
    }
}

impl GnssSource for Sp3Source {
    fn position(&self, sat: &str, t_gps: &EpochUtc) -> Option<Vec3> {
        let t = secs(t_gps) - secs(&self.start);
        if !(0.0..=self.end_s).contains(&t) {
            return None;
        }
        self.interps.get(sat).map(|i| i.position_teme(t))
    }
    fn satellites(&self) -> Vec<String> {
        self.interps.keys().cloned().collect()
    }
}

/// Predict every configured signal of every satellite from `start` to `end` every `step_s`.
pub fn predict_series(
    model: &Model,
    src: &dyn GnssSource,
    start: &EpochUtc,
    end: &EpochUtc,
    step_s: f64,
) -> Result<Vec<Prediction>, String> {
    if step_s.is_nan() || step_s <= 0.0 {
        return Err("step must be > 0".into());
    }
    let (a, b) = (secs(start), secs(end));
    let n = ((b - a) / step_s).floor() as usize;
    if n > 2_000_000 {
        return Err("too many epochs".into());
    }
    let mut out = Vec::new();
    for k in 0..=n {
        let t = epoch_from_secs(a + k as f64 * step_s);
        let Some(g) = model.geometry(&t) else {
            continue;
        };
        for sat in src.satellites() {
            let Some(tx) = src.position(&sat, &t) else {
                continue;
            };
            for (key, _) in model.config.signals.iter() {
                let (sys, sig) = key.split_at(1);
                if sat.starts_with(sys) {
                    if let Some(p) = model.predict_link(&g, tx, &sat, &sig[1..], &fmt_epoch(&t)) {
                        out.push(p);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Predictions as CSV.
pub fn predictions_csv(p: &[Prediction]) -> String {
    let mut s = String::from(
        "time,sat,signal,band,pattern,theta_tx_deg,z_rx_deg,range_m,fsl_db,eirp_dbw,rx_gain_dbi,occulted_earth,occulted_moon,cn0_dbhz\n",
    );
    let o = |x: Option<f64>| x.map(|v| format!("{v:.3}")).unwrap_or_default();
    for r in p {
        s.push_str(&format!(
            "{},{},{},{},{},{:.4},{:.4},{:.1},{:.3},{},{},{},{},{}\n",
            r.time,
            r.sat,
            r.signal,
            r.band,
            r.pattern,
            r.theta_tx_deg,
            r.z_rx_deg,
            r.range_m,
            r.fsl_db,
            o(r.eirp_dbw),
            o(r.rx_gain_dbi),
            r.occulted_earth,
            r.occulted_moon,
            o(r.cn0_dbhz)
        ));
    }
    s
}

// ------------------------------------------------------------------------------ observations

/// One observed C/N0.
#[derive(Clone, Debug, PartialEq)]
pub struct ObservedCn0 {
    pub time: EpochUtc,
    pub sat: String,
    pub signal: String,
    pub cn0_dbhz: f64,
}

/// Read observed C/N0 from a LuGRE-style CSV through the D5 converter (`S<signal>` codes).
pub fn read_observations(csv: &str) -> Result<Vec<ObservedCn0>, String> {
    let lx = super::lunex::from_csv(csv, &super::lunex::CsvOptions::default())?;
    let mut out = Vec::new();
    for e in &lx.epochs {
        for so in &e.sats {
            let sys = so.sat.chars().next().unwrap_or(' ');
            let Some((_, codes)) = lx.header.obs_types.iter().find(|(c, _)| *c == sys) else {
                continue;
            };
            for (i, c) in codes.iter().enumerate() {
                if let (Some(sig), Some(Some(o))) = (c.strip_prefix('S'), so.obs.get(i)) {
                    out.push(ObservedCn0 {
                        time: e.time,
                        sat: so.sat.clone(),
                        signal: sig.to_string(),
                        cn0_dbhz: o.value,
                    });
                }
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------------------ pre-registration

/// The fixed analysis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analysis {
    /// Signals scored (`G:1C`, ...); every other observation is excluded and counted.
    pub signals: Vec<String>,
    /// Observations below this C/N0 are excluded (the receiver's reliable floor).
    #[serde(default)]
    pub min_observed_dbhz: Option<f64>,
    /// A cluster (satellite + signal) with fewer observations is dropped.
    pub min_cluster_obs: usize,
    /// Coverage factor of the reported interval.
    pub coverage_k: f64,
    pub bootstrap_n: usize,
    pub bootstrap_seed: u64,
    pub published_shortfall_db: [f64; 2],
}

/// A pre-registered bar: `quantity op value`, optionally on one signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar {
    pub id: String,
    /// `shortfall_db`, `shortfall_lo_db`, `shortfall_hi_db`, `abs_bias_db`, `u_total_db`,
    /// `n_clusters`, `n_obs` or `overlap_published` (1 or 0).
    pub quantity: String,
    /// `ge` or `le`.
    pub op: String,
    pub value: f64,
    #[serde(default)]
    pub signal: Option<String>,
    #[serde(default)]
    pub hypothesis: String,
}

/// The pre-registration file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreRegistration {
    pub format: String,
    pub title: String,
    pub registrant: String,
    pub registered_utc: String,
    /// Must be `false`: the registrant declares no observation had been opened.
    pub data_seen: bool,
    pub model_sha256: String,
    /// What the observation data will be (for the record), e.g. the Zenodo DOI.
    pub data_source: String,
    pub analysis: Analysis,
    pub bar: Vec<Bar>,
}

const QUANTITIES: [&str; 8] = [
    "shortfall_db",
    "shortfall_lo_db",
    "shortfall_hi_db",
    "abs_bias_db",
    "u_total_db",
    "n_clusters",
    "n_obs",
    "overlap_published",
];

impl PreRegistration {
    /// Parse and check a pre-registration's own consistency.
    pub fn parse(text: &str) -> Result<PreRegistration, String> {
        let p: PreRegistration =
            toml::from_str(text).map_err(|e| format!("pre-registration: {e}"))?;
        if p.format != PREREG_FORMAT {
            return Err(format!("pre-registration: format must be {PREREG_FORMAT}"));
        }
        if p.data_seen {
            return Err(
                "pre-registration declares data_seen = true: not a pre-registration".into(),
            );
        }
        let a = &p.analysis;
        if a.signals.is_empty()
            || a.min_cluster_obs == 0
            || a.coverage_k.is_nan()
            || a.coverage_k <= 0.0
        {
            return Err(
                "pre-registration: analysis needs signals, min_cluster_obs >= 1, coverage_k > 0"
                    .into(),
            );
        }
        if p.bar.is_empty() {
            return Err("pre-registration: no bars".into());
        }
        for b in &p.bar {
            if !QUANTITIES.contains(&b.quantity.as_str()) {
                return Err(format!("bar {}: unknown quantity '{}'", b.id, b.quantity));
            }
            if b.op != "ge" && b.op != "le" {
                return Err(format!("bar {}: op must be ge or le", b.id));
            }
        }
        Ok(p)
    }

    /// The default template for a model: the analysis and the four default bars.
    pub fn template(
        model_sha256: &str,
        title: &str,
        registrant: &str,
        utc: &str,
        signals: Vec<String>,
    ) -> PreRegistration {
        let bar = |id: &str, q: &str, op: &str, v: f64, h: &str| Bar {
            id: id.into(),
            quantity: q.into(),
            op: op.into(),
            value: v,
            signal: None,
            hypothesis: h.into(),
        };
        PreRegistration {
            format: PREREG_FORMAT.into(),
            title: title.into(),
            registrant: registrant.into(),
            registered_utc: utc.into(),
            data_seen: false,
            model_sha256: model_sha256.into(),
            data_source:
                "LuGRE open dataset, Zenodo doi:10.5281/zenodo.16411686 (schema to be confirmed)"
                    .into(),
            analysis: Analysis {
                signals,
                min_observed_dbhz: None,
                min_cluster_obs: 5,
                coverage_k: 2.0,
                bootstrap_n: 2000,
                bootstrap_seed: 20_261_119,
                published_shortfall_db: PUBLISHED_SHORTFALL_DB,
            },
            bar: vec![
                bar(
                    "B1-sufficiency",
                    "n_clusters",
                    "ge",
                    4.0,
                    "enough independent passes to estimate the bias",
                ),
                bar(
                    "B2-precision",
                    "u_total_db",
                    "le",
                    2.0,
                    "the bias is known to 2 dB (k=1)",
                ),
                bar(
                    "B3-shortfall-detected",
                    "shortfall_lo_db",
                    "ge",
                    0.0,
                    "observed C/N0 is below the independent prediction (k-interval excludes zero)",
                ),
                bar(
                    "B4-consistent-with-published",
                    "overlap_published",
                    "ge",
                    1.0,
                    "the shortfall interval overlaps the published 7-12 dB",
                ),
            ],
        }
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

// ------------------------------------------------------------------------------ scoring

/// One residual.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Residual {
    pub time: String,
    pub sat: String,
    pub signal: String,
    pub pattern: String,
    pub observed_dbhz: f64,
    pub predicted_dbhz: f64,
    /// observed − predicted (dB).
    pub residual_db: f64,
    pub slope_db_per_deg: f64,
}

/// The bias statistic of a set of residuals.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BiasStat {
    pub signal: String,
    pub n_obs: usize,
    pub n_clusters: usize,
    /// Mean of cluster means of observed − predicted (dB).
    pub bias_db: f64,
    pub u_a_db: f64,
    pub u_b_db: f64,
    pub u_total_db: f64,
    pub k: f64,
    /// predicted − observed = −bias (dB), and its k-interval.
    pub shortfall_db: f64,
    pub shortfall_lo_db: f64,
    pub shortfall_hi_db: f64,
    /// Percentile cluster-bootstrap 95 % interval of the shortfall (Type A only).
    pub bootstrap_95_db: [f64; 2],
    pub overlap_published: bool,
    pub inside_published: bool,
    pub budget_b: Budget,
}

/// Counts of what was used and excluded.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Counts {
    pub observations: usize,
    pub excluded_signal: usize,
    pub excluded_floor: usize,
    pub no_prediction: usize,
    pub matched: usize,
    pub clusters_used: usize,
    pub clusters_dropped: usize,
}

/// A bar's outcome.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BarOutcome {
    pub id: String,
    pub quantity: String,
    pub signal: String,
    pub op: String,
    pub value: f64,
    pub observed: f64,
    pub met: bool,
    pub hypothesis: String,
}

/// The score report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScoreReport {
    pub status: String,
    pub title: String,
    pub registered_utc: String,
    pub registrant: String,
    pub prereg_sha256: String,
    pub model_sha256: String,
    pub ephemeris_sha256: String,
    pub observations_sha256: String,
    pub published_source: String,
    pub counts: Counts,
    pub overall: BiasStat,
    pub per_signal: Vec<BiasStat>,
    pub bars: Vec<BarOutcome>,
    pub all_bars_met: bool,
}

/// Common-mode inputs of the Type B budget.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeBInputs {
    /// Per pattern (key/band): its standard uncertainty (dB).
    pub pattern_u_db: BTreeMap<String, f64>,
    pub rx_gain_u_db: f64,
    /// d(10 log10 T_sys)/d T_A × u_T (dB).
    pub antenna_temp_u_db: f64,
    pub nf_u_db: f64,
    pub implementation_u_db: f64,
    pub angle_u_deg: f64,
}

impl TypeBInputs {
    pub fn from_model(m: &Model) -> TypeBInputs {
        let r = &m.config.receiver;
        let ts = m.system_temperature_k();
        let l = 10f64.powf(r.pre_lna_loss_db / 10.0);
        let f = 10f64.powf(r.lna_nf_db / 10.0);
        let db_per_k = 10.0 / std::f64::consts::LN_10 / ts;
        TypeBInputs {
            pattern_u_db: m
                .config
                .transmit
                .iter()
                .map(|p| (format!("{}/{}", p.key, p.band), p.u_db))
                .collect(),
            rx_gain_u_db: r.antenna_gain_u_db,
            antenna_temp_u_db: db_per_k * r.antenna_temp_u_k,
            // dT/dNF(dB) = L·T0·F·ln10/10, so the dB sensitivity is L·T0·F/T_sys.
            nf_u_db: l * T0_K * f / ts * r.lna_nf_u_db,
            implementation_u_db: r.implementation_loss_u_db,
            angle_u_deg: r.ephemeris_angle_u_deg,
        }
    }

    /// The zero budget (a perfect model): only Type A remains.
    pub fn none() -> TypeBInputs {
        TypeBInputs {
            pattern_u_db: BTreeMap::new(),
            rx_gain_u_db: 0.0,
            antenna_temp_u_db: 0.0,
            nf_u_db: 0.0,
            implementation_u_db: 0.0,
            angle_u_deg: 0.0,
        }
    }
}

/// The Type B budget of the mean-of-cluster-means for these clusters. A pattern error is common
/// to the clusters that use it: with `n_p` of `J` clusters on pattern `p`, it contributes
/// `(n_p/J)·u_p`. Receive gain, noise and implementation loss are common to all. The geometric
/// term is `mean(|slope|)·u_θ`, taken as fully correlated (conservative).
fn type_b(inp: &TypeBInputs, clusters: &[(String, Vec<&Residual>)]) -> Budget {
    let j = clusters.len().max(1) as f64;
    let mut terms = Vec::new();
    let mut by_pat: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, rs) in clusters {
        *by_pat.entry(rs[0].pattern.as_str()).or_default() += 1;
    }
    for (p, n) in by_pat {
        let u = inp.pattern_u_db.get(p).copied().unwrap_or(0.0);
        terms.push(Term::normal(
            &format!("transmit EIRP table {p}"),
            0.0,
            u * n as f64 / j,
            TermKind::B,
            "input table uncertainty",
        ));
    }
    let slope: f64 = clusters
        .iter()
        .map(|(_, rs)| rs.iter().map(|r| r.slope_db_per_deg.abs()).sum::<f64>() / rs.len() as f64)
        .sum::<f64>()
        / j;
    for (n, u, s) in [
        ("receive antenna gain", inp.rx_gain_u_db, "input"),
        (
            "antenna noise temperature",
            inp.antenna_temp_u_db,
            "dB sensitivity of 10log10(T_sys)",
        ),
        (
            "LNA noise figure",
            inp.nf_u_db,
            "dB sensitivity of 10log10(T_sys)",
        ),
        ("implementation loss", inp.implementation_u_db, "input"),
        (
            "geometry: transmit angle x EIRP slope",
            slope * inp.angle_u_deg,
            "mean |dEIRP/dtheta| x u_theta",
        ),
    ] {
        terms.push(Term::normal(n, 0.0, u, TermKind::B, s));
    }
    Budget::new(terms)
}

fn bias_stat(
    signal: &str,
    clusters: &[(String, Vec<&Residual>)],
    inp: &TypeBInputs,
    a: &Analysis,
) -> BiasStat {
    let means: Vec<f64> = clusters
        .iter()
        .map(|(_, rs)| rs.iter().map(|r| r.residual_db).sum::<f64>() / rs.len() as f64)
        .collect();
    let j = means.len();
    let bias = if j > 0 {
        means.iter().sum::<f64>() / j as f64
    } else {
        f64::NAN
    };
    let u_a = if j >= 2 {
        (means.iter().map(|m| (m - bias).powi(2)).sum::<f64>() / (j * (j - 1)) as f64).sqrt()
    } else {
        f64::INFINITY
    };
    let budget_b = type_b(inp, clusters);
    let u_b = budget_b.u_c();
    let u = (u_a * u_a + u_b * u_b).sqrt();
    let s = -bias;
    let (lo, hi) = (s - a.coverage_k * u, s + a.coverage_k * u);
    // Percentile cluster bootstrap.
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(a.bootstrap_seed);
    let mut boot: Vec<f64> = (0..a.bootstrap_n)
        .filter(|_| j > 0)
        .map(|_| -(0..j).map(|_| means[rng.gen_range(0..j)]).sum::<f64>() / j as f64)
        .collect();
    boot.sort_by(|x, y| x.total_cmp(y));
    let q = |p: f64| {
        if boot.is_empty() {
            f64::NAN
        } else {
            boot[((p * (boot.len() - 1) as f64).round() as usize).min(boot.len() - 1)]
        }
    };
    let [pl, ph] = a.published_shortfall_db;
    BiasStat {
        signal: signal.into(),
        n_obs: clusters.iter().map(|c| c.1.len()).sum(),
        n_clusters: j,
        bias_db: bias,
        u_a_db: u_a,
        u_b_db: u_b,
        u_total_db: u,
        k: a.coverage_k,
        shortfall_db: s,
        shortfall_lo_db: lo,
        shortfall_hi_db: hi,
        bootstrap_95_db: [q(0.025), q(0.975)],
        overlap_published: lo <= ph && hi >= pl,
        inside_published: lo >= pl && hi <= ph,
        budget_b,
    }
}

/// Score residuals under an analysis (the in-memory core of [`score`]).
pub fn score_residuals(
    residuals: &[Residual],
    a: &Analysis,
    inp: &TypeBInputs,
) -> (BiasStat, Vec<BiasStat>, usize, usize) {
    let mut by: BTreeMap<String, Vec<&Residual>> = BTreeMap::new();
    for r in residuals {
        by.entry(format!("{}:{}", r.sat, r.signal))
            .or_default()
            .push(r);
    }
    let total = by.len();
    let clusters: Vec<(String, Vec<&Residual>)> = by
        .into_iter()
        .filter(|(_, v)| v.len() >= a.min_cluster_obs)
        .collect();
    let dropped = total - clusters.len();
    let overall = bias_stat("all", &clusters, inp, a);
    let mut per = Vec::new();
    for s in &a.signals {
        let (sys, sig) = s.split_once(':').unwrap_or(("", s));
        let sub: Vec<(String, Vec<&Residual>)> = clusters
            .iter()
            .filter(|(_, rs)| rs[0].sat.starts_with(sys) && rs[0].signal == sig)
            .cloned()
            .collect();
        if !sub.is_empty() {
            per.push(bias_stat(s, &sub, inp, a));
        }
    }
    (overall, per, clusters.len(), dropped)
}

fn quantity(s: &BiasStat, q: &str) -> f64 {
    match q {
        "shortfall_db" => s.shortfall_db,
        "shortfall_lo_db" => s.shortfall_lo_db,
        "shortfall_hi_db" => s.shortfall_hi_db,
        "abs_bias_db" => s.bias_db.abs(),
        "u_total_db" => s.u_total_db,
        "n_clusters" => s.n_clusters as f64,
        "n_obs" => s.n_obs as f64,
        "overlap_published" => {
            if s.overlap_published {
                1.0
            } else {
                0.0
            }
        }
        _ => f64::NAN,
    }
}

/// Score the bars on the statistics.
pub fn score_bars(bars: &[Bar], overall: &BiasStat, per: &[BiasStat]) -> Vec<BarOutcome> {
    bars.iter()
        .map(|b| {
            let s = match &b.signal {
                None => Some(overall),
                Some(sig) => per.iter().find(|p| &p.signal == sig),
            };
            let x = s.map(|s| quantity(s, &b.quantity)).unwrap_or(f64::NAN);
            // A missing or non-finite figure fails the bar.
            let met = x.is_finite()
                && if b.op == "ge" {
                    x >= b.value
                } else {
                    x <= b.value
                };
            BarOutcome {
                id: b.id.clone(),
                quantity: b.quantity.clone(),
                signal: b.signal.clone().unwrap_or_else(|| "all".into()),
                op: b.op.clone(),
                value: b.value,
                observed: x,
                met,
                hypothesis: b.hypothesis.clone(),
            }
        })
        .collect()
}

/// Residuals of observations against the model, with the exclusion counts.
pub fn residuals(
    model: &Model,
    src: &dyn GnssSource,
    obs: &[ObservedCn0],
    a: &Analysis,
) -> (Vec<Residual>, Counts) {
    let mut c = Counts {
        observations: obs.len(),
        ..Counts::default()
    };
    let mut out = Vec::new();
    for o in obs {
        let key = format!("{}:{}", &o.sat[..1], o.signal);
        if !a.signals.contains(&key) {
            c.excluded_signal += 1;
            continue;
        }
        if a.min_observed_dbhz.is_some_and(|f| o.cn0_dbhz < f) {
            c.excluded_floor += 1;
            continue;
        }
        match model
            .predict(src, &o.time, &o.sat, &o.signal)
            .and_then(|p| p.cn0_dbhz.map(|v| (p, v)))
        {
            Some((p, v)) => {
                c.matched += 1;
                out.push(Residual {
                    time: p.time,
                    sat: o.sat.clone(),
                    signal: o.signal.clone(),
                    pattern: p.pattern,
                    observed_dbhz: o.cn0_dbhz,
                    predicted_dbhz: v,
                    residual_db: o.cn0_dbhz - v,
                    slope_db_per_deg: p.slope_db_per_deg,
                });
            }
            None => c.no_prediction += 1,
        }
    }
    (out, c)
}

/// The scoring inputs (paths).
pub struct ScoreInputs<'a> {
    pub prereg: &'a Path,
    /// The SHA-256 published at registration.
    pub prereg_sha256: &'a str,
    pub model: &'a Path,
    pub sp3: &'a Path,
    pub observations: &'a Path,
}

/// Score observations against a pre-registered analysis. Refuses unless the pre-registration
/// has the published hash, declares no data seen, and the model file has the registered hash.
pub fn score(inp: &ScoreInputs) -> Result<(ScoreReport, Vec<Residual>), String> {
    let pb = std::fs::read(inp.prereg).map_err(|e| format!("{}: {e}", inp.prereg.display()))?;
    let ps = sha256_hex(&pb);
    if !ps.eq_ignore_ascii_case(inp.prereg_sha256.trim()) {
        return Err(format!(
            "pre-registration SHA-256 {ps} is not the published {}: refusing to score",
            inp.prereg_sha256.trim()
        ));
    }
    let pre = PreRegistration::parse(
        std::str::from_utf8(&pb).map_err(|_| "pre-registration: not UTF-8")?,
    )?;
    let mb = std::fs::read(inp.model).map_err(|e| format!("{}: {e}", inp.model.display()))?;
    let ms = sha256_hex(&mb);
    if !ms.eq_ignore_ascii_case(&pre.model_sha256) {
        return Err(format!(
            "model SHA-256 {ms} is not the pre-registered {}: the model changed after registration",
            pre.model_sha256
        ));
    }
    let base: PathBuf = inp.model.parent().unwrap_or(Path::new(".")).to_path_buf();
    let model = Model::from_bytes(&mb, &base)?;
    let sp3 =
        std::fs::read_to_string(inp.sp3).map_err(|e| format!("{}: {e}", inp.sp3.display()))?;
    let src = Sp3Source::parse(&sp3)?;
    let ob = std::fs::read_to_string(inp.observations)
        .map_err(|e| format!("{}: {e}", inp.observations.display()))?;
    let obs = read_observations(&ob)?;
    let (res, mut counts) = residuals(&model, &src, &obs, &pre.analysis);
    let (overall, per, used, dropped) =
        score_residuals(&res, &pre.analysis, &TypeBInputs::from_model(&model));
    counts.clusters_used = used;
    counts.clusters_dropped = dropped;
    let bars = score_bars(&pre.bar, &overall, &per);
    let all = bars.iter().all(|b| b.met);
    Ok((
        ScoreReport {
            status: STATUS.into(),
            title: pre.title.clone(),
            registered_utc: pre.registered_utc.clone(),
            registrant: pre.registrant.clone(),
            prereg_sha256: ps,
            model_sha256: ms,
            ephemeris_sha256: src.sha256.clone(),
            observations_sha256: sha256_hex(ob.as_bytes()),
            published_source: PUBLISHED_SOURCE.into(),
            counts,
            overall,
            per_signal: per,
            bars,
            all_bars_met: all,
        },
        res,
    ))
}

/// Human-readable report.
pub fn render_text(r: &ScoreReport) -> String {
    let mut s = format!(
        "{}\n{}\npre-registration {} ({} by {})\nmodel {}\nephemeris {}\nobservations {}\n\n",
        r.status,
        r.title,
        r.prereg_sha256,
        r.registered_utc,
        r.registrant,
        r.model_sha256,
        r.ephemeris_sha256,
        r.observations_sha256
    );
    let c = &r.counts;
    s.push_str(&format!(
        "observations {}: matched {}, excluded (signal) {}, excluded (floor) {}, no prediction {}; clusters used {}, dropped {}\n\n",
        c.observations, c.matched, c.excluded_signal, c.excluded_floor, c.no_prediction, c.clusters_used, c.clusters_dropped
    ));
    s.push_str("signal   n_obs clusters  shortfall dB  u_A    u_B    U(k)   interval           bootstrap95        published 7-12\n");
    for b in std::iter::once(&r.overall).chain(r.per_signal.iter()) {
        s.push_str(&format!(
            "{:<8} {:>5} {:>8}  {:>10.2}  {:>5.2}  {:>5.2}  {:>5.2}  [{:>6.2}, {:>6.2}]  [{:>6.2}, {:>6.2}]  {}\n",
            b.signal, b.n_obs, b.n_clusters, b.shortfall_db, b.u_a_db, b.u_b_db, b.k * b.u_total_db,
            b.shortfall_lo_db, b.shortfall_hi_db, b.bootstrap_95_db[0], b.bootstrap_95_db[1],
            if b.inside_published { "inside" } else if b.overlap_published { "overlaps" } else { "outside" }
        ));
    }
    s.push_str("\nbars:\n");
    for b in &r.bars {
        s.push_str(&format!(
            "  {:<32} {} {} {} {}: observed {:.3} -> {}\n",
            b.id,
            b.signal,
            b.quantity,
            b.op,
            b.value,
            b.observed,
            if b.met { "MET" } else { "NOT MET" }
        ));
    }
    s.push_str(&format!(
        "\nsource of the published figure: {}\n",
        r.published_source
    ));
    s
}

/// Write `report.json`, `report.txt` and `residuals.csv`.
pub fn write_dir(out: &Path, r: &ScoreReport, res: &[Residual]) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let j = serde_json::to_string_pretty(r).map_err(|e| e.to_string())? + "\n";
    std::fs::write(out.join("report.json"), j).map_err(|e| e.to_string())?;
    std::fs::write(out.join("report.txt"), render_text(r)).map_err(|e| e.to_string())?;
    let mut csv =
        String::from("time,sat,signal,pattern,observed_dbhz,predicted_dbhz,residual_db\n");
    for x in res {
        csv.push_str(&format!(
            "{},{},{},{},{:.3},{:.3},{:.3}\n",
            x.time, x.sat, x.signal, x.pattern, x.observed_dbhz, x.predicted_dbhz, x.residual_db
        ));
    }
    std::fs::write(out.join("residuals.csv"), csv).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------ synthetic example

/// The synthetic example's truth.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyntheticTruth {
    pub note: String,
    pub injected_bias_db: f64,
    pub cluster_sigma_db: f64,
    pub white_sigma_db: f64,
    pub seed: u64,
    pub prereg_sha256: String,
}

fn eirp_table(peak_dbw: f64) -> String {
    // A SYNTHETIC shape: shaped main lobe to ~21°, a first null near 26°, side lobes ~15-20 dB
    // down, back-off to 90°. Invented for tests; not ACE or Lockheed data.
    let pts = [
        (0.0, -1.5),
        (8.0, -0.5),
        (13.0, 0.0),
        (16.0, -1.0),
        (19.0, -4.0),
        (21.0, -9.0),
        (23.0, -16.0),
        (26.0, -24.0),
        (29.0, -17.0),
        (33.0, -15.5),
        (38.0, -18.0),
        (45.0, -21.0),
        (60.0, -24.0),
        (75.0, -27.0),
        (90.0, -30.0),
    ];
    let mut s = String::from(
        "# SYNTHETIC transmit EIRP pattern (invented shape, not measured)\nangle_deg,eirp_dbw\n",
    );
    for (a, g) in pts {
        s.push_str(&format!("{a},{:.2}\n", peak_dbw + g));
    }
    s
}

fn rx_table() -> String {
    // A SYNTHETIC high-gain antenna, 15.35 dBi peak (LuGRE's stated peak gain), ~10° half-power.
    let mut s = String::from(
        "# SYNTHETIC receive pattern, 15.35 dBi peak (shape invented)\nangle_deg,gain_dbi\n",
    );
    for (a, g) in [
        (0.0, 15.35),
        (5.0, 14.6),
        (10.0, 12.35),
        (15.0, 8.6),
        (20.0, 3.4),
        (30.0, -3.0),
        (60.0, -8.0),
        (90.0, -12.0),
        (180.0, -15.0),
    ] {
        s.push_str(&format!("{a},{g}\n"));
    }
    s
}

/// Write a synthetic data set into `dir`: a GPS-like and a Galileo-like Walker constellation as
/// SP3 (the engine's SGP4 Walker and SP3 writer), synthetic EIRP and receive tables, a model at
/// Mare Crisium (the Blue Ghost M1 region), a pre-registration (with its SHA-256 in
/// `prereg.sha256`), observations `obs.csv` = prediction + `bias_db` + a per-cluster offset
/// N(0, 0.8 dB) + white noise N(0, 0.5 dB), and `truth.json`. Returns the truth.
pub fn write_synthetic(dir: &Path, bias_db: f64, seed: u64) -> Result<SyntheticTruth, String> {
    use kshana::walker::WalkerSgp4;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let w = |p: &str, s: String| std::fs::write(dir.join(p), s).map_err(|e| e.to_string());
    let gps = WalkerSgp4 {
        altitude_km: 20_180.0,
        inclination_deg: 55.0,
        planes: 6,
        sats_per_plane: 4,
        phasing_f: 1.0,
    };
    let gal = WalkerSgp4 {
        altitude_km: 23_222.0,
        inclination_deg: 56.0,
        planes: 3,
        sats_per_plane: 8,
        phasing_f: 1.0,
    };
    let mut ids = Vec::new();
    let mut sats = Vec::new();
    for (k, s) in gps.satellites().into_iter().enumerate() {
        ids.push(format!("G{:02}", k + 1));
        sats.push(s);
    }
    for (k, s) in gal.satellites().into_iter().enumerate() {
        ids.push(format!("E{:02}", k + 1));
        sats.push(s);
    }
    let start = EpochUtc {
        year: 2025,
        month: 3,
        day: 3,
        hour: 0,
        minute: 0,
        second: 0.0,
    };
    let jd0 = kshana::timescales::julian_date(2025, 3, 3, 0, 0, 0.0);
    let f = kshana::sp3::Sp3File::from_propagators(&ids, &sats, start, jd0, 300.0, 289);
    w("gnss.sp3", f.to_sp3_string())?;
    let tables = [
        ("gps_l1_eirp.csv", eirp_table(27.0)),
        ("gps_l5_eirp.csv", eirp_table(29.0)),
        ("gal_e1_eirp.csv", eirp_table(26.0)),
        ("gal_e5a_eirp.csv", eirp_table(28.5)),
        ("rx_hga.csv", rx_table()),
    ];
    let mut sha = BTreeMap::new();
    for (n, t) in &tables {
        sha.insert(*n, sha256_hex(t.as_bytes()));
        w(n, t.clone())?;
    }
    let model = format!(
        "title = \"SYNTHETIC LuGRE-like surface receiver (Mare Crisium)\"\n\
         [receiver]\nboresight = \"earth\"\nantenna_pattern = \"rx_hga.csv\"\nantenna_pattern_sha256 = \"{}\"\n\
         antenna_gain_u_db = 0.5\nantenna_temp_k = 130.0\nantenna_temp_u_k = 30.0\nlna_nf_db = 1.5\nlna_nf_u_db = 0.2\n\
         pre_lna_loss_db = 0.3\nimplementation_loss_db = 1.5\nimplementation_loss_u_db = 0.5\n\
         earth_mask_altitude_m = 100000.0\nephemeris_angle_u_deg = 0.06\n\
         [receiver.site]\nkind = \"surface\"\nlat_deg = 18.56\nlon_deg = 61.81\nheight_m = 0.0\n\
         [[transmit]]\nkey = \"G\"\nband = \"L1\"\nfile = \"gps_l1_eirp.csv\"\nsha256 = \"{}\"\nu_db = 1.5\n\
         [[transmit]]\nkey = \"G\"\nband = \"L5\"\nfile = \"gps_l5_eirp.csv\"\nsha256 = \"{}\"\nu_db = 1.5\n\
         [[transmit]]\nkey = \"E\"\nband = \"E1\"\nfile = \"gal_e1_eirp.csv\"\nsha256 = \"{}\"\nu_db = 1.5\n\
         [[transmit]]\nkey = \"E\"\nband = \"E5a\"\nfile = \"gal_e5a_eirp.csv\"\nsha256 = \"{}\"\nu_db = 1.5\n\
         [signals]\n\"G:1C\" = \"L1\"\n\"G:5Q\" = \"L5\"\n\"E:1C\" = \"E1\"\n\"E:5Q\" = \"E5a\"\n",
        sha["rx_hga.csv"], sha["gps_l1_eirp.csv"], sha["gps_l5_eirp.csv"], sha["gal_e1_eirp.csv"], sha["gal_e5a_eirp.csv"]
    );
    w("model.toml", model.clone())?;
    let pre = PreRegistration::template(
        &sha256_hex(model.as_bytes()),
        "SYNTHETIC LuGRE C/N0 reproduction (harness test)",
        "Kshana Pro test",
        "2026-11-19T00:00:00Z",
        vec!["G:1C".into(), "G:5Q".into(), "E:1C".into(), "E:5Q".into()],
    );
    let pt = pre.to_toml()?;
    let prereg_sha = sha256_hex(pt.as_bytes());
    w("prereg.toml", pt)?;
    w("prereg.sha256", format!("{prereg_sha}\n"))?;
    let m = Model::load(&dir.join("model.toml"))?;
    let src = Sp3Source::parse(&f.to_sp3_string())?;
    let end = EpochUtc { day: 4, ..start };
    let preds = predict_series(&m, &src, &start, &end, 600.0)?;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let mut offs: BTreeMap<String, f64> = BTreeMap::new();
    let (cs, ws) = (0.8, 0.5);
    let mut csv = String::from("# SYNTHETIC observations (prediction + injected bias + noise)\ntime,sv,signal,pseudorange_m,carrier_cycles,doppler_hz,cn0_dbhz\n");
    for p in &preds {
        let Some(c) = p.cn0_dbhz else { continue };
        // Only links the receiver would track (truth above 15 dB-Hz).
        if c + bias_db < 15.0 {
            continue;
        }
        let key = format!("{}:{}", p.sat, p.signal);
        let o = *offs.entry(key).or_insert_with(|| cs * std_normal(&mut rng));
        let v = c + bias_db + o + ws * std_normal(&mut rng);
        csv.push_str(&format!("{},{},{},,,,{:.3}\n", p.time, p.sat, p.signal, v));
    }
    w("obs.csv", csv)?;
    let truth = SyntheticTruth {
        note:
            "SYNTHETIC: observations generated from the model itself; tests the harness, not LuGRE"
                .into(),
        injected_bias_db: bias_db,
        cluster_sigma_db: cs,
        white_sigma_db: ws,
        seed,
        prereg_sha256: prereg_sha,
    };
    w(
        "truth.json",
        serde_json::to_string_pretty(&truth).map_err(|e| e.to_string())? + "\n",
    )?;
    Ok(truth)
}
