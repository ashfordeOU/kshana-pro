// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D4. Broadcast encoding of a lunar time scale's offset from UTC (proposal "KLTO-1")
//!
//! **This is a proposal pending LunaNet LNIS AD5 (reference frames and time), not a standard.**
//! AD5 is in drafting and not public (research note §1.1); NASA's open question 5 asks how the
//! TL–UTC offset should be represented in broadcast messages (gap 5). This module is a concrete,
//! testable answer to put in front of the AD5 vendor review: a 147-bit message, a codec, its
//! quantisation bounds, a fit from the D1 model, and JSON conformance vectors
//! (`tests/fixtures/lunar_pro/klto1_conformance_vectors.json`). Every field width and scale
//! below is a design choice of this proposal, modelled on the GPS UTC parameters of IS-GPS-200
//! (`A0` 2^-30 s, `A1` 2^-50 s/s) only as precedent; nothing here is taken from AD5.
//!
//! ## The message (MSB first, 147 bits, carried in 19 bytes with 5 zero pad bits)
//!
//! | Field | Bits | Type | Scale | Meaning |
//! |-------|------|------|-------|---------|
//! | `version` | 4 | u | 1 | 1 for KLTO-1 |
//! | `scale` | 2 | u | — | 0 TCL, 1 TL, 2 and 3 reserved |
//! | `reference` | 4 | u | — | 0 UTC (BIPM); 1–15 a UTC(k) from a registry (proposal) |
//! | `iod` | 8 | u | — | issue of data |
//! | `wn_ot` | 13 | u | 1 week | weeks since the KLTO epoch, 2026-01-01T00:00:00 of the broadcast scale |
//! | `t_ot` | 16 | u | 16 s | time of applicability in the week (< 604 800 s) |
//! | `n` | 16 | s | 1 s | integer seconds of the offset (includes UTC leap seconds) |
//! | `a0` | 32 | s | 2^-32 s | fractional offset, `|a0| ≤ 0.5 s` (resolution 0.23 ns) |
//! | `a1` | 24 | s | 2^-50 s/s | rate (range ±7.45e-9; TL − UTC is ≈ 6.48e-10) |
//! | `a2` | 20 | s | 2^-64 s/s² | rate change (the ≈127 µs monthly term needs it beyond a few hours) |
//! | `validity` | 4 | u | index | window centred on `t_ot`: 1 h, 2 h, 4 h, 6 h, 12 h, 1 d, 2 d, 4 d, 7 d; 9–15 reserved |
//! | `uclass` | 4 | u | index | bound on the expanded (k = 2) error of the broadcast offset in the window: 0.5, 1, 2, 5, 10, 20, 50, 100, 200, 500 ns, 1, 10, 100 µs, 1 ms; 14 > 1 ms; 15 not monitored |
//!
//! `offset(t) = n + a0 + a1·dt + a2·dt²`, `dt = t − t_ot` in seconds of the broadcast scale,
//! defined as (broadcast scale − UTC) for `|dt| ≤ validity/2`. A pending leap second is not
//! encoded (an open item of the proposal: GPS-style `ΔtLSF`, `WN_LSF`, `DN` would be added);
//! the fit refuses a window that contains one. No CRC: integrity is the carrying message's
//! (AFS frame or LNIS message) job.
//!
//! Evidence label: **MODELLED** (round-trip, quantisation-bound and D1-fit tests).

use kshana::timescales::TwoPartJd;
use serde::{Deserialize, Serialize};

use super::time::{LunarOption, Model, Scale};

/// The statement every output of this module carries.
pub const STATUS: &str =
    "KLTO-1 is a Kshana PROPOSAL pending LunaNet LNIS AD5; it is not a standard";

