// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! GNSS-denied competence and drill packs for bridge simulators (research 05 #44, 07 B17,
//! G04, G12, 07b R03, R16, M03, 12 #1728).
//!
//! A drill is a scripted event sequence (jamming, a position drag-off, a time spoof, a
//! replay delay, a natural loss of sky as the negative control) rendered to bridge NMEA by
//! the open engine's training generator ([`kshana::nmea_synth`]), with:
//!
//! * the **instructor log** from the generator (what was injected when, with the true
//!   track), and the consistency aid's own reading of the same stream ([`super::consistency`])
//!   as the instructor's reference for when the evidence became visible;
//! * an **assessment rubric**: the observable crew actions, each with the window (seconds
//!   after the event's onset) in which it earns full marks, a late allowance at half marks,
//!   and items that must *not* happen (reporting spoofing during a natural loss of sky);
//! * **scoring** of a crew action log (`t_s,action[,by][,note]`) against the rubric, and a
//!   **training record** (trainee, vessel, instructor, date, score, pass mark) for the
//!   company's safety management system, signed with the rest of the directory.
//!
//! A readiness checklist (paper charts, sextant and tables, DR plotting practice, the
//! company procedure for GNSS interference) is scored the same way, with no stimulus.
//!
//! The rubric items describe what an assessor observes on the bridge; they are training
//! content measured against the IMO competence themes for navigation with GNSS affected
//! (STCW tables of competence) and the company procedure, not instructions for a real
//! voyage. MODELLED: the drills are synthetic; pass marks are assessor choices.

use serde::{Deserialize, Serialize};

/// The drill pack's format tag.
pub const FORMAT: &str = "kshana-pro-bridge-drill/1";
/// The score sheet's format tag.
pub const SCORE_FORMAT: &str = "kshana-pro-bridge-drill-score/1";

/// One rubric item.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: String,
    pub description: String,
    /// Full marks when done between these times after the event onset, seconds.
    pub from_s: f64,
    pub by_s: f64,
    /// Half marks when done this much later still.
    pub late_s: f64,
    pub points: u32,
    /// The item scores when the action is NOT recorded (a false report, for example).
    #[serde(default)]
    pub must_not: bool,
}

/// A drill.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Drill {
    pub format: String,
    pub id: String,
    pub title: String,
    pub objectives: Vec<String>,
    /// The competence themes the drill is measured against.
    pub competence: Vec<String>,
    /// Seconds from the start of the run to the onset the rubric times count from.
    pub onset_s: f64,
    /// Percentage of the available points needed to pass.
    pub pass_mark_pct: f64,
    pub rubric: Vec<Item>,
    /// Whether the drill has a stimulus (the readiness checklist has none).
    pub has_stimulus: bool,
    pub label: String,
}

fn item(id: &str, d: &str, from: f64, by: f64, late: f64, pts: u32) -> Item {
    Item {
        id: id.into(),
        description: d.into(),
        from_s: from,
        by_s: by,
        late_s: late,
        points: pts,
        must_not: false,
    }
}

fn must_not(id: &str, d: &str, pts: u32) -> Item {
    Item {
        id: id.into(),
        description: d.into(),
        from_s: 0.0,
        by_s: f64::MAX,
        late_s: 0.0,
        points: pts,
        must_not: true,
    }
}

const COASTAL_DRAG: &str = r#"
[scenario]
name = "drill-coastal-drag-off"
description = "Coastal passage; the reported position is dragged 1.2 km towards the land while the fix stays valid."
trainer_note = "Nothing alarms. Clues: GNSS COG/SOG part from gyro heading and log ground speed; GSV levels go uniform; the plotted position crosses the radar picture."
seed = 51
start_utc = "2026-09-14T13:00:00Z"
duration_s = 1500
rate_hz = 1
[vessel]
lat_deg = 54.20
lon_deg = 11.90
speed_kn = 12
[[waypoint]]
lat_deg = 54.45
lon_deg = 12.60
speed_kn = 12
[environment]
current_speed_kn = 0.8
current_set_deg = 60
[[event]]
kind = "drag-off"
label = "Drag towards the coast"
start_s = 400
duration_s = 700
ramp_s = 360
recovery_s = 0
final_offset_m = 1200
relative_bearing_deg = 270
counterfeit_cn0_dbhz = 46
"#;

