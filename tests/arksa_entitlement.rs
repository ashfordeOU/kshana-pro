// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `arksa` entitlement (roadmap 0.42 X1).
//!
//! * Tokens issued before the claim existed still verify; they lack the claim.
//! * Only an Arksa issuer key can grant it: a Pro key's token carrying it is refused, and
//!   editing it into or out of a token breaks the signature.
//! * Every Arksa-exclusive kind and feature refuses a standalone Pro token, whatever its
//!   licence tier or feature grants, with a message that names no kind and no method.
//! * An Arksa-entitled token runs them (build with the `arksa` feature).
//! * A default build contains none of them and refuses them by the manifest's hash.
//!
//! The two seeds below are TEST keys. They exist only in the tests and sign nothing that
//! ships; the public half of the Arksa one is `license::PLACEHOLDER_ARKSA_ISSUER_PUBKEY`.

use kshana_pro::license::tiers::{self, NOT_IN_THIS_EDITION};
use kshana_pro::license::{
    self, issue, issue_arksa, issue_v2, verify_with, Entitlement, Keyring, LicenseClaims, Tier,
    VerifyOptions,
};

const PRO_TEST_SEED: [u8; 32] = [7; 32];
const ARKSA_TEST_SEED: [u8; 32] = *b"arksa-issuer-test-key-not-real!!";

fn ring() -> Keyring {
    let mut r = Keyring::single(license::public_key_for(&PRO_TEST_SEED));
    r.add_arksa_issuer(license::public_key_for(&ARKSA_TEST_SEED));
    r
}

fn claims(tier: &str) -> LicenseClaims {
    LicenseClaims {
        licensee: "ACME Space".into(),
        tier: tier.into(),
        expires_unix: 10_000,
        products: vec!["cli".into(), "service".into()],
        ..Default::default()
    }
}

fn at(now: i64) -> VerifyOptions<'static> {
    VerifyOptions::at(now)
}

/// Rewrite the JSON payload of a signed token, keeping its signature.
fn edit_payload(token: &str, f: impl Fn(String) -> String) -> String {
    let (p, sig) = token.split_once('.').unwrap();
    let json = String::from_utf8(hex::decode(p).unwrap()).unwrap();
    format!("{}.{sig}", hex::encode(f(json)))
}

