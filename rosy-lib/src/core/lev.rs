//! # LEV Runtime Helper
//!
//! Computes eigenvalues and eigenvectors of a real matrix using
//! the Francis QR algorithm (implicit double shift).
//!
//! 1. Reduce to upper Hessenberg form via Householder reflections.
//! 2. Iterate QR steps with implicit double shifts until convergence.
//! 3. Extract eigenvalues; back-substitute for eigenvectors.
//!
//! When the i-th eigenvalue is complex (positive imaginary part),
//! columns i and i+1 of V contain the real and imaginary parts of
//! the corresponding eigenvector (COSY convention).

use anyhow::Result;
use num_complex::Complex64;

/// Compute eigenvalues and eigenvectors of the n×n leading submatrix of `matrix`.
///
/// Returns `(eig_real, eig_imag, eigvecs)` where:
/// - `eig_real[i]`, `eig_imag[i]` are the real/imaginary parts of the i-th eigenvalue
/// - `eigvecs` is `alloc_dim × alloc_dim`, columns hold eigenvectors
///
/// For a real eigenvalue, column i of V is the eigenvector.
/// For a complex pair (λ, λ̄) at indices i, i+1: column i = Re(v), column i+1 = Im(v).
pub fn rosy_lev(
    matrix: &Vec<Vec<f64>>,
    n: usize,
    alloc_dim: usize,
) -> Result<(Vec<f64>, Vec<f64>, Vec<Vec<f64>>)> {
    if n == 0 {
        let empty_matrix = vec![vec![0.0; alloc_dim]; alloc_dim];
        return Ok((vec![0.0; alloc_dim], vec![0.0; alloc_dim], empty_matrix));
    }

    // Extract n×n working copy.  Keep the original matrix because the
    // eigenvectors are most robustly recovered from (A - lambda I)v = 0
    // after the QR iteration has supplied the eigenvalues.
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
    let original = h.clone();

    // Accumulator for similarity transforms (will hold eigenvectors of original matrix)
    let mut q_accum = eye(n);

    // 1. Reduce to upper Hessenberg form: Q^T A Q = H
    hessenberg_reduce(&mut h, &mut q_accum, n);

    // 2. Francis QR iteration on H, accumulating transforms into q_accum
    francis_qr(&mut h, &mut q_accum, n)?;

    // 3. Extract eigenvalues from the quasi-upper-triangular H
    let mut eig_real = vec![0.0; alloc_dim];
    let mut eig_imag = vec![0.0; alloc_dim];
    extract_eigenvalues(&h, n, &mut eig_real, &mut eig_imag);

    // 4. Recover eigenvectors directly from the original matrix.  A real
    // Schur 2x2 block is not generally of the special [[a,b],[-b,a]] form,
    // so seeding its real/imaginary vectors as coordinate axes (the former
    // implementation) does not solve the eigenvector equation.
    let mut eigvecs = vec![vec![0.0; alloc_dim]; alloc_dim];
    let mut col = 0;
    while col < n {
        let lambda = Complex64::new(eig_real[col], eig_imag[col]);
        let vector = complex_null_vector(&original, lambda);
        if eig_imag[col].abs() >= 1e-14 && col + 1 < n {
            for row in 0..n {
                eigvecs[row][col] = vector[row].re;
                eigvecs[row][col + 1] = vector[row].im;
            }
            col += 2;
        } else {
            for row in 0..n {
                eigvecs[row][col] = vector[row].re;
            }
            col += 1;
        }
    }

    Ok((eig_real, eig_imag, eigvecs))
}

