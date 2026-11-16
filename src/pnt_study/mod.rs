// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Independent verification studies for new PNT services (roadmap 0.42 E4)
//!
//! A study answers, from the user's side and under bars written down **before** the study
//! runs, how a new PNT service would cover, how well its geometry supports accuracy, and how its
//! authentication behaves:
//!
//! * **Coverage and geometry accuracy** of a LEO service, alone and with GPS and Galileo, from its
//!   published orbital design, through the open engine's `leo-pvt` kind (mode `polar`):
//!   satellites in view, PDOP, HDOP, VDOP and availability per latitude. A modelled 95 %
//!   horizontal accuracy is reported for range errors the study **assumes** (stated, never taken
//!   from a vendor unless the vendor publishes them).
//! * **Integrity of an authentication service** (Galileo OSNMA), at protocol level: seeded
//!   single-bit tampering of authenticated navigation bits, with the page CRC recomputed, run
//!   through the open OSNMA verifier as the user receiver; the share detected, and no failure on
//!   the untampered stream.
//!
//! Each study names its pre-registration file and its SHA-256; the run refuses a file that does
//! not match, so the bars cannot be changed after the fact without it showing. Every bar is
//! reported MET or NOT MET with its worst value; a study passes only when all are met.
//!
//! **Scope.** Ground and user-side studies only (users on or near the Earth's surface). Lunar
//! studies, the referee engine's space part and any study that designs, proves, approves or
//! operates a spacecraft are not part of this crate (roadmap 0.42 section 1a).
//!
//! Everything here is MODELLED: geometry from published designs, integrity on the data given.

pub mod osnma_v2;

use std::path::{Path, PathBuf};

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::osnma::cli::{parse_public_key, verify_report, VerifyParams};
use kshana::osnma::input::parse_pages;
use kshana::osnma::page::InavPage;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The scenario kind.
pub const KIND: &str = "pro-pnt-verification";
/// The output format tag.
pub const FORMAT: &str = "kshana-pro-pnt-verification/1";
/// The method.
pub const METHOD_ID: &str = "kshana-pro/pnt-verification/v1";
/// The file a study writes into its report directory.
pub const RESULT_FILE: &str = "verification-study.json";
/// The open engine the method is pinned to.
pub const OPEN_ENGINE_PIN: &str = "0.41.0";

/// The honesty label every study carries.
pub const LABEL: &str = "MODELLED: coverage and geometry from the service's published design \
through the open engine; integrity at protocol level on the data named; bars written down before \
the run. A study states what the model shows under those bars; it does not approve a service, \
rate it fit for any use, or say anything about equipment in the field.";

/// The ADKD 0 pieces of the Galileo I/NAV words (word type, first bit, length): the open
/// engine's `osnma::navdata` table (ICD Table 14 and Annex B).
pub const ADKD0_PIECES: [(u8, usize, usize); 5] = [
    (1, 6, 120),
    (2, 6, 120),
    (3, 6, 122),
    (4, 6, 120),
    (5, 6, 67),
];

/// A Walker shell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shell {
    /// Walker pattern, `delta` or `star`; unset is the open engine's default (delta).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    pub total: u32,
    pub planes: u32,
    pub phasing: u32,
    pub altitude_km: f64,
    pub inclination_deg: f64,
}

/// A GNSS reference system (an open constellation preset).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GnssSystem {
    pub name: String,
    pub preset: String,
}

/// A LEO system of the service under study.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeoSystem {
    pub name: String,
    pub shell: Vec<Shell>,
}

/// The geometry part of a study.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub duration_s: f64,
    pub step_s: f64,
    pub lat_step_deg: f64,
    pub lon_step_deg: f64,
    pub mask_deg: f64,
    pub pdop_threshold: f64,
    #[serde(default)]
    pub gnss: Vec<GnssSystem>,
    pub leo: Vec<LeoSystem>,
    /// Range errors (UERE, 1 sigma, m) the study assumes for the derived accuracy.
    #[serde(default)]
    pub assumed_uere_m: Vec<f64>,
}

