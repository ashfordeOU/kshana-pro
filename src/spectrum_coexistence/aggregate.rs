// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The aggregation arithmetic, and nothing else.
//!
//! The effective carrier-to-noise density of a victim receiver is `C / (N0 + I)`, with
//! `I` the sum over the interfering satellites of received power times the spectral
//! separation coefficient (SSC) (International Telecommunication Union Recommendation
//! ITU-R M.1831; Betz 2001; Kaplan and Hegarty, *Understanding GPS/GNSS*, 3rd edition,
//! section 9.4). The loss of C/N0 is therefore `10 log10(1 + I/N0)`. Every input to
//! these functions (a power, an SSC, a noise density, a count of satellites in view) is
//! read from an open-engine run; these functions only add them up.

/// The single-satellite interference-to-noise ratio `P kappa / N0`, in the same form
/// and order the open `leo-signal` kind uses for its own single-satellite loss.
pub fn single_ratio(power_dbw: f64, ssc_db_per_hz: f64, noise_density_dbw_per_hz: f64) -> f64 {
    10f64.powf(power_dbw / 10.0) * 10f64.powf(ssc_db_per_hz / 10.0)
        / 10f64.powf(noise_density_dbw_per_hz / 10.0)
}

/// A ratio scaled from one received power to another.
pub fn scale_to_power(ratio: f64, from_dbw: f64, to_dbw: f64) -> f64 {
    ratio * 10f64.powf((to_dbw - from_dbw) / 10.0)
}

/// Loss of C/N0 (dB) for an interference-to-noise ratio: `10 log10(1 + ratio)`.
pub fn loss_db(ratio: f64) -> f64 {
    10.0 * (1.0 + ratio).log10()
}

/// A power ratio in decibels.
pub fn ratio_db(ratio: f64) -> f64 {
    10.0 * ratio.log10()
}

/// The worst cell of one aggregated grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Worst {
    pub lat_index: usize,
    pub lon_index: usize,
    /// Satellites in view at that cell (the count the grid was aggregated with).
    pub in_view: f64,
    /// Aggregate interference-to-noise ratio there.
    pub ratio: f64,
    /// Loss of C/N0 there (dB).
    pub loss_db: f64,
}

/// Aggregate a per-satellite ratio over a grid of satellites in view: per cell
/// `ratio = count x per_satellite` and `loss = 10 log10(1 + ratio)`. A cell with no
/// satellite in view has no interferer and carries `None` (the caller states why).
/// The worst cell is the first cell, in row-major order, with the largest ratio;
/// `None` when no cell has a satellite in view.
pub fn aggregate_grid<C: Copy + Into<f64>>(
    counts: &[Vec<C>],
    per_satellite: f64,
) -> (Vec<Vec<Option<f64>>>, Option<Worst>) {
    let mut worst: Option<Worst> = None;
    let grid = counts
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .enumerate()
                .map(|(j, &c)| {
                    let n: f64 = c.into();
                    if n <= 0.0 {
                        return None;
                    }
                    let ratio = n * per_satellite;
                    let loss = loss_db(ratio);
                    if worst.is_none_or(|w| ratio > w.ratio) {
                        worst = Some(Worst {
                            lat_index: i,
                            lon_index: j,
                            in_view: n,
                            ratio,
                            loss_db: loss,
                        });
                    }
                    Some(loss)
                })
                .collect()
        })
        .collect();
    (grid, worst)
}

/// A counts grid of whole satellites as `f64` (for [`aggregate_grid`]).
pub fn as_f64(counts: &[Vec<u64>]) -> Vec<Vec<f64>> {
    counts
        .iter()
        .map(|r| r.iter().map(|&c| c as f64).collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loss_of_zero_interference_is_zero_and_equal_power_is_three_db() {
        assert_eq!(loss_db(0.0), 0.0);
        assert!((loss_db(1.0) - 3.010_299_956_639_812).abs() < 1e-12);
    }

    #[test]
    fn single_ratio_is_power_times_ssc_over_noise() {
        // -150 dBW, -90 dB/Hz, -200 dBW/Hz: 10^(-15) * 10^(-9) / 10^(-20) = 1e-4.
        assert!((single_ratio(-150.0, -90.0, -200.0) - 1e-4).abs() < 1e-16);
    }

    #[test]
    fn scaling_by_3_0103_db_doubles_the_ratio() {
        let r = 2.5e-5;
        let s = scale_to_power(r, -150.0, -150.0 + 10.0 * 2f64.log10());
        assert!((s / r - 2.0).abs() < 1e-12);
    }

    #[test]
    fn grid_skips_empty_cells_and_finds_the_first_worst() {
        let counts = vec![vec![0.0, 2.0], vec![3.0, 3.0]];
        let (g, w) = aggregate_grid(&counts, 0.1);
        assert_eq!(g[0][0], None);
        assert!((g[0][1].unwrap() - loss_db(0.2)).abs() < 1e-15);
        let w = w.unwrap();
        assert_eq!((w.lat_index, w.lon_index), (1, 0));
        assert_eq!(w.in_view, 3.0);
    }

    #[test]
    fn a_grid_with_nothing_in_view_has_no_worst_cell() {
        let (g, w) = aggregate_grid(&[vec![0.0, 0.0]], 1.0);
        assert!(g[0].iter().all(Option::is_none));
        assert!(w.is_none());
    }
}
