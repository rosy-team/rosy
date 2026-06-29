//! DA (Differential Algebra) — Flat-array Taylor series with free-list pool allocator.
//!
//! All hot-path operations are O(K) where K = number of non-zero terms.
//! A thread-local free-list recycles DA coefficient arrays — after the first
//! loop iteration, subsequent iterations allocate zero times.
//!
//! Invariant: entries NOT in the `nonzero` list are ALWAYS zero.
//! This is maintained by all operations and by the pool's clear-on-return.

use anyhow::{Context, Result};
use num_complex::Complex64;
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

use super::config::{MULT_INVALID, TaylorRuntime, get_config, get_runtime};
use super::{MAX_VARS, Monomial};

// ============================================================================
// Coefficient trait + pool
// ============================================================================

pub trait DACoefficient:
    Clone
    + Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + AddAssign
    + MulAdd
    + Neg<Output = Self>
    + PartialEq
    + fmt::Debug
    + fmt::Display
{
    fn zero() -> Self;
    fn one() -> Self;
    fn from_usize(n: usize) -> Self;
    fn abs(&self) -> f64;

    /// Get a zeroed Vec from the thread-local free list, or allocate fresh.
    fn pool_alloc(n: usize) -> Vec<Self>;
    /// Return a Vec to the thread-local free list for reuse.
    fn pool_return(v: Vec<Self>);
}

// ── f64 pool ────────────────────────────────────────────────────────────────

thread_local! {
    static F64_POOL: RefCell<Vec<Vec<f64>>> = RefCell::new(Vec::new());
    static BITSET_POOL: RefCell<Vec<Vec<u64>>> = RefCell::new(Vec::new());
}

/// Get a zeroed bitset from the thread-local pool, or allocate fresh.
pub(crate) fn bitset_pool_alloc(words: usize) -> Vec<u64> {
    BITSET_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        while let Some(v) = pool.pop() {
            if v.len() == words {
                return v;
            }
        }
        vec![0u64; words]
    })
}

/// Return a bitset to the pool after clearing the set bits (O(popcount)).
pub(crate) fn bitset_pool_return(mut v: Vec<u64>) {
    if v.is_empty() {
        return;
    }
    // Clear only the set words — most words are zero
    for w in v.iter_mut() {
        *w = 0;
    }
    BITSET_POOL.with(|pool| pool.borrow_mut().push(v));
}

impl DACoefficient for f64 {
    #[inline(always)]
    fn zero() -> Self {
        0.0
    }
    #[inline(always)]
    fn one() -> Self {
        1.0
    }
    #[inline(always)]
    fn from_usize(n: usize) -> Self {
        n as f64
    }
    #[inline(always)]
    fn abs(&self) -> f64 {
        f64::abs(*self)
    }

    fn pool_alloc(n: usize) -> Vec<Self> {
        F64_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            while let Some(v) = pool.pop() {
                if v.len() == n {
                    return v; // Pre-zeroed from clear-on-return
                }
                // Wrong size (DAINI changed parameters), discard
            }
            vec![0.0; n]
        })
    }

    fn pool_return(v: Vec<Self>) {
        if v.is_empty() {
            return;
        }
        F64_POOL.with(|pool| pool.borrow_mut().push(v));
    }
}

// ── Complex64 pool ──────────────────────────────────────────────────────────

thread_local! {
    static C64_POOL: RefCell<Vec<Vec<Complex64>>> = RefCell::new(Vec::new());
}

impl DACoefficient for Complex64 {
    #[inline(always)]
    fn zero() -> Self {
        Complex64::new(0.0, 0.0)
    }
    #[inline(always)]
    fn one() -> Self {
        Complex64::new(1.0, 0.0)
    }
    #[inline(always)]
    fn from_usize(n: usize) -> Self {
        Complex64::new(n as f64, 0.0)
    }
    #[inline(always)]
    fn abs(&self) -> f64 {
        self.norm()
    }

    fn pool_alloc(n: usize) -> Vec<Self> {
        C64_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            while let Some(v) = pool.pop() {
                if v.len() == n {
                    return v;
                }
            }
            vec![Complex64::new(0.0, 0.0); n]
        })
    }

    fn pool_return(v: Vec<Self>) {
        if v.is_empty() {
            return;
        }
        C64_POOL.with(|pool| pool.borrow_mut().push(v));
    }
}