/// The integrity part of a study: OSNMA tampering on a page file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Integrity {
    /// A page file in the open engine's plain page format.
    pub pages: String,
    /// A file holding the public key as `ID:TYPE:HEX`.
    pub public_key: String,
    pub flips: usize,
    pub seed: u64,
    /// Where the pages come from (shown in the report).
    pub provenance: String,
}

/// A bar of the study.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bar {
    pub id: String,
    /// `leo-availability`, `leo-median-hdop`, `fused-availability`, `gnss-availability`,
    /// `osnma-tamper-detection` or `osnma-clean-failures`.
    pub quantity: String,
    #[serde(default)]
    pub lat_min: Option<f64>,
    #[serde(default)]
    pub lat_max: Option<f64>,
    /// `ge` or `le`.
    pub op: String,
    pub value: f64,
}

/// One admissible configuration of a LEO design (method v2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub id: String,
    pub leo: Vec<LeoSystem>,
}

/// A study request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudySpec {
    pub kind: String,
    pub title: String,
    pub service: String,
    pub service_source: String,
    /// The pre-registration file (relative to the spec) and its SHA-256.
    pub pre_registration: String,
    pub pre_registration_sha256: String,
    #[serde(default)]
    pub geometry: Option<Geometry>,
    #[serde(default)]
    pub integrity: Option<Integrity>,
    /// Method v2 (E4b): the admissible configurations of the LEO design; the geometry is run on
    /// each (in place of `geometry.leo`) and a geometry bar is met only if met in all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub configuration: Vec<Configuration>,
    pub bar: Vec<Bar>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

/// A per-latitude row of the geometry results.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatRow {
    pub lat_deg: f64,
    pub gnss: GroupFig,
    pub leo: GroupFig,
    pub fused: GroupFig,
    /// Modelled 95 % horizontal accuracy of the LEO service alone, `2 x median HDOP x UERE`, per
    /// assumed UERE (m).
    pub leo_h95_m_assumed: Vec<Option<f64>>,
}

/// The figures of one group at one latitude (the open engine's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupFig {
    pub mean_in_view: f64,
    pub median_pdop: Option<f64>,
    pub median_hdop: Option<f64>,
    pub median_vdop: Option<f64>,
    pub availability: f64,
}

/// The integrity results.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IntegrityResult {
    pub provenance: String,
    pub pages_sha256: String,
    pub pages: usize,
    pub clean_overall: String,
    pub clean_failed_satellites: usize,
    pub flips: usize,
    pub detected: usize,
    pub detection_share: f64,
    /// Flips the tampered satellite was not reported `failed` after: (svid, GST, word type, bit).
    pub undetected: Vec<(u8, u32, u8, usize)>,
}

/// One bar's outcome.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BarOutcome {
    pub id: String,
    pub quantity: String,
    pub op: String,
    pub value: f64,
    pub lat_range: Option<[f64; 2]>,
    /// The value closest to failing (or the failing one), with its latitude when it has one.
    pub worst: Option<f64>,
    pub worst_lat_deg: Option<f64>,
    pub met: bool,
}

/// A study's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StudyResult {
    pub kind: &'static str,
    pub format: &'static str,
    pub method_id: &'static str,
    pub open_engine: String,
    pub label: &'static str,
    pub title: String,
    pub service: String,
    pub service_source: String,
    pub pre_registration: String,
    pub pre_registration_sha256: String,
    pub geometry_config: Option<Geometry>,
    pub geometry: Vec<LatRow>,
    pub integrity: Option<IntegrityResult>,
    pub bars: Vec<BarOutcome>,
    pub passed: bool,
    pub scope: &'static str,
}

/// What every study says about its scope.
pub const SCOPE: &str = "Ground and user-side study (users on or near the Earth's surface). \
Lunar studies and studies of spacecraft are not part of this product.";

