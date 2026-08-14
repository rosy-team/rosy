//! LDET - Matrix determinant computation.
//!
//! Computes the determinant of an n×n matrix using LU decomposition
//! with partial pivoting. The matrix is stored as a `Vec<Vec<f64>>`.

use anyhow::{Result, bail};

/// Compute the determinant of an n×n matrix using LU decomposition
/// with partial pivoting.
///
/// Arguments:
/// - `matrix`: the input matrix (`Vec<Vec<f64>>`), allocation dimension × allocation dimension
/// - `n`: number of actual rows/columns to use
/// - `alloc_dim`: allocation dimension (used for indexing, 1-based in COSY convention)
/// - `det`: output determinant value (written in-place)
pub fn rosy_ldet(
    matrix: &Vec<Vec<f64>>,
    n: usize,
    _alloc_dim: usize,
) -> Result<f64> {
    if n == 0 {
        return Ok(1.0);
    }
    if matrix.len() < n {
        bail!("LDET: matrix has fewer rows ({}) than n ({})", matrix.len(), n);
    }
    for row in matrix.iter().take(n) {
        if row.len() < n {
            bail!("LDET: matrix row has fewer columns ({}) than n ({})", row.len(), n);
        }
    }

    // Copy the relevant submatrix into a working buffer
    let mut a: Vec<Vec<f64>> = (0..n)
        .map(|i| matrix[i][..n].to_vec())
        .collect();

    let mut det = 1.0f64;
    let mut sign = 1.0f64;

    for col in 0..n {
        // Find pivot row (largest absolute value in column)
        let mut max_row = col;
        let mut max_val = a[col][col].abs();
        for row in (col + 1)..n {
            let v = a[row][col].abs();
            if v > max_val {
                max_val = v;
                max_row = row;
            }
        }

        // Swap rows if necessary
        if max_row != col {
            a.swap(col, max_row);
            sign = -sign;
        }

        let pivot = a[col][col];
        if pivot.abs() < 1e-15 {
            // Singular matrix — determinant is zero
            return Ok(0.0);
        }

        det *= pivot;

        // Eliminate below the pivot
        for row in (col + 1)..n {
            let factor = a[row][col] / pivot;
            for c in col..n {
                let v = a[col][c];
                a[row][c] -= factor * v;
            }
        }
    }

    Ok(det * sign)
}
