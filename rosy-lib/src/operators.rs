pub mod add;
pub mod and;
pub mod concat;
pub mod div;
pub mod eq;
pub mod extract;
pub mod gt;
pub mod gte;
pub mod lt;
pub mod lte;
pub mod mult;
pub mod neq;
pub mod not;
pub mod or;
pub mod pow;
pub mod sub;

pub use add::RosyAdd;
pub use and::RosyAnd;
pub use concat::RosyConcat;
pub use div::RosyDiv;
pub use eq::RosyEq;
pub use extract::RosyExtract;
pub use gt::RosyGt;
pub use gte::RosyGte;
pub use lt::RosyLt;
pub use lte::RosyLte;
pub use mult::RosyMult;
pub use neq::RosyNeq;
pub use not::RosyNot;
pub use or::RosyOr;
pub use pow::RosyPow;
pub use sub::RosySub;

use crate::{RosyBaseType, RosyType};

pub(crate) fn dim0(lhs: &RosyType, rhs: &RosyType) -> Option<(RosyBaseType, RosyBaseType)> {
    if lhs.dimensions == 0 && rhs.dimensions == 0 {
        Some((lhs.base_type, rhs.base_type))
    } else {
        None
    }
}

/// Arithmetic (+ - * /) result type. `with_lo` allows LO op LO → LO.
pub(crate) fn arith_return(lhs: &RosyType, rhs: &RosyType, with_lo: bool) -> Option<RosyType> {
    use RosyBaseType::*;
    match dim0(lhs, rhs)? {
        (RE, RE) => Some(RosyType::RE()),
        (RE, CM) | (CM, RE) | (CM, CM) => Some(RosyType::CM()),
        (RE, VE) | (VE, RE) | (VE, VE) => Some(RosyType::VE()),
        (RE, DA) | (DA, RE) | (DA, DA) => Some(RosyType::DA()),
        (RE, CD)
        | (CD, RE)
        | (CM, DA)
        | (CM, CD)
        | (DA, CM)
        | (DA, CD)
        | (CD, CM)
        | (CD, DA)
        | (CD, CD) => Some(RosyType::CD()),
        (LO, LO) if with_lo => Some(RosyType::LO()),
        _ => None,
    }
}