// ── FMA trait ───────────────────────────────────────────────────────────────

pub trait MulAdd {
    fn mul_add(self, a: Self, b: Self) -> Self;
}

impl MulAdd for f64 {
    #[inline(always)]
    fn mul_add(self, a: Self, b: Self) -> Self {
        f64::mul_add(self, a, b)
    }
}

impl MulAdd for Complex64 {
    #[inline(always)]
    fn mul_add(self, a: Self, b: Self) -> Self {
        self * a + b
    }
}

// ============================================================================
// DA struct
// ============================================================================

/// Generic Taylor polynomial with flat-array storage, non-zero tracking,
/// and free-list pool allocation.
///
/// Invariant: `coeffs[i] == T::zero()` for all `i` NOT in `nonzero`.
pub struct DA<T: DACoefficient> {
    pub coeffs: Vec<T>,
    pub nonzero: Vec<u32>,
}

// Manual Clone: allocates from pool, copies only nonzero entries — O(K)
impl<T: DACoefficient> Clone for DA<T> {
    fn clone(&self) -> Self {
        let n = self.coeffs.len();
        if n == 0 {
            return Self {
                coeffs: Vec::new(),
                nonzero: Vec::new(),
            };
        }
        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            coeffs[i as usize] = self.coeffs[i as usize];
        }
        Self {
            coeffs,
            nonzero: self.nonzero.clone(),
        }
    }
}

// Drop: clear nonzero entries O(K) and return array to pool
impl<T: DACoefficient> Drop for DA<T> {
    fn drop(&mut self) {
        if self.coeffs.is_empty() {
            return;
        }
        // Restore invariant: zero out entries we used
        for &i in &self.nonzero {
            self.coeffs[i as usize] = T::zero();
        }
        let v = std::mem::take(&mut self.coeffs);
        T::pool_return(v);
    }
}