fn sha_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

impl StudySpec {
    pub const KIND: &'static str = KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Independent verification study of a new PNT service (E4), user side: coverage and geometry accuracy of a LEO service from its published design (alone and with GNSS), and OSNMA authentication integrity under seeded tampering; bars from a named pre-registration whose SHA-256 is checked. Ground users only. MODELLED.",
            required_fields: &["title", "service", "service_source", "pre_registration", "pre_registration_sha256", "bar"],
            optional_fields: &["geometry", "integrity"],
        }
    }

    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut s: Self = toml::from_str(&src).map_err(|e| format!("invalid {KIND} study: {e}"))?;
        s.base_dir = path.parent().map(Path::to_path_buf);
        Ok((s, src))
    }

    fn path(&self, p: &str) -> PathBuf {
        match &self.base_dir {
            Some(d) if Path::new(p).is_relative() => d.join(p),
            _ => PathBuf::from(p),
        }
    }
}

/// The open `leo-pvt` polar scenario a geometry block stands for.
pub fn polar_scenario(g: &Geometry) -> String {
    let mut s = format!(
        "kind = \"leo-pvt\"\nmode = \"polar\"\nduration_s = {:?}\nstep_s = {:?}\n\n[polar]\nlat_step_deg = {:?}\nlon_step_deg = {:?}\npdop_threshold = {:?}\n",
        g.duration_s, g.step_s, g.lat_step_deg, g.lon_step_deg, g.pdop_threshold
    );
    for x in &g.gnss {
        s.push_str(&format!(
            "\n[[system]]\nname = {:?}\nrole = \"gnss\"\npreset = {:?}\nmask_deg = {:?}\n",
            x.name, x.preset, g.mask_deg
        ));
    }
    for x in &g.leo {
        s.push_str(&format!(
            "\n[[system]]\nname = {:?}\nrole = \"leo\"\nmask_deg = {:?}\n",
            x.name, g.mask_deg
        ));
        for sh in &x.shell {
            if let Some(p) = &sh.pattern {
                s.push_str(&format!("\n[[system.shell]]\npattern = {p:?}"));
            } else {
                s.push_str("\n[[system.shell]]");
            }
            s.push_str(&format!(
                "\ntotal = {}\nplanes = {}\nphasing = {}\naltitude_km = {:?}\ninclination_deg = {:?}\n",
                sh.total, sh.planes, sh.phasing, sh.altitude_km, sh.inclination_deg
            ));
        }
    }
    s
}

fn fig(v: &serde_json::Value) -> Result<GroupFig, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("leo-pvt row: {e}"))
}

/// Run the geometry part through the open engine.
pub fn run_geometry(g: &Geometry) -> Result<Vec<LatRow>, String> {
    if g.leo.is_empty() {
        return Err("geometry needs at least one LEO system".into());
    }
    let out = kshana::api::run_scenario(&polar_scenario(g)).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&out.json).map_err(|e| e.to_string())?;
    let rows = v["polar"]["rows"]
        .as_array()
        .ok_or("leo-pvt gave no polar rows")?;
    rows.iter()
        .map(|r| {
            let leo = fig(&r["leo"])?;
            Ok(LatRow {
                lat_deg: r["lat_deg"].as_f64().ok_or("row without lat_deg")?,
                gnss: fig(&r["gnss"])?,
                fused: fig(&r["fused"])?,
                leo_h95_m_assumed: g
                    .assumed_uere_m
                    .iter()
                    .map(|u| leo.median_hdop.map(|h| 2.0 * h * u))
                    .collect(),
                leo,
            })
        })
        .collect()
}

fn page_line(p: &InavPage) -> String {
    format!("{} {} {}", p.svid, p.gst, hex::encode(p.bytes()))
}

fn word_type(p: &InavPage) -> u8 {
    (p.data_word()[0] >> 2) & 0x3F
}

