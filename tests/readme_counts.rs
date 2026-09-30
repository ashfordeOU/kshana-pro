// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! README count guard.
//!
//! The README states how many Pro packs are registered and how many integration tests
//! prove the overlay; the overlay architecture note states the pack count too. The pack
//! count drifted once (the README said seven while the registry held sixteen). This
//! test reads every such claim and fails if it no longer matches
//! `registry::pro_kinds()` or the `tests/*.rs` files on disk.

const README: &str = include_str!("../README.md");
const ARCHITECTURE: &str = include_str!("../docs/OVERLAY-ARCHITECTURE.md");

fn word_to_number(w: &str) -> Option<usize> {
    const WORDS: [&str; 31] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
        "twenty-one",
        "twenty-two",
        "twenty-three",
        "twenty-four",
        "twenty-five",
        "twenty-six",
        "twenty-seven",
        "twenty-eight",
        "twenty-nine",
        "thirty",
    ];
    let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
    w.parse::<usize>()
        .ok()
        .or_else(|| WORDS.iter().position(|x| x.eq_ignore_ascii_case(w)))
}

/// Every number written immediately before `phrase` in `doc`.
fn claims_in(doc: &str, phrase: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, _) in doc.match_indices(phrase) {
        let before = &doc[..i];
        if let Some(tok) = before.split_whitespace().last() {
            if let Some(n) = word_to_number(tok) {
                out.push(n);
            }
        }
    }
    out
}

fn claims(phrase: &str) -> Vec<usize> {
    claims_in(README, phrase)
}

#[test]
fn readme_pro_pack_count_matches_the_registry() {
    let registered = kshana_pro::registry::pro_kinds().len();
    let found = claims("Pro packs");
    assert!(
        found.len() >= 2,
        "expected the README to state the Pro pack count at least twice, found {found:?}"
    );
    for n in found {
        assert_eq!(
            n, registered,
            "README says {n} Pro packs; src/registry.rs registers {registered}"
        );
    }
    // The sentence under "Built Pro packs" states the count a third time ("the full list
    // of the <count> registered in src/registry.rs"); it stayed at sixteen when the
    // seventeenth pack landed.
    let third = claims("registered in");
    assert!(
        !third.is_empty(),
        "README no longer states how many packs are registered in src/registry.rs"
    );
    for n in third {
        assert_eq!(
            n, registered,
            "README says {n} registered in src/registry.rs; it registers {registered}"
        );
    }
}

#[test]
fn architecture_doc_pro_pack_count_matches_the_registry() {
    let registered = kshana_pro::registry::pro_kinds().len();
    let found = claims_in(ARCHITECTURE, "Pro packs");
    assert!(
        !found.is_empty(),
        "docs/OVERLAY-ARCHITECTURE.md no longer states the count"
    );
    for n in found {
        assert_eq!(
            n, registered,
            "docs/OVERLAY-ARCHITECTURE.md says {n} Pro packs; src/registry.rs registers {registered}"
        );
    }
}

#[test]
fn readme_integration_test_count_matches_the_tests_directory() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let on_disk = std::fs::read_dir(dir)
        .expect("tests/ is readable")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .count();
    let found = claims("integration tests");
    assert!(
        !found.is_empty(),
        "README no longer states the integration-test count"
    );
    for n in found {
        assert_eq!(
            n, on_disk,
            "README says {n} integration tests; tests/ holds {on_disk}"
        );
    }
}
