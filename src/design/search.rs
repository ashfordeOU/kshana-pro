// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The search: exhaustive enumeration when the feasible space fits the budget, and a
//! seeded NSGA-II (the elitist non-dominated sorting genetic algorithm of Deb, Pratap,
//! Agarwal and Meyarivan, IEEE Transactions on Evolutionary Computation 6(2), 2002) when
//! it does not.
//!
//! The kernel knows nothing about scenarios. A design is a vector of level indices; the
//! caller supplies a feasibility test and a batch scorer. Random numbers come from one
//! ChaCha stream seeded by the study, and scores are consumed in the order the designs
//! were generated, so a search is a pure function of its inputs.
//!
//! The search never spends a run on a design it has already evaluated: an offspring
//! that repeats an evaluated design is mutated again, and if that does not give a new
//! design an unexplored feasible design is taken instead. With a budget that covers the
//! whole feasible space the search therefore ends having run every feasible design.

use std::collections::{HashMap, HashSet};

use kshana::mcda::Objective;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::front::{crowding, rank, Fitness};

/// Scores a batch of designs: one [`Fitness`] per design, in order.
pub type Scorer<'a> = dyn FnMut(&[Vec<usize>]) -> Vec<Fitness> + 'a;

/// The shape of a search problem.
pub struct Problem<'a> {
    /// Number of levels of each variable.
    pub levels: Vec<usize>,
    /// Per variable: true when neighbouring levels are close in value (a range), false
    /// for an unordered list of choices.
    pub ordered: Vec<bool>,
    pub senses: Vec<Objective>,
    /// The design constraints: a design that fails is never evaluated.
    pub feasible: &'a (dyn Fn(&[usize]) -> bool + Sync),
    /// Every feasible design, when the space is small enough to list; used to draw
    /// unexplored designs without replacement.
    pub all_feasible: Option<Vec<Vec<usize>>>,
}

/// Search settings.
#[derive(Debug, Clone)]
pub struct Config {
    pub population: usize,
    pub generations: usize,
    /// The most evaluations allowed.
    pub budget: usize,
    pub seed: u64,
}

/// Everything a search did, in evaluation order.
#[derive(Debug, Clone, Default)]
pub struct SearchLog {
    pub designs: Vec<Vec<usize>>,
    pub fitness: Vec<Fitness>,
    /// The generation each design was evaluated in (0 is the initial population).
    pub generation: Vec<usize>,
    /// Candidates that repeated an evaluated design and so cost no run.
    pub cache_hits: usize,
    /// Distinct candidates rejected by a design constraint, never evaluated.
    pub infeasible: usize,
    /// Generations completed after the initial population.
    pub generations_run: usize,
    /// True when no unexplored feasible design was left to evaluate.
    pub space_exhausted: bool,
}

/// Distribution index of the simulated binary crossover.
const ETA_CROSSOVER: f64 = 15.0;
/// Distribution index of the polynomial mutation.
const ETA_MUTATION: f64 = 20.0;
/// Probability that a pair of parents is crossed.
const P_CROSSOVER: f64 = 0.9;
/// Attempts to turn a repeated or infeasible offspring into a new feasible design
/// before an unexplored design is taken instead.
const REPAIR_TRIES: usize = 8;
/// Attempts to draw an unexplored feasible design at random when the space is too
/// large to list.
const SAMPLE_TRIES: usize = 4000;

struct State<'a, 'p> {
    problem: &'a Problem<'p>,
    rng: ChaCha8Rng,
    seen: HashMap<Vec<usize>, usize>,
    rejected: HashSet<Vec<usize>>,
    /// Shuffled feasible designs not yet handed out (listable spaces only).
    pool: Option<Vec<Vec<usize>>>,
    log: SearchLog,
}

impl State<'_, '_> {
    fn random_design(&mut self) -> Vec<usize> {
        self.problem
            .levels
            .iter()
            .map(|&n| self.rng.gen_range(0..n))
            .collect()
    }

