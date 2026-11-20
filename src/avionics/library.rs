// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The built-in procedures (`avionics/procedures/<id>.toml`), compiled in. Every procedure is
//! MODELLED: modelled on the documents its `modelled_on` names (cited by section only), with
//! representative stimulus values and the limits left to the applicant.

use super::procedure::Procedure;

/// The built-in procedures: (id, TOML source).
pub const PROCEDURES: &[(&str, &str)] = &[
    (
        "araim-h-dual-constellation",
        include_str!("../../avionics/procedures/araim-h-dual-constellation.toml"),
    ),
    (
        "araim-institutional-unusable",
        include_str!("../../avionics/procedures/araim-institutional-unusable.toml"),
    ),
    (
        "araim-leo-augmentation",
        include_str!("../../avionics/procedures/araim-leo-augmentation.toml"),
    ),
    (
        "araim-published-vectors",
        include_str!("../../avionics/procedures/araim-published-vectors.toml"),
    ),
    (
        "gbas-vdb-loss-and-fas",
        include_str!("../../avionics/procedures/gbas-vdb-loss-and-fas.toml"),
    ),
    (
        "spoof-detection-output",
        include_str!("../../avionics/procedures/spoof-detection-output.toml"),
    ),
    (
        "sbas-fast-correction-step",
        include_str!("../../avionics/procedures/sbas-fast-correction-step.toml"),
    ),
    (
        "sbas-fast-correction-timeout",
        include_str!("../../avionics/procedures/sbas-fast-correction-timeout.toml"),
    ),
    (
        "sbas-message-loss",
        include_str!("../../avionics/procedures/sbas-message-loss.toml"),
    ),
    (
        "sbas-mt0-do-not-use",
        include_str!("../../avionics/procedures/sbas-mt0-do-not-use.toml"),
    ),
    (
        "sbas-udrei-do-not-use",
        include_str!("../../avionics/procedures/sbas-udrei-do-not-use.toml"),
    ),
    (
        "sbas-udrei-not-monitored",
        include_str!("../../avionics/procedures/sbas-udrei-not-monitored.toml"),
    ),
];

/// One built-in procedure by id.
pub fn find(id: &str) -> Option<&'static str> {
    PROCEDURES.iter().find(|p| p.0 == id).map(|p| p.1)
}

/// Parse one built-in procedure.
pub fn get(id: &str) -> Result<Procedure, String> {
    let src = find(id).ok_or_else(|| {
        format!(
            "unknown procedure '{id}' (expected {})",
            PROCEDURES
                .iter()
                .map(|p| p.0)
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    Procedure::parse(src)
}
