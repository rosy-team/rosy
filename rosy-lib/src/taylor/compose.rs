//! Order-by-order DA function composition (COSY DAFUN-style).
//!
//! Fills homogeneous degree `n` from degrees `< n` via Cauchy products instead
//! of Horner-evaluating a univariate series in the full `δf`. For a dense
//! argument that is one triangular convolution, not `order` full multiplies.

use anyhow::{Result, ensure};

use super::config::{MULT_INVALID, TaylorRuntime, get_runtime};
use super::da::{DA, DACoefficient};
use super::scratch::{ScratchFrame, scratch_alloc};

fn degree_range(rt: &TaylorRuntime, d: usize) -> std::ops::Range<usize> {
    let off = &rt.degree_offset;
    if d + 1 >= off.len() {
        return 0..0;
    }
    off[d]..off[d + 1]
}

/// `out += scale * [lhs]_deg_l * [rhs]_deg_r`.
///
/// `lhs` / `rhs` / `out` may alias. The recurrences only write degree
/// `deg_l + deg_r`, which is disjoint from the two source blocks.
fn homog_mul_scaled_add(
    out: *mut f64,
    lhs: *const f64,
    rhs: *const f64,
    deg_l: usize,
    deg_r: usize,
    scale: f64,
    rt: &TaylorRuntime,
) {
    if scale == 0.0 {
        return;
    }
    let n = rt.num_monomials;
    let lo_l = degree_range(rt, deg_l);
    let lo_r = degree_range(rt, deg_r);
    if lo_l.is_empty() || lo_r.is_empty() {
        return;
    }

    if let Some(table) = &rt.mult_table {
        for i in lo_l {
            let ci = unsafe { *lhs.add(i) };
            if ci == 0.0 {
                continue;
            }
            let cis = ci * scale;
            let row = i * n;
            for j in lo_r.clone() {
                let bj = unsafe { *rhs.add(j) };
                if bj == 0.0 {
                    continue;
                }
                let k = table[row + j];
                if k != MULT_INVALID {
                    let slot = unsafe { &mut *out.add(k as usize) };
                    *slot = cis.mul_add(bj, *slot);
                }
            }
        }
    } else {
        let trunc = rt.config.max_order;
        for i in lo_l {
            let ci = unsafe { *lhs.add(i) };
            if ci == 0.0 {
                continue;
            }
            let cis = ci * scale;
            for j in lo_r.clone() {
                let bj = unsafe { *rhs.add(j) };
                if bj == 0.0 {
                    continue;
                }
                let product = rt.monomial_list[i].multiply(&rt.monomial_list[j]);
                if product.within_order(trunc) {
                    if let Some(&k) = rt.monomial_index.get(&product) {
                        let slot = unsafe { &mut *out.add(k as usize) };
                        *slot = cis.mul_add(bj, *slot);
                    }
                }
            }
        }
    }
}

fn copy_buf_to_da(n: usize, buf: &[f64], epsilon: f64) -> DA<f64> {
    let mut coeffs = f64::pool_alloc(n);
    let mut nonzero = Vec::new();
    for (i, &v) in buf.iter().enumerate() {
        if v.abs() > epsilon {
            coeffs[i] = v;
            nonzero.push(i as u32);
        }
    }
    DA { coeffs, nonzero }
}

fn scratch_slice(n: usize) -> Result<(*mut f64, ScratchFrame)> {
    let frame = ScratchFrame::enter();
    let p = scratch_alloc(n)?;
    if n > 0 {
        unsafe {
            std::slice::from_raw_parts_mut(p, n).fill(0.0);
        }
    }
    Ok((p, frame))
}

fn with_scratch_buf<R>(n: usize, f: impl FnOnce(&mut [f64]) -> Result<R>) -> Result<R> {
    let (p, _frame) = scratch_slice(n)?;
    let buf = unsafe { std::slice::from_raw_parts_mut(p, n) };
    f(buf)
}

fn with_scratch_pair<R>(n: usize, f: impl FnOnce(&mut [f64], &mut [f64]) -> Result<R>) -> Result<R> {
    let frame = ScratchFrame::enter();
    let pa = scratch_alloc(n)?;
    let pb = scratch_alloc(n)?;
    let result = unsafe {
        let a = std::slice::from_raw_parts_mut(pa, n);
        let b = std::slice::from_raw_parts_mut(pb, n);
        a.fill(0.0);
        b.fill(0.0);
        f(a, b)
    };
    drop(frame);
    result
}

