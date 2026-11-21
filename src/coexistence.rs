// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Co-existence and compatibility interference catalogue for civil links
//!
//! A civil radio link has to keep working beside its neighbours: other users of the same
//! channel, transmitters in adjacent bands, and the unwanted emissions every transmitter makes.
//! This catalogue writes down the published protection criteria and test parameters for that
//! co-existence, each value with the document, clause or table it was read from, and turns
//! them into a **scenario pack** for a stated victim receiver: the interference level, the
//! frequency relation and the pass criterion of each case, as data for a test plan.
//!
//! Categories: **co-channel** (interference-to-noise criteria), **adjacent-band blocking**
//! (receiver blocking test points of harmonised standards), **out-of-band and spurious
//! emissions** of neighbouring transmitters (the spurious-domain limits and the out-of-band /
//! spurious boundary), and **intermodulation** (no value was read for this edition: rows say
//! "to confirm").
//!
//! It is a co-existence and compatibility catalogue: it models unintentional interference from
//! lawful neighbouring services only. It contains no hostile-interference model and synthesises
//! no waveform. Each row cites what it was read from; a row without a read source says
//! "to confirm".

use serde::{Deserialize, Serialize};

/// Pro feature of the scenario pack.
pub const FEATURE: &str = "pro-coexistence-catalogue";
/// Format tags.
pub const FORMAT: &str = "kshana-pro-coexistence-scenarios/1";
/// The marker of a row without a read source.
pub const TO_CONFIRM: &str = "to confirm";
/// Scope.
pub const SCOPE: &str = "Co-existence and compatibility with lawful neighbouring services: \
unintentional interference only, no hostile-interference model, no waveform synthesis. Each \
value is cited to the document it was read from or marked to confirm.";

/// A source document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Source {
    pub key: &'static str,
    pub citation: &'static str,
    pub url: &'static str,
    pub read: &'static str,
}

/// Sources read for this edition (2026-11-21).
pub const SOURCES: &[Source] = &[
    Source {
        key: "ITU-R-SM.329-13",
        citation: "Recommendation ITU-R SM.329-13 (09/2024), Unwanted emissions in the spurious domain",
        url: "https://itu.int/dms_pubrec/itu-r/rec/sm/R-REC-SM.329-13-202409-I!!PDF-E.pdf",
        read: "§ 4.1 (recommended reference bandwidths) and Table 2 (Category A spurious domain limits).",
    },
    Source {
        key: "ITU-R-SM.1539-2",
        citation: "Recommendation ITU-R SM.1539-2 (09/2024), Variation of the boundary between the out-of-band and spurious domains",
        url: "https://www.itu.int/dms_pubrec/itu-r/rec/sm/R-REC-SM.1539-2-202409-I!!PDF-E.pdf",
        read: "Table 1 (separation by case) and Table 2 (guideline values by frequency range).",
    },
    Source {
        key: "ITU-R-F.758-8",
        citation: "Recommendation ITU-R F.758-8 (02/2025), System parameters and considerations in the development of criteria for sharing or compatibility between digital fixed wireless systems in the fixed service and systems in other services",
        url: "https://www.itu.int/dms_pubrec/itu-r/rec/f/R-REC-F.758-8-202502-I!!PDF-E.pdf",
        read: "§ 4.13 and Table 5 (guidance in the choice of I/N values for long-term interference).",
    },
    Source {
        key: "ITU-R-M.2031-0",
        citation: "Recommendation ITU-R M.2031-0 (12/2012), Characteristics and protection criteria of receiving earth stations and characteristics of transmitting space stations of the radionavigation-satellite service (space-to-Earth) operating in the band 5 010-5 030 MHz",
        url: "https://www.itu.int/dms_pubrec/itu-r/rec/m/R-REC-M.2031-0-201212-S!!PDF-E.pdf",
        read: "recommends 1-2 and Annex 1, Table 1-1 (aggregate interference thresholds).",
    },
    Source {
        key: "ETSI-EN-300-328",
        citation: "ETSI EN 300 328 V2.2.2 (2019-07), Wideband transmission systems; Data transmission equipment operating in the 2,4 GHz band; Harmonised Standard for access to radio spectrum",
        url: "https://www.etsi.org/deliver/etsi_en/300300_300399/300328/02.02.02_60/en_300328v020202p.pdf",
        read: "Clause 4.3.1.10 Table 4 (FHSS transmitter spurious limits) and clause 4.3.1.12 Tables 6-8 (FHSS receiver blocking, categories 1-3; performance criterion PER or FER ≤ 10 %). The non-FHSS clauses (4.3.2.x, Tables 14-16) and the out-of-band mask of Figure 1 were not transcribed.",
    },
    Source {
        key: "ETSI-EN-303-413",
        citation: "ETSI EN 303 413 V1.2.1 (2021-04), Satellite Earth Stations and Systems (SES); GNSS receivers; Radio equipment operating in the 1 164 MHz to 1 300 MHz and 1 559 MHz to 1 610 MHz frequency bands; Harmonised Standard for access to radio spectrum",
        url: "https://www.etsi.org/deliver/etsi_en/303400_303499/303413/01.02.01_60/en_303413v010201p.pdf",
        read: "Clause 4.2.1 (receiver blocking: ΔC/N0 ≤ 1 dB; Tables 4-2, 4-3, 4-4).",
    },
];