const OPEN_SEA_JAM: &str = r#"
[scenario]
name = "drill-open-sea-jamming"
description = "Open sea; reception degrades until the fix is lost for several minutes."
trainer_note = "GSV levels fall together, then GGA quality 0 and RMC V. Heading and log stay healthy: the fall-back is dead reckoning on them."
seed = 52
start_utc = "2026-09-14T15:00:00Z"
duration_s = 1200
rate_hz = 1
[vessel]
lat_deg = 55.80
lon_deg = 18.20
speed_kn = 13
[[waypoint]]
lat_deg = 56.10
lon_deg = 18.90
speed_kn = 13
[[event]]
kind = "jamming"
label = "Jamming"
start_s = 300
duration_s = 480
ramp_s = 90
recovery_s = 60
cn0_drop_db = 36
"#;

const PORT_TIME: &str = r#"
[scenario]
name = "drill-port-time-spoof"
description = "Port approach; the GNSS time runs ahead while the position stays correct."
trainer_note = "Position and fix status look right. Compare the GNSS time with the ship's clock: it creeps ahead to 40 s."
seed = 53
start_utc = "2026-09-15T21:00:00Z"
duration_s = 1200
rate_hz = 1
[vessel]
lat_deg = 59.40
lon_deg = 24.50
speed_kn = 9
[[waypoint]]
lat_deg = 59.46
lon_deg = 24.72
speed_kn = 6
[[event]]
kind = "time-spoof"
label = "Time runs ahead"
start_s = 300
duration_s = 600
ramp_s = 120
recovery_s = 0
offset_s = 40
"#;

const REPLAY: &str = r#"
[scenario]
name = "drill-replay-delay"
description = "A replayed signal makes the receiver lag the ship by a growing delay."
trainer_note = "The fix stays valid; position and time lag. The reported speed falls below the log while the delay grows."
seed = 54
start_utc = "2026-09-16T08:00:00Z"
duration_s = 1200
rate_hz = 1
[vessel]
lat_deg = 57.00
lon_deg = 20.00
speed_kn = 14
[[waypoint]]
lat_deg = 57.30
lon_deg = 20.70
speed_kn = 14
[[event]]
kind = "replay-delay"
label = "Replay"
start_s = 300
duration_s = 600
ramp_s = 240
recovery_s = 0
delay_s = 30
counterfeit_cn0_dbhz = 44
"#;

const FJORD: &str = r#"
[scenario]
name = "drill-natural-degradation"
description = "Entering a fjord: steep sides block the low sky; nothing is counterfeit."
trainer_note = "The negative control. Satellites behind the terrain drop out with no SNR; the rest keep their levels. HDOP rises and the fix may degrade, but GNSS still agrees with the gyro and log."
seed = 55
start_utc = "2026-09-17T10:00:00Z"
duration_s = 1200
rate_hz = 1
[vessel]
lat_deg = 60.80
lon_deg = 5.00
speed_kn = 10
[[waypoint]]
lat_deg = 61.10
lon_deg = 5.00
speed_kn = 10
[[event]]
kind = "natural-degradation"
label = "Fjord"
start_s = 300
duration_s = 600
ramp_s = 120
recovery_s = 120
skyline = { preset = "fjord" }
"#;

