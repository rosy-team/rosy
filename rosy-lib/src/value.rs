//! Runtime tagged value for COSY-style type reuse on one variable.

use anyhow::{Result, bail};
use std::borrow::Cow;

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
    /// Nested COSY cell (array of ANY).
    Arr(Vec<RosyValue>),
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
            Self::Arr(_) => "ARR",
        }
    }

    pub fn expect_re(&self) -> Result<RE> {
        match self {
            Self::RE(v) => Ok(*v),
            other => Ok(other.as_f64()),
        }
    }
    pub fn expect_st(self) -> Result<ST> {
        match self {
            Self::ST(v) => Ok(v),
            other => Ok((&other).rosy_to_string()),
        }
    }
    pub fn expect_lo(self) -> Result<LO> {
        match self {
            Self::LO(v) => Ok(v),
            other => Ok(other.as_f64() != 0.0),
        }
    }
    pub fn expect_cm(self) -> Result<CM> {
        match self {
            Self::CM(v) => Ok(v),
            Self::CD(v) => Ok(v.constant_part()),
            other => Ok(CM::new(other.as_f64(), 0.0)),
        }
    }
    pub fn expect_ve(self) -> Result<VE> {
        match self {
            Self::VE(v) => Ok(v),
            Self::Arr(v) => Ok(v.into_iter().map(|x| x.as_f64()).collect()),
            other => Ok(vec![other.as_f64()]),
        }
    }
    pub fn expect_da(self) -> Result<DA> {
        match self {
            Self::DA(v) => Ok(v),
            // ponytail: needs DAINI, same as any other DA ctor
            other => Ok(DA::constant(other.as_f64())),
        }
    }
    pub fn expect_cd(self) -> Result<CD> {
        Ok(self.as_cd())
    }

    /// Promote DA / RE / CM into CD so fox CDNFDA/CPOLVAL/CDFLO see the series.
    pub fn as_cd(&self) -> CD {
        match self {
            Self::CD(v) => v.clone(),
            Self::DA(v) => CD::from_da(v),
            Self::CM(v) => CD::complex_constant(*v),
            Self::Arr(v) => v.first().map(|x| x.as_cd()).unwrap_or_else(CD::zero),
            other => CD::constant(other.as_f64()),
        }
    }
    pub fn expect_arr(self) -> Result<Vec<RosyValue>> {
        match self {
            Self::Arr(v) => Ok(v),
            Self::VE(v) => Ok(v.into_iter().map(RosyValue::RE).collect()),
            other => Ok(vec![other]),
        }
    }
    pub fn expect_arr2(self) -> Result<Vec<Vec<RosyValue>>> {
        Ok(self
            .expect_arr()?
            .into_iter()
            .map(|row| match row {
                RosyValue::Arr(v) => v,
                other => vec![other],
            })
            .collect())
    }
    pub fn expect_re2(self) -> Result<Vec<Vec<f64>>> {
        Ok(self
            .expect_arr2()?
            .into_iter()
            .map(|row| row.into_iter().map(|x| x.as_f64()).collect())
            .collect())
    }
}

