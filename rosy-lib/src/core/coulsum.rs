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

/// Calls so far in this process, and how many the operators answered.
pub static COULSUM_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static COULSUM_BY_OPERATORS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// `|c| > DAEPS` keeps a coefficient; anything else is zero.
#[inline(always)]
fn keep(c: f64, epsilon: f64) -> f64 {
    if c.abs() > epsilon { c } else { 0.0 }
}

/// Series kept per pair, each `cap` coefficients long.
const THEIRS: usize = 0; // 3: the other particle's x, y, z as last seen
const D: usize = 3; // 3: coordinate differences
const TZ: usize = 6; // RELG^2 * dz
const R2: usize = 7;
const ROOT_RAW: usize = 8; // square root before the cutoff: the recurrence reads these
const ROOT: usize = 9;
const CUBE: usize = 10;
const INVERSE: usize = 11;
const TERM: usize = 12; // 3: this pair's contribution on each axis
const SERIES: usize = 15;

/// Which coefficients of a pair's series are nonzero, one bit each.
const M_D: usize = 0; // 3
const M_TZ: usize = 3;
const M_XX: usize = 4;
const M_XY: usize = 5; // xx + yy
const M_R2: usize = 6;
const M_ROOT: usize = 7;
const M_INVERSE: usize = 8;
const M_SCALED: usize = 9; // 3: the charged differences
const M_TERM: usize = 12; // 3
const M_THEIRS: usize = 15; // 3
const MASKS: usize = 18;

/// Whether a term list made of these blocks, each ascending, is ascending
/// as a whole.
fn ascending(blocks: &[u64]) -> bool {
    let mut seen = 0u64;
    for &block in blocks {
        if block != 0 && seen != 0 && block.trailing_zeros() < 64 - seen.leading_zeros() {
            return false;
        }
        seen |= block;
    }
    true
}

/// Coefficient `c` of the product of two of a pair's series, summed the
/// way `DA * DA` sums it.
#[inline(always)]
fn product(block: &[f64], cap: usize, a: usize, b: usize, c: usize, epsilon: f64) -> f64 {
    let (a, b) = (
        &block[a * cap..a * cap + c + 1],
        &block[b * cap..b * cap + c + 1],
    );
    let mut acc = 0.0;
    for i in 0..=c {
        acc = fmadd(a[i], b[c - i], acc);
    }
    keep(acc, epsilon)
}

/// What earlier calls worked out, kept so a coefficient is computed once.
///
/// A Picard iteration raises the order by one each pass, and coefficient
/// `c` of every series here depends only on coefficients `0..=c` of the
/// two particles. So the next pass needs one new coefficient per series,
/// not the whole series again. Nothing is trusted: each call compares the
/// particles' coefficients with the copies the kept values were made
/// from, bit for bit, and recomputes from the first difference.
#[derive(Default)]
struct Memo {
    slots: usize,
    cap: usize,
    /// `slots * slots` blocks of `SERIES * cap`, pair `(i, k)` at `i * slots + k`.
    pairs: Vec<f64>,
    /// Coefficients each pair holds.
    have: Vec<u32>,
    /// [`MASKS`] bit sets per pair.
    masks: Vec<u64>,
    /// Each sum's term list, in the order the operators would hold it.
    listed: [Vec<u32>; 3],
    charge: Vec<u64>,
    /// Per particle `i`: its x, y, z as last seen, and what they were used with.
    mine: Vec<f64>,
    mine_have: Vec<u32>,
    scalars: Vec<[u64; 2]>,
    sum: [Vec<f64>; 3],
    ratios: Vec<f64>,
}

thread_local! {
    static MEMO: std::cell::RefCell<Memo> = std::cell::RefCell::new(Memo::default());
}