impl<T: DACoefficient> PartialEq for DA<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.nonzero.len() != other.nonzero.len() {
            return false;
        }
        for &i in &self.nonzero {
            if self.coeffs[i as usize] != other.coeffs[i as usize] {
                return false;
            }
        }
        for &i in &other.nonzero {
            if self.coeffs[i as usize] != other.coeffs[i as usize] {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// Basic methods
// ============================================================================

// Default delegates to zero(); requires the Taylor runtime to be initialized
// (i.e., DAINI must have been called) before any DA value is constructed.
impl<T: DACoefficient> Default for DA<T> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<T: DACoefficient> DA<T> {
    pub fn zero() -> Self {
        let rt = get_runtime().expect("Taylor system not initialized (call OV first)");
        Self {
            coeffs: T::pool_alloc(rt.num_monomials),
            nonzero: Vec::new(),
        }
    }

    pub fn from_coeff(value: T) -> Self {
        let rt = get_runtime().expect("Taylor system not initialized");
        let epsilon = rt.config.epsilon;
        let mut coeffs = T::pool_alloc(rt.num_monomials);
        let mut nonzero = Vec::new();
        if value.abs() > epsilon {
            coeffs[0] = value;
            nonzero.push(0);
        }
        Self { coeffs, nonzero }
    }

    pub fn variable(var_index: usize) -> Result<Self> {
        let rt = get_runtime()?;
        if var_index == 0 || var_index > rt.config.num_vars {
            anyhow::bail!(
                "Variable index {} out of range [1, {}]",
                var_index,
                rt.config.num_vars
            );
        }
        let flat_idx = rt.variable_indices[var_index - 1];
        let mut coeffs = T::pool_alloc(rt.num_monomials);
        coeffs[flat_idx as usize] = T::one();
        Ok(Self {
            coeffs,
            nonzero: vec![flat_idx],
        })
    }

    #[inline]
    pub fn constant_part(&self) -> T {
        self.coeffs[0]
    }

    pub fn get_coeff(&self, monomial: &Monomial) -> T {
        let rt = get_runtime().expect("Taylor system not initialized");
        if let Some(&idx) = rt.monomial_index.get(monomial) {
            self.coeffs[idx as usize]
        } else {
            T::zero()
        }
    }

    pub fn set_coeff(&mut self, monomial: Monomial, value: T) {
        let rt = get_runtime().expect("Taylor system not initialized");
        let epsilon = rt.config.epsilon;
        let idx = match rt.monomial_index.get(&monomial) {
            Some(&i) => i,
            None => return,
        };
        let was_nz = self.coeffs[idx as usize].abs() > epsilon;
        self.coeffs[idx as usize] = value;
        let is_nz = value.abs() > epsilon;

        if is_nz && !was_nz {
            self.nonzero.push(idx);
        } else if !is_nz && was_nz {
            if let Some(pos) = self.nonzero.iter().position(|&i| i == idx) {
                self.nonzero.swap_remove(pos);
            }
            self.coeffs[idx as usize] = T::zero();
        }
    }

    pub fn trim(&mut self) {
        let rt = get_runtime().expect("Taylor system not initialized");
        let epsilon = rt.config.epsilon;
        self.nonzero
            .retain(|&i| self.coeffs[i as usize].abs() > epsilon);
    }

    #[inline]
    pub fn num_terms(&self) -> usize {
        self.nonzero.len()
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.nonzero.is_empty()
    }

    pub fn from_coeffs(hash_coeffs: FxHashMap<Monomial, T>) -> Self {
        let rt = get_runtime().expect("Taylor system not initialized");
        let epsilon = rt.config.epsilon;
        let mut coeffs = T::pool_alloc(rt.num_monomials);
        let mut nonzero = Vec::new();
        for (mono, coeff) in hash_coeffs {
            if let Some(&idx) = rt.monomial_index.get(&mono) {
                if coeff.abs() > epsilon {
                    coeffs[idx as usize] = coeff;
                    nonzero.push(idx);
                }
            }
        }
        Self { coeffs, nonzero }
    }

    pub fn coeffs_entries<'a>(
        &'a self,
        rt: &'a TaylorRuntime,
    ) -> impl Iterator<Item = (&'a Monomial, T)> + 'a {
        self.nonzero
            .iter()
            .map(move |&i| (&rt.monomial_list[i as usize], self.coeffs[i as usize]))
    }

    pub fn coeffs_iter(&self) -> Vec<(Monomial, T)> {
        let rt = get_runtime().expect("Taylor system not initialized");
        self.nonzero
            .iter()
            .map(|&i| (rt.monomial_list[i as usize], self.coeffs[i as usize]))
            .collect()
    }

    /// O(1) amortized. Used by Horner's method.
    pub fn add_constant_in_place(&mut self, value: T) {
        let was_nz = self.coeffs[0].abs() > 1e-15;
        self.coeffs[0] = self.coeffs[0] + value;
        let is_nz = self.coeffs[0].abs() > 1e-15;

        if is_nz {
            let mut seen_constant = false;
            self.nonzero.retain(|&i| {
                if i == 0 {
                    if seen_constant {
                        false
                    } else {
                        seen_constant = true;
                        true
                    }
                } else {
                    true
                }
            });
            if !seen_constant {
                self.nonzero.push(0);
            }
        } else if was_nz || self.nonzero.contains(&0) {
            self.nonzero.retain(|&i| i != 0);
            self.coeffs[0] = T::zero();
        }
    }

    /// Create the "prime" (non-constant) part of a DA for series evaluation.
    ///
    /// Clones the DA and zeroes its constant part in O(1). Unlike the old
    /// pattern of `da.clone()` + `set_coeff(Monomial::constant(), 0.0)`,
    /// this avoids the RwLock acquisition and HashMap lookup in `set_coeff`.
    pub fn make_prime(&self) -> Self {
        let mut prime = self.clone();
        if prime.coeffs[0].abs() > 1e-15 {
            prime.coeffs[0] = T::zero();
            // Remove 0 from nonzero list
            if let Some(pos) = prime.nonzero.iter().position(|&i| i == 0) {
                prime.nonzero.swap_remove(pos);
            }
        }
        prime
    }
}

// ============================================================================
// Addition: O(K_self + K_rhs), pool-allocated result
// ============================================================================

impl<T: DACoefficient> Add<&DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;

    fn add(self, rhs: &DA<T>) -> Self::Output {
        let rt = get_runtime()?;
        let n = rt.num_monomials;
        let epsilon = rt.config.epsilon;
        let max_order = rt.config.max_order as u8;
        let orders = &rt.monomial_orders;

        let words = (n + 63) / 64;
        let mut self_set = vec![0u64; words];

        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            let iu = i as usize;
            if orders[iu] <= max_order {
                coeffs[iu] = self.coeffs[iu];
                self_set[iu / 64] |= 1u64 << (iu % 64);
            }
        }

        for &j in &rhs.nonzero {
            let ju = j as usize;
            if orders[ju] <= max_order {
                coeffs[ju] = coeffs[ju] + rhs.coeffs[ju];
            }
        }

        let mut nonzero = Vec::with_capacity(self.nonzero.len() + rhs.nonzero.len());
        let mut result_set = vec![0u64; words];
        for &i in &self.nonzero {
            let iu = i as usize;
            if orders[iu] <= max_order && coeffs[iu].abs() > epsilon {
                if result_set[iu / 64] & (1u64 << (iu % 64)) == 0 {
                    nonzero.push(i);
                    result_set[iu / 64] |= 1u64 << (iu % 64);
                }
            } else {
                coeffs[iu] = T::zero();
            }
        }
        for &j in &rhs.nonzero {
            let ju = j as usize;
            if self_set[ju / 64] & (1u64 << (ju % 64)) == 0 {
                if orders[ju] <= max_order && coeffs[ju].abs() > epsilon {
                    if result_set[ju / 64] & (1u64 << (ju % 64)) == 0 {
                        nonzero.push(j);
                        result_set[ju / 64] |= 1u64 << (ju % 64);
                    }
                } else {
                    coeffs[ju] = T::zero();
                }
            }
        }

        Ok(DA { coeffs, nonzero })
    }
}

