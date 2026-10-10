// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! INVARIANT: the changelog records the open-engine version this overlay is pinned to.
//!
//! The pin moved from 0.29.0 to 0.32.0 with no changelog entry, and nothing noticed.
//! This test reads the exact `kshana = "=X.Y.Z"` pin from `Cargo.toml` and requires the
//! phrase "engine X.Y.Z" in the changelog's `[Unreleased]` section or its newest release
//! section, so the next pin bump has to say so in the same change.

const CARGO_TOML: &str = include_str!("../Cargo.toml");
const CHANGELOG: &str = include_str!("../CHANGELOG.md");

/// The engine version from the `kshana = "=X.Y.Z"` dependency line.
fn pinned_engine_version() -> &'static str {
    let line = CARGO_TOML
        .lines()
        .find(|l| l.trim_start().starts_with("kshana = "))
        .expect("Cargo.toml has no `kshana = ...` dependency line");
    let quoted = line
        .split('"')
        .nth(1)
        .expect("the kshana dependency is not a quoted version requirement");
    quoted.trim_start_matches('=')
}

/// The text of `[Unreleased]` plus the newest release section (up to the next one).
fn current_sections() -> &'static str {
    let start = CHANGELOG
        .find("## [Unreleased]")
        .expect("CHANGELOG.md has no [Unreleased] section");
    let rest = &CHANGELOG[start..];
    // Skip [Unreleased] and the newest release heading; stop at the one after.
    let end = rest
        .match_indices("\n## [")
        .nth(1)
        .map_or(rest.len(), |(i, _)| i);
    &rest[..end]
}

#[test]
fn changelog_names_the_pinned_engine_version() {
    let v = pinned_engine_version();
    assert!(
        v.split('.').count() == 3 && v.split('.').all(|p| p.parse::<u32>().is_ok()),
        "expected an exact X.Y.Z engine pin, got {v:?}"
    );
    assert!(
        current_sections().contains(&format!("engine {v}")),
        "Cargo.toml pins kshana {v}, but CHANGELOG.md's [Unreleased] and newest release \
         sections never say \"engine {v}\". Record the pin bump in [Unreleased]."
    );
}
