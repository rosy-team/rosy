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

use anyhow::{Result, bail};

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

    // 4. Compute eigenvectors by back-substitution on the Schur form,
    //    then transform back to original basis via q_accum.
    let eigvecs_schur = schur_eigenvectors(&h, &eig_real, &eig_imag, n);

    // Transform: V = Q * V_schur
    let mut eigvecs = vec![vec![0.0; alloc_dim]; alloc_dim];
    for i in 0..n {
        for j in 0..n {
            let mut s = 0.0;
            for k in 0..n {
                s += q_accum[i][k] * eigvecs_schur[k][j];
            }
            eigvecs[i][j] = s;
        }
    }

    Ok((eig_real, eig_imag, eigvecs))
}

fn eye(n: usize) -> Vec<Vec<f64>> {
    let mut m = vec![vec![0.0; n]; n];
    for i in 0..n { m[i][i] = 1.0; }
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
        if norm_x < 1e-15 { continue; }

        let sign = if x[0] >= 0.0 { 1.0 } else { -1.0 };
        x[0] += sign * norm_x;
        let norm_v = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm_v < 1e-15 { continue; }
        for v in x.iter_mut() { *v /= norm_v; }

        // Apply P = I - 2vv^T to A from left: A <- P * A
        // Affects rows k+1..n
        for j in 0..n {
            let mut dot = 0.0;
            for i in 0..x.len() { dot += x[i] * a[k + 1 + i][j]; }
            let two_dot = 2.0 * dot;
            for i in 0..x.len() { a[k + 1 + i][j] -= two_dot * x[i]; }
        }

        // Apply P to A from right: A <- A * P
        // Affects columns k+1..n
        for i in 0..n {
            let mut dot = 0.0;
            for j in 0..x.len() { dot += a[i][k + 1 + j] * x[j]; }
            let two_dot = 2.0 * dot;
            for j in 0..x.len() { a[i][k + 1 + j] -= two_dot * x[j]; }
        }

        // Accumulate into Q: Q <- Q * P
        for i in 0..n {
            let mut dot = 0.0;
            for j in 0..x.len() { dot += q[i][k + 1 + j] * x[j]; }
            let two_dot = 2.0 * dot;
            for j in 0..x.len() { q[i][k + 1 + j] -= two_dot * x[j]; }
        }
    }
}

