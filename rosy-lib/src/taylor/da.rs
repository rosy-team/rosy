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

use super::config::{MULT_INVALID, TaylorRuntime, get_runtime};
use super::scratch::scratch_reserve;
use super::{MAX_VARS, Monomial};

/// Best-effort bump reserve for one operator result. Drop restores the cursor.
/// Misses fall through: named DA results still live in the coeff pool.
#[inline]
fn account_result_temp(n: usize) -> super::scratch::ScratchFrame {
    scratch_reserve(n)
}

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

thread_local! {
    static NZ_POOL: RefCell<Vec<Vec<u32>>> = const { RefCell::new(Vec::new()) };
}

/// An empty nonzero list with room for `cap` entries, reused from a
/// thread-local pool. Growing a fresh `Vec` by `push` cost a malloc and two
/// or three reallocs per DA operation; for PHAD's 9-term SIMO series that
/// was about a third of each add or multiply.
#[inline]
pub(crate) fn nz_alloc(cap: usize) -> Vec<u32> {
    let mut v = NZ_POOL
        .try_with(|pool| pool.borrow_mut().pop())
        .ok()
        .flatten()
        .unwrap_or_default();
    v.clear();
    if v.capacity() < cap {
        v.reserve(cap);
    }
    v
}

#[inline]
pub(crate) fn nz_return(v: Vec<u32>) {
    if v.capacity() == 0 {
        return;
    }
    let _ = NZ_POOL.try_with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < 4096 {
            pool.push(v);
        }
    });
}

/// Copy of a nonzero list in a pooled buffer.
#[inline]
pub(crate) fn nz_copy(src: &[u32]) -> Vec<u32> {
    let mut v = nz_alloc(src.len());
    v.extend_from_slice(src);
    v
}

/// Words a bitset keeps on the stack. 8 words cover 512 monomials, which is
/// every DA PHAD's SIMO integrator and FMM use.
const INLINE_BITSET_WORDS: usize = 8;

/// A zeroed bitset. Small ones live on the stack; a pooled `Vec` cost two
/// thread-local lookups and a clear per DA add, subtract, or multiply.
pub(crate) enum Bitset {
    Inline([u64; INLINE_BITSET_WORDS], usize),
    Heap(Vec<u64>),
}

impl std::ops::Deref for Bitset {
    type Target = [u64];
    #[inline]
    fn deref(&self) -> &[u64] {
        match self {
            Bitset::Inline(w, n) => &w[..*n],
            Bitset::Heap(v) => v,
        }
    }
}

impl std::ops::DerefMut for Bitset {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u64] {
        match self {
            Bitset::Inline(w, n) => &mut w[..*n],
            Bitset::Heap(v) => v,
        }
    }
}

/// Get a zeroed bitset: on the stack when small, else from the pool.
#[inline]
pub(crate) fn bitset_pool_alloc(words: usize) -> Bitset {
    if words <= INLINE_BITSET_WORDS {
        return Bitset::Inline([0; INLINE_BITSET_WORDS], words);
    }
    Bitset::Heap(BITSET_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        while let Some(v) = pool.pop() {
            if v.len() == words {
                return v;
            }
        }
        vec![0u64; words]
    }))
}

/// Return a pooled bitset after clearing it. Stack bitsets just drop.
#[inline]
pub(crate) fn bitset_pool_return(b: Bitset) {
    let Bitset::Heap(mut v) = b else {
        return;
    };
    if v.is_empty() {
        return;
    }
    for w in v.iter_mut() {
        *w = 0;
    }
    let _ = BITSET_POOL.try_with(|pool| pool.borrow_mut().push(v));
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
        fmadd(self, a, b)
    }
}