/// The page bit that carries bit `wb` of the page's 128-bit word.
pub fn page_bit_of_word_bit(wb: usize) -> usize {
    if wb < 112 {
        2 + wb
    } else {
        120 + 2 + (wb - 112)
    }
}

fn failed_sat(report: &serde_json::Value, svid: u8) -> bool {
    let name = format!("E{svid:02}");
    report["satellites"].as_array().is_some_and(|a| {
        a.iter()
            .any(|s| s["sat"] == name.as_str() && s["status"] == "failed")
    })
}

/// Run the integrity part: the clean stream, then each seeded flip.
pub fn run_integrity(
    i: &Integrity,
    pages_text: &str,
    key_text: &str,
) -> Result<IntegrityResult, String> {
    let pages = parse_pages(pages_text)?;
    let key = parse_public_key(key_text.trim())?;
    let params = |label: &'static str| VerifyParams {
        label,
        format: Some("pages"),
        start_gst: None,
        merkle_root: None,
        keys: vec![key.clone()],
        reference: None,
        max_err: None,
    };
    let text_of = |ps: &[InavPage]| {
        let mut s = String::new();
        for p in ps {
            s.push_str(&page_line(p));
            s.push('\n');
        }
        s
    };
    let (clean, _) = verify_report(text_of(&pages).into_bytes(), &params("(clean)"))?;
    let clean_failed = clean["satellites"]
        .as_array()
        .map(|a| a.iter().filter(|s| s["status"] == "failed").count())
        .unwrap_or(0);
    // Eligible pages: ADKD 0 word types, in a sub-frame followed by at least two more sub-frames
    // of the same satellite.
    let sf = |p: &InavPage| p.gst / 30;
    let mut last_sf: std::collections::BTreeMap<u8, u32> = std::collections::BTreeMap::new();
    for p in &pages {
        let e = last_sf.entry(p.svid).or_insert(0);
        *e = (*e).max(sf(p));
    }
    let eligible: Vec<usize> = (0..pages.len())
        .filter(|&k| {
            let p = &pages[k];
            ADKD0_PIECES.iter().any(|x| x.0 == word_type(p))
                && sf(p) + 2 <= last_sf.get(&p.svid).copied().unwrap_or(0)
        })
        .collect();
    if eligible.is_empty() {
        return Err("no page is eligible for tampering".into());
    }
    let mut rng = ChaCha8Rng::seed_from_u64(i.seed);
    let mut detected = 0usize;
    let mut undetected = Vec::new();
    for _ in 0..i.flips {
        let k = eligible[rng.gen_range(0..eligible.len())];
        let p = &pages[k];
        let wt = word_type(p);
        let (_, first, len) = ADKD0_PIECES
            .iter()
            .copied()
            .find(|x| x.0 == wt)
            .expect("eligible");
        let wb = first + rng.gen_range(0..len);
        let pb = page_bit_of_word_bit(wb);
        let mut bytes = [0u8; 30];
        bytes.copy_from_slice(p.bytes());
        bytes[pb / 8] ^= 1 << (7 - pb % 8);
        let mut t = pages.clone();
        t[k] = InavPage::from_bytes(p.svid, p.gst, &bytes).with_valid_crc();
        let (r, _) = verify_report(text_of(&t).into_bytes(), &params("(tampered)"))?;
        if failed_sat(&r, p.svid) {
            detected += 1;
        } else {
            undetected.push((p.svid, p.gst, wt, wb));
        }
    }
    Ok(IntegrityResult {
        provenance: i.provenance.clone(),
        pages_sha256: sha_hex(pages_text.as_bytes()),
        pages: pages.len(),
        clean_overall: clean["overall"].as_str().unwrap_or("").to_string(),
        clean_failed_satellites: clean_failed,
        flips: i.flips,
        detected,
        detection_share: if i.flips == 0 {
            0.0
        } else {
            detected as f64 / i.flips as f64
        },
        undetected,
    })
}

