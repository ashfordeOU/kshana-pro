// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! D5 — independent cross-check harnesses. Each harness compares the model with an outside
//! reference under a stated tolerance and produces a report whose
//! [`LedgerRow`] switches the model's label to VALIDATED only when the reference is
//! independent and the comparison passes ([`super::decide_label`]).
//!
//! **No reference values are shipped.** The tests use tables generated from this crate's
//! own model (marked `synthetic: true` / "SYNTHETIC"), which checks the harness logic and
//! nothing else.
//!
//! # (a) GRWAVE
//!
//! GRWAVE is the ITU-R Study Group 3 program implementing the smooth-earth groundwave theory
//! of ITU-R P.368 (residue series, extended flat earth, exponential atmosphere).
//!
//! **Table format** (`kshana-grwave-table v1`, text, UTF-8): `# key: value` header lines,
//! then a CSV header and rows:
//!
//! ```text
//! # format: kshana-grwave-table v1
//! # source: GRWAVE <version>, run <date> by <who>     (or "SYNTHETIC: …")
//! # synthetic: false
//! # freq_khz: 100
//! # sigma_s_m: 5
//! # eps_r: 70
//! # power_kw: 1          (radiated power the field refers to; GRWAVE: 1 kW)
//! # htt_m: 0             (transmitter / receiver heights)
//! # hrr_m: 0
//! # refrac_n: 315        (surface refractivity N_s and scale height of the
//! # hscale_km: 7.35       exponential atmosphere)
//! distance_km,field_dbuv_m,sf_us
//! 10,109.54,0.010
//! ```
//!
//! `sf_us` (optional) is the phase delay beyond propagation at `c/n` (`n = 1.000338`) in µs,
//! i.e. the Secondary Factor of a homogeneous path; whoever converts GRWAVE's phase output
//! into it must state the conversion in `source`.
//!
//! **Model side**: [`smooth_earth_reference`] (this crate's [`SmoothEarth`]: field of a
//! short monopole `E₁√P/d · √(θ/sinθ) · |N(kd)| · |W|`, SF `−arg(N·W)/ω`) with an effective
//! earth-radius factor `k`; GRWAVE models an exponential atmosphere, whose surface gradient
//! corresponds to `k ≈ 4/3`, so compare with `k = 4/3` and expect a growing difference at
//! long range from the atmosphere model alone.
//!
//! **Runner** ([`GrwaveRunner`]): when a GRWAVE executable is configured (explicit path or
//! the `KSHANA_GRWAVE` environment variable) it writes an input deck ([`grwave_deck`]),
//! runs the program (deck on stdin, or as an argument where `{deck}` appears in `args`),
//! keeps the output next to the deck and parses its numeric table
//! ([`parse_grwave_output`], configurable column positions). The deck uses GRWAVE's
//! keyword-per-line input (`FREQ` MHz, `SIGMA` S/m, `EPSLON`, `IPOLRN` 1 = vertical,
//! `HTT`/`HRR` m, `DMIN`/`DMAX`/`DSTEP` km, `REFRAC` N-units, `HSCALE` km, `GO`) as described
//! in the GRWAVE documentation as recalled when this was written: **NEEDS CONFIRMATION
//! against the ITU distribution** (keyword names, units, output column layout) before the
//! first real run; the column positions are configurable for that reason.
//!
//! # (b) Digitised ITU-R P.368 / P.684 curves
//!
//! CSV (`kshana-digitised-curve v1`) with `# key: value` headers (`source`, `figure`,
//! `kind` = groundwave | skywave, `freq_khz`, `power_kw`, `digitisation_uncertainty_db`,
//! `synthetic`) and columns `class,sigma_s_m,eps_r,distance_km,field_dbuv_m` (σ, ε_r may be
//! empty for skywave curves; `class` names the curve: a ground class, "day", "night").
//! Allowed difference per point = model tolerance + digitisation uncertainty.
//!
//! # (c) Published ASF / ECD surveys
//!
//! CSV with a header naming at least `lat`, `lon`, `station`, `asf_ns`, `uncertainty_ns`
//! (optional `chain`, `ecd_us`, `ecd_uncertainty_us`; other columns ignored; `#` comments).
//! Each point is compared with the ground path of a [`PropagationModel`] (ASF, or SF + ASF
//! when the survey reports the correction beyond PF); a point is "within" when
//! `|residual| ≤ √((k·u)² + model_tol²)`. The report has per-station and overall residual
//! statistics, the reduced χ² and a map-ready CSV.

use super::{csv_fields, stats, LedgerRow, Reference, Stats};
use crate::chain::{Chain, Station};
use crate::geo::Geo;
use crate::prop::ground::{E1_UV_M_PER_SQRT_KW_AT_1KM, N_ATM};
use crate::prop::sky::{IonoConfig, Ionospheric};
use crate::prop::smooth_earth::{near_field, Ground, SmoothEarth};
use crate::prop::{PathKind, PropagationModel, C};
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::path::PathBuf;

fn header_map(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix('#'))
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect()
}

fn hnum(h: &BTreeMap<String, String>, k: &str, default: Option<f64>) -> Result<f64, String> {
    match h.get(k) {
        Some(v) => v
            .split_whitespace()
            .next()
            .unwrap_or("")
            .parse()
            .map_err(|e| format!("header {k}: {e}")),
        None => default.ok_or_else(|| format!("missing header \"# {k}: …\"")),
    }
}

fn hbool(h: &BTreeMap<String, String>, k: &str) -> bool {
    h.get(k)
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "yes" | "1"))
        .unwrap_or(false)
}

/// Data rows of a CSV with a header line: (header names lower-case, rows of fields).
type CsvTable = (Vec<String>, Vec<(usize, Vec<String>)>);

fn csv_table(text: &str) -> Result<CsvTable, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let (_, head) = lines.next().ok_or("CSV: no header line")?;
    let head: Vec<String> = csv_fields(head).iter().map(|s| s.to_lowercase()).collect();
    Ok((head, lines.map(|(i, l)| (i + 1, csv_fields(l))).collect()))
}

fn col(head: &[String], name: &str) -> Option<usize> {
    head.iter().position(|h| h == name)
}

fn num_at(f: &[String], i: usize, line: usize, what: &str) -> Result<f64, String> {
    f.get(i)
        .ok_or_else(|| format!("line {line}: missing {what}"))?
        .parse()
        .map_err(|e| format!("line {line}: {what}: {e}"))
}

