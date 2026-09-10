//! # MBLOCK Runtime Helper
//!
//! Transforms a real matrix with distinct eigenvalues to 2x2/1x1
//! block-diagonal form using its real eigenvector basis.
//!
//! This follows COSY's `MBLOC`/`EVPREP`: complex conjugate pairs are placed
//! first, real eigenvectors second, and each adjacent pair is symplectically
//! normalized before the inverse is formed.

use anyhow::{Result, bail};

/// Block-diagonalize the n×n leading submatrix of `matrix`.
///
/// Returns `(transform, inverse_transform)` both sized `alloc_dim × alloc_dim`.
/// `inverse_transform * matrix * transform` is block-diagonal.
pub fn rosy_mblock(
    matrix: &impl crate::AsReMat,
    n: impl crate::IntoF64,
    alloc_dim: impl crate::IntoF64,
) -> Result<(Vec<Vec<f64>>, Vec<Vec<f64>>)> {
    let matrix = matrix.to_re_mat();
    let n = crate::rosy_as_usize(&n.into_f64());
    let alloc_dim = crate::rosy_as_usize(&alloc_dim.into_f64());
    if n == 0 {
        let empty = vec![vec![0.0; alloc_dim]; alloc_dim];
        return Ok((empty.clone(), empty));
    }

    let (_eig_real, eig_imag, eigvecs) = super::lev::rosy_lev(&matrix, n, n)?;

    // EVPREP ordering: complete complex pairs first, then real roots.
    let mut columns = Vec::with_capacity(n);
    let mut col = 0;
    while col < n {
        if eig_imag[col].abs() >= 1e-10 && col + 1 < n {
            columns.push(col);
            columns.push(col + 1);
            col += 2;
        } else {
            col += 1;
        }
    }
    for (col, imag) in eig_imag.iter().take(n).enumerate() {
        if imag.abs() < 1e-10 {
            columns.push(col);
        }
    }

    if columns.len() != n {
        bail!("MBLOCK: failed to pair all {} eigenvectors", n);
    }

    let mut transform = vec![vec![0.0; alloc_dim]; alloc_dim];
    for row in 0..n {
        for (dest_col, source_col) in columns.iter().copied().enumerate() {
            transform[row][dest_col] = eigvecs[row][source_col];
        }
    }

    // COSY EVPREP normalization.  Besides making T symplectic for a
    // symplectic input map, the sign of FAC selects ν versus 1-ν.
    for pair_col in (0..n.saturating_sub(1)).step_by(2) {
        let mut factor = 0.0;
        for row in (0..n.saturating_sub(1)).step_by(2) {
            factor += transform[row][pair_col] * transform[row + 1][pair_col + 1]
                - transform[row][pair_col + 1] * transform[row + 1][pair_col];
        }
        if factor.abs() < 1e-10 {
            factor = 1e-10;
        }
        for row in 0..n {
            transform[row][pair_col + 1] /= factor;
        }
    }

    let (inverse, error) = super::linv::rosy_linv(&transform, n, alloc_dim)?;
    if error != 0.0 {
        bail!("MBLOCK: eigenvector matrix is singular");
    }

    Ok((transform, inverse))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coupled_ring_matrix() -> Vec<Vec<f64>> {
        vec![
            vec![-0.3798553, -0.03918255, -0.6712767, -0.06924235],
            vec![2.507634, -0.3798501, 4.431523, -0.6712692],
            vec![0.6712784, 0.06924260, -0.3798552, -0.03918181],
            vec![-4.431506, 0.6712676, 2.507681, -0.3798501],
        ]
    }

    #[test]
    fn coupled_ring_is_block_diagonal_with_cosy_orientation() {
        let matrix = coupled_ring_matrix();
        let (transform, inverse) = rosy_mblock(&matrix, 4.0, 4.0).unwrap();

        let mut blocked = vec![vec![0.0; 4]; 4];
        for row in 0..4 {
            for col in 0..4 {
                for i in 0..4 {
                    for j in 0..4 {
                        blocked[row][col] += inverse[row][i] * matrix[i][j] * transform[j][col];
                    }
                }
            }
        }

        for row in 0..4 {
            for col in 0..4 {
                if row / 2 != col / 2 {
                    assert!(blocked[row][col].abs() < 1e-10);
                }
            }
        }
        assert!(blocked[0][1] < 0.0);
        assert!(blocked[2][3] > 0.0);
    }
}
