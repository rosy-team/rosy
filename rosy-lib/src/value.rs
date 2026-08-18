//! Runtime tagged value for COSY-style type reuse on one variable.

use anyhow::{Result, bail};

use crate::core::display::RosyDisplay;
use crate::intrinsics::*;
use crate::operators::*;
use crate::{BinaryOp, CD, CM, DA, LO, RE, ST, VE};

#[derive(Clone, Debug)]
pub enum RosyValue {
    RE(RE),
    ST(ST),
    LO(LO),
    CM(CM),
    VE(VE),
    DA(DA),
    CD(CD),
}

impl RosyValue {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::RE(_) => "RE",
            Self::ST(_) => "ST",
            Self::LO(_) => "LO",
            Self::CM(_) => "CM",
            Self::VE(_) => "VE",
            Self::DA(_) => "DA",
            Self::CD(_) => "CD",
        }
    }

    pub fn expect_re(self) -> Result<RE> {
        match self {
            Self::RE(v) => Ok(v),
            other => bail!("expected RE, held {}", other.kind_name()),
        }
    }
    pub fn expect_st(self) -> Result<ST> {
        match self {
            Self::ST(v) => Ok(v),
            other => bail!("expected ST, held {}", other.kind_name()),
        }
    }
    pub fn expect_lo(self) -> Result<LO> {
        match self {
            Self::LO(v) => Ok(v),
            other => bail!("expected LO, held {}", other.kind_name()),
        }
    }
    pub fn expect_cm(self) -> Result<CM> {
        match self {
            Self::CM(v) => Ok(v),
            other => bail!("expected CM, held {}", other.kind_name()),
        }
    }
    pub fn expect_ve(self) -> Result<VE> {
        match self {
            Self::VE(v) => Ok(v),
            other => bail!("expected VE, held {}", other.kind_name()),
        }
    }
    pub fn expect_da(self) -> Result<DA> {
        match self {
            Self::DA(v) => Ok(v),
            other => bail!("expected DA, held {}", other.kind_name()),
        }
    }
    pub fn expect_cd(self) -> Result<CD> {
        match self {
            Self::CD(v) => Ok(v),
            other => bail!("expected CD, held {}", other.kind_name()),
        }
    }
}

impl From<RE> for RosyValue {
    fn from(v: RE) -> Self {
        Self::RE(v)
    }
}
impl From<ST> for RosyValue {
    fn from(v: ST) -> Self {
        Self::ST(v)
    }
}
impl From<LO> for RosyValue {
    fn from(v: LO) -> Self {
        Self::LO(v)
    }
}
impl From<CM> for RosyValue {
    fn from(v: CM) -> Self {
        Self::CM(v)
    }
}
impl From<VE> for RosyValue {
    fn from(v: VE) -> Self {
        Self::VE(v)
    }
}
impl From<DA> for RosyValue {
    fn from(v: DA) -> Self {
        Self::DA(v)
    }
}
impl From<CD> for RosyValue {
    fn from(v: CD) -> Self {
        Self::CD(v)
    }
}
impl From<&RE> for RosyValue {
    fn from(v: &RE) -> Self {
        Self::RE(*v)
    }
}
impl From<&ST> for RosyValue {
    fn from(v: &ST) -> Self {
        Self::ST(v.clone())
    }
}
impl From<&LO> for RosyValue {
    fn from(v: &LO) -> Self {
        Self::LO(*v)
    }
}
impl From<&CM> for RosyValue {
    fn from(v: &CM) -> Self {
        Self::CM(*v)
    }
}
impl From<&VE> for RosyValue {
    fn from(v: &VE) -> Self {
        Self::VE(v.clone())
    }
}
impl From<&DA> for RosyValue {
    fn from(v: &DA) -> Self {
        Self::DA(v.clone())
    }
}
impl From<&CD> for RosyValue {
    fn from(v: &CD) -> Self {
        Self::CD(v.clone())
    }
}

