use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, VE, DA};

/// Type registry for ACOS intrinsic function.
///
/// According to COSY INFINITY manual, ACOS supports:
/// - RE -> RE
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: CM is NOT supported for ACOS in COSY.
pub const ACOS_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "0.5"),
    IntrinsicTypeRule::new("VE", "VE", "0.1&0.2&0.3"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
];

/// Get the return type of ACOS for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::VE(), RosyType::VE()),
            (RosyType::DA(), RosyType::DA()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for computing arccosine of Rosy data types.
pub trait RosyACOS {
    type Output;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output>;
}

/// ACOS for real numbers
impl RosyACOS for RE {
    type Output = RE;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.acos())
    }
}

/// ACOS for vectors (elementwise)
impl RosyACOS for VE {
    type Output = VE;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.acos()).collect())
    }
}

/// ACOS for DA (Taylor composition)
///
/// acos(x) = pi/2 - asin(x), so derivatives are the negatives of asin derivatives
/// (except the constant term).
impl RosyACOS for DA {
    type Output = DA;
    fn rosy_acos(&self) -> anyhow::Result<Self::Output> {
        da_acos(self)
    }
}

/// Compute arccosine of a DA object using Taylor series composition.
///
/// Uses the identity acos(x) = pi/2 - asin(x), so derivatives of acos are
/// the negative of the derivatives of asin (for n >= 1).
fn da_acos(da: &DA) -> anyhow::Result<DA> {
    use crate::taylor::DACoefficient;

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Compute derivatives of acos at f0 (same recurrence as asin, but derivs[1] = -1/sqrt(1-x^2))
    let mut derivs = vec![0.0f64; nocut + 1];
    derivs[0] = f0.acos();
    if nocut >= 1 {
        let denom = (1.0 - f0 * f0).sqrt();
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ACOS is not differentiable at x = ±1"));
        }
        derivs[1] = -1.0 / denom;
    }
    if nocut >= 2 {
        let denom = 1.0 - f0 * f0;
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ACOS is not differentiable at x = ±1"));
        }
        for n in 1..nocut {
            let n_f = n as f64;
            derivs[n + 1] = ((2.0 * n_f - 1.0) * f0 * derivs[n] + (n_f - 1.0).powi(2) * derivs[n - 1]) / (1.0 - f0 * f0);
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

