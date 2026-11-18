// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Software eLoran receiver (stream S10): acquisition, phase decoding, cycle
//! identification, ToA/ECD/SNR, data-channel hooks, position and time solutions.
//!
//! The receiver processes real samples offline (simulated, recorded or replayed): per
//! channel `Vec<f64>`s or an [`crate::io::Recording`], with the sample rate, the scene
//! start ([`AbsTime`], which places the GRI epochs) and the configured chains. Sample `n`
//! is taken at scene time `n / fs`, so every time it reports is on the receiver's sample
//! clock.
//!
//! # Processing chain
//!
//! 1. **Front end** ([`dsp`]): 90–110 kHz linear-phase windowed-sinc band-pass, applied
//!    zero-phase by FFT overlap-save (the FIR's `M`-sample group delay removed exactly).
//! 2. **Acquisition** ([`acquire`]): per chain, 2-GRI phase-code-cycle folding of the
//!    complex baseband and correlation with each role's pulse-group template; coarse group
//!    arrival per station (10 µs bins) and the bearing for crossed loops.
//! 3. **Cross-rate mitigation**: once every chain is acquired, the receiver knows where
//!    every other chain's pulses arrive; samples within `guard_us` of those pulses
//!    (stronger than `min_rel_db` below the tracked station) are **blanked** in tracking.
//!    Phase-code averaging over N GRIs further reduces what is left.
//! 4. **Tracking** ([`track`]): per station and window of `track_gris` GRIs, a joint
//!    least-squares fit of the phase-decoded pulses gives the envelope time (matched
//!    envelope), the carrier phase, cycle identification, ToA, ECD, SNR in 20 kHz and
//!    signal strength; cycle-slip flags from carrier continuity between windows.
//! 5. **Antennas**: one E-field channel; or two crossed H loops, combined per station by
//!    beam steering (relative bearing `θ` from the loops' complex amplitudes, input
//!    `cos θ·A + sin θ·B`, 180° ambiguity resolved by half-cycle identification) or
//!    processed per loop.
//! 6. **Solutions** ([`solve`]): time solution at a known position, range-domain
//!    position + clock (all-in-view, multi-chain), TDoA fixes.
//! 7. **Truth comparison** ([`truth`]) and the **LDC hook** ([`Receiver::pulse_timing`]):
//!    per-pulse timing offsets and phases of a group for a data-channel decoder.
//! 8. **Data-channel decoding** ([`ldc`]): with [`RxConfig::ldc`] set (the chains' LDC
//!    configurations), every tracked station's channel symbols are decided from
//!    [`Receiver::pulse_timing_with`] and decoded (RS/CRC/messages/authentication) into
//!    [`RxOutput::ldc`].
//!
//! # References
//!
//! * S. Lo, B. Peterson, P. Enge et al., eLoran receiver design and performance
//!   (Stanford GPS Lab / FAA Loran Evaluation papers, ION, 2006–2009).
//! * U.S. Coast Guard, *Loran-C User Handbook*, COMDTPUB P16562.6 (1992): standard
//!   sampling point, cycle identification and ECD.
//! * U.S. Coast Guard, *Specification of the Transmitted LORAN-C Signal*,
//!   COMDTINST M16562.4A.
//!
//! # Limitations
//!
//! * Block (batch) processing, not a closed-loop real-time tracker; each window assumes a
//!   constant arrival time (fine for static and ship-speed receivers over a few GRIs).
//! * White-noise estimator: no impulsive-noise clipping, no CW notch filters, no skywave
//!   rejection beyond the configurable fit window (the default window ends 70 µs after
//!   the envelope origin, so skywave delayed by less than that biases the estimates).
//! * Acquisition is not repeated if a station appears later (e.g. after an off fault);
//!   blanking uses the coarse acquisition arrivals (adequate while the receiver moves less
//!   than the guard over the recording).
//! * Cross-chain stations sharing exact pulse positions for the whole recording (an exact
//!   GRI coincidence) can only be separated by the phase codes.