fn cmp(op: &str, x: f64, v: f64) -> Result<bool, String> {
    match op {
        "ge" => Ok(x >= v),
        "le" => Ok(x <= v),
        o => Err(format!("bar op '{o}' is not ge or le")),
    }
}

/// Score the bars on the results.
pub fn score_bars(
    bars: &[Bar],
    rows: &[LatRow],
    integ: Option<&IntegrityResult>,
) -> Result<Vec<BarOutcome>, String> {
    bars.iter()
        .map(|b| {
            let lat = match (b.lat_min, b.lat_max) {
                (Some(a), Some(z)) => Some([a, z]),
                (None, None) => None,
                _ => return Err(format!("bar {}: give both lat_min and lat_max", b.id)),
            };
            let pick: Option<fn(&LatRow) -> Option<f64>> = match b.quantity.as_str() {
                "leo-availability" => Some(|r| Some(r.leo.availability)),
                "fused-availability" => Some(|r| Some(r.fused.availability)),
                "gnss-availability" => Some(|r| Some(r.gnss.availability)),
                "leo-median-hdop" => Some(|r| r.leo.median_hdop),
                "osnma-tamper-detection" | "osnma-clean-failures" => None,
                q => return Err(format!("bar {}: unknown quantity '{q}'", b.id)),
            };
            let (worst, worst_lat, met) = if let Some(f) = pick {
                let [a, z] = lat.ok_or_else(|| format!("bar {}: needs a latitude range", b.id))?;
                let sel: Vec<&LatRow> = rows
                    .iter()
                    .filter(|r| r.lat_deg >= a - 1e-9 && r.lat_deg <= z + 1e-9)
                    .collect();
                if sel.is_empty() {
                    return Err(format!("bar {}: no latitude in range", b.id));
                }
                let mut worst: Option<(f64, f64)> = None;
                let mut met = true;
                for r in sel {
                    // A missing figure (no fix at all) fails the bar.
                    let x = f(r);
                    let ok = match x {
                        Some(x) => cmp(&b.op, x, b.value)?,
                        None => false,
                    };
                    met &= ok;
                    let key = x.unwrap_or(if b.op == "ge" {
                        f64::NEG_INFINITY
                    } else {
                        f64::INFINITY
                    });
                    let worse = match worst {
                        None => true,
                        Some((w, _)) => {
                            if b.op == "ge" {
                                key < w
                            } else {
                                key > w
                            }
                        }
                    };
                    if worse {
                        worst = Some((key, r.lat_deg));
                    }
                }
                (worst.map(|w| w.0), worst.map(|w| w.1), met)
            } else {
                let i = integ
                    .ok_or_else(|| format!("bar {}: the study has no integrity part", b.id))?;
                let x = if b.quantity == "osnma-tamper-detection" {
                    i.detection_share
                } else {
                    i.clean_failed_satellites as f64
                };
                (Some(x), None, cmp(&b.op, x, b.value)?)
            };
            Ok(BarOutcome {
                id: b.id.clone(),
                quantity: b.quantity.clone(),
                op: b.op.clone(),
                value: b.value,
                lat_range: lat,
                worst: worst.filter(|w| w.is_finite()),
                worst_lat_deg: worst_lat,
                met,
            })
        })
        .collect()
}

