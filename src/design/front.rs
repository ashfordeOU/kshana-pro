// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The front, its knee, its hypervolume and the comparison with a reference design.
//!
//! The front and the knee a study reports come from the open engine's own kernel,
//! [`kshana::mcda::pareto`]. The non-dominated sort in this file ([`own_front`],
//! [`rank`]) is used only to select survivors inside the search and to trace the
//! search's progress; a test holds it to agree with the open kernel.

use kshana::mcda::pareto::{dominates, knee_point, pareto_front};
use kshana::mcda::Objective;

/// Indices of the non-dominated `points`, from the open engine's kernel.
pub fn kernel_front(points: &[Vec<f64>], senses: &[Objective]) -> Vec<usize> {
    pareto_front(points, senses)
}

/// The knee of the front of `points`, from the open engine's kernel: the front point
/// farthest from the chord between the two most widely separated front points, in
/// objective space scaled to the front's own range.
pub fn kernel_knee(
    points: &[Vec<f64>],
    senses: &[Objective],
) -> Option<kshana::mcda::pareto::KneePoint> {
    knee_point(points, senses)
}

/// True when `a` dominates `b` (the open kernel's relation).
pub fn kernel_dominates(a: &[f64], b: &[f64], senses: &[Objective]) -> bool {
    dominates(a, b, senses)
}

fn min_form(p: &[f64], senses: &[Objective]) -> Vec<f64> {
    p.iter()
        .zip(senses)
        .map(|(x, s)| s.min_sign() * x)
        .collect()
}

fn dom_min(a: &[f64], b: &[f64]) -> bool {
    let mut strict = false;
    for (x, y) in a.iter().zip(b) {
        if x > y {
            return false;
        }
        if x < y {
            strict = true;
        }
    }
    strict
}

/// Indices of the non-dominated `points`, ascending. The search's own filter.
pub fn own_front(points: &[Vec<f64>], senses: &[Objective]) -> Vec<usize> {
    let m: Vec<Vec<f64>> = points.iter().map(|p| min_form(p, senses)).collect();
    (0..m.len())
        .filter(|&i| !(0..m.len()).any(|j| j != i && dom_min(&m[j], &m[i])))
        .collect()
}

/// One individual as the search ranks it.
#[derive(Debug, Clone, PartialEq)]
pub enum Fitness {
    /// Objectives (as given, not sign-adjusted) and the number of broken result
    /// constraints.
    Scored {
        objectives: Vec<f64>,
        violations: u32,
    },
    /// The run failed; it ranks below every scored individual.
    Failed,
}

/// Constrained domination: a scored individual beats a failed one; fewer broken
/// constraints beat more; with the same number broken, plain domination decides.
fn beats(a: &Fitness, b: &Fitness, senses: &[Objective]) -> bool {
    match (a, b) {
        (Fitness::Scored { .. }, Fitness::Failed) => true,
        (Fitness::Failed, _) => false,
        (
            Fitness::Scored {
                objectives: oa,
                violations: va,
            },
            Fitness::Scored {
                objectives: ob,
                violations: vb,
            },
        ) => {
            if va != vb {
                va < vb
            } else {
                // Plain domination, sign-adjusted so smaller is better on every objective.
                let mut strict = false;
                for ((x, y), s) in oa.iter().zip(ob).zip(senses) {
                    let (x, y) = (s.min_sign() * x, s.min_sign() * y);
                    if x > y {
                        return false;
                    }
                    if x < y {
                        strict = true;
                    }
                }
                strict
            }
        }
    }
}

/// Non-dominated rank of every individual (0 is best): rank 0 is beaten by none, rank
/// 1 only by rank 0, and so on.
pub fn rank(pop: &[Fitness], senses: &[Objective]) -> Vec<usize> {
    let n = pop.len();
    let mut beaten_by = vec![0usize; n];
    let mut beats_list: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in 0..n {
            if i != j && beats(&pop[i], &pop[j], senses) {
                beats_list[i].push(j);
                beaten_by[j] += 1;
            }
        }
    }
    let mut ranks = vec![usize::MAX; n];
    let mut current: Vec<usize> = (0..n).filter(|&i| beaten_by[i] == 0).collect();
    let mut level = 0;
    while !current.is_empty() {
        let mut next = Vec::new();
        for &i in &current {
            ranks[i] = level;
        }
        for &i in &current {
            for &j in &beats_list[i] {
                beaten_by[j] -= 1;
                if beaten_by[j] == 0 {
                    next.push(j);
                }
            }
        }
        next.sort_unstable();
        current = next;
        level += 1;
    }
    ranks
}