/// Whether the operators' term list for `mine - theirs`, cut down to the
/// coefficients in `kept`, is in ascending order. The list is `mine`'s
/// terms in their order, then the terms only `theirs` has, in its order.
fn difference_in_order(mine: &DA, theirs: &DA, kept: u64, len: usize) -> bool {
    let mut own = 0u64;
    let mut last = None;
    let mut rising = true;
    let mut visit = |index: u32, own: u64| {
        if (index as usize) < len && kept >> index & 1 == 1 && own >> index & 1 == 0 {
            rising &= last.is_none_or(|l| l < index);
            last = Some(index);
        }
    };
    for &index in &mine.nonzero {
        visit(index, 0);
    }
    for &index in &mine.nonzero {
        if (index as usize) < 64 {
            own |= 1 << index;
        }
    }
    for &index in &theirs.nonzero {
        visit(index, own);
    }
    rising
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
///
/// The series operators add a sparse product's terms in the order of the
/// left operand's term list, and a list can fall out of ascending order
/// when two series with zeros in different places are combined. The sums
/// here are ascending. Each call works out, from which coefficients are
/// zero, whether any product would have met such a list; if so the call
/// is handed to the operators themselves ([`coulsum_by_operators`]). The
/// sums come back with their term lists in the operators' order.
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
    ensure!(
        rt.config.num_vars == 1,
        "COULSUM needs series in one variable (DAINI order 1)"
    );
    let order = rt.config.max_order as usize;
    let epsilon = rt.config.epsilon;
    let monomials = rt.num_monomials;

    // A product of series with `a` and `b` terms takes the operators'
    // dense path, which ignores list order, when `a * b` is above this.
    let cut = order.min(rt.dense_prefix.len().saturating_sub(1));
    let dense_len = rt.dense_prefix.get(cut).copied().unwrap_or(0);
    let dense = dense_len > 0 && !rt.dense_ij.is_empty();
    drop(rt);

    let by_list = |a: u32, b: u32| !(dense && (a as usize) * (b as usize) > dense_len);
    let (i, n) = (i as usize, n as usize);
    ensure!(
        i >= 1 && i <= n,
        "COULSUM: particle {i} is not one of 1..{n}"
    );
    ensure!(
        y.len() >= 6 * n && phi.len() >= 6 * n && mq.len() >= 2 * n,
        "COULSUM: an array is too short for {n} particles"
    );

    let want_radii = want_radii != 0.0;
    ensure!(
        !want_radii || order >= 2,
        "COULSUM: radii need order 2 or more"
    );

    let len = order + 1;
    ensure!(
        len <= monomials,
        "COULSUM: order {order} is above the DAINI order"
    );

    let coefficient = |series: &DA, c: usize| series.coeffs.get(c).copied().unwrap_or(0.0);
    COULSUM_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if len > 64 {
        COULSUM_BY_OPERATORS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return coulsum_by_operators(y, mq, i, n, relg, order, want_radii, phi, radii);
    }
    let relg2 = relg.powf(2.0);

    // A scalar at or under the cutoff multiplies to nothing.
    let live = relg2.abs() > epsilon;

    let in_order = MEMO.with(|memo| -> Result<bool> {
        let mut in_order = true;
        let m = &mut *memo.borrow_mut();
        if m.cap != monomials || m.slots < n {
            let slots = n.max(m.slots);
            *m = Memo {
                slots,
                cap: monomials,
                pairs: vec![0.0; slots * slots * SERIES * monomials],
                have: vec![0; slots * slots],
                masks: vec![0; slots * slots * MASKS],
                charge: vec![0; slots * slots],
                mine: vec![0.0; slots * 3 * monomials],
                mine_have: vec![0; slots],
                scalars: vec![[0; 2]; slots],
                ..Memo::default()
            };
        }
        let cap = m.cap;

        for v in &mut m.sum {
            v.clear();
            v.resize(len, 0.0);
        }
        for list in &mut m.listed {
            list.clear();
        }

        let window = if len == 64 {
            u64::MAX
        } else {
            (1u64 << len) - 1
        };
        let mut sum_set = [0u64; 3];

        if want_radii {
            radii.clear();
        }

        // How far this particle's own series still match what was kept.
        let me = i - 1;
        let mine_block = &mut m.mine[me * 3 * cap..(me + 1) * 3 * cap];
        let scalars = [relg2.to_bits(), epsilon.to_bits()];
        let mut mine_same = if m.scalars[me] == scalars {
            (m.mine_have[me] as usize).min(len)
        } else {
            0
        };
        for axis in 0..3 {
            let series = &y[6 * me + axis];
            let kept = &mine_block[axis * cap..];
            let mut c = 0;
            while c < mine_same && coefficient(series, c).to_bits() == kept[c].to_bits() {
                c += 1;
            }
            mine_same = c;
        }

        for k in 1..=n {
            if k == i {
                continue;
            }
            let pair = me * m.slots + (k - 1);
            let block = &mut m.pairs[pair * SERIES * cap..(pair + 1) * SERIES * cap];
            let charge = mq[2 * (k - 1) + 1];
            let mut same = if m.charge[pair] == charge.to_bits() {
                (m.have[pair] as usize).min(mine_same)
            } else {
                0
            };
            for axis in 0..3 {
                let series = &y[6 * (k - 1) + axis];
                let kept = &block[(THEIRS + axis) * cap..];
                let mut c = 0;
                while c < same && coefficient(series, c).to_bits() == kept[c].to_bits() {
                    c += 1;
                }
                same = c;
            }
            let charged = charge.abs() > epsilon;
            let masks = &mut m.masks[pair * MASKS..(pair + 1) * MASKS];
            if same < len {
                let kept = (1u64 << same) - 1;
                for mask in masks.iter_mut() {
                    *mask &= kept;
                }
            }
            for c in same..len {
                let bit = |v: f64| u64::from(v != 0.0) << c;
                for axis in 0..3 {
                    let theirs = coefficient(&y[6 * (k - 1) + axis], c);
                    block[(THEIRS + axis) * cap + c] = theirs;
                    block[(D + axis) * cap + c] =
                        keep(coefficient(&y[6 * me + axis], c) - theirs, epsilon);
                    masks[M_THEIRS + axis] |= bit(theirs);
                    masks[M_D + axis] |= bit(block[(D + axis) * cap + c]);
                }
                block[TZ * cap + c] = if live {
                    keep(block[(D + 2) * cap + c] * relg2, epsilon)
                } else {
                    0.0
                };
                masks[M_TZ] |= bit(block[TZ * cap + c]);
                let xx = product(block, cap, D, D, c, epsilon);
                let yy = product(block, cap, D + 1, D + 1, c, epsilon);
                let zz = product(block, cap, TZ, D + 2, c, epsilon);
                let xy = keep(xx + yy, epsilon);
                block[R2 * cap + c] = keep(xy + zz, epsilon);
                masks[M_XX] |= bit(xx);
                masks[M_XY] |= bit(xy);
                masks[M_R2] |= bit(block[R2 * cap + c]);

                if c == 0 {
                    block[ROOT_RAW * cap] = block[R2 * cap].sqrt();
                } else {
                    let half = 0.5 / block[ROOT_RAW * cap];
                    let mut v = block[R2 * cap + c];
                    for j in 1..c {
                        let (a, b) = (block[ROOT_RAW * cap + j], block[ROOT_RAW * cap + c - j]);
                        if a != 0.0 && b != 0.0 {
                            v = fmadd(a * -1.0, b, v);
                        }
                    }
                    block[ROOT_RAW * cap + c] = v * half;
                }
                block[ROOT * cap + c] = keep(block[ROOT_RAW * cap + c], epsilon);
                masks[M_ROOT] |= bit(block[ROOT * cap + c]);
                block[CUBE * cap + c] = product(block, cap, R2, ROOT, c, epsilon);

                let g0 = block[CUBE * cap];
                let mut sum = 0.0;
                for g in 1..=c {
                    sum = fmadd(block[CUBE * cap + g], block[INVERSE * cap + c - g], sum);
                }
                let numerator = if c == 0 { 1.0 } else { 0.0 };
                block[INVERSE * cap + c] = keep((numerator - sum) * (1.0 / g0), epsilon);
                masks[M_INVERSE] |= bit(block[INVERSE * cap + c]);

                for axis in 0..3 {
                    // The charged difference times the inverse cube.
                    let mut acc = 0.0;
                    for j in 0..=c {
                        let d = block[(D + axis) * cap + j];
                        let scaled = if charged {
                            keep(d * charge, epsilon)
                        } else {
                            0.0
                        };
                        acc = fmadd(scaled, block[INVERSE * cap + c - j], acc);
                    }
                    block[(TERM + axis) * cap + c] = keep(acc, epsilon);
                    masks[M_TERM + axis] |= bit(block[(TERM + axis) * cap + c]);
                    let d = block[(D + axis) * cap + c];
                    masks[M_SCALED + axis] |= bit(if charged {
                        keep(d * charge, epsilon)
                    } else {
                        0.0
                    });
                }
            }
            // Would the operators have multiplied by a list out of order?
            // A difference lists this particle's terms, then the other's
            // that this one lacks; a sum lists its first operand's terms,
            // then what each later one adds.
            let count = |mask: u64| (mask & window).count_ones();
            for axis in 0..3 {
                let listed = |mask: u64| difference_in_order(&y[6 * me + axis], &y[6 * (k - 1) + axis], mask, len);
                let d = masks[M_D + axis];
                if axis < 2 {
                    in_order &= !by_list(count(d), count(d)) || listed(d);
                } else {
                    in_order &= !by_list(count(masks[M_TZ]), count(d)) || listed(masks[M_TZ]);
                }
                let scaled = masks[M_SCALED + axis];
                in_order &= !by_list(count(scaled), count(masks[M_INVERSE])) || listed(scaled);
            }
            let (xx, xy, r2) = (
                masks[M_XX] & window,
                masks[M_XY] & window,
                masks[M_R2] & window,
            );
            in_order &= !by_list(count(r2), count(masks[M_ROOT]))
                || ascending(&[xx & xy & r2, xy & !xx & r2, r2 & !xy]);
            let term_sets = [
                masks[M_TERM] & window,
                masks[M_TERM + 1] & window,
                masks[M_TERM + 2] & window,
            ];
            if same < len {
                m.have[pair] = len as u32;
                m.charge[pair] = charge.to_bits();
            }
            let block = &m.pairs[pair * SERIES * cap..(pair + 1) * SERIES * cap];
            let f0 = block[R2 * cap];
            if !(f0 > 0.0) {
                m.have[pair] = 0;
                bail!("COULSUM: particles {i} and {k} are at the same place");
            }
            let g0 = block[CUBE * cap];
            if !g0.abs().is_finite() || g0.abs() <= epsilon {
                m.have[pair] = 0;
                bail!("COULSUM: division by zero for particles {i} and {k}");
            }
            if want_radii {
                m.ratios.clear();
                let c0 = f0.abs();
                m.ratios.push(block[R2 * cap + 1].abs() / c0);
                for j in 2..=order {
                    m.ratios
                        .push((block[R2 * cap + j].abs() / c0).powf(1.0 / j as f64));
                }
                m.ratios.sort_unstable_by(f64::total_cmp);
                radii.push(1.0 / (m.ratios[order - 2] + m.ratios[order - 1]));
            }
            for axis in 0..3 {
                let term = &block[(TERM + axis) * cap..(TERM + axis) * cap + len];
                let mut nonzero = 0u64;
                for (c, (total, &t)) in m.sum[axis].iter_mut().zip(term).enumerate() {
                    *total = keep(*total + t, epsilon);
                    nonzero |= u64::from(*total != 0.0) << c;
                }
                // The sum keeps its list order, drops what cancelled, and
                // appends the terms only this pair brought.
                let (old, new) = (sum_set[axis], term_sets[axis] & !sum_set[axis] & nonzero);
                if old & !nonzero != 0 {
                    m.listed[axis].retain(|&c| nonzero >> c & 1 == 1);
                }
                let mut rest = new;
                while rest != 0 {
                    m.listed[axis].push(rest.trailing_zeros());
                    rest &= rest - 1;
                }
                sum_set[axis] = (old & nonzero) | new;
            }
        }
        if mine_same < len {
            let mine_block = &mut m.mine[me * 3 * cap..(me + 1) * 3 * cap];
            for axis in 0..3 {
                for c in mine_same..len {
                    mine_block[axis * cap + c] = coefficient(&y[6 * me + axis], c);
                }
            }
            m.mine_have[me] = len as u32;
            m.scalars[me] = scalars;
            // Pairs with particles beyond `n` were not refreshed just now,
            // and they were worked out from the series this replaces.
            for k in n..m.slots {
                let have = &mut m.have[me * m.slots + k];
                *have = (*have).min(mine_same as u32);
            }
        }
        for axis in 0..3 {
            let (mut coeffs, mut nonzero) = f64::pool_alloc_da(monomials, len);
            for &c in &m.listed[axis] {
                coeffs[c as usize] = m.sum[axis][c as usize];
                nonzero.push(c);
            }
            phi[6 * me + 3 + axis] = DA { coeffs, nonzero };
        }
        Ok(in_order)
    })?;
    if !in_order {
        COULSUM_BY_OPERATORS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        #[cfg(test)]
        tests::FALLBACKS.with(|f| f.set(f.get() + 1));
        return coulsum_by_operators(y, mq, i, n, relg, order, want_radii, phi, radii);
    }
    Ok(())
}

