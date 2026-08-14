//! Global configuration and precomputed runtime tables for Taylor series computations.
//!
//! `init_taylor()` builds the monomial ordering, index mappings, and (when feasible)
//! the N×N multiplication index table. All tables are immutable after init; only
//! `epsilon` and `max_order` can be changed at runtime via `set_epsilon()` /
//! `set_truncation_order()`.

use std::sync::RwLock;
use anyhow::{Result, Context, bail};
use rustc_hash::FxHashMap;

use super::{DEFAULT_EPSILON, MAX_VARS, Monomial, monomial::enumerate_monomials};

/// Scalar configuration for Taylor series computations.
#[derive(Debug, Clone, Copy)]
pub struct TaylorConfig {
    /// Maximum order of polynomials (can be changed by DANOT)
    pub max_order: u32,
    /// Number of variables
    pub num_vars: usize,
    /// Epsilon threshold for coefficient truncation (can be changed by DAEPS)
    pub epsilon: f64,
}

impl TaylorConfig {
    pub fn new(max_order: u32, num_vars: usize, epsilon: f64) -> Result<Self> {
        if num_vars > MAX_VARS {
            bail!(
                "Number of variables ({}) exceeds maximum ({})",
                num_vars, MAX_VARS
            );
        }
        Ok(Self { max_order, num_vars, epsilon })
    }
}

/// Sentinel value in `mult_table` indicating the product exceeds `init_order`.
pub const MULT_INVALID: u32 = u32::MAX;

/// Maximum memory (bytes) for the multiplication index table.
/// Above this threshold, DA multiply falls back to on-the-fly index computation.
const MAX_MULT_TABLE_BYTES: usize = 256 * 1024 * 1024;

/// Sentinel value for invalid derivative/integral targets (target monomial doesn't exist or exceeds order).
pub const DERIV_INVALID: u32 = u32::MAX;

/// Precomputed runtime tables for fast DA operations.
///
/// Built once at `init_taylor()` time. The multiplication table enables
/// O(1) product-index lookup instead of runtime exponent addition + hash.
pub struct TaylorRuntime {
    /// Mutable configuration (epsilon and max_order can change via DAEPS/DANOT)
    pub config: TaylorConfig,
    /// The order used when building the mult_table
    pub init_order: u32,
    /// Total number of monomials: C(max_order + num_vars, num_vars)
    pub num_monomials: usize,
    /// Index → Monomial mapping (graded lexicographic order)
    pub monomial_list: Vec<Monomial>,
    /// Monomial → Index mapping (for set_coeff, get_coeff, division)
    pub monomial_index: FxHashMap<Monomial, u32>,
    /// Index → total_order (fast lookup for DANOT truncation checks)
    pub monomial_orders: Vec<u8>,
    /// Flat-index of each variable's monomial: `variable_indices[v]` = index of x\_{v+1}
    pub variable_indices: [u32; MAX_VARS],
    /// Multiplication index table: `mult_table[i * num_monomials + j]` = index of
    /// monomial_i × monomial_j, or `MULT_INVALID` if the product exceeds init_order.
    /// `None` when num_monomials is too large for a table (falls back to on-the-fly).
    pub mult_table: Option<Vec<u32>>,
    /// Precomputed derivative target indices: `deriv_target[v * num_monomials + k]` =
    /// flat index of the monomial obtained by decrementing exponent `v` of monomial `k`,
    /// or `DERIV_INVALID` if exponent `v` is 0.
    pub deriv_target: Vec<u32>,
    /// Precomputed derivative exponents: `deriv_exponent[v * num_monomials + k]` =
    /// the exponent of variable `v` in monomial `k` (the multiplier for differentiation).
    pub deriv_exponent: Vec<u8>,
    /// Precomputed integral target indices: `integ_target[v * num_monomials + k]` =
    /// flat index of the monomial obtained by incrementing exponent `v` of monomial `k`,
    /// or `DERIV_INVALID` if the result would exceed init_order.
    pub integ_target: Vec<u32>,
}