/// Crowding distance of each member of one rank (`members` indexes `pop`): the sum over
/// objectives of the gap between its two neighbours, scaled by the objective's range in
/// the rank. Boundary members get infinity.
pub fn crowding(pop: &[Fitness], members: &[usize]) -> Vec<f64> {
    let mut dist = vec![0.0f64; members.len()];
    let objs: Vec<Option<&Vec<f64>>> = members
        .iter()
        .map(|&i| match &pop[i] {
            Fitness::Scored { objectives, .. } => Some(objectives),
            Fitness::Failed => None,
        })
        .collect();
    let m = objs.iter().flatten().map(|o| o.len()).next().unwrap_or(0);
    if members.len() <= 2 || m == 0 || objs.iter().any(|o| o.is_none()) {
        return vec![f64::INFINITY; members.len()];
    }
    for k in 0..m {
        let mut order: Vec<usize> = (0..members.len()).collect();
        order.sort_by(|&a, &b| {
            objs[a].unwrap()[k]
                .total_cmp(&objs[b].unwrap()[k])
                .then(members[a].cmp(&members[b]))
        });
        let lo = objs[order[0]].unwrap()[k];
        let hi = objs[order[order.len() - 1]].unwrap()[k];
        dist[order[0]] = f64::INFINITY;
        dist[order[order.len() - 1]] = f64::INFINITY;
        if hi > lo {
            for w in 1..order.len() - 1 {
                let gap = objs[order[w + 1]].unwrap()[k] - objs[order[w - 1]].unwrap()[k];
                dist[order[w]] += gap / (hi - lo);
            }
        }
    }
    dist
}

// ─── Hypervolume ────────────────────────────────────────────────────────────────

/// The box a hypervolume is measured in: per objective, the best value (`ideal`) and
/// the worst value (`nadir`) among the scored designs of the study, as given (not
/// sign-adjusted).
#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub ideal: Vec<f64>,
    pub nadir: Vec<f64>,
}

impl Bounds {
    /// The bounds of a set of points. `None` for an empty set.
    pub fn of(points: &[Vec<f64>], senses: &[Objective]) -> Option<Bounds> {
        let first = points.first()?;
        let mut ideal = first.clone();
        let mut nadir = first.clone();
        for p in points {
            for k in 0..senses.len() {
                let s = senses[k].min_sign();
                if s * p[k] < s * ideal[k] {
                    ideal[k] = p[k];
                }
                if s * p[k] > s * nadir[k] {
                    nadir[k] = p[k];
                }
            }
        }
        Some(Bounds { ideal, nadir })
    }

    /// A point scaled so 0 is the ideal and 1 the nadir on every objective.
    fn scale(&self, p: &[f64], senses: &[Objective]) -> Vec<f64> {
        (0..senses.len())
            .map(|k| {
                let s = senses[k].min_sign();
                let range = s * self.nadir[k] - s * self.ideal[k];
                if range <= 0.0 {
                    0.0
                } else {
                    ((s * p[k] - s * self.ideal[k]) / range).clamp(0.0, 1.0)
                }
            })
            .collect()
    }
}

fn hv2(points: &mut [(f64, f64)]) -> f64 {
    points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut hv = 0.0;
    let mut floor = 1.0;
    for &(x, y) in points.iter() {
        if y < floor {
            hv += (1.0 - x) * (floor - y);
            floor = y;
        }
    }
    hv
}

/// The share of the box between the ideal and the nadir that `points` dominate: 1 when
/// one point holds the best value of every objective at once, 0 when nothing better
/// than the nadir was found. Exact for two and three objectives.
pub fn hypervolume(points: &[Vec<f64>], senses: &[Objective], bounds: &Bounds) -> f64 {
    let scaled: Vec<Vec<f64>> = points.iter().map(|p| bounds.scale(p, senses)).collect();
    match senses.len() {
        2 => {
            let mut flat: Vec<(f64, f64)> = scaled.iter().map(|p| (p[0], p[1])).collect();
            hv2(&mut flat)
        }
        3 => {
            // Slice along the third objective: between two consecutive levels the
            // dominated area is that of the points at or below the lower level.
            let mut order: Vec<usize> = (0..scaled.len()).collect();
            order.sort_by(|&a, &b| scaled[a][2].total_cmp(&scaled[b][2]).then(a.cmp(&b)));
            let mut hv = 0.0;
            for (n, &i) in order.iter().enumerate() {
                let top = order.get(n + 1).map(|&j| scaled[j][2]).unwrap_or(1.0);
                let depth = top - scaled[i][2];
                if depth <= 0.0 {
                    continue;
                }
                let mut slice: Vec<(f64, f64)> = order[..=n]
                    .iter()
                    .map(|&j| (scaled[j][0], scaled[j][1]))
                    .collect();
                hv += hv2(&mut slice) * depth;
            }
            hv
        }
        _ => f64::NAN,
    }
}

// ─── Reference design ───────────────────────────────────────────────────────────

/// Where a reference design sits against the front.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Positions in the front list of the designs that dominate the reference.
    pub dominated_by: Vec<usize>,
    /// Positions in the front list of the designs the reference dominates (only
    /// possible for a reference from outside the design space, or one that breaks a
    /// constraint).
    pub dominates: Vec<usize>,
    /// Positions in the front list of the designs with exactly the reference's
    /// objective values.
    pub equals: Vec<usize>,
}

impl Placement {
    /// True when no front design dominates the reference.
    pub fn on_front(&self) -> bool {
        self.dominated_by.is_empty()
    }
}

