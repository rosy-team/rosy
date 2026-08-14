use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, VE, DA};

/// Type registry for ATAN intrinsic function.
///
/// According to COSY INFINITY manual, ATAN supports:
/// - RE -> RE
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: CM is NOT supported for ATAN in COSY.
pub const ATAN_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("VE", "VE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
];

/// Get the return type of ATAN for a given input type.
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
    use crate::taylor::DACoefficient;

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