/// Find a right null vector of `A - lambda I` by rank-revealing Gaussian
/// elimination.  MBLOCK requires distinct eigenvalues, so forcing at most
/// `n - 1` pivots leaves the one-dimensional eigenspace as the free column.
fn complex_null_vector(matrix: &[Vec<f64>], lambda: Complex64) -> Vec<Complex64> {
    let n = matrix.len();
    if n == 0 {
        return Vec::new();
    }

    let mut a = vec![vec![Complex64::new(0.0, 0.0); n]; n];
    let mut scale = 0.0_f64;
    for row in 0..n {
        for col in 0..n {
            a[row][col] = Complex64::new(matrix[row][col], 0.0);
            if row == col {
                a[row][col] -= lambda;
            }
            scale = scale.max(a[row][col].norm());
        }
    }

    let tolerance = 1e-12 * scale.max(1.0);
    let mut pivot_columns = Vec::with_capacity(n.saturating_sub(1));
    let mut pivot_row = 0;
    for col in 0..n {
        if pivot_row >= n.saturating_sub(1) {
            break;
        }

        let mut best_row = pivot_row;
        let mut best_norm = a[pivot_row][col].norm();
        for row in (pivot_row + 1)..n {
            let candidate = a[row][col].norm();
            if candidate > best_norm {
                best_norm = candidate;
                best_row = row;
            }
        }
        if best_norm <= tolerance {
            continue;
        }

        a.swap(pivot_row, best_row);
        let pivot = a[pivot_row][col];
        for row in (pivot_row + 1)..n {
            let factor = a[row][col] / pivot;
            a[row][col] = Complex64::new(0.0, 0.0);
            for trailing_col in (col + 1)..n {
                let pivot_value = a[pivot_row][trailing_col];
                a[row][trailing_col] -= factor * pivot_value;
            }
        }
        pivot_columns.push(col);
        pivot_row += 1;
    }

    let free_col = (0..n)
        .rev()
        .find(|col| !pivot_columns.contains(col))
        .unwrap_or(n - 1);
    let mut vector = vec![Complex64::new(0.0, 0.0); n];
    vector[free_col] = Complex64::new(1.0, 0.0);

    for row in (0..pivot_columns.len()).rev() {
        let col = pivot_columns[row];
        let mut sum = Complex64::new(0.0, 0.0);
        for trailing_col in (col + 1)..n {
            sum += a[row][trailing_col] * vector[trailing_col];
        }
        vector[col] = -sum / a[row][col];
    }

    let norm = vector
        .iter()
        .map(|value| value.norm_sqr())
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn eye(n: usize) -> Vec<Vec<f64>> {
    let mut m = vec![vec![0.0; n]; n];
    for i in 0..n {
        m[i][i] = 1.0;
    }
    m
}

/// Reduce A to upper Hessenberg form via Householder reflections.
/// Accumulates transforms: q_accum = q_accum * P1 * P2 * ...
fn hessenberg_reduce(a: &mut Vec<Vec<f64>>, q: &mut Vec<Vec<f64>>, n: usize) {
    for k in 0..n.saturating_sub(2) {
        // Build Householder vector for column k, rows k+1..n
        let mut x = vec![0.0; n - k - 1];
        for i in 0..x.len() {
            x[i] = a[k + 1 + i][k];
        }
        let norm_x = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm_x < 1e-15 {
            continue;
        }

        let sign = if x[0] >= 0.0 { 1.0 } else { -1.0 };
        x[0] += sign * norm_x;
        let norm_v = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm_v < 1e-15 {
            continue;
        }
        for v in x.iter_mut() {
            *v /= norm_v;
        }

        // Apply P = I - 2vv^T to A from left: A <- P * A
        // Affects rows k+1..n
        for j in 0..n {
            let mut dot = 0.0;
            for i in 0..x.len() {
                dot += x[i] * a[k + 1 + i][j];
            }
            let two_dot = 2.0 * dot;
            for i in 0..x.len() {
                a[k + 1 + i][j] -= two_dot * x[i];
            }
        }

        // Apply P to A from right: A <- A * P
        // Affects columns k+1..n
        for i in 0..n {
            let mut dot = 0.0;
            for j in 0..x.len() {
                dot += a[i][k + 1 + j] * x[j];
            }
            let two_dot = 2.0 * dot;
            for j in 0..x.len() {
                a[i][k + 1 + j] -= two_dot * x[j];
            }
        }

        // Accumulate into Q: Q <- Q * P
        for i in 0..n {
            let mut dot = 0.0;
            for j in 0..x.len() {
                dot += q[i][k + 1 + j] * x[j];
            }
            let two_dot = 2.0 * dot;
            for j in 0..x.len() {
                q[i][k + 1 + j] -= two_dot * x[j];
            }
        }
    }
}

/// Francis QR iteration with implicit double shifts.
/// Converges H to quasi-upper-triangular (real Schur) form.
fn francis_qr(h: &mut Vec<Vec<f64>>, q: &mut Vec<Vec<f64>>, n: usize) -> Result<()> {
    let max_iter = 100 * n;
    let mut p = n; // active submatrix is rows/cols 0..p

    for _iter in 0..max_iter {
        if p <= 1 {
            return Ok(());
        }

        // Deflation: check if h[p-1][p-2] is negligible
        let tol = 1e-14 * (h[p - 2][p - 2].abs() + h[p - 1][p - 1].abs()).max(1e-30);
        if h[p - 1][p - 2].abs() <= tol {
            h[p - 1][p - 2] = 0.0;
            p -= 1;
            continue;
        }

        // Check for 2×2 block deflation
        if p >= 3 {
            let tol2 = 1e-14 * (h[p - 3][p - 3].abs() + h[p - 2][p - 2].abs()).max(1e-30);
            if h[p - 2][p - 3].abs() <= tol2 {
                h[p - 2][p - 3] = 0.0;
                // Check if bottom 2×2 is already deflated
                if is_2x2_converged(h, p) {
                    p -= 2;
                    continue;
                }
            }
        }

        // Find start of active unreduced block
        let mut l = p - 2;
        while l > 0 {
            let tol_l = 1e-14 * (h[l - 1][l - 1].abs() + h[l][l].abs()).max(1e-30);
            if h[l][l - 1].abs() <= tol_l {
                h[l][l - 1] = 0.0;
                break;
            }
            l -= 1;
        }

        // Wilkinson shift from bottom-right 2×2
        implicit_qr_step(h, q, l, p, n);
    }

    // If we didn't fully converge, return what we have — eigenvalues may be approximate
    Ok(())
}

/// Check if the bottom 2×2 block represents a converged complex pair.
fn is_2x2_converged(h: &Vec<Vec<f64>>, p: usize) -> bool {
    let a = h[p - 2][p - 2];
    let b = h[p - 2][p - 1];
    let c = h[p - 1][p - 2];
    let d = h[p - 1][p - 1];
    // Complex pair if discriminant < 0
    let tr = a + d;
    let det = a * d - b * c;
    tr * tr < 4.0 * det
}

/// Single implicit QR step with Wilkinson shift on H[l..p, l..p].
fn implicit_qr_step(h: &mut Vec<Vec<f64>>, q: &mut Vec<Vec<f64>>, l: usize, p: usize, n: usize) {
    // Wilkinson shift: eigenvalues of bottom-right 2×2
    let a = h[p - 2][p - 2];
    let b = h[p - 2][p - 1];
    let c = h[p - 1][p - 2];
    let d = h[p - 1][p - 1];
    let tr = a + d;
    let det = a * d - b * c;

    // First column of (H - s1*I)(H - s2*I) where s1,s2 are shifts
    let mut x = h[l][l] * h[l][l] + h[l][l + 1] * h[l + 1][l] - tr * h[l][l] + det;
    let mut y = h[l + 1][l] * (h[l][l] + h[l + 1][l + 1] - tr);
    let mut z = if l + 2 < p {
        h[l + 2][l + 1] * h[l + 1][l]
    } else {
        0.0
    };

    for k in l..p.saturating_sub(1) {
        // Build Householder to zero out [y, z] in [x, y, z]
        let (v, beta) = householder3(x, y, z, k + 2 < p);

        // Apply from left: H <- P * H (rows k..min(k+3,p), all active columns)
        let r_start = if k > l { k - 1 } else { l };
        for j in r_start..n {
            let mut dot = v[0] * h[k][j];
            dot += v[1] * h[k + 1][j];
            if k + 2 < p {
                dot += v[2] * h[k + 2][j];
            }
            let bd = beta * dot;
            h[k][j] -= bd * v[0];
            h[k + 1][j] -= bd * v[1];
            if k + 2 < p {
                h[k + 2][j] -= bd * v[2];
            }
        }

        // Apply from right: H <- H * P (all rows, columns k..min(k+3,p))
        // Row k+3 can contain the last non-zero entry touched by this
        // right-side reflector (the subdiagonal in column k+2).  Omitting it
        // breaks H <- P H P similarity and corrupts the eigenvalues.
        let c_end = (k + 4).min(p).min(n);
        for i in 0..c_end {
            let mut dot = v[0] * h[i][k];
            dot += v[1] * h[i][k + 1];
            if k + 2 < p {
                dot += v[2] * h[i][k + 2];
            }
            let bd = beta * dot;
            h[i][k] -= bd * v[0];
            h[i][k + 1] -= bd * v[1];
            if k + 2 < p {
                h[i][k + 2] -= bd * v[2];
            }
        }

        // Accumulate into Q: Q <- Q * P
        for i in 0..n {
            let mut dot = v[0] * q[i][k];
            dot += v[1] * q[i][k + 1];
            if k + 2 < p {
                dot += v[2] * q[i][k + 2];
            }
            let bd = beta * dot;
            q[i][k] -= bd * v[0];
            q[i][k + 1] -= bd * v[1];
            if k + 2 < p {
                q[i][k + 2] -= bd * v[2];
            }
        }

        // Prepare for next bulge chase
        if k + 3 < p {
            x = h[k + 1][k];
            y = h[k + 2][k];
            z = if k + 3 < p { h[k + 3][k] } else { 0.0 };
        } else {
            x = h[k + 1][k];
            y = if k + 2 < p { h[k + 2][k] } else { 0.0 };
            z = 0.0;
        }
    }
}

/// Build Householder reflector for [x, y, z] (or [x, y] if not use_z).
/// Returns (v, beta) where P = I - beta * v * v^T.
fn householder3(x: f64, y: f64, z: f64, use_z: bool) -> ([f64; 3], f64) {
    let norm = if use_z {
        (x * x + y * y + z * z).sqrt()
    } else {
        (x * x + y * y).sqrt()
    };
    if norm < 1e-30 {
        return ([1.0, 0.0, 0.0], 0.0);
    }
    let sign = if x >= 0.0 { 1.0 } else { -1.0 };
    let v0 = x + sign * norm;
    let v1 = y;
    let v2 = if use_z { z } else { 0.0 };
    let norm_v_sq = v0 * v0 + v1 * v1 + v2 * v2;
    if norm_v_sq < 1e-30 {
        return ([1.0, 0.0, 0.0], 0.0);
    }
    let beta = 2.0 / norm_v_sq;
    ([v0, v1, v2], beta)
}

/// Extract eigenvalues from quasi-upper-triangular (real Schur) form.
fn extract_eigenvalues(
    h: &Vec<Vec<f64>>,
    n: usize,
    eig_real: &mut Vec<f64>,
    eig_imag: &mut Vec<f64>,
) {
    let mut i = 0;
    while i < n {
        if i + 1 < n
            && h[i + 1][i].abs() > 1e-14 * (h[i][i].abs() + h[i + 1][i + 1].abs()).max(1e-30)
        {
            // 2×2 block: complex conjugate pair
            let a = h[i][i];
            let b = h[i][i + 1];
            let c = h[i + 1][i];
            let d = h[i + 1][i + 1];
            let tr = a + d;
            let det = a * d - b * c;
            let disc = tr * tr - 4.0 * det;
            if disc < 0.0 {
                eig_real[i] = tr / 2.0;
                eig_real[i + 1] = tr / 2.0;
                eig_imag[i] = (-disc).sqrt() / 2.0;
                eig_imag[i + 1] = -(-disc).sqrt() / 2.0;
            } else {
                let sqrt_disc = disc.sqrt();
                eig_real[i] = (tr + sqrt_disc) / 2.0;
                eig_real[i + 1] = (tr - sqrt_disc) / 2.0;
                eig_imag[i] = 0.0;
                eig_imag[i + 1] = 0.0;
            }
            i += 2;
        } else {
            // 1×1 block: real eigenvalue
            eig_real[i] = h[i][i];
            eig_imag[i] = 0.0;
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupled_ring_eigenpairs_satisfy_original_matrix() {
        let matrix = vec![
            vec![-0.3798553, -0.03918255, -0.6712767, -0.06924235],
            vec![2.507634, -0.3798501, 4.431523, -0.6712692],
            vec![0.6712784, 0.06924260, -0.3798552, -0.03918181],
            vec![-4.431506, 0.6712676, 2.507681, -0.3798501],
        ];
        let (real, imag, vectors) = rosy_lev(&matrix, 4, 4).unwrap();

        assert!((real[0] - 0.1740868812).abs() < 1e-9);
        assert!((imag[0] - 0.9847302887).abs() < 1e-9);
        assert!((real[2] + 0.9337922312).abs() < 1e-9);
        assert!((imag[2] - 0.3578156612).abs() < 1e-9);

        for col in [0, 2] {
            let lambda = Complex64::new(real[col], imag[col]);
            let vector: Vec<_> = (0..4)
                .map(|row| Complex64::new(vectors[row][col], vectors[row][col + 1]))
                .collect();
            for row in 0..4 {
                let applied = (0..4)
                    .map(|inner| matrix[row][inner] * vector[inner])
                    .sum::<Complex64>();
                assert!((applied - lambda * vector[row]).norm() < 1e-10);
            }
        }
    }
}