pub mod acquire;
pub mod dsp;
pub mod ldc;
pub mod solve;
pub mod track;
pub mod truth;

pub use acquire::Acquisition;
pub use dsp::{BandPassSpec, Fir, C64};
pub use ldc::StationLdc;
pub use solve::{
    nominal_emission_s, observations, position_solution, predicted_delay_s, tdoa_fix,
    time_solution, Corrections, Observation, PositionConfig, PositionSolution, StationClock, TdFix,
    TimeDifference, TimeSolution,
};
pub use truth::{clock_errors, position_error_m, toa_errors, ClockError, StationErrors};

use crate::chain::{Chain, StationStatus};
use crate::geo::Geo;
use crate::ldc::{LdcConfig, LdcDecoder};
use crate::signal::Parity;
use crate::time::AbsTime;
use track::{Blanking, PulseRef, Src, Template, WindowFit};

/// A processing unit of a station: (loop channel, channel weights, polarity known, steer).
type Unit = (Option<usize>, Vec<(usize, f64)>, bool, bool);

/// Antenna input arrangement.
#[derive(Clone, Debug, PartialEq)]
pub enum Antenna {
    /// One vertical E-field channel (polarity known).
    EField { channel: usize },
    /// Two crossed H-field loops (`a` on the bow axis, `b` 90° to starboard).
    CrossedLoop { a: usize, b: usize, mode: LoopMode },
}

/// How crossed loops are processed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    /// Per station, steer the beam to the estimated bearing (one measurement per station).
    BeamSteer,
    /// Each loop on its own (one measurement per station per loop).
    PerLoop,
}

/// Cross-rate interference mitigation.
#[derive(Clone, Debug, PartialEq)]
pub struct CriMitigation {
    /// Blank samples overlapping other chains' pulses.
    pub blank: bool,
    /// Guard before and after an interfering pulse (µs).
    pub guard_us: f64,
    /// Blank only interferers stronger than (tracked station + `min_rel_db`) dB.
    pub min_rel_db: f64,
}

impl Default for CriMitigation {
    fn default() -> Self {
        CriMitigation {
            blank: true,
            guard_us: 50.0,
            min_rel_db: -20.0,
        }
    }
}

/// Receiver configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct RxConfig {
    pub band: BandPassSpec,
    pub antenna: Antenna,
    /// GRIs integrated per tracking window (an even number covers whole phase-code cycles).
    pub track_gris: usize,
    /// Fit window relative to each pulse's envelope origin (µs).
    pub fit_window_us: (f64, f64),
    /// Envelope search half-range in the first window (µs) and in later windows.
    pub envelope_search_us: f64,
    pub track_search_us: f64,
    /// Acquisition search half-width around a predicted arrival (µs) when an approximate
    /// position is known.
    pub acq_search_halfwidth_us: f64,
    /// Acquisition detection threshold (peak to median, dB).
    pub acq_threshold_db: f64,
    pub cri: CriMitigation,
    /// Approximate receiver position (narrows acquisition).
    pub approx_position: Option<Geo>,
    /// Propagation corrections used by acquisition predictions (and available to the
    /// solutions).
    pub corrections: Corrections,
    /// Expected propagation-induced ECD (µs) added to each station's transmitted ECD for
    /// cycle identification.
    pub propagation_ecd_us: f64,
    /// Field (µV/m) of input value 1.0 (the simulator's `output_scale_uv_m`) for the
    /// reported field strength.
    pub output_scale_uv_m: f64,
    /// Data-channel decoding: the LDC configuration of each chain, by chain index (`None`,
    /// or a missing entry, for no decoding; empty by default). Normally
    /// `Scenario::ldc_configs()`; each entry is completed from its chain
    /// ([`LdcConfig::for_chain`]). Decoded messages appear in [`RxOutput::ldc`].
    pub ldc: Vec<Option<LdcConfig>>,
}

