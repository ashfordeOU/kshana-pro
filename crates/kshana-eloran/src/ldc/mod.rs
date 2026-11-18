// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! LORAN Data Channel schemes (stream S2 implements Eurofix, 9th-pulse and generic PPM
//! with FEC/CRC). Each scheme is a [`crate::signal::GroupModulator`].

use crate::signal::GroupModulator;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum LdcConfig {
    /// No modulation (placeholder until the schemes land).
    None,
}

pub fn build(cfg: &LdcConfig) -> Result<Option<Box<dyn GroupModulator>>, String> {
    match cfg {
        LdcConfig::None => Ok(None),
    }
}
