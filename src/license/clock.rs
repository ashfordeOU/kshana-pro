// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Clock rollback detection, basics (roadmap 0.42 C1, C10).
//!
//! A licence and a Pro pack expire at a signed time, so setting the computer's clock back
//! would stretch them. The guard keeps the latest time it has seen in a small file next to
//! the licence or the packs (`clock.json`). When the clock now reads earlier than that by
//! more than [`TOLERANCE_SECS`], the clock has been set back: the check reports it and the
//! caller uses the latest time seen instead of the clock, so nothing gains time.
//!
//! What this is not: a trusted time source. Someone who deletes the file and sets the clock
//! back before the first check is not caught, and a clock moved forward is believed (that
//! only shortens their own licence). The 35-day online renewal (server time) bounds what a
//! machine that stays offline can gain.

use std::path::{Path, PathBuf};

/// How far back the clock may read before it counts as set back: daylight-saving changes,
/// time-zone travel and small corrections stay inside it.
pub const TOLERANCE_SECS: i64 = 6 * 3600;

/// What the guard keeps on disk.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClockState {
    /// The latest Unix time any check has seen.
    pub max_seen_unix: i64,
    /// How many times the clock was found set back.
    #[serde(default)]
    pub rollbacks: u32,
}

/// The result of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockCheck {
    /// The time to use: the clock, or the latest time seen when the clock was set back.
    pub effective_unix: i64,
    /// The clock read earlier than the latest time seen by more than the tolerance.
    pub rolled_back: bool,
    /// How far back, seconds (0 when not set back).
    pub behind_secs: i64,
}

/// Pure check: `now` against the stored state; returns the check and the state to store.
pub fn observe(state: &ClockState, now: i64) -> (ClockCheck, ClockState) {
    let behind = state.max_seen_unix - now;
    let rolled_back = behind > TOLERANCE_SECS;
    let effective = if rolled_back {
        state.max_seen_unix
    } else {
        now
    };
    let next = ClockState {
        max_seen_unix: state.max_seen_unix.max(now),
        rollbacks: state.rollbacks + u32::from(rolled_back),
    };
    (
        ClockCheck {
            effective_unix: effective,
            rolled_back,
            behind_secs: if rolled_back { behind } else { 0 },
        },
        next,
    )
}

/// A file-backed guard.
#[derive(Debug, Clone)]
pub struct ClockGuard {
    path: PathBuf,
}

impl ClockGuard {
    /// The guard whose state lives in `dir/clock.json`.
    pub fn in_dir(dir: &Path) -> ClockGuard {
        ClockGuard {
            path: dir.join("clock.json"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The stored state (empty when absent or unreadable).
    pub fn state(&self) -> ClockState {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Check `now` and store the new latest time. A write failure does not fail the check
    /// (a read-only store still gets the comparison).
    pub fn check(&self, now: i64) -> ClockCheck {
        let (c, next) = observe(&self.state(), now);
        if next != self.state() {
            if let Some(d) = self.path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let tmp = self
                .path
                .with_extension(format!("tmp{}", std::process::id()));
            if std::fs::write(&tmp, serde_json::to_string(&next).unwrap_or_default()).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
        c
    }

    /// A warning for the user when the clock was found set back.
    pub fn warning(c: &ClockCheck) -> Option<String> {
        c.rolled_back.then(|| {
            format!(
                "this computer's clock reads {} hour(s) earlier than a time already seen; the licence uses the later time",
                c.behind_secs / 3600
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_time_is_believed_and_backward_within_tolerance_is_ignored() {
        let s = ClockState::default();
        let (c, s) = observe(&s, 1_000_000);
        assert!(!c.rolled_back);
        assert_eq!(c.effective_unix, 1_000_000);
        let (c, s) = observe(&s, 1_000_000 - TOLERANCE_SECS);
        assert!(!c.rolled_back);
        assert_eq!(c.effective_unix, 1_000_000 - TOLERANCE_SECS);
        assert_eq!(s.max_seen_unix, 1_000_000);
    }

    #[test]
    fn rollback_is_detected_and_the_latest_time_is_used() {
        let s = ClockState {
            max_seen_unix: 2_000_000,
            rollbacks: 0,
        };
        let (c, s2) = observe(&s, 2_000_000 - 10 * 86_400);
        assert!(c.rolled_back);
        assert_eq!(c.effective_unix, 2_000_000);
        assert_eq!(c.behind_secs, 10 * 86_400);
        assert_eq!(s2.rollbacks, 1);
        assert_eq!(s2.max_seen_unix, 2_000_000);
    }
}
