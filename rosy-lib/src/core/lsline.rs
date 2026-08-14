//! # LSLINE Runtime Helper
//!
//! Computes the least-squares fit line y = a*x + b for n pairs of (x, y) values.
//!
//! ## Arguments
//! - `x`: x-values as `Vec<f64>`
//! - `y`: y-values as `Vec<f64>`
//! - `n`: number of pairs to use (as usize)
//!
//! ## Returns
//! `(a, b)` — slope and intercept of the best-fit line

use anyhow::{Result, bail};

/// Compute least-squares linear fit y = a*x + b.
///
/// Returns `(a, b)` where `a` is the slope and `b` is the intercept.
pub fn rosy_lsline(x: &Vec<f64>, y: &Vec<f64>, n: usize) -> Result<(f64, f64)> {
    if n == 0 {
        bail!("LSLINE: n must be greater than 0");
    }
    if x.len() < n {
        bail!("LSLINE: x array has fewer elements ({}) than n ({})", x.len(), n);
    }
    if y.len() < n {
        bail!("LSLINE: y array has fewer elements ({}) than n ({})", y.len(), n);
    }

    let fn_ = n as f64;
    let sum_x: f64 = x[..n].iter().sum();
    let sum_y: f64 = y[..n].iter().sum();
    let sum_xy: f64 = x[..n].iter().zip(y[..n].iter()).map(|(xi, yi)| xi * yi).sum();
    let sum_x2: f64 = x[..n].iter().map(|xi| xi * xi).sum();

    let denom = fn_ * sum_x2 - sum_x * sum_x;
    if denom.abs() < 1e-15 {
        bail!("LSLINE: degenerate x data (all x values are equal)");
    }

    let a = (fn_ * sum_xy - sum_x * sum_y) / denom;
    let b = (sum_y - a * sum_x) / fn_;

    Ok((a, b))
}