/// Message length in bits and bytes.
pub const BITS: usize = 147;
pub const BYTES: usize = 19;
/// The KLTO epoch: 2026-01-01T00:00:00 of the broadcast scale, as a JD.
pub const EPOCH_JD: f64 = 2_461_041.5;
pub const WEEK_S: f64 = 604_800.0;
pub const T_OT_LSB_S: f64 = 16.0;
pub const A0_LSB: f64 = 1.0 / 4_294_967_296.0; // 2^-32
pub const A1_LSB: f64 = 8.881_784_197_001_252e-16; // 2^-50
pub const A2_LSB: f64 = 5.421_010_862_427_522e-20; // 2^-64
/// Validity windows (s) by index.
pub const VALIDITY_S: [f64; 9] = [
    3_600.0, 7_200.0, 14_400.0, 21_600.0, 43_200.0, 86_400.0, 172_800.0, 345_600.0, 604_800.0,
];
/// Uncertainty-class bounds (ns) by index 0–13; 14 means > 1 ms, 15 not monitored.
pub const UCLASS_NS: [f64; 14] = [
    0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1e3, 1e4, 1e5, 1e6,
];

/// The broadcast scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BroadcastScale {
    Tcl,
    Tl,
}

impl BroadcastScale {
    pub fn code(self) -> u8 {
        match self {
            BroadcastScale::Tcl => 0,
            BroadcastScale::Tl => 1,
        }
    }
    pub fn option(self) -> LunarOption {
        match self {
            BroadcastScale::Tcl => LunarOption::Tcl,
            BroadcastScale::Tl => LunarOption::Tl,
        }
    }
    pub fn from_option(o: LunarOption) -> Self {
        match o {
            LunarOption::Tcl => BroadcastScale::Tcl,
            LunarOption::Tl => BroadcastScale::Tl,
        }
    }
}

/// The message as integers (what is on the wire).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub version: u8,
    pub scale: BroadcastScale,
    pub reference: u8,
    pub iod: u8,
    pub wn_ot: u16,
    /// In units of 16 s.
    pub t_ot: u16,
    pub n: i16,
    pub a0: i32,
    /// 24-bit signed.
    pub a1: i32,
    /// 20-bit signed.
    pub a2: i32,
    pub validity: u8,
    pub uclass: u8,
}

/// The message in physical units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Physical {
    pub scale: BroadcastScale,
    pub reference: u8,
    pub iod: u8,
    pub wn_ot: u16,
    pub t_ot_s: f64,
    /// The whole offset at `t_ot` (s); split into `n` and `a0` on quantisation.
    pub offset_s: f64,
    pub rate: f64,
    pub rate_change: f64,
    pub validity_s: f64,
    /// The bound on the expanded error (ns); `None` for "not monitored".
    pub u_bound_ns: Option<f64>,
}

fn signed_range(bits: u32) -> (i64, i64) {
    (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1)
}

fn round_to(v: f64, lsb: f64, bits: u32, name: &str) -> Result<i32, String> {
    let q = (v / lsb).round();
    let (lo, hi) = signed_range(bits);
    if !q.is_finite() || q < lo as f64 || q > hi as f64 {
        return Err(format!(
            "{name} = {v:e} is outside the {bits}-bit field range"
        ));
    }
    Ok(q as i32)
}

impl Physical {
    /// Quantise to the wire integers (round to nearest), refusing out-of-range values.
    pub fn quantise(&self) -> Result<Message, String> {
        if !(0.0..WEEK_S).contains(&self.t_ot_s) {
            return Err(format!("t_ot {} s is outside the week", self.t_ot_s));
        }
        let t_ot = (self.t_ot_s / T_OT_LSB_S).round();
        if (t_ot * T_OT_LSB_S - self.t_ot_s).abs() > 1e-9 {
            return Err(format!("t_ot {} s is not a multiple of 16 s", self.t_ot_s));
        }
        if self.wn_ot >= 1 << 13 || self.reference > 15 {
            return Err("wn_ot or reference out of range".into());
        }
        let n = self.offset_s.round();
        if n.abs() > i16::MAX as f64 {
            return Err(format!("offset {} s out of range", self.offset_s));
        }
        // |offset − n| ≤ 0.5 s; exactly +0.5 s would be 2^31, one past the field: carry it.
        let mut n = n as i16;
        let mut q = ((self.offset_s - n as f64) / A0_LSB).round();
        if q >= 2_147_483_648.0 {
            n = n.checked_add(1).ok_or("offset out of range")?;
            q -= 4_294_967_296.0;
        }
        let a0 = round_to(q * A0_LSB, A0_LSB, 32, "a0")?;
        let validity = VALIDITY_S
            .iter()
            .position(|v| (v - self.validity_s).abs() < 1e-9)
            .ok_or_else(|| {
                format!(
                    "validity {} s is not one of {VALIDITY_S:?}",
                    self.validity_s
                )
            })? as u8;
        Ok(Message {
            version: 1,
            scale: self.scale,
            reference: self.reference,
            iod: self.iod,
            wn_ot: self.wn_ot,
            t_ot: t_ot as u16,
            n,
            a0,
            a1: round_to(self.rate, A1_LSB, 24, "a1")?,
            a2: round_to(self.rate_change, A2_LSB, 20, "a2")?,
            validity,
            uclass: uclass_for(self.u_bound_ns),
        })
    }
}

