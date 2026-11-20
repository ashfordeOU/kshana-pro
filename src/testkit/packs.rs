// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Scenario packs (data in `testkit/packs/<pack>/<id>.toml`), compiled in. Every scenario is
//! MODELLED: representative parameters, modelled on the themes named in its `modelled_on`;
//! none is a test method of the documents it names, and none says a receiver meets them.

/// The scenarios of a pack: (id, TOML source).
pub type PackItems = &'static [(&'static str, &'static str)];

/// The scenario packs this build carries: (pack, description, scenarios).
pub const PACKS: &[(&str, &str, PackItems)] = &[
    (
        "dp-vessel",
        "DP vessels: GNSS spoofing and jamming on station, modelled on the themes of IMCA IN 1730",
        &[
            (
                "dp-gnss-jamming",
                include_str!("../../testkit/packs/dp-vessel/dp-gnss-jamming.toml"),
            ),
            (
                "dp-position-reference-disagreement",
                include_str!(
                    "../../testkit/packs/dp-vessel/dp-position-reference-disagreement.toml"
                ),
            ),
            (
                "dp-slow-drift",
                include_str!("../../testkit/packs/dp-vessel/dp-slow-drift.toml"),
            ),
            (
                "dp-stable-but-wrong",
                include_str!("../../testkit/packs/dp-vessel/dp-stable-but-wrong.toml"),
            ),
        ],
    ),
    (
        "aviation",
        "Aviation: post-spoof recovery and reset, jamming on approach",
        &[
            (
                "aviation-jamming-approach",
                include_str!("../../testkit/packs/aviation/aviation-jamming-approach.toml"),
            ),
            (
                "aviation-post-spoof-recovery",
                include_str!("../../testkit/packs/aviation/aviation-post-spoof-recovery.toml"),
            ),
            (
                "aviation-post-spoof-reset",
                include_str!("../../testkit/packs/aviation/aviation-post-spoof-reset.toml"),
            ),
        ],
    ),
    (
        "uas",
        "Drones (UAS): drag-off, jamming and meaconing",
        &[
            (
                "uas-hover-drag-off",
                include_str!("../../testkit/packs/uas/uas-hover-drag-off.toml"),
            ),
            (
                "uas-jamming-flight",
                include_str!("../../testkit/packs/uas/uas-jamming-flight.toml"),
            ),
            (
                "uas-meaconing",
                include_str!("../../testkit/packs/uas/uas-meaconing.toml"),
            ),
        ],
    ),
    (
        "timing",
        "Timing receivers: time push, holdover under jamming, meaconing delay",
        &[
            (
                "timing-holdover-under-jamming",
                include_str!("../../testkit/packs/timing/timing-holdover-under-jamming.toml"),
            ),
            (
                "timing-matched-power-time-push",
                include_str!("../../testkit/packs/timing/timing-matched-power-time-push.toml"),
            ),
            (
                "timing-meaconing-delay",
                include_str!("../../testkit/packs/timing/timing-meaconing-delay.toml"),
            ),
        ],
    ),
    (
        "rpcf",
        "Resilience-level test procedures modelled on the DHS RPCF levels 1 to 4",
        &[
            (
                "rpcf-level-1",
                include_str!("../../testkit/packs/rpcf/rpcf-level-1.toml"),
            ),
            (
                "rpcf-level-2",
                include_str!("../../testkit/packs/rpcf/rpcf-level-2.toml"),
            ),
            (
                "rpcf-level-3",
                include_str!("../../testkit/packs/rpcf/rpcf-level-3.toml"),
            ),
            (
                "rpcf-level-4",
                include_str!("../../testkit/packs/rpcf/rpcf-level-4.toml"),
            ),
        ],
    ),
    (
        "receiver-flags",
        "Receiver flags: CW and broadband jamming, simplistic, drag and time-push spoofing, clean baseline (flag-efficacy scorecards)",
        &[
            (
                "flags-clean",
                include_str!("../../testkit/packs/receiver-flags/flags-clean.toml"),
            ),
            (
                "flags-jam-broadband",
                include_str!("../../testkit/packs/receiver-flags/flags-jam-broadband.toml"),
            ),
            (
                "flags-jam-cw",
                include_str!("../../testkit/packs/receiver-flags/flags-jam-cw.toml"),
            ),
            (
                "flags-spoof-drag",
                include_str!("../../testkit/packs/receiver-flags/flags-spoof-drag.toml"),
            ),
            (
                "flags-spoof-simplistic",
                include_str!("../../testkit/packs/receiver-flags/flags-spoof-simplistic.toml"),
            ),
            (
                "flags-spoof-time-push",
                include_str!("../../testkit/packs/receiver-flags/flags-spoof-time-push.toml"),
            ),
        ],
    ),
];

/// The scenarios of one pack.
pub fn pack(name: &str) -> Result<PackItems, String> {
    PACKS
        .iter()
        .find(|p| p.0 == name)
        .map(|p| p.2)
        .ok_or_else(|| {
            format!(
                "unknown pack '{name}' (expected {})",
                PACKS.iter().map(|p| p.0).collect::<Vec<_>>().join(", ")
            )
        })
}

/// One pack scenario by `<pack>/<id>` or bare id.
pub fn find(name: &str) -> Option<(&'static str, &'static str)> {
    let (p, id) = match name.split_once('/') {
        Some((p, i)) => (Some(p), i),
        None => (None, name),
    };
    PACKS
        .iter()
        .filter(|x| p.is_none_or(|p| p == x.0))
        .flat_map(|x| x.2.iter().map(move |s| (x.0, s)))
        .find(|(_, s)| s.0 == id)
        .map(|(p, s)| (p, s.1))
}