/// Category of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    CoChannel,
    AdjacentBandBlocking,
    OutOfBandBoundary,
    SpuriousEmission,
    Intermodulation,
}

/// One catalogue entry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub id: &'static str,
    pub category: Category,
    /// Victim or service it applies to.
    pub applies_to: &'static str,
    /// Frequency relation (band, test frequency, offset rule).
    pub frequency: &'static str,
    /// Level: dB value, dBm, dBW or a formula, with its unit.
    pub level: &'static str,
    /// Interferer bandwidth or reference bandwidth.
    pub bandwidth: &'static str,
    /// Interferer type (CW, AWGN, ...), continuous unless stated.
    pub signal: &'static str,
    /// Pass criterion at the victim.
    pub criterion: &'static str,
    pub source: &'static str,
    pub clause: &'static str,
}

// A table row constructor: one argument per column keeps the catalogue readable.
#[allow(clippy::too_many_arguments)]
const fn e(
    id: &'static str,
    category: Category,
    applies_to: &'static str,
    frequency: &'static str,
    level: &'static str,
    bandwidth: &'static str,
    signal: &'static str,
    criterion: &'static str,
    source: &'static str,
    clause: &'static str,
) -> Entry {
    Entry {
        id,
        category,
        applies_to,
        frequency,
        level,
        bandwidth,
        signal,
        criterion,
        source,
        clause,
    }
}

use Category::*;