#[test]
fn old_tokens_verify_and_lack_the_claim() {
    let r = ring();
    let v1 = issue(
        &PRO_TEST_SEED,
        &LicenseClaims {
            licensee: "ACME".into(),
            tier: "enterprise".into(),
            expires_unix: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let v2 = issue_v2(&PRO_TEST_SEED, &claims("enterprise")).unwrap();
    for t in [&v1, &v2] {
        let l = verify_with(&r, t, &at(1)).unwrap();
        assert!(!l.claims.arksa);
        assert!(!l.entitlement.arksa);
        // The wire form of a token without the claim does not mention it.
        let json = String::from_utf8(hex::decode(t.split_once('.').unwrap().0).unwrap()).unwrap();
        assert!(!json.contains("arksa"), "{json}");
    }
}

#[test]
fn an_arksa_token_verifies_under_the_issuer_slot_only() {
    let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    let l = verify_with(&ring(), &t, &at(1)).unwrap();
    assert!(l.claims.arksa && l.entitlement.arksa);
    assert_eq!(l.entitlement.tier, Tier::Pro);
    // A build with no Arksa issuer (every default build) refuses it.
    let pro_only = Keyring::single(license::public_key_for(&PRO_TEST_SEED));
    assert!(verify_with(&pro_only, &t, &at(1)).is_err());
    // The issuer key is not a Pro key: a token without the claim signed by it is refused.
    let plain = issue_v2(&ARKSA_TEST_SEED, &claims("enterprise")).unwrap();
    assert!(verify_with(&ring(), &plain, &at(1)).is_err());
    // Expiry still applies.
    assert!(verify_with(&ring(), &t, &at(10_000)).is_err());
}

#[test]
fn a_pro_key_cannot_grant_the_claim() {
    let mut c = claims("enterprise");
    c.arksa = true;
    let forged = issue_v2(&PRO_TEST_SEED, &c).unwrap();
    assert!(verify_with(&ring(), &forged, &at(1)).is_err());
    let forged_v1 = issue(&PRO_TEST_SEED, &c).unwrap();
    assert!(verify_with(&ring(), &forged_v1, &at(1)).is_err());
}

#[test]
fn tampered_claims_fail() {
    let r = ring();
    // The claim added to a Pro token.
    let pro = issue_v2(&PRO_TEST_SEED, &claims("enterprise")).unwrap();
    let added = edit_payload(&pro, |j| j.replacen('{', "{\"arksa\":true,", 1));
    assert!(verify_with(&r, &added, &at(1)).is_err());
    // The claim removed from an Arksa token.
    let ark = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    let removed = edit_payload(&ark, |j| j.replace(",\"arksa\":true", ""));
    assert_ne!(removed, ark);
    assert!(verify_with(&r, &removed, &at(1)).is_err());
    // Another claim of an Arksa token changed: tier, expiry, licensee.
    for (from, to) in [
        ("\"tier\":\"pro\"", "\"tier\":\"enterprise\""),
        ("\"expires_unix\":10000", "\"expires_unix\":0"),
        ("ACME Space", "Someone Else"),
    ] {
        let t = edit_payload(&ark, |j| j.replace(from, to));
        assert_ne!(t, ark, "{from}");
        assert!(verify_with(&r, &t, &at(1)).is_err(), "{from}");
    }
    // The key ID of an Arksa token swapped to the Pro key's.
    let pro_kid = license::key_id(&license::public_key_for(&PRO_TEST_SEED));
    let ark_kid = license::key_id(&license::public_key_for(&ARKSA_TEST_SEED));
    let swapped = edit_payload(&ark, |j| j.replace(&ark_kid, &pro_kid));
    assert!(verify_with(&r, &swapped, &at(1)).is_err());
}

#[test]
fn a_retired_issuer_key_is_refused() {
    let mut r = ring();
    r.retire(&license::key_id(&license::public_key_for(&ARKSA_TEST_SEED)));
    let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    assert!(verify_with(&r, &t, &at(1)).is_err());
    assert!(r.arksa_issuer_ids().is_empty());
}

/// A standalone Pro token at the highest tier, granting every Arksa-exclusive id by name.
fn standalone_enterprise_naming(ids: &[&str]) -> Entitlement {
    let t = issue_v2(
        &PRO_TEST_SEED,
        &LicenseClaims {
            features: ids.iter().map(|s| s.to_string()).collect(),
            ..claims("enterprise")
        },
    )
    .unwrap();
    verify_with(&ring(), &t, &at(1)).unwrap().entitlement
}

/// Assert `kind` is refused for `ent` with the neutral message.
fn assert_refused_neutrally(ent: &Entitlement, kind: &str) {
    assert!(!ent.allows(kind, 1), "{kind}");
    let e = license::check(ent, kind, 1).unwrap_err().to_string();
    assert!(e.contains(NOT_IN_THIS_EDITION), "{kind}: {e}");
    assert!(!e.contains(kind), "the refusal names the kind: {e}");
    let src = format!("kind = \"{kind}\"\n");
    let e = kshana_pro::registry::run_gated(&src, ent, 1)
        .unwrap_err()
        .to_string();
    assert!(e.contains(NOT_IN_THIS_EDITION), "{kind}: {e}");
    assert!(!e.contains(kind), "the refusal names the kind: {e}");
}

#[cfg(feature = "arksa")]
mod with_arksa {
    use super::*;

    #[test]
    fn placeholder_issuer_key_is_the_test_key() {
        assert_eq!(
            hex::encode(license::public_key_for(&ARKSA_TEST_SEED)),
            license::PLACEHOLDER_ARKSA_ISSUER_PUBKEY
        );
    }

    #[test]
    fn the_arksa_exclusive_list_is_the_one_in_section_1a() {
        let mut kinds = tiers::ARKSA_KINDS.to_vec();
        kinds.sort();
        assert_eq!(
            kinds,
            [
                "pro-beacon-placement",
                "pro-cislunar-observability-study",
                "pro-design-optimise",
                "pro-lunar-signal-security",
                "pro-lunar-time-budget-study",
                "pro-mbse",
                "pro-optical-rf-hybrid-study",
                "pro-realtime-frame-eop-study",
                "pro-spectrum-coexistence",
            ]
        );
        for k in tiers::ARKSA_KINDS {
            assert!(
                kshana_pro::registry::is_pro_kind(k),
                "{k} is a registry kind"
            );
            assert_eq!(tiers::edition_of(k), Some(tiers::Edition::Arksa), "{k}");
        }
        for f in tiers::ARKSA_FEATURES {
            assert_eq!(tiers::edition_of(f), Some(tiers::Edition::Arksa), "{f}");
        }
    }

    #[test]
    fn a_standalone_pro_token_is_refused_for_each_arksa_exclusive_kind() {
        let ids: Vec<&str> = tiers::ARKSA_KINDS
            .iter()
            .chain(tiers::ARKSA_FEATURES)
            .copied()
            .collect();
        let standalone = standalone_enterprise_naming(&ids);
        assert!(!standalone.arksa);
        for id in &ids {
            assert_refused_neutrally(&standalone, id);
        }
        // The command gates refuse it too, with the same message.
        let e = license::ensure_mbse_entitled(&standalone, 1)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains(NOT_IN_THIS_EDITION) && !e.contains("mbse"),
            "{e}"
        );
        let e = kshana_pro::mission::ensure_entitled(&standalone, 1)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains(NOT_IN_THIS_EDITION) && !e.contains("mission"),
            "{e}"
        );
        // Evaluation, too.
        for id in &ids {
            assert_refused_neutrally(&Entitlement::evaluation(), id);
        }
        // Pro kinds are unchanged for the same token.
        assert!(standalone.allows("pro-twin", 1));
        assert!(standalone.allows("pro-batch", 1));
    }

    #[test]
    fn an_expired_arksa_token_runs_nothing_exclusive() {
        let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
        let mut o = at(20_000);
        o.allow_expired = true; // the grace path hands the caller an expired entitlement
        let ent = verify_with(&ring(), &t, &o).unwrap().entitlement;
        assert!(ent.arksa);
        for k in tiers::ARKSA_KINDS {
            assert!(!ent.allows(k, 20_000), "{k}");
        }
    }

    /// Small, fast sources for each Arksa-exclusive kind (the same configs as the
    /// registry's own build-and-run test).
    fn src(kind: &str) -> &'static str {
        match kind {
            "pro-mbse" => {
                "kind = \"pro-mbse\"\ntitle = \"t\"\n[[requirement]]\nid = \"SYS-001\"\ntext = \"mass <= 12 kg\"\ntarget = 12.0\nachieved = 10.5\nlower_is_better = true\n"
            }
            "pro-design-optimise" => {
                r#"kind = "pro-design-optimise"
title = "t"
seed = 1
base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""

[[variable]]
name = "eirp_dbw"
key = "eirp_dbw"
type = "real"
min = 40.0
max = 50.0
step = 5.0

[[objective]]
name = "margin_db"
sense = "max"
path = "margin_db"

[[objective]]
name = "eirp_dbw"
sense = "min"
variable = "eirp_dbw"
"#
            }
            "pro-spectrum-coexistence" => {
                r#"kind = "pro-spectrum-coexistence"
title = "t"
max_degradation_db = 1.0
limit_source = "test limit"
victims = ["gps-l5"]

[grid]
grid_step_deg = 45.0
duration_s = 600.0
step_s = 300.0

[[plan]]
label = "one"
signal = "generic-l"
mask_deg = 10.0

[[plan.satellite]]
altitude_km = 1000.0
inclination_deg = 53.0
"#
            }
            "pro-lunar-signal-security" => "kind = \"pro-lunar-signal-security\"\n",
            "pro-beacon-placement" => {
                "kind = \"pro-beacon-placement\"\nuser_n_lat = 1\nuser_n_lon = 1\ncand_n_lat = 2\ncand_n_lon = 2\nn_epochs = 1\nk_beacons = 1\n"
            }
            "pro-lunar-time-budget-study" => {
                "kind = \"pro-lunar-time-budget-study\"\nclock = \"passive-h-maser\"\ntau_min_s = 1.0\ntau_max_s = 1000.0\npoints_per_decade = 2\n"
            }
            "pro-cislunar-observability-study" => {
                "kind = \"pro-cislunar-observability-study\"\nperilune_km = [20000.0, 32000.0]\nref_phase_frac = [0.0, 0.25]\narc = 0.05\nepochs = 4\nsteps = 800\nwith_range_rate = true\n"
            }
            "pro-optical-rf-hybrid-study" => "kind = \"pro-optical-rf-hybrid-study\"\n",
            "pro-realtime-frame-eop-study" => {
                "kind = \"pro-realtime-frame-eop-study\"\nhorizons_days = [1]\n"
            }
            other => panic!("no source for {other}"),
        }
    }

    #[test]
    fn an_arksa_entitled_token_runs_each_arksa_exclusive_kind() {
        let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
        let ent = verify_with(&ring(), &t, &at(1)).unwrap().entitlement;
        for k in tiers::ARKSA_KINDS {
            assert!(ent.allows(k, 1), "{k}");
            let out = kshana_pro::registry::run_gated(src(k), &ent, 1)
                .unwrap_or_else(|e| panic!("{k} must run under an Arksa token: {e}"));
            assert!(!out.summary.is_empty(), "{k}");
        }
        for f in tiers::ARKSA_FEATURES {
            assert!(license::check(&ent, f, 1).is_ok(), "{f}");
        }
        assert!(license::ensure_mbse_entitled(&ent, 1).is_ok());
        assert!(kshana_pro::mission::ensure_entitled(&ent, 1).is_ok());
    }
}