impl From<&RosyValue> for RosyValue {
    fn from(v: &RosyValue) -> Self {
        v.clone()
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
        Self::Arr(v.into_iter().map(RosyValue::RE).collect())
    }
}
impl From<Vec<RosyValue>> for RosyValue {
    fn from(v: Vec<RosyValue>) -> Self {
        Self::Arr(v)
    }
}
impl From<Vec<Vec<RosyValue>>> for RosyValue {
    fn from(v: Vec<Vec<RosyValue>>) -> Self {
        Self::Arr(v.into_iter().map(RosyValue::Arr).collect())
    }
}
impl From<&Vec<RosyValue>> for RosyValue {
    fn from(v: &Vec<RosyValue>) -> Self {
        Self::Arr(v.clone())
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
impl From<Vec<DA>> for RosyValue {
    fn from(v: Vec<DA>) -> Self {
        Self::Arr(v.into_iter().map(RosyValue::DA).collect())
    }
}
impl From<Vec<CD>> for RosyValue {
    fn from(v: Vec<CD>) -> Self {
        Self::Arr(v.into_iter().map(RosyValue::CD).collect())
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
        Self::Arr(v.iter().copied().map(RosyValue::RE).collect())
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
            RosyValue::Arr(v) => format!(
                "[{}]",
                v.iter()
                    .map(|x| x.rosy_display())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

impl RosyST for &RosyValue {
    fn rosy_to_string(self) -> String {
        match self {
            RosyValue::RE(v) => v.rosy_to_string(),
            other => other.rosy_display(),
        }
    }
}

impl RosyLO for &RosyValue {
    fn rosy_to_logical(self) -> LO {
        match self {
            RosyValue::RE(v) => v.rosy_to_logical(),
            RosyValue::LO(v) => v.rosy_to_logical(),
            other => other.as_f64() != 0.0,
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
            RosyValue::Arr(_) => self.clone().expect_ve()?.rosy_cm(),
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
            RosyValue::Arr(_) => {
                let v = self.clone().expect_ve()?;
                Ok(RosyValue::from(v.rosy_sqr()?))
            }
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
            RosyValue::Arr(v) => v.len() as RE,
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
            RosyValue::Arr(_) => Ok(5.0),
        }
    }
}

impl Default for RosyValue {
    fn default() -> Self {
        Self::RE(0.0)
    }
}

impl From<Vec<Vec<f64>>> for RosyValue {
    fn from(v: Vec<Vec<f64>>) -> Self {
        Self::Arr(
            v.into_iter()
                .map(|row| RosyValue::Arr(row.into_iter().map(RosyValue::RE).collect()))
                .collect(),
        )
    }
}

impl AsRef<[f64]> for RosyValue {
    fn as_ref(&self) -> &[f64] {
        match self {
            Self::VE(v) => v.as_slice(),
            other => panic!("cannot index {} as a vector", other.kind_name()),
        }
    }
}

impl RosyValue {
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::RE(v) => *v,
            Self::LO(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Self::CM(c) => c.re,
            Self::VE(v) => v.first().copied().unwrap_or(0.0),
            Self::DA(d) => d.constant_part(),
            Self::CD(d) => d.constant_part().re,
            Self::ST(s) => s.trim().replace(['D', 'd'], "E").parse().unwrap_or(0.0),
            Self::Arr(v) => v.first().map(|x| x.as_f64()).unwrap_or(0.0),
        }
    }

    pub fn round(&self) -> f64 {
        self.as_f64().round()
    }

    pub fn abs(&self) -> f64 {
        self.as_f64().abs()
    }

    pub fn powi(&self, n: i32) -> f64 {
        self.as_f64().powi(n)
    }

    pub fn sqrt(&self) -> f64 {
        self.as_f64().sqrt()
    }

    pub fn atan(&self) -> f64 {
        self.as_f64().atan()
    }

    pub fn len(&self) -> usize {
        match self {
            Self::VE(v) => v.len(),
            Self::ST(s) => s.len(),
            _ => 1,
        }
    }

    pub fn trim(&self) -> String {
        match self {
            Self::ST(s) => s.trim().to_string(),
            other => other.as_f64().to_string(),
        }
    }

    pub fn iter(&self) -> std::vec::IntoIter<f64> {
        match self {
            Self::VE(v) => v.clone().into_iter(),
            Self::RE(v) => vec![*v].into_iter(),
            _ => Vec::new().into_iter(),
        }
    }
}

impl std::ops::Neg for RosyValue {
    type Output = RosyValue;
    fn neg(self) -> Self {
        rosy_dyn_binary(BinaryOp::Sub, &RosyValue::RE(0.0), &self).unwrap_or(RosyValue::RE(0.0))
    }
}

impl std::ops::Neg for &RosyValue {
    type Output = RosyValue;
    fn neg(self) -> RosyValue {
        rosy_dyn_binary(BinaryOp::Sub, &RosyValue::RE(0.0), self).unwrap_or(RosyValue::RE(0.0))
    }
}

impl std::ops::Add for RosyValue {
    type Output = RosyValue;
    fn add(self, rhs: Self) -> Self {
        rosy_dyn_binary(BinaryOp::Add, &self, &rhs).unwrap_or(RosyValue::RE(0.0))
    }
}
impl std::ops::Sub for RosyValue {
    type Output = RosyValue;
    fn sub(self, rhs: Self) -> Self {
        rosy_dyn_binary(BinaryOp::Sub, &self, &rhs).unwrap_or(RosyValue::RE(0.0))
    }
}
impl std::ops::Mul for RosyValue {
    type Output = RosyValue;
    fn mul(self, rhs: Self) -> Self {
        rosy_dyn_binary(BinaryOp::Mult, &self, &rhs).unwrap_or(RosyValue::RE(0.0))
    }
}
impl std::ops::Div for RosyValue {
    type Output = RosyValue;
    fn div(self, rhs: Self) -> Self {
        rosy_dyn_binary(BinaryOp::Div, &self, &rhs).unwrap_or(RosyValue::RE(0.0))
    }
}

macro_rules! rosy_value_arith_f64 {
    ($trait:ident, $method:ident, $op:ident) => {
        impl std::ops::$trait<f64> for RosyValue {
            type Output = RosyValue;
            fn $method(self, rhs: f64) -> Self {
                rosy_dyn_binary(BinaryOp::$op, &self, &RosyValue::RE(rhs))
                    .unwrap_or(RosyValue::RE(0.0))
            }
        }
        impl std::ops::$trait<RosyValue> for f64 {
            type Output = RosyValue;
            fn $method(self, rhs: RosyValue) -> Self::Output {
                rosy_dyn_binary(BinaryOp::$op, &RosyValue::RE(self), &rhs)
                    .unwrap_or(RosyValue::RE(0.0))
            }
        }
    };
}
rosy_value_arith_f64!(Add, add, Add);
rosy_value_arith_f64!(Sub, sub, Sub);
rosy_value_arith_f64!(Mul, mul, Mult);
rosy_value_arith_f64!(Div, div, Div);

impl PartialEq for RosyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ST(a), Self::ST(b)) => a == b,
            (Self::LO(a), Self::LO(b)) => a == b,
            _ => self.as_f64() == other.as_f64(),
        }
    }
}
impl PartialEq<f64> for RosyValue {
    fn eq(&self, other: &f64) -> bool {
        self.as_f64() == *other
    }
}
impl PartialEq<RosyValue> for f64 {
    fn eq(&self, other: &RosyValue) -> bool {
        *self == other.as_f64()
    }
}
impl PartialOrd for RosyValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_f64().partial_cmp(&other.as_f64())
    }
}
impl PartialOrd<f64> for RosyValue {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.as_f64().partial_cmp(other)
    }
}
impl PartialOrd<RosyValue> for f64 {
    fn partial_cmp(&self, other: &RosyValue) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&other.as_f64())
    }
}

macro_rules! rosy_value_unary {
    ($trait:ident, $method:ident, $($var:ident),+) => {
        impl $trait for RosyValue {
            type Output = RosyValue;
            fn $method(&self) -> Result<Self::Output> {
                match self {
                    $(Self::$var(v) => Ok(RosyValue::from(v.$method()?)),)+
                    Self::Arr(_) => {
                        let v = self.clone().expect_ve()?;
                        Ok(RosyValue::from(v.$method()?))
                    }
                    other => bail!(
                        concat!(stringify!($method), " not defined for {}"),
                        other.kind_name()
                    ),
                }
            }
        }
    };
}

