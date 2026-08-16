pub mod add;
pub mod sub;
pub mod mult;
pub mod div;
pub mod pow;
pub mod extract;
pub mod concat;
pub mod eq;
pub mod neq;
pub mod lt;
pub mod gt;
pub mod lte;
pub mod gte;
pub mod not;
pub mod and;
pub mod or;

pub use add::RosyAdd;
pub use sub::RosySub;
pub use mult::RosyMult;
pub use div::RosyDiv;
pub use pow::RosyPow;
pub use concat::RosyConcat;
pub use extract::RosyExtract;
pub use eq::RosyEq;
pub use neq::RosyNeq;
pub use lt::RosyLt;
pub use gt::RosyGt;
pub use lte::RosyLte;
pub use gte::RosyGte;
pub use not::RosyNot;
pub use and::RosyAnd;
pub use or::RosyOr;

use crate::{RosyType, RosyBaseType};

pub(crate) fn dim0(lhs: &RosyType, rhs: &RosyType) -> Option<(RosyBaseType, RosyBaseType)> {
    if lhs.dimensions == 0 && rhs.dimensions == 0 {
        Some((lhs.base_type, rhs.base_type))
    } else {
        None
    }
}

/// Arithmetic (+ - * /) result type. `with_lo` allows LO op LO → LO.
pub(crate) fn arith_return(
    lhs: &RosyType,
    rhs: &RosyType,
    with_lo: bool,
) -> Option<RosyType> {
    use RosyBaseType::*;
    match dim0(lhs, rhs)? {
        (RE, RE) => Some(RosyType::RE()),
        (RE, CM) | (CM, RE) | (CM, CM) => Some(RosyType::CM()),
        (RE, VE) | (VE, RE) | (VE, VE) => Some(RosyType::VE()),
        (RE, DA) | (DA, RE) | (DA, DA) => Some(RosyType::DA()),
        (RE, CD) | (CD, RE) | (CM, DA) | (CM, CD) | (DA, CM) | (DA, CD) | (CD, CM) | (CD, DA)
        | (CD, CD) => Some(RosyType::CD()),
        (LO, LO) if with_lo => Some(RosyType::LO()),
        _ => None,
    }
}