/// Read guard wrapper that dereferences directly to `TaylorRuntime`.
pub struct RuntimeRef(std::sync::RwLockReadGuard<'static, Option<TaylorRuntime>>);

impl std::ops::Deref for RuntimeRef {
    type Target = TaylorRuntime;
    #[inline]
    fn deref(&self) -> &TaylorRuntime {
        // SAFETY: get_runtime() checks is_some() before constructing RuntimeRef
        unsafe { self.0.as_ref().unwrap_unchecked() }
    }
}

static TAYLOR_RUNTIME: RwLock<Option<TaylorRuntime>> = RwLock::new(None);

/// Weight vector set by DANOTW. Each element is the "cost" of one power of the
/// corresponding variable. `None` means unweighted (all weights = 1).
/// Consumed and cleared by the next `init_taylor()` call.
static WEIGHT_VECTOR: RwLock<Option<Vec<u32>>> = RwLock::new(None);

/// Set the per-variable weight vector (called by DANOTW transpiled code).
/// Weights must all be ≥ 1.
pub fn set_weight_vector(weights: Vec<u32>) -> Result<()> {
    for (i, &w) in weights.iter().enumerate() {
        if w < 1 {
            anyhow::bail!("DANOTW: weight for variable {} is {} — weights must be positive integers ≥ 1", i + 1, w);
        }
    }
    let mut guard = WEIGHT_VECTOR.write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire weight vector lock: {}", e))?;
    *guard = Some(weights);
    Ok(())
}

/// Take and clear the weight vector (consumed by init_taylor).
fn take_weight_vector() -> Result<Option<Vec<u32>>> {
    let mut guard = WEIGHT_VECTOR.write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire weight vector lock: {}", e))?;
    Ok(guard.take())
}

/// Filter template set by DAFSET. Each element corresponds to one component of the DA array.
/// A monomial is kept by DAFILT only if the corresponding coefficient in this template is nonzero.
/// `None` means filtering is disabled.
pub static FILTER_DA: RwLock<Option<Vec<super::da::DA<f64>>>> = RwLock::new(None);

/// Set the DAFILT template (DAFSET). Pass `None` to disable filtering.
pub fn set_filter_da(template: Option<Vec<super::da::DA<f64>>>) -> Result<()> {
    let mut guard = FILTER_DA.write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire filter lock: {}", e))?;
    *guard = template;
    Ok(())
}

/// Get a snapshot of the current filter template (cloned).
pub fn get_filter_da() -> Result<Option<Vec<super::da::DA<f64>>>> {
    let guard = FILTER_DA.read()
        .map_err(|e| anyhow::anyhow!("Failed to acquire filter lock: {}", e))?;
    Ok(guard.clone())
}

/// Initialize the Taylor series system and precompute runtime tables.
///
/// Must be called before any DA/CD operations. Builds:
/// - Monomial enumeration in graded lexicographic order
/// - Monomial → flat-index mapping
/// - N×N multiplication index table (when memory permits)
///
/// # Arguments
/// * `max_order` - Maximum order of Taylor expansions
/// * `num_vars` - Number of variables (≤ MAX_VARS)
pub fn init_taylor(max_order: u32, num_vars: usize) -> Result<usize> {
    let mut guard = TAYLOR_RUNTIME.write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire runtime lock: {}", e))?;

    if guard.is_some() {
        bail!("Taylor system already initialized. Call cleanup_taylor() first.");
    }

    let config = TaylorConfig::new(max_order, num_vars, DEFAULT_EPSILON)?;

    // Consume the weight vector (set by DANOTW, if any)
    let weights = take_weight_vector()?;
    // Trim to num_vars (extra weights ignored per spec)
    let weights: Option<Vec<u32>> = weights.map(|w| w.into_iter().take(num_vars).collect());

    // Build monomial list in graded lexicographic order
    let monomial_list = enumerate_monomials(max_order, num_vars as u32, weights.as_deref());
    let num_monomials = monomial_list.len();

    // Build reverse index: Monomial → flat index
    let mut monomial_index = FxHashMap::with_capacity_and_hasher(num_monomials, Default::default());
    for (i, mono) in monomial_list.iter().enumerate() {
        monomial_index.insert(*mono, i as u32);
    }

    // Build order lookup table
    let monomial_orders: Vec<u8> = monomial_list.iter().map(|m| m.total_order).collect();

    // Build variable index lookup
    // With weights, variable v has internal exponent w_v (not 1)
    let mut variable_indices = [0u32; MAX_VARS];
    for v in 0..num_vars {
        let w = weights.as_ref().map_or(1u8, |ws| ws[v] as u8);
        let mut exponents = [0u8; MAX_VARS];
        exponents[v] = w;
        let mono = Monomial::new(exponents);
        variable_indices[v] = *monomial_index.get(&mono)
            .unwrap_or_else(|| panic!(
                "BUG: variable monomial x{} (weight={}) missing from enumeration (order={}, nvars={})",
                v + 1, w, max_order, num_vars
            ));
    }

    // Build multiplication table (if it fits in memory)
    let table_bytes = (num_monomials as u128) * (num_monomials as u128) * 4;
    let mult_table = if table_bytes <= MAX_MULT_TABLE_BYTES as u128 {
        let n = num_monomials;
        let mut table = vec![MULT_INVALID; n * n];
        for i in 0..n {
            let row = i * n;
            for j in 0..n {
                let product = monomial_list[i].multiply(&monomial_list[j]);
                if product.within_order(max_order) {
                    if let Some(&idx) = monomial_index.get(&product) {
                        table[row + j] = idx;
                    }
                }
            }
        }
        Some(table)
    } else {
        None
    };

    // Build precomputed derivative/integral index tables (#19 + #21).
    // For each variable v and monomial index k, precompute:
    //   deriv_target[v*N+k]  = index of monomial with exponents[v] decremented by 1
    //   deriv_exponent[v*N+k] = exponents[v] (the derivative multiplier)
    //   integ_target[v*N+k]  = index of monomial with exponents[v] incremented by 1
    let table_size = num_vars * num_monomials;
    let mut deriv_target = vec![DERIV_INVALID; table_size];
    let mut deriv_exponent = vec![0u8; table_size];
    let mut integ_target = vec![DERIV_INVALID; table_size];

    for v in 0..num_vars {
        let base = v * num_monomials;
        for k in 0..num_monomials {
            let mono = &monomial_list[k];
            let exp_v = mono.exponents[v];
            deriv_exponent[base + k] = exp_v;

            // Derivative target: decrement exponent v
            if exp_v > 0 {
                let mut new_exp = mono.exponents;
                new_exp[v] -= 1;
                let new_mono = Monomial::new(new_exp);
                if let Some(&idx) = monomial_index.get(&new_mono) {
                    deriv_target[base + k] = idx;
                }
            }

            // Integral target: increment exponent v
            let mut new_exp = mono.exponents;
            new_exp[v] += 1;
            let new_mono = Monomial::new(new_exp);
            if new_mono.within_order(max_order) {
                if let Some(&idx) = monomial_index.get(&new_mono) {
                    integ_target[base + k] = idx;
                }
            }
        }
    }

    *guard = Some(TaylorRuntime {
        config,
        init_order: max_order,
        num_monomials,
        monomial_list,
        monomial_index,
        monomial_orders,
        variable_indices,
        mult_table,
        deriv_target,
        deriv_exponent,
        integ_target,
    });

    Ok(num_monomials)
}

/// Print the DA addressing arrays (monomial index → exponents mapping).
///
/// This is the Rosy equivalent of COSY's DAINI debug dump (3rd argument nonzero).
/// For each monomial index, prints the exponents in a tabular format.
pub fn dump_addressing_arrays() -> Result<()> {
    let rt = get_runtime().context("Cannot dump addressing arrays: DA not initialized")?;
    eprintln!("    ARRAY SETUP DONE, BEGIN PRINTING");
    eprintln!("    {:>6}  {:>6}  EXPONENTS", "INDEX", "ORDER");
    for (i, mono) in rt.monomial_list.iter().enumerate() {
        let exps: Vec<String> = mono.exponents[..rt.config.num_vars]
            .iter()
            .map(|e| format!("{}", e))
            .collect();
        eprintln!("    {:>6}  {:>6}  {}", i + 1, mono.total_order, exps.join(" "));
    }
    Ok(())
}

/// Get a read-locked reference to the runtime tables.
///
/// The returned `RuntimeRef` holds a read lock for its lifetime.
/// In single-threaded Rosy programs, this has no contention.
#[inline]
pub fn get_runtime() -> Result<RuntimeRef> {
    let guard = TAYLOR_RUNTIME.read()
        .map_err(|e| anyhow::anyhow!("Failed to acquire runtime lock: {}", e))?;
    if guard.is_none() {
        bail!("Taylor system not initialized. Call init_taylor() first.");
    }
    Ok(RuntimeRef(guard))
}

/// Get the current Taylor configuration (convenience wrapper).
pub fn get_config() -> Result<TaylorConfig> {
    let rt = get_runtime()?;
    Ok(rt.config)
}

/// Set the epsilon value for coefficient truncation.
///
/// # Returns
/// The previous epsilon value
pub fn set_epsilon(epsilon: f64) -> Result<f64> {
    let mut guard = TAYLOR_RUNTIME.write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire runtime lock: {}", e))?;
    let rt = guard.as_mut()
        .ok_or_else(|| anyhow::anyhow!("Taylor system not initialized"))?;
    let old = rt.config.epsilon;
    rt.config.epsilon = epsilon;
    Ok(old)
}

/// Set the momentary truncation order for DA/CD computations (DANOT).
///
/// Cannot exceed the order used at initialization.
///
/// # Returns
/// The previous truncation order
pub fn set_truncation_order(order: u32) -> Result<u32> {
    let mut guard = TAYLOR_RUNTIME.write()
        .map_err(|e| anyhow::anyhow!("Failed to acquire runtime lock: {}", e))?;
    let rt = guard.as_mut()
        .ok_or_else(|| anyhow::anyhow!("Taylor system not initialized"))?;
    if order > rt.init_order {
        bail!(
            "Cannot set truncation order ({}) above init order ({})",
            order, rt.init_order
        );
    }
    let old = rt.config.max_order;
    rt.config.max_order = order;
    Ok(old)
}

/// Check if Taylor system is initialized.
pub fn is_initialized() -> bool {
    TAYLOR_RUNTIME.read()
        .map(|g| g.is_some())
        .unwrap_or(false)
}

/// Clean up the Taylor system (for re-initialization).
pub fn cleanup_taylor() {
    if let Ok(mut guard) = TAYLOR_RUNTIME.write() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_config_creation() {
        let cfg = TaylorConfig::new(10, 3, 1e-15).unwrap();
        assert_eq!(cfg.max_order, 10);
        assert_eq!(cfg.num_vars, 3);
        assert_eq!(cfg.epsilon, 1e-15);
    }

    #[test]
    fn test_config_too_many_vars() {
        let result = TaylorConfig::new(10, super::super::MAX_VARS + 1, 1e-15);
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_global_init() {
        cleanup_taylor();

        assert!(!is_initialized());

        init_taylor(5, 2).unwrap();
        assert!(is_initialized());

        let cfg = get_config().unwrap();
        assert_eq!(cfg.max_order, 5);
        assert_eq!(cfg.num_vars, 2);

        // Check runtime tables
        let rt = get_runtime().unwrap();
        // C(5+2, 2) = 21 monomials for order 5, 2 vars
        assert_eq!(rt.num_monomials, 21);
        assert_eq!(rt.monomial_list.len(), 21);
        assert!(rt.mult_table.is_some());
        let table = rt.mult_table.as_ref().unwrap();
        assert_eq!(table.len(), 21 * 21);

        // Constant monomial is at index 0
        assert_eq!(rt.monomial_list[0], Monomial::constant());
        // Variable indices are populated
        assert!(rt.variable_indices[0] > 0); // x1 is not the constant
        assert!(rt.variable_indices[1] > 0); // x2 is not the constant

        drop(rt);
        cleanup_taylor();
        assert!(!is_initialized());
    }

    #[test]
    #[serial]
    fn test_mult_table_correctness() {
        cleanup_taylor();
        init_taylor(3, 2).unwrap();

        let rt = get_runtime().unwrap();
        let table = rt.mult_table.as_ref().unwrap();
        let n = rt.num_monomials;

        // Verify: for every valid (i,j), table[i*n+j] gives the correct product index
        for i in 0..n {
            for j in 0..n {
                let product = rt.monomial_list[i].multiply(&rt.monomial_list[j]);
                let k = table[i * n + j];
                if product.within_order(3) {
                    assert_ne!(k, MULT_INVALID, "Product of monomials {} and {} should be valid", i, j);
                    assert_eq!(rt.monomial_list[k as usize], product);
                } else {
                    assert_eq!(k, MULT_INVALID, "Product of monomials {} and {} should be INVALID", i, j);
                }
            }
        }

        drop(rt);
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn test_danot_cannot_exceed_init_order() {
        cleanup_taylor();
        init_taylor(5, 2).unwrap();

        // Can reduce
        let old = set_truncation_order(3).unwrap();
        assert_eq!(old, 5);

        // Can restore
        let old = set_truncation_order(5).unwrap();
        assert_eq!(old, 3);

        // Cannot exceed
        let result = set_truncation_order(6);
        assert!(result.is_err());

        cleanup_taylor();
    }
}