impl RosyCONS for RosyValue {
    type Output = RosyValue;
    fn rosy_cons(&self) -> Result<Self::Output> {
        match self {
            Self::RE(v) => Ok(RosyValue::from(v.rosy_cons()?)),
            Self::CM(v) => Ok(RosyValue::from(v.rosy_cons()?)),
            Self::VE(v) => Ok(RosyValue::from(v.rosy_cons()?)),
            Self::DA(v) => Ok(RosyValue::from(v.rosy_cons()?)),
            Self::CD(v) => Ok(RosyValue::from(v.rosy_cons()?)),
            Self::Arr(_) => {
                let v = self.clone().expect_ve()?;
                Ok(RosyValue::from(v.rosy_cons()?))
            }
            other => bail!("rosy_cons not defined for {}", other.kind_name()),
        }
    }
}

impl RosyABS for RosyValue {
    type Output = RosyValue;
    fn rosy_abs(&self) -> Result<Self::Output> {
        match self {
            Self::RE(v) => Ok(RosyValue::from(v.rosy_abs()?)),
            Self::CM(v) => Ok(RosyValue::from(v.rosy_abs()?)),
            Self::VE(v) => Ok(RosyValue::from(v.rosy_abs()?)),
            Self::DA(v) => Ok(RosyValue::from(v.rosy_abs()?)),
            Self::CD(v) => Ok(RosyValue::from(v.rosy_abs()?)),
            Self::Arr(_) => {
                let v = self.clone().expect_ve()?;
                Ok(RosyValue::from(v.rosy_abs()?))
            }
            other => bail!("rosy_abs not defined for {}", other.kind_name()),
        }
    }
}
rosy_value_unary!(RosySQRT, rosy_sqrt, RE, CM, VE, DA);
rosy_value_unary!(RosySIN, rosy_sin, RE, CM, VE, DA, CD);
rosy_value_unary!(RosyCOS, rosy_cos, RE, CM, VE, DA, CD);
rosy_value_unary!(RosyEXP, rosy_exp, RE, CM, VE, DA, CD);
rosy_value_unary!(RosyLOG, rosy_log, RE, CM, VE, DA);
rosy_value_unary!(RosyTAN, rosy_tan, RE, VE, DA);
rosy_value_unary!(RosyATAN, rosy_atan, RE, VE, DA);
rosy_value_unary!(RosyCOSH, rosy_cosh, RE, CM, VE, DA);
rosy_value_unary!(RosySINH, rosy_sinh, RE, CM, VE, DA);
rosy_value_unary!(RosyTANH, rosy_tanh, RE, VE, DA);
rosy_value_unary!(RosyACOS, rosy_acos, RE, VE, DA);
rosy_value_unary!(RosyASIN, rosy_asin, RE, VE, DA);
rosy_value_unary!(RosyISRT, rosy_isrt, RE, VE, DA);
rosy_value_unary!(RosyINT, rosy_int, RE, VE);
rosy_value_unary!(RosyNINT, rosy_nint, RE, VE);

impl RosyREAL for RosyValue {
    type Output = RosyValue;
    fn rosy_real(&self) -> Result<Self::Output> {
        match self {
            Self::RE(v) => Ok(RosyValue::from(v.rosy_real()?)),
            Self::CM(v) => Ok(RosyValue::from(v.rosy_real()?)),
            Self::DA(v) => Ok(RosyValue::from(v.rosy_real()?)),
            Self::CD(v) => Ok(RosyValue::from(v.rosy_real()?)),
            other => bail!("rosy_real not defined for {}", other.kind_name()),
        }
    }
}
impl RosyIMAG for RosyValue {
    type Output = RosyValue;
    fn rosy_imag(&self) -> Result<Self::Output> {
        match self {
            Self::RE(v) => Ok(RosyValue::from(v.rosy_imag()?)),
            Self::CM(v) => Ok(RosyValue::from(v.rosy_imag()?)),
            Self::DA(v) => Ok(RosyValue::from(v.rosy_imag()?)),
            Self::CD(v) => Ok(RosyValue::from(v.rosy_imag()?)),
            other => bail!("rosy_imag not defined for {}", other.kind_name()),
        }
    }
}

impl RosyVMAX for RosyValue {
    fn rosy_vmax(&self) -> Result<RE> {
        match self {
            Self::RE(v) => v.rosy_vmax(),
            Self::VE(v) => v.rosy_vmax(),
            Self::Arr(v) => v
                .iter()
                .map(|x| x.as_f64())
                .reduce(f64::max)
                .ok_or_else(|| anyhow::anyhow!("VMAX of empty ARR")),
            other => bail!("VMAX not defined for {}", other.kind_name()),
        }
    }
}

impl crate::intrinsics::conj::RosyCONJ for RosyValue {
    type Output = RosyValue;
    fn rosy_conj(&self) -> Result<Self::Output> {
        match self {
            Self::CM(v) => Ok(RosyValue::from(v.rosy_conj()?)),
            Self::CD(v) => Ok(RosyValue::from(v.rosy_conj()?)),
            Self::RE(v) => Ok(RosyValue::RE(*v)),
            other => bail!("CONJ not defined for {}", other.kind_name()),
        }
    }
}

impl RosyVMIN for RosyValue {
    fn rosy_vmin(&self) -> Result<RE> {
        match self {
            Self::RE(v) => v.rosy_vmin(),
            Self::VE(v) => v.rosy_vmin(),
            Self::Arr(v) => v
                .iter()
                .map(|x| x.as_f64())
                .reduce(f64::min)
                .ok_or_else(|| anyhow::anyhow!("VMIN of empty ARR")),
            other => bail!("VMIN not defined for {}", other.kind_name()),
        }
    }
}