/// `x*y + z`. A hardware FMA when the target has one. Otherwise a separate
/// multiply and add: `f64::mul_add` without the `fma` target feature calls a
/// software routine, and that call dominated DA products on generic x86-64.
#[inline(always)]
pub fn fmadd(x: f64, y: f64, z: f64) -> f64 {
    #[cfg(any(target_feature = "fma", target_arch = "aarch64"))]
    {
        f64::mul_add(x, y, z)
    }
    #[cfg(not(any(target_feature = "fma", target_arch = "aarch64")))]
    {
        x * y + z
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
        // Copy every stored term. DANOT truncates results of operations,
        // it does not erase coefficients already sitting in a value.
        // COSY still differentiates a stored x^2 after DANOT 1.
        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            coeffs[i as usize] = self.coeffs[i as usize];
        }
        Self {
            coeffs,
            nonzero: nz_copy(&self.nonzero),
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
        nz_return(std::mem::take(&mut self.nonzero));
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
        let mut nonzero = nz_alloc(1);
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
        self.nonzero.retain(|&i| {
            if self.coeffs[i as usize].abs() > epsilon {
                true
            } else {
                self.coeffs[i as usize] = T::zero();
                false
            }
        });
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
        let max_order = rt.config.max_order;
        for (mono, coeff) in hash_coeffs {
            if mono.total_order as u32 > max_order {
                continue;
            }
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
        let epsilon = get_runtime().map(|rt| rt.config.epsilon).unwrap_or(1e-15);
        let was_nz = self.coeffs[0].abs() > epsilon;
        self.coeffs[0] = self.coeffs[0] + value;
        let is_nz = self.coeffs[0].abs() > epsilon;

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
        let epsilon = get_runtime().map(|rt| rt.config.epsilon).unwrap_or(1e-15);
        if prime.coeffs[0].abs() > epsilon {
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

        let _scratch = account_result_temp(n);
        let words = (n + 63) / 64;
        let mut self_set = bitset_pool_alloc(words);

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

        let mut nonzero = nz_alloc((self.nonzero.len() + rhs.nonzero.len()).min(n));
        let mut result_set = bitset_pool_alloc(words);
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

        bitset_pool_return(self_set);
        bitset_pool_return(result_set);
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
        let _scratch = account_result_temp(n);
        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            coeffs[i as usize] = -self.coeffs[i as usize];
        }
        DA {
            coeffs,
            nonzero: nz_copy(&self.nonzero),
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

        let _scratch = account_result_temp(n);
        let words = (n + 63) / 64;
        let mut self_set = bitset_pool_alloc(words);

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

        let mut nonzero = nz_alloc((self.nonzero.len() + rhs.nonzero.len()).min(n));
        let mut result_set = bitset_pool_alloc(words);
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

        bitset_pool_return(self_set);
        bitset_pool_return(result_set);
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
        let _scratch = account_result_temp(n);
        let mut result = T::pool_alloc(n);
        let words = (n + 63) / 64;
        let mut written = bitset_pool_alloc(words);
        let mut nonzero = Vec::new();
        multiply_truncated_into(
            &lhs.coeffs,
            &lhs.nonzero,
            &rhs.coeffs,
            &rhs.nonzero,
            trunc_order,
            rt,
            &mut result,
            &mut nonzero,
            &mut written,
        );
        bitset_pool_return(written);
        Ok(DA {
            coeffs: result,
            nonzero,
        })
    }
}

/// Scatter-multiply into `out`. `out` must be length `rt.num_monomials`.
/// Previous contents of `out` at `out_nz` are zeroed; `written` is cleared.
/// Does not check remaining scratch — caller reserved buffers already.
pub(crate) fn multiply_truncated_into<T: DACoefficient>(
    lhs_coeffs: &[T],
    lhs_nz: &[u32],
    rhs_coeffs: &[T],
    rhs_nz: &[u32],
    trunc_order: u32,
    rt: &TaylorRuntime,
    out: &mut [T],
    out_nz: &mut Vec<u32>,
    written: &mut [u64],
) {
    for &i in out_nz.iter() {
        let iu = i as usize;
        if iu < out.len() {
            out[iu] = T::zero();
        }
    }
    out_nz.clear();
    written.fill(0);

    let n = rt.num_monomials;
    let epsilon = rt.config.epsilon;
    let orders = &rt.monomial_orders;
    let trunc_order_u8 = trunc_order as u8;

    if let Some(table) = &rt.mult_table {
        for &i in lhs_nz {
            let ci = lhs_coeffs[i as usize];
            let oi = orders[i as usize];
            if oi > trunc_order_u8 {
                continue;
            }
            let max_b_order = trunc_order_u8 - oi;
            let row = i as usize * n;
            for &j in rhs_nz {
                if orders[j as usize] > max_b_order {
                    continue;
                }
                let k = table[row + j as usize];
                if k != MULT_INVALID {
                    let ku = k as usize;
                    out[ku] = ci.mul_add(rhs_coeffs[j as usize], out[ku]);
                    written[ku / 64] |= 1u64 << (ku % 64);
                }
            }
        }
    } else {
        for &i in lhs_nz {
            let ci = lhs_coeffs[i as usize];
            let oi = orders[i as usize];
            if oi > trunc_order_u8 {
                continue;
            }
            let max_b_order = trunc_order_u8 - oi;
            for &j in rhs_nz {
                if orders[j as usize] > max_b_order {
                    continue;
                }
                let product = rt.monomial_list[i as usize].multiply(&rt.monomial_list[j as usize]);
                if product.within_order(trunc_order) {
                    if let Some(&k) = rt.monomial_index.get(&product) {
                        let ku = k as usize;
                        out[ku] = ci.mul_add(rhs_coeffs[j as usize], out[ku]);
                        written[ku / 64] |= 1u64 << (ku % 64);
                    }
                }
            }
        }
    }

    let words = written.len();
    for word_idx in 0..words {
        let mut word = written[word_idx];
        written[word_idx] = 0;
        while word != 0 {
            let bit = word.trailing_zeros() as usize;
            let idx = word_idx * 64 + bit;
            if idx < n {
                if out[idx].abs() > epsilon {
                    out_nz.push(idx as u32);
                } else {
                    out[idx] = T::zero();
                }
            }
            word &= word - 1;
        }
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

        let _scratch = account_result_temp(n);
        let mut result = T::pool_alloc(n);

        // Dense operands: walk only the valid index triples. An order-6,
        // 3-variable product (PHAD's FMM) is 924 of them, not 84*84 lookups.
        let cut = (max_order as usize).min(rt.dense_prefix.len().saturating_sub(1));
        let dense_len = rt.dense_prefix.get(cut).copied().unwrap_or(0);
        if dense_len > 0
            && !rt.dense_ij.is_empty()
            && self.nonzero.len() * rhs.nonzero.len() > dense_len
        {
            let a = &self.coeffs;
            let b = &rhs.coeffs;
            let ij = &rt.dense_ij;
            let gk = &rt.dense_group_k;
            let ng = rt.dense_group_prefix[cut];
            let groups = &rt.dense_groups[..=ng];
            // One sum per `k`, in the same (i, j) order as before, so the
            // result is bit-identical. Four independent sums at a time keep
            // the FMA units busy instead of waiting on one chain.
            // Every i, j < n and every t < dense_len were built at DAINI, and
            // both operands hold n coefficients. Checked once here, not per term.
            assert!(a.len() >= n && b.len() >= n && ij.len() >= dense_len);
            assert!(groups.last().is_some_and(|&e| e as usize <= dense_len));
            let term = |t: usize, acc: T| {
                // SAFETY: t < dense_len <= ij.len(); i, j < n <= a.len(), b.len().
                unsafe {
                    let p = *ij.get_unchecked(t);
                    a.get_unchecked((p & 0xffff) as usize)
                        .mul_add(*b.get_unchecked((p >> 16) as usize), acc)
                }
            };
            let mut g = 0;
            while g + 4 <= ng {
                let s = [
                    groups[g] as usize,
                    groups[g + 1] as usize,
                    groups[g + 2] as usize,
                    groups[g + 3] as usize,
                ];
                let e = [s[1], s[2], s[3], groups[g + 4] as usize];
                let len = (0..4).map(|u| e[u] - s[u]).min().unwrap_or(0);
                let mut acc = [T::zero(); 4];
                for t in 0..len {
                    acc[0] = term(s[0] + t, acc[0]);
                    acc[1] = term(s[1] + t, acc[1]);
                    acc[2] = term(s[2] + t, acc[2]);
                    acc[3] = term(s[3] + t, acc[3]);
                }
                for u in 0..4 {
                    for t in s[u] + len..e[u] {
                        acc[u] = term(t, acc[u]);
                    }
                    result[gk[g + u] as usize] = acc[u];
                }
                g += 4;
            }
            while g < ng {
                let (s, e) = (groups[g] as usize, groups[g + 1] as usize);
                let mut acc = T::zero();
                for t in s..e {
                    acc = term(t, acc);
                }
                result[gk[g] as usize] = acc;
                g += 1;
            }
            let mut nonzero = nz_alloc(n);
            for (idx, c) in result.iter_mut().enumerate() {
                if c.abs() > epsilon {
                    nonzero.push(idx as u32);
                } else {
                    *c = T::zero();
                }
            }
            return Ok(DA {
                coeffs: result,
                nonzero,
            });
        }

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

        let mut nonzero = nz_alloc((self.nonzero.len() * rhs.nonzero.len()).min(n));
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
        let _scratch = account_result_temp(rt.num_monomials);
        let mut coeffs = T::pool_alloc(rt.num_monomials);
        let mut nonzero = nz_alloc(self.nonzero.len());
        for &i in &self.nonzero {
            let scaled = self.coeffs[i as usize] * rhs;
            if scaled.abs() > epsilon {
                coeffs[i as usize] = scaled;
                nonzero.push(i);
            }
        }
        Ok(DA { coeffs, nonzero })
    }
}

/// A divisor constant at or under `DAEPS` is zero. No runtime means exact zero.
fn da_divisor_vanishes<T: DACoefficient>(g0: T) -> bool {
    let eps = get_runtime().map(|rt| rt.config.epsilon).unwrap_or(0.0);
    !g0.abs().is_finite() || g0.abs() <= eps
}

// ============================================================================
// Division: order-by-order, pool-allocated
// ============================================================================

impl<T: DACoefficient> Div<&DA<T>> for &DA<T> {
    type Output = Result<DA<T>>;

    fn div(self, rhs: &DA<T>) -> Self::Output {
        let g0 = rhs.coeffs[0];
        // PHAD sets `DAEPS 10^(-100)`. A constant the algebra kept, such as
        // 9e-16, is a real divisor. Only a term at or under the cutoff is zero.
        if da_divisor_vanishes(g0) {
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

        let _scratch = account_result_temp(n);
        let mut result = T::pool_alloc(n);
        let mut nonzero = nz_alloc(n);

        if let Some(quot) = &rt.quot_table {
            // Same terms in the same order as the exponent search below.
            for m_idx in 0..n {
                if rt.monomial_orders[m_idx] as u32 > max_order {
                    break;
                }
                let mut sum = T::zero();
                for &(g_idx, g_coeff) in &g_entries {
                    let d = quot[g_idx as usize * n + m_idx];
                    if d != MULT_INVALID {
                        sum = g_coeff.mul_add(result[d as usize], sum);
                    }
                }
                let q_m = (self.coeffs[m_idx] - sum) * g0_inv;
                if q_m.abs() > epsilon {
                    result[m_idx] = q_m;
                    nonzero.push(m_idx as u32);
                }
            }
            return Ok(DA {
                coeffs: result,
                nonzero,
            });
        }

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
        if da_divisor_vanishes(rhs) {
            anyhow::bail!("Division by zero");
        }
        let n = self.coeffs.len();
        let _scratch = account_result_temp(n);
        let mut coeffs = T::pool_alloc(n);
        for &i in &self.nonzero {
            coeffs[i as usize] = self.coeffs[i as usize] / rhs;
        }
        Ok(DA {
            coeffs,
            nonzero: nz_copy(&self.nonzero),
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
            nonzero: nz_copy(&da.nonzero),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    use crate::taylor::config::{cleanup_taylor, get_runtime, init_taylor, set_epsilon};

    #[test]
    #[serial]
    fn scalar_multiply_prunes_coefficients_below_runtime_epsilon() {
        cleanup_taylor();
        init_taylor(2, 2).unwrap();
        set_epsilon(1e-16).unwrap();

        let x = DA::<f64>::variable(1).unwrap();
        let scaled = (&x * 8.0e-17).unwrap();

        assert!(scaled.is_zero());

        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn scalar_multiply_keeps_coefficients_above_runtime_epsilon() {
        cleanup_taylor();
        init_taylor(2, 2).unwrap();
        set_epsilon(1e-16).unwrap();

        let x = DA::<f64>::variable(1).unwrap();
        let scaled = (&x * 2.0e-16).unwrap();

        assert_eq!(scaled.num_terms(), 1);

        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn division_allows_a_constant_above_daeps() {
        cleanup_taylor();
        init_taylor(4, 1).unwrap();
        // PHAD uses `DAEPS 10^(-100)`. 9e-16 is a kept coefficient, not zero.
        set_epsilon(1e-100).unwrap();
        let tiny = DA::<f64>::from_coeff(9.44e-16);
        let quot = (&DA::<f64>::from_coeff(1.0) / &tiny).unwrap();
        let c0 = quot.coeffs[0];
        assert!((c0 - 1.0 / 9.44e-16).abs() / c0.abs() < 1e-12);
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn division_rejects_a_constant_at_or_under_daeps() {
        cleanup_taylor();
        init_taylor(2, 1).unwrap();
        set_epsilon(1e-15).unwrap();
        let tiny = DA::<f64>::from_coeff(9.44e-16);
        assert!((&DA::<f64>::from_coeff(1.0) / &tiny).is_err());
        assert!((&DA::<f64>::from_coeff(1.0) / &DA::<f64>::from_coeff(0.0)).is_err());
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn da_add_works_when_scrlen_too_small() {
        cleanup_taylor();
        let mut c = crate::DEFAULT_SCRLEN as f64;
        crate::rosy_scrlen(&mut c).unwrap();

        init_taylor(1, 1).unwrap();
        // n = 2 monomials; 1 word cannot hold a result temp — must heap-fallback.
        let mut tiny = 1.0;
        crate::rosy_scrlen(&mut tiny).unwrap();

        let x = DA::<f64>::variable(1).unwrap();
        let sum = (&x + &x).expect("DA add should heap-fallback when SCRLEN is too small");
        assert!(
            (sum.coeffs[get_runtime().unwrap().variable_indices[0] as usize] - 2.0).abs() < 1e-14
        );

        let mut q = -1.0;
        crate::rosy_scrlen(&mut q).unwrap();
        assert_eq!(q, 1.0);

        let mut restore = crate::DEFAULT_SCRLEN as f64;
        crate::rosy_scrlen(&mut restore).unwrap();
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn horner_scratch_ping_pong_matches_heap_horner() {
        cleanup_taylor();
        init_taylor(5, 2).unwrap();
        let x = DA::<f64>::variable(1).unwrap();
        let prime = x.make_prime();
        // exp series truncated: 1 + x + x^2/2 + x^3/6 + x^4/24 + x^5/120
        let xf = [1.0, 1.0, 0.5, 1.0 / 6.0, 1.0 / 24.0, 1.0 / 120.0];
        let ping = {
            let rt = get_runtime().unwrap();
            DA::<f64>::horner_eval_with_rt(&prime, &xf, &rt).unwrap()
        };

        // Heap path: successive truncated muls (same algebra as the old Horner).
        // Drop RuntimeRef before add_constant_in_place — std RwLock is not reentrant.
        let mut heap = DA::<f64>::from_coeff(xf[xf.len() - 1]);
        for i in (0..xf.len() - 1).rev() {
            let steps_from_end = (xf.len() - 1) - i;
            {
                let rt = get_runtime().unwrap();
                let trunc_order = (steps_from_end as u32).min(rt.config.max_order);
                heap = DA::multiply_truncated_with_rt(&heap, &prime, trunc_order, &rt).unwrap();
            }
            heap.add_constant_in_place(xf[i]);
        }
        assert_eq!(ping, heap);

        cleanup_taylor();
    }
}
