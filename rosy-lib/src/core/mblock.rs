//! # MBLOCK Runtime Helper
//!
//! Transforms a quadratic matrix to block-diagonal form using
//! real Schur decomposition.
//!
//! Computes orthogonal Q such that Q^T A Q = T is quasi-upper-triangular
//! (block-diagonal with 1×1 and 2×2 blocks).
//!
//! Returns `(Q, Q^{-1})` where Q^{-1} = Q^T (since Q is orthogonal).

use anyhow::Result;

/// Block-diagonalize the n×n leading submatrix of `matrix`.
///
/// Returns `(transform, inverse_transform)` both sized `alloc_dim × alloc_dim`.
/// `inverse_transform^T * matrix * transform` is block-diagonal.
///
/// Uses the same Hessenberg + Francis QR infrastructure as LEV.
pub fn rosy_mblock(
    matrix: &Vec<Vec<f64>>,
    n: usize,
    alloc_dim: usize,
) -> Result<(Vec<Vec<f64>>, Vec<Vec<f64>>)> {
    if n == 0 {
        let empty = vec![vec![0.0; alloc_dim]; alloc_dim];
        return Ok((empty.clone(), empty));
    }

    // Extract n×n working copy
    let mut h = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            h[i][j] = if i < matrix.len() && j < matrix[i].len() {
                matrix[i][j]
            } else {
                0.0
            };
        }
    }

    // Accumulator for similarity transforms
    let mut q = eye(n);

    // 1. Reduce to upper Hessenberg form
    super::lev::hessenberg_reduce_pub(&mut h, &mut q, n);

    // 2. Francis QR iteration → quasi-upper-triangular (real Schur) form
    super::lev::francis_qr_pub(&mut h, &mut q, n)?;

    // Q is the transformation matrix: Q^T * A * Q = T (block-diagonal)
    // Q^{-1} = Q^T for orthogonal Q

    // Pad into alloc_dim × alloc_dim
    let mut transform = vec![vec![0.0; alloc_dim]; alloc_dim];
    let mut inverse = vec![vec![0.0; alloc_dim]; alloc_dim];

    for i in 0..n {
        for j in 0..n {
            transform[i][j] = q[i][j];
            inverse[i][j] = q[j][i]; // transpose
        }
    }

    Ok((transform, inverse))
}

fn eye(n: usize) -> Vec<Vec<f64>> {
    let mut m = vec![vec![0.0; n]; n];
    for i in 0..n { m[i][i] = 1.0; }
    m
}