/// The smallest class whose bound is at least `u_ns` (14 beyond 1 ms, 15 for `None`).
pub fn uclass_for(u_ns: Option<f64>) -> u8 {
    match u_ns {
        None => 15,
        Some(u) => UCLASS_NS
            .iter()
            .position(|b| u <= *b)
            .map_or(14, |i| i as u8),
    }
}

impl Message {
    /// Physical values.
    pub fn physical(&self) -> Physical {
        Physical {
            scale: self.scale,
            reference: self.reference,
            iod: self.iod,
            wn_ot: self.wn_ot,
            t_ot_s: self.t_ot as f64 * T_OT_LSB_S,
            offset_s: self.n as f64 + self.a0 as f64 * A0_LSB,
            rate: self.a1 as f64 * A1_LSB,
            rate_change: self.a2 as f64 * A2_LSB,
            validity_s: VALIDITY_S
                .get(self.validity as usize)
                .copied()
                .unwrap_or(f64::NAN),
            u_bound_ns: match self.uclass {
                15 => None,
                14 => Some(f64::INFINITY),
                i => Some(UCLASS_NS[i as usize]),
            },
        }
    }

    /// `offset(dt) = n + a0 + a1 dt + a2 dt²` (s).
    pub fn offset_at(&self, dt_s: f64) -> f64 {
        self.n as f64
            + self.a0 as f64 * A0_LSB
            + self.a1 as f64 * A1_LSB * dt_s
            + self.a2 as f64 * A2_LSB * dt_s * dt_s
    }

    /// The largest error quantisation can add at `dt` (half an LSB of each coefficient).
    pub fn quantisation_bound(dt_s: f64) -> f64 {
        0.5 * (A0_LSB + A1_LSB * dt_s.abs() + A2_LSB * dt_s * dt_s)
    }

    /// Encode to 19 bytes.
    pub fn encode(&self) -> Result<[u8; BYTES], String> {
        self.check()?;
        let mut w = BitWriter::default();
        w.put(self.version as u64, 4);
        w.put(self.scale.code() as u64, 2);
        w.put(self.reference as u64, 4);
        w.put(self.iod as u64, 8);
        w.put(self.wn_ot as u64, 13);
        w.put(self.t_ot as u64, 16);
        w.put_signed(self.n as i64, 16);
        w.put_signed(self.a0 as i64, 32);
        w.put_signed(self.a1 as i64, 24);
        w.put_signed(self.a2 as i64, 20);
        w.put(self.validity as u64, 4);
        w.put(self.uclass as u64, 4);
        debug_assert_eq!(w.n, BITS);
        let mut out = [0u8; BYTES];
        out.copy_from_slice(&w.bytes()[..BYTES]);
        Ok(out)
    }

