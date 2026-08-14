use std::collections::HashMap;

use crate::{IntrinsicTypeRule, RosyType};
use crate::{RE, CM, VE, DA, CD};

/// Type registry for SIN intrinsic function.
/// 
/// According to COSY INFINITY manual, SIN supports:
/// - RE -> RE
/// - CM -> CM (complex sin)
/// - VE -> VE (elementwise)
/// - DA -> DA (Taylor composition)
/// - CD -> CD (complex Taylor composition)
pub const SIN_REGISTRY: &[IntrinsicTypeRule] = &[
    IntrinsicTypeRule::new("RE", "RE", "1.5"),
    IntrinsicTypeRule::new("CM", "CM", "CM(1.5&2.5)"),
    IntrinsicTypeRule::new("VE", "VE", "1.5&2.5&3.5"),
    IntrinsicTypeRule::new("DA", "DA", "DA(1)"),
    IntrinsicTypeRule::new("CD", "CD", "CD(1)"),
];

/// Get the return type of SIN for a given input type.
pub fn get_return_type(input: &RosyType) -> Option<RosyType> {
    let registry: HashMap<RosyType, RosyType> = {
        let mut m = HashMap::new();
        let all = vec![
            (RosyType::RE(), RosyType::RE()),
            (RosyType::CM(), RosyType::CM()),
            (RosyType::VE(), RosyType::VE()),
            (RosyType::DA(), RosyType::DA()),
            (RosyType::CD(), RosyType::CD()),
        ];
        for (input_type, result_type) in all {
            m.insert(input_type, result_type);
        }
        m
    };

    registry.get(input).copied()
}

/// Trait for computing sine of Rosy data types.
pub trait RosySIN {
    type Output;
    fn rosy_sin(&self) -> anyhow::Result<Self::Output>;
}

/// SIN for real numbers
impl RosySIN for RE {
    type Output = RE;
    fn rosy_sin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sin())
    }
}

/// SIN for complex numbers
impl RosySIN for CM {
    type Output = CM;
    fn rosy_sin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.sin())
    }
}

/// SIN for vectors (elementwise)
impl RosySIN for VE {
    type Output = VE;
    fn rosy_sin(&self) -> anyhow::Result<Self::Output> {
        Ok(self.iter().map(|x| x.sin()).collect())
    }
}

/// SIN for DA (Taylor composition)
impl RosySIN for DA {
    type Output = DA;
    fn rosy_sin(&self) -> anyhow::Result<Self::Output> {
        da_sin(self)
    }
}

/// SIN for CD (complex Taylor composition)
impl RosySIN for CD {
    type Output = CD;
    fn rosy_sin(&self) -> anyhow::Result<Self::Output> {
        cd_sin(self)
    }
}

/// Compute sine of a DA object using Horner's method for Taylor composition.
///
/// Evaluates P(δf) = c₀ + δf·(c₁ + δf·(c₂ + ...)) where c_n = d^n(sin)(f₀)/n!
/// Horner's reduces allocations from 3 per iteration to 1 (just the DA×DA multiply).
fn da_sin(da: &DA) -> anyhow::Result<DA> {
    let rt = crate::taylor::get_runtime()?;
    let nocut = rt.config.max_order as usize;

    let f0 = da.constant_part();
    let da_prime = da.make_prime();

    // DACE-style recurrence: xf[i] = -xf[i-2] / (i*(i-1))
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.sin());
    if nocut >= 1 { xf.push(f0.cos()); }
    for i in 2..=nocut {
        xf.push(-xf[i - 2] / ((i * (i - 1)) as f64));
    }

    DA::horner_eval_with_rt(&da_prime, &xf, &rt)
}

/// Compute sine of a CD object using Horner's method for Taylor composition.
fn cd_sin(cd: &CD) -> anyhow::Result<CD> {
    use crate::taylor::DACoefficient;
    use num_complex::Complex64;

    let config = crate::taylor::get_config()?;
    let nocut = config.max_order as usize;

    let f0 = cd.constant_part();
    let cd_prime = cd.make_prime();

    // DACE-style recurrence for complex sin coefficients
    let mut xf = Vec::with_capacity(nocut + 1);
    xf.push(f0.sin());
    if nocut >= 1 { xf.push(f0.cos()); }
    for i in 2..=nocut {
        xf.push(-xf[i - 2] / Complex64::new((i * (i - 1)) as f64, 0.0));
    }

    CD::horner_eval(&cd_prime, &xf)
}