impl<T: DACoefficient> Add<DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn add(self, rhs: DA<T>) -> Self::Output {
        &self + &rhs
    }
}
impl<T: DACoefficient> Add<&DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn add(self, rhs: &DA<T>) -> Self::Output {
        &self + rhs
    }
}
impl<T: DACoefficient> Add<DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;
    fn add(self, rhs: DA<T>) -> Self::Output {
        self + &rhs
    }
}
impl<T: DACoefficient> Add<T> for DA<T> {
    type Output = Result<DA<T>>;
    fn add(self, rhs: T) -> Self::Output {
        &self + rhs
    }
}
impl<T: DACoefficient> Add<T> for &DA<T> {
    type Output = Result<DA<T>>;
    fn add(self, rhs: T) -> Self::Output {
        let mut result = self.clone();
        result.add_constant_in_place(rhs);
        Ok(result)
    }
}

// ============================================================================
// Negation: O(K), pool-allocated result
// ============================================================================

impl<T: DACoefficient> Neg for DA<T> {
    type Output = DA<T>;
    fn neg(self) -> Self::Output {
        -&self
    }
}

impl<T: DACoefficient> Neg for &DA<T> {
    type Output = DA<T>;
    fn neg(self) -> Self::Output {
        let n = self.coeffs.len();
        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            coeffs[i as usize] = -self.coeffs[i as usize];
        }
        DA {
            coeffs,
            nonzero: self.nonzero.clone(),
        }
    }
}

// ============================================================================
// Subtraction: O(K_self + K_rhs), pool-allocated result
// ============================================================================

