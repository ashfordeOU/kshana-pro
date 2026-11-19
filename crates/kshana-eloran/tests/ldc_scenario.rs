// PROPRIETARY — Kshana Pro.
//! The Eurofix LDC scenario runs through the full scene, and its data changes the signal.
use kshana_eloran::ldc::{build_modulator, LdcDecoder};
use kshana_eloran::scenario::Scenario;
use kshana_eloran::signal::Role;

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios/ldc-eurofix.toml");

#[test]
fn ldc_eurofix_scenario_runs() {
    let src = std::fs::read_to_string(PATH).unwrap();
    let (sink, truth, stats) = kshana_eloran::run_to_memory(&src).unwrap();
    assert_eq!(stats.channels, 1);
    assert!(stats.truth_records > 0 && !truth.is_empty());
    assert!(stats.pulses > 0);

    // same scenario without the data channel: the samples differ (pulses 3–8 move)
    let sc = Scenario::from_toml(&src).unwrap();
    let ldc = sc.chains[0].ldc.clone().unwrap();
    let mut plain = sc.clone();
    plain.chains[0].ldc = None;
    let mut scene = plain.build().unwrap();
    let mut s2 = kshana_eloran::MemSink::default();
    let mut t2 = vec![];
    scene.run(&mut s2, &mut t2).unwrap();
    assert_eq!(s2.channels[0].len(), sink.channels[0].len());
    assert_ne!(s2.channels[0], sink.channels[0]);
    // pulse 1 timing (the truth ToA) is unchanged by the data channel
    assert_eq!(t2.len(), truth.len());
    for (a, b) in t2.iter().zip(&truth) {
        assert_eq!(a.toa_s, b.toa_s);
    }

    // the scenario's schedule decodes: one full cycle from the configuration alone
    let chain = &sc.chains[0].chain;
    let m = build_modulator(&ldc, Some(chain)).unwrap().unwrap();
    let dec = LdcDecoder::for_chain(&ldc, chain).unwrap().unwrap();
    let cycle = m.c.plans[0].cycle_frames;
    let f0 = 1_000 * cycle;
    let g = dec.groups_per_frame() as i64;
    let syms = m.symbols(1, Role::Secondary, f0 * g, (f0 + cycle) * g);
    let rep = dec.decode_symbols(&syms);
    assert_eq!(rep.messages.len(), 8);
    assert_eq!(
        dec.verify_auth(1, &rep, f0).last().unwrap().verified,
        Some(true)
    );
}

#[test]
fn ldc_gri_comes_from_the_chain() {
    let src = std::fs::read_to_string(PATH).unwrap();
    let sc = Scenario::from_toml(&src).unwrap();
    let ldc = sc.chains[0].ldc.clone().unwrap();
    // the scenario's utc_time message needs the GRI, and the ldc table does not state it
    assert_eq!(ldc.common().unwrap().gri_us, None);
    assert!(build_modulator(&ldc, None).is_err());
    let chain = &sc.chains[0].chain;
    let m = build_modulator(&ldc, Some(chain)).unwrap().unwrap();
    assert_eq!(m.c.gri_us, Some(chain.gri_us));
    assert_eq!(
        sc.ldc_configs().unwrap()[0]
            .as_ref()
            .unwrap()
            .common()
            .unwrap()
            .gri_us,
        Some(67_310)
    );
    sc.build().unwrap();
    // an explicit gri_us is an override: accepted when it agrees, an error otherwise
    for (gri, ok) in [(67_310, true), (79_800, false)] {
        let with = src.replace(
            "ldc = { scheme = \"eurofix\",",
            &format!("ldc = {{ scheme = \"eurofix\", gri_us = {gri},"),
        );
        let s2 = Scenario::from_toml(&with).unwrap();
        assert_eq!(
            s2.chains[0].ldc.as_ref().unwrap().common().unwrap().gri_us,
            Some(gri)
        );
        assert_eq!(s2.build().is_ok(), ok, "gri {gri}");
    }
}
