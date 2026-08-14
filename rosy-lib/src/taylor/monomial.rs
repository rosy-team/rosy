//! Monomial representation for multivariate polynomials.

use std::cmp::Ordering;
use std::fmt;

use super::MAX_VARS;

/// Represents a monomial as a product of variables with given exponents.
///
/// For example, x₁²x₂³ is represented as exponents = [2, 3, 0, 0, ...].
/// Uses graded lexicographic ordering for canonical form.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Monomial {
    /// Exponents for each variable (fixed-size array for cache efficiency)
    pub exponents: [u8; MAX_VARS],
    /// Cached total order (sum of exponents) for fast filtering
    pub total_order: u8,
}

impl Monomial {
    /// Create a new monomial from exponents.
    ///
    /// # Arguments
    /// * `exponents` - Array of exponents for each variable
    ///
    /// # Returns
    /// A new monomial with cached total order
    pub fn new(exponents: [u8; MAX_VARS]) -> Self {
        let total_order = exponents.iter().map(|&e| e as u8).sum();
        Self {
            exponents,
            total_order,
        }
    }

    /// Create a constant monomial (all exponents zero).
    pub fn constant() -> Self {
        Self {
            exponents: [0; MAX_VARS],
            total_order: 0,
        }
    }

    /// Create a monomial for the i-th variable (exponent 1, all others 0).
    ///
    /// # Arguments
    /// * `var_index` - Index of the variable (0-based)
    ///
    /// # Returns
    /// A monomial representing the single variable
    pub fn variable(var_index: usize) -> Self {
        let mut exponents = [0; MAX_VARS];
        exponents[var_index] = 1;
        Self {
            exponents,
            total_order: 1,
        }
    }

    /// Multiply two monomials by adding their exponents.
    ///
    /// # Arguments
    /// * `other` - The monomial to multiply with
    ///
    /// # Returns
    /// The product monomial
    pub fn multiply(&self, other: &Self) -> Self {
        let mut result_exponents = [0; MAX_VARS];
        for i in 0..MAX_VARS {
            result_exponents[i] = self.exponents[i].saturating_add(other.exponents[i]);
        }
        Self {
            exponents: result_exponents,
            total_order: self.total_order.saturating_add(other.total_order),
        }
    }

    /// Check if this monomial is within the given maximum order.
    pub fn within_order(&self, max_order: u32) -> bool {
        (self.total_order as u32) <= max_order
    }
}

/// Enumerate all monomials up to a given order with a given number of variables,
/// sorted in graded lexicographic order.
///
/// When `weights` is `Some`, exponents for variable `v` step in multiples of
/// `weights[v]` and the order budget is consumed by `weights[v]` per step.
/// Internal exponents are stored as `weight * actual_exponent`.
pub fn enumerate_monomials(max_order: u32, num_vars: u32, weights: Option<&[u32]>) -> Vec<Monomial> {
    let mut result = Vec::new();
    enumerate_monomials_recursive(max_order, num_vars as usize, 0, weights, &mut [0u8; MAX_VARS], &mut result);
    result.sort();
    result
}

fn enumerate_monomials_recursive(
    remaining_order: u32,
    num_vars: usize,
    var_idx: usize,
    weights: Option<&[u32]>,
    current: &mut [u8; MAX_VARS],
    result: &mut Vec<Monomial>,
) {
    if var_idx == num_vars {
        result.push(Monomial::new(*current));
        return;
    }
    let step = weights.map_or(1u32, |w| w[var_idx]);
    let mut cost = 0u32;
    while cost <= remaining_order {
        current[var_idx] = cost as u8; // internal exponent = weight * actual_exponent = cost
        enumerate_monomials_recursive(
            remaining_order - cost,
            num_vars,
            var_idx + 1,
            weights,
            current,
            result,
        );
        cost += step;
    }
    current[var_idx] = 0;
}

/// Graded lexicographic ordering: order by total degree first, then lexicographically.
impl Ord for Monomial {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by total order
        match self.total_order.cmp(&other.total_order) {
            Ordering::Equal => {
                // If same order, compare lexicographically
                self.exponents.cmp(&other.exponents)
            }
            ord => ord,
        }
    }
}

impl PartialOrd for Monomial {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for Monomial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Monomial(")?;
        let mut first = true;
        for (i, &exp) in self.exponents.iter().enumerate() {
            if exp > 0 {
                if !first {
                    write!(f, "*")?;
                }
                write!(f, "x{}^{}", i + 1, exp)?;
                first = false;
            }
        }
        if first {
            write!(f, "1")?; // constant monomial
        }
        write!(f, ")")
    }
}

impl fmt::Display for Monomial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (i, &exp) in self.exponents.iter().enumerate() {
            if exp > 0 {
                if !first {
                    write!(f, "*")?;
                }
                if exp == 1 {
                    write!(f, "x{}", i + 1)?;
                } else {
                    write!(f, "x{}^{}", i + 1, exp)?;
                }
                first = false;
            }
        }
        if first {
            write!(f, "1")?; // constant monomial
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monomial_creation() {
        let m = Monomial::constant();
        assert_eq!(m.total_order, 0);

        let m = Monomial::variable(0);
        assert_eq!(m.total_order, 1);
        assert_eq!(m.exponents[0], 1);
    }

    #[test]
    fn test_monomial_multiplication() {
        let m1 = Monomial::variable(0); // x1
        let m2 = Monomial::variable(1); // x2
        let product = m1.multiply(&m2);
        
        assert_eq!(product.exponents[0], 1);
        assert_eq!(product.exponents[1], 1);
        assert_eq!(product.total_order, 2);
    }

    #[test]
    fn test_monomial_ordering() {
        let m1 = Monomial::constant(); // order 0
        let m2 = Monomial::variable(0); // order 1
        let m3 = {
            let mut exp = [0; MAX_VARS];
            exp[0] = 2; // x1^2, order 2
            Monomial::new(exp)
        };

        assert!(m1 < m2);
        assert!(m2 < m3);
    }
}
