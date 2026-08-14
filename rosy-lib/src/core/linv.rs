//! # LINV Runtime Helper
//!
//! Inverts a quadratic matrix using Gaussian elimination with partial pivoting.
//!
//! ## Arguments
//! - `matrix`: input matrix as `Vec<Vec<f64>>` (row-major)
//! - `n`: number of actual entries (dimension)
//! - `alloc_dim`: allocation dimension (used for indexing into the padded matrix)
//!
//! ## Returns
//! `(inverse: Vec<Vec<f64>>, error_flag: f64)`
//! where `error_flag` is `0.0` on success and `132.0` if the matrix is singular.

use anyhow::Result;

/// Invert an `n x n` submatrix of `matrix` (which may be allocated as `alloc_dim x alloc_dim`).
///
/// Returns `(inverse, error_flag)` where `error_flag` is `0.0` on success or `132.0` if singular.
pub fn rosy_linv(
    matrix: &Vec<Vec<f64>>,
    n: usize,
    alloc_dim: usize,
) -> Result<(Vec<Vec<f64>>, f64)> {
    // Build augmented matrix [A | I] of size n x 2n
    let mut aug: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut row = Vec::with_capacity(2 * n);
            for j in 0..n {
                let val = if i < matrix.len() && j < matrix[i].len() {
                    matrix[i][j]
                } else {
                    0.0
                };
                row.push(val);
            }
            for j in 0..n {
                row.push(if i == j { 1.0 } else { 0.0 });
            }
            row
        })
        .collect();

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot row
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..n {
            if aug[row][col].abs() > max_val {
                max_val = aug[row][col].abs();
                max_row = row;
            }
        }

        // Swap rows
        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        if pivot.abs() < 1e-12 {
            // Singular matrix
            let inv = vec![vec![0.0; alloc_dim]; alloc_dim];
            return Ok((inv, 132.0));
        }

        // Scale pivot row
        let pivot_inv = 1.0 / pivot;
        for j in 0..(2 * n) {
            aug[col][j] *= pivot_inv;
        }

        // Eliminate column
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = aug[row][col];
            if factor == 0.0 {
                continue;
            }
            for j in 0..(2 * n) {
                let subtract = factor * aug[col][j];
                aug[row][j] -= subtract;
            }
        }
    }

    // Extract the inverse from the right half of the augmented matrix
    // Result is allocated as alloc_dim x alloc_dim (zero-padded beyond n)
    let mut inv = vec![vec![0.0_f64; alloc_dim]; alloc_dim];
    for i in 0..n {
        for j in 0..n {
            inv[i][j] = aug[i][n + j];
        }
    }

    Ok((inv, 0.0))
}
