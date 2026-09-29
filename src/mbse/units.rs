// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Unit algebra for acceptance criteria.
//!
//! The open engine states a unit for every emitted field in its field-units schema
//! (`docs/field-units-schema.json`) and in the `units` block of each result document.
//! Those unit strings are free text with one house spelling: SI symbols in ASCII, `^`
//! for powers, `*` and `/` for products and quotients, `count` for a cardinality and
//! `1` for anything dimensionless. This module parses that spelling (plus the older
//! dimensionless spellings the engine still carries) into a scale and a dimension, so
//! a criterion literal such as `30 min` can be checked against, and converted to, a
//! field whose unit is `s`.
//!
//! Logarithmic units (`dB`, `dBW`, `dB-Hz`, ...) are kept as opaque atoms: they compare
//! only with the identical atom and are never scaled. An atom the table does not know
//! (for example `aperture` in `aperture*s`) is also opaque, so it still has to match
//! exactly. A unit string that is not a unit at all (`mixed - see note`) does not
//! parse, and a criterion over such a field is reported as not verified.

use std::collections::BTreeMap;

/// A parsed unit: a multiplicative scale to the coherent base and the exponent of each
/// base dimension. Base dimensions are `s`, `m`, `kg`, `K`, `rad`, `count`, and any
/// opaque atom (logarithmic or unknown), each by name.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Value in this unit × `scale` = value in the coherent base unit.
    pub scale: f64,
    /// Base dimension name → exponent. Empty means dimensionless.
    pub dims: BTreeMap<String, i32>,
}

impl Unit {
    /// The dimensionless unit `1`.
    pub fn one() -> Unit {
        Unit {
            scale: 1.0,
            dims: BTreeMap::new(),
        }
    }

    /// True when the unit carries no dimension.
    pub fn is_dimensionless(&self) -> bool {
        self.dims.is_empty()
    }

    /// True when `self` and `other` have the same dimension, so a value in one can be
    /// converted to the other.
    pub fn commensurable(&self, other: &Unit) -> bool {
        self.dims == other.dims
    }

    /// Convert `value` expressed in `self` to `target`. `None` if the dimensions differ.
    pub fn convert(&self, value: f64, target: &Unit) -> Option<f64> {
        if !self.commensurable(target) {
            return None;
        }
        // Multiply by a factor >= 1 or divide by one, so exact decimal ratios stay
        // exact (95 % is 95 / 100 = 0.95, not 95 × 0.01 = 0.9500000000000001).
        let f = self.scale / target.scale;
        Some(if f >= 1.0 {
            value * f
        } else {
            value / (target.scale / self.scale)
        })
    }

    /// The coherent-base spelling of the dimension, e.g. `m/s` or `s`, used in
    /// mismatch messages and in the SysML export.
    pub fn dimension_string(&self) -> String {
        if self.dims.is_empty() {
            return "1".into();
        }
        let mut num = Vec::new();
        let mut den = Vec::new();
        for (k, &e) in &self.dims {
            let s = if e.abs() == 1 {
                k.clone()
            } else {
                format!("{k}^{}", e.abs())
            };
            if e > 0 {
                num.push(s)
            } else {
                den.push(s)
            }
        }
        let n = if num.is_empty() {
            "1".to_string()
        } else {
            num.join("*")
        };
        if den.is_empty() {
            n
        } else {
            format!("{n}/{}", den.join("/"))
        }
    }

    fn mul(mut self, other: &Unit, sign: i32) -> Unit {
        self.scale *= other.scale.powi(sign);
        for (k, e) in &other.dims {
            let v = self.dims.entry(k.clone()).or_insert(0);
            *v += e * sign;
            if *v == 0 {
                self.dims.remove(k);
            }
        }
        self
    }

    fn pow(mut self, p: i32) -> Unit {
        self.scale = self.scale.powi(p);
        for e in self.dims.values_mut() {
            *e *= p;
        }
        self.dims.retain(|_, e| *e != 0);
        self
    }
}

fn base(scale: f64, dims: &[(&str, i32)]) -> Unit {
    Unit {
        scale,
        dims: dims.iter().map(|(k, e)| (k.to_string(), *e)).collect(),
    }
}

fn opaque(name: &str) -> Unit {
    base(1.0, &[(name, 1)])
}

