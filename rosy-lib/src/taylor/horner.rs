//! Horner evaluation and FixedMultiplier for DA transcendentals.
//!
//! Separated from da.rs to avoid LLVM code layout changes that degrade
//! the hot-path performance of the regular DA multiply.

use anyhow::Result;
use num_complex::Complex64;

#[cfg(feature = "nightly-simd")]
use std::simd::prelude::*;
#[cfg(feature = "nightly-simd")]
use std::simd::StdFloat;

use super::da::{DA, DACoefficient};
use super::config::{get_runtime, MULT_INVALID, TaylorRuntime};

// ============================================================================
// FixedMultiplier — Cache-optimized multiply for a fixed RHS operand
// ============================================================================

/// Precomputed multiplication structure for a fixed sparse RHS.
///
/// Groups contributions by output monomial index for sequential writes
/// (optimal cache behavior), and enables SIMD accumulation with the
/// `nightly-simd` feature. Used by `horner_eval` where the same DA
/// (da_prime) is multiplied repeatedly.
///
/// Layout: for each output monomial k in 0..N, the slice
/// `lhs_indices[offsets[k]..offsets[k+1]]` and `rhs_coeffs[offsets[k]..offsets[k+1]]`
/// contain the LHS indices and fixed RHS coefficients that contribute to k.
///
/// The `output_max_order` array stores the order of each output monomial for
/// progressive truncation support.
pub struct FixedMultiplier {
    offsets: Vec<u32>,
    lhs_indices: Vec<u32>,
    rhs_coeffs: Vec<f64>,
    num_monomials: usize,
    output_orders: Vec<u8>,
}

impl FixedMultiplier {
    /// Build a FixedMultiplier for a given DA (the fixed RHS in multiplication).
    ///
    /// Precomputes all (output_k, lhs_i, rhs_coeff_j) triples from the mult_table,
    /// grouped by output index for sequential writes at multiply time.
    pub fn new(rhs: &DA<f64>, rt: &TaylorRuntime) -> Option<Self> {
        let table = rt.mult_table.as_ref()?;
        let n = rt.num_monomials;
        let max_order = rt.config.max_order;

        // Count contributions per output monomial
        let mut counts = vec![0u32; n];
        for &j in &rhs.nonzero {
            let ju = j as usize;
            for i in 0..n {
                let k = table[i * n + ju];
                if k != MULT_INVALID && rt.monomial_orders[k as usize] as u32 <= max_order {
                    counts[k as usize] += 1;
                }
            }
        }

        // Build offset array (prefix sum)
        let mut offsets = vec![0u32; n + 1];
        for k in 0..n {
            offsets[k + 1] = offsets[k] + counts[k];
        }
        let total = offsets[n] as usize;

        // Fill in pairs, using write_pos to track insertion point per group
        let mut lhs_indices = vec![0u32; total];
        let mut rhs_coeffs = vec![0.0f64; total];
        let mut write_pos = offsets[..n].to_vec();

        for &j in &rhs.nonzero {
            let ju = j as usize;
            let bj = rhs.coeffs[ju];
            for i in 0..n {
                let k = table[i * n + ju];
                if k != MULT_INVALID && rt.monomial_orders[k as usize] as u32 <= max_order {
                    let pos = write_pos[k as usize] as usize;
                    lhs_indices[pos] = i as u32;
                    rhs_coeffs[pos] = bj;
                    write_pos[k as usize] += 1;
                }
            }
        }

        Some(Self { offsets, lhs_indices, rhs_coeffs, num_monomials: n, output_orders: rt.monomial_orders.clone() })
    }

    /// Multiply LHS by the fixed RHS, returning a new DA.
    pub fn multiply_to_da(&self, lhs: &DA<f64>, epsilon: f64) -> DA<f64> {
        self.multiply_to_da_truncated(lhs, epsilon, u32::MAX)
    }

    /// Multiply LHS by the fixed RHS with progressive truncation.
    ///
    /// Only produces output monomials with order <= `trunc_order`.
    /// Used by Horner evaluation for progressive order truncation (#18).
    pub fn multiply_to_da_truncated(&self, lhs: &DA<f64>, epsilon: f64, trunc_order: u32) -> DA<f64> {
        let n = self.num_monomials;
        let mut coeffs = f64::pool_alloc(n);
        let mut nonzero = Vec::new();

        for k in 0..n {
            // Progressive truncation: skip output monomials above the current truncation order
            if self.output_orders[k] as u32 > trunc_order { continue; }

            let start = self.offsets[k] as usize;
            let end = self.offsets[k + 1] as usize;
            if start == end { continue; }

            let sum = Self::accumulate(&lhs.coeffs, &self.lhs_indices, &self.rhs_coeffs, start, end);

            if sum.abs() > epsilon {
                coeffs[k] = sum;
                nonzero.push(k as u32);
            }
        }

        DA { coeffs, nonzero }
    }

