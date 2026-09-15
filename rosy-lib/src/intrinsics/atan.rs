use crate::RosyType;
use crate::{RE, VE, DA};

/// Get the return type of ATAN for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    match input {
        t if *t == RosyType::RE() => Some(RosyType::RE()),
        t if *t == RosyType::VE() => Some(RosyType::VE()),
        t if *t == RosyType::DA() => Some(RosyType::DA()),
        _ => None,
    }
}

/// Trait for computing arctangent of Rosy data types.
pub trait RosyATAN {
    type Output;
    fn rosy_atan(&self) -> anyhow::Result<Self::Output>;
}

/// ATAN for real numbers
impl RosyATAN for RE {
    type Output = RE;
    fn rosy_atan(&self) -> anyhow::Result<Self::Output> {
        Ok(self.atan())
    }
}

/// ATAN for vectors (elementwise)
impl RosyATAN for VE {
    type Output = VE;
    fn rosy_atan(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.atan()).collect())
    }
}

/// ATAN for DA (Taylor composition)
///
/// atan'(x) = 1/(1+x²), higher derivatives computed via recurrence.
impl RosyATAN for DA {
    type Output = DA;
    fn rosy_atan(&self) -> anyhow::Result<Self::Output> {
        da_atan(self)
    }
}

/// Compute arctangent of a DA object using Taylor series composition.
///
/// Uses the recurrence for atan derived from (1+x²)*f'(x) = 1:
/// Differentiating n times: (1+x²)*f^(n+1) + 2*n*x*f^(n) + n*(n-1)*f^(n-1) = 0
/// => f^(n+1) = -[2*n*x*f^(n) + n*(n-1)*f^(n-1)] / (1+x²)
fn da_atan(da: &DA) -> anyhow::Result<DA> {
    

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Compute derivatives of atan at f0 via recurrence
    let mut derivs = vec![0.0f64; nocut + 1];
    derivs[0] = f0.atan();
    if nocut >= 1 {
        derivs[1] = 1.0 / (1.0 + f0 * f0);
    }
    if nocut >= 2 {
        let denom = 1.0 + f0 * f0;
        for n in 1..nocut {
            let n_f = n as f64;
            derivs[n + 1] = -(2.0 * n_f * f0 * derivs[n] + n_f * (n_f - 1.0) * derivs[n - 1]) / denom;
        }
    }

    // Taylor coefficients c_n = derivs[n] / n!
    let mut xf = Vec::with_capacity(nocut + 1);
    let mut factorial = 1.0;
    for n in 0..=nocut {
        if n > 0 { factorial *= n as f64; }
        xf.push(derivs[n] / factorial);
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taylor::config::{cleanup_taylor, get_runtime, init_taylor, set_epsilon};
    use serial_test::serial;

    #[test]
    #[serial]
    fn atan_far_field_linear_coefficient_matches_analytic_derivative() {
        cleanup_taylor();
        init_taylor(3, 1).unwrap();
        set_epsilon(1e-16).unwrap();

        // First CML field evaluation: atan(-50000 + 10*x).
        let mut argument = (DA::variable(1).unwrap() * 10.0).unwrap();
        argument.add_constant_in_place(-50000.0);
        let result = argument.rosy_atan().unwrap();
        let linear = get_runtime().unwrap().variable_indices[0] as usize;
        let expected = 10.0 / (1.0 + 50000.0 * 50000.0);
        assert!(
            (result.coeffs[linear] - expected).abs() <= 2.0 * f64::EPSILON * expected,
            "expected {expected:.17e}, got {:.17e}",
            result.coeffs[linear]
        );

        cleanup_taylor();
    }
}