/// `exp(f)_n = Σ_{k=1}^{n} (k/n) f_k exp(f)_{n-k}`, with `exp(f)_0 = exp(f₀)`.
pub fn compose_exp(da: &DA<f64>) -> Result<DA<f64>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let f0 = da.constant_part();
    let exp0 = f0.exp();

    with_scratch_buf(n, |out| {
        out[0] = exp0;
        for deg in 1..=order {
            let inv = 1.0 / (deg as f64);
            let op = out.as_mut_ptr();
            let fp = da.coeffs.as_ptr();
            for k in 1..=deg {
                homog_mul_scaled_add(op, fp, op, k, deg - k, (k as f64) * inv, &rt);
            }
        }
        Ok(copy_buf_to_da(n, out, epsilon))
    })
}

/// `log(f)_n = (f_n - Σ_{k=1}^{n-1} (k/n) log(f)_k f_{n-k}) / f₀`.
pub fn compose_log(da: &DA<f64>) -> Result<DA<f64>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let f0 = da.constant_part();
    ensure!(f0 != 0.0, "LOG: constant part of DA argument must be non-zero");
    let ln0 = f0.ln();
    let inv_f0 = 1.0 / f0;

    with_scratch_buf(n, |out| {
        out[0] = ln0;
        for deg in 1..=order {
            for i in degree_range(&rt, deg) {
                out[i] = da.coeffs[i] * inv_f0;
            }
            let inv_n = 1.0 / (deg as f64);
            let op = out.as_mut_ptr();
            let fp = da.coeffs.as_ptr();
            for k in 1..deg {
                homog_mul_scaled_add(op, op, fp, k, deg - k, -(k as f64) * inv_n * inv_f0, &rt);
            }
        }
        Ok(copy_buf_to_da(n, out, epsilon))
    })
}

/// `√f_n = (f_n - Σ_{k=1}^{n-1} √f_k √f_{n-k}) / (2 √f₀)`.
pub fn compose_sqrt(da: &DA<f64>) -> Result<DA<f64>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let f0 = da.constant_part();
    ensure!(f0 > 0.0, "SQRT: constant part of DA must be positive, got {f0}");
    let sqrt0 = f0.sqrt();
    let inv_den = 0.5 / sqrt0;

    with_scratch_buf(n, |out| {
        out[0] = sqrt0;
        for deg in 1..=order {
            for i in degree_range(&rt, deg) {
                out[i] = da.coeffs[i];
            }
            let op = out.as_mut_ptr();
            for k in 1..deg {
                homog_mul_scaled_add(op, op, op, k, deg - k, -1.0, &rt);
            }
            for i in degree_range(&rt, deg) {
                out[i] *= inv_den;
            }
        }
        Ok(copy_buf_to_da(n, out, epsilon))
    })
}

/// Coupled recurrence: `s_n = Σ (k/n) f_k c_{n-k}`, `c_n = -Σ (k/n) f_k s_{n-k}`.
fn compose_sin_cos(da: &DA<f64>) -> Result<(DA<f64>, DA<f64>)> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let f0 = da.constant_part();
    let s0 = f0.sin();
    let c0 = f0.cos();

    with_scratch_pair(n, |s, c| {
        s[0] = s0;
        c[0] = c0;
        for deg in 1..=order {
            let inv = 1.0 / (deg as f64);
            let sp = s.as_mut_ptr();
            let cp = c.as_mut_ptr();
            let fp = da.coeffs.as_ptr();
            for k in 1..=deg {
                let sk = (k as f64) * inv;
                homog_mul_scaled_add(sp, fp, cp, k, deg - k, sk, &rt);
                homog_mul_scaled_add(cp, fp, sp, k, deg - k, -sk, &rt);
            }
        }
        Ok((
            copy_buf_to_da(n, s, epsilon),
            copy_buf_to_da(n, c, epsilon),
        ))
    })
}

pub fn compose_sin(da: &DA<f64>) -> Result<DA<f64>> {
    Ok(compose_sin_cos(da)?.0)
}

pub fn compose_cos(da: &DA<f64>) -> Result<DA<f64>> {
    Ok(compose_sin_cos(da)?.1)
}

