use crate::taylor::config::DERIV_INVALID;
use crate::taylor::{DACoefficient, get_runtime};
use crate::{CD, DA};

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
    da: &crate::taylor::da::DA<T>,
    var_idx: usize,
) -> anyhow::Result<crate::taylor::da::DA<T>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let epsilon = rt.config.epsilon;
    let max_order = rt.config.max_order;
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
        // Stored terms above DANOT still count when the derivative itself
        // is inside the order. x^3 at DANOT 2 becomes 3 x^2.
        if (rt.monomial_orders[target as usize] as u32) > max_order {
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

    Ok(crate::taylor::da::DA { coeffs, nonzero })
}

/// Generic anti-derivative (integral) using precomputed index tables (issues #19 + #21).
fn da_antiderivative<T: DACoefficient>(
    da: &crate::taylor::da::DA<T>,
    var_idx: usize,
) -> anyhow::Result<crate::taylor::da::DA<T>> {
    let rt = get_runtime()?;
    let n = rt.num_monomials;
    let epsilon = rt.config.epsilon;
    let max_order = rt.config.max_order;
    let base = var_idx * n;

    let mut coeffs = T::pool_alloc(n);
    let mut nonzero = Vec::new();

    for &idx in &da.nonzero {
        let i = idx as usize;
        if (rt.monomial_orders[i] as u32) > max_order {
            continue;
        }
        let target = rt.integ_target[base + i];
        if target == DERIV_INVALID {
            continue;
        }
        if (rt.monomial_orders[target as usize] as u32) > max_order {
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

    Ok(crate::taylor::da::DA { coeffs, nonzero })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taylor::{Monomial, cleanup_taylor, init_taylor, set_truncation_order};
    use serial_test::serial;

    #[test]
    #[serial]
    fn der_ignores_terms_above_current_order() -> anyhow::Result<()> {
        cleanup_taylor();
        init_taylor(3, 1)?;
        let x = DA::variable(1)?;
        let x3 = (&(&x * &x)? * &x)?;
        set_truncation_order(2)?;
        let d = x3.rosy_derive(1)?;
        let mut exps = [0u8; crate::taylor::MAX_VARS];
        exps[0] = 2;
        let x2 = Monomial::new(exps);
        assert!(
            (d.get_coeff(&x2) - 3.0).abs() < 1e-12,
            "x^3 at DANOT 2 still differentiates to 3 x^2, got {}",
            d.get_coeff(&x2)
        );
        cleanup_taylor();
        Ok(())
    }

    #[test]
    #[serial]
    fn der_of_stored_square_survives_danot_1() -> anyhow::Result<()> {
        cleanup_taylor();
        init_taylor(3, 1)?;
        let x = DA::variable(1)?;
        let x2 = (&x * &x)?;
        set_truncation_order(1)?;
        let d = x2.rosy_derive(1)?;
        let x1 = Monomial::variable(0);
        assert!(
            (d.get_coeff(&x1) - 2.0).abs() < 1e-12,
            "stored x^2 at DANOT 1 differentiates to 2x, got {}",
            d.get_coeff(&x1)
        );
        cleanup_taylor();
        Ok(())
    }

    #[test]
    #[serial]
    fn der_of_square_in_five_variables() -> anyhow::Result<()> {
        cleanup_taylor();
        init_taylor(3, 5)?;
        let x = DA::variable(1)?;
        let x2 = (&x * &x)?;
        set_truncation_order(1)?;
        let d = x2.rosy_derive(1)?;
        let x1 = Monomial::variable(0);
        assert!(
            (d.get_coeff(&x1) - 2.0).abs() < 1e-12,
            "5-var x^2 at DANOT 1, got {}",
            d.get_coeff(&x1)
        );
        cleanup_taylor();
        Ok(())
    }

    #[test]
    #[serial]
    fn der_through_rosy_value() -> anyhow::Result<()> {
        cleanup_taylor();
        init_taylor(3, 5)?;
        let x = DA::variable(1)?;
        let x2 = (&x * &x)?;
        let wrapped = crate::RosyValue::DA(x2);
        set_truncation_order(1)?;
        let out = crate::rosy_dyn_binary(
            crate::BinaryOp::Derive,
            &wrapped,
            &crate::RosyValue::RE(1.0),
        )?;
        let d = out.expect_da()?;
        let x1 = Monomial::variable(0);
        assert!(
            (d.get_coeff(&x1) - 2.0).abs() < 1e-12,
            "RosyValue path got {}",
            d.get_coeff(&x1)
        );
        cleanup_taylor();
        Ok(())
    }

    #[test]
    #[serial]
    fn integ_does_not_write_above_current_order() -> anyhow::Result<()> {
        cleanup_taylor();
        init_taylor(3, 1)?;
        let x = DA::variable(1)?;
        let x2 = (&x * &x)?;
        set_truncation_order(2)?;
        let p = x2.rosy_derive(-1)?;
        let orders = {
            let rt = crate::taylor::get_runtime()?;
            rt.monomial_orders.clone()
        };
        for &i in &p.nonzero {
            assert!(
                (orders[i as usize] as u32) <= 2,
                "integral leaked order {}",
                orders[i as usize]
            );
        }
        cleanup_taylor();
        Ok(())
    }
}