/// Run a study.
pub fn run_study(s: &StudySpec) -> Result<StudyResult, String> {
    if s.kind != KIND {
        return Err(format!("kind must be {KIND}"));
    }
    let pr = s.path(&s.pre_registration);
    let prb = std::fs::read(&pr).map_err(|e| format!("{}: {e}", pr.display()))?;
    let got = sha_hex(&prb);
    if got != s.pre_registration_sha256 {
        return Err(format!(
            "the pre-registration {} has SHA-256 {got}, not the {} the study names: the bars were written against another file",
            s.pre_registration, s.pre_registration_sha256
        ));
    }
    if s.bar.is_empty() {
        return Err("a study needs at least one bar".into());
    }
    let rows = match &s.geometry {
        Some(g) => run_geometry(g)?,
        None => Vec::new(),
    };
    let integ = match &s.integrity {
        Some(i) => {
            let pt = std::fs::read_to_string(s.path(&i.pages))
                .map_err(|e| format!("{}: {e}", i.pages))?;
            let kt = std::fs::read_to_string(s.path(&i.public_key))
                .map_err(|e| format!("{}: {e}", i.public_key))?;
            Some(run_integrity(i, &pt, &kt)?)
        }
        None => None,
    };
    let bars = score_bars(&s.bar, &rows, integ.as_ref())?;
    let passed = bars.iter().all(|b| b.met);
    Ok(StudyResult {
        kind: KIND,
        format: FORMAT,
        method_id: METHOD_ID,
        open_engine: format!("kshana {OPEN_ENGINE_PIN}"),
        label: LABEL,
        title: s.title.clone(),
        service: s.service.clone(),
        service_source: s.service_source.clone(),
        pre_registration: s.pre_registration.clone(),
        pre_registration_sha256: s.pre_registration_sha256.clone(),
        geometry_config: s.geometry.clone(),
        geometry: rows,
        integrity: integ,
        bars,
        passed,
        scope: SCOPE,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn opt(x: Option<f64>, d: usize) -> String {
    x.map(|v| format!("{v:.d$}"))
        .unwrap_or_else(|| "n/a".into())
}

/// The study's report page.
pub fn render_html(r: &StudyResult) -> String {
    let mut h = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:2em;max-width:72em}}table{{border-collapse:collapse}}\
         td,th{{border:1px solid #bbb;padding:.25em .5em;text-align:right}}th:first-child,td:first-child{{text-align:left}}.l{{color:#555}}</style></head><body>\
         <h1>{}</h1><p><b>Service:</b> {} ({})</p><p class=\"l\">{}</p><p class=\"l\">{}</p>\
         <p>Pre-registration: {} (SHA-256 {}). Method {}; open engine {}.</p><h2>Bars</h2>\
         <table><tr><th>Bar</th><th>Quantity</th><th>Latitudes</th><th>Bar</th><th>Worst</th><th>At</th><th>Outcome</th></tr>",
        esc(&r.title),
        esc(&r.title),
        esc(&r.service),
        esc(&r.service_source),
        esc(r.label),
        esc(r.scope),
        esc(&r.pre_registration),
        r.pre_registration_sha256,
        r.method_id,
        esc(&r.open_engine),
    );
    for b in &r.bars {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&b.id),
            esc(&b.quantity),
            b.lat_range
                .map(|[a, z]| format!("{a}° to {z}°"))
                .unwrap_or_default(),
            if b.op == "ge" { "≥" } else { "≤" },
            b.value,
            opt(b.worst, 4),
            b.worst_lat_deg.map(|l| format!("{l}°")).unwrap_or_default(),
            if b.met { "MET" } else { "NOT MET" }
        ));
    }
    h.push_str(&format!(
        "</table><p><b>Study {}.</b></p>",
        if r.passed {
            "passes: every bar met"
        } else {
            "does not pass: at least one bar not met"
        }
    ));
    if !r.geometry.is_empty() {
        h.push_str("<h2>Geometry by latitude</h2><table><tr><th>Latitude</th><th>LEO in view</th><th>LEO PDOP</th><th>LEO HDOP</th><th>LEO availability</th><th>GNSS availability</th><th>All availability</th><th>LEO 95 % horizontal (assumed UERE)</th></tr>");
        for row in &r.geometry {
            h.push_str(&format!(
                "<tr><td>{}°</td><td>{:.1}</td><td>{}</td><td>{}</td><td>{:.4}</td><td>{:.4}</td><td>{:.4}</td><td>{}</td></tr>",
                row.lat_deg,
                row.leo.mean_in_view,
                opt(row.leo.median_pdop, 2),
                opt(row.leo.median_hdop, 2),
                row.leo.availability,
                row.gnss.availability,
                row.fused.availability,
                row.leo_h95_m_assumed
                    .iter()
                    .map(|x| opt(*x, 2))
                    .collect::<Vec<_>>()
                    .join(" / ")
            ));
        }
        h.push_str("</table>");
    }
    if let Some(i) = &r.integrity {
        h.push_str(&format!(
            "<h2>Authentication integrity</h2><p>{}</p><p>Untampered stream: overall {}, {} satellites failed. \
             Tampered: {} of {} flips detected ({:.4}).</p>",
            esc(&i.provenance),
            esc(&i.clean_overall),
            i.clean_failed_satellites,
            i.detected,
            i.flips,
            i.detection_share
        ));
    }
    h.push_str("</body></html>\n");
    h
}

