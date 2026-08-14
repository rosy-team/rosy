use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, VE, DA};

/// Type registry for ISRT3 intrinsic function (x^(-3/2)).
///
/// According to COSY INFINITY manual, ISRT3 supports:
/// - RE -> RE
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: DA test value uses `4.0 + DA(1)` (constant part = 4, linear = x1)
/// because ISRT3 requires a positive constant part for the binomial expansion.
pub const ISRT3_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "4.0"),
    IntrinsicTypeRule::new("VE", "VE", "4.0&9.0&16.0"),
    IntrinsicTypeRule::new("DA", "DA", "4.0 + DA(1)"),
];

/// Get the return type of ISRT3 for a given input type.
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

/// Trait for computing x^(-3/2) of Rosy data types.
pub trait RosyISRT3 {
    type Output;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output>;
}

/// ISRT3 for real numbers: x^(-3/2)
impl RosyISRT3 for RE {
    type Output = RE;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        Ok(self.powf(-1.5))
    }
}

/// ISRT3 for vectors (elementwise)
impl RosyISRT3 for VE {
    type Output = VE;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.powf(-1.5)).collect())
    }
}

/// ISRT3 for DA (Taylor composition via binomial series with alpha = -1.5)
impl RosyISRT3 for DA {
    type Output = DA;
    fn rosy_isrt3(&self) -> anyhow::Result<Self::Output> {
        da_isrt3(self)
    }
}

/// Compute x^(-3/2) of a DA object using binomial series.
///
/// Uses: f^alpha where alpha = -1.5
/// f^alpha = f0^alpha * (1 + u)^alpha  where u = (f - f0) / f0
/// (1 + u)^alpha = sum_{n=0}^{N} C(alpha, n) * u^n
fn da_isrt3(da: &DA) -> anyhow::Result<DA> {
    use crate::taylor::DACoefficient;

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(f0 > 0.0, "ISRT3: constant part of DA must be positive, got {}", f0);

    let alpha = -1.5_f64;
    let f0_alpha = f0.powf(alpha);
    let da_prime = da.make_prime();
    let da_delta = (&da_prime * DA::from_coeff(1.0 / f0))?;

    // Binomial coefficients C(alpha, n) via recurrence
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(1.0);
    let mut binom_coeff = alpha;
    for n in 1..=nocut {
        xf.push(binom_coeff);
        binom_coeff *= (alpha - n as f64) / (n as f64 + 1.0);
    }

    let mut result = DA::horner_eval_with_rt(&da_delta, &xf, &rt)?;
    result = (&result * DA::from_coeff(f0_alpha))?;
    Ok(result)
}

