//! `COULSUM`: the Coulomb sum over Taylor series in time.
//!
//! A Taylor integrator for charged particles spends its time in one loop:
//! for particle `i`, add up `n_k (r_i - r_k) / |r_i - r_k|^3` over every
//! other particle `k`, where each coordinate is a series in time. Written
//! with series operators that is a dozen temporaries per pair. This does
//! the same arithmetic on plain coefficient arrays.
//!
//! Every coefficient is produced by the same operations in the same order
//! as the series operators would use, including the `DAEPS` cutoff after
//! each one, so the result is bit for bit what the loop below gives:
//!
//! ```text
//! LOOP K 1 N; IF K#I;
//!   R2 := (X(I)-X(K))*(X(I)-X(K));
//!   R2 := R2 + (Y(I)-Y(K))*(Y(I)-Y(K));
//!   R2 := R2 + RELG^2*(Z(I)-Z(K))*(Z(I)-Z(K));
//!   W  := 1/(R2*SQRT(R2));
//!   FX := FX + MQ(2*K)*(X(I)-X(K))*W;   (and FY, FZ)
//! ENDIF; ENDLOOP;
//! ```
//!
//! Particle `p` keeps its six series at `6*(p-1)+1 ..= 6*(p-1)+6` (x, y,
//! z, then momenta) and its charge factor at `MQ(2*p)`.

use anyhow::{Result, bail, ensure};

use crate::taylor::da::{DACoefficient, fmadd};
use crate::taylor::{DA, get_runtime};

/// `|c| > DAEPS` keeps a coefficient; anything else is zero.
#[inline(always)]
fn keep(c: f64, epsilon: f64) -> f64 {
    if c.abs() > epsilon { c } else { 0.0 }
}

/// Coefficient `k` of a series product, summed the way `DA * DA` sums it.
#[inline(always)]
fn product(a: &[f64], b: &[f64], k: usize, epsilon: f64) -> f64 {
    let mut acc = 0.0;
    for i in 0..=k {
        acc = fmadd(a[i], b[k - i], acc);
    }
    keep(acc, epsilon)
}

/// Working series for one pair, each `order + 1` long.
#[derive(Default)]
struct Scratch {
    d: [Vec<f64>; 3],
    /// `RELG^2 * dz`.
    tz: Vec<f64>,
    r2: Vec<f64>,
    /// Square root before the cutoff: the recurrence reads these.
    root_raw: Vec<f64>,
    root: Vec<f64>,
    cube: Vec<f64>,
    inverse: Vec<f64>,
    scaled: Vec<f64>,
    sum: [Vec<f64>; 3],
    ratios: Vec<f64>,
}

thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::new(Scratch::default());
}

