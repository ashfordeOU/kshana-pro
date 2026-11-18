// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! eLoran threat models for resilience testing (stream F4).
//!
//! # Licence gating (IMPORTANT)
//!
//! These are **synthetic** signal and data-channel threat models for red-team / resilience
//! evaluation of eLoran receivers and timing users. They are **product-level
//! licence-gated**: kshana-pro MUST expose them only as an **Enterprise "vetted buyer"**
//! capability, exactly as it gates its `attack-iq` feature — the same entitlement check, the
//! same buyer vetting, the same audit logging. They are for **files / conducted** use only
//! (recordings and cabled/attenuated bench set-ups); nothing here is intended for, or
//! should be built into, an over-the-air emitter. Every model produces a full
//! [`AttackTruth`] record so an operator can always tell a test injection from a real
//! signal, and detection metrics ([`station_metrics`], [`auth_failures`]) are provided so
//! the defensive side can be scored against the very attacks that generated the data.
//!
//! All four families are additive scene components ([`crate::interf::Interferer`]) or
//! message-layer forgeries; none modify the honest transmitters.
//!
//! | model          | mechanism                                                      |
//! |----------------|----------------------------------------------------------------|
//! | [`Jammer`]     | LF jamming: wideband noise or CW at a chosen field strength     |
//! | [`Spoofer`]    | a fake station/chain of real LORAN pulses (GRI, timing offset, power, optional slow time pull) |
//! | [`Meaconer`]   | record-and-delay of a real station (added delay and gain)       |
//! | [`DataChannelAttack`] | forged LDC messages with wrong or absent authentication  |
//!
//! All signal field strengths are RMS µV/m, matching [`crate::interf`] and the simulator's
//! truth; a model's `add` returns µV/m (the scene scales it like any interferer).

use crate::chain::Chain;
use crate::interf::{Interferer, NOISE_BW_HZ};
use crate::ldc::hmac::{hmac_sha256, tags_equal};
use crate::ldc::modulator::auth_input;
use crate::ldc::msg::{DloranSpecEntry, Message, MessageSpec, MsgContext};
use crate::ldc::rx::{AuthCheck, DecodeReport, DecodedMessage, LdcDecoder};
use crate::ldc::LdcConfig;
use crate::rx::{observations, Measurement, StationLdc};
use crate::signal::{Parity, PulseSignal, Role};
use crate::time::{AbsTime, Ps};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// The kind of a threat injection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttackKind {
    JammerNoise,
    JammerCw,
    Spoof,
    Meacon,
    ForgedData,
}

/// Ground-truth record of a threat injection (so a test injection is always distinguishable
/// from a real signal).
#[derive(Clone, Debug, PartialEq)]
pub struct AttackTruth {
    pub kind: AttackKind,
    /// Active window in scene time (s).
    pub from_s: f64,
    pub to_s: f64,
    /// Target chain/station, when the attack targets one.
    pub target: Option<(String, String)>,
    /// Peak/representative field strength at the receiver (µV/m), when a signal attack.
    pub field_uv_m: f64,
    /// Human-readable description (parameters).
    pub description: String,
}

// ---------------------------------------------------------------------------------------
// F4a — LF jamming
// ---------------------------------------------------------------------------------------

/// Jammer waveform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JamWaveform {
    /// Wideband Gaussian noise; `field_uv_m` is the RMS in 20 kHz (as elsewhere in the crate).
    Noise,
    /// Continuous tone at `freq_hz`; `field_uv_m` is its RMS field.
    Cw { freq_hz: f64 },
}

/// LF jammer (noise or CW).
pub struct Jammer {
    waveform: JamWaveform,
    field_uv_m: f64,
    from_s: f64,
    to_s: f64,
    seed: u64,
    phase: f64,
    rngs: Vec<ChaCha8Rng>,
}