/// The catalogue.
pub const CATALOGUE: &[Entry] = &[
    // Co-channel: interference-to-noise criteria.
    e("cc-fs-below-3ghz", CoChannel, "fixed-service receiver", "30 MHz to 3 GHz", "I/N = -6 dB (aggregate, long term)", "receiver bandwidth", "aggregate interference", "I/N at or below the value", "ITU-R-F.758-8", "Table 5"),
    e("cc-fs-above-3ghz", CoChannel, "fixed-service receiver", "above 3 GHz", "I/N = -10 dB (aggregate, long term)", "receiver bandwidth", "aggregate interference", "I/N at or below the value", "ITU-R-F.758-8", "§ 4.13, Table 5"),
    e("cc-fs-uwb-indoor", CoChannel, "indoor FWA terminal (fixed service)", "3 to 6 GHz", "I/N = -13 dB", "receiver bandwidth", "UWB emissions (compatibility)", "I/N at or below the value", "ITU-R-F.758-8", "Table 5"),
    e("cc-rnss-c-track-nb", CoChannel, "RNSS receiving earth station (Galileo, C-band)", "5 010 to 5 030 MHz", "-157.1 dBW", "narrow-band (< 700 Hz)", "aggregate continuous interference", "at or below the tracking-mode threshold at the passive antenna output", "ITU-R-M.2031-0", "Annex 1, Table 1-1"),
    e("cc-rnss-c-acq-nb", CoChannel, "RNSS receiving earth station (Galileo, C-band)", "5 010 to 5 030 MHz", "-160.1 dBW", "narrow-band (< 700 Hz)", "aggregate continuous interference", "at or below the acquisition-mode threshold", "ITU-R-M.2031-0", "Annex 1, Table 1-1"),
    e("cc-rnss-c-track-wb", CoChannel, "RNSS receiving earth station (Galileo, C-band)", "5 010 to 5 030 MHz", "-147.1 dB(W/MHz)", "wideband (> 1 MHz)", "aggregate continuous interference", "at or below the tracking-mode threshold", "ITU-R-M.2031-0", "Annex 1, Table 1-1"),
    e("cc-rnss-c-acq-wb", CoChannel, "RNSS receiving earth station (Galileo, C-band)", "5 010 to 5 030 MHz", "-150.1 dB(W/MHz)", "wideband (> 1 MHz)", "aggregate continuous interference", "at or below the acquisition-mode threshold", "ITU-R-M.2031-0", "Annex 1, Table 1-1"),
    // Adjacent-band blocking: GNSS receivers (EN 303 413).
    e("blk-gnss-l1-1524", AdjacentBandBlocking, "GNSS receiver, 1 559-1 610 MHz", "1 524 MHz (MSS space-to-Earth, 1 518-1 525 MHz)", "-65 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-2 and 4-4"),
    e("blk-gnss-l1-1548", AdjacentBandBlocking, "GNSS receiver, 1 559-1 610 MHz", "1 548 MHz (MSS space-to-Earth, 1 525-1 549 MHz)", "-95 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-2 and 4-4"),
    e("blk-gnss-l1-1554", AdjacentBandBlocking, "GNSS receiver, 1 559-1 610 MHz", "1 554 MHz (MSS space-to-Earth, 1 549-1 559 MHz)", "-105 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-2 and 4-4"),
    e("blk-gnss-l1-1615", AdjacentBandBlocking, "GNSS receiver, 1 559-1 610 MHz", "1 615 MHz (MSS Earth-to-space, 1 610-1 626 MHz)", "-105 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-2 and 4-4"),
    e("blk-gnss-l1-1627", AdjacentBandBlocking, "GNSS receiver, 1 559-1 610 MHz", "1 627 MHz (MSS Earth-to-space, 1 626-1 640 MHz)", "-85 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-2 and 4-4"),
    e("blk-gnss-l5-1154", AdjacentBandBlocking, "GNSS receiver, 1 164-1 300 MHz", "1 154 MHz (AM(R)S, ARNS, 960-1 164 MHz)", "-75 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-3 and 4-4"),
    e("blk-gnss-l5-1310", AdjacentBandBlocking, "GNSS receiver, 1 164-1 300 MHz", "1 310 MHz (radiolocation, ARNS, RNSS Earth-to-space, 1 300-1 350 MHz)", "-85 dBm", "1 MHz", "AWGN", "ΔC/N0 ≤ 1 dB", "ETSI-EN-303-413", "4.2.1.2, Tables 4-3 and 4-4"),
    // Adjacent-band blocking: 2.4 GHz wideband data equipment (EN 300 328, FHSS Tables 6-8).
    e("blk-24-cat1", AdjacentBandBlocking, "2.4 GHz wideband data receiver (FHSS), receiver category 1", "2 300, 2 330, 2 360, 2 380, 2 504, 2 524, 2 584, 2 674 MHz", "-34 dBm", "-", "CW", "PER or FER ≤ 10 % (or no loss of the intended function) with the wanted signal the table states", "ETSI-EN-300-328", "4.3.1.12, Table 6"),
    e("blk-24-cat2", AdjacentBandBlocking, "2.4 GHz wideband data receiver (FHSS), receiver category 2", "2 300, 2 380, 2 504, 2 584 MHz", "-34 dBm", "-", "CW", "PER or FER ≤ 10 % (or no loss of the intended function) with the wanted signal the table states", "ETSI-EN-300-328", "4.3.1.12, Table 7"),
    e("blk-24-cat3", AdjacentBandBlocking, "2.4 GHz wideband data receiver (FHSS), receiver category 3", "2 300, 2 380, 2 504, 2 584 MHz", "-34 dBm", "-", "CW", "PER or FER ≤ 10 % (or no loss of the intended function) with the wanted signal the table states", "ETSI-EN-300-328", "4.3.1.12, Table 8"),
    // Out-of-band / spurious boundary.
    e("oob-boundary-normal", OutOfBandBoundary, "any transmitter (neighbour)", "centre frequency ± 2.5 × necessary bandwidth (normal case)", "boundary", "-", "-", "emissions beyond the boundary are spurious-domain", "ITU-R-SM.1539-2", "Table 1"),
    e("oob-boundary-wide-1-3ghz", OutOfBandBoundary, "any transmitter (neighbour), 1-3 GHz", "centre ± (50 MHz + 1.5 × BN) when BN > 50 MHz", "boundary", "-", "-", "wideband case", "ITU-R-SM.1539-2", "Table 2"),
    e("oob-boundary-wide-3-10ghz", OutOfBandBoundary, "any transmitter (neighbour), 3-10 GHz", "centre ± (100 MHz + 1.5 × BN) when BN > 100 MHz", "boundary", "-", "-", "wideband case", "ITU-R-SM.1539-2", "Table 2"),
    // Spurious emissions of neighbouring transmitters.
    e("spur-cat-a-general", SpuriousEmission, "transmitters of all services except those listed", "spurious domain", "43 + 10 log P dB or 70 dBc, whichever is less stringent", "1 kHz (9-150 kHz), 10 kHz (150 kHz-30 MHz), 100 kHz (30 MHz-1 GHz), 1 MHz (above 1 GHz)", "unwanted emission", "attenuation below the power supplied to the antenna line", "ITU-R-SM.329-13", "§ 4.1, Table 2"),
    e("spur-cat-a-space", SpuriousEmission, "space services (earth and space stations)", "spurious domain", "43 + 10 log P dB or 60 dBc, whichever is less stringent", "4 kHz (space services)", "unwanted emission", "attenuation below the power supplied to the antenna line", "ITU-R-SM.329-13", "§ 4.1, Table 2"),
    e("spur-24-tx-1-12ghz", SpuriousEmission, "2.4 GHz wideband data transmitter", "1 GHz to 12.75 GHz", "-30 dBm", "1 MHz", "unwanted emission", "maximum power", "ETSI-EN-300-328", "4.3.1.10, Table 4"),
    e("spur-24-tx-broadcast", SpuriousEmission, "2.4 GHz wideband data transmitter", "47-74, 87.5-118, 174-230, 470-694 MHz", "-54 dBm", "100 kHz", "unwanted emission", "maximum power", "ETSI-EN-300-328", "4.3.1.10, Table 4"),
    e("spur-24-tx-other", SpuriousEmission, "2.4 GHz wideband data transmitter", "30-47, 74-87.5, 118-174, 230-470, 694-1 000 MHz", "-36 dBm", "100 kHz", "unwanted emission", "maximum power", "ETSI-EN-300-328", "4.3.1.10, Table 4"),
    // Intermodulation: not read for this edition.
    e("im-third-order", Intermodulation, "any receiver", "2f1 - f2 and 2f2 - f1 falling in the operating channel", TO_CONFIRM, TO_CONFIRM, "two CW or modulated signals", TO_CONFIRM, TO_CONFIRM, "-"),
];