impl RosyREConvert for RosyValue {
    fn rosy_re_convert(&self) -> Result<RE> {
        Ok(self.as_f64())
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

/// An operand of [`rosy_dyn_binary`]. Borrow where possible: PHAD reads
/// `LIST|J` on long lists inside loops, so a copy here is quadratic.
pub trait ToRosy {
    fn to_rosy(&self) -> Cow<'_, RosyValue>;
    /// Element `i` (1-based) of a list, without building the whole operand.
    /// `None` means the caller must take the full path.
    fn index_re(&self, _i: f64) -> Option<RosyValue> {
        None
    }
    /// `LIST|(a&b)` in bounds, without building the whole operand.
    fn slice_re(&self, _start: usize, _end: usize) -> Option<RosyValue> {
        None
    }
    /// The operand as one real, when it is exactly that.
    #[inline]
    fn as_scalar_re(&self) -> Option<f64> {
        None
    }
}

/// A 1-based inclusive range inside `len`, as `VE|VE` accepts it.
fn range_ok(start: usize, end: usize, len: usize) -> bool {
    start >= 1 && end >= start && end <= len
}
impl ToRosy for RosyValue {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Borrowed(self)
    }
    #[inline]
    fn as_scalar_re(&self) -> Option<f64> {
        match peel_singleton_arr(self) {
            RosyValue::RE(x) => Some(*x),
            _ => None,
        }
    }
    fn index_re(&self, i: f64) -> Option<RosyValue> {
        match self {
            RosyValue::VE(v) => v.index_re(i),
            // One element peels to that cell, so `DA|k` stays a coefficient.
            RosyValue::Arr(items) if items.len() > 1 => {
                items.get((i.round() as usize).wrapping_sub(1)).cloned()
            }
            _ => None,
        }
    }
    fn slice_re(&self, start: usize, end: usize) -> Option<RosyValue> {
        match self {
            RosyValue::VE(v) => v.slice_re(start, end),
            // A series in the range stays a list of series on the full path.
            // Only the range is checked: scanning the whole list made every
            // `snumbs|(a&b)` cost the length of `snumbs`.
            RosyValue::Arr(items)
                if items.len() > 1
                    && range_ok(start, end, items.len())
                    && !items[start - 1..end]
                        .iter()
                        .any(|x| matches!(x, RosyValue::DA(_) | RosyValue::CD(_))) =>
            {
                Some(RosyValue::VE(
                    items[start - 1..end].iter().map(|x| x.as_f64()).collect(),
                ))
            }
            _ => None,
        }
    }
}
impl ToRosy for f64 {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Owned(RosyValue::RE(*self))
    }
    #[inline]
    fn as_scalar_re(&self) -> Option<f64> {
        Some(*self)
    }
}
impl ToRosy for String {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Owned(RosyValue::ST(self.clone()))
    }
}
impl ToRosy for bool {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Owned(RosyValue::LO(*self))
    }
}
impl ToRosy for Vec<f64> {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Owned(RosyValue::VE(self.clone()))
    }
    fn index_re(&self, i: f64) -> Option<RosyValue> {
        self.get((i.round() as usize).wrapping_sub(1))
            .map(|x| RosyValue::RE(*x))
    }
    fn slice_re(&self, start: usize, end: usize) -> Option<RosyValue> {
        range_ok(start, end, self.len()).then(|| RosyValue::VE(self[start - 1..end].to_vec()))
    }
}
impl ToRosy for Vec<RosyValue> {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Owned(RosyValue::Arr(self.clone()))
    }
}
impl ToRosy for Vec<Vec<RosyValue>> {
    fn to_rosy(&self) -> Cow<'_, RosyValue> {
        Cow::Owned(RosyValue::from(self.clone()))
    }
}

fn peel_singleton_arr(v: &RosyValue) -> &RosyValue {
    match v {
        RosyValue::Arr(items) if items.len() == 1 => peel_singleton_arr(&items[0]),
        _ => v,
    }
}

fn arr_as_ve(v: &RosyValue) -> Cow<'_, RosyValue> {
    match v {
        RosyValue::Arr(items)
            if !items
                .iter()
                .any(|x| matches!(x, RosyValue::DA(_) | RosyValue::CD(_))) =>
        {
            Cow::Owned(RosyValue::VE(items.iter().map(|x| x.as_f64()).collect()))
        }
        other => Cow::Borrowed(other),
    }
}

fn cm_as_ve_if_other_ve<'a>(a: Cow<'a, RosyValue>, b: &RosyValue) -> Cow<'a, RosyValue> {
    match (a.as_ref(), b) {
        (RosyValue::CM(c), RosyValue::VE(_)) => Cow::Owned(RosyValue::VE(vec![c.re, c.im])),
        _ => a,
    }
}