impl RosyDisplay for &RosyValue {
    fn rosy_display(self) -> String {
        match self {
            RosyValue::RE(v) => v.rosy_display(),
            RosyValue::ST(v) => v.rosy_display(),
            RosyValue::LO(v) => v.rosy_display(),
            RosyValue::CM(v) => v.rosy_display(),
            RosyValue::VE(v) => v.rosy_display(),
            RosyValue::DA(v) => v.rosy_display(),
            RosyValue::CD(v) => v.rosy_display(),
        }
    }
}

impl RosyST for &RosyValue {
    fn rosy_to_string(self) -> String {
        self.rosy_display()
    }
}

impl RosyLO for &RosyValue {
    fn rosy_to_logical(self) -> LO {
        match self {
            RosyValue::RE(v) => v.rosy_to_logical(),
            RosyValue::LO(v) => v.rosy_to_logical(),
            other => panic!("LO() not defined for {}", other.kind_name()),
        }
    }
}

impl RosyCM for &RosyValue {
    fn rosy_cm(self) -> Result<CM> {
        match self {
            RosyValue::RE(v) => v.rosy_cm(),
            RosyValue::CM(v) => v.rosy_cm(),
            RosyValue::VE(v) => v.rosy_cm(),
            RosyValue::CD(v) => v.rosy_cm(),
            other => bail!("CM() not defined for {}", other.kind_name()),
        }
    }
}

impl RosySQR for RosyValue {
    type Output = RosyValue;
    fn rosy_sqr(&self) -> Result<Self::Output> {
        match self {
            RosyValue::RE(v) => Ok(RosyValue::from(v.rosy_sqr()?)),
            RosyValue::CM(v) => Ok(RosyValue::from(v.rosy_sqr()?)),
            RosyValue::VE(v) => Ok(RosyValue::from(v.rosy_sqr()?)),
            RosyValue::DA(v) => Ok(RosyValue::from(v.rosy_sqr()?)),
            RosyValue::CD(v) => Ok(RosyValue::from(v.rosy_sqr()?)),
            other => bail!("SQR not defined for {}", other.kind_name()),
        }
    }
}

impl RosyLENGTH for RosyValue {
    fn rosy_length(&self) -> RE {
        match self {
            RosyValue::RE(v) => v.rosy_length(),
            RosyValue::ST(v) => v.rosy_length(),
            RosyValue::LO(v) => v.rosy_length(),
            RosyValue::CM(v) => v.rosy_length(),
            RosyValue::VE(v) => v.rosy_length(),
            RosyValue::DA(v) => v.rosy_length(),
            RosyValue::CD(v) => v.rosy_length(),
        }
    }
}

impl RosyTYPE for RosyValue {
    fn rosy_type(&self) -> Result<RE> {
        match self {
            RosyValue::RE(v) => v.rosy_type(),
            RosyValue::ST(v) => v.rosy_type(),
            RosyValue::LO(v) => v.rosy_type(),
            RosyValue::CM(v) => v.rosy_type(),
            RosyValue::VE(v) => v.rosy_type(),
            RosyValue::DA(v) => v.rosy_type(),
            RosyValue::CD(v) => v.rosy_type(),
        }
    }
}

macro_rules! dyn_pairs {
    ($lhs:expr, $rhs:expr, $method:ident, $($L:ident, $R:ident);+ $(;)?) => {
        match ($lhs, $rhs) {
            $((RosyValue::$L(l), RosyValue::$R(r)) => Ok(RosyValue::from(l.$method(r)?)),)+
            (l, r) => bail!(
                "{} not defined for {} and {}",
                stringify!($method),
                l.kind_name(),
                r.kind_name()
            ),
        }
    };
}

