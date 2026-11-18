// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receive antennas and output channels.
//!
//! Each simulator output channel represents one antenna element of the receiver under test:
//!
//! * [`OutputChannel::EField`]: a vertical E-field (whip) antenna, omnidirectional for the
//!   vertically polarised groundwave.
//! * [`OutputChannel::HLoop`]: one loop of a crossed-loop (bi-axial) H-field antenna. The
//!   groundwave's magnetic field is horizontal and perpendicular to the direction of
//!   propagation, so a loop responds as `cos(β − ψ − a)`, where β is the bearing of the
//!   station from the receiver, ψ the vehicle heading and `a` the bearing (relative to the
//!   bow) of the loop's maximum response. Two loops 90° apart give the two independent RF
//!   channels a bi-axial H-field receiver needs; the sign flip behind the loop's null is the
//!   physical 180° phase reversal.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputChannel {
    EField {
        #[serde(default = "one")]
        gain: f64,
    },
    HLoop {
        /// Bearing of maximum response relative to the bow (deg); 0 for loop A, 90 for B.
        #[serde(default)]
        axis_deg: f64,
        #[serde(default = "one")]
        gain: f64,
    },
}

fn one() -> f64 {
    1.0
}

impl OutputChannel {
    /// Linear gain toward a station at true bearing `bearing_rad` for heading `heading_rad`.
    #[inline]
    pub fn gain(&self, bearing_rad: f64, heading_rad: f64) -> f64 {
        match self {
            OutputChannel::EField { gain } => *gain,
            OutputChannel::HLoop { axis_deg, gain } => {
                gain * (bearing_rad - heading_rad - axis_deg.to_radians()).cos()
            }
        }
    }
    /// A crossed-loop pair (loop A on the bow axis, loop B athwartships).
    pub fn crossed_loops() -> Vec<OutputChannel> {
        vec![
            OutputChannel::HLoop {
                axis_deg: 0.0,
                gain: 1.0,
            },
            OutputChannel::HLoop {
                axis_deg: 90.0,
                gain: 1.0,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn crossed_loops_recover_bearing() {
        let l = OutputChannel::crossed_loops();
        for b_deg in [0.0f64, 30.0, 135.0, 250.0] {
            let b = b_deg.to_radians();
            let a = l[0].gain(b, 0.0);
            let c = l[1].gain(b, 0.0);
            let est = c.atan2(a).rem_euclid(std::f64::consts::TAU).to_degrees();
            assert!((est - b_deg).abs() < 1e-9, "{est} vs {b_deg}");
            assert!(((a * a + c * c) - 1.0).abs() < 1e-12);
        }
        // turning the vehicle by 90° moves the response from loop A to loop B
        assert!((l[0].gain(FRAC_PI_2, FRAC_PI_2) - 1.0).abs() < 1e-12);
        assert!(OutputChannel::EField { gain: 1.0 }.gain(1.0, 2.0) == 1.0);
    }
}
