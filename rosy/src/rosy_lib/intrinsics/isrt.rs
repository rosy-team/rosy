use std::collections::HashMap;

use crate::rosy_lib::{DA, RE, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for ISRT intrinsic function (inverse square root, x^(-1/2)).
///
/// According to COSY INFINITY manual, ISRT supports:
/// - RE -> RE
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: DA test value uses `4.0 + DA(1)` (constant part = 4, linear = x1)
/// because ISRT requires a positive constant part for the binomial expansion.
pub const ISRT_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "4.0"),
    IntrinsicTypeRule::new("VE", "VE", "4.0&9.0&16.0"),
    IntrinsicTypeRule::new("DA", "DA", "4.0 + DA(1)"),
];

/// Get the return type of ISRT for a given input type.
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

/// Trait for computing inverse square root of Rosy data types.
pub trait RosyISRT {
    type Output;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output>;
}

/// ISRT for real numbers: x^(-1/2) = 1/sqrt(x)
impl RosyISRT for RE {
    type Output = RE;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output> {
        Ok(1.0 / self.sqrt())
    }
}

/// ISRT for vectors (elementwise)
impl RosyISRT for VE {
    type Output = VE;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| 1.0 / x.sqrt()).collect())
    }
}

/// ISRT for DA (Taylor composition via binomial series with alpha = -0.5)
impl RosyISRT for DA {
    type Output = DA;
    fn rosy_isrt(&self) -> anyhow::Result<Self::Output> {
        da_isrt(self)
    }
}

/// Compute inverse square root of a DA object using binomial series.
///
/// Uses: f^alpha where alpha = -0.5
/// f^alpha = f0^alpha * (1 + u)^alpha  where u = (f - f0) / f0
/// (1 + u)^alpha = sum_{n=0}^{N} C(alpha, n) * u^n
/// C(alpha, n) = alpha*(alpha-1)*...*(alpha-n+1) / n!
fn da_isrt(da: &DA) -> anyhow::Result<DA> {
    use crate::rosy_lib::taylor::DACoefficient;

    let rt = crate::rosy_lib::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(
        f0 > 0.0,
        "ISRT: constant part of DA must be positive, got {}",
        f0
    );

    let alpha = -0.5_f64;
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