impl Default for RxConfig {
    fn default() -> Self {
        RxConfig {
            band: BandPassSpec::default(),
            antenna: Antenna::EField { channel: 0 },
            track_gris: 4,
            fit_window_us: (-20.0, 70.0),
            envelope_search_us: 25.0,
            track_search_us: 3.0,
            acq_search_halfwidth_us: 200.0,
            acq_threshold_db: 10.0,
            cri: CriMitigation::default(),
            approx_position: None,
            corrections: Corrections::default(),
            propagation_ecd_us: 0.0,
            output_scale_uv_m: 1.0,
            ldc: Vec::new(),
        }
    }
}

/// Receiver input: real samples per channel, sample rate and scene start.
#[derive(Clone, Debug)]
pub struct RxInput {
    pub channels: Vec<Vec<f64>>,
    pub fs_hz: f64,
    pub start: AbsTime,
}

impl RxInput {
    pub fn new(channels: Vec<Vec<f64>>, fs_hz: f64, start: AbsTime) -> RxInput {
        RxInput {
            channels,
            fs_hz,
            start,
        }
    }
    /// From a SigMF recording (the scene start is not in the file; pass it).
    pub fn from_recording(rec: &crate::io::Recording, start: AbsTime) -> RxInput {
        RxInput {
            channels: rec.channels.clone(),
            fs_hz: rec.fs_hz,
            start,
        }
    }
}

/// One station measurement (one tracking window).
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    pub chain_index: usize,
    pub station_index: usize,
    pub chain: String,
    pub station: String,
    /// Loop channel for per-loop processing; `None` for E-field or beam-steered input.
    pub channel: Option<usize>,
    /// Reference group (the middle group of the window) number since the Loran epoch.
    pub group: i64,
    /// Time of arrival of the reference group's pulse-0 time origin (scene s), from the
    /// identified carrier cycle (standard zero crossing − 30 µs).
    pub toa_s: f64,
    /// Envelope origin (scene s).
    pub envelope_toa_s: f64,
    /// ECD = envelope − carrier (µs).
    pub ecd_us: f64,
    /// SNR in 20 kHz (dB): RMS signal at the pulse peak over RMS noise in 20 kHz.
    pub snr_db: f64,
    /// RMS signal at the pulse peak (input units) and as a field (µV/m).
    pub signal_rms: f64,
    pub field_uv_m: f64,
    /// Carrier continuity disagreed with the envelope's cycle choice.
    pub cycle_slip: bool,
    /// Bearing relative to loop A's axis (deg, 0..360), beam-steered loops only.
    pub bearing_deg: Option<f64>,
    pub pulses: usize,
    pub samples: usize,
}

/// One tracking window of a station, kept for the LDC hook.
#[derive(Clone, Debug)]
pub struct TrackWindow {
    pub group: i64,
    pub fit: WindowFit,
    /// Channel weights used (after polarity resolution).
    pub weights: Vec<(usize, f64)>,
}

/// All windows of one station (and channel).
#[derive(Clone, Debug)]
pub struct StationTrack {
    pub chain_index: usize,
    pub station_index: usize,
    pub channel: Option<usize>,
    pub windows: Vec<TrackWindow>,
}

/// Receiver output.
#[derive(Clone, Debug)]
pub struct RxOutput {
    pub acquisitions: Vec<Acquisition>,
    pub tracks: Vec<StationTrack>,
    pub measurements: Vec<Measurement>,
    /// Decoded data channel per tracked station (and loop channel) of every chain with an
    /// LDC configuration in [`RxConfig::ldc`] (empty otherwise).
    pub ldc: Vec<StationLdc>,
}