/// The victim of a scenario pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Victim {
    pub name: String,
    /// Entries of the catalogue that apply (ids).
    pub entries: Vec<String>,
    /// Receiver noise figure (dB) and bandwidth (Hz), for converting I/N criteria to a level.
    #[serde(default)]
    pub noise_figure_db: Option<f64>,
    #[serde(default)]
    pub bandwidth_hz: Option<f64>,
}

/// A neighbouring transmitter (for spurious-emission cases).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Neighbour {
    pub name: String,
    /// Mean power supplied to the antenna line (W).
    pub power_w: f64,
    /// Catalogue entry of its spurious limit.
    pub spurious_entry: String,
    /// Coupling loss from the neighbour's antenna line to the victim's receiver input (dB).
    pub coupling_loss_db: f64,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub victim: Victim,
    #[serde(default, rename = "neighbour")]
    pub neighbours: Vec<Neighbour>,
}

/// One scenario.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scenario {
    pub id: String,
    pub entry: String,
    pub category: Category,
    pub frequency: String,
    /// Level at the victim receiver input, as stated or derived (with the derivation).
    pub level: String,
    pub derivation: Option<String>,
    pub bandwidth: String,
    pub signal: String,
    pub criterion: String,
    pub source: String,
    pub clause: String,
}