impl<T: DACoefficient> Sub<&DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;

    fn sub(self, rhs: &DA<T>) -> Self::Output {
        let rt = get_runtime()?;
        let n = rt.num_monomials;
        let epsilon = rt.config.epsilon;
        let max_order = rt.config.max_order as u8;
        let orders = &rt.monomial_orders;

        let words = (n + 63) / 64;
        let mut self_set = vec![0u64; words];

        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            let iu = i as usize;
            if orders[iu] <= max_order {
                coeffs[iu] = self.coeffs[iu];
                self_set[iu / 64] |= 1u64 << (iu % 64);
            }
        }

        for &j in &rhs.nonzero {
            let ju = j as usize;
            if orders[ju] <= max_order {
                coeffs[ju] = coeffs[ju] - rhs.coeffs[ju];
            }
        }

        let mut nonzero = Vec::with_capacity(self.nonzero.len() + rhs.nonzero.len());
        let mut result_set = vec![0u64; words];
        for &i in &self.nonzero {
            let iu = i as usize;
            if orders[iu] <= max_order && coeffs[iu].abs() > epsilon {
                if result_set[iu / 64] & (1u64 << (iu % 64)) == 0 {
                    nonzero.push(i);
                    result_set[iu / 64] |= 1u64 << (iu % 64);
                }
            } else {
                coeffs[iu] = T::zero();
            }
        }
        for &j in &rhs.nonzero {
            let ju = j as usize;
            if self_set[ju / 64] & (1u64 << (ju % 64)) == 0 {
                if orders[ju] <= max_order && coeffs[ju].abs() > epsilon {
                    if result_set[ju / 64] & (1u64 << (ju % 64)) == 0 {
                        nonzero.push(j);
                        result_set[ju / 64] |= 1u64 << (ju % 64);
                    }
                } else {
                    coeffs[ju] = T::zero();
                }
            }
        }

        Ok(DA { coeffs, nonzero })
    }
}

impl<T: DACoefficient> Sub<DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn sub(self, rhs: DA<T>) -> Self::Output {
        &self - &rhs
    }
}
impl<T: DACoefficient> Sub<&DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn sub(self, rhs: &DA<T>) -> Self::Output {
        &self - rhs
    }
}
impl<T: DACoefficient> Sub<DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;
    fn sub(self, rhs: DA<T>) -> Self::Output {
        self - &rhs
    }
}
impl<T: DACoefficient> Sub<T> for DA<T> {
    type Output = Result<DA<T>>;
    fn sub(self, rhs: T) -> Self::Output {
        &self - rhs
    }
}
impl<T: DACoefficient> Sub<T> for &DA<T> {
    type Output = Result<DA<T>>;
    fn sub(self, rhs: T) -> Self::Output {
        self + (-rhs)
    }
}

// ============================================================================
// Multiplication: O(K²) inner loop, pool-allocated result, bitset tracking
// ============================================================================

impl<T: DACoefficient> DA<T> {
    /// Multiply two DAs with a custom truncation order (for progressive Horner).
    ///
    /// Uses inline order checking to skip pairs where `order_a + order_b > trunc_order`.
    /// No auxiliary data structures are allocated — just one comparison per pair.
    /// This enables progressive order truncation in Horner evaluation (issue #18).
    pub fn multiply_truncated(&self, rhs: &DA<T>, trunc_order: u32) -> Result<DA<T>> {
        let rt = get_runtime()?;
        Self::multiply_truncated_with_rt(self, rhs, trunc_order, &rt)
    }

    /// Inner implementation that takes an already-acquired runtime reference.
    /// Avoids redundant RwLock acquisition when called in a loop (e.g. Horner).
    pub(crate) fn multiply_truncated_with_rt(
        lhs: &DA<T>,
        rhs: &DA<T>,
        trunc_order: u32,
        rt: &TaylorRuntime,
    ) -> Result<DA<T>> {
        let n = rt.num_monomials;
        let epsilon = rt.config.epsilon;
        let orders = &rt.monomial_orders;
        let trunc_order_u8 = trunc_order as u8;

        let mut result = T::pool_alloc(n);
        let words = (n + 63) / 64;
        let mut written = bitset_pool_alloc(words);

        if let Some(table) = &rt.mult_table {
            for &i in &lhs.nonzero {
                let ci = lhs.coeffs[i as usize];
                let oi = orders[i as usize];
                if oi > trunc_order_u8 {
                    continue;
                }
                let max_b_order = trunc_order_u8 - oi;
                let row = i as usize * n;
                for &j in &rhs.nonzero {
                    if orders[j as usize] > max_b_order {
                        continue;
                    }
                    let k = table[row + j as usize];
                    if k != MULT_INVALID {
                        let ku = k as usize;
                        result[ku] = ci.mul_add(rhs.coeffs[j as usize], result[ku]);
                        written[ku / 64] |= 1u64 << (ku % 64);
                    }
                }
            }
        } else {
            for &i in &lhs.nonzero {
                let ci = lhs.coeffs[i as usize];
                let oi = orders[i as usize];
                if oi > trunc_order_u8 {
                    continue;
                }
                let max_b_order = trunc_order_u8 - oi;
                for &j in &rhs.nonzero {
                    if orders[j as usize] > max_b_order {
                        continue;
                    }
                    let product =
                        rt.monomial_list[i as usize].multiply(&rt.monomial_list[j as usize]);
                    if product.within_order(trunc_order) {
                        if let Some(&k) = rt.monomial_index.get(&product) {
                            let ku = k as usize;
                            result[ku] = ci.mul_add(rhs.coeffs[j as usize], result[ku]);
                            written[ku / 64] |= 1u64 << (ku % 64);
                        }
                    }
                }
            }
        }

        let mut nonzero = Vec::new();
        for word_idx in 0..words {
            let mut word = written[word_idx];
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let idx = word_idx * 64 + bit;
                if idx < n {
                    if result[idx].abs() > epsilon {
                        nonzero.push(idx as u32);
                    } else {
                        result[idx] = T::zero();
                    }
                }
                word &= word - 1;
            }
        }