#[cfg(not(feature = "arksa"))]
mod default_build {
    use super::*;

    #[test]
    fn a_default_build_contains_no_arksa_exclusive_kind_and_refuses_them_by_hash() {
        assert!(tiers::ARKSA_KINDS.is_empty() && tiers::ARKSA_FEATURES.is_empty());
        let m = tiers::manifest().unwrap();
        assert!(m.count_in(tiers::Edition::Arksa) > 0);
        for k in kshana_pro::registry::pro_kinds() {
            assert_ne!(tiers::edition_of(k.name), Some(tiers::Edition::Arksa));
        }
        // Two of the hidden names, here in the test only: refused by hash, neutrally,
        // even under an Arksa-entitled token (the code is not in this build).
        let standalone = standalone_enterprise_naming(&["pro-mbse", "pro-lunar-signal-security"]);
        for k in ["pro-mbse", "pro-lunar-signal-security"] {
            assert_eq!(tiers::edition_of(k), Some(tiers::Edition::Arksa));
            assert_refused_neutrally(&standalone, k);
        }
        let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
        let ent = verify_with(&ring(), &t, &at(1)).unwrap().entitlement;
        let e = kshana_pro::registry::run_gated("kind = \"pro-mbse\"\n", &ent, 1)
            .unwrap_err()
            .to_string();
        assert!(e.contains(NOT_IN_THIS_EDITION), "{e}");
    }
}
