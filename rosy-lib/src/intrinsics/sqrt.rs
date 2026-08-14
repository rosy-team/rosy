use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, VE, DA};

/// Type registry for SQRT intrinsic function.
///
/// According to COSY INFINITY manual, SQRT supports:
/// - RE -> RE
/// - CM -> CM
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
///
/// Note: DA test value uses EXP(DA(1)) to ensure a positive constant part,
/// which is required for the binomial series expansion of sqrt.
pub const SQRT_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "4.0"),
    IntrinsicTypeRule::new("CM", "CM", "CM(3.0&4.0)"),
    IntrinsicTypeRule::new("VE", "VE", "1.0&4.0&9.0"),
    IntrinsicTypeRule::new("DA", "DA", "EXP(DA(1))"),
];

/// Get the return type of SQRT for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::CM(), RosyType::CM()),
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

/// Trait for computing the square root of Rosy data types.
pub trait RosySQRT {
    type Output;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output>;
}

/// SQRT for real numbers
impl RosySQRT for RE {
    type Output = RE;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sqrt())
    }
}

/// SQRT for complex numbers
impl RosySQRT for CM {
    type Output = CM;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sqrt())
    }
}

/// SQRT for vectors (elementwise)
impl RosySQRT for VE {
    type Output = VE;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.sqrt()).collect())
    }
}

/// SQRT for DA (Taylor composition via binomial series)
impl RosySQRT for DA {
    type Output = DA;
    fn rosy_sqrt(&self) -> anyhow::Result<Self::Output> {
        da_sqrt(self)
    }
}

/// Compute square root of a DA object using binomial series expansion.
///
/// Uses: sqrt(f) = sqrt(f0) * sqrt(1 + u)  where u = (f - f0) / f0
/// sqrt(1 + u) = sum_{n=0}^{N} C(1/2, n) * u^n
/// where C(1/2, n) = (1/2)(1/2-1)...(1/2-n+1) / n!
///
/// Requires: f0 = constant part of the DA > 0 (sqrt is not analytic at 0).
fn da_sqrt(da: &DA) -> anyhow::Result<DA> {
    use crate::taylor::DACoefficient;

    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    anyhow::ensure!(f0 > 0.0, "SQRT: constant part of DA must be positive, got {}", f0);

    let sqrt_f0 = f0.sqrt();
    let da_prime = da.make_prime();
    let da_delta = (&da_prime * DA::from_coeff(1.0 / f0))?;

    // Binomial coefficients C(1/2, n) via recurrence
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(1.0);
    let mut binom_coeff = 0.5_f64;
    for n in 1..=nocut {
        xf.push(binom_coeff);
        binom_coeff *= (0.5 - n as f64) / (n as f64 + 1.0);
    }

    let mut result = DA::horner_eval_with_rt(&da_delta, &xf, &rt)?;
    result = (&result * DA::from_coeff(sqrt_f0))?;
    Ok(result)
}