        bitset_pool_return(written);

        Ok(DA {
            coeffs: result,
            nonzero,
        })
    }
}

impl<T: DACoefficient> Mul<&DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;

    fn mul(self, rhs: &DA<T>) -> Self::Output {
        let rt = get_runtime()?;
        let n = rt.num_monomials;
        let epsilon = rt.config.epsilon;
        let max_order = rt.config.max_order;
        let order_check = max_order < rt.init_order;

        let mut result = T::pool_alloc(n);

        let words = (n + 63) / 64;
        let mut written = bitset_pool_alloc(words);

        if let Some(table) = &rt.mult_table {
            for &i in &self.nonzero {
                let ci = self.coeffs[i as usize];
                let row = i as usize * n;
                for &j in &rhs.nonzero {
                    let k = table[row + j as usize];
                    if k != MULT_INVALID
                        && (!order_check || rt.monomial_orders[k as usize] as u32 <= max_order)
                    {
                        let ku = k as usize;
                        result[ku] = ci.mul_add(rhs.coeffs[j as usize], result[ku]);
                        written[ku / 64] |= 1u64 << (ku % 64);
                    }
                }
            }
        } else {
            for &i in &self.nonzero {
                let ci = self.coeffs[i as usize];
                for &j in &rhs.nonzero {
                    let product =
                        rt.monomial_list[i as usize].multiply(&rt.monomial_list[j as usize]);
                    if product.within_order(max_order) {
                        if let Some(&k) = rt.monomial_index.get(&product) {
                            let ku = k as usize;
                            result[ku] = ci.mul_add(rhs.coeffs[j as usize], result[ku]);
                            written[ku / 64] |= 1u64 << (ku % 64);
                        }
                    }
                }
            }
        }

        let mut nonzero = Vec::new();
        for word_idx in 0..words {
            let mut word = written[word_idx];
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let idx = word_idx * 64 + bit;
                if idx < n {
                    if result[idx].abs() > epsilon {
                        nonzero.push(idx as u32);
                    } else {
                        result[idx] = T::zero();
                    }
                }
                word &= word - 1;
            }
        }

        bitset_pool_return(written);

        Ok(DA {
            coeffs: result,
            nonzero,
        })
    }
}

impl<T: DACoefficient> Mul<DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn mul(self, rhs: DA<T>) -> Self::Output {
        &self * &rhs
    }
}
impl<T: DACoefficient> Mul<&DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn mul(self, rhs: &DA<T>) -> Self::Output {
        &self * rhs
    }
}
impl<T: DACoefficient> Mul<DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;
    fn mul(self, rhs: DA<T>) -> Self::Output {
        self * &rhs
    }
}

// Scalar multiply: O(K), pool-allocated
impl<T: DACoefficient> Mul<T> for DA<T> {
    type Output = Result<DA<T>>;
    fn mul(self, rhs: T) -> Self::Output {
        &self * rhs
    }
}