/// The same sum with the series operators, as the loop in the module
/// comment would run it. The reference for [`coulsum`], and what it falls
/// back to when term order could change the rounding.
#[allow(clippy::too_many_arguments)]
pub fn coulsum_by_operators(
    y: &[DA],
    mq: &[f64],
    i: usize,
    n: usize,
    relg: f64,
    order: usize,
    want_radii: bool,
    phi: &mut [DA],
    radii: &mut Vec<f64>,
) -> Result<()> {
    use crate::intrinsics::sqrt::RosySQRT;
    use crate::operators::*;
    let mut sum = [
        DA::from_coeff(0.0),
        DA::from_coeff(0.0),
        DA::from_coeff(0.0),
    ];
    if want_radii {
        radii.clear();
    }
    let mine = 6 * (i - 1);
    for k in 1..=n {
        if k == i {
            continue;
        }
        let theirs = 6 * (k - 1);
        let d = |axis: usize| RosySub::rosy_sub(&y[mine + axis], &y[theirs + axis]);
        let mut r2 = RosyMult::rosy_mult(&d(0)?, &d(0)?)?;
        r2.add_assign_exact(&RosyMult::rosy_mult(&d(1)?, &d(1)?)?)?;
        let scaled = RosyMult::rosy_mult(&RosyPow::rosy_pow(&relg, &2f64)?, &d(2)?)?;
        r2.add_assign_exact(&RosyMult::rosy_mult(&scaled, &d(2)?)?)?;
        if want_radii {
            let c = |j: usize| RosyExtract::rosy_extract(&r2, &(j as f64)).map(f64::abs);
            let mut ratios = vec![c(1)? / c(0)?];
            for j in 2..=order {
                ratios.push((c(j)? / c(0)?).powf(1.0 / j as f64));
            }
            ratios.sort_by(f64::total_cmp);
            radii.push(1.0 / (ratios[order - 2] + ratios[order - 1]));
        }
        let w = RosyDiv::rosy_div(
            &1f64,
            &RosyMult::rosy_mult(&r2, &RosySQRT::rosy_sqrt(&r2)?)?,
        )?;
        for (axis, total) in sum.iter_mut().enumerate() {
            let term =
                RosyMult::rosy_mult(&RosyMult::rosy_mult(&mq[2 * (k - 1) + 1], &d(axis)?)?, &w)?;
            total.add_assign_exact(&term)?;
        }
    }
    for (axis, total) in sum.into_iter().enumerate() {
        phi[mine + 3 + axis] = total;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taylor::{init_taylor, set_truncation_order as set_max_order};

    thread_local! {
        pub(super) static FALLBACKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    fn by_operators(
        y: &[DA],
        mq: &[f64],
        i: usize,
        n: usize,
        relg: f64,
        order: usize,
        radii: Option<&mut Vec<f64>>,
    ) -> [DA; 3] {
        let mut phi = vec![DA::zero(); 6 * n];
        let mut unused = Vec::new();
        let want = radii.is_some();
        coulsum_by_operators(
            y,
            mq,
            i,
            n,
            relg,
            order,
            want,
            &mut phi,
            radii.unwrap_or(&mut unused),
        )
        .unwrap();
        let at = 6 * (i - 1) + 3;
        [phi[at].clone(), phi[at + 1].clone(), phi[at + 2].clone()]
    }

    fn bits(series: &DA, order: usize) -> Vec<u64> {
        (0..=order)
            .map(|k| series.coeffs.get(k).copied().unwrap_or(0.0).to_bits())
            .collect()
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
        let mut y: Vec<DA> = Vec::new();
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
                    series.set_coeff(
                        crate::taylor::Monomial::new(exponents),
                        random() * 0.3f64.powi(k as i32),
                    );
                }
                y.push(series);
            }
        }
        let mq: Vec<f64> = (0..2 * n)
            .map(|j| if j == 7 { 0.0 } else { 1.0 + random() })
            .collect();
        for order in 0..=full {
            set_max_order(order as u32).unwrap();
            for i in 1..=n {
                let want = order >= 2;
                let mut expected_radii = Vec::new();
                let expected = by_operators(
                    &y,
                    &mq,
                    i,
                    n,
                    1.37,
                    order,
                    want.then_some(&mut expected_radii),
                );
                let mut phi = vec![DA::zero(); 6 * n];
                let mut radii = vec![123.0];
                coulsum(
                    &y,
                    &mq,
                    i as f64,
                    n as f64,
                    1.37,
                    if want { 1.0 } else { 0.0 },
                    &mut phi,
                    &mut radii,
                )
                .unwrap();
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
        // Kept values must never leak: change inputs between calls, jump
        // between orders, shrink the set, and move a charge.
        let mut mq = mq;
        let check = |y: &[DA], mq: &[f64], n: usize, order: usize, i: usize| {
            set_max_order(order as u32).unwrap();
            let expected = by_operators(y, mq, i, n, 1.37, order, None);
            let mut phi = vec![DA::zero(); 6 * n];
            let mut radii = Vec::new();
            coulsum(y, mq, i as f64, n as f64, 1.37, 0.0, &mut phi, &mut radii).unwrap();
            for axis in 0..3 {
                assert_eq!(
                    bits(&phi[6 * (i - 1) + 3 + axis], full),
                    bits(&expected[axis], full),
                    "order {order}, particle {i}, n {n}"
                );
                assert_eq!(
                    phi[6 * (i - 1) + 3 + axis].nonzero,
                    expected[axis].nonzero,
                    "term list, order {order}, particle {i}, n {n}"
                );
            }
        };
        // A cold start: nobody moves across the beam, so x and y have no
        // linear term while z does. Every particle has the same zeros, and
        // the kept values must still answer.
        let mut cold: Vec<DA> = Vec::new();
        for p in 0..n {
            for axis in 0..6 {
                let mut series = DA::from_coeff(p as f64 * 0.9 + axis as f64 * 0.23 + random());
                for k in 1..=full {
                    if k == 1 && axis < 2 {
                        continue;
                    }
                    let mut exponents = [0u8; crate::taylor::MAX_VARS];
                    exponents[0] = k as u8;
                    series.set_coeff(
                        crate::taylor::Monomial::new(exponents),
                        random() * 0.3f64.powi(k as i32),
                    );
                }
                cold.push(series);
            }
        }
        for order in 0..=full {
            for i in 1..=n {
                check(&cold, &mq, n, order, i);
            }
        }
        assert_eq!(
            FALLBACKS.with(|f| f.get()),
            0,
            "a cold start must not need the operators"
        );
        // An integrator adds the constant last, so a series it is still
        // building lists the constant last. Those are full length, and
        // full-length products do not go by list order; the shorter series
        // of particles already done come from POLVAL in ascending order.
        let mut rotated = y.clone();
        for series in &mut rotated {
            if series.nonzero.first() == Some(&0) && series.nonzero.len() == full + 1 {
                series.nonzero.rotate_left(1);
            }
        }
        for order in 0..=full {
            for i in 1..=n {
                check(&rotated, &mq, n, order, i);
            }
        }
        assert_eq!(FALLBACKS.with(|f| f.get()), 0, "a constant listed last must not need the operators");
        // First with changes that keep every term list as it is, so the
        // kept values are what answers.
        assert_eq!(
            FALLBACKS.with(|f| f.get()),
            0,
            "clean series must not need the operators"
        );
        for round in 0..60usize {
            let particle = [0, 1, 3, 4, 6, 7][round % 6];
            let top = if particle % 3 == 0 { full } else { 5 };
            let at = round * 5 % (top + 1);
            let mut exponents = [0u8; crate::taylor::MAX_VARS];
            exponents[0] = at as u8;
            let size = if at == 0 {
                3.0 + round as f64
            } else {
                0.3f64.powi(at as i32)
            };
            y[6 * particle + round % 3].set_coeff(
                crate::taylor::Monomial::new(exponents),
                size * (1.0 + random()),
            );
            if round % 9 == 4 {
                mq[2 * (round % n) + 1] = 0.5 + random();
            }
            let n_now = if round % 4 == 3 { 5 } else { n };
            for order in [
                round * 3 % (full + 1),
                full,
                round % 4,
                (round * 11 + 2) % (full + 1),
            ] {
                for i in 1..=n_now {
                    check(&y, &mq, n_now, order, i);
                }
            }
        }
        assert_eq!(
            FALLBACKS.with(|f| f.get()),
            0,
            "clean series must not need the operators"
        );
        for round in 0..40usize {
            let particle = round * 7 % n;
            let at = round * 5 % (full + 1);
            let mut exponents = [0u8; crate::taylor::MAX_VARS];
            exponents[0] = at as u8;
            let shift = if at == 0 { 3.0 + round as f64 } else { 0.01 };
            y[6 * particle + round % 3].set_coeff(
                crate::taylor::Monomial::new(exponents),
                shift * (1.0 + random()),
            );
            if round % 9 == 4 {
                mq[2 * (round % n) + 1] = 0.5 + random();
            }
            let n_now = if round % 4 == 3 { 5 } else { n };
            for order in [
                round * 3 % (full + 1),
                full,
                round % 4,
                (round * 11 + 2) % (full + 1),
            ] {
                for i in 1..=n_now {
                    check(&y, &mq, n_now, order, i);
                }
            }
        }
        set_max_order(full as u32).unwrap();
        crate::taylor::set_epsilon(epsilon_before).unwrap();
    }
}