/// Per-pulse timing of one group (LDC decoder input).
#[derive(Clone, Debug, PartialEq)]
pub struct PulseTiming {
    /// Pulse index in the group (nominal pulses first, then the extra offsets).
    pub k: usize,
    /// Nominal offset from the group's time origin (µs).
    pub nominal_offset_us: f64,
    /// Timing offset of the pulse relative to nominal (ns, carrier-phase based, within
    /// ± half a carrier period; positive = late).
    pub offset_ns: f64,
    /// Carrier phase relative to the expected (code-decoded) phase (rad, (−π, π]); a
    /// phase-modulated pulse reads ≈ ±π.
    pub phase_rad: f64,
    /// Amplitude relative to the station's tracked amplitude.
    pub amplitude_rel: f64,
}

/// A pulse a data channel inserts into a group (input of [`Receiver::pulse_timing_with`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtraPulse {
    /// Nominal offset from the group's time origin (µs).
    pub offset_us: f64,
    /// Expected carrier phase (rad): the pulse's phase code (0 or π).
    pub phase_rad: f64,
}

/// Per-pulse timing of a station's group.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupPulseTiming {
    pub chain: String,
    pub station: String,
    pub group: i64,
    /// Group time origin used as the reference (scene s).
    pub toa_s: f64,
    pub pulses: Vec<PulseTiming>,
}

/// The receiver: front-end output plus configuration; run [`Receiver::run`].
pub struct Receiver {
    pub cfg: RxConfig,
    pub chains: Vec<Chain>,
    pub fs: f64,
    pub start: AbsTime,
    pub fir: Fir,
    /// Band-pass filtered channels (zero phase).
    pub filtered: Vec<Vec<f64>>,
    pub templates: Vec<Template>,
    /// Valid sample range (filter edge transients excluded).
    pub n_valid: (usize, usize),
    /// LDC decoder per chain (from [`RxConfig::ldc`]).
    pub ldc_decoders: Vec<Option<LdcDecoder>>,
}