impl<T: DACoefficient> Mul<T> for &DA<T> {
    type Output = Result<DA<T>>;
    fn mul(self, rhs: T) -> Self::Output {
        let rt = get_runtime()?;
        let epsilon = rt.config.epsilon;
        if rhs.abs() <= epsilon {
            return Ok(DA::zero());
        }
        let mut coeffs = T::pool_alloc(rt.num_monomials);
        for &i in &self.nonzero {
            coeffs[i as usize] = self.coeffs[i as usize] * rhs;
        }
        Ok(DA {
            coeffs,
            nonzero: self.nonzero.clone(),
        })
    }
}

// ============================================================================
// Division: order-by-order, pool-allocated
// ============================================================================

impl<T: DACoefficient> Div<&DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;

    fn div(self, rhs: &DA<T>) -> Self::Output {
        let g0 = rhs.coeffs[0];
        if g0.abs() < 1e-15 {
            return Err(anyhow::anyhow!("Division by zero"))
                .with_context(|| format!("...while dividing with {rhs:#?}"));
        }
        let rt = get_runtime()?;
        let n = rt.num_monomials;
        let max_order = rt.config.max_order;
        let epsilon = rt.config.epsilon;
        let g0_inv = T::one() / g0;

        let mut g_entries: Vec<(u32, T)> = rhs
            .nonzero
            .iter()
            .filter(|&&i| i != 0)
            .map(|&i| (i, rhs.coeffs[i as usize]))
            .collect();
        g_entries.sort_unstable_by_key(|&(i, _)| i);

        let mut result = T::pool_alloc(n);
        let mut nonzero = Vec::new();

        for m_idx in 0..n {
            if rt.monomial_orders[m_idx] as u32 > max_order {
                break;
            }
            let f_m = self.coeffs[m_idx];
            let mut sum = T::zero();
            let mono_m = &rt.monomial_list[m_idx];

            for &(g_idx, g_coeff) in &g_entries {
                let mono_g = &rt.monomial_list[g_idx as usize];
                let mut diff = [0u8; MAX_VARS];
                let mut valid = true;
                for v in 0..MAX_VARS {
                    if mono_m.exponents[v] >= mono_g.exponents[v] {
                        diff[v] = mono_m.exponents[v] - mono_g.exponents[v];
                    } else {
                        valid = false;
                        break;
                    }
                }
                if valid {
                    let diff_mono = Monomial::new(diff);
                    if let Some(&diff_idx) = rt.monomial_index.get(&diff_mono) {
                        sum = g_coeff.mul_add(result[diff_idx as usize], sum);
                    }
                }
            }

            let q_m = (f_m - sum) * g0_inv;
            if q_m.abs() > epsilon {
                result[m_idx] = q_m;
                nonzero.push(m_idx as u32);
            }
        }

        Ok(DA {
            coeffs: result,
            nonzero,
        })
    }
}

impl<T: DACoefficient> Div<DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn div(self, rhs: DA<T>) -> Self::Output {
        &self / &rhs
    }
}
impl<T: DACoefficient> Div<DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;
    fn div(self, rhs: DA<T>) -> Self::Output {
        self / &rhs
    }
}
impl<T: DACoefficient> Div<&DA<T>> for DA<T> {
    type Output = Result<DA<T>>;
    fn div(self, rhs: &DA<T>) -> Self::Output {
        &self / rhs
    }
}

// Scalar division: O(K), pool-allocated
impl<T: DACoefficient> Div<T> for DA<T> {
    type Output = Result<DA<T>>;
    fn div(self, rhs: T) -> Self::Output {
        &self / rhs
    }
}
impl<T: DACoefficient> Div<T> for &DA<T> {
    type Output = Result<DA<T>>;
    fn div(self, rhs: T) -> Self::Output {
        if rhs.abs() < 1e-15 {
            anyhow::bail!("Division by zero");
        }
        let n = self.coeffs.len();
        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            coeffs[i as usize] = self.coeffs[i as usize] / rhs;
        }
        Ok(DA {
            coeffs,
            nonzero: self.nonzero.clone(),
        })
    }
}

// ============================================================================
// Display & Debug
// ============================================================================