    fn check(&self) -> Result<(), String> {
        let (lo24, hi24) = signed_range(24);
        let (lo20, hi20) = signed_range(20);
        if self.version != 1 {
            return Err(format!("unsupported version {}", self.version));
        }
        if self.reference > 15 || self.wn_ot >= 1 << 13 {
            return Err("reference or wn_ot out of range".into());
        }
        if self.t_ot as f64 * T_OT_LSB_S >= WEEK_S {
            return Err("t_ot outside the week".into());
        }
        if !(lo24..=hi24).contains(&(self.a1 as i64)) || !(lo20..=hi20).contains(&(self.a2 as i64))
        {
            return Err("a1 or a2 outside its field".into());
        }
        if self.validity as usize >= VALIDITY_S.len() || self.uclass > 15 {
            return Err("validity or uclass index out of range".into());
        }
        Ok(())
    }

    /// Decode 19 bytes.
    pub fn decode(b: &[u8]) -> Result<Message, String> {
        if b.len() != BYTES {
            return Err(format!(
                "a KLTO-1 message is {BYTES} bytes, got {}",
                b.len()
            ));
        }
        let mut r = BitReader { b, pos: 0 };
        let version = r.get(4) as u8;
        let scale = match r.get(2) {
            0 => BroadcastScale::Tcl,
            1 => BroadcastScale::Tl,
            s => return Err(format!("reserved scale code {s}")),
        };
        let m = Message {
            version,
            scale,
            reference: r.get(4) as u8,
            iod: r.get(8) as u8,
            wn_ot: r.get(13) as u16,
            t_ot: r.get(16) as u16,
            n: r.get_signed(16) as i16,
            a0: r.get_signed(32) as i32,
            a1: r.get_signed(24) as i32,
            a2: r.get_signed(20) as i32,
            validity: r.get(4) as u8,
            uclass: r.get(4) as u8,
        };
        if r.get(5) != 0 {
            return Err("non-zero pad bits".into());
        }
        m.check()?;
        Ok(m)
    }
}

#[derive(Default)]
struct BitWriter {
    acc: Vec<u8>,
    n: usize,
}

impl BitWriter {
    fn put(&mut self, v: u64, bits: usize) {
        for i in (0..bits).rev() {
            if self.n.is_multiple_of(8) {
                self.acc.push(0);
            }
            if (v >> i) & 1 == 1 {
                let last = self.acc.len() - 1;
                self.acc[last] |= 0x80 >> (self.n % 8);
            }
            self.n += 1;
        }
    }
    fn put_signed(&mut self, v: i64, bits: usize) {
        self.put((v as u64) & ((1u64 << bits) - 1), bits);
    }
    fn bytes(&self) -> Vec<u8> {
        let mut b = self.acc.clone();
        b.resize(BYTES, 0);
        b
    }
}

struct BitReader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn get(&mut self, bits: usize) -> u64 {
        let mut v = 0u64;
        for _ in 0..bits {
            let bit = (self.b[self.pos / 8] >> (7 - self.pos % 8)) & 1;
            v = (v << 1) | bit as u64;
            self.pos += 1;
        }
        v
    }
    fn get_signed(&mut self, bits: usize) -> i64 {
        let v = self.get(bits);
        if v >> (bits - 1) & 1 == 1 {
            v as i64 - (1i64 << bits)
        } else {
            v as i64
        }
    }
}

/// `(wn_ot, t_ot seconds)` of a broadcast-scale JD, `t_ot` snapped down to the 16 s grid.
pub fn week_and_tow(jd_scale: TwoPartJd) -> Result<(u16, f64), String> {
    let s = jd_scale.diff_seconds(TwoPartJd::from_f64(EPOCH_JD));
    if s < 0.0 {
        return Err("before the KLTO epoch (2026-01-01)".into());
    }
    let wn = (s / WEEK_S).floor();
    if wn >= 8192.0 {
        return Err("beyond the 13-bit week range".into());
    }
    let tow = ((s - wn * WEEK_S) / T_OT_LSB_S).floor() * T_OT_LSB_S;
    Ok((wn as u16, tow))
}