/// Francis QR iteration with implicit double shifts.
/// Converges H to quasi-upper-triangular (real Schur) form.
fn francis_qr(h: &mut Vec<Vec<f64>>, q: &mut Vec<Vec<f64>>, n: usize) -> Result<()> {
    let max_iter = 100 * n;
    let mut p = n; // active submatrix is rows/cols 0..p

    for _iter in 0..max_iter {
        if p <= 1 { return Ok(()); }

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
fn implicit_qr_step(
    h: &mut Vec<Vec<f64>>,
    q: &mut Vec<Vec<f64>>,
    l: usize,
    p: usize,
    n: usize,
) {
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
    let mut z = if l + 2 < p { h[l + 2][l + 1] * h[l + 1][l] } else { 0.0 };

    for k in l..p.saturating_sub(1) {
        // Build Householder to zero out [y, z] in [x, y, z]
        let (v, beta) = householder3(x, y, z, k + 2 < p);

        // Apply from left: H <- P * H (rows k..min(k+3,p), all active columns)
        let r_start = if k > l { k - 1 } else { l };
        for j in r_start..n {
            let mut dot = v[0] * h[k][j];
            dot += v[1] * h[k + 1][j];
            if k + 2 < p { dot += v[2] * h[k + 2][j]; }
            let bd = beta * dot;
            h[k][j] -= bd * v[0];
            h[k + 1][j] -= bd * v[1];
            if k + 2 < p { h[k + 2][j] -= bd * v[2]; }
        }

        // Apply from right: H <- H * P (all rows, columns k..min(k+3,p))
        let c_end = (k + 3).min(p).min(n);
        for i in 0..c_end {
            let mut dot = v[0] * h[i][k];
            dot += v[1] * h[i][k + 1];
            if k + 2 < p { dot += v[2] * h[i][k + 2]; }
            let bd = beta * dot;
            h[i][k] -= bd * v[0];
            h[i][k + 1] -= bd * v[1];
            if k + 2 < p { h[i][k + 2] -= bd * v[2]; }
        }

        // Accumulate into Q: Q <- Q * P
        for i in 0..n {
            let mut dot = v[0] * q[i][k];
            dot += v[1] * q[i][k + 1];
            if k + 2 < p { dot += v[2] * q[i][k + 2]; }
            let bd = beta * dot;
            q[i][k] -= bd * v[0];
            q[i][k + 1] -= bd * v[1];
            if k + 2 < p { q[i][k + 2] -= bd * v[2]; }
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
fn extract_eigenvalues(h: &Vec<Vec<f64>>, n: usize, eig_real: &mut Vec<f64>, eig_imag: &mut Vec<f64>) {
    let mut i = 0;
    while i < n {
        if i + 1 < n && h[i + 1][i].abs() > 1e-14 * (h[i][i].abs() + h[i + 1][i + 1].abs()).max(1e-30) {
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

/// Compute eigenvectors of the quasi-upper-triangular (Schur) matrix T.
///
/// For real eigenvalues, solves (T - λI)x = 0 by back-substitution.
/// For complex pairs, solves (T - (σ+iω)I)(u + iv) = 0.
/// Returns columns in the COSY convention: for complex pair at i,i+1,
/// column i = Re(eigvec), column i+1 = Im(eigvec).
fn schur_eigenvectors(t: &Vec<Vec<f64>>, eig_real: &[f64], eig_imag: &[f64], n: usize) -> Vec<Vec<f64>> {
    let mut vecs = vec![vec![0.0; n]; n];
    let mut i = 0;

    while i < n {
        if eig_imag[i].abs() < 1e-14 {
            // Real eigenvalue: back-substitution on (T - λI)
            real_eigenvector(t, eig_real[i], i, n, &mut vecs);
            i += 1;
        } else {
            // Complex pair at i, i+1
            complex_eigenvector_pair(t, eig_real[i], eig_imag[i], i, n, &mut vecs);
            i += 2;
        }
    }
    vecs
}

/// Back-substitution for a real eigenvector of quasi-upper-triangular T.
fn real_eigenvector(t: &Vec<Vec<f64>>, lambda: f64, col: usize, n: usize, vecs: &mut Vec<Vec<f64>>) {
    // Work array
    let mut x = vec![0.0; n];
    x[col] = 1.0;

    // Back-substitute: for j = col-1 down to 0
    for j in (0..col).rev() {
        let diag = t[j][j] - lambda;
        let mut sum = 0.0;
        for k in (j + 1)..=col {
            sum += t[j][k] * x[k];
        }
        if diag.abs() > 1e-30 {
            x[j] = -sum / diag;
        } else {
            x[j] = -sum / 1e-30;
        }
    }

    // Normalize
    let norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm > 1e-30 {
        for v in x.iter_mut() { *v /= norm; }
    }

    for j in 0..n { vecs[j][col] = x[j]; }
}

/// Back-substitution for a complex eigenvector pair.
/// Column `col` gets Re(v), column `col+1` gets Im(v).
fn complex_eigenvector_pair(
    t: &Vec<Vec<f64>>,
    sigma: f64,
    omega: f64,
    col: usize,
    n: usize,
    vecs: &mut Vec<Vec<f64>>,
) {
    let mut xr = vec![0.0; n]; // real part
    let mut xi = vec![0.0; n]; // imaginary part
    xr[col] = 1.0;
    xi[col + 1] = 1.0;

    // Back-substitute in 2×2 blocks: (T - (σ+iω)I)(xr + i·xi) = 0
    for j in (0..col).rev() {
        let mut sum_r = 0.0;
        let mut sum_i = 0.0;
        for k in (j + 1)..=col + 1 {
            sum_r += t[j][k] * xr[k];
            sum_i += t[j][k] * xi[k];
        }
        let dr = t[j][j] - sigma;
        let di = -omega;
        let denom = dr * dr + di * di;
        if denom > 1e-60 {
            xr[j] = -(sum_r * dr + sum_i * di) / denom;
            xi[j] = -(sum_i * dr - sum_r * di) / denom;
        }
    }

    // Normalize
    let norm = (xr.iter().zip(xi.iter()).map(|(r, i)| r * r + i * i).sum::<f64>()).sqrt();
    if norm > 1e-30 {
        for v in xr.iter_mut() { *v /= norm; }
        for v in xi.iter_mut() { *v /= norm; }
    }

    for j in 0..n {
        vecs[j][col] = xr[j];
        vecs[j][col + 1] = xi[j];
    }
}

// Public wrappers for MBLOCK reuse
pub fn hessenberg_reduce_pub(a: &mut Vec<Vec<f64>>, q: &mut Vec<Vec<f64>>, n: usize) {
    hessenberg_reduce(a, q, n);
}
pub fn francis_qr_pub(h: &mut Vec<Vec<f64>>, q: &mut Vec<Vec<f64>>, n: usize) -> Result<()> {
    francis_qr(h, q, n)
}
