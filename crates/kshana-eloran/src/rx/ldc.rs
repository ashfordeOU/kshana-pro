// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Data-channel (LDC) decoding inside the software receiver.
//!
//! Enabled per chain by [`super::RxConfig::ldc`] (the chain's [`crate::ldc::LdcConfig`],
//! normally `Scenario::ldc_configs()`). After tracking, for every tracked station that
//! carries data:
//!
//! 1. **Per group**: [`Receiver::pulse_timing_with`] measures each pulse's timing offset
//!    and carrier phase against the station's tracked carrier (the data pulse of a
//!    9th-pulse scheme is added at its nominal place with its phase code). Pulses whose
//!    possible shift exceeds [`CARRIER_ONLY_MAX_SHIFT_US`] get envelope-resolved timing
//!    (their own envelope searched over the shift range plus [`ENVELOPE_MARGIN_US`]), so
//!    PPM beyond ± half a carrier cycle decodes. [`LdcDecoder::decide`] removes the
//!    common offset (unmodulated pulses) and decides the channel symbol.
//! 2. **Per station**: the symbols of whole frames inside the valid part of the recording
//!    (frames are aligned to the Loran epoch: group `g` is symbol `g mod G` of frame
//!    `⌊g / G⌋`) go to [`LdcDecoder::decode_symbols`] (RS decoding with corrected-symbol
//!    counts, frame CRC, message CRC-24Q) and [`LdcDecoder::verify_auth`].
//!
//! The result per station is a [`StationLdc`] in [`super::RxOutput::ldc`].

use super::{ExtraPulse, Receiver, StationTrack};
use crate::ldc::mapper::{DATA_PULSE, LDC_PULSES};
use crate::ldc::msg::Message;
use crate::ldc::rx::{AuthCheck, DecodeReport, DecodedMessage, LdcDecoder};
use crate::signal::Parity;

/// Largest pulse shift (µs) decided from the carrier phase alone (unambiguous to ± 5 µs;
/// margin for noise). Larger shifts use envelope-resolved timing.
pub const CARRIER_ONLY_MAX_SHIFT_US: f64 = 3.0;
/// Search margin (µs) beyond the largest shift for envelope-resolved timing.
pub const ENVELOPE_MARGIN_US: f64 = 4.0;
/// Pulses read weaker than this fraction of the tracked amplitude count as not measured.
const MIN_AMPLITUDE_REL: f64 = 0.2;

/// The decoded data channel of one tracked station (and loop channel).
#[derive(Clone, Debug, PartialEq)]
pub struct StationLdc {
    pub chain_index: usize,
    pub station_index: usize,
    pub chain: String,
    pub station: String,
    /// Loop channel for per-loop processing; `None` for E-field or beam-steered input.
    pub channel: Option<usize>,
    /// LDC scheme name.
    pub scheme: String,
    /// Group number (since the Loran epoch) of `symbols[0]`, a frame boundary.
    pub first_group: i64,
    /// Absolute frame number of `symbols[0]` (`first_group / groups per frame`).
    pub first_frame: i64,
    /// Decided channel symbols, one per group, whole frames.
    pub symbols: Vec<u32>,
    /// Groups whose pulse timing could not be measured (symbol 0 used).
    pub groups_missing: usize,
    /// Frames (RS/CRC status, corrected symbols) and messages (with corrected-symbol
    /// counts), message CRC failures.
    pub report: DecodeReport,
    /// Authentication check of every decoded auth message.
    pub auth: Vec<AuthCheck>,
}

impl StationLdc {
    /// The decoded messages, in order.
    pub fn messages(&self) -> impl Iterator<Item = &Message> {
        self.report.messages.iter().map(|m| &m.message)
    }
    /// Absolute frame number of a decoded message's first frame.
    pub fn message_frame(&self, m: &DecodedMessage) -> i64 {
        self.first_frame + m.first_frame as i64
    }
    /// RS symbol errors corrected over all frames.
    pub fn symbols_corrected(&self) -> usize {
        self.report.symbols_corrected
    }
}

