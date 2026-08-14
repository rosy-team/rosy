use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, VE, DA};

/// Type registry for ASIN intrinsic function.
///
/// According to COSY INFINITY manual, ASIN supports:
/// - RE -> RE
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: CM is NOT supported for ASIN in COSY.
pub const ASIN_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "0.5"),
    IntrinsicTypeRule::new("VE", "VE", "0.1&0.2&0.3"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
];

/// Get the return type of ASIN for a given input type.
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

/// Trait for computing arcsine of Rosy data types.
pub trait RosyASIN {
    type Output;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output>;
}

/// ASIN for real numbers
impl RosyASIN for RE {
    type Output = RE;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.asin())
    }
}

/// ASIN for vectors (elementwise)
impl RosyASIN for VE {
    type Output = VE;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.asin()).collect())
    }
}

/// ASIN for DA (Taylor composition)
///
/// Uses Taylor series: asin(f₀ + δf) = asin(f₀) + Σ (d^n/dx^n asin(x)|_{x=f₀} / n!) * (δf)^n
/// asin'(x) = 1/sqrt(1-x²), higher derivatives computed numerically via recurrence.
impl RosyASIN for DA {
    type Output = DA;
    fn rosy_asin(&self) -> anyhow::Result<Self::Output> {
        da_asin(self)
    }
}

/// Compute arcsine of a DA object using Taylor series composition.
fn da_asin(da: &DA) -> anyhow::Result<DA> {
    use crate::taylor::DACoefficient;

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Compute derivatives of asin at f0 via recurrence
    let mut derivs = vec![0.0f64; nocut + 1];
    derivs[0] = f0.asin();
    if nocut >= 1 {
        let denom = (1.0 - f0 * f0).sqrt();
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ASIN is not differentiable at x = ±1"));
        }
        derivs[1] = 1.0 / denom;
    }
    if nocut >= 2 {
        let denom = 1.0 - f0 * f0;
        if denom.abs() < 1e-15 {
            return Err(anyhow::anyhow!("ASIN is not differentiable at x = ±1"));
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