    /// Accumulate the dot product for one output monomial.
    /// With `nightly-simd`: uses f64x4 SIMD FMA.
    /// Without: scalar FMA loop.
    #[inline]
    fn accumulate(lhs_coeffs: &[f64], lhs_indices: &[u32], rhs_coeffs: &[f64], start: usize, end: usize) -> f64 {
        #[cfg(feature = "nightly-simd")]
        {
            const LANES: usize = 4;
            let count = end - start;
            let chunks = count / LANES;
            let mut acc = Simd::<f64, LANES>::splat(0.0);

            for c in 0..chunks {
                let off = start + c * LANES;
                // Gather 4 LHS coefficients at scattered indices
                let a = Simd::<f64, LANES>::from_array([
                    lhs_coeffs[lhs_indices[off] as usize],
                    lhs_coeffs[lhs_indices[off + 1] as usize],
                    lhs_coeffs[lhs_indices[off + 2] as usize],
                    lhs_coeffs[lhs_indices[off + 3] as usize],
                ]);
                // Load 4 contiguous fixed RHS coefficients
                let b = Simd::<f64, LANES>::from_slice(&rhs_coeffs[off..]);
                acc = a.mul_add(b, acc);
            }

            let mut sum = acc.reduce_sum();
            for p in (start + chunks * LANES)..end {
                sum = lhs_coeffs[lhs_indices[p] as usize].mul_add(rhs_coeffs[p], sum);
            }
            return sum;
        }

        #[cfg(not(feature = "nightly-simd"))]
        {
            let mut sum = 0.0f64;
            for p in start..end {
                sum = lhs_coeffs[lhs_indices[p] as usize].mul_add(rhs_coeffs[p], sum);
            }
            sum
        }
    }
}

// ============================================================================
// Horner evaluation — shared by all DA/CD transcendentals
// ============================================================================

impl DA<f64> {
    /// Evaluate a polynomial P(x) = c₀ + x·(c₁ + x·(c₂ + ···)) via Horner's method.
    ///
    /// `da_prime` is the fixed DA multiplied at each step (typically the input DA
    /// with constant part removed). Uses progressive order truncation: at step `i`
    /// from the end, the multiply is truncated to order `(n-1-i)` since higher-order
    /// terms cannot contribute to the final result (issue #18).
    ///
    /// Acquires the runtime lock once and passes it to all inner multiplies,
    /// avoiding repeated RwLock acquisition in the hot loop.
    #[inline(always)]
    pub fn horner_eval(da_prime: &DA<f64>, taylor_coeffs: &[f64]) -> Result<DA<f64>> {
        let rt = get_runtime()?;
        Self::horner_eval_with_rt(da_prime, taylor_coeffs, &rt)
    }

    /// Horner evaluation with an already-acquired runtime reference.
    ///
    /// Avoids the RwLock acquire when the caller already holds it
    /// (e.g. transcendental functions that need config + Horner).
    #[inline(always)]
    pub fn horner_eval_with_rt(da_prime: &DA<f64>, taylor_coeffs: &[f64], rt: &TaylorRuntime) -> Result<DA<f64>> {
        let n = taylor_coeffs.len();
        if n == 0 { return Ok(DA::zero()); }
        if n == 1 { return Ok(DA::from_coeff(taylor_coeffs[0])); }

        let full_order = rt.config.max_order;

        let mut result = DA::from_coeff(taylor_coeffs[n - 1]);
        for i in (0..n - 1).rev() {
            let steps_from_end = (n - 1) - i;
            let trunc_order = (steps_from_end as u32).min(full_order);
            result = DA::multiply_truncated_with_rt(&result, da_prime, trunc_order, &rt)?;
            result.add_constant_in_place(taylor_coeffs[i]);
        }
        Ok(result)
    }

    /// Horner evaluation with FixedMultiplier for large DA arrays (N >= 500).
    ///
    /// Uses cache-optimized output-grouped multiplication with optional SIMD
    /// and progressive order truncation. Falls back to standard scatter multiply
    /// if mult_table is unavailable.
    pub fn horner_eval_fixed(da_prime: &DA<f64>, taylor_coeffs: &[f64]) -> Result<DA<f64>> {
        let n = taylor_coeffs.len();
        if n == 0 { return Ok(DA::zero()); }
        if n == 1 { return Ok(DA::from_coeff(taylor_coeffs[0])); }

        let rt = get_runtime()?;
        let epsilon = rt.config.epsilon;
        let full_order = rt.config.max_order;
        if let Some(fixed) = FixedMultiplier::new(da_prime, &rt) {
            let mut result = DA::from_coeff(taylor_coeffs[n - 1]);
            for i in (0..n - 1).rev() {
                let steps_from_end = (n - 1) - i;
                let trunc_order = (steps_from_end as u32).min(full_order);
                result = fixed.multiply_to_da_truncated(&result, epsilon, trunc_order);
                result.add_constant_in_place(taylor_coeffs[i]);
            }
            return Ok(result);
        }

        // Fallback: standard scatter multiply with progressive truncation (lock held)
        let mut result = DA::from_coeff(taylor_coeffs[n - 1]);
        for i in (0..n - 1).rev() {
            let steps_from_end = (n - 1) - i;
            let trunc_order = (steps_from_end as u32).min(full_order);
            result = DA::multiply_truncated_with_rt(&result, da_prime, trunc_order, &rt)?;
            result.add_constant_in_place(taylor_coeffs[i]);
        }
        Ok(result)
    }
}

impl DA<Complex64> {
    /// Horner evaluation for Complex DA with progressive truncation.
    /// Holds runtime lock for the entire loop.
    #[inline]
    pub fn horner_eval(cd_prime: &DA<Complex64>, taylor_coeffs: &[Complex64]) -> Result<DA<Complex64>> {
        let n = taylor_coeffs.len();
        if n == 0 { return Ok(DA::zero()); }
        if n == 1 { return Ok(DA::from_coeff(taylor_coeffs[0])); }

        let rt = get_runtime()?;
        let full_order = rt.config.max_order;

        let mut result = DA::from_coeff(taylor_coeffs[n - 1]);
        for i in (0..n - 1).rev() {
            let steps_from_end = (n - 1) - i;
            let trunc_order = (steps_from_end as u32).min(full_order);
            result = DA::multiply_truncated_with_rt(&result, cd_prime, trunc_order, &rt)?;
            result.add_constant_in_place(taylor_coeffs[i]);
        }

        Ok(result)
    }
}
