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
    const UNITS: [&str; 20] = [
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
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    let w = w
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '-')
        .to_ascii_lowercase();
    if let Ok(n) = w.parse::<usize>() {
        return Some(n);
    }
    if let Some(n) = UNITS.iter().position(|x| *x == w) {
        return Some(n);
    }
    let (tens, unit) = match w.split_once('-') {
        Some((t, u)) => (t, Some(u)),
        None => (w.as_str(), None),
    };
    let t = TENS.iter().position(|x| *x == tens)?;
    let u = match unit {
        None => 0,
        Some(u) => match UNITS[1..10].iter().position(|x| *x == u) {
            Some(i) => i + 1,
            None => return None,
        },
    };
    Some(20 + 10 * t + u)
}

/// Every number written immediately before `phrase` in `doc`.
fn claims_in(doc: &str, phrase: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, _) in doc.match_indices(phrase) {
        if let Some(n) = number_before(&doc[..i]) {
            out.push(n);
        }
    }
    out
}

/// The number written at the end of `before`: one word or numeral, or a count of a hundred
/// or more spelled out ("one hundred", "one hundred and five", "two hundred twelve").
fn number_before(before: &str) -> Option<usize> {
    let toks: Vec<&str> = before.split_whitespace().rev().take(4).collect();
    let word = |i: usize| {
        toks.get(i).map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase()
        })
    };
    let hundreds = |i: usize| match toks.get(i).and_then(|t| word_to_number(t)) {
        Some(h) if (1..10).contains(&h) => Some(h * 100),
        _ => None,
    };
    if word(0).as_deref() == Some("hundred") {
        return hundreds(1);
    }
    let n = word_to_number(toks.first()?)?;
    if n < 100 {
        if word(1).as_deref() == Some("hundred") {
            if let Some(h) = hundreds(2) {
                return Some(h + n);
            }
        }
        if word(1).as_deref() == Some("and") && word(2).as_deref() == Some("hundred") {
            if let Some(h) = hundreds(3) {
                return Some(h + n);
            }
        }
    }
    Some(n)
}

fn claims(phrase: &str) -> Vec<usize> {
    claims_in(README, phrase)
}

/// The number of Pro kinds of every edition (the manifest lists them all, the Arksa-exclusive
/// ones by hash).
fn registered_kinds() -> usize {
    kshana_pro::license::tiers::manifest()
        .expect("the tier manifest verifies")
        .kind_count()
}

#[test]
fn readme_pro_pack_count_matches_the_registry() {
    // Every Pro kind of every edition, from the signed tier manifest: a default build does
    // not contain the Arksa-exclusive kinds (docs/PROTECTION.md) but still counts them.
    let registered = registered_kinds();
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
    // Every Pro kind of every edition, from the signed tier manifest: a default build does
    // not contain the Arksa-exclusive kinds (docs/PROTECTION.md) but still counts them.
    let registered = registered_kinds();
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

#[test]
fn number_words_parse() {
    assert_eq!(word_to_number("twenty-nine"), Some(29));
    assert_eq!(word_to_number("thirty-one"), Some(31));
    assert_eq!(word_to_number("forty"), Some(40));
    assert_eq!(word_to_number("**Sixteen"), Some(16));
    assert_eq!(word_to_number("thirty-zero"), None);
    assert_eq!(word_to_number("word"), None);
    assert_eq!(number_before("plus **one hundred and five"), Some(105));
    assert_eq!(number_before("plus one hundred"), Some(100));
    assert_eq!(number_before("plus two hundred twelve"), Some(212));
    assert_eq!(number_before("plus **ninety-nine"), Some(99));
}