impl Receiver {
    /// Design the front end and filter the input.
    pub fn new(input: &RxInput, chains: &[Chain], cfg: RxConfig) -> Result<Receiver, String> {
        if input.channels.is_empty() {
            return Err("receiver: no input channels".into());
        }
        let need = match &cfg.antenna {
            Antenna::EField { channel } => *channel,
            Antenna::CrossedLoop { a, b, .. } => (*a).max(*b),
        };
        if need >= input.channels.len() {
            return Err(format!(
                "receiver: antenna uses channel {need}, input has {}",
                input.channels.len()
            ));
        }
        if cfg.track_gris == 0 {
            return Err("receiver: track_gris must be ≥ 1".into());
        }
        for c in chains {
            c.validate()?;
        }
        let fir = Fir::design(cfg.band, input.fs_hz)?;
        let refs: Vec<&[f64]> = input.channels.iter().map(|c| c.as_slice()).collect();
        let filtered = fir.filter_many(&refs);
        let len = input.channels.iter().map(|c| c.len()).min().unwrap_or(0);
        let n_valid = (fir.half.min(len), len.saturating_sub(fir.half));
        let templates = chains
            .iter()
            .map(|c| Template::new(&c.signal, &fir))
            .collect();
        if cfg.ldc.len() > chains.len() {
            return Err(format!(
                "receiver: {} LDC configurations for {} chains",
                cfg.ldc.len(),
                chains.len()
            ));
        }
        let ldc_decoders = chains
            .iter()
            .enumerate()
            .map(|(i, c)| match cfg.ldc.get(i).and_then(|l| l.as_ref()) {
                Some(l) => LdcDecoder::for_chain(l, c)
                    .map_err(|e| format!("receiver: chain {}: {e}", c.name)),
                None => Ok(None),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Receiver {
            ldc_decoders,
            cfg,
            chains: chains.to_vec(),
            fs: input.fs_hz,
            start: input.start,
            fir,
            filtered,
            templates,
            n_valid,
        })
    }

    fn antenna_channels(&self) -> Vec<usize> {
        match &self.cfg.antenna {
            Antenna::EField { channel } => vec![*channel],
            Antenna::CrossedLoop { a, b, .. } => vec![*a, *b],
        }
    }

    /// Acquire every station of every chain.
    pub fn acquire(&self) -> Vec<Acquisition> {
        let ch = self.antenna_channels();
        self.chains
            .iter()
            .enumerate()
            .flat_map(|(ci, c)| {
                acquire::acquire_chain(
                    &self.filtered,
                    &ch,
                    self.fs,
                    self.n_valid,
                    self.start,
                    c,
                    ci,
                    &self.cfg,
                )
            })
            .collect()
    }

    /// Blanking intervals for a station: other chains' acquired pulses.
    fn blanking_for(&self, acq: &[Acquisition], me: &Acquisition) -> Blanking {
        if !self.cfg.cri.blank {
            return Blanking::default();
        }
        let guard = self.cfg.cri.guard_us * 1e-6;
        let thr = me.amplitude * 10f64.powf(self.cfg.cri.min_rel_db / 20.0);
        let t_end = self.n_valid.1 as f64 / self.fs;
        let mut iv = Vec::new();
        for a in acq
            .iter()
            .filter(|a| a.found && a.chain_index != me.chain_index && a.amplitude >= thr)
        {
            let ch = &self.chains[a.chain_index];
            let st = &ch.stations[a.station_index];
            let gri = ch.gri_us as f64 * 1e-6;
            let ext = ch.signal.envelope.extent_s();
            let offs: Vec<f64> = ch
                .signal
                .group(st.role, Parity::A)
                .iter()
                .map(|p| p.offset.as_s())
                .collect();
            let j0 = ((-a.toa0_s - 0.02) / gri).floor() as i64;
            let j1 = ((t_end - a.toa0_s) / gri).ceil() as i64;
            for j in j0..=j1 {
                let g = a.toa0_s + j as f64 * gri;
                for o in &offs {
                    iv.push((g + o - guard, g + o + ext + guard));
                }
            }
        }
        Blanking::new(iv)
    }

    /// Track every acquired station.
    pub fn track(&self, acq: &[Acquisition]) -> (Vec<StationTrack>, Vec<Measurement>) {
        let mut tracks = Vec::new();
        let mut meas = Vec::new();
        for a in acq.iter().filter(|a| a.found) {
            let ch = &self.chains[a.chain_index];
            if ch.stations[a.station_index].status == StationStatus::Off {
                continue;
            }
            let blank = self.blanking_for(acq, a);
            let units: Vec<Unit> = match &self.cfg.antenna {
                Antenna::EField { channel } => vec![(None, vec![(*channel, 1.0)], true, false)],
                Antenna::CrossedLoop {
                    a: la,
                    b: lb,
                    mode: LoopMode::BeamSteer,
                } => {
                    let th = a.bearing_rad.unwrap_or(0.0);
                    vec![(None, vec![(*la, th.cos()), (*lb, th.sin())], false, true)]
                }
                Antenna::CrossedLoop {
                    a: la,
                    b: lb,
                    mode: LoopMode::PerLoop,
                } => vec![
                    (Some(*la), vec![(*la, 1.0)], false, false),
                    (Some(*lb), vec![(*lb, 1.0)], false, false),
                ],
            };
            for (channel, weights, pol_known, steer) in units {
                let (t, m) = self.track_unit(a, &blank, channel, weights, pol_known, steer);
                tracks.push(t);
                meas.extend(m);
            }
        }
        (tracks, meas)
    }

    fn pulses_for(
        &self,
        ci: usize,
        si: usize,
        groups: std::ops::Range<i64>,
        g_ref: i64,
    ) -> Vec<PulseRef> {
        let ch = &self.chains[ci];
        let st = &ch.stations[si];
        let gri = ch.gri_us as f64 * 1e-6;
        let mut v = Vec::new();
        for g in groups {
            for p in ch.signal.group(st.role, Parity::of_group(g)) {
                v.push(PulseRef {
                    d_s: (g - g_ref) as f64 * gri + p.offset.as_s(),
                    phase_rad: p.phase_rad,
                });
            }
        }
        v
    }

    /// Steering angle from the per-loop least-squares amplitudes at envelope time `t_env`.
    fn steer_angle(
        &self,
        tpl: &Template,
        loops: (usize, usize),
        pulses: &[PulseRef],
        t_env: f64,
        blank: &Blanking,
    ) -> Option<f64> {
        let w = self.window_s();
        let fit = |c: usize| {
            let src = Src {
                chans: &self.filtered,
                weights: vec![(c, 1.0)],
                fs: self.fs,
                n_lo: self.n_valid.0,
                n_hi: self.n_valid.1,
            };
            let d = track::collect(&src, tpl, pulses, t_env, w, blank);
            track::fit_w(&d, tpl, 0.0).1
        };
        let (wa, wb) = (fit(loops.0), fit(loops.1));
        if wa.abs() == 0.0 && wb.abs() == 0.0 {
            return None;
        }
        Some(0.5 * (2.0 * (wa * wb.conj()).re).atan2(wa.norm_sqr() - wb.norm_sqr()))
    }

    fn window_s(&self) -> (f64, f64) {
        (
            self.cfg.fit_window_us.0 * 1e-6,
            self.cfg.fit_window_us.1 * 1e-6,
        )
    }

    fn track_unit(
        &self,
        a: &Acquisition,
        blank: &Blanking,
        channel: Option<usize>,
        mut weights: Vec<(usize, f64)>,
        polarity_known: bool,
        steer: bool,
    ) -> (StationTrack, Vec<Measurement>) {
        let (ci, si) = (a.chain_index, a.station_index);
        let ch = &self.chains[ci];
        let st = &ch.stations[si];
        let tpl = &self.templates[ci];
        let gri = ch.gri_us as f64 * 1e-6;
        let span = ch.signal.group_span_s(st.role);
        let ecd_exp = (st.ecd_us + self.cfg.propagation_ecd_us) * 1e-6;
        let (t_lo, t_hi) = (
            self.n_valid.0 as f64 / self.fs,
            self.n_valid.1 as f64 / self.fs,
        );
        let j_lo = ((t_lo - a.toa0_s + 30e-6) / gri).ceil() as i64;
        let j_hi = ((t_hi - a.toa0_s - span) / gri).floor() as i64;
        let n = self.cfg.track_gris as i64;
        let mut track = StationTrack {
            chain_index: ci,
            station_index: si,
            channel,
            windows: vec![],
        };
        let mut meas = Vec::new();
        let mut prev: Option<(i64, WindowFit)> = None;
        let w = self.window_s();
        let mut j = j_lo;
        while j <= j_hi {
            let len = n.min(j_hi - j + 1);
            if len < (n + 1) / 2 {
                break;
            }
            let g0 = a.group0 + j;
            let g_ref = g0 + len / 2;
            let pulses = self.pulses_for(ci, si, g0..g0 + len, g_ref);
            let (anchor, search) = match &prev {
                Some((pg, pf)) => (
                    pf.t_env + (g_ref - pg) as f64 * gri,
                    self.cfg.track_search_us * 1e-6,
                ),
                None => (
                    a.toa0_s + (g_ref - a.group0) as f64 * gri + ecd_exp,
                    self.cfg.envelope_search_us * 1e-6,
                ),
            };
            let fit_once = |weights: &Vec<(usize, f64)>, anchor: f64, search: f64| {
                let src = Src {
                    chans: &self.filtered,
                    weights: weights.clone(),
                    fs: self.fs,
                    n_lo: self.n_valid.0,
                    n_hi: self.n_valid.1,
                };
                track::fit_window(
                    &src,
                    tpl,
                    &pulses,
                    anchor,
                    search,
                    w,
                    blank,
                    ecd_exp,
                    polarity_known,
                )
            };
            let mut fit = None;
            if steer {
                let lp = (weights[0].0, weights[1].0);
                if let Some(th) = self.steer_angle(tpl, lp, &pulses, anchor, blank) {
                    weights = vec![(lp.0, th.cos()), (lp.1, th.sin())];
                }
                if let Some(f) = fit_once(&weights, anchor, search) {
                    if let Some(th) = self.steer_angle(tpl, lp, &pulses, f.t_env, blank) {
                        weights = vec![(lp.0, th.cos()), (lp.1, th.sin())];
                    }
                    fit = fit_once(&weights, f.t_env, 2e-6);
                }
            } else {
                fit = fit_once(&weights, anchor, search);
            }
            let Some(fit) = fit else {
                j += len;
                continue;
            };
            if fit.flipped {
                for wt in weights.iter_mut() {
                    wt.1 = -wt.1;
                }
            }
            // cycle-slip check: carrier continuity vs the envelope's choice
            let cycle_slip = match &prev {
                Some((pg, pf)) => {
                    let dt = (g_ref - pg) as f64 * gri;
                    let pred = pf.t_car + dt + (fit.t_env - (pf.t_env + dt));
                    let (cand, _) = track::select_cycle(fit.w, tpl.omega, pred, true);
                    (cand - fit.t_car).abs() > tpl.period() / 4.0
                }
                None => false,
            };
            let r = fit.w.abs();
            let wsum2: f64 = weights.iter().map(|(_, x)| x * x).sum();
            let sigma20 = (fit.res_var * 40_000.0 / (self.fs * self.fir.noise_gain * wsum2)).sqrt();
            let srms = r / std::f64::consts::SQRT_2;
            let bearing_deg = if steer {
                Some(
                    weights[1]
                        .1
                        .atan2(weights[0].1)
                        .to_degrees()
                        .rem_euclid(360.0),
                )
            } else {
                None
            };
            meas.push(Measurement {
                chain_index: ci,
                station_index: si,
                chain: ch.name.clone(),
                station: st.id.clone(),
                channel,
                group: g_ref,
                toa_s: fit.t_car,
                envelope_toa_s: fit.t_env,
                ecd_us: (fit.t_env - fit.t_car) * 1e6,
                snr_db: if sigma20 > 0.0 {
                    20.0 * (srms / sigma20).log10()
                } else {
                    f64::INFINITY
                },
                signal_rms: srms,
                field_uv_m: srms * self.cfg.output_scale_uv_m,
                cycle_slip,
                bearing_deg,
                pulses: fit.pulses,
                samples: fit.samples,
            });
            track.windows.push(TrackWindow {
                group: g_ref,
                fit,
                weights: weights.clone(),
            });
            prev = Some((g_ref, fit));
            j += len;
        }
        (track, meas)
    }

    /// Full run: acquisition, tracking, measurements.
    pub fn run(&self) -> RxOutput {
        let acquisitions = self.acquire();
        let (tracks, measurements) = self.track(&acquisitions);
        let ldc = self.decode_ldc(&tracks);
        RxOutput {
            acquisitions,
            tracks,
            measurements,
            ldc,
        }
    }

    /// **LDC hook**: timing of every pulse of `group` of a tracked station, relative to its
    /// nominal place in the group. The station's tracked envelope and carrier (of the
    /// window nearest `group`) are the reference; each pulse's carrier phase is fitted on
    /// its own with the envelope fixed. `extra_offsets_us` adds pulses a data channel
    /// inserts (e.g. a ninth secondary pulse) with phase 0. See
    /// [`Receiver::pulse_timing_with`] for inserted pulses with a phase code and for
    /// shifts beyond ± half a carrier cycle.
    pub fn pulse_timing(
        &self,
        track: &StationTrack,
        group: i64,
        extra_offsets_us: &[f64],
    ) -> Option<GroupPulseTiming> {
        let extra: Vec<ExtraPulse> = extra_offsets_us
            .iter()
            .map(|o| ExtraPulse {
                offset_us: *o,
                phase_rad: 0.0,
            })
            .collect();
        self.pulse_timing_with(track, group, &extra, &[])
    }

    /// [`Receiver::pulse_timing`] with inserted pulses given with their carrier phase (a
    /// data pulse's phase code; without it a −1-coded pulse would read half a cycle off),
    /// and per-pulse **envelope-resolved** timing: for pulse `k` (index into the returned
    /// list: the group's nominal pulses, then `extra`) with `envelope_search_us[k] > 0`,
    /// the pulse's own envelope is searched over ± that range around its nominal place
    /// and the carrier cycle nearest the envelope is taken, so the offset is no longer
    /// limited to ± half a carrier period (5 µs). Needed by PPM schemes whose shifts
    /// exceed ±5 µs (e.g. the 9th-pulse LDC: 32 states 1 µs apart, ±15.5 µs). The single
    /// pulse envelope must be good to ±5 µs for the cycle to be right.
    pub fn pulse_timing_with(
        &self,
        track: &StationTrack,
        group: i64,
        extra: &[ExtraPulse],
        envelope_search_us: &[f64],
    ) -> Option<GroupPulseTiming> {
        let win = track
            .windows
            .iter()
            .min_by_key(|w| (w.group - group).abs())?;
        let ch = &self.chains[track.chain_index];
        let st = &ch.stations[track.station_index];
        let tpl = &self.templates[track.chain_index];
        let gri = ch.gri_us as f64 * 1e-6;
        let shift = (group - win.group) as f64 * gri;
        let t_car = win.fit.t_car + shift;
        let t_env = win.fit.t_env + shift;
        let src = Src {
            chans: &self.filtered,
            weights: win.weights.clone(),
            fs: self.fs,
            n_lo: self.n_valid.0,
            n_hi: self.n_valid.1,
        };
        let blank = Blanking::default();
        let mut specs: Vec<(f64, f64)> = ch
            .signal
            .group(st.role, Parity::of_group(group))
            .iter()
            .map(|p| (p.offset.as_s(), p.phase_rad))
            .collect();
        specs.extend(extra.iter().map(|e| (e.offset_us * 1e-6, e.phase_rad)));
        let r = win.fit.w.abs();
        let period = tpl.period();
        let ws = self.window_s();
        let pulses = specs
            .iter()
            .enumerate()
            .map(|(k, (off, ph))| {
                let pr = [PulseRef {
                    d_s: *off,
                    phase_rad: *ph,
                }];
                let search = envelope_search_us.get(k).copied().unwrap_or(0.0).max(0.0) * 1e-6;
                // envelope of this pulse alone (wide timing), else the station's envelope
                let delta = if search > 0.0 {
                    let d0 = track::collect(
                        &src,
                        tpl,
                        &pr,
                        t_env,
                        (ws.0 - search, ws.1 + search),
                        &blank,
                    );
                    track::search_envelope(&d0, tpl, -search, search)
                } else {
                    0.0
                };
                let d = track::collect(&src, tpl, &pr, t_env + delta, ws, &blank);
                let (_, wi) = track::fit_w(&d, tpl, 0.0);
                let ti = -wi.arg() / tpl.omega;
                let kk = ((t_car + delta - ti) / period).round();
                let off_s = ti + kk * period - t_car;
                let rel = wi * C64::cis(tpl.omega * t_car);
                PulseTiming {
                    k,
                    nominal_offset_us: off * 1e6,
                    offset_ns: off_s * 1e9,
                    phase_rad: rel.arg(),
                    amplitude_rel: if r > 0.0 { wi.abs() / r } else { 0.0 },
                }
            })
            .collect();
        Some(GroupPulseTiming {
            chain: ch.name.clone(),
            station: st.id.clone(),
            group,
            toa_s: t_car,
            pulses,
        })
    }
}

/// Convenience: build the receiver and run it.
pub fn process(
    input: &RxInput,
    chains: &[Chain],
    cfg: RxConfig,
) -> Result<(Receiver, RxOutput), String> {
    let rx = Receiver::new(input, chains, cfg)?;
    let out = rx.run();
    Ok((rx, out))
}