/// Sum the Coulomb terms on particle `i` from the other `n - 1` particles
/// at the current truncation order, and store the three sums in
/// `phi(6*(i-1)+4 ..= 6*(i-1)+6)`.
///
/// With `want_radii` nonzero, `radii` becomes one convergence-radius
/// estimate per pair, in `k` order: `1/(a + b)` for the two largest of
/// `|c_j / c_0|^(1/j)`, `j = 1 ..= order`, over the coefficients of the
/// squared distance. It needs order 2 or more. Otherwise `radii` is left
/// alone.
#[allow(clippy::too_many_arguments)]
pub fn coulsum(
    y: &[DA],
    mq: &[f64],
    i: f64,
    n: f64,
    relg: f64,
    want_radii: f64,
    phi: &mut [DA],
    radii: &mut Vec<f64>,
) -> Result<()> {
    let rt = get_runtime()?;
    ensure!(rt.config.num_vars == 1, "COULSUM needs series in one variable (DAINI order 1)");
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let monomials = rt.num_monomials;
    drop(rt);
    let (i, n) = (i as usize, n as usize);
    ensure!(i >= 1 && i <= n, "COULSUM: particle {i} is not one of 1..{n}");
    ensure!(y.len() >= 6 * n && phi.len() >= 6 * n && mq.len() >= 2 * n, "COULSUM: an array is too short for {n} particles");
    let want_radii = want_radii != 0.0;
    ensure!(!want_radii || order >= 2, "COULSUM: radii need order 2 or more");
    let len = order + 1;
    let coefficient = |series: &DA, k: usize| series.coeffs.get(k).copied().unwrap_or(0.0);
    let relg2 = relg.powf(2.0);

    SCRATCH.with(|scratch| -> Result<()> {
        let s = &mut *scratch.borrow_mut();
        for v in s.d.iter_mut().chain(s.sum.iter_mut()).chain([
            &mut s.tz, &mut s.r2, &mut s.root_raw, &mut s.root, &mut s.cube, &mut s.inverse, &mut s.scaled,
        ]) {
            v.clear();
            v.resize(len, 0.0);
        }
        if want_radii {
            radii.clear();
        }
        let mine = 6 * (i - 1);
        for k in 1..=n {
            if k == i {
                continue;
            }
            let theirs = 6 * (k - 1);
            for axis in 0..3 {
                let (a, b) = (&y[mine + axis], &y[theirs + axis]);
                for c in 0..len {
                    s.d[axis][c] = keep(coefficient(a, c) - coefficient(b, c), epsilon);
                }
            }
            // A scalar at or under the cutoff multiplies to nothing.
            let live = relg2.abs() > epsilon;
            for c in 0..len {
                s.tz[c] = if live { keep(s.d[2][c] * relg2, epsilon) } else { 0.0 };
            }
            for c in 0..len {
                let xx = product(&s.d[0], &s.d[0], c, epsilon);
                let yy = product(&s.d[1], &s.d[1], c, epsilon);
                let zz = product(&s.tz, &s.d[2], c, epsilon);
                s.r2[c] = keep(keep(xx + yy, epsilon) + zz, epsilon);
            }
            if want_radii {
                s.ratios.clear();
                let c0 = s.r2[0].abs();
                s.ratios.push(s.r2[1].abs() / c0);
                for j in 2..=order {
                    s.ratios.push((s.r2[j].abs() / c0).powf(1.0 / j as f64));
                }
                s.ratios.sort_unstable_by(f64::total_cmp);
                radii.push(1.0 / (s.ratios[order - 2] + s.ratios[order - 1]));
            }

            let f0 = s.r2[0];
            if !(f0 > 0.0) {
                bail!("COULSUM: particles {i} and {k} are at the same place");
            }
            let root0 = f0.sqrt();
            let half = 0.5 / root0;
            s.root_raw[0] = root0;
            for degree in 1..len {
                let mut v = s.r2[degree];
                for j in 1..degree {
                    let (a, b) = (s.root_raw[j], s.root_raw[degree - j]);
                    if a != 0.0 && b != 0.0 {
                        v = fmadd(a * -1.0, b, v);
                    }
                }
                s.root_raw[degree] = v * half;
            }
            for c in 0..len {
                s.root[c] = keep(s.root_raw[c], epsilon);
            }
            for c in 0..len {
                s.cube[c] = product(&s.r2, &s.root, c, epsilon);
            }
            let g0 = s.cube[0];
            if !g0.abs().is_finite() || g0.abs() <= epsilon {
                bail!("COULSUM: division by zero for particles {i} and {k}");
            }
            let g0_inv = 1.0 / g0;
            for c in 0..len {
                let mut sum = 0.0;
                for g in 1..=c {
                    sum = fmadd(s.cube[g], s.inverse[c - g], sum);
                }
                let numerator = if c == 0 { 1.0 } else { 0.0 };
                s.inverse[c] = keep((numerator - sum) * g0_inv, epsilon);
            }
            let charge = mq[2 * (k - 1) + 1];
            let charged = charge.abs() > epsilon;
            for axis in 0..3 {
                for c in 0..len {
                    s.scaled[c] = if charged { keep(s.d[axis][c] * charge, epsilon) } else { 0.0 };
                }
                for c in 0..len {
                    let term = product(&s.scaled, &s.inverse, c, epsilon);
                    s.sum[axis][c] = keep(s.sum[axis][c] + term, epsilon);
                }
            }
        }
        for axis in 0..3 {
            let (mut coeffs, mut nonzero) = f64::pool_alloc_da(monomials, len);
            for c in 0..len {
                let v = s.sum[axis][c];
                if v != 0.0 {
                    coeffs[c] = v;
                    nonzero.push(c as u32);
                }
            }
            phi[mine + 3 + axis] = DA { coeffs, nonzero };
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taylor::{init_taylor, set_truncation_order as set_max_order};

    /// The loop in the module comment, with the series operators.
    fn by_operators(y: &[DA], mq: &[f64], i: usize, n: usize, relg: f64, order: usize, radii: Option<&mut Vec<f64>>) -> [DA; 3] {
        use crate::operators::*;
        use crate::intrinsics::sqrt::RosySQRT;
        let mut sum = [DA::from_coeff(0.0), DA::from_coeff(0.0), DA::from_coeff(0.0)];
        let mut radii = radii;
        let mine = 6 * (i - 1);
        for k in 1..=n {
            if k == i {
                continue;
            }
            let theirs = 6 * (k - 1);
            let d = |axis: usize| RosySub::rosy_sub(&y[mine + axis], &y[theirs + axis]).unwrap();
            let mut r2 = RosyMult::rosy_mult(&d(0), &d(0)).unwrap();
            r2.add_assign_exact(&RosyMult::rosy_mult(&d(1), &d(1)).unwrap()).unwrap();
            let scaled = RosyMult::rosy_mult(&RosyPow::rosy_pow(&relg, &2f64).unwrap(), &d(2)).unwrap();
            r2.add_assign_exact(&RosyMult::rosy_mult(&scaled, &d(2)).unwrap()).unwrap();
            if let Some(radii) = radii.as_deref_mut() {
                let c = |j: usize| RosyExtract::rosy_extract(&r2, &(j as f64)).unwrap().abs();
                let mut ratios = vec![c(1) / c(0)];
                for j in 2..=order {
                    ratios.push((c(j) / c(0)).powf(1.0 / j as f64));
                }
                ratios.sort_by(f64::total_cmp);
                radii.push(1.0 / (ratios[order - 2] + ratios[order - 1]));
            }
            let w = RosyDiv::rosy_div(&1f64, &RosyMult::rosy_mult(&r2, &RosySQRT::rosy_sqrt(&r2).unwrap()).unwrap()).unwrap();
            for axis in 0..3 {
                let term = RosyMult::rosy_mult(&RosyMult::rosy_mult(&mq[2 * (k - 1) + 1], &d(axis)).unwrap(), &w).unwrap();
                sum[axis].add_assign_exact(&term).unwrap();
            }
        }
        sum
    }

    fn bits(series: &DA, order: usize) -> Vec<u64> {
        (0..=order).map(|k| series.coeffs.get(k).copied().unwrap_or(0.0).to_bits()).collect()
    }

    #[test]
    fn matches_the_series_operators_bit_for_bit() {
        let full = 12usize;
        init_taylor(full as u32, 1).unwrap();
        let epsilon_before = crate::taylor::get_runtime().unwrap().config.epsilon;
        crate::taylor::set_epsilon(1e-100).unwrap();
        let mut state = 0x9e3779b97f4a7c15u64;
        let mut random = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let n = 9usize;
        let mut y = Vec::new();
        for p in 0..n {
            for axis in 0..6 {
                let mut series = DA::from_coeff(p as f64 * 0.7 + axis as f64 * 0.31 + random());
                // Some particles are full series, some stop early, one is constant.
                let top = match p % 3 {
                    0 => full,
                    1 => 5,
                    _ => 0,
                };
                for k in 1..=top {
                    let mut exponents = [0u8; crate::taylor::MAX_VARS];
                    exponents[0] = k as u8;
                    series.set_coeff(crate::taylor::Monomial::new(exponents), random() * 0.3f64.powi(k as i32));
                }
                y.push(series);
            }
        }
        let mq: Vec<f64> = (0..2 * n).map(|j| if j == 7 { 0.0 } else { 1.0 + random() }).collect();
        for order in 0..=full {
            set_max_order(order as u32).unwrap();
            for i in 1..=n {
                let want = order >= 2;
                let mut expected_radii = Vec::new();
                let expected = by_operators(&y, &mq, i, n, 1.37, order, want.then_some(&mut expected_radii));
                let mut phi = vec![DA::zero(); 6 * n];
                let mut radii = vec![123.0];
                coulsum(&y, &mq, i as f64, n as f64, 1.37, if want { 1.0 } else { 0.0 }, &mut phi, &mut radii).unwrap();
                for axis in 0..3 {
                    assert_eq!(
                        bits(&phi[6 * (i - 1) + 3 + axis], full),
                        bits(&expected[axis], full),
                        "order {order}, particle {i}, axis {axis}"
                    );
                }
                if want {
                    let a: Vec<u64> = radii.iter().map(|r| r.to_bits()).collect();
                    let b: Vec<u64> = expected_radii.iter().map(|r| r.to_bits()).collect();
                    assert_eq!(a, b, "radii at order {order}, particle {i}");
                } else {
                    assert_eq!(radii, vec![123.0]);
                }
            }
        }
        set_max_order(full as u32).unwrap();
        crate::taylor::set_epsilon(epsilon_before).unwrap();
    }
}
