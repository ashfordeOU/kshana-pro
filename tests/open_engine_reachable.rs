// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Every open kind is reachable through Kshana Pro, and gives the open engine's answer.
//!
//! "Pro is a superset of the open engine" is only true if each open kind actually runs
//! through the Pro entry points and returns what the open engine returns. The registry
//! tests prove the kinds are *registered*; this test *runs* them.
//!
//! For every kind the embedded engine lists (`kshana::api::list_scenario_kinds`), it
//! takes a real scenario file from the engine's own `scenarios/` directory and runs it
//! three ways:
//!
//! 1. directly through the open engine (`kshana::api::run_scenario`),
//! 2. through the Pro dispatcher at Evaluation tier, that is with no licence at all
//!    (`registry::run_gated`),
//! 3. by id through the open ∪ Pro registry (`registry::pro_registry().build(..)`).
//!
//! All three must succeed and give the same result document, byte for byte. A kind
//! with no runnable scenario file fails the test, so a new open kind cannot arrive
//! unproven.
//!
//! The engine's source directory is read from this crate's `Cargo.toml` and
//! `Cargo.lock`, so the test works the same whether the engine is a path dependency
//! (development) or the published crate (release), which ships its `scenarios/`
//! directory.
//!
//! Scenario files of the engine's optional Celeste in-orbit-demonstration preset
//! (`*celeste-iod*`) are never used: that preset is removable, and this test must pass
//! with it absent. Every kind has a generic scenario file without it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana_pro::license::Entitlement;
use kshana_pro::registry;

/// The source directory of the `kshana` package this build embeds.
///
/// Read from this crate's own manifest and lock file, with no subprocess and no
/// network: a path dependency names its directory; a published version is unpacked by
/// Cargo under `$CARGO_HOME/registry/src/<index>/kshana-<version>`.
fn engine_dir() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: toml::Value = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("Cargo.toml is readable")
        .parse()
        .expect("Cargo.toml is TOML");
    let dep = manifest
        .get("dependencies")
        .and_then(|d| d.get("kshana"))
        .expect("Cargo.toml declares the kshana dependency");
    if let Some(path) = dep.get("path").and_then(toml::Value::as_str) {
        let dir = root.join(path);
        assert!(
            dir.join("scenarios").is_dir(),
            "the kshana path dependency {} has no scenarios directory",
            dir.display()
        );
        return dir;
    }

    let lock: toml::Value = std::fs::read_to_string(root.join("Cargo.lock"))
        .expect("Cargo.lock is readable")
        .parse()
        .expect("Cargo.lock is TOML");
    let version = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock lists packages")
        .iter()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("kshana"))
        .and_then(|p| p.get("version").and_then(toml::Value::as_str))
        .expect("Cargo.lock pins the kshana package");
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .expect("CARGO_HOME or HOME is set");
    let src = cargo_home.join("registry").join("src");
    std::fs::read_dir(&src)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()))
        .filter_map(Result::ok)
        .map(|index| index.path().join(format!("kshana-{version}")))
        .find(|dir| dir.join("scenarios").is_dir())
        .unwrap_or_else(|| {
            panic!(
                "kshana {version} is not unpacked under {} with its scenarios directory",
                src.display()
            )
        })
}

/// Generic scenario files of the engine, as `(file name, text)`, in file-name order.
/// Suites (`*.suite.toml`) are lists of scenarios, not scenarios; the Celeste preset's
/// files are excluded (see the module comment).
fn generic_scenarios(dir: &Path) -> Vec<(String, String)> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("scenarios"))
        .expect("the engine ships its scenarios directory")
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".toml") && !n.ends_with(".suite.toml"))
        .filter(|n| !n.contains("celeste-iod"))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|n| {
            let text = std::fs::read_to_string(dir.join("scenarios").join(&n))
                .unwrap_or_else(|e| panic!("cannot read scenario {n}: {e}"));
            (n, text)
        })
        .collect()
}

/// The kind the open engine gives a scenario: its `kind` key, or `clock` when the key
/// is absent (the engine's historical default, which three bundled clock files rely on).
fn open_kind(src: &str) -> String {
    registry::peek_kind(src).unwrap_or_else(|| "clock".to_string())
}

#[test]
fn every_open_kind_runs_through_pro_and_matches_the_open_engine() {
    let dir = engine_dir();
    // Some scenarios name data files relative to the engine's root (as the engine's
    // own documentation runs them). This is the only test in this binary, so changing
    // the working directory cannot race another test.
    std::env::set_current_dir(&dir).expect("the engine directory is enterable");

    let mut by_kind: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (name, text) in generic_scenarios(&dir) {
        by_kind
            .entry(open_kind(&text))
            .or_default()
            .push((name, text));
    }

    let kinds = kshana::api::list_scenario_kinds();
    assert_eq!(
        kinds.len(),
        kshana_pro::open_engine_kinds(),
        "the Pro crate reports a different open kind count than the engine"
    );

    let evaluation = Entitlement::evaluation();
    let reg = registry::pro_registry();
    let mut failures: Vec<String> = Vec::new();
    let mut proven = 0usize;

    for meta in &kinds {
        let Some(files) = by_kind.get(meta.name) else {
            failures.push(format!(
                "{}: the engine ships no generic scenario file of this kind",
                meta.name
            ));
            continue;
        };
        // The first file of the kind (file-name order) is the one proven.
        let (file, src) = &files[0];

        let open = match kshana::api::run_scenario(src) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{} ({file}): open engine failed: {e}", meta.name));
                continue;
            }
        };
        let via_dispatcher = match registry::run_gated(src, &evaluation, 0) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!(
                    "{} ({file}): Pro dispatcher at Evaluation tier failed: {e}",
                    meta.name
                ));
                continue;
            }
        };
        let id = kshana::registry::ScenarioId::from_static(meta.name);
        let via_registry = match reg.build(&id, src).and_then(|s| {
            s.run()
                .map_err(|e| format!("run through the registry failed: {e}"))
        }) {
            Ok(o) => o,
            Err(e) => {
                failures.push(format!("{} ({file}): open ∪ Pro registry: {e}", meta.name));
                continue;
            }
        };

        if via_dispatcher.json != open.json {
            failures.push(format!(
                "{} ({file}): the Pro dispatcher's result differs from the open engine's",
                meta.name
            ));
            continue;
        }
        if via_registry.json != open.json {
            failures.push(format!(
                "{} ({file}): the open ∪ Pro registry's result differs from the open engine's",
                meta.name
            ));
            continue;
        }
        if via_dispatcher.summary != open.summary {
            failures.push(format!(
                "{} ({file}): the Pro dispatcher's summary differs from the open engine's",
                meta.name
            ));
            continue;
        }
        proven += 1;
    }

    assert!(
        failures.is_empty(),
        "{} of {} open kinds are not proven reachable through Pro:\n  {}",
        failures.len(),
        kinds.len(),
        failures.join("\n  ")
    );
    assert_eq!(proven, kinds.len());
}