/// The pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub scope: &'static str,
    pub title: String,
    pub victim: Victim,
    pub scenarios: Vec<Scenario>,
    pub to_confirm: Vec<String>,
    pub sources: Vec<Source>,
    pub verdict: String,
}

/// Thermal noise floor kTB·F (dBm) for bandwidth `b_hz` and noise figure `nf_db`.
pub fn noise_dbm(b_hz: f64, nf_db: f64) -> f64 {
    -174.0 + 10.0 * b_hz.log10() + nf_db
}

/// Spurious-domain level (dBm in the reference bandwidth) of a transmitter of `p_w` watts under
/// "43 + 10 log P dB or `cap` dBc, whichever is less stringent" (SM.329 Table 2).
pub fn sm329_level_dbm(p_w: f64, cap_dbc: f64) -> f64 {
    let p_dbm = 10.0 * (p_w * 1e3).log10();
    let att = (43.0 + 10.0 * p_w.log10()).min(cap_dbc);
    p_dbm - att
}

/// Build the scenario pack.
pub fn build(q: &Request) -> Result<Pack, String> {
    let mut scenarios = vec![];
    let mut to_confirm = vec![];
    let find = |id: &str| CATALOGUE.iter().find(|x| x.id == id);
    for id in &q.victim.entries {
        let en = find(id).ok_or_else(|| format!("victim entry '{id}' is not in the catalogue"))?;
        if en.source == TO_CONFIRM {
            to_confirm.push(en.id.to_string());
            continue;
        }
        let (level, derivation) =
            match (en.category, q.victim.noise_figure_db, q.victim.bandwidth_hz) {
                (Category::CoChannel, Some(nf), Some(b)) if en.level.starts_with("I/N") => {
                    let inr: f64 = en
                        .level
                        .trim_start_matches("I/N = ")
                        .split(' ')
                        .next()
                        .and_then(|x| x.parse().ok())
                        .ok_or("unreadable I/N")?;
                    let n = noise_dbm(b, nf);
                    (
                        format!("{:.1} dBm (aggregate)", n + inr),
                        Some(format!(
                        "N = -174 + 10 log10({b} Hz) + {nf} dB = {n:.1} dBm; I = N + ({inr} dB)"
                    )),
                    )
                }
                _ => (en.level.to_string(), None),
            };
        scenarios.push(Scenario {
            id: format!("S{:02}", scenarios.len() + 1),
            entry: en.id.into(),
            category: en.category,
            frequency: en.frequency.into(),
            level,
            derivation,
            bandwidth: en.bandwidth.into(),
            signal: en.signal.into(),
            criterion: en.criterion.into(),
            source: en.source.into(),
            clause: en.clause.into(),
        });
    }
    for nb in &q.neighbours {
        let en = find(&nb.spurious_entry)
            .filter(|x| x.category == Category::SpuriousEmission)
            .ok_or_else(|| {
                format!(
                    "neighbour {}: '{}' is not a spurious-emission entry",
                    nb.name, nb.spurious_entry
                )
            })?;
        if nb.power_w.is_nan() || nb.power_w <= 0.0 || nb.coupling_loss_db.is_nan() {
            return Err(format!(
                "neighbour {}: positive power and a coupling loss",
                nb.name
            ));
        }
        let (lvl, how) = if en.level.starts_with("43 + 10 log P") {
            let cap: f64 = if en.level.contains("60 dBc") {
                60.0
            } else {
                70.0
            };
            let l = sm329_level_dbm(nb.power_w, cap);
            (
                l,
                format!(
                    "{} W: {:.1} dBm - min(43 + 10 log P, {cap}) dB",
                    nb.power_w,
                    10.0 * (nb.power_w * 1e3).log10()
                ),
            )
        } else {
            let l: f64 = en
                .level
                .trim_end_matches(" dBm")
                .parse()
                .map_err(|_| format!("{}: level not in dBm", en.id))?;
            (l, format!("limit {} dBm", l))
        };
        scenarios.push(Scenario {
            id: format!("S{:02}", scenarios.len() + 1),
            entry: en.id.into(),
            category: en.category,
            frequency: en.frequency.into(),
            level: format!("{:.1} dBm at the victim input", lvl - nb.coupling_loss_db),
            derivation: Some(format!(
                "{}: {how} = {lvl:.1} dBm at the limit, less {} dB coupling",
                nb.name, nb.coupling_loss_db
            )),
            bandwidth: en.bandwidth.into(),
            signal: en.signal.into(),
            criterion: "compare with the victim's co-channel criterion".into(),
            source: en.source.into(),
            clause: en.clause.into(),
        });
    }
    let verdict = format!(
        "{} scenario(s) for {}; {} entr(ies) to confirm",
        scenarios.len(),
        q.victim.name,
        to_confirm.len()
    );
    Ok(Pack {
        format: FORMAT,
        scope: SCOPE,
        title: q.title.clone(),
        victim: q.victim.clone(),
        scenarios,
        to_confirm,
        sources: SOURCES.to_vec(),
        verdict,
    })
}