/// Spellings of the dimensionless unit the engine's released documents carry. A unit
/// string containing `(dimensionless)` is also dimensionless.
const DIMENSIONLESS: &[&str] = &[
    "1",
    "dimensionless",
    "fraction",
    "probability",
    "ratio",
    "index",
    "unitless",
    "flag",
];

/// One atomic unit symbol, or `None` for a symbol that is not a unit name at all (it
/// contains a space or other character no unit symbol has).
fn atom(sym: &str) -> Option<Unit> {
    let pi = std::f64::consts::PI;
    let u = match sym {
        // time
        "s" | "sec" => base(1.0, &[("s", 1)]),
        "ms" => base(1e-3, &[("s", 1)]),
        "us" | "µs" | "μs" => base(1e-6, &[("s", 1)]),
        "ns" => base(1e-9, &[("s", 1)]),
        "ps" => base(1e-12, &[("s", 1)]),
        "fs" => base(1e-15, &[("s", 1)]),
        "min" => base(60.0, &[("s", 1)]),
        "h" | "hr" => base(3600.0, &[("s", 1)]),
        "d" | "day" => base(86_400.0, &[("s", 1)]),
        // length
        "m" => base(1.0, &[("m", 1)]),
        "km" => base(1e3, &[("m", 1)]),
        "cm" => base(1e-2, &[("m", 1)]),
        "mm" => base(1e-3, &[("m", 1)]),
        "um" | "µm" | "μm" => base(1e-6, &[("m", 1)]),
        "nm" => base(1e-9, &[("m", 1)]),
        "AU" | "au" => base(149_597_870_700.0, &[("m", 1)]),
        // frequency
        "Hz" => base(1.0, &[("s", -1)]),
        "kHz" => base(1e3, &[("s", -1)]),
        "MHz" => base(1e6, &[("s", -1)]),
        "GHz" => base(1e9, &[("s", -1)]),
        // angle
        "rad" => base(1.0, &[("rad", 1)]),
        "mrad" => base(1e-3, &[("rad", 1)]),
        "urad" | "µrad" => base(1e-6, &[("rad", 1)]),
        "deg" | "°" => base(pi / 180.0, &[("rad", 1)]),
        "arcmin" => base(pi / 180.0 / 60.0, &[("rad", 1)]),
        "arcsec" => base(pi / 180.0 / 3600.0, &[("rad", 1)]),
        "mas" => base(pi / 180.0 / 3.6e6, &[("rad", 1)]),
        // mass, temperature, power, force
        "kg" => base(1.0, &[("kg", 1)]),
        "g" => base(1e-3, &[("kg", 1)]),
        "K" => base(1.0, &[("K", 1)]),
        "W" => base(1.0, &[("kg", 1), ("m", 2), ("s", -3)]),
        "mW" => base(1e-3, &[("kg", 1), ("m", 2), ("s", -3)]),
        "N" => base(1.0, &[("kg", 1), ("m", 1), ("s", -2)]),
        "J" => base(1.0, &[("kg", 1), ("m", 2), ("s", -2)]),
        // gravity anomaly unit (1 mGal = 1e-5 m/s^2)
        "mGal" => base(1e-5, &[("m", 1), ("s", -2)]),
        "Gal" => base(1e-2, &[("m", 1), ("s", -2)]),
        // magnetic flux density
        "T" => base(1.0, &[("kg", 1), ("s", -2), ("A", -1)]),
        "nT" => base(1e-9, &[("kg", 1), ("s", -2), ("A", -1)]),
        // counts and dimensionless scales
        "count" | "counts" => base(1.0, &[("count", 1)]),
        "%" | "percent" => base(1e-2, &[]),
        "ppm" => base(1e-6, &[]),
        "ppb" => base(1e-9, &[]),
        // logarithmic units are opaque: identical atom only, never scaled
        "dB" | "dBW" | "dBm" | "dBi" | "dBic" | "dB-Hz" | "dBHz" | "dB/K" | "dBW/Hz" | "dBm/Hz" => {
            opaque(match sym {
                "dBHz" => "dB-Hz",
                other => other,
            })
        }
        _ if DIMENSIONLESS.contains(&sym) => Unit::one(),
        _ => {
            // An unknown but well-formed symbol is an opaque atom; anything with a
            // character no unit symbol has is not a unit.
            if !sym.is_empty()
                && sym
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '°')
                && sym.chars().next().is_some_and(|c| c.is_alphabetic())
            {
                opaque(sym)
            } else {
                return None;
            }
        }
    };
    Some(u)
}