/// The built-in drills: (drill, scenario TOML or empty for a checklist).
pub fn all() -> Vec<(Drill, &'static str)> {
    let mk = |id: &str,
              title: &str,
              objectives: &[&str],
              competence: &[&str],
              onset: f64,
              rubric: Vec<Item>,
              scn: &'static str| {
        (
            Drill {
                format: FORMAT.into(),
                id: id.into(),
                title: title.into(),
                objectives: objectives.iter().map(|s| s.to_string()).collect(),
                competence: competence.iter().map(|s| s.to_string()).collect(),
                onset_s: onset,
                pass_mark_pct: 70.0,
                rubric,
                has_stimulus: !scn.is_empty(),
                label: super::LABEL.into(),
            },
            scn,
        )
    };
    let report = |by: f64| {
        item(
            "report-master",
            "Reports the disagreement to the master",
            0.0,
            by,
            300.0,
            10,
        )
    };
    let log = item(
        "log-entry",
        "Records times, readings and the sensors involved in the log book",
        0.0,
        900.0,
        600.0,
        10,
    );
    let notify = item(
        "notify-authority",
        "Reports the interference by the company procedure (coastal authority or VTS)",
        0.0,
        1200.0,
        600.0,
        10,
    );
    vec![
        mk(
            "drill-coastal-drag-off",
            "Position drag-off in coastal waters",
            &[
                "Recognise a valid-looking GNSS position that disagrees with gyro, log and radar",
                "Cross-check the position by independent means",
                "Report and record the event",
            ],
            &[
                "Plan and conduct a passage and determine position (STCW A-II/1, A-II/2)",
                "Use of radar and ECDIS to maintain the safety of navigation",
                "Navigating with GNSS affected (IMO competence theme)",
            ],
            400.0,
            vec![
                item(
                    "detect-disagreement",
                    "Notices GNSS course/speed departing from gyro heading and log",
                    0.0,
                    240.0,
                    180.0,
                    25,
                ),
                item(
                    "cross-check-radar",
                    "Cross-checks the position by radar ranges/bearings or visual bearings",
                    0.0,
                    360.0,
                    240.0,
                    25,
                ),
                item(
                    "check-depth",
                    "Compares the echo-sounder depth with the charted depth",
                    0.0,
                    420.0,
                    240.0,
                    10,
                ),
                report(420.0),
                log.clone(),
                notify.clone(),
            ],
            COASTAL_DRAG,
        ),
        mk(
            "drill-open-sea-jamming",
            "Loss of GNSS by jamming in open sea",
            &[
                "Recognise jamming from falling signal levels before the fix is lost",
                "Keep the position by dead reckoning on gyro and log",
                "Report and record the event",
            ],
            &[
                "Determine position by dead reckoning (STCW A-II/1)",
                "Navigating with GNSS affected (IMO competence theme)",
            ],
            300.0,
            vec![
                item(
                    "acknowledge-loss",
                    "Acknowledges the loss-of-position alarm and states the cause considered",
                    0.0,
                    180.0,
                    120.0,
                    20,
                ),
                item(
                    "start-dr",
                    "Marks the last good position and starts a DR/EP plot from gyro and log",
                    0.0,
                    240.0,
                    180.0,
                    30,
                ),
                item(
                    "cross-check-radar",
                    "Checks the DR position by radar or visual means where available",
                    0.0,
                    480.0,
                    240.0,
                    10,
                ),
                report(300.0),
                log.clone(),
                notify.clone(),
            ],
            OPEN_SEA_JAM,
        ),
        mk(
            "drill-port-time-spoof",
            "GNSS time running ahead on a port approach",
            &[
                "Recognise a time error while the position is correct",
                "Identify the systems that take time from GNSS",
            ],
            &[
                "Navigating with GNSS affected (IMO competence theme)",
                "Bridge equipment that depends on GNSS time (AIS, VDR, GMDSS)",
            ],
            300.0,
            vec![
                item(
                    "detect-time",
                    "Notices the GNSS time departing from the ship's clock",
                    0.0,
                    360.0,
                    240.0,
                    30,
                ),
                item(
                    "list-affected",
                    "Names the bridge systems that take their time from GNSS",
                    0.0,
                    600.0,
                    300.0,
                    20,
                ),
                report(600.0),
                log.clone(),
                notify.clone(),
            ],
            PORT_TIME,
        ),
        mk(
            "drill-replay-delay",
            "Replayed GNSS signal (meaconing)",
            &[
                "Recognise a lagging GNSS position and time with a valid fix",
                "Cross-check by independent means and report",
            ],
            &["Navigating with GNSS affected (IMO competence theme)"],
            300.0,
            vec![
                item(
                    "detect-lag",
                    "Notices GNSS speed below the log, or the position lagging the radar picture",
                    0.0,
                    300.0,
                    240.0,
                    30,
                ),
                item(
                    "cross-check-radar",
                    "Cross-checks the position by radar or visual bearings",
                    0.0,
                    420.0,
                    240.0,
                    20,
                ),
                report(480.0),
                log.clone(),
                notify.clone(),
            ],
            REPLAY,
        ),
        mk(
            "drill-natural-degradation",
            "Natural loss of sky in a fjord (negative control)",
            &[
                "Tell natural degradation from interference",
                "Avoid a false interference report",
            ],
            &["Navigating with GNSS affected (IMO competence theme)"],
            300.0,
            vec![
                item(
                    "note-degradation",
                    "Notes the higher HDOP or fewer satellites and relates it to the terrain",
                    0.0,
                    480.0,
                    240.0,
                    40,
                ),
                must_not(
                    "report-spoofing",
                    "Reports spoofing or jamming although GNSS agrees with gyro and log",
                    40,
                ),
                log,
            ],
            FJORD,
        ),
        mk(
            "readiness-checklist",
            "GNSS-denied readiness: equipment and practice",
            &["Confirm the means and the practice to navigate without GNSS are on board"],
            &[
                "Determine position by celestial navigation and dead reckoning (STCW A-II/1)",
                "Company procedure for GNSS interference",
            ],
            0.0,
            vec![
                item("paper-charts", "Paper charts (or an approved back-up arrangement) for the voyage are on board and corrected", 0.0, f64::MAX, 0.0, 15),
                item("sextant", "A sextant, nautical almanac and sight-reduction tables are on board", 0.0, f64::MAX, 0.0, 15),
                item("sight-practice", "An officer has taken and reduced a sight within the last three months", 0.0, f64::MAX, 0.0, 15),
                item("dr-practice", "A DR/EP plot was kept for at least one hour within the last three months", 0.0, f64::MAX, 0.0, 15),
                item("procedure", "The company procedure for GNSS interference is on the bridge and known to the watch officers", 0.0, f64::MAX, 0.0, 20),
                item("dependency-list", "The bridge has a list of the equipment that depends on GNSS position or time", 0.0, f64::MAX, 0.0, 20),
            ],
            "",
        ),
    ]
}