/// HTML page.
pub fn html(p: &Pack) -> String {
    crate::transport::page(
        &p.title,
        p.scope,
        &p.verdict,
        &[],
        &[
            (
                "Scenarios".into(),
                vec![
                    "id",
                    "category",
                    "frequency",
                    "level",
                    "bandwidth",
                    "signal",
                    "criterion",
                    "source",
                ],
                p.scenarios
                    .iter()
                    .map(|s| {
                        vec![
                            s.id.clone(),
                            serde_json::to_value(s.category)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_default(),
                            s.frequency.clone(),
                            match &s.derivation {
                                Some(d) => format!("{} ({d})", s.level),
                                None => s.level.clone(),
                            },
                            s.bandwidth.clone(),
                            s.signal.clone(),
                            s.criterion.clone(),
                            format!("{} {}", s.source, s.clause),
                        ]
                    })
                    .collect(),
            ),
            (
                "Sources".into(),
                vec!["key", "document", "read"],
                p.sources
                    .iter()
                    .map(|s| vec![s.key.into(), s.citation.into(), s.read.into()])
                    .collect(),
            ),
        ],
    )
}

/// A SYNTHETIC request: a 2.4 GHz category 1 data receiver beside a GNSS receiver and a
/// neighbouring 10 W land-mobile transmitter.
pub fn synth() -> String {
    r#"title = "SYNTHETIC co-existence scenario pack"

[victim]
name = "SYNTHETIC 2.4 GHz data link receiver"
entries = ["cc-fs-below-3ghz", "blk-24-cat1", "oob-boundary-normal", "im-third-order"]
noise_figure_db = 8.0
bandwidth_hz = 20000000.0

[[neighbour]]
name = "SYNTHETIC 10 W land-mobile transmitter"
power_w = 10.0
spurious_entry = "spur-cat-a-general"
coupling_loss_db = 60.0

[[neighbour]]
name = "SYNTHETIC co-located 2.4 GHz transmitter"
power_w = 0.1
spurious_entry = "spur-24-tx-1-12ghz"
coupling_loss_db = 40.0
"#
    .into()
}