/// `sh_n = Σ (k/n) f_k ch_{n-k}`, `ch_n = Σ (k/n) f_k sh_{n-k}`.
fn compose_sinh_cosh(da: &DA<f64>) -> Result<(DA<f64>, DA<f64>)> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let f0 = da.constant_part();
    let sh0 = f0.sinh();
    let ch0 = f0.cosh();

    with_scratch_pair(n, |sh, ch| {
        sh[0] = sh0;
        ch[0] = ch0;
        for deg in 1..=order {
            let inv = 1.0 / (deg as f64);
            let shp = sh.as_mut_ptr();
            let chp = ch.as_mut_ptr();
            let fp = da.coeffs.as_ptr();
            for k in 1..=deg {
                let sk = (k as f64) * inv;
                homog_mul_scaled_add(shp, fp, chp, k, deg - k, sk, &rt);
                homog_mul_scaled_add(chp, fp, shp, k, deg - k, sk, &rt);
            }
        }
        Ok((
            copy_buf_to_da(n, sh, epsilon),
            copy_buf_to_da(n, ch, epsilon),
        ))
    })
}

pub fn compose_sinh(da: &DA<f64>) -> Result<DA<f64>> {
    Ok(compose_sinh_cosh(da)?.0)
}

pub fn compose_cosh(da: &DA<f64>) -> Result<DA<f64>> {
    Ok(compose_sinh_cosh(da)?.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taylor::config::{cleanup_taylor, get_runtime, init_taylor};
    use serial_test::serial;

    fn coeffs_close(a: &DA<f64>, b: &DA<f64>, tol: f64) -> bool {
        let n = a.coeffs.len().max(b.coeffs.len());
        for i in 0..n {
            let av = a.coeffs.get(i).copied().unwrap_or(0.0);
            let bv = b.coeffs.get(i).copied().unwrap_or(0.0);
            if (av - bv).abs() > tol {
                return false;
            }
        }
        true
    }

    fn horner_exp(da: &DA<f64>) -> DA<f64> {
        let rt = get_runtime().unwrap();
        let nocut = rt.config.max_order as usize;
        let f0 = da.constant_part();
        let exp_f0 = f0.exp();
        let da_prime = da.make_prime();
        let mut xf = Vec::with_capacity(nocut + 1);
        xf.push(1.0);
        for i in 1..=nocut {
            xf.push(xf[i - 1] / (i as f64));
        }
        let result = DA::<f64>::horner_eval_with_rt(&da_prime, &xf, &rt).unwrap();
        drop(rt);
        (&result * DA::<f64>::from_coeff(exp_f0)).unwrap()
    }

    fn sample_da() -> DA<f64> {
        let x = DA::<f64>::variable(1).unwrap();
        let y = DA::<f64>::variable(2).unwrap();
        let mut f = (&x + &y).unwrap();
        f = (&f + 0.5).unwrap();
        f = (&f + &(&x * &y).unwrap()).unwrap();
        f
    }

    #[test]
    #[serial]
    fn compose_exp_matches_horner() {
        cleanup_taylor();
        init_taylor(5, 2).unwrap();
        let f = sample_da();
        let a = compose_exp(&f).unwrap();
        let b = horner_exp(&f);
        assert!(
            coeffs_close(&a, &b, 1e-12),
            "exp compose vs Horner diverged"
        );
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn compose_log_roundtrip_exp() {
        cleanup_taylor();
        init_taylor(5, 2).unwrap();
        let f = sample_da();
        let e = compose_exp(&f).unwrap();
        let back = compose_log(&e).unwrap();
        assert!(
            coeffs_close(&f, &back, 1e-10),
            "log(exp(f)) should recover f"
        );
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn compose_sqrt_constant_and_linear() {
        cleanup_taylor();
        init_taylor(5, 2).unwrap();
        let x = DA::<f64>::variable(1).unwrap();
        let f = (&x + 4.0).unwrap();
        let s = compose_sqrt(&f).unwrap();
        assert!((s.constant_part() - 2.0).abs() < 1e-14);
        // d/dx sqrt(4+x) at 0 is 1/4
        let idx = get_runtime().unwrap().variable_indices[0] as usize;
        assert!((s.coeffs[idx] - 0.25).abs() < 1e-12);
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn compose_sin_cos_pythagorean() {
        cleanup_taylor();
        init_taylor(5, 2).unwrap();
        let f = sample_da();
        let s = compose_sin(&f).unwrap();
        let c = compose_cos(&f).unwrap();
        let ss = (&s * &s).unwrap();
        let cc = (&c * &c).unwrap();
        let sum = (&ss + &cc).unwrap();
        assert!((sum.constant_part() - 1.0).abs() < 1e-12);
        for &i in &sum.nonzero {
            if i == 0 {
                continue;
            }
            assert!(
                sum.coeffs[i as usize].abs() < 1e-10,
                "sin^2+cos^2 term {i} = {}",
                sum.coeffs[i as usize]
            );
        }
        cleanup_taylor();
    }
}