fn opt_at(f: &[String], i: Option<usize>, line: usize, what: &str) -> Result<Option<f64>, String> {
    match i {
        Some(i) if f.get(i).map(|s| !s.is_empty()).unwrap_or(false) => {
            num_at(f, i, line, what).map(Some)
        }
        _ => Ok(None),
    }
}

/// A point-by-point comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointResidual {
    /// Curve / class / station the point belongs to.
    pub series: String,
    pub distance_km: f64,
    pub reference: f64,
    pub model: f64,
    /// reference − model.
    pub residual: f64,
    pub allowed: f64,
    pub pass: bool,
}

/// Report of a curve comparison (GRWAVE tables, digitised curves).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveReport {
    pub reference: String,
    pub synthetic: bool,
    pub quantity: String,
    pub unit: String,
    pub points: Vec<PointResidual>,
    /// Residual statistics overall and per series.
    pub overall: Stats,
    pub per_series: Vec<(String, Stats, bool)>,
    pub tolerance: String,
    pub pass: bool,
}

impl CurveReport {
    fn build(
        reference: &str,
        synthetic: bool,
        quantity: &str,
        unit: &str,
        points: Vec<PointResidual>,
        tolerance: String,
    ) -> CurveReport {
        let mut series: Vec<String> = points.iter().map(|p| p.series.clone()).collect();
        series.dedup();
        series.sort();
        series.dedup();
        let per_series = series
            .into_iter()
            .map(|s| {
                let v: Vec<&PointResidual> = points.iter().filter(|p| p.series == s).collect();
                let st = stats(&v.iter().map(|p| p.residual).collect::<Vec<_>>());
                let ok = v.iter().all(|p| p.pass);
                (s, st, ok)
            })
            .collect();
        let overall = stats(&points.iter().map(|p| p.residual).collect::<Vec<_>>());
        let pass = !points.is_empty() && points.iter().all(|p| p.pass);
        CurveReport {
            reference: reference.into(),
            synthetic,
            quantity: quantity.into(),
            unit: unit.into(),
            points,
            overall,
            per_series,
            tolerance,
            pass,
        }
    }
    /// Ledger row (a synthetic report never gives VALIDATED).
    pub fn ledger_row(&self, model: &str, reference: &Reference, test: &str) -> LedgerRow {
        let mut r = reference.clone();
        if self.synthetic {
            r.kind = super::ReferenceKind::Synthetic;
        }
        LedgerRow::new(
            model,
            &r,
            test,
            &self.tolerance,
            &format!(
                "{} points, {} residual mean {:.3} rms {:.3} max |{:.3}| {}",
                self.points.len(),
                self.quantity,
                self.overall.mean,
                self.overall.rms,
                self.overall.max_abs,
                self.unit
            ),
            self.pass,
            self.points.len(),
        )
    }
}

// ───────────────────────────── (a) GRWAVE ─────────────────────────────

/// Table format tag.
pub const GRWAVE_TABLE_FORMAT: &str = "kshana-grwave-table v1";

/// Run parameters of a GRWAVE table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrwaveMeta {
    pub source: String,
    pub synthetic: bool,
    pub freq_khz: f64,
    pub sigma_s_m: f64,
    pub eps_r: f64,
    pub power_kw: f64,
    pub htt_m: f64,
    pub hrr_m: f64,
    pub refrac_n: f64,
    pub hscale_km: f64,
}

impl GrwaveMeta {
    /// 100 kHz, 1 kW, ground-level terminals, ITU standard atmosphere (N_s = 315, 7.35 km).
    pub fn loran(sigma_s_m: f64, eps_r: f64, source: &str) -> GrwaveMeta {
        GrwaveMeta {
            source: source.into(),
            synthetic: false,
            freq_khz: 100.0,
            sigma_s_m,
            eps_r,
            power_kw: 1.0,
            htt_m: 0.0,
            hrr_m: 0.0,
            refrac_n: 315.0,
            hscale_km: 7.35,
        }
    }
}

/// One row of a GRWAVE table.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrwaveRow {
    pub distance_km: f64,
    pub field_dbuv_m: f64,
    pub sf_us: Option<f64>,
}

/// A GRWAVE result table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrwaveTable {
    pub meta: GrwaveMeta,
    pub rows: Vec<GrwaveRow>,
}