/// Place `reference` against the `front` points (each a vector of objective values).
pub fn place(reference: &[f64], front: &[Vec<f64>], senses: &[Objective]) -> Placement {
    let mut p = Placement {
        dominated_by: Vec::new(),
        dominates: Vec::new(),
        equals: Vec::new(),
    };
    for (i, f) in front.iter().enumerate() {
        if kernel_dominates(f, reference, senses) {
            p.dominated_by.push(i);
        } else if kernel_dominates(reference, f, senses) {
            p.dominates.push(i);
        } else if f.as_slice() == reference {
            p.equals.push(i);
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::mcda::Objective::{Max, Min};
    use rand::{Rng, SeedableRng};

    #[test]
    fn own_front_agrees_with_the_open_kernel() {
        // Random clouds with many ties (coarse values), two and three objectives, every
        // mix of senses: the search's own filter and rank 0 must be the kernel's front.
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(11);
        for senses in [
            vec![Min, Min],
            vec![Min, Max],
            vec![Max, Max],
            vec![Min, Max, Min],
            vec![Max, Max, Max],
        ] {
            for trial in 0..40 {
                let n = 1 + (trial * 7) % 120;
                let points: Vec<Vec<f64>> = (0..n)
                    .map(|_| {
                        senses
                            .iter()
                            .map(|_| f64::from(rng.gen_range(0..12)))
                            .collect()
                    })
                    .collect();
                let kernel = kernel_front(&points, &senses);
                assert_eq!(own_front(&points, &senses), kernel);
                let pop: Vec<Fitness> = points
                    .iter()
                    .map(|p| Fitness::Scored {
                        objectives: p.clone(),
                        violations: 0,
                    })
                    .collect();
                let ranks = rank(&pop, &senses);
                let rank0: Vec<usize> = (0..n).filter(|&i| ranks[i] == 0).collect();
                assert_eq!(rank0, kernel);
                assert!(ranks.iter().all(|&r| r != usize::MAX));
            }
        }
    }

    #[test]
    fn rank_puts_violations_and_failures_last() {
        let s = [Min, Min];
        let pop = vec![
            Fitness::Scored {
                objectives: vec![1.0, 1.0],
                violations: 1,
            },
            Fitness::Scored {
                objectives: vec![5.0, 5.0],
                violations: 0,
            },
            Fitness::Failed,
        ];
        assert_eq!(rank(&pop, &s), vec![1, 0, 2]);
    }

    #[test]
    fn hypervolume_known_answers() {
        let s = [Min, Min];
        let b = Bounds {
            ideal: vec![0.0, 0.0],
            nadir: vec![1.0, 1.0],
        };
        // One point at the ideal dominates the whole box.
        assert!((hypervolume(&[vec![0.0, 0.0]], &s, &b) - 1.0).abs() < 1e-12);
        // One point in the middle dominates a quarter.
        assert!((hypervolume(&[vec![0.5, 0.5]], &s, &b) - 0.25).abs() < 1e-12);
        // A staircase, swept left to right: (0,1) adds nothing, (0.25,0.75) adds
        // 0.75 x 0.25, (0.5,0.5) adds 0.5 x 0.25 and (0.75,0.25) adds 0.25 x 0.25.
        let stairs = vec![
            vec![0.0, 1.0],
            vec![0.25, 0.75],
            vec![0.5, 0.5],
            vec![0.75, 0.25],
        ];
        assert!((hypervolume(&stairs, &s, &b) - 0.375).abs() < 1e-12);
        // Senses flip the box: maximise both, bounds the other way round.
        let smax = [Max, Max];
        let bmax = Bounds {
            ideal: vec![1.0, 1.0],
            nadir: vec![0.0, 0.0],
        };
        assert!((hypervolume(&[vec![0.5, 0.5]], &smax, &bmax) - 0.25).abs() < 1e-12);
        // Three objectives: one point at the centre dominates an eighth. Two points
        // (0.5,0.5,0) and (0,0,0.5): a quarter of the area over the lower half of the
        // depth, then the whole area over the upper half, 0.125 + 0.5.
        let s3 = [Min, Min, Min];
        let b3 = Bounds {
            ideal: vec![0.0; 3],
            nadir: vec![1.0; 3],
        };
        assert!((hypervolume(&[vec![0.5, 0.5, 0.5]], &s3, &b3) - 0.125).abs() < 1e-12);
        let two = vec![vec![0.5, 0.5, 0.0], vec![0.0, 0.0, 0.5]];
        assert!((hypervolume(&two, &s3, &b3) - 0.625).abs() < 1e-12);
    }

    #[test]
    fn a_reference_is_placed_against_the_front() {
        let s = [Min, Max];
        let front = vec![vec![8.0, 20.0], vec![12.0, 55.0], vec![24.0, 100.0]];
        let inside = place(&[14.0, 19.0], &front, &s);
        assert_eq!(inside.dominated_by, vec![0, 1]);
        assert!(!inside.on_front());
        let on = place(&[12.0, 55.0], &front, &s);
        assert!(on.on_front());
        assert_eq!(on.equals, vec![1]);
        let better = place(&[8.0, 60.0], &front, &s);
        assert!(better.on_front());
        assert_eq!(better.dominates, vec![0, 1]);
    }
}
