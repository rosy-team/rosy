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
    matrix: &impl crate::AsReMat,
    n: impl crate::IntoF64,
    alloc_dim: impl crate::IntoF64,
) -> Result<(Vec<Vec<f64>>, f64)> {
    let matrix = matrix.to_re_mat();
    let n = crate::rosy_as_usize(&n.into_f64());
    let alloc_dim = crate::rosy_as_usize(&alloc_dim.into_f64());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverts_known_2x2() {
        let m = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let (inv, err) = rosy_linv(&m, 2.0, 2.0).unwrap();
        assert_eq!(err, 0.0);
        assert!((inv[0][0] + 2.0).abs() < 1e-9);
        assert!((inv[0][1] - 1.0).abs() < 1e-9);
        assert!((inv[1][0] - 1.5).abs() < 1e-9);
        assert!((inv[1][1] + 0.5).abs() < 1e-9);
    }

    #[test]
    fn identity_is_not_singular() {
        let m = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let (inv, err) = rosy_linv(&m, 2.0, 2.0).unwrap();
        assert_eq!(err, 0.0);
        assert!((inv[0][0] - 1.0).abs() < 1e-12);
        assert!((inv[1][1] - 1.0).abs() < 1e-12);
    }
}