/// The drill `id` with its scenario TOML.
pub fn find(id: &str) -> Result<(Drill, &'static str), String> {
    all().into_iter().find(|(d, _)| d.id == id).ok_or_else(|| {
        format!(
            "unknown drill '{id}' (known: {})",
            all()
                .iter()
                .map(|(d, _)| d.id.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// A built drill.
pub struct BuiltDrill {
    pub drill: Drill,
    pub scenario_toml: String,
    pub stimulus: Option<String>,
    pub instructor_json: Option<String>,
    pub instructor_text: Option<String>,
    pub aid: Option<super::consistency::Report>,
}

/// Build a drill: generate its stimulus and run the consistency aid on it.
pub fn build(id: &str) -> Result<BuiltDrill, String> {
    let (drill, scn) = find(id)?;
    if scn.is_empty() {
        return Ok(BuiltDrill {
            drill,
            scenario_toml: String::new(),
            stimulus: None,
            instructor_json: None,
            instructor_text: None,
            aid: None,
        });
    }
    let g = kshana::nmea_synth::generate_from_toml(scn, None)?;
    let stimulus = g.nmea_text();
    let aid = super::consistency::run(stimulus.as_bytes(), &Default::default())?;
    Ok(BuiltDrill {
        drill,
        scenario_toml: scn.trim_start().to_string(),
        instructor_json: Some(g.log.to_json()),
        instructor_text: Some(g.log.to_text()),
        stimulus: Some(stimulus),
        aid: Some(aid),
    })
}

/// Write a built drill into `dir`.
pub fn write_dir(dir: &std::path::Path, b: &BuiltDrill) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let w = |n: &str, t: &str| std::fs::write(dir.join(n), t).map_err(|e| format!("{n}: {e}"));
    w(
        "drill.json",
        &(serde_json::to_string_pretty(&b.drill).map_err(|e| e.to_string())? + "\n"),
    )?;
    if let Some(s) = &b.stimulus {
        w("scenario.toml", &b.scenario_toml)?;
        w("stimulus.nmea", s)?;
    }
    if let Some(s) = &b.instructor_json {
        w("instructor-log.json", s)?;
    }
    if let Some(s) = &b.instructor_text {
        w("instructor-log.txt", s)?;
    }
    if let Some(a) = &b.aid {
        w(
            "aid-reference.txt",
            &format!(
                "The consistency aid's reading of this stimulus, for the instructor: when the \
                 evidence became visible to an automatic cross-check. Not a model answer for \
                 the crew.\n\n{}",
                super::consistency::summary_text(a)
            ),
        )?;
    }
    let mut rub = String::from("id,description,from_s,by_s,late_s,points,must_not\n");
    for i in &b.drill.rubric {
        let fmt = |x: f64| {
            if x == f64::MAX {
                String::new()
            } else {
                x.to_string()
            }
        };
        rub.push_str(&format!(
            "{},\"{}\",{},{},{},{},{}\n",
            i.id,
            i.description.replace('"', "'"),
            fmt(i.from_s),
            fmt(i.by_s),
            i.late_s,
            i.points,
            i.must_not
        ));
    }
    w("rubric.csv", &rub)
}

/// One recorded crew action.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub t_s: f64,
    pub action: String,
    pub by: String,
}

/// Parse a crew action log: `t_s,action[,by][,note]` with a header. A checklist log may
/// leave `t_s` empty.
pub fn parse_actions(text: &str) -> Result<Vec<Action>, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head: Vec<String> = lines
        .next()
        .ok_or("empty action log")?
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| head.iter().position(|h| h == n);
    let (Some(ct), Some(ca)) = (col("t_s"), col("action")) else {
        return Err("the action log needs columns t_s and action".into());
    };
    let cb = col("by");
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let t = match f.get(ct).copied().unwrap_or("") {
            "" => 0.0,
            v => v
                .parse::<f64>()
                .map_err(|_| format!("row {}: t_s is not a number", i + 2))?,
        };
        let action = f.get(ca).copied().unwrap_or("").to_string();
        if action.is_empty() {
            return Err(format!("row {}: no action", i + 2));
        }
        out.push(Action {
            t_s: t,
            action,
            by: cb
                .and_then(|c| f.get(c))
                .map(|s| s.to_string())
                .unwrap_or_default(),
        });
    }
    Ok(out)
}

/// One item's score.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ItemScore {
    pub id: String,
    pub points: u32,
    pub earned: f64,
    pub status: String,
    /// When the action was recorded, seconds after the onset.
    pub at_s: Option<f64>,
}