impl Receiver {
    /// The channel symbol of `group` of a tracked station, decided by `dec`; `None` when
    /// the group's pulse timing cannot be measured.
    pub fn ldc_symbol(&self, dec: &LdcDecoder, track: &StationTrack, group: i64) -> Option<u32> {
        let ch = &self.chains[track.chain_index];
        let role = ch.stations[track.station_index].role;
        let parity = Parity::of_group(group);
        let nominal = ch.signal.group(role, parity);
        let n_nom = nominal.len();
        let mut extra = Vec::new();
        if let (Some(dp), Some(p8)) = (dec.data_pulse, nominal.iter().find(|p| p.k == 7)) {
            let code = match parity {
                Parity::A => dp.code_a,
                Parity::B => dp.code_b,
            };
            extra.push(ExtraPulse {
                offset_us: p8.offset.as_us() + dp.after(role),
                phase_rad: if code < 0 { std::f64::consts::PI } else { 0.0 },
            });
        }
        let shift = dec.max_shift_us();
        let ldc_index = |k: usize| if k < n_nom { (nominal[k].k < 8).then_some(nominal[k].k) } else { Some(DATA_PULSE) };
        let search: Vec<f64> = (0..n_nom + extra.len())
            .map(|k| match ldc_index(k).map(|i| shift[i]) {
                Some(s) if s > CARRIER_ONLY_MAX_SHIFT_US => s + ENVELOPE_MARGIN_US,
                _ => 0.0,
            })
            .collect();
        let pt = self.pulse_timing_with(track, group, &extra, &search)?;
        let mut offsets = [f64::NAN; LDC_PULSES];
        let mut phases = [f64::NAN; LDC_PULSES];
        for p in &pt.pulses {
            let Some(i) = ldc_index(p.k) else { continue };
            if p.amplitude_rel >= MIN_AMPLITUDE_REL && p.offset_ns.is_finite() {
                offsets[i] = p.offset_ns;
                phases[i] = p.phase_rad;
            }
        }
        if offsets.iter().all(|x| x.is_nan()) {
            return None;
        }
        Some(dec.decide(&offsets, Some(&phases)))
    }

    /// Decode the data channel of one tracked station with `dec`: whole frames inside the
    /// valid sample range. `None` if the station carries no data or was not tracked.
    pub fn decode_ldc_track(&self, dec: &LdcDecoder, track: &StationTrack) -> Option<StationLdc> {
        if !dec.modulates(track.station_index) || track.windows.is_empty() {
            return None;
        }
        let ch = &self.chains[track.chain_index];
        let st = &ch.stations[track.station_index];
        let gri = ch.gri_us as f64 * 1e-6;
        let g_per_f = dec.groups_per_frame() as i64;
        // extent of a group around its carrier origin, data pulse and searches included
        let max_shift = dec.max_shift_us().iter().cloned().fold(0.0, f64::max) + ENVELOPE_MARGIN_US;
        let nominal = ch.signal.group(st.role, Parity::A);
        let p8_us = nominal.iter().find(|p| p.k == 7).map(|p| p.offset.as_us()).unwrap_or(0.0);
        let last_offset_us = nominal
            .iter()
            .map(|p| p.offset.as_us())
            .fold(0.0, f64::max)
            .max(dec.data_pulse.map(|d| p8_us + d.after(st.role)).unwrap_or(0.0));
        let lo = self.n_valid.0 as f64 / self.fs - (self.cfg.fit_window_us.0 - max_shift) * 1e-6;
        let hi = self.n_valid.1 as f64 / self.fs - (last_offset_us + self.cfg.fit_window_us.1 + max_shift) * 1e-6;
        let valid = |g: i64| {
            let w = track.windows.iter().min_by_key(|w| (w.group - g).abs()).expect("non-empty");
            let t = w.fit.t_car + (g - w.group) as f64 * gri;
            t >= lo && t <= hi
        };
        let span = self.cfg.track_gris as i64 + 1;
        let (first, last) = (track.windows[0].group, track.windows[track.windows.len() - 1].group);
        let mut ga = first - span;
        while ga <= last + span && !valid(ga) {
            ga += 1;
        }
        let mut gb = last + span;
        while gb >= ga && !valid(gb) {
            gb -= 1;
        }
        let first_frame = -((-ga).div_euclid(g_per_f)); // ⌈ga / G⌉
        let first_group = first_frame * g_per_f;
        let frames = if gb >= first_group { (gb + 1 - first_group) / g_per_f } else { 0 };
        let mut groups_missing = 0;
        let symbols: Vec<u32> = (first_group..first_group + frames * g_per_f)
            .map(|g| {
                self.ldc_symbol(dec, track, g).unwrap_or_else(|| {
                    groups_missing += 1;
                    0
                })
            })
            .collect();
        let report = dec.decode_symbols(&symbols);
        let auth = dec.verify_auth(track.station_index, &report, first_frame);
        Some(StationLdc {
            chain_index: track.chain_index,
            station_index: track.station_index,
            chain: ch.name.clone(),
            station: st.id.clone(),
            channel: track.channel,
            scheme: dec.name.clone(),
            first_group,
            first_frame,
            symbols,
            groups_missing,
            report,
            auth,
        })
    }

    /// Decode the data channel of every tracked station of every chain with a decoder.
    pub fn decode_ldc(&self, tracks: &[StationTrack]) -> Vec<StationLdc> {
        tracks
            .iter()
            .filter_map(|t| {
                let dec = self.ldc_decoders.get(t.chain_index)?.as_ref()?;
                self.decode_ldc_track(dec, t)
            })
            .collect()
    }
}