/// Write a study's report directory: `verification-study.json` and `report.html`.
pub fn write_dir(out: &Path, r: &StudyResult, spec_src: &str) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let w = |name: &str, b: &[u8]| {
        std::fs::write(out.join(name), b).map_err(|e| format!("{name}: {e}"))
    };
    w(
        RESULT_FILE,
        (serde_json::to_string_pretty(r).expect("plain data") + "\n").as_bytes(),
    )?;
    w("report.html", render_html(r).as_bytes())?;
    w("study.toml", spec_src.as_bytes())
}

/// An empty study template (`kshana-pro verify-service template`).
pub const TEMPLATE: &str = include_str!("template.toml");

impl Scenario for StudySpec {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let r = run_study(self).map_err(KshanaError::InvalidInput)?;
        let met = r.bars.iter().filter(|b| b.met).count();
        Ok(RunOutput {
            summary: format!(
                "Kshana Pro PNT verification study | {} | {met} of {} bars met | {} | MODELLED",
                r.service,
                r.bars.len(),
                if r.passed { "passes" } else { "does not pass" }
            ),
            json: serde_json::to_string(&r).expect("plain data"),
            svg: String::new(),
            csv: None,
        })
    }
}

impl ExternalPack for StudySpec {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(KIND));
    }
}

/// One configuration's geometry and bar outcomes (method v2).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConfigurationResult {
    pub id: String,
    pub leo: Vec<LeoSystem>,
    pub geometry: Vec<LatRow>,
    pub bars: Vec<BarOutcome>,
}

/// Method v2 geometry: the study's pre-registration is checked, then the geometry is run on
/// every configuration with the study's grid. Only the geometry bars are scored here.
pub fn run_configurations(s: &StudySpec) -> Result<Vec<ConfigurationResult>, String> {
    let pr = s.path(&s.pre_registration);
    let prb = std::fs::read(&pr).map_err(|e| format!("{}: {e}", pr.display()))?;
    let got = sha_hex(&prb);
    if got != s.pre_registration_sha256 {
        return Err(format!(
            "the pre-registration {} has SHA-256 {got}, not the {} the study names",
            s.pre_registration, s.pre_registration_sha256
        ));
    }
    let g = s
        .geometry
        .as_ref()
        .ok_or("method v2 needs a geometry block")?;
    let bars: Vec<Bar> = s
        .bar
        .iter()
        .filter(|b| !b.quantity.starts_with("osnma"))
        .cloned()
        .collect();
    s.configuration
        .iter()
        .map(|c| {
            let gc = Geometry {
                leo: c.leo.clone(),
                ..g.clone()
            };
            let rows = run_geometry(&gc)?;
            let bars = score_bars(&bars, &rows, None)?;
            Ok(ConfigurationResult {
                id: c.id.clone(),
                leo: c.leo.clone(),
                geometry: rows,
                bars,
            })
        })
        .collect()
}