/// The trainee and session a score sheet is for.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub trainee: String,
    pub vessel_or_centre: String,
    pub instructor: String,
    pub date: String,
}

/// A score sheet and training record.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScoreSheet {
    pub format: String,
    pub drill: String,
    pub title: String,
    pub session: Session,
    pub items: Vec<ItemScore>,
    pub earned: f64,
    pub available: u32,
    pub percent: f64,
    pub pass_mark_pct: f64,
    pub passed: bool,
    /// Actions recorded that match no rubric item.
    pub unmatched_actions: Vec<String>,
    pub competence: Vec<String>,
    pub label: String,
    pub record_note: String,
}

/// Score a crew action log against a drill's rubric.
pub fn score(d: &Drill, actions: &[Action], session: Session) -> ScoreSheet {
    let mut items = Vec::new();
    let mut earned = 0.0;
    let mut available = 0;
    for it in &d.rubric {
        available += it.points;
        let first = actions
            .iter()
            .filter(|a| a.action == it.id)
            .map(|a| a.t_s - d.onset_s)
            .min_by(|a, b| a.total_cmp(b));
        let (e, status) = if it.must_not {
            match first {
                Some(_) => (0.0, "recorded (should not have been)".to_string()),
                None => (it.points as f64, "not recorded (correct)".to_string()),
            }
        } else {
            match first {
                None => (0.0, "not recorded".to_string()),
                Some(t) if t < it.from_s - 1e-9 => (0.0, "before the event".to_string()),
                Some(t) if t <= it.by_s + 1e-9 => (it.points as f64, "in time".to_string()),
                Some(t) if t <= it.by_s + it.late_s + 1e-9 => {
                    (it.points as f64 / 2.0, "late (half marks)".to_string())
                }
                Some(_) => (0.0, "too late".to_string()),
            }
        };
        earned += e;
        items.push(ItemScore {
            id: it.id.clone(),
            points: it.points,
            earned: e,
            status,
            at_s: if it.must_not { None } else { first },
        });
    }
    let percent = if available == 0 {
        0.0
    } else {
        super::sig9(100.0 * earned / available as f64)
    };
    let mut unmatched: Vec<String> = actions
        .iter()
        .filter(|a| !d.rubric.iter().any(|i| i.id == a.action))
        .map(|a| a.action.clone())
        .collect();
    unmatched.sort();
    unmatched.dedup();
    ScoreSheet {
        format: SCORE_FORMAT.into(),
        drill: d.id.clone(),
        title: d.title.clone(),
        session,
        items,
        earned,
        available,
        percent,
        pass_mark_pct: d.pass_mark_pct,
        passed: percent >= d.pass_mark_pct,
        unmatched_actions: unmatched,
        competence: d.competence.clone(),
        label: d.label.clone(),
        record_note: "Training record for the safety management system: a drill on a \
                      simulator or a training stream, scored against the rubric in this \
                      directory. It records what was observed; it does not attest \
                      competence."
            .into(),
    }
}