    /// An unexplored feasible design not already in `batch`, or `None` when none can
    /// be found.
    fn fresh(&mut self, batch: &HashSet<Vec<usize>>) -> Option<Vec<usize>> {
        if let Some(pool) = &mut self.pool {
            while let Some(d) = pool.pop() {
                if !self.seen.contains_key(&d) && !batch.contains(&d) {
                    return Some(d);
                }
            }
            return None;
        }
        for _ in 0..SAMPLE_TRIES {
            let d = self.random_design();
            if !(self.problem.feasible)(&d) {
                if self.rejected.insert(d) {
                    self.log.infeasible += 1;
                }
                continue;
            }
            if !self.seen.contains_key(&d) && !batch.contains(&d) {
                return Some(d);
            }
        }
        None
    }

    /// Polynomial mutation of one ordered gene, on the level index. A move that
    /// rounds back to the same level becomes one step up or down, so a mutation on a
    /// coarse grid always changes the design.
    fn mutate_gene(&mut self, value: usize, var: usize) -> usize {
        let n = self.problem.levels[var];
        if n <= 1 {
            return value;
        }
        if !self.problem.ordered[var] {
            let mut other = self.rng.gen_range(0..n - 1);
            if other >= value {
                other += 1;
            }
            return other;
        }
        let hi = (n - 1) as f64;
        let x = value as f64;
        let u: f64 = self.rng.gen();
        let delta = if u < 0.5 {
            (2.0 * u).powf(1.0 / (ETA_MUTATION + 1.0)) - 1.0
        } else {
            1.0 - (2.0 * (1.0 - u)).powf(1.0 / (ETA_MUTATION + 1.0))
        };
        let moved = (x + delta * hi).round().clamp(0.0, hi) as usize;
        if moved != value {
            return moved;
        }
        if value == 0 {
            1
        } else if value == n - 1 || u < 0.5 {
            value - 1
        } else {
            value + 1
        }
    }

    fn mutate(&mut self, d: &mut [usize]) {
        let p = 1.0 / d.len() as f64;
        for (var, gene) in d.iter_mut().enumerate() {
            if self.rng.gen::<f64>() < p {
                *gene = self.mutate_gene(*gene, var);
            }
        }
    }

    /// Simulated binary crossover (Deb and Agrawal, 1995) on the level indices of ordered
    /// genes, with its per-gene exchange; a plain swap for unordered ones.
    fn crossover(&mut self, a: &[usize], b: &[usize]) -> (Vec<usize>, Vec<usize>) {
        let (mut c1, mut c2) = (a.to_vec(), b.to_vec());
        if self.rng.gen::<f64>() >= P_CROSSOVER {
            return (c1, c2);
        }
        for var in 0..a.len() {
            if self.rng.gen::<f64>() >= 0.5 {
                continue;
            }
            if !self.problem.ordered[var] {
                std::mem::swap(&mut c1[var], &mut c2[var]);
                continue;
            }
            let hi = (self.problem.levels[var] - 1) as f64;
            let (x1, x2) = (a[var] as f64, b[var] as f64);
            let u: f64 = self.rng.gen();
            let beta = if u <= 0.5 {
                (2.0 * u).powf(1.0 / (ETA_CROSSOVER + 1.0))
            } else {
                (1.0 / (2.0 * (1.0 - u))).powf(1.0 / (ETA_CROSSOVER + 1.0))
            };
            let y1 = 0.5 * ((1.0 + beta) * x1 + (1.0 - beta) * x2);
            let y2 = 0.5 * ((1.0 - beta) * x1 + (1.0 + beta) * x2);
            c1[var] = y1.round().clamp(0.0, hi) as usize;
            c2[var] = y2.round().clamp(0.0, hi) as usize;
            // The exchange step of the published operator: which child takes which
            // value is a coin toss. Without it each child stays next to one parent
            // on every gene and the crossover recombines nothing.
            if self.rng.gen::<f64>() < 0.5 {
                std::mem::swap(&mut c1[var], &mut c2[var]);
            }
        }
        (c1, c2)
    }