impl Jammer {
    /// Wideband-noise jammer of RMS field `field_rms_20k` (µV/m in 20 kHz) over `[from_s, to_s)`.
    pub fn noise(field_rms_20k: f64, from_s: f64, to_s: f64, seed: u64) -> Jammer {
        Jammer {
            waveform: JamWaveform::Noise,
            field_uv_m: field_rms_20k,
            from_s,
            to_s,
            seed,
            phase: 0.0,
            rngs: vec![],
        }
    }
    /// CW jammer: a tone at `freq_hz`, RMS field `field_rms`.
    pub fn cw(freq_hz: f64, field_rms: f64, from_s: f64, to_s: f64) -> Jammer {
        Jammer {
            waveform: JamWaveform::Cw { freq_hz },
            field_uv_m: field_rms,
            from_s,
            to_s,
            seed: 0,
            phase: 0.0,
            rngs: vec![],
        }
    }
    pub fn truth(&self) -> AttackTruth {
        let (kind, desc) = match self.waveform {
            JamWaveform::Noise => (
                AttackKind::JammerNoise,
                format!("wideband noise jammer, {:.0} µV/m in 20 kHz", self.field_uv_m),
            ),
            JamWaveform::Cw { freq_hz } => (
                AttackKind::JammerCw,
                format!("CW jammer at {:.0} Hz, {:.0} µV/m", freq_hz, self.field_uv_m),
            ),
        };
        AttackTruth {
            kind,
            from_s: self.from_s,
            to_s: self.to_s,
            target: None,
            field_uv_m: self.field_uv_m,
            description: desc,
        }
    }
}