impl<T: DACoefficient> fmt::Debug for DA<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rt = get_runtime().map_err(|_| fmt::Error)?;
        write!(f, "DA[")?;
        let mut entries: Vec<_> = self
            .nonzero
            .iter()
            .map(|&i| (i, self.coeffs[i as usize]))
            .collect();
        entries.sort_by_key(|&(i, _)| i);
        for (idx, (i, coeff)) in entries.iter().enumerate() {
            if idx > 0 {
                write!(f, " + ")?;
            }
            write!(f, "{}*{}", coeff, rt.monomial_list[*i as usize])?;
        }
        if entries.is_empty() {
            write!(f, "0")?;
        }
        write!(f, "]")
    }
}

impl<T: DACoefficient> fmt::Display for DA<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.nonzero.is_empty() {
            return write!(f, "0");
        }
        let rt = get_runtime().map_err(|_| fmt::Error)?;
        let mut entries: Vec<_> = self
            .nonzero
            .iter()
            .map(|&i| (i, self.coeffs[i as usize]))
            .collect();
        entries.sort_by_key(|&(i, _)| i);
        for (idx, &(i, coeff)) in entries.iter().enumerate() {
            if idx > 0 {
                write!(f, " + ")?;
            }
            let mono = &rt.monomial_list[i as usize];
            write!(f, "{}", coeff)?;
            if mono.total_order > 0 {
                write!(f, "*{}", mono)?;
            }
        }
        Ok(())
    }
}

// ============================================================================
// Convenience constructors
// ============================================================================

impl DA<f64> {
    pub fn constant(value: f64) -> Self {
        Self::from_coeff(value)
    }
}

impl DA<Complex64> {
    pub fn constant(value: f64) -> Self {
        Self::from_coeff(Complex64::new(value, 0.0))
    }
    pub fn complex_constant(value: Complex64) -> Self {
        Self::from_coeff(value)
    }

    pub fn from_da(da: &DA<f64>) -> Self {
        let n = da.coeffs.len();
        let mut coeffs = Complex64::pool_alloc(n);
        for &i in &da.nonzero {
            coeffs[i as usize] = Complex64::new(da.coeffs[i as usize], 0.0);
        }
        Self {
            coeffs,
            nonzero: da.nonzero.clone(),
        }
    }

    pub fn from_da_parts(real: &DA<f64>, imag: &DA<f64>) -> Self {
        let rt = get_runtime().expect("Taylor system not initialized");
        let n = rt.num_monomials;
        let epsilon = rt.config.epsilon;
        let mut coeffs = Complex64::pool_alloc(n);

        let words = (n + 63) / 64;
        let mut nz_set = vec![0u64; words];

        for &i in &real.nonzero {
            coeffs[i as usize].re = real.coeffs[i as usize];
            nz_set[i as usize / 64] |= 1u64 << (i as usize % 64);
        }
        for &i in &imag.nonzero {
            coeffs[i as usize].im = imag.coeffs[i as usize];
            nz_set[i as usize / 64] |= 1u64 << (i as usize % 64);
        }

        let mut nonzero = Vec::new();
        for word_idx in 0..words {
            let mut word = nz_set[word_idx];
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let idx = word_idx * 64 + bit;
                if idx < n && coeffs[idx].abs() > epsilon {
                    nonzero.push(idx as u32);
                }
                word &= word - 1;
            }
        }
        Self { coeffs, nonzero }
    }

    pub fn real_part(&self) -> DA<f64> {
        let rt = get_runtime().expect("Taylor system not initialized");
        let epsilon = rt.config.epsilon;
        let mut coeffs = f64::pool_alloc(rt.num_monomials);
        let mut nonzero = Vec::new();
        for &i in &self.nonzero {
            let re = self.coeffs[i as usize].re;
            if re.abs() > epsilon {
                coeffs[i as usize] = re;
                nonzero.push(i);
            }
        }
        DA { coeffs, nonzero }
    }

    pub fn imag_part(&self) -> DA<f64> {
        let rt = get_runtime().expect("Taylor system not initialized");
        let epsilon = rt.config.epsilon;
        let mut coeffs = f64::pool_alloc(rt.num_monomials);
        let mut nonzero = Vec::new();
        for &i in &self.nonzero {
            let im = self.coeffs[i as usize].im;
            if im.abs() > epsilon {
                coeffs[i as usize] = im;
                nonzero.push(i);
            }
        }
        DA { coeffs, nonzero }
    }
}