    /// Turn `child` into a new feasible design for `batch`, or `None` when the space
    /// has no unexplored design left.
    fn admit(&mut self, child: Vec<usize>, batch: &HashSet<Vec<usize>>) -> Option<Vec<usize>> {
        let mut candidate = child.clone();
        for attempt in 0..=REPAIR_TRIES {
            if !(self.problem.feasible)(&candidate) {
                if self.rejected.insert(candidate.clone()) {
                    self.log.infeasible += 1;
                }
            } else if self.seen.contains_key(&candidate) {
                self.log.cache_hits += 1;
            } else if !batch.contains(&candidate) {
                return Some(candidate);
            }
            if attempt < REPAIR_TRIES {
                candidate = child.clone();
                let var = self.rng.gen_range(0..candidate.len());
                candidate[var] = self.mutate_gene(candidate[var], var);
                if attempt >= REPAIR_TRIES / 2 {
                    // A second gene: a lone change rarely repairs a broken pattern.
                    let var = self.rng.gen_range(0..candidate.len());
                    candidate[var] = self.mutate_gene(candidate[var], var);
                }
            }
        }
        self.fresh(batch)
    }

    fn record(
        &mut self,
        batch: Vec<Vec<usize>>,
        generation: usize,
        score_batch: &mut Scorer<'_>,
    ) -> Vec<usize> {
        let scores = score_batch(&batch);
        assert_eq!(scores.len(), batch.len(), "one score per design");
        let mut ids = Vec::with_capacity(batch.len());
        for (d, f) in batch.into_iter().zip(scores) {
            let id = self.log.designs.len();
            self.seen.insert(d.clone(), id);
            self.log.designs.push(d);
            self.log.fitness.push(f);
            self.log.generation.push(generation);
            ids.push(id);
        }
        ids
    }
}

/// Rank and crowding distance of each member of `ids` (indices into the log).
fn rank_and_crowd(log: &SearchLog, ids: &[usize], senses: &[Objective]) -> (Vec<usize>, Vec<f64>) {
    let pop: Vec<Fitness> = ids.iter().map(|&i| log.fitness[i].clone()).collect();
    let ranks = rank(&pop, senses);
    let mut crowd = vec![0.0; ids.len()];
    let top = ranks.iter().copied().max().unwrap_or(0);
    for level in 0..=top {
        let members: Vec<usize> = (0..ids.len()).filter(|&i| ranks[i] == level).collect();
        for (m, d) in members.iter().zip(crowding(&pop, &members)) {
            crowd[*m] = d;
        }
    }
    (ranks, crowd)
}

/// Run NSGA-II. `score_batch` receives each batch of new designs and returns one
/// fitness per design, in order.
pub fn nsga2(problem: &Problem<'_>, cfg: &Config, score_batch: &mut Scorer<'_>) -> SearchLog {
    let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed);
    let pool = problem.all_feasible.clone().map(|mut all| {
        all.shuffle(&mut rng);
        all
    });
    let mut st = State {
        problem,
        rng,
        seen: HashMap::new(),
        rejected: HashSet::new(),
        pool,
        log: SearchLog::default(),
    };
    let population = cfg.population.max(2);

    // Initial population: unexplored feasible designs drawn at random.
    let mut batch_set = HashSet::new();
    let mut batch = Vec::new();
    while batch.len() < population.min(cfg.budget) {
        match st.fresh(&batch_set) {
            Some(d) => {
                batch_set.insert(d.clone());
                batch.push(d);
            }
            None => {
                st.log.space_exhausted = true;
                break;
            }
        }
    }
    let mut pop = st.record(batch, 0, score_batch);

    for generation in 1..=cfg.generations {
        let left = cfg.budget.saturating_sub(st.log.designs.len());
        if left == 0 || st.log.space_exhausted || pop.len() < 2 {
            break;
        }
        let (ranks, crowd) = rank_and_crowd(&st.log, &pop, &problem.senses);
        let tournament = |rng: &mut ChaCha8Rng| -> usize {
            let a = rng.gen_range(0..pop.len());
            let b = rng.gen_range(0..pop.len());
            let a_wins = ranks[a] < ranks[b]
                || (ranks[a] == ranks[b]
                    && (crowd[a] > crowd[b] || (crowd[a] == crowd[b] && a <= b)));
            if a_wins {
                a
            } else {
                b
            }
        };
        let want = population.min(left);
        let mut batch_set = HashSet::new();
        let mut batch = Vec::new();
        'fill: while batch.len() < want {
            let p1 = st.log.designs[pop[tournament(&mut st.rng)]].clone();
            let p2 = st.log.designs[pop[tournament(&mut st.rng)]].clone();
            let (mut c1, mut c2) = st.crossover(&p1, &p2);
            st.mutate(&mut c1);
            st.mutate(&mut c2);
            for child in [c1, c2] {
                if batch.len() >= want {
                    break;
                }
                match st.admit(child, &batch_set) {
                    Some(d) => {
                        batch_set.insert(d.clone());
                        batch.push(d);
                    }
                    None => {
                        st.log.space_exhausted = true;
                        break 'fill;
                    }
                }
            }
        }
        if batch.is_empty() {
            break;
        }
        let offspring = st.record(batch, generation, score_batch);
        st.log.generations_run = generation;

        // Survivors: the best `population` of parents and offspring together.
        let mut merged = pop.clone();
        merged.extend(offspring);
        let (ranks, crowd) = rank_and_crowd(&st.log, &merged, &problem.senses);
        let mut order: Vec<usize> = (0..merged.len()).collect();
        order.sort_by(|&a, &b| {
            ranks[a]
                .cmp(&ranks[b])
                .then(crowd[b].total_cmp(&crowd[a]))
                .then(merged[a].cmp(&merged[b]))
        });
        pop = order
            .into_iter()
            .take(population)
            .map(|i| merged[i])
            .collect();
    }
    st.log
}

