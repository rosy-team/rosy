use std::collections::HashMap;

use crate::rosy_lib::{CM, DA, RE, VE};
use crate::rosy_lib::{IntrinsicTypeRule, RosyType};

/// Type registry for SINH intrinsic function.
///
/// According to COSY INFINITY manual, SINH supports:
/// - RE -> RE
/// - CM -> CM (complex hyperbolic sine)
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
pub const SINH_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "CM", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("VE", "VE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
];

/// Get the return type of SINH for a given input type.
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

/// Trait for computing hyperbolic sine of Rosy data types.
pub trait RosySINH {
    type Output;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output>;
}

/// SINH for real numbers
impl RosySINH for RE {
    type Output = RE;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sinh())
    }
}

/// SINH for complex numbers
impl RosySINH for CM {
    type Output = CM;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sinh())
    }
}

/// SINH for vectors (elementwise)
impl RosySINH for VE {
    type Output = VE;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.sinh()).collect())
    }
}

/// SINH for DA (Taylor composition)
impl RosySINH for DA {
    type Output = DA;
    fn rosy_sinh(&self) -> anyhow::Result<Self::Output> {
        da_sinh(self)
    }
}

/// Compute hyperbolic sine of a DA object using Horner's method.
///
/// `c_n = [sinh_f0, cosh_f0][n%2] / n!`
fn da_sinh(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::rosy_lib::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // Recurrence: xf[i] = xf[i-2] / (i*(i-1))  (no negation for sinh/cosh)
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.sinh());
    if nocut >= 1 {
        xf.push(f0.cosh());
    }
    for i in 2..=nocut {
        xf.push(xf[i - 2] / ((i * (i - 1)) as f64));
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}
