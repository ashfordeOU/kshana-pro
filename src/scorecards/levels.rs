// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Resilience levels as test evidence (roadmap 0.42, E3; used by T4 too).
//!
//! A scorecard **maps** its test results to the resilience levels of the DHS Resilient PNT
//! Conformance Framework and to the resilience topics of the IEEE P1952 draft standard
//! (resilient PNT user equipment). The mapping says which level the campaign's evidence
//! supports under the bars fixed before scoring, case by case: `met`, `not met` or `not
//! assessed` (the campaign lacks the data, for example no truth track). It is test evidence
//! for a reader who applies the framework, never a statement that the equipment meets it.
//!
//! The level wording below is Kshana's short summary; the published framework is the
//! authority (`report-templates/scorecards/dhs-rpcf-levels.json`).

use serde::{Deserialize, Serialize};

/// The result of one criterion on one case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Met,
    NotMet,
    NotAssessed,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Met => "met",
            Verdict::NotMet => "not met",
            Verdict::NotAssessed => "not assessed",
        }
    }
    pub fn from_bool(b: Option<bool>) -> Verdict {
        match b {
            Some(true) => Verdict::Met,
            Some(false) => Verdict::NotMet,
            None => Verdict::NotAssessed,
        }
    }
    /// Both must hold: a miss wins over a gap, a gap over a pass.
    pub fn and(self, o: Verdict) -> Verdict {
        match (self, o) {
            (Verdict::NotMet, _) | (_, Verdict::NotMet) => Verdict::NotMet,
            (Verdict::NotAssessed, _) | (_, Verdict::NotAssessed) => Verdict::NotAssessed,
            _ => Verdict::Met,
        }
    }
}

/// The four levels, as summarised here.
pub const LEVELS: [(u8, &str); 4] = [
    (1, "Recovers after the threat is removed"),
    (
        2,
        "Recovers, and gives no misleading output without warning during the threat",
    ),
    (
        3,
        "Level 2, and keeps its output within a stated bound during the threat",
    ),
    (4, "Level 3, and keeps full performance during the threat"),
];

/// The per-level verdicts of one case (index 0 is level 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseLevels {
    pub l1: Verdict,
    pub l2: Verdict,
    pub l3: Verdict,
    pub l4: Verdict,
}

impl CaseLevels {
    /// From the four criteria, each standing alone; each level includes the ones below it.
    pub fn from_criteria(
        recover: Verdict,
        no_misleading: Verdict,
        bounded: Verdict,
        full: Verdict,
    ) -> Self {
        let l1 = recover;
        let l2 = l1.and(no_misleading);
        let l3 = l2.and(bounded);
        let l4 = l3.and(full);
        CaseLevels { l1, l2, l3, l4 }
    }
    pub fn get(&self, level: u8) -> Verdict {
        match level {
            1 => self.l1,
            2 => self.l2,
            3 => self.l3,
            _ => self.l4,
        }
    }
}

/// What a campaign's evidence supports at one level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelEvidence {
    pub level: u8,
    pub summary: String,
    /// `supported` (every case met it), `not supported` (a case missed it) or `not assessed`.
    pub evidence: String,
    pub cases_met: usize,
    pub cases_not_met: usize,
    pub cases_not_assessed: usize,
}

/// The campaign-level mapping: each level, and the highest one every case supports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelMapping {
    pub framework: String,
    pub wording: String,
    pub levels: Vec<LevelEvidence>,
    /// The highest level whose evidence is `supported` for every case (0: none).
    pub highest_supported: u8,
}

/// The framework name as written in every mapping.
pub const FRAMEWORK: &str =
    "DHS Resilient PNT Conformance Framework (resilience levels 1 to 4), as summarised by Kshana";

/// The wording rule written into every mapping.
pub const WORDING: &str =
    "Test evidence mapped to the framework's resilience levels under the bars \
stated before scoring. It is not a statement that the equipment meets the framework or any \
standard; the reader applies the framework.";

/// Map a set of cases.
pub fn map(cases: &[CaseLevels]) -> LevelMapping {
    let mut levels = Vec::new();
    let mut highest = 0u8;
    let mut chain = true;
    for (lvl, summary) in LEVELS {
        let (mut met, mut not, mut na) = (0, 0, 0);
        for c in cases {
            match c.get(lvl) {
                Verdict::Met => met += 1,
                Verdict::NotMet => not += 1,
                Verdict::NotAssessed => na += 1,
            }
        }
        let evidence = if cases.is_empty() || na > 0 && not == 0 {
            "not assessed"
        } else if not > 0 {
            "not supported"
        } else {
            "supported"
        };
        if chain && evidence == "supported" {
            highest = lvl;
        } else {
            chain = false;
        }
        levels.push(LevelEvidence {
            level: lvl,
            summary: summary.into(),
            evidence: evidence.into(),
            cases_met: met,
            cases_not_met: not,
            cases_not_assessed: na,
        });
    }
    LevelMapping {
        framework: FRAMEWORK.into(),
        wording: WORDING.into(),
        levels,
        highest_supported: highest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Verdict::*;

    #[test]
    fn levels_nest_and_a_gap_stops_the_chain() {
        let c = CaseLevels::from_criteria(Met, NotAssessed, Met, Met);
        assert_eq!(
            (c.l1, c.l2, c.l3, c.l4),
            (Met, NotAssessed, NotAssessed, NotAssessed)
        );
        let d = CaseLevels::from_criteria(Met, Met, NotMet, Met);
        assert_eq!(d.l4, NotMet);
        let m = map(&[CaseLevels::from_criteria(Met, Met, Met, NotMet), d.clone()]);
        assert_eq!(m.highest_supported, 2);
        assert_eq!(m.levels[2].evidence, "not supported");
        assert_eq!(map(&[]).highest_supported, 0);
        let g = map(&[c]);
        assert_eq!(g.highest_supported, 1);
        assert_eq!(g.levels[1].evidence, "not assessed");
    }
}