/// The fit of a broadcast message to the D1 model over its validity window.
#[derive(Clone, Debug, Serialize)]
pub struct Fit {
    pub message: Message,
    pub physical: Physical,
    /// Largest |broadcast − model| over the window, after quantisation (s).
    pub max_residual_s: f64,
    /// Expanded (k = 2) D1 model uncertainty of the offset at `t_ot` (s).
    pub model_u_k2_s: f64,
    /// The error bound the class was chosen from (s): model U + residual.
    pub bound_s: f64,
    pub status: &'static str,
}

/// Fit a KLTO-1 message to the D1 model: (scale − UTC) sampled over the window centred on
/// `centre` (a JD in the broadcast scale, snapped to the 16 s grid), least-squares quadratic,
/// quantised; the class bounds the model's expanded uncertainty plus the largest residual.
pub fn fit(
    model: &Model,
    scale: BroadcastScale,
    centre: TwoPartJd,
    validity_index: usize,
    iod: u8,
    reference: u8,
) -> Result<Fit, String> {
    let v = *VALIDITY_S
        .get(validity_index)
        .ok_or_else(|| format!("validity index {validity_index} out of range"))?;
    let (wn, tow) = week_and_tow(centre)?;
    let t_ot = TwoPartJd::from_f64(EPOCH_JD).add_seconds(wn as f64 * WEEK_S + tow);
    let lunar = scale.option().scale();
    let off = |dt: f64| -> f64 {
        // UTC − lunar at the lunar instant, negated.
        -model
            .convert(lunar, t_ot.add_seconds(dt), Scale::Utc)
            .offset
            .value()
    };
    let n = 25;
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let dt = v * (i as f64 / (n - 1) as f64 - 0.5);
            (dt, off(dt))
        })
        .collect();
    let y0 = pts[n / 2].1;
    let (c0, c1, c2) = quad_fit(&pts, y0);
    let worst_raw = pts
        .iter()
        .map(|(t, y)| (c0 + c1 * t + c2 * t * t - y).abs())
        .fold(0.0, f64::max);
    if worst_raw > 0.25 {
        return Err("the validity window contains a UTC leap second (offset jumps by 1 s); shorten or move it".into());
    }
    let u_k2 = 2.0 * model.convert(lunar, t_ot, Scale::Utc).offset.u_c();
    let mut phys = Physical {
        scale,
        reference,
        iod,
        wn_ot: wn,
        t_ot_s: tow,
        offset_s: c0,
        rate: c1,
        rate_change: c2,
        validity_s: v,
        u_bound_ns: None,
    };
    let mut msg = phys.quantise()?;
    let max_res = pts
        .iter()
        .map(|(t, y)| (msg.offset_at(*t) - y).abs())
        .fold(0.0, f64::max);
    let bound = u_k2 + max_res;
    phys.u_bound_ns = Some(bound * 1e9);
    msg.uclass = uclass_for(phys.u_bound_ns);
    Ok(Fit {
        physical: msg.physical(),
        message: msg,
        max_residual_s: max_res,
        model_u_k2_s: u_k2,
        bound_s: bound,
        status: STATUS,
    })
}

/// Least-squares `y − y0 = c0' + c1 t + c2 t²` (centred and offset for conditioning).
fn quad_fit(pts: &[(f64, f64)], y0: f64) -> (f64, f64, f64) {
    let scale = pts.iter().map(|p| p.0.abs()).fold(1.0, f64::max);
    let mut ata = [[0.0f64; 3]; 3];
    let mut atb = [0.0f64; 3];
    for &(t, y) in pts {
        let x = t / scale;
        let row = [1.0, x, x * x];
        for i in 0..3 {
            for j in 0..3 {
                ata[i][j] += row[i] * row[j];
            }
            atb[i] += row[i] * (y - y0);
        }
    }
    let c = solve3(ata, atb);
    (y0 + c[0], c[1] / scale, c[2] / (scale * scale))
}

fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> [f64; 3] {
    for k in 0..3 {
        let p = (k..3)
            .max_by(|&i, &j| a[i][k].abs().total_cmp(&a[j][k].abs()))
            .unwrap_or(k);
        a.swap(k, p);
        b.swap(k, p);
        for i in k + 1..3 {
            let f = a[i][k] / a[k][k];
            let pivot = a[k];
            for (aij, akj) in a[i].iter_mut().zip(pivot).skip(k) {
                *aij -= f * akj;
            }
            b[i] -= f * b[k];
        }
    }
    let mut x = [0.0; 3];
    for k in (0..3).rev() {
        let s: f64 = (k + 1..3).map(|j| a[k][j] * x[j]).sum();
        x[k] = (b[k] - s) / a[k][k];
    }
    x
}

/// One conformance vector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vector {
    pub name: String,
    pub description: String,
    pub message: Message,
    pub hex: String,
    /// `(dt_s, offset_s)` evaluated from the decoded integers.
    pub eval: Vec<(f64, f64)>,
}

/// The conformance file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VectorFile {
    pub format: String,
    pub status: String,
    pub bits: usize,
    pub vectors: Vec<Vector>,
}

/// Hex of bytes (lower case).
pub fn to_hex(b: &[u8]) -> String {
    hex::encode(b)
}

fn vector(name: &str, description: &str, m: Message) -> Result<Vector, String> {
    let v = VALIDITY_S[m.validity as usize];
    let eval = [-0.5, -0.25, 0.0, 0.25, 0.5]
        .iter()
        .map(|f| (f * v, m.offset_at(f * v)))
        .collect();
    Ok(Vector {
        name: name.into(),
        description: description.into(),
        hex: to_hex(&m.encode()?),
        message: m,
        eval,
    })
}

/// The conformance vectors: two fitted from the D1 model (TL and TCL, 2026-11-19, 1 day) and
/// field-edge cases.
pub fn conformance_vectors() -> Result<VectorFile, String> {
    let model = Model::default_model();
    let centre = super::time::parse_iso("2026-11-19T12:00:00")?;
    let tl = fit(&model, BroadcastScale::Tl, centre, 5, 1, 0)?.message;
    let tcl = fit(&model, BroadcastScale::Tcl, centre, 5, 2, 0)?.message;
    let (lo24, hi24) = signed_range(24);
    let (lo20, hi20) = signed_range(20);
    let edge_max = Message {
        version: 1,
        scale: BroadcastScale::Tl,
        reference: 15,
        iod: 255,
        wn_ot: 8191,
        t_ot: (WEEK_S / T_OT_LSB_S) as u16 - 1,
        n: i16::MAX,
        a0: i32::MAX,
        a1: hi24 as i32,
        a2: hi20 as i32,
        validity: 8,
        uclass: 15,
    };
    let edge_min = Message {
        version: 1,
        scale: BroadcastScale::Tcl,
        reference: 0,
        iod: 0,
        wn_ot: 0,
        t_ot: 0,
        n: i16::MIN,
        a0: i32::MIN,
        a1: lo24 as i32,
        a2: lo20 as i32,
        validity: 0,
        uclass: 0,
    };
    let lsb = Message {
        version: 1,
        scale: BroadcastScale::Tl,
        reference: 3,
        iod: 42,
        wn_ot: 46,
        t_ot: 1,
        n: 70,
        a0: 1,
        a1: -1,
        a2: 1,
        validity: 2,
        uclass: 7,
    };
    Ok(VectorFile {
        format: "kshana-klto1-conformance-vectors v1".into(),
        status: STATUS.into(),
        bits: BITS,
        vectors: vec![
            vector("d1-tl-2026-11-19-1d", "TL - UTC fitted to the D1 model (LTE440 rate, kshana-equatorial-j2 W0_L), centred 2026-11-19T12:00 TL, 1 day validity", tl)?,
            vector("d1-tcl-2026-11-19-1d", "TCL - UTC fitted to the D1 model (LTE440 rate), centred 2026-11-19T12:00 TCL, 1 day validity", tcl)?,
            vector("edge-max", "every field at its largest value", edge_max)?,
            vector("edge-min", "every field at its smallest value", edge_min)?,
            vector("one-lsb", "single-LSB coefficients and alternating signs", lsb)?,
        ],
    })
}