fn as_da_list(v: &RosyValue) -> Option<Vec<DA>> {
    match v {
        RosyValue::DA(d) => Some(vec![d.clone()]),
        RosyValue::Arr(items)
            if !items.is_empty() && items.iter().all(|x| matches!(x, RosyValue::DA(_))) =>
        {
            Some(
                items
                    .iter()
                    .map(|x| match x {
                        RosyValue::DA(d) => d.clone(),
                        _ => unreachable!(),
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

fn as_cd_list(v: &RosyValue) -> Option<Vec<CD>> {
    match v {
        RosyValue::CD(d) => Some(vec![d.clone()]),
        RosyValue::Arr(items)
            if !items.is_empty() && items.iter().all(|x| matches!(x, RosyValue::CD(_))) =>
        {
            Some(
                items
                    .iter()
                    .map(|x| match x {
                        RosyValue::CD(d) => d.clone(),
                        _ => unreachable!(),
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// COSY compares the constant term. A boxed cell (`EMPTY#0`) still arrives here.
fn da_const_rel(op: BinaryOp, lhs: &RosyValue, rhs: &RosyValue) -> Option<bool> {
    if !matches!(
        op,
        BinaryOp::Eq | BinaryOp::Neq | BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Lte | BinaryOp::Gte
    ) {
        return None;
    }
    if !matches!(lhs, RosyValue::DA(_)) && !matches!(rhs, RosyValue::DA(_)) {
        return None;
    }
    let c = |v: &RosyValue| match v {
        RosyValue::RE(x) => Some(*x),
        RosyValue::DA(d) => Some(d.constant_part()),
        _ => None,
    };
    let (a, b) = (c(lhs)?, c(rhs)?);
    Some(match op {
        BinaryOp::Eq => a == b,
        BinaryOp::Neq => a != b,
        BinaryOp::Lt => a < b,
        BinaryOp::Gt => a > b,
        BinaryOp::Lte => a <= b,
        BinaryOp::Gte => a >= b,
        _ => return None,
    })
}

/// `X := X & CELL` for a list `X`, appended in place. A real or a list of
/// reals is the common case. Anything else takes the full path, so the
/// result and its errors are unchanged.
pub fn rosy_append_ve(dest: &mut Vec<f64>, rhs: &RosyValue) -> Result<()> {
    match peel_singleton_arr(rhs) {
        RosyValue::RE(x) => dest.push(*x),
        RosyValue::VE(v) => dest.extend_from_slice(v),
        RosyValue::Arr(items) if items.iter().all(|x| matches!(x, RosyValue::RE(_))) => {
            dest.extend(items.iter().map(|x| x.as_f64()))
        }
        _ => {
            let joined = rosy_dyn_binary(BinaryOp::Concat, &*dest, rhs)?.expect_ve()?;
            *dest = joined;
        }
    }
    Ok(())
}

pub fn rosy_dyn_binary(op: BinaryOp, lhs: &impl ToRosy, rhs: &impl ToRosy) -> Result<RosyValue> {
    // Two reals: the same f64 result the RE,RE arms give, without the
    // dispatch. Untyped fox index cells (`Y(IINDEX+4)`) land here.
    if let (Some(a), Some(b)) = (lhs.as_scalar_re(), rhs.as_scalar_re()) {
        match op {
            BinaryOp::Add => return Ok(RosyValue::RE(a + b)),
            BinaryOp::Sub => return Ok(RosyValue::RE(a - b)),
            BinaryOp::Mult => return Ok(RosyValue::RE(a * b)),
            BinaryOp::Div => return Ok(RosyValue::RE(a / b)),
            BinaryOp::Eq => return Ok(RosyValue::LO(a == b)),
            BinaryOp::Neq => return Ok(RosyValue::LO(a != b)),
            BinaryOp::Lt => return Ok(RosyValue::LO(a < b)),
            BinaryOp::Gt => return Ok(RosyValue::LO(a > b)),
            BinaryOp::Lte => return Ok(RosyValue::LO(a <= b)),
            BinaryOp::Gte => return Ok(RosyValue::LO(a >= b)),
            _ => {}
        }
    }
    if op == BinaryOp::Extract {
        let index = rhs.to_rosy();
        match peel_singleton_arr(index.as_ref()) {
            RosyValue::RE(i) => {
                if let Some(found) = lhs.index_re(*i) {
                    return Ok(found);
                }
            }
            RosyValue::VE(r) if r.len() == 2 => {
                let (a, b) = (r[0].round() as usize, r[1].round() as usize);
                if let Some(found) = lhs.slice_re(a, b) {
                    return Ok(found);
                }
            }
            RosyValue::Arr(r)
                if r.len() == 2 && r.iter().all(|x| matches!(x, RosyValue::RE(_))) =>
            {
                let (a, b) = (
                    r[0].as_f64().round() as usize,
                    r[1].as_f64().round() as usize,
                );
                if let Some(found) = lhs.slice_re(a, b) {
                    return Ok(found);
                }
            }
            _ => {}
        }
    }
    let lhs = lhs.to_rosy();
    let rhs = rhs.to_rosy();
    let lhs = peel_singleton_arr(lhs.as_ref());
    let rhs = peel_singleton_arr(rhs.as_ref());
    if let Some(lo) = da_const_rel(op, lhs, rhs) {
        return Ok(RosyValue::LO(lo));
    }
    match op {
        BinaryOp::Add => {
            let (lhs, rhs) = (arr_as_ve(lhs), arr_as_ve(rhs));
            let lhs = cm_as_ve_if_other_ve(lhs, &rhs);
            let rhs = cm_as_ve_if_other_ve(rhs, &lhs);
            dyn_pairs!(
                lhs.as_ref(), rhs.as_ref(), rosy_add,
                RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
                CM, RE; CM, CM; CM, DA; CM, CD;
                VE, RE; VE, VE;
                DA, RE; DA, CM; DA, DA; DA, CD;
                CD, RE; CD, CM; CD, DA; CD, CD;
                LO, LO
            )
        }
        BinaryOp::Sub => {
            let (lhs, rhs) = (arr_as_ve(lhs), arr_as_ve(rhs));
            dyn_pairs!(
                lhs.as_ref(), rhs.as_ref(), rosy_sub,
                RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
                CM, RE; CM, CM; CM, DA; CM, CD;
                VE, RE; VE, VE;
                DA, RE; DA, CM; DA, DA; DA, CD;
                CD, RE; CD, CM; CD, DA; CD, CD
            )
        }
        BinaryOp::Mult => {
            let (lhs, rhs) = (arr_as_ve(lhs), arr_as_ve(rhs));
            let lhs = cm_as_ve_if_other_ve(lhs, &rhs);
            let rhs = cm_as_ve_if_other_ve(rhs, &lhs);
            dyn_pairs!(
                lhs.as_ref(), rhs.as_ref(), rosy_mult,
                RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
                CM, RE; CM, CM; CM, DA; CM, CD;
                VE, RE; VE, VE; VE, DA;
                DA, RE; DA, CM; DA, DA; DA, CD; DA, VE;
                CD, RE; CD, CM; CD, DA; CD, CD;
                LO, LO
            )
        }
        BinaryOp::Div => {
            let (lhs, rhs) = (arr_as_ve(lhs), arr_as_ve(rhs));
            dyn_pairs!(
                lhs.as_ref(), rhs.as_ref(), rosy_div,
                RE, RE; RE, CM; RE, VE; RE, DA; RE, CD;
                CM, RE; CM, CM; CM, DA; CM, CD;
                VE, RE; VE, VE;
                DA, RE; DA, CM; DA, DA; DA, CD;
                CD, RE; CD, CM; CD, DA; CD, CD
            )
        }
        BinaryOp::Pow => {
            let (lhs, rhs) = (arr_as_ve(lhs), arr_as_ve(rhs));
            dyn_pairs!(lhs.as_ref(), rhs.as_ref(), rosy_pow, RE, RE; VE, RE; DA, RE; CD, RE)
        }
        BinaryOp::Extract => {
            if let (RosyValue::Arr(items), RosyValue::RE(i)) = (lhs, rhs) {
                let idx = i.round() as usize;
                if idx < 1 || idx > items.len() {
                    bail!(
                        "extract index {idx} out of bounds for ARR of {}",
                        items.len()
                    );
                }
                return Ok(items[idx - 1].clone());
            }
            // `TIMEBINS|(2&LENGTH)` is a range. An Arr that holds a series
            // must stay a series. Flattening it to f64 is what makes this bail.
            if let RosyValue::Arr(items) = lhs
                && items
                    .iter()
                    .any(|x| matches!(x, RosyValue::DA(_) | RosyValue::CD(_)))
            {
                let idx: Vec<f64> = match rhs {
                    RosyValue::VE(v) => v.clone(),
                    RosyValue::Arr(a) => a.iter().map(|x| x.as_f64()).collect(),
                    _ => bail!("rosy_extract not defined for ARR and {}", rhs.kind_name()),
                };
                if idx.len() != 2 {
                    bail!(
                        "Vector extraction with vector index requires exactly two elements (start and end)"
                    );
                }
                let start = idx[0].round() as usize;
                let end = idx[1].round() as usize;
                if start == 0 || end == 0 || start > items.len() || end > items.len() || start > end
                {
                    bail!(
                        "Vector index range {}-{} out of bounds (1-{})",
                        start,
                        end,
                        items.len()
                    );
                }
                return Ok(RosyValue::Arr(items[start - 1..end].to_vec()));
            }
            let rhs = arr_as_ve(rhs);
            let lhs = arr_as_ve(lhs);
            dyn_pairs!(
                lhs.as_ref(), rhs.as_ref(), rosy_extract,
                ST, RE; ST, VE;
                CM, RE;
                VE, RE; VE, VE;
                DA, RE; DA, VE;
                CD, RE; CD, VE;
                RE, RE; RE, VE
            )
        }
        BinaryOp::Concat => {
            if matches!(lhs, RosyValue::ST(_)) || matches!(rhs, RosyValue::ST(_)) {
                return Ok(RosyValue::ST(format!(
                    "{}{}",
                    lhs.rosy_display(),
                    rhs.rosy_display()
                )));
            }
            if let (Some(a), Some(b)) = (as_da_list(lhs), as_da_list(rhs)) {
                return Ok(RosyValue::Arr(
                    a.into_iter().chain(b).map(RosyValue::DA).collect(),
                ));
            }
            if let (Some(a), RosyValue::RE(x)) = (as_da_list(lhs), rhs) {
                let mut out = a;
                out.push(DA::constant(*x));
                return Ok(RosyValue::Arr(out.into_iter().map(RosyValue::DA).collect()));
            }
            if let (RosyValue::RE(x), Some(b)) = (lhs, as_da_list(rhs)) {
                let mut out = vec![DA::constant(*x)];
                out.extend(b);
                return Ok(RosyValue::Arr(out.into_iter().map(RosyValue::DA).collect()));
            }
            if let (Some(a), Some(b)) = (as_cd_list(lhs), as_cd_list(rhs)) {
                return Ok(RosyValue::Arr(
                    a.into_iter().chain(b).map(RosyValue::CD).collect(),
                ));
            }
            if matches!(lhs, RosyValue::Arr(_)) || matches!(rhs, RosyValue::Arr(_)) {
                let mut a = lhs.clone().expect_arr()?;
                a.extend(rhs.clone().expect_arr()?);
                return Ok(RosyValue::Arr(a));
            }
            dyn_pairs!(
                lhs, rhs, rosy_concat,
                RE, RE; RE, VE;
                ST, ST;
                VE, RE; VE, VE
            )
        }
        BinaryOp::Eq => {
            if matches!(lhs, RosyValue::Arr(_)) && matches!(rhs, RosyValue::Arr(_)) {
                match (arr_as_ve(lhs).as_ref(), arr_as_ve(rhs).as_ref()) {
                    (RosyValue::VE(a), RosyValue::VE(b)) => return Ok(RosyValue::LO(a == b)),
                    _ => {}
                }
            }
            dyn_pairs!(lhs, rhs, rosy_eq, RE, RE; ST, ST; LO, LO; RE, ST; ST, RE)
        }
        BinaryOp::Neq => {
            if matches!(lhs, RosyValue::Arr(_)) && matches!(rhs, RosyValue::Arr(_)) {
                match (arr_as_ve(lhs).as_ref(), arr_as_ve(rhs).as_ref()) {
                    (RosyValue::VE(a), RosyValue::VE(b)) => return Ok(RosyValue::LO(a != b)),
                    _ => {}
                }
            }
            match (lhs, rhs) {
                (RosyValue::RE(a), RosyValue::ST(b)) => {
                    return Ok(RosyValue::LO(!a.rosy_eq(b)?));
                }
                (RosyValue::ST(a), RosyValue::RE(b)) => {
                    return Ok(RosyValue::LO(!a.rosy_eq(b)?));
                }
                _ => {}
            }
            dyn_pairs!(lhs, rhs, rosy_neq, RE, RE; ST, ST; LO, LO)
        }
        BinaryOp::Lt => dyn_pairs!(lhs, rhs, rosy_lt, RE, RE; ST, ST),
        BinaryOp::Gt => dyn_pairs!(lhs, rhs, rosy_gt, RE, RE; ST, ST),
        BinaryOp::Lte => dyn_pairs!(lhs, rhs, rosy_lte, RE, RE; ST, ST),
        BinaryOp::Gte => dyn_pairs!(lhs, rhs, rosy_gte, RE, RE; ST, ST),
        BinaryOp::And => match (lhs, rhs) {
            (RosyValue::LO(a), RosyValue::LO(b)) => Ok(RosyValue::from(a.rosy_and(b)?)),
            (l, r) => Ok(RosyValue::LO(l.as_f64() != 0.0 && r.as_f64() != 0.0)),
        },
        BinaryOp::Or => match (lhs, rhs) {
            (RosyValue::LO(a), RosyValue::LO(b)) => Ok(RosyValue::from(a.rosy_or(b)?)),
            (l, r) => Ok(RosyValue::LO(l.as_f64() != 0.0 || r.as_f64() != 0.0)),
        },
        BinaryOp::Derive => match (lhs, rhs) {
            (RosyValue::DA(a), RosyValue::RE(i)) => Ok(RosyValue::DA(a.rosy_derive(*i as i64)?)),
            (RosyValue::CD(a), RosyValue::RE(i)) => Ok(RosyValue::CD(a.rosy_derive(*i as i64)?)),
            (RosyValue::Arr(items), RosyValue::RE(i)) => {
                let out: Result<Vec<_>> = items
                    .iter()
                    .map(|cell| match cell {
                        RosyValue::DA(a) => Ok(RosyValue::DA(a.rosy_derive(*i as i64)?)),
                        RosyValue::CD(a) => Ok(RosyValue::CD(a.rosy_derive(*i as i64)?)),
                        _ => Ok(RosyValue::RE(0.0)),
                    })
                    .collect();
                Ok(RosyValue::Arr(out?))
            }
            (RosyValue::RE(_), _) => Ok(RosyValue::RE(0.0)),
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
        assert_eq!(RosyValue::RE(4.0).expect_ve().unwrap(), vec![4.0]);
        assert!(RosyValue::RE(1.0).expect_lo().unwrap());
        assert_eq!(RosyValue::RE(2.0).expect_cm().unwrap().re, 2.0);
        assert_eq!(RosyValue::RE(3.0).expect_st().unwrap().is_empty(), false);
    }

    #[test]
    fn concat_arr_and_re() {
        let a = RosyValue::from(vec![1.0, 2.0]);
        let b = RosyValue::RE(3.0);
        let c = rosy_dyn_binary(BinaryOp::Concat, &a, &b).unwrap();
        assert_eq!(c.expect_ve().unwrap(), vec![1.0, 2.0, 3.0]);
        let d = rosy_dyn_binary(BinaryOp::Add, &a, &b).unwrap();
        assert_eq!(d.expect_ve().unwrap(), vec![4.0, 5.0]);
    }

    #[test]
    #[serial_test::serial]
    fn dyn_compare_uses_da_constant_term() {
        // Reals stay on the old arms. No Taylor runtime required.
        let lo = |v: RosyValue| matches!(v, RosyValue::LO(true));
        assert!(lo(rosy_dyn_binary(
            BinaryOp::Neq,
            &RosyValue::RE(1.0),
            &RosyValue::RE(0.0)
        )
        .unwrap()));
        assert!(!lo(rosy_dyn_binary(
            BinaryOp::Neq,
            &RosyValue::RE(1.0),
            &RosyValue::RE(1.0)
        )
        .unwrap()));
        assert!(lo(rosy_dyn_binary(
            BinaryOp::Neq,
            &RosyValue::ST("a".into()),
            &RosyValue::ST("b".into())
        )
        .unwrap()));

        crate::taylor::config::cleanup_taylor();
        crate::taylor::config::init_taylor(2, 1).unwrap();
        crate::taylor::config::set_epsilon(1e-100).unwrap();

        let one = RosyValue::DA(DA::from_coeff(1.0));
        let zero = RosyValue::RE(0.0);
        let tiny = RosyValue::DA(DA::from_coeff(9.44e-16));
        // Constant 0, plus a linear term. COSY still treats this as 0.
        let slope = RosyValue::DA(DA::variable(1).unwrap());
        let wrapped = RosyValue::Arr(vec![one.clone()]);

        assert!(lo(rosy_dyn_binary(BinaryOp::Neq, &one, &zero).unwrap()));
        assert!(lo(rosy_dyn_binary(BinaryOp::Neq, &zero, &one).unwrap()));
        assert!(!lo(rosy_dyn_binary(BinaryOp::Eq, &one, &zero).unwrap()));
        assert!(lo(
            rosy_dyn_binary(BinaryOp::Eq, &one, &RosyValue::RE(1.0)).unwrap()
        ));
        assert!(lo(rosy_dyn_binary(BinaryOp::Lt, &zero, &one).unwrap()));
        assert!(lo(rosy_dyn_binary(BinaryOp::Gt, &one, &zero).unwrap()));
        assert!(lo(rosy_dyn_binary(
            BinaryOp::Lte,
            &one,
            &RosyValue::RE(1.0)
        )
        .unwrap()));
        assert!(lo(rosy_dyn_binary(
            BinaryOp::Gte,
            &RosyValue::RE(1.0),
            &one
        )
        .unwrap()));
        assert!(lo(rosy_dyn_binary(BinaryOp::Neq, &tiny, &zero).unwrap()));
        assert!(!lo(rosy_dyn_binary(BinaryOp::Neq, &slope, &zero).unwrap()));
        assert!(lo(rosy_dyn_binary(BinaryOp::Eq, &slope, &zero).unwrap()));
        assert!(lo(rosy_dyn_binary(BinaryOp::Neq, &one, &slope).unwrap()));
        // `EMPTY := PARTICLES` when the vector has one series.
        assert!(lo(rosy_dyn_binary(BinaryOp::Neq, &wrapped, &zero).unwrap()));

        crate::taylor::config::cleanup_taylor();
    }

    #[test]
    #[serial_test::serial]
    fn dyn_extract_slices_an_arr_of_da() {
        crate::taylor::config::cleanup_taylor();
        crate::taylor::config::init_taylor(2, 1).unwrap();
        let x = DA::variable(1).unwrap();
        let timed = (&DA::from_coeff(1.5) + &x).unwrap();
        assert!(timed.num_terms() > 1);
        let bins = RosyValue::Arr(vec![
            RosyValue::DA(DA::from_coeff(0.0)),
            RosyValue::DA(timed),
        ]);
        // SHORTLIST(TIMEBINS, 1) drops the leading placeholder.
        let got =
            rosy_dyn_binary(BinaryOp::Extract, &bins, &RosyValue::VE(vec![2.0, 2.0])).unwrap();
        let RosyValue::Arr(items) = got else {
            panic!("range of a series list should stay an ARR");
        };
        assert_eq!(items.len(), 1);
        let RosyValue::DA(d) = &items[0] else {
            panic!("sliced element should stay a DA");
        };
        assert!((d.constant_part() - 1.5).abs() < 1e-12);
        assert!(d.num_terms() > 1);

        let reals = RosyValue::Arr(vec![
            RosyValue::RE(1.0),
            RosyValue::RE(2.0),
            RosyValue::RE(3.0),
        ]);
        let sliced =
            rosy_dyn_binary(BinaryOp::Extract, &reals, &RosyValue::VE(vec![1.0, 2.0])).unwrap();
        assert_eq!(sliced.expect_ve().unwrap(), vec![1.0, 2.0]);
        crate::taylor::config::cleanup_taylor();
    }
}

/// The borrowing fast paths must answer exactly as the full path did.
#[cfg(test)]
mod fast_path_tests {
    use super::*;

    fn re(x: f64) -> RosyValue {
        RosyValue::RE(x)
    }

    #[test]
    fn element_reads_match_full_path() {
        let ve = vec![1.0, 2.0, 3.0];
        let arr = RosyValue::Arr(vec![re(4.0), re(5.0), re(6.0)]);
        let cell = RosyValue::VE(ve.clone());
        for i in [1.0, 2.0, 3.0, 2.4, 2.6] {
            let want = RosyExtract::rosy_extract(&ve, &(i as f64)).unwrap();
            assert_eq!(
                rosy_dyn_binary(BinaryOp::Extract, &ve, &i).unwrap(),
                re(want)
            );
            assert_eq!(
                rosy_dyn_binary(BinaryOp::Extract, &cell, &re(i)).unwrap(),
                re(want)
            );
        }
        assert_eq!(
            rosy_dyn_binary(BinaryOp::Extract, &arr, &re(2.0)).unwrap(),
            re(5.0)
        );
        // Out of range still takes the full path, and still fails there.
        assert!(rosy_dyn_binary(BinaryOp::Extract, &ve, &4.0).is_err());
        assert!(rosy_dyn_binary(BinaryOp::Extract, &arr, &re(0.0)).is_err());
    }

    #[test]
    fn range_reads_match_full_path() {
        let ve = vec![1.0, 2.0, 3.0, 4.0];
        let cell = RosyValue::VE(ve.clone());
        let arr = RosyValue::Arr(ve.iter().map(|x| re(*x)).collect());
        let range = RosyValue::VE(vec![2.0, 3.0]);
        let want = RosyValue::VE(vec![2.0, 3.0]);
        assert_eq!(
            rosy_dyn_binary(BinaryOp::Extract, &ve, &range).unwrap(),
            want
        );
        assert_eq!(
            rosy_dyn_binary(BinaryOp::Extract, &cell, &range).unwrap(),
            want
        );
        assert_eq!(
            rosy_dyn_binary(BinaryOp::Extract, &arr, &range).unwrap(),
            want
        );
        let boxed = RosyValue::Arr(vec![re(2.0), re(3.0)]);
        assert_eq!(
            rosy_dyn_binary(BinaryOp::Extract, &cell, &boxed).unwrap(),
            want
        );
        assert!(rosy_dyn_binary(BinaryOp::Extract, &ve, &RosyValue::VE(vec![3.0, 5.0])).is_err());
        assert!(rosy_dyn_binary(BinaryOp::Extract, &ve, &RosyValue::VE(vec![3.0, 2.0])).is_err());
    }

    #[test]
    #[serial_test::serial]
    fn singleton_series_cell_stays_a_coefficient() {
        crate::taylor::config::cleanup_taylor();
        crate::taylor::config::init_taylor(3, 1).unwrap();
        let x = DA::variable(1).unwrap();
        let s = (&(&x * &x).unwrap() + 2.0).unwrap();
        let one = RosyValue::Arr(vec![RosyValue::DA(s)]);
        // `CELL|1` on a one-element cell is the order-1 coefficient, not the cell.
        let got = rosy_dyn_binary(BinaryOp::Extract, &one, &re(2.0)).unwrap();
        assert_eq!(got.as_f64(), 1.0);
        crate::taylor::config::cleanup_taylor();
    }
}
