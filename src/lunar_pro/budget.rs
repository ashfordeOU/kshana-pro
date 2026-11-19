// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! A GUM (JCGM 100:2008) uncertainty budget: named terms, each with its value contribution and
//! its standard-uncertainty contribution `c_i · u(x_i)`, combined by the law of propagation of
//! uncertainty for uncorrelated inputs, `u_c² = Σ (c_i u(x_i))²`.
//!
//! Correlation is handled by name: two budgets that share a term name share the input quantity
//! behind it (for example the TCL secular rate in both a TCL and a TL offset), so their
//! difference ([`Budget::minus`]) takes the difference of the signed contributions for that term
//! (full positive correlation) instead of adding them in quadrature. Every other pair of terms is
//! treated as uncorrelated.
//!
//! [`Budget::monte_carlo_std`] propagates the same budget by sampling (JCGM 101:2008, Monte
//! Carlo method) so a test can check that the analytic sum is right. Evidence label: MODELLED.

use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

/// GUM evaluation type of a term.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TermKind {
    /// Type A: evaluated by statistical analysis of a series of observations.
    A,
    /// Type B: evaluated by other means (published values, model bounds, allocations).
    B,
    /// A defining constant or an exact relation: zero uncertainty by definition.
    Defining,
}

/// The probability distribution assigned to the input quantity behind a term.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Dist {
    /// Normal, standard uncertainty `u`.
    Normal,
    /// Rectangular of half-width `a`, standard uncertainty `u = a / √3`.
    Rectangular,
    /// Exact (no uncertainty).
    Exact,
}

/// One budget term.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Term {
    /// Stable identifier of the input quantity (shared names are correlated).
    pub name: String,
    /// Contribution to the measurand, in the measurand's unit.
    pub value: f64,
    /// Signed standard-uncertainty contribution `c_i · u(x_i)`, in the measurand's unit.
    pub u: f64,
    pub kind: TermKind,
    pub dist: Dist,
    /// Where the value and its uncertainty come from.
    pub source: String,
}

impl Term {
    pub fn exact(name: &str, value: f64, source: &str) -> Self {
        Term {
            name: name.into(),
            value,
            u: 0.0,
            kind: TermKind::Defining,
            dist: Dist::Exact,
            source: source.into(),
        }
    }
    pub fn normal(name: &str, value: f64, u: f64, kind: TermKind, source: &str) -> Self {
        Term {
            name: name.into(),
            value,
            u,
            kind,
            dist: Dist::Normal,
            source: source.into(),
        }
    }
    /// A rectangular term of half-width `half_width` (its standard uncertainty is `a/√3`).
    pub fn rectangular(name: &str, value: f64, half_width: f64, source: &str) -> Self {
        Term {
            name: name.into(),
            value,
            u: half_width / 3f64.sqrt(),
            kind: TermKind::B,
            dist: Dist::Rectangular,
            source: source.into(),
        }
    }
    /// The same term with both contributions multiplied by `k` (a sensitivity coefficient).
    pub fn scaled(&self, k: f64) -> Self {
        Term {
            value: self.value * k,
            u: self.u * k,
            ..self.clone()
        }
    }
}

/// A budget: the measurand is the sum of the term values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub terms: Vec<Term>,
}

impl Budget {
    pub fn new(terms: Vec<Term>) -> Self {
        Budget { terms }
    }
    /// The measurand (sum of the term values).
    pub fn value(&self) -> f64 {
        self.terms.iter().map(|t| t.value).sum()
    }
    /// The combined standard uncertainty `u_c = √(Σ u_i²)`.
    pub fn u_c(&self) -> f64 {
        self.terms.iter().map(|t| t.u * t.u).sum::<f64>().sqrt()
    }
    /// The expanded uncertainty `U = k u_c`.
    pub fn expanded(&self, k: f64) -> f64 {
        k * self.u_c()
    }
    pub fn push(&mut self, t: Term) {
        self.terms.push(t);
    }
    /// Add a term, or merge it into an existing term of the same name (correlated).
    pub fn add_merge(&mut self, t: Term) {
        if let Some(e) = self.terms.iter_mut().find(|e| e.name == t.name) {
            e.value += t.value;
            e.u += t.u;
        } else {
            self.terms.push(t);
        }
    }
    /// `self − other`, same-named terms fully correlated.
    pub fn minus(&self, other: &Budget) -> Budget {
        let mut out = self.clone();
        for t in &other.terms {
            out.add_merge(t.scaled(-1.0));
        }
        out
    }
    /// The whole budget multiplied by `k`.
    pub fn scaled(&self, k: f64) -> Budget {
        Budget {
            terms: self.terms.iter().map(|t| t.scaled(k)).collect(),
        }
    }
    /// The term named `name`, if present.
    pub fn term(&self, name: &str) -> Option<&Term> {
        self.terms.iter().find(|t| t.name == name)
    }
    /// The standard deviation of the measurand from `n` Monte Carlo draws (JCGM 101): each input
    /// drawn from its distribution, scaled by its signed contribution.
    pub fn monte_carlo_std(&self, n: usize, seed: u64) -> f64 {
        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(seed);
        let mut s = 0.0;
        let mut s2 = 0.0;
        for _ in 0..n {
            let mut y = 0.0;
            for t in &self.terms {
                let z = match t.dist {
                    Dist::Exact => 0.0,
                    Dist::Rectangular => 3f64.sqrt() * rng.gen_range(-1.0..1.0),
                    Dist::Normal => std_normal(&mut rng),
                };
                y += t.u * z;
            }
            s += y;
            s2 += y * y;
        }
        let nf = n as f64;
        ((s2 - s * s / nf) / (nf - 1.0)).max(0.0).sqrt()
    }
}

/// A standard normal draw (Box–Muller).
pub fn std_normal<R: Rng>(rng: &mut R) -> f64 {
    let u1: f64 = rng.gen_range(f64::MIN_POSITIVE..1.0);
    let u2: f64 = rng.gen();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}