/// The score sheet as text.
pub fn score_text(s: &ScoreSheet) -> String {
    let mut t = format!(
        "Drill: {} ({})\nTrainee: {}  Vessel/centre: {}  Instructor: {}  Date: {}\n\n",
        s.title,
        s.drill,
        s.session.trainee,
        s.session.vessel_or_centre,
        s.session.instructor,
        s.session.date
    );
    for i in &s.items {
        t.push_str(&format!(
            "  {:<22} {:>5.1}/{:<3} {}{}\n",
            i.id,
            i.earned,
            i.points,
            i.status,
            i.at_s.map(|x| format!(" at +{x:.0} s")).unwrap_or_default()
        ));
    }
    t.push_str(&format!(
        "\nScore {:.1}/{} = {:.1} % (pass mark {:.0} %): {}\n{}\n{}\n",
        s.earned,
        s.available,
        s.percent,
        s.pass_mark_pct,
        if s.passed { "PASS" } else { "NOT YET" },
        s.record_note,
        s.label
    ));
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drill_parses_and_every_rubric_is_sane() {
        for (d, scn) in all() {
            assert!(!d.rubric.is_empty(), "{}", d.id);
            assert!(d.rubric.iter().all(|i| i.points > 0 && i.by_s >= i.from_s));
            if !scn.is_empty() {
                kshana::nmea_synth::TrainingScenario::parse(scn)
                    .unwrap_or_else(|e| panic!("{}: {e}", d.id));
            }
        }
        assert!(find("nope").is_err());
    }

    #[test]
    fn scoring_in_time_late_and_must_not() {
        let (d, _) = find("drill-natural-degradation").unwrap();
        let a = parse_actions("t_s,action,by\n500,note-degradation,OOW\n").unwrap();
        let s = score(&d, &a, Session::default());
        assert_eq!(s.earned, 80.0);
        assert!(s.passed, "80/90 is above the 70 % pass mark");
        let a =
            parse_actions("t_s,action\n500,note-degradation\n600,report-spoofing\n700,log-entry\n")
                .unwrap();
        let s = score(&d, &a, Session::default());
        assert_eq!(s.earned, 50.0);
        let (d, _) = find("drill-open-sea-jamming").unwrap();
        // start-dr 80 s past its window (onset 300 s, window 240 s): half marks.
        let a = parse_actions("t_s,action\n620,start-dr\n").unwrap();
        let s = score(&d, &a, Session::default());
        assert_eq!(
            s.items.iter().find(|i| i.id == "start-dr").unwrap().earned,
            15.0
        );
        let a = parse_actions("t_s,action\n100,start-dr\n,unknown\n").unwrap();
        let s = score(&d, &a, Session::default());
        assert_eq!(
            s.items.iter().find(|i| i.id == "start-dr").unwrap().earned,
            0.0
        );
        assert_eq!(s.unmatched_actions, vec!["unknown".to_string()]);
    }
}