pub fn rosy_dyn_binary(op: BinaryOp, lhs: &RosyValue, rhs: &RosyValue) -> Result<RosyValue> {
    match op {
        BinaryOp::Add => dyn_pairs!(
            lhs, rhs, rosy_add,
            RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
            CM, RE; CM, CM; CM, DA; CM, CD;
            VE, RE; VE, VE;
            DA, RE; DA, CM; DA, DA; DA, CD;
            CD, RE; CD, CM; CD, DA; CD, CD;
            LO, LO
        ),
        BinaryOp::Sub => dyn_pairs!(
            lhs, rhs, rosy_sub,
            RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
            CM, RE; CM, CM; CM, DA; CM, CD;
            VE, RE; VE, VE;
            DA, RE; DA, CM; DA, DA; DA, CD;
            CD, RE; CD, CM; CD, DA; CD, CD
        ),
        BinaryOp::Mult => dyn_pairs!(
            lhs, rhs, rosy_mult,
            RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
            CM, RE; CM, CM; CM, DA; CM, CD;
            VE, RE; VE, VE;
            DA, RE; DA, CM; DA, DA; DA, CD;
            CD, RE; CD, CM; CD, DA; CD, CD;
            LO, LO
        ),
        BinaryOp::Div => dyn_pairs!(
            lhs, rhs, rosy_div,
            RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
            CM, RE; CM, CM; CM, DA; CM, CD;
            VE, RE; VE, VE;
            DA, RE; DA, CM; DA, DA; DA, CD;
            CD, RE; CD, CM; CD, DA; CD, CD
        ),
        BinaryOp::Pow => dyn_pairs!(lhs, rhs, rosy_pow, RE, RE; VE, RE; DA, RE; CD, RE),
        BinaryOp::Extract => dyn_pairs!(
            lhs, rhs, rosy_extract,
            ST, RE; ST, VE;
            CM, RE;
            VE, RE; VE, VE;
            DA, RE; DA, VE;
            CD, RE; CD, VE
        ),
        BinaryOp::Concat => dyn_pairs!(
            lhs, rhs, rosy_concat,
            RE, RE; RE, VE;
            ST, ST;
            VE, RE; VE, VE
        ),
        BinaryOp::Eq => dyn_pairs!(lhs, rhs, rosy_eq, RE, RE; ST, ST; LO, LO),
        BinaryOp::Neq => dyn_pairs!(lhs, rhs, rosy_neq, RE, RE; ST, ST; LO, LO),
        BinaryOp::Lt => dyn_pairs!(lhs, rhs, rosy_lt, RE, RE; ST, ST),
        BinaryOp::Gt => dyn_pairs!(lhs, rhs, rosy_gt, RE, RE; ST, ST),
        BinaryOp::Lte => dyn_pairs!(lhs, rhs, rosy_lte, RE, RE; ST, ST),
        BinaryOp::Gte => dyn_pairs!(lhs, rhs, rosy_gte, RE, RE; ST, ST),
        BinaryOp::And => dyn_pairs!(lhs, rhs, rosy_and, LO, LO),
        BinaryOp::Or => dyn_pairs!(lhs, rhs, rosy_or, LO, LO),
        BinaryOp::Derive => match (lhs, rhs) {
            (RosyValue::DA(a), RosyValue::RE(i)) => Ok(RosyValue::DA(a.rosy_derive(*i as i64)?)),
            (RosyValue::CD(a), RosyValue::RE(i)) => Ok(RosyValue::CD(a.rosy_derive(*i as i64)?)),
            (l, r) => bail!("% not defined for {} and {}", l.kind_name(), r.kind_name()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_st_roundtrip() {
        let a = RosyValue::RE(2.0);
        let b = RosyValue::RE(3.0);
        let s = rosy_dyn_binary(BinaryOp::Add, &a, &b).unwrap();
        assert_eq!(s.expect_re().unwrap(), 5.0);
        assert_eq!((&RosyValue::LO(true)).rosy_to_string(), "TRUE");
    }
}