/// Score every feasible design, in design order.
pub fn exhaustive(designs: Vec<Vec<usize>>, score_batch: &mut Scorer<'_>) -> SearchLog {
    let fitness = score_batch(&designs);
    assert_eq!(fitness.len(), designs.len(), "one score per design");
    SearchLog {
        generation: vec![0; designs.len()],
        designs,
        fitness,
        cache_hits: 0,
        infeasible: 0,
        generations_run: 0,
        space_exhausted: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::mcda::Objective::Min;

    fn toy() -> (Vec<usize>, Vec<bool>) {
        (vec![6, 6, 3], vec![true, true, false])
    }

    fn toy_scores(batch: &[Vec<usize>]) -> Vec<Fitness> {
        batch
            .iter()
            .map(|d| Fitness::Scored {
                objectives: vec![d[0] as f64 + d[2] as f64, (5 - d[0]) as f64 + d[1] as f64],
                violations: 0,
            })
            .collect()
    }

    #[test]
    fn a_budget_that_covers_the_space_runs_every_feasible_design_once() {
        let (levels, ordered) = toy();
        let feasible = |d: &[usize]| (d[0] + d[1]) % 2 == 0;
        let mut all = Vec::new();
        for a in 0..6 {
            for b in 0..6 {
                for c in 0..3 {
                    if feasible(&[a, b, c]) {
                        all.push(vec![a, b, c]);
                    }
                }
            }
        }
        let n = all.len();
        assert_eq!(n, 54);
        let problem = Problem {
            levels,
            ordered,
            senses: vec![Min, Min],
            feasible: &feasible,
            all_feasible: Some(all.clone()),
        };
        let cfg = Config {
            population: 10,
            generations: 50,
            budget: 1000,
            seed: 3,
        };
        let log = nsga2(&problem, &cfg, &mut toy_scores);
        assert_eq!(log.designs.len(), n);
        assert!(log.space_exhausted);
        let mut got = log.designs.clone();
        got.sort();
        all.sort();
        assert_eq!(got, all, "every feasible design exactly once");
        assert!(log.designs.iter().all(|d| feasible(d)));
    }

    #[test]
    fn the_budget_is_never_exceeded_and_the_seed_decides_the_path() {
        let (levels, ordered) = toy();
        let feasible = |_: &[usize]| true;
        let run = |seed: u64, budget: usize| {
            let problem = Problem {
                levels: levels.clone(),
                ordered: ordered.clone(),
                senses: vec![Min, Min],
                feasible: &feasible,
                all_feasible: None,
            };
            let cfg = Config {
                population: 8,
                generations: 40,
                budget,
                seed,
            };
            nsga2(&problem, &cfg, &mut toy_scores)
        };
        let a = run(1, 37);
        assert_eq!(a.designs.len(), 37);
        let unique: HashSet<&Vec<usize>> = a.designs.iter().collect();
        assert_eq!(unique.len(), 37, "no design is evaluated twice");
        assert_eq!(a.designs, run(1, 37).designs);
        assert_ne!(a.designs, run(2, 37).designs);
    }
}