impl Interferer for Jammer {
    fn name(&self) -> String {
        self.truth().description
    }
    fn add(&mut self, t0: Ps, fs_hz: f64, ch: usize, out: &mut [f64]) {
        let t0s = t0.as_s();
        match self.waveform {
            JamWaveform::Cw { freq_hz } => {
                let amp = self.field_uv_m * std::f64::consts::SQRT_2;
                for (n, v) in out.iter_mut().enumerate() {
                    let t = t0s + n as f64 / fs_hz;
                    if t >= self.from_s && t < self.to_s {
                        *v += amp * (std::f64::consts::TAU * freq_hz * t + self.phase).sin();
                    }
                }
            }
            JamWaveform::Noise => {
                while self.rngs.len() <= ch {
                    let k = self.rngs.len() as u64;
                    self.rngs.push(ChaCha8Rng::seed_from_u64(
                        self.seed.wrapping_mul(0x9E37_79B9).wrapping_add(k ^ 0x7a22),
                    ));
                }
                let sigma = self.field_uv_m * ((fs_hz / 2.0) / NOISE_BW_HZ).sqrt();
                let rng = &mut self.rngs[ch];
                for (n, v) in out.iter_mut().enumerate() {
                    let t = t0s + n as f64 / fs_hz;
                    if t >= self.from_s && t < self.to_s {
                        *v += sigma * crate::timebase::gauss(rng);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Shared LORAN-pulse synthesiser for spoofing and meaconing
// ---------------------------------------------------------------------------------------

/// One synthetic emitter in a fake pulse train.
#[derive(Clone, Debug)]
struct Emit {
    role: Role,
    emission_delay_s: f64,
    field_uv_m: f64,
    ecd_s: f64,
}

/// Synthesise LORAN groups of `emits` into `out` (µV/m, carrier polarity from the phase
/// code), aligned to the real Loran epoch via `scene_start`/`gri_ps`. The apparent arrival
/// of a group's pulses is shifted by `base_offset_s + pull_rate_ns_s·(group scene time)`,
/// and only arrivals inside `[from_s, to_s)` are emitted.
#[allow(clippy::too_many_arguments)]
fn synth_groups(
    signal: &PulseSignal,
    gri_ps: i128,
    scene_start: i128,
    emits: &[Emit],
    base_offset_s: f64,
    pull_rate_ns_s: f64,
    from_s: f64,
    to_s: f64,
    t0: Ps,
    fs: f64,
    out: &mut [f64],
) {
    if out.is_empty() {
        return;
    }
    let a_ps = t0.0 as i128;
    let b_ps = a_ps + ((out.len() as f64 / fs * 1e12) as i128) + gri_ps;
    let gri_s = gri_ps as f64 * 1e-12;
    let extent = signal.envelope.extent_s();
    let k0 = (scene_start + a_ps).div_euclid(gri_ps) - 1;
    let k1 = (scene_start + b_ps).div_euclid(gri_ps) + 1;
    let t0s = t0.as_s();
    let sqrt2 = std::f64::consts::SQRT_2;
    for k in k0..=k1 {
        let group_start_s = ((k as i128 * gri_ps) - scene_start) as f64 * 1e-12;
        let pull = base_offset_s + pull_rate_ns_s * 1e-9 * group_start_s;
        let parity = Parity::of_group(k as i64);
        for e in emits {
            for p in signal.group(e.role, parity) {
                let arrival = group_start_s + e.emission_delay_s + p.offset.as_s() + pull;
                if arrival < from_s || arrival >= to_s {
                    continue;
                }
                let n_lo = (((arrival - t0s) * fs).ceil()).max(0.0) as usize;
                let n_hi_f = ((arrival + extent - t0s) * fs).floor();
                if n_hi_f < 0.0 {
                    continue;
                }
                let n_hi = (n_hi_f as usize).min(out.len() - 1);
                let amp = e.field_uv_m * sqrt2 * p.amp;
                for (n, slot) in out.iter_mut().enumerate().take(n_hi + 1).skip(n_lo) {
                    let t = t0s + n as f64 / fs;
                    let tau = t - arrival;
                    let v = amp * signal.pulse(tau, e.ecd_s, p.phase_rad);
                    if v != 0.0 {
                        *slot += v;
                    }
                }
            }
        }
    }
}

fn emits_from_chain(chain: &Chain, master_field_uv_m: f64) -> Vec<Emit> {
    let master_power = chain
        .stations
        .iter()
        .find(|s| s.role == Role::Master)
        .map(|s| s.power_kw)
        .unwrap_or_else(|| chain.stations.first().map(|s| s.power_kw).unwrap_or(400.0));
    chain
        .stations
        .iter()
        .map(|s| Emit {
            role: s.role,
            emission_delay_s: s.emission_delay_us * 1e-6,
            field_uv_m: master_field_uv_m * (s.power_kw / master_power).sqrt(),
            ecd_s: s.ecd_us * 1e-6,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// F4b — pulse-structured spoofing (a fake station/chain)
// ---------------------------------------------------------------------------------------

/// A spoofer: a fake station/chain transmitting real LORAN pulses with a chosen GRI (the
/// chain's), timing offset and power, optionally pulling the apparent time slowly.
pub struct Spoofer {
    chain: Chain,
    gri_ps: i128,
    scene_start: i128,
    emits: Vec<Emit>,
    master_field_uv_m: f64,
    timing_offset_s: f64,
    pull_rate_ns_s: f64,
    from_s: f64,
    to_s: f64,
}

impl Spoofer {
    /// A spoofer broadcasting `chain`'s pulse structure at `master_field_uv_m` (secondaries
    /// scaled by √power), apparent-time-shifted by `timing_offset_s` and pulled at
    /// `pull_rate_ns_s` (ns per scene second), active over `[from_s, to_s)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain: &Chain,
        scene_start: AbsTime,
        master_field_uv_m: f64,
        timing_offset_s: f64,
        pull_rate_ns_s: f64,
        from_s: f64,
        to_s: f64,
    ) -> Result<Spoofer, String> {
        chain.validate()?;
        Ok(Spoofer {
            gri_ps: chain.gri().0 as i128,
            scene_start: scene_start.0,
            emits: emits_from_chain(chain, master_field_uv_m),
            chain: chain.clone(),
            master_field_uv_m,
            timing_offset_s,
            pull_rate_ns_s,
            from_s,
            to_s,
        })
    }
    pub fn truth(&self) -> AttackTruth {
        AttackTruth {
            kind: AttackKind::Spoof,
            from_s: self.from_s,
            to_s: self.to_s,
            target: Some((self.chain.name.clone(), "*".into())),
            field_uv_m: self.master_field_uv_m,
            description: format!(
                "spoof of chain {} (GRI {} µs), offset {:.0} ns, pull {:.1} ns/s",
                self.chain.name,
                self.chain.gri_us,
                self.timing_offset_s * 1e9,
                self.pull_rate_ns_s
            ),
        }
    }
}

impl Interferer for Spoofer {
    fn name(&self) -> String {
        self.truth().description
    }
    fn add(&mut self, t0: Ps, fs_hz: f64, _ch: usize, out: &mut [f64]) {
        synth_groups(
            &self.chain.signal,
            self.gri_ps,
            self.scene_start,
            &self.emits,
            self.timing_offset_s,
            self.pull_rate_ns_s,
            self.from_s,
            self.to_s,
            t0,
            fs_hz,
            out,
        );
    }
}

// ---------------------------------------------------------------------------------------
// F4c — meaconing (record and delay of a real station)
// ---------------------------------------------------------------------------------------

/// A meaconer: re-radiates one real station's pulses delayed by `extra_delay_s` with a
/// re-radiated field of `source_field_uv_m` scaled by `gain_db`. Modelled: the delayed
/// replica is synthesised from the victim station's known pulse structure (no access to the
/// live samples is assumed), so it is a representative late copy, not a captured waveform.
pub struct Meaconer {
    chain: Chain,
    station_index: usize,
    gri_ps: i128,
    scene_start: i128,
    emits: Vec<Emit>,
    extra_delay_s: f64,
    from_s: f64,
    to_s: f64,
    field_uv_m: f64,
}

impl Meaconer {
    pub fn new(
        chain: &Chain,
        station_index: usize,
        scene_start: AbsTime,
        extra_delay_s: f64,
        source_field_uv_m: f64,
        gain_db: f64,
        from_s: f64,
        to_s: f64,
    ) -> Result<Meaconer, String> {
        chain.validate()?;
        if station_index >= chain.stations.len() {
            return Err("meaconer: station index out of range".into());
        }
        let st = &chain.stations[station_index];
        let field = source_field_uv_m * 10f64.powf(gain_db / 20.0);
        let emits = vec![Emit {
            role: st.role,
            emission_delay_s: st.emission_delay_us * 1e-6,
            field_uv_m: field,
            ecd_s: st.ecd_us * 1e-6,
        }];
        Ok(Meaconer {
            gri_ps: chain.gri().0 as i128,
            scene_start: scene_start.0,
            emits,
            extra_delay_s,
            from_s,
            to_s,
            field_uv_m: field,
            chain: chain.clone(),
            station_index,
        })
    }
    pub fn truth(&self) -> AttackTruth {
        AttackTruth {
            kind: AttackKind::Meacon,
            from_s: self.from_s,
            to_s: self.to_s,
            target: Some((
                self.chain.name.clone(),
                self.chain.stations[self.station_index].id.clone(),
            )),
            field_uv_m: self.field_uv_m,
            description: format!(
                "meacon of {}/{}: +{:.1} µs delay",
                self.chain.name,
                self.chain.stations[self.station_index].id,
                self.extra_delay_s * 1e6
            ),
        }
    }
}

impl Interferer for Meaconer {
    fn name(&self) -> String {
        self.truth().description
    }
    fn add(&mut self, t0: Ps, fs_hz: f64, _ch: usize, out: &mut [f64]) {
        synth_groups(
            &self.chain.signal,
            self.gri_ps,
            self.scene_start,
            &self.emits,
            self.extra_delay_s,
            0.0,
            self.from_s,
            self.to_s,
            t0,
            fs_hz,
            out,
        );
    }
}

// ---------------------------------------------------------------------------------------
// F4d — data-channel attack (forged LDC messages)
// ---------------------------------------------------------------------------------------

/// How a forged message set is authenticated.
#[derive(Clone, Debug, PartialEq)]
pub enum ForgedAuth {
    /// No auth message at all (the receiver sees unauthenticated data).
    Absent,
    /// An auth message whose tag is computed with the wrong key.
    WrongKey(Vec<u8>),
}

/// A forged data-channel message set (e.g. false differential corrections or a false
/// "station usable" integrity message) with wrong or absent authentication.
#[derive(Clone, Debug)]
pub struct DataChannelAttack {
    pub messages: Vec<Message>,
    pub auth: ForgedAuth,
    pub station: usize,
    pub first_frame: i64,
}

impl DataChannelAttack {
    /// Forge false differential-LORAN corrections.
    pub fn forged_dloran(
        reference_site: u16,
        corrections: Vec<DloranSpecEntry>,
        auth: ForgedAuth,
        station: usize,
        first_frame: i64,
    ) -> DataChannelAttack {
        let ctx = MsgContext {
            station,
            role: Role::Master,
            first_group: first_frame,
            gri_us: None,
        };
        let messages = corrections
            .chunks(3)
            .map(|c| {
                MessageSpec::Dloran {
                    reference_site,
                    age_s: 0,
                    corrections: c.to_vec(),
                }
                .render(&ctx)
            })
            .collect();
        DataChannelAttack {
            messages,
            auth,
            station,
            first_frame,
        }
    }

    /// Build the forged [`DecodeReport`] (data messages followed by any forged auth message)
    /// as a receiver's LDC layer would hand it to [`LdcDecoder::verify_auth`]. The covered
    /// data messages are laid out on consecutive frames.
    pub fn decode_report(&self) -> DecodeReport {
        let mut messages: Vec<DecodedMessage> = Vec::new();
        let mut frame = 0usize;
        for m in &self.messages {
            let body = m.to_bits();
            messages.push(DecodedMessage {
                message: m.clone(),
                first_frame: frame,
                frames: 1,
                corrected_symbols: 0,
                body_bits: body,
            });
            frame += 1;
        }
        if let ForgedAuth::WrongKey(key) = &self.auth {
            let covered = self.messages.len();
            let blocks: Vec<Vec<bool>> = messages.iter().map(|m| m.body_bits.clone()).collect();
            let tag_full = hmac_sha256(
                key,
                &auth_input(self.station, self.first_frame + frame as i64, &blocks),
            );
            let tag = tag_full[..8].to_vec();
            let auth_msg = Message::Auth {
                key_id: 0,
                covered: covered as u8,
                tag: tag.clone(),
            };
            messages.push(DecodedMessage {
                message: auth_msg.clone(),
                first_frame: frame,
                frames: 1,
                corrected_symbols: 0,
                body_bits: auth_msg.to_bits(),
            });
        }
        DecodeReport {
            messages,
            ..Default::default()
        }
    }

    /// Run the authentication check with the *genuine* key: every present auth message that
    /// does not verify is a flagged forgery. Returns the checks and whether the attack would
    /// be caught (a wrong-key tag fails; an absent tag leaves the data unauthenticated).
    pub fn check(&self, genuine_key: &[u8]) -> (Vec<AuthCheck>, bool) {
        let rep = self.decode_report();
        let dec = LdcDecoder {
            name: "auth-check".into(),
            codec: forged_codec(),
            mapper: forged_mapper(),
            data_pulse: None,
            stations: None,
            auth_key: genuine_key.to_vec(),
        };
        let checks = dec.verify_auth(self.station, &rep, self.first_frame);
        let caught = match &self.auth {
            ForgedAuth::Absent => true, // no auth message present → data is unauthenticated
            ForgedAuth::WrongKey(_) => checks.iter().any(|c| c.verified == Some(false)),
        };
        (checks, caught)
    }
}

/// A placeholder codec/mapper for the authentication-only decoder (verify_auth uses neither).
fn forged_codec() -> crate::ldc::modulator::FrameCodec {
    LdcConfig::ninth_pulse(Default::default())
        .compile()
        .unwrap()
        .unwrap()
        .codec
}
fn forged_mapper() -> crate::ldc::mapper::Mapper {
    LdcConfig::ninth_pulse(Default::default())
        .compile()
        .unwrap()
        .unwrap()
        .mapper
}

// ---------------------------------------------------------------------------------------
// Detection metrics
// ---------------------------------------------------------------------------------------

/// Per-station detection metrics over a measurement sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct StationMetrics {
    pub chain: String,
    pub station: String,
    pub channel: Option<usize>,
    pub n: usize,
    pub snr_mean_db: f64,
    /// Largest SNR change between consecutive measurements (dB).
    pub snr_max_jump_db: f64,
    /// Largest deviation of the pseudo-delay from its median (ns) — a ToA jump.
    pub toa_max_jump_ns: f64,
    pub ecd_mean_us: f64,
    pub ecd_std_us: f64,
    pub cycle_slips: usize,
}

/// Compute per-station detection metrics (SNR jumps, ToA jumps, ECD spread, cycle slips)
/// from receiver measurements — the raw material of spoofing / meaconing / jamming
/// detectors.
pub fn station_metrics(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
) -> Vec<StationMetrics> {
    use std::collections::BTreeMap;
    let mut by: BTreeMap<(usize, usize, Option<usize>), Vec<&Measurement>> = BTreeMap::new();
    for m in meas {
        by.entry((m.chain_index, m.station_index, m.channel))
            .or_default()
            .push(m);
    }
    let mut out = Vec::new();
    for ((ci, si, ch), mut v) in by {
        v.sort_by_key(|m| m.group);
        let n = v.len();
        let snr_mean = v.iter().map(|m| m.snr_db).sum::<f64>() / n as f64;
        let mut snr_jump = 0.0f64;
        for w in v.windows(2) {
            snr_jump = snr_jump.max((w[1].snr_db - w[0].snr_db).abs());
        }
        let mut pd: Vec<f64> = v
            .iter()
            .map(|m| {
                m.toa_s - crate::rx::nominal_emission_s(&chains[ci], si, m.group, start)
            })
            .collect();
        let mut sorted = pd.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[sorted.len() / 2];
        let toa_jump = pd
            .iter_mut()
            .map(|d| (*d - median).abs())
            .fold(0.0, f64::max)
            * 1e9;
        let ecd_mean = v.iter().map(|m| m.ecd_us).sum::<f64>() / n as f64;
        let ecd_var = v.iter().map(|m| (m.ecd_us - ecd_mean).powi(2)).sum::<f64>()
            / (n.max(1) as f64);
        out.push(StationMetrics {
            chain: v[0].chain.clone(),
            station: v[0].station.clone(),
            channel: ch,
            n,
            snr_mean_db: snr_mean,
            snr_max_jump_db: snr_jump,
            toa_max_jump_ns: toa_jump,
            ecd_mean_us: ecd_mean,
            ecd_std_us: ecd_var.sqrt(),
            cycle_slips: v.iter().filter(|m| m.cycle_slip).count(),
        });
    }
    out
}

/// Count flagged authentication failures across decoded data channels (an auth message that
/// did not verify). A separate count of auth messages that could not be checked (coverage
/// missing) is also returned.
pub fn auth_failures(ldcs: &[StationLdc]) -> (usize, usize) {
    let mut failed = 0;
    let mut unchecked = 0;
    for l in ldcs {
        for a in &l.auth {
            match a.verified {
                Some(false) => failed += 1,
                None => unchecked += 1,
                Some(true) => {}
            }
        }
    }
    (failed, unchecked)
}

/// A simple ToA-jump detector: stations whose measured pseudo-delay departs from its own
/// median by more than `limit_ns` (meaconing / spoofing / pull).
pub fn toa_anomalies(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
    limit_ns: f64,
) -> Vec<StationMetrics> {
    station_metrics(meas, chains, start)
        .into_iter()
        .filter(|m| m.toa_max_jump_ns > limit_ns)
        .collect()
}

/// Build the full-station observation baseline (used by tests / callers to compare an
/// attacked run against a clean one).
pub fn observation_delays(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
) -> Vec<(String, f64)> {
    observations(meas, chains, start)
        .into_iter()
        .map(|o| (format!("{}/{}", o.chain, o.station), o.pseudo_delay_s))
        .collect()
}

#[cfg(test)]
mod tests;