impl GrwaveTable {
    /// Parse the `kshana-grwave-table v1` text format.
    pub fn parse(text: &str) -> Result<GrwaveTable, String> {
        let h = header_map(text);
        if h.get("format").map(|s| s.as_str()) != Some(GRWAVE_TABLE_FORMAT) {
            return Err(format!("not a \"{GRWAVE_TABLE_FORMAT}\" table"));
        }
        let meta = GrwaveMeta {
            source: h.get("source").cloned().unwrap_or_default(),
            synthetic: hbool(&h, "synthetic"),
            freq_khz: hnum(&h, "freq_khz", None)?,
            sigma_s_m: hnum(&h, "sigma_s_m", None)?,
            eps_r: hnum(&h, "eps_r", None)?,
            power_kw: hnum(&h, "power_kw", Some(1.0))?,
            htt_m: hnum(&h, "htt_m", Some(0.0))?,
            hrr_m: hnum(&h, "hrr_m", Some(0.0))?,
            refrac_n: hnum(&h, "refrac_n", Some(315.0))?,
            hscale_km: hnum(&h, "hscale_km", Some(7.35))?,
        };
        let (head, rows) = csv_table(text)?;
        let (cd, cf) = (
            col(&head, "distance_km").ok_or("missing column distance_km")?,
            col(&head, "field_dbuv_m").ok_or("missing column field_dbuv_m")?,
        );
        let cs = col(&head, "sf_us");
        let rows = rows
            .iter()
            .map(|(ln, f)| {
                Ok(GrwaveRow {
                    distance_km: num_at(f, cd, *ln, "distance_km")?,
                    field_dbuv_m: num_at(f, cf, *ln, "field_dbuv_m")?,
                    sf_us: opt_at(f, cs, *ln, "sf_us")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if rows.is_empty() {
            return Err("GRWAVE table has no rows".into());
        }
        Ok(GrwaveTable { meta, rows })
    }

    /// Write the `kshana-grwave-table v1` text format.
    pub fn to_text(&self) -> String {
        let m = &self.meta;
        let mut s = format!(
            "# format: {GRWAVE_TABLE_FORMAT}\n# source: {}\n# synthetic: {}\n# freq_khz: {}\n# sigma_s_m: {}\n# eps_r: {}\n# power_kw: {}\n# htt_m: {}\n# hrr_m: {}\n# refrac_n: {}\n# hscale_km: {}\n",
            m.source, m.synthetic, m.freq_khz, m.sigma_s_m, m.eps_r, m.power_kw, m.htt_m, m.hrr_m, m.refrac_n, m.hscale_km
        );
        let with_sf = self.rows.iter().any(|r| r.sf_us.is_some());
        s.push_str(if with_sf {
            "distance_km,field_dbuv_m,sf_us\n"
        } else {
            "distance_km,field_dbuv_m\n"
        });
        for r in &self.rows {
            match (with_sf, r.sf_us) {
                (true, Some(v)) => s.push_str(&format!("{},{:.4},{:.6}\n", r.distance_km, r.field_dbuv_m, v)),
                (true, None) => s.push_str(&format!("{},{:.4},\n", r.distance_km, r.field_dbuv_m)),
                _ => s.push_str(&format!("{},{:.4}\n", r.distance_km, r.field_dbuv_m)),
            }
        }
        s
    }
}

/// This crate's smooth-earth groundwave at `distances_km` (ascending or not): field
/// (dB(µV/m)) of a short monopole radiating `power_kw`, and the SF (phase delay beyond `c/n`,
/// ns, induction terms included; phase unwrapped on a ≤ 2 km grid from the transmitter).
pub fn smooth_earth_reference(
    f_hz: f64,
    ground: Ground,
    k_factor: f64,
    power_kw: f64,
    distances_km: &[f64],
) -> Vec<(f64, f64)> {
    let se = SmoothEarth::new(ground, f_hz, k_factor, N_ATM);
    let w = 2.0 * PI * f_hz;
    let kw = N_ATM * w / C;
    let total = |d_m: f64| near_field(kw * d_m) * se.w(d_m);
    let mut order: Vec<usize> = (0..distances_km.len()).collect();
    order.sort_by(|a, b| distances_km[*a].partial_cmp(&distances_km[*b]).unwrap());
    let mut out = vec![(f64::NAN, f64::NAN); distances_km.len()];
    let (mut d_prev, mut ph_prev) = (0.0f64, 0.0f64);
    for i in order {
        let d = distances_km[i] * 1000.0;
        if !(d > 0.0) {
            continue;
        }
        // unwrap along the way (the induction terms make the phase steep near the source,
        // so start the walk at 1 km)
        let start = d_prev.max(1000.0);
        let steps = (((d - start) / 2000.0).ceil() as usize).max(1);
        let mut ph = if d_prev == 0.0 { total(start).arg() } else { ph_prev };
        for s in 1..=steps {
            let x = start + (d - start) * s as f64 / steps as f64;
            let mut a = total(x).arg();
            while a - ph > PI {
                a -= 2.0 * PI;
            }
            while a - ph < -PI {
                a += 2.0 * PI;
            }
            ph = a;
        }
        let z = total(d);
        let field = E1_UV_M_PER_SQRT_KW_AT_1KM * power_kw.sqrt() / (d / 1000.0)
            * se.spreading(d)
            * z.abs();
        out[i] = (20.0 * field.log10(), -ph / w * 1e9);
        d_prev = d;
        ph_prev = ph;
    }
    out
}

/// Tolerances of a groundwave curve comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundwaveTolerance {
    pub field_db: f64,
    pub sf_ns: f64,
    /// Distance range compared (km).
    pub d_min_km: f64,
    pub d_max_km: f64,
}

impl Default for GroundwaveTolerance {
    fn default() -> Self {
        GroundwaveTolerance {
            field_db: 1.0,
            sf_ns: 50.0,
            d_min_km: 0.0,
            d_max_km: f64::INFINITY,
        }
    }
}

/// Compare a GRWAVE table with the smooth-earth model (`k_factor`, see the module docs).
/// Returns the field report and, when the table has `sf_us`, the SF report.
pub fn compare_grwave(
    t: &GrwaveTable,
    k_factor: f64,
    tol: &GroundwaveTolerance,
) -> (CurveReport, Option<CurveReport>) {
    let rows: Vec<&GrwaveRow> = t
        .rows
        .iter()
        .filter(|r| r.distance_km >= tol.d_min_km && r.distance_km <= tol.d_max_km)
        .collect();
    let d: Vec<f64> = rows.iter().map(|r| r.distance_km).collect();
    let g = Ground {
        sigma: t.meta.sigma_s_m,
        eps_r: t.meta.eps_r,
    };
    let m = smooth_earth_reference(t.meta.freq_khz * 1e3, g, k_factor, t.meta.power_kw, &d);
    let series = format!("σ={} S/m ε_r={}", t.meta.sigma_s_m, t.meta.eps_r);
    let pt = |r: &GrwaveRow, refv: f64, model: f64, allowed: f64| {
        let res = refv - model;
        PointResidual {
            series: series.clone(),
            distance_km: r.distance_km,
            reference: refv,
            model,
            residual: res,
            allowed,
            pass: res.is_finite() && res.abs() <= allowed,
        }
    };
    let field = rows
        .iter()
        .zip(&m)
        .map(|(r, (f, _))| pt(r, r.field_dbuv_m, *f, tol.field_db))
        .collect();
    let fr = CurveReport::build(
        &t.meta.source,
        t.meta.synthetic,
        "field strength",
        "dB",
        field,
        format!(
            "±{} dB, {}–{} km, k = {}",
            tol.field_db, tol.d_min_km, tol.d_max_km, k_factor
        ),
    );
    let sf: Vec<PointResidual> = rows
        .iter()
        .zip(&m)
        .filter_map(|(r, (_, s))| r.sf_us.map(|v| pt(r, v * 1e3, *s, tol.sf_ns)))
        .collect();
    let sr = if sf.is_empty() {
        None
    } else {
        Some(CurveReport::build(
            &t.meta.source,
            t.meta.synthetic,
            "secondary factor",
            "ns",
            sf,
            format!("±{} ns, k = {}", tol.sf_ns, k_factor),
        ))
    };
    (fr, sr)
}

/// GRWAVE input deck (keyword per line; see the module docs — NEEDS CONFIRMATION against the
/// ITU distribution).
pub fn grwave_deck(m: &GrwaveMeta, d_min_km: f64, d_max_km: f64, d_step_km: f64) -> String {
    format!(
        "FREQ {}\nSIGMA {}\nEPSLON {}\nIPOLRN 1\nHTT {}\nHRR {}\nDMIN {}\nDMAX {}\nDSTEP {}\nREFRAC {}\nHSCALE {}\nGO\n",
        m.freq_khz / 1000.0,
        m.sigma_s_m,
        m.eps_r,
        m.htt_m,
        m.hrr_m,
        d_min_km,
        d_max_km,
        d_step_km,
        m.refrac_n,
        m.hscale_km
    )
}

/// Column positions (0-based, whitespace/comma separated numeric tokens) of GRWAVE's output
/// table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrwaveColumns {
    pub distance: usize,
    pub field: usize,
    /// Column holding the phase delay beyond `c/n` in µs, if the output (after any agreed
    /// conversion) provides it.
    pub sf_us: Option<usize>,
}

impl Default for GrwaveColumns {
    fn default() -> Self {
        GrwaveColumns {
            distance: 0,
            field: 1,
            sf_us: None,
        }
    }
}

/// Parse the numeric table of a GRWAVE output text: every line whose tokens up to the
/// highest configured column are all numbers is a row; other lines (titles, echoes of the
/// input, headings) are skipped.
pub fn parse_grwave_output(text: &str, cols: &GrwaveColumns, meta: GrwaveMeta) -> Result<GrwaveTable, String> {
    let need = cols.distance.max(cols.field).max(cols.sf_us.unwrap_or(0));
    let mut rows = Vec::new();
    for line in text.lines() {
        let toks: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .collect();
        if toks.len() <= need {
            continue;
        }
        let nums: Option<Vec<f64>> = toks[..=need].iter().map(|t| t.parse().ok()).collect();
        if let Some(n) = nums {
            rows.push(GrwaveRow {
                distance_km: n[cols.distance],
                field_dbuv_m: n[cols.field],
                sf_us: cols.sf_us.map(|i| n[i]),
            });
        }
    }
    if rows.is_empty() {
        return Err("GRWAVE output: no numeric table rows found (check GrwaveColumns)".into());
    }
    Ok(GrwaveTable { meta, rows })
}

/// Runs GRWAVE when an executable is configured.
#[derive(Clone, Debug, PartialEq)]
pub struct GrwaveRunner {
    pub exe: Option<PathBuf>,
    /// Arguments; an argument equal to `{deck}` is replaced by the deck path, otherwise the
    /// deck is written to the program's standard input.
    pub args: Vec<String>,
    pub workdir: PathBuf,
    pub columns: GrwaveColumns,
}

/// Outcome of [`GrwaveRunner::run`].
#[derive(Clone, Debug, PartialEq)]
pub enum RunOutcome {
    /// No executable configured: nothing run (not a failure).
    NotConfigured,
    Ran(GrwaveTable),
}

impl GrwaveRunner {
    /// Executable from the `KSHANA_GRWAVE` environment variable (if set).
    pub fn from_env(workdir: PathBuf) -> GrwaveRunner {
        GrwaveRunner {
            exe: std::env::var_os("KSHANA_GRWAVE").map(PathBuf::from),
            args: Vec::new(),
            workdir,
            columns: GrwaveColumns::default(),
        }
    }

    /// Write the deck, run GRWAVE, keep its output (`grwave_input.txt`,
    /// `grwave_output.txt` in `workdir`) and parse it.
    pub fn run(&self, meta: &GrwaveMeta, d_min_km: f64, d_max_km: f64, d_step_km: f64) -> Result<RunOutcome, String> {
        let Some(exe) = &self.exe else {
            return Ok(RunOutcome::NotConfigured);
        };
        std::fs::create_dir_all(&self.workdir).map_err(|e| format!("{}: {e}", self.workdir.display()))?;
        let deck_path = self.workdir.join("grwave_input.txt");
        let deck = grwave_deck(meta, d_min_km, d_max_km, d_step_km);
        std::fs::write(&deck_path, &deck).map_err(|e| format!("{}: {e}", deck_path.display()))?;
        let mut via_arg = false;
        let args: Vec<String> = self
            .args
            .iter()
            .map(|a| {
                if a == "{deck}" {
                    via_arg = true;
                    deck_path.display().to_string()
                } else {
                    a.clone()
                }
            })
            .collect();
        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args).current_dir(&self.workdir);
        cmd.stdin(if via_arg {
            std::process::Stdio::null()
        } else {
            std::process::Stdio::from(
                std::fs::File::open(&deck_path).map_err(|e| format!("{}: {e}", deck_path.display()))?,
            )
        });
        let out = cmd
            .output()
            .map_err(|e| format!("running {}: {e}", exe.display()))?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let out_path = self.workdir.join("grwave_output.txt");
        std::fs::write(&out_path, &text).map_err(|e| format!("{}: {e}", out_path.display()))?;
        if !out.status.success() {
            return Err(format!(
                "{} exited with {}: {}",
                exe.display(),
                out.status,
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let mut m = meta.clone();
        if m.source.is_empty() {
            m.source = format!("GRWAVE run: {}", exe.display());
        }
        Ok(RunOutcome::Ran(parse_grwave_output(&text, &self.columns, m)?))
    }
}

// ───────────────────────────── (b) digitised curves ─────────────────────────────

/// Format tag of digitised curve files.
pub const DIGITISED_FORMAT: &str = "kshana-digitised-curve v1";

/// Kind of a digitised figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    Groundwave,
    Skywave,
}

/// One digitised point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveRow {
    pub class: String,
    pub sigma_s_m: Option<f64>,
    pub eps_r: Option<f64>,
    pub distance_km: f64,
    pub field_dbuv_m: f64,
}

/// A digitised figure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DigitisedCurves {
    pub source: String,
    pub figure: String,
    pub kind: CurveKind,
    pub freq_khz: f64,
    pub power_kw: f64,
    pub digitisation_uncertainty_db: f64,
    pub synthetic: bool,
    pub rows: Vec<CurveRow>,
}

impl DigitisedCurves {
    pub fn parse(text: &str) -> Result<DigitisedCurves, String> {
        let h = header_map(text);
        if h.get("format").map(|s| s.as_str()) != Some(DIGITISED_FORMAT) {
            return Err(format!("not a \"{DIGITISED_FORMAT}\" file"));
        }
        let kind = match h.get("kind").map(|s| s.to_lowercase()).as_deref() {
            Some("groundwave") => CurveKind::Groundwave,
            Some("skywave") => CurveKind::Skywave,
            k => return Err(format!("kind must be groundwave or skywave, got {k:?}")),
        };
        let (head, rows) = csv_table(text)?;
        let c = |n: &str| col(&head, n).ok_or_else(|| format!("missing column {n}"));
        let (cc, cd, cf) = (c("class")?, c("distance_km")?, c("field_dbuv_m")?);
        let (cs, ce) = (col(&head, "sigma_s_m"), col(&head, "eps_r"));
        let rows = rows
            .iter()
            .map(|(ln, f)| {
                Ok(CurveRow {
                    class: f.get(cc).cloned().unwrap_or_default(),
                    sigma_s_m: opt_at(f, cs, *ln, "sigma_s_m")?,
                    eps_r: opt_at(f, ce, *ln, "eps_r")?,
                    distance_km: num_at(f, cd, *ln, "distance_km")?,
                    field_dbuv_m: num_at(f, cf, *ln, "field_dbuv_m")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if rows.is_empty() {
            return Err("digitised curves: no rows".into());
        }
        Ok(DigitisedCurves {
            source: h.get("source").cloned().unwrap_or_default(),
            figure: h.get("figure").cloned().unwrap_or_default(),
            kind,
            freq_khz: hnum(&h, "freq_khz", None)?,
            power_kw: hnum(&h, "power_kw", Some(1.0))?,
            digitisation_uncertainty_db: hnum(&h, "digitisation_uncertainty_db", Some(0.0))?,
            synthetic: hbool(&h, "synthetic"),
            rows,
        })
    }

    pub fn to_text(&self) -> String {
        let mut s = format!(
            "# format: {DIGITISED_FORMAT}\n# source: {}\n# figure: {}\n# kind: {}\n# freq_khz: {}\n# power_kw: {}\n# digitisation_uncertainty_db: {}\n# synthetic: {}\nclass,sigma_s_m,eps_r,distance_km,field_dbuv_m\n",
            self.source,
            self.figure,
            match self.kind {
                CurveKind::Groundwave => "groundwave",
                CurveKind::Skywave => "skywave",
            },
            self.freq_khz,
            self.power_kw,
            self.digitisation_uncertainty_db,
            self.synthetic
        );
        let o = |v: Option<f64>| v.map(|x| x.to_string()).unwrap_or_default();
        for r in &self.rows {
            s.push_str(&format!(
                "{},{},{},{},{:.4}\n",
                r.class,
                o(r.sigma_s_m),
                o(r.eps_r),
                r.distance_km,
                r.field_dbuv_m
            ));
        }
        s
    }
}

/// Compare digitised curves with `model` (field in dB(µV/m) for a row); allowed difference
/// per point = `tol_db` + the file's digitisation uncertainty.
pub fn compare_digitised(
    curves: &DigitisedCurves,
    model: &dyn Fn(&CurveRow) -> Result<f64, String>,
    tol_db: f64,
) -> Result<CurveReport, String> {
    let allowed = tol_db + curves.digitisation_uncertainty_db;
    let pts = curves
        .rows
        .iter()
        .map(|r| {
            let m = model(r)?;
            let res = r.field_dbuv_m - m;
            Ok(PointResidual {
                series: r.class.clone(),
                distance_km: r.distance_km,
                reference: r.field_dbuv_m,
                model: m,
                residual: res,
                allowed,
                pass: res.is_finite() && res.abs() <= allowed,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CurveReport::build(
        &format!("{} {}", curves.source, curves.figure),
        curves.synthetic,
        "field strength",
        "dB",
        pts,
        format!(
            "±({tol_db} dB model + {} dB digitisation)",
            curves.digitisation_uncertainty_db
        ),
    ))
}

/// Groundwave model for digitised P.368-style curves: smooth earth, the row's σ and ε_r
/// (ε_r from σ when empty), `k_factor`, the file's frequency and power.
pub fn groundwave_curve_model(k_factor: f64, freq_khz: f64, power_kw: f64) -> impl Fn(&CurveRow) -> Result<f64, String> {
    move |r: &CurveRow| {
        let s = r.sigma_s_m.ok_or_else(|| format!("class {}: no sigma_s_m", r.class))?;
        let g = Ground {
            sigma: s,
            eps_r: r
                .eps_r
                .unwrap_or_else(|| crate::prop::conductivity::eps_for_sigma(s)),
        };
        Ok(smooth_earth_reference(freq_khz * 1e3, g, k_factor, power_kw, &[r.distance_km])[0].0)
    }
}

/// Skywave model for digitised P.684-style curves: the crate's wave-hop field
/// ([`Ionospheric::p684_field`], one hop) with day weight 1 for class "day", 0 for "night"
/// (other classes: `default_day_weight`), reflection height interpolated between the
/// configuration's night and day heights.
pub fn skywave_curve_model(cfg: IonoConfig, power_kw: f64, default_day_weight: f64) -> Result<impl Fn(&CurveRow) -> Result<f64, String>, String> {
    let ion = Ionospheric::new(IonoConfig {
        daylight: crate::prop::sky::Daylight::Night,
        ..cfg
    })?;
    Ok(move |r: &CurveRow| {
        let w = match r.class.to_lowercase().as_str() {
            "day" => 1.0,
            "night" => 0.0,
            _ => default_day_weight,
        };
        let h = (ion.cfg.h_night_km + w * (ion.cfg.h_day_km - ion.cfg.h_night_km)) * 1000.0;
        let e = ion.p684_field(power_kw, r.distance_km * 1000.0, h, 1, w);
        Ok(20.0 * e.log10())
    })
}

// ───────────────────────────── (c) ASF / ECD survey ─────────────────────────────

/// One published survey point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurveyPoint {
    pub line: usize,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub chain: Option<String>,
    pub station: String,
    pub asf_ns: f64,
    pub uncertainty_ns: f64,
    pub ecd_us: Option<f64>,
    pub ecd_uncertainty_us: Option<f64>,
}

/// Parse a survey CSV (see the module docs).
pub fn parse_survey_csv(text: &str) -> Result<Vec<SurveyPoint>, String> {
    let (head, rows) = csv_table(text)?;
    let c = |n: &str| col(&head, n).ok_or_else(|| format!("survey CSV: missing column {n}"));
    let (cla, clo, cst, ca, cu) = (c("lat")?, c("lon")?, c("station")?, c("asf_ns")?, c("uncertainty_ns")?);
    let (cch, ce, ceu) = (col(&head, "chain"), col(&head, "ecd_us"), col(&head, "ecd_uncertainty_us"));
    rows.iter()
        .map(|(ln, f)| {
            let u = num_at(f, cu, *ln, "uncertainty_ns")?;
            if !(u > 0.0) {
                return Err(format!("line {ln}: uncertainty_ns must be positive"));
            }
            Ok(SurveyPoint {
                line: *ln,
                lat_deg: num_at(f, cla, *ln, "lat")?,
                lon_deg: num_at(f, clo, *ln, "lon")?,
                chain: cch.and_then(|i| f.get(i)).filter(|s| !s.is_empty()).cloned(),
                station: f.get(cst).cloned().unwrap_or_default(),
                asf_ns: num_at(f, ca, *ln, "asf_ns")?,
                uncertainty_ns: u,
                ecd_us: opt_at(f, ce, *ln, "ecd_us")?,
                ecd_uncertainty_us: opt_at(f, ceu, *ln, "ecd_uncertainty_us")?,
            })
        })
        .collect()
}

/// What the survey's ASF column holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurveyQuantity {
    /// ASF proper (beyond PF + seawater SF), the RTCM convention.
    Asf,
    /// The correction beyond PF (SF + ASF).
    SfPlusAsf,
}

/// Tolerances of a survey comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurveyTolerance {
    /// Model tolerance combined in quadrature with k × the point uncertainty (ns).
    pub model_ns: f64,
    pub k: f64,
    /// Minimum fraction of points within their allowance.
    pub min_fraction: f64,
    /// Maximum overall RMS residual (ns).
    pub rms_ns_max: f64,
    /// ECD model tolerance (µs) combined with k × the ECD uncertainty.
    pub ecd_model_us: f64,
}

impl Default for SurveyTolerance {
    fn default() -> Self {
        SurveyTolerance {
            model_ns: 50.0,
            k: 2.0,
            min_fraction: 0.95,
            rms_ns_max: 100.0,
            ecd_model_us: 0.5,
        }
    }
}

/// One compared survey point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurveyResidual {
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// "chain/station".
    pub station: String,
    pub measured_ns: f64,
    pub model_ns: f64,
    /// measured − model.
    pub residual_ns: f64,
    pub uncertainty_ns: f64,
    pub normalized: f64,
    pub within: bool,
    pub ecd_measured_us: Option<f64>,
    pub ecd_model_us: f64,
    pub ecd_residual_us: Option<f64>,
}

/// Survey comparison report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurveyReport {
    pub reference: String,
    pub model: String,
    pub quantity: SurveyQuantity,
    pub points: Vec<SurveyResidual>,
    pub per_station: Vec<(String, Stats)>,
    pub overall: Stats,
    pub ecd: Option<Stats>,
    pub reduced_chi2: f64,
    pub fraction_within: f64,
    /// Points whose station matched no chain (line: text).
    pub unmatched: Vec<String>,
    pub tolerance: SurveyTolerance,
    pub pass: bool,
}

fn find_station<'a>(chains: &'a [Chain], p: &SurveyPoint) -> Result<(&'a Chain, &'a Station), String> {
    let (cname, sname) = match (&p.chain, p.station.split_once('/')) {
        (Some(c), _) => (Some(c.as_str()), p.station.as_str()),
        (None, Some((c, s))) => (Some(c), s),
        (None, None) => (None, p.station.as_str()),
    };
    let mut hits = Vec::new();
    for c in chains {
        if cname.map(|n| n == c.name || c.name.starts_with(n)).unwrap_or(true) {
            for s in &c.stations {
                if s.id == sname || (!s.name.is_empty() && s.name == sname) {
                    hits.push((c, s));
                }
            }
        }
    }
    match hits.len() {
        1 => Ok(hits[0]),
        0 => Err(format!("line {}: station {:?} not found", p.line, p.station)),
        _ => Err(format!(
            "line {}: station {:?} is ambiguous (add a chain column)",
            p.line, p.station
        )),
    }
}

/// Compare survey points with the ground path of `model` (see the module docs).
pub fn compare_survey(
    points: &[SurveyPoint],
    chains: &[Chain],
    model: &dyn PropagationModel,
    quantity: SurveyQuantity,
    tol: &SurveyTolerance,
    reference: &str,
) -> SurveyReport {
    let mut out = Vec::new();
    let mut unmatched = Vec::new();
    for p in points {
        let (c, st) = match find_station(chains, p) {
            Ok(x) => x,
            Err(e) => {
                unmatched.push(e);
                continue;
            }
        };
        let rx = RxState {
            t_s: 0.0,
            pos: Geo::new(p.lat_deg, p.lon_deg),
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        let Some(g) = model.paths(st, &rx).into_iter().find(|q| q.kind == PathKind::Ground) else {
            unmatched.push(format!("line {}: model gave no ground path", p.line));
            continue;
        };
        let m = match quantity {
            SurveyQuantity::Asf => g.asf_ns,
            SurveyQuantity::SfPlusAsf => g.sf_ns + g.asf_ns,
        };
        let res = p.asf_ns - m;
        let allowed = ((tol.k * p.uncertainty_ns).powi(2) + tol.model_ns.powi(2)).sqrt();
        let ecd_model = g.ecd_s * 1e6 + st.ecd_us;
        out.push(SurveyResidual {
            lat_deg: p.lat_deg,
            lon_deg: p.lon_deg,
            station: format!("{}/{}", c.name, st.id),
            measured_ns: p.asf_ns,
            model_ns: m,
            residual_ns: res,
            uncertainty_ns: p.uncertainty_ns,
            normalized: res / p.uncertainty_ns,
            within: res.abs() <= allowed,
            ecd_measured_us: p.ecd_us,
            ecd_model_us: ecd_model,
            ecd_residual_us: p.ecd_us.map(|e| e - ecd_model),
        });
    }
    let mut keys: Vec<String> = out.iter().map(|r| r.station.clone()).collect();
    keys.sort();
    keys.dedup();
    let per_station = keys
        .into_iter()
        .map(|k| {
            let v: Vec<f64> = out.iter().filter(|r| r.station == k).map(|r| r.residual_ns).collect();
            (k, stats(&v))
        })
        .collect();
    let overall = stats(&out.iter().map(|r| r.residual_ns).collect::<Vec<_>>());
    let ecd_res: Vec<f64> = out.iter().filter_map(|r| r.ecd_residual_us).collect();
    let n = out.len();
    let chi2: f64 = out.iter().map(|r| r.normalized * r.normalized).sum();
    let fraction = if n > 0 {
        out.iter().filter(|r| r.within).count() as f64 / n as f64
    } else {
        0.0
    };
    let ecd_ok = points.iter().zip(&out).all(|(p, r)| match (r.ecd_residual_us, p.ecd_uncertainty_us) {
        (Some(e), u) => {
            e.abs() <= ((tol.k * u.unwrap_or(0.0)).powi(2) + tol.ecd_model_us.powi(2)).sqrt()
        }
        _ => true,
    });
    SurveyReport {
        reference: reference.into(),
        model: model.name(),
        quantity,
        pass: n > 0 && fraction >= tol.min_fraction && overall.rms <= tol.rms_ns_max && ecd_ok,
        points: out,
        per_station,
        overall,
        ecd: if ecd_res.is_empty() { None } else { Some(stats(&ecd_res)) },
        reduced_chi2: if n > 0 { chi2 / n as f64 } else { f64::NAN },
        fraction_within: fraction,
        unmatched,
        tolerance: tol.clone(),
    }
}

impl SurveyReport {
    /// Map-ready CSV: one line per compared point.
    pub fn map_csv(&self) -> String {
        let mut s = String::from(
            "lat_deg,lon_deg,station,measured_ns,model_ns,residual_ns,uncertainty_ns,normalized,within,ecd_measured_us,ecd_model_us,ecd_residual_us\n",
        );
        let o = |v: Option<f64>| v.map(|x| format!("{x:.4}")).unwrap_or_default();
        for r in &self.points {
            s.push_str(&format!(
                "{:.6},{:.6},{},{:.2},{:.2},{:.2},{:.2},{:.3},{},{},{:.4},{}\n",
                r.lat_deg,
                r.lon_deg,
                r.station,
                r.measured_ns,
                r.model_ns,
                r.residual_ns,
                r.uncertainty_ns,
                r.normalized,
                r.within,
                o(r.ecd_measured_us),
                r.ecd_model_us,
                o(r.ecd_residual_us)
            ));
        }
        s
    }

    pub fn ledger_row(&self, model: &str, reference: &Reference) -> LedgerRow {
        let t = &self.tolerance;
        LedgerRow::new(
            model,
            reference,
            &format!("valid::crosscheck::compare_survey ({} points)", self.points.len()),
            &format!(
                "≥ {:.0} % of points within √(({}·u)² + {} ns²); RMS ≤ {} ns",
                t.min_fraction * 100.0,
                t.k,
                t.model_ns,
                t.rms_ns_max
            ),
            &format!(
                "mean {:.1} ns, RMS {:.1} ns, {:.0} % within, reduced χ² {:.2}",
                self.overall.mean,
                self.overall.rms,
                self.fraction_within * 100.0,
                self.reduced_chi2
            ),
            self.pass,
            self.points.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::ground::{GroundConfig, K_LORAN};
    use crate::prop::BuildContext;
    use crate::time::AbsTime;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    const K_ITU: f64 = 4.0 / 3.0;

    /// SYNTHETIC table generated from this crate's model (not GRWAVE output).
    fn synthetic_table(sigma: f64, eps: f64) -> GrwaveTable {
        let d: Vec<f64> = (1..=20).map(|i| i as f64 * 100.0).collect();
        let m = smooth_earth_reference(100e3, Ground { sigma, eps_r: eps }, K_ITU, 1.0, &d);
        let mut meta = GrwaveMeta::loran(sigma, eps, "SYNTHETIC: generated by kshana-eloran smooth_earth (k = 4/3), not GRWAVE");
        meta.synthetic = true;
        GrwaveTable {
            meta,
            rows: d
                .iter()
                .zip(&m)
                .map(|(d, (f, s))| GrwaveRow { distance_km: *d, field_dbuv_m: *f, sf_us: Some(s / 1e3) })
                .collect(),
        }
    }

    #[test]
    fn smooth_earth_reference_short_range_and_sf() {
        // 1 kW over sea: ≈ 300 mV/m · (1 km / d) at short range → 109.5 dB(µV/m) at 1 km
        let m = smooth_earth_reference(100e3, Ground { sigma: 5.0, eps_r: 80.0 }, K_LORAN, 1.0, &[10.0, 1000.0]);
        assert!((m[0].0 - (20.0 * 30_000f64.log10())).abs() < 0.5, "{m:?}");
        // the seawater SF at 1000 km is a few µs at most and positive
        assert!(m[1].1 > 500.0 && m[1].1 < 5000.0, "{m:?}");
    }

    #[test]
    fn grwave_table_round_trip_and_comparison() {
        let t = synthetic_table(0.003, 15.0);
        let back = GrwaveTable::parse(&t.to_text()).unwrap();
        assert_eq!(back.rows.len(), 20);
        assert!(back.meta.synthetic);
        let (f, s) = compare_grwave(&back, K_ITU, &GroundwaveTolerance::default());
        assert!(f.pass && f.overall.max_abs < 0.01, "{:?}", f.overall);
        assert!(s.unwrap().pass);
        // synthetic data never validate
        let row = f.ledger_row(
            "Groundwave field, smooth earth",
            &Reference::new("GRWAVE", super::super::ReferenceKind::IndependentProgram),
            "valid::crosscheck::compare_grwave",
        );
        assert_eq!(row.label, super::super::Label::Modelled);
        assert!(row.to_markdown().contains("synthetic"));
        // a 2 dB discrepancy beyond 1000 km is caught on exactly those points
        let mut bad = back.clone();
        for r in bad.rows.iter_mut().filter(|r| r.distance_km > 1000.0) {
            r.field_dbuv_m += 2.0;
        }
        let (f, _) = compare_grwave(&bad, K_ITU, &GroundwaveTolerance::default());
        assert!(!f.pass);
        assert_eq!(f.points.iter().filter(|p| !p.pass).count(), 10);
        // the ITU convention differs from the LORAN calibration (k = 1.24) at long range
        let (f, _) = compare_grwave(&back, K_LORAN, &GroundwaveTolerance::default());
        assert!(f.overall.max_abs > 0.05);
    }

    #[test]
    fn grwave_deck_parser_and_runner() {
        let meta = GrwaveMeta::loran(5.0, 70.0, "");
        let deck = grwave_deck(&meta, 10.0, 2000.0, 10.0);
        for k in ["FREQ 0.1\n", "SIGMA 5\n", "EPSLON 70\n", "IPOLRN 1\n", "DMAX 2000\n", "GO\n"] {
            assert!(deck.contains(k), "{deck}");
        }
        // SYNTHETIC output text in a GRWAVE-like layout (titles, headings, numeric rows)
        let text = "GRWAVE-LIKE SYNTHETIC OUTPUT\n FREQ = 0.1 MHz\n  DIST(KM)   FIELD(DBUV/M)  LOSS(DB)\n   10.0   89.50   60.1\n   20.0   83.40  66.2\n";
        let t = parse_grwave_output(text, &GrwaveColumns::default(), meta.clone()).unwrap();
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].field_dbuv_m, 83.4);
        assert!(parse_grwave_output("no table", &GrwaveColumns::default(), meta.clone()).is_err());
        let dir = std::env::temp_dir().join(format!("kshana-grwave-test-{}", std::process::id()));
        let none = GrwaveRunner { exe: None, args: vec![], workdir: dir.clone(), columns: Default::default() };
        assert_eq!(none.run(&meta, 10.0, 20.0, 10.0).unwrap(), RunOutcome::NotConfigured);
        #[cfg(unix)]
        {
            // a stand-in "executable" that checks the deck on stdin and prints a table
            let fake = GrwaveRunner {
                exe: Some("/bin/sh".into()),
                args: vec!["-c".into(), "grep -q '^GO' && printf 'HDR\\n10 89.5\\n20 83.4\\n'".into()],
                workdir: dir.clone(),
                columns: Default::default(),
            };
            match fake.run(&meta, 10.0, 20.0, 10.0).unwrap() {
                RunOutcome::Ran(t) => assert_eq!(t.rows.len(), 2),
                o => panic!("{o:?}"),
            }
            assert!(dir.join("grwave_input.txt").exists() && dir.join("grwave_output.txt").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn digitised_curve_harness() {
        // SYNTHETIC curves from this crate's model for three ground classes
        let model = groundwave_curve_model(K_ITU, 100.0, 1.0);
        let mut rows = Vec::new();
        for (class, s, e) in [("sea", 5.0, 70.0), ("wet", 0.01, 30.0), ("dry", 0.001, 15.0)] {
            for d in [50.0, 200.0, 500.0, 1000.0] {
                let mut r = CurveRow { class: class.into(), sigma_s_m: Some(s), eps_r: Some(e), distance_km: d, field_dbuv_m: 0.0 };
                r.field_dbuv_m = model(&r).unwrap();
                rows.push(r);
            }
        }
        let curves = DigitisedCurves {
            source: "SYNTHETIC: generated by kshana-eloran (not ITU-R P.368 values)".into(),
            figure: "none".into(),
            kind: CurveKind::Groundwave,
            freq_khz: 100.0,
            power_kw: 1.0,
            digitisation_uncertainty_db: 0.5,
            synthetic: true,
            rows,
        };
        let back = DigitisedCurves::parse(&curves.to_text()).unwrap();
        assert_eq!(back.rows.len(), 12);
        let rep = compare_digitised(&back, &model, 1.0).unwrap();
        assert!(rep.pass && rep.per_series.len() == 3);
        // a 3 dB error in one class fails that class only
        let mut bad = back.clone();
        for r in bad.rows.iter_mut().filter(|r| r.class == "dry") {
            r.field_dbuv_m -= 3.0;
        }
        let rep = compare_digitised(&bad, &model, 1.0).unwrap();
        assert!(!rep.pass);
        for (c, _, ok) in &rep.per_series {
            assert_eq!(*ok, c != "dry", "{c}");
        }
        // skywave model harness on SYNTHETIC night/day points
        let sky = skywave_curve_model(IonoConfig::default(), 1.0, 0.5).unwrap();
        let r = CurveRow { class: "night".into(), sigma_s_m: None, eps_r: None, distance_km: 1000.0, field_dbuv_m: 0.0 };
        let night = sky(&r).unwrap();
        let day = sky(&CurveRow { class: "day".into(), ..r.clone() }).unwrap();
        assert!(night > day, "night {night} day {day}");
        assert!(DigitisedCurves::parse("# format: other\n").is_err());
    }

    #[test]
    fn survey_import_comparison_and_map() {
        let chains: Vec<Chain> = vec![toml::from_str(
            "name = \"sv\"\ngri_us = 79800\n\
             [[stations]]\nid = \"M\"\nrole = \"master\"\nposition = { lat_deg = 56.5, lon_deg = 1.0 }\n\
             [[stations]]\nid = \"X\"\nrole = \"secondary\"\nposition = { lat_deg = 53.5, lon_deg = 1.5 }\nemission_delay_us = 13000.0\n",
        )
        .unwrap()];
        let ctx = BuildContext::new(AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap(), 1.0, 0);
        let se: GroundConfig = toml::from_str("model = \"smooth_earth\"\nsigma_s_m = 0.01").unwrap();
        let model = crate::prop::ground::build(&se, &ctx).unwrap();
        // SYNTHETIC survey: the model's own ASF at 12 points plus 10 ns noise
        let mut rng = ChaCha8Rng::seed_from_u64(11);
        let c0 = &chains[0];
        let mut text = String::from("# SYNTHETIC survey generated by kshana-eloran tests\nlat,lon,chain,station,asf_ns,uncertainty_ns,source\n");
        for i in 0..12 {
            let st = &c0.stations[i % c0.stations.len()];
            let pos = Geo::new(st.position.lat_deg + 1.0 + 0.2 * i as f64, st.position.lon_deg + 0.5);
            let rx = RxState { t_s: 0.0, pos, heading_rad: 0.0, speed_mps: 0.0 };
            let a = model.paths(st, &rx)[0].asf_ns + 10.0 * crate::timebase::gauss(&mut rng);
            text.push_str(&format!("{},{},{},{},{a:.2},10,synthetic\n", pos.lat_deg, pos.lon_deg, c0.name, st.id));
        }
        let pts = parse_survey_csv(&text).unwrap();
        assert_eq!(pts.len(), 12);
        let rep = compare_survey(&pts, &chains, model.as_ref(), SurveyQuantity::Asf, &Default::default(), "SYNTHETIC");
        assert!(rep.unmatched.is_empty(), "{:?}", rep.unmatched);
        assert!(rep.pass, "{:?}", rep.overall);
        assert!(rep.reduced_chi2 > 0.3 && rep.reduced_chi2 < 2.5, "{}", rep.reduced_chi2);
        assert_eq!(rep.map_csv().lines().count(), 13);
        assert_eq!(
            rep.ledger_row("Land ASF", &Reference::synthetic("survey")).label,
            super::super::Label::Modelled
        );
        // a PF-only model (ASF = 0) is rejected
        let simple = crate::prop::ground::build(&GroundConfig::default(), &ctx).unwrap();
        let bad = compare_survey(&pts, &chains, simple.as_ref(), SurveyQuantity::Asf, &Default::default(), "SYNTHETIC");
        assert!(!bad.pass && bad.overall.mean.abs() > 100.0, "{:?}", bad.overall);
        // unknown station is reported, not fatal
        let mut p2 = pts.clone();
        p2[0].station = "Q".into();
        assert_eq!(compare_survey(&p2, &chains, model.as_ref(), SurveyQuantity::Asf, &Default::default(), "x").unmatched.len(), 1);
        assert!(parse_survey_csv("lat,lon,station\n1,2,M\n").is_err());
    }
}
