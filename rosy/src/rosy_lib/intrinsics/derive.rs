use crate::rosy_lib::taylor::config::DERIV_INVALID;
use crate::rosy_lib::taylor::{DACoefficient, get_runtime};
use crate::rosy_lib::{CD, DA};

/// Trait for derivation/anti-derivation of DA types.
/// Positive index = partial derivative, negative index = anti-derivative (integral).
pub trait RosyDerive {
    type Output;
    fn rosy_derive(&self, var_index: i64) -> anyhow::Result<Self::Output>;
}

/// Generic derivative using precomputed index tables (issues #19 + #21).
///
/// Zero allocations beyond the pool-allocated output array. Single linear scan
/// over nonzero entries with O(1) index lookups via `deriv_target`/`deriv_exponent`.
fn da_derivative<T: DACoefficient>(
    da: &crate::rosy_lib::taylor::da::DA<T>,
    var_idx: usize,
) -> anyhow::Result<crate::rosy_lib::taylor::da::DA<T>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let epsilon = rt.config.epsilon;
    let base = var_idx * n;

    let mut coeffs = T::pool_alloc(n);
    let mut nonzero = Vec::new();

    for &idx in &da.nonzero {
        let i = idx as usize;
        let exp_v = rt.deriv_exponent[base + i];
        if exp_v == 0 {
            continue;
        }

        let target = rt.deriv_target[base + i];
        if target == DERIV_INVALID {
            continue;
        }

        let new_coeff = da.coeffs[i] * T::from_usize(exp_v as usize);
        if new_coeff.abs() > epsilon {
            let tu = target as usize;
            coeffs[tu] = coeffs[tu] + new_coeff;
            nonzero.push(target);
        }
    }

    // Deduplicate nonzero list and filter small coefficients
    nonzero.sort_unstable();
    nonzero.dedup();
    nonzero.retain(|&k| {
        if coeffs[k as usize].abs() > epsilon {
            true
        } else {
            coeffs[k as usize] = T::zero();
            false
        }
    });

    Ok(crate::rosy_lib::taylor::da::DA { coeffs, nonzero })
}

/// Generic anti-derivative (integral) using precomputed index tables (issues #19 + #21).
fn da_antiderivative<T: DACoefficient>(
    da: &crate::rosy_lib::taylor::da::DA<T>,
    var_idx: usize,
) -> anyhow::Result<crate::rosy_lib::taylor::da::DA<T>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let epsilon = rt.config.epsilon;
    let base = var_idx * n;

    let mut coeffs = T::pool_alloc(n);
    let mut nonzero = Vec::new();

    for &idx in &da.nonzero {
        let i = idx as usize;
        let target = rt.integ_target[base + i];
        if target == DERIV_INVALID {
            continue;
        }

        let exp_v = rt.deriv_exponent[base + i];
        let new_exp = exp_v as usize + 1;
        let new_coeff = da.coeffs[i] / T::from_usize(new_exp);

        if new_coeff.abs() > epsilon {
            let tu = target as usize;
            coeffs[tu] = coeffs[tu] + new_coeff;
            nonzero.push(target);
        }
    }

    // Deduplicate nonzero list and filter small coefficients
    nonzero.sort_unstable();
    nonzero.dedup();
    nonzero.retain(|&k| {
        if coeffs[k as usize].abs() > epsilon {
            true
        } else {
            coeffs[k as usize] = T::zero();
            false
        }
    });

    Ok(crate::rosy_lib::taylor::da::DA { coeffs, nonzero })
}

impl RosyDerive for DA {
    type Output = DA;
    fn rosy_derive(&self, var_index: i64) -> anyhow::Result<Self::Output> {
        if var_index == 0 {
            anyhow::bail!("Derivation variable index cannot be 0");
        }

        if var_index > 0 {
            // Positive: partial derivative w.r.t. variable var_index
            let idx = (var_index as usize) - 1; // Convert to 0-based
            da_derivative(self, idx)
        } else {
            // Negative: anti-derivative (integral) w.r.t. variable |var_index|
            let idx = ((-var_index) as usize) - 1; // Convert to 0-based
            da_antiderivative(self, idx)
        }
    }
}

impl RosyDerive for CD {
    type Output = CD;
    fn rosy_derive(&self, var_index: i64) -> anyhow::Result<Self::Output> {
        if var_index == 0 {
            anyhow::bail!("Derivation variable index cannot be 0");
        }

        if var_index > 0 {
            let idx = (var_index as usize) - 1;
            da_derivative(self, idx)
        } else {
            let idx = ((-var_index) as usize) - 1;
            da_antiderivative(self, idx)
        }
    }
}