/// Parse a unit string in the engine's house spelling. Returns `None` for a string that
/// is not a unit (for example `mixed - see note`).
pub fn parse_unit(raw: &str) -> Option<Unit> {
    let s = raw.trim();
    if s.is_empty() {
        return Some(Unit::one());
    }
    if s.contains("(dimensionless)") || DIMENSIONLESS.contains(&s) {
        return Some(Unit::one());
    }
    // Opaque compound logarithmic units are matched whole before splitting on `/`.
    if matches!(s, "dB/K" | "dBW/Hz" | "dBm/Hz") {
        return atom(s);
    }
    if s.contains(' ') {
        return None;
    }
    let mut out = Unit::one();
    let mut sign = 1;
    let mut token = String::new();
    let flush = |tok: &str, sign: i32, out: Unit| -> Option<Unit> {
        let (sym, pow) = match tok.split_once('^') {
            Some((a, p)) => (a, p.parse::<i32>().ok()?),
            None => (tok, 1),
        };
        let u = if sym == "1" { Unit::one() } else { atom(sym)? };
        Some(out.mul(&u.pow(pow), sign))
    };
    for ch in s.chars() {
        match ch {
            '*' | '/' => {
                if token.is_empty() {
                    return None;
                }
                out = flush(&token, sign, out)?;
                token.clear();
                sign = if ch == '/' { -1 } else { 1 };
            }
            _ => token.push(ch),
        }
    }
    if token.is_empty() {
        return None;
    }
    flush(&token, sign, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_scales_convert() {
        let min = parse_unit("min").unwrap();
        let s = parse_unit("s").unwrap();
        assert_eq!(min.convert(30.0, &s), Some(1800.0));
        let ns = parse_unit("ns").unwrap();
        assert!((parse_unit("us").unwrap().convert(1.0, &ns).unwrap() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn compound_units_reduce() {
        assert_eq!(
            parse_unit("s^2/s").unwrap().dims,
            parse_unit("s").unwrap().dims
        );
        let mps = parse_unit("m/s").unwrap();
        let kmh = parse_unit("km/h").unwrap();
        assert!((kmh.convert(36.0, &mps).unwrap() - 10.0).abs() < 1e-12);
        assert_eq!(parse_unit("ns/s").unwrap().dims, BTreeMap::new());
    }

    #[test]
    fn dimension_mismatch_refuses_conversion() {
        assert!(parse_unit("m")
            .unwrap()
            .convert(1.0, &parse_unit("ns").unwrap())
            .is_none());
        assert!(!parse_unit("1")
            .unwrap()
            .commensurable(&parse_unit("deg").unwrap()));
    }

    #[test]
    fn dimensionless_spellings_all_parse_to_one() {
        for s in [
            "1",
            "dimensionless",
            "fraction (dimensionless)",
            "ratio (dimensionless)",
            "probability",
        ] {
            assert!(parse_unit(s).unwrap().is_dimensionless(), "{s}");
        }
        let pct = parse_unit("%").unwrap();
        assert_eq!(pct.convert(95.0, &Unit::one()), Some(0.95));
    }

    #[test]
    fn logarithmic_units_are_opaque() {
        let dbhz = parse_unit("dB-Hz").unwrap();
        assert!(dbhz.commensurable(&parse_unit("dBHz").unwrap()));
        assert!(!dbhz.commensurable(&parse_unit("dB").unwrap()));
        assert!(!dbhz.commensurable(&parse_unit("Hz").unwrap()));
    }

    #[test]
    fn prose_is_not_a_unit() {
        assert!(parse_unit("mixed - see note").is_none());
        assert!(parse_unit("see the sibling `unit` field").is_none());
        // An unknown well-formed atom is opaque and must match exactly.
        let a = parse_unit("aperture*s").unwrap();
        assert!(!a.commensurable(&parse_unit("s").unwrap()));
        assert!(a.commensurable(&parse_unit("aperture*s").unwrap()));
    }

    #[test]
    fn dimension_string_is_readable() {
        assert_eq!(parse_unit("km/h").unwrap().dimension_string(), "m/s");
        assert_eq!(parse_unit("ns").unwrap().dimension_string(), "s");
        assert_eq!(parse_unit("1").unwrap().dimension_string(), "1");
    }
}
