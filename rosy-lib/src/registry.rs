//! Central lookup for operators and unary/binary intrinsics.
//!
//! Per-operator / per-intrinsic modules still own the actual type tables and
//! trait impls. This module is the name → type-rule / emit-name map the
//! compiler should read instead of hard-coding `operators::add::get_return_type`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::RosyType;
use crate::intrinsics;
use crate::operators;

/// Binary operator as the compiler and type-inference pass see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mult,
    Div,
    Pow,
    Extract,
    Concat,
    Derive,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
}

impl BinaryOp {
    /// Result type of `lhs op rhs`, or `None` if the pair is illegal.
    pub fn return_type(self, lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
        match self {
            Self::Add => operators::add::get_return_type(lhs, rhs),
            Self::Sub => operators::sub::get_return_type(lhs, rhs),
            Self::Mult => operators::mult::get_return_type(lhs, rhs),
            Self::Div => operators::div::get_return_type(lhs, rhs),
            Self::Pow => operators::pow::get_return_type(lhs, rhs),
            Self::Extract => operators::extract::get_return_type(lhs, rhs),
            Self::Concat => operators::concat::get_return_type(lhs, rhs),
            Self::Eq => operators::eq::get_return_type(lhs, rhs),
            Self::Neq => operators::neq::get_return_type(lhs, rhs),
            Self::Lt => operators::lt::get_return_type(lhs, rhs),
            Self::Gt => operators::gt::get_return_type(lhs, rhs),
            Self::Lte => operators::lte::get_return_type(lhs, rhs),
            Self::Gte => operators::gte::get_return_type(lhs, rhs),
            Self::And => operators::and::get_return_type(lhs, rhs),
            Self::Or => operators::or::get_return_type(lhs, rhs),
            Self::Derive => match *lhs {
                t if t == RosyType::DA() => Some(RosyType::DA()),
                t if t == RosyType::CD() => Some(RosyType::CD()),
                _ => None,
            },
        }
    }

    /// Trait method the transpiler emits, e.g. `RosyAdd::rosy_add`.
    pub fn rust_call(self) -> &'static str {
        match self {
            Self::Add => "RosyAdd::rosy_add",
            Self::Sub => "RosySub::rosy_sub",
            Self::Mult => "RosyMult::rosy_mult",
            Self::Div => "RosyDiv::rosy_div",
            Self::Pow => "RosyPow::rosy_pow",
            Self::Extract => "RosyExtract::rosy_extract",
            Self::Concat => "RosyConcat::rosy_concat",
            Self::Eq => "RosyEq::rosy_eq",
            Self::Neq => "RosyNeq::rosy_neq",
            Self::Lt => "RosyLt::rosy_lt",
            Self::Gt => "RosyGt::rosy_gt",
            Self::Lte => "RosyLte::rosy_lte",
            Self::Gte => "RosyGte::rosy_gte",
            Self::And => "RosyAnd::rosy_and",
            Self::Or => "RosyOr::rosy_or",
            Self::Derive => "RosyDerive::rosy_derive",
        }
    }
}

/// Prefix unary operators (`NOT`, unary `-`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Not,
    Neg,
}

impl UnaryOp {
    pub fn return_type(self, operand: &RosyType) -> Option<RosyType> {
        match self {
            Self::Not => operators::not::get_return_type(operand),
            // Unary minus is typed as `0 - operand`.
            Self::Neg => operators::sub::get_return_type(&RosyType::RE(), operand),
        }
    }

    pub fn rust_call(self) -> &'static str {
        match self {
            Self::Not => "RosyNot::rosy_not",
            Self::Neg => "RosySub::rosy_sub",
        }
    }
}

/// How an intrinsic's result type is computed.
#[derive(Clone, Copy)]
pub enum IntrinsicTyping {
    Unary(fn(&RosyType) -> Option<RosyType>),
    Binary(fn(&RosyType, &RosyType) -> Option<RosyType>),
}

/// One named intrinsic (SIN, ST, POSITION, …).
#[derive(Clone, Copy)]
pub struct Intrinsic {
    pub name: &'static str,
    pub arity: usize,
    pub rust_call: &'static str,
    /// If `Some`, RE arguments emit `x` plus this suffix (e.g. `.sin()`, `.powi(2)`).
    pub native_re: Option<&'static str>,
    /// Whether the trait call returns `Result` (emit `?`).
    pub fallible: bool,
    pub typing: IntrinsicTyping,
}

impl Intrinsic {
    pub fn unary_return_type(self, input: &RosyType) -> Option<RosyType> {
        match self.typing {
            IntrinsicTyping::Unary(f) => f(input),
            IntrinsicTyping::Binary(_) => None,
        }
    }

    pub fn binary_return_type(self, lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
        match self.typing {
            IntrinsicTyping::Binary(f) => f(lhs, rhs),
            IntrinsicTyping::Unary(_) => None,
        }
    }
}

macro_rules! unary {
    ($name:literal, $arity:literal, $call:literal, $ty:path) => {
        unary!($name, $arity, $call, $ty, None, true)
    };
    ($name:literal, $arity:literal, $call:literal, $ty:path, $native:expr) => {
        unary!($name, $arity, $call, $ty, $native, true)
    };
    ($name:literal, $arity:literal, $call:literal, $ty:path, $native:expr, $fallible:expr) => {
        Intrinsic {
            name: $name,
            arity: $arity,
            rust_call: $call,
            native_re: $native,
            fallible: $fallible,
            typing: IntrinsicTyping::Unary($ty),
        }
    };
}

macro_rules! binary {
    ($name:literal, $arity:literal, $call:literal, $ty:path) => {
        binary!($name, $arity, $call, $ty, true)
    };
    ($name:literal, $arity:literal, $call:literal, $ty:path, $fallible:expr) => {
        Intrinsic {
            name: $name,
            arity: $arity,
            rust_call: $call,
            native_re: None,
            fallible: $fallible,
            typing: IntrinsicTyping::Binary($ty),
        }
    };
}

/// All named expression-level intrinsics. Source of truth for lookup by name.
pub static INTRINSICS: &[Intrinsic] = &[
    unary!(
        "SIN",
        1,
        "RosySIN::rosy_sin",
        intrinsics::sin::get_return_type,
        Some(".sin()")
    ),
    unary!(
        "COS",
        1,
        "RosyCOS::rosy_cos",
        intrinsics::cos::get_return_type,
        Some(".cos()")
    ),
    unary!(
        "TAN",
        1,
        "RosyTAN::rosy_tan",
        intrinsics::tan::get_return_type,
        Some(".tan()")
    ),
    unary!(
        "ASIN",
        1,
        "RosyASIN::rosy_asin",
        intrinsics::asin::get_return_type,
        Some(".asin()")
    ),
    unary!(
        "ACOS",
        1,
        "RosyACOS::rosy_acos",
        intrinsics::acos::get_return_type,
        Some(".acos()")
    ),
    unary!(
        "ATAN",
        1,
        "RosyATAN::rosy_atan",
        intrinsics::atan::get_return_type,
        Some(".atan()")
    ),
    unary!(
        "SINH",
        1,
        "RosySINH::rosy_sinh",
        intrinsics::sinh::get_return_type,
        Some(".sinh()")
    ),
    unary!(
        "COSH",
        1,
        "RosyCOSH::rosy_cosh",
        intrinsics::cosh::get_return_type,
        Some(".cosh()")
    ),
    unary!(
        "TANH",
        1,
        "RosyTANH::rosy_tanh",
        intrinsics::tanh::get_return_type,
        Some(".tanh()")
    ),
    unary!(
        "SQR",
        1,
        "RosySQR::rosy_sqr",
        intrinsics::sqr::get_return_type,
        Some(".powi(2)")
    ),
    unary!(
        "SQRT",
        1,
        "RosySQRT::rosy_sqrt",
        intrinsics::sqrt::get_return_type,
        Some(".sqrt()")
    ),
    unary!(
        "EXP",
        1,
        "RosyEXP::rosy_exp",
        intrinsics::exp::get_return_type,
        Some(".exp()")
    ),
    unary!(
        "LOG",
        1,
        "RosyLOG::rosy_log",
        intrinsics::log::get_return_type,
        Some(".ln()")
    ),
    unary!(
        "ABS",
        1,
        "RosyABS::rosy_abs",
        intrinsics::abs::get_return_type,
        Some(".abs()")
    ),
    unary!(
        "NORM",
        1,
        "RosyNORM::rosy_norm",
        intrinsics::norm::get_return_type
    ),
    unary!(
        "CONS",
        1,
        "RosyCONS::rosy_cons",
        intrinsics::cons::get_return_type
    ),
    unary!(
        "INT",
        1,
        "RosyINT::rosy_int",
        intrinsics::int_fn::get_return_type,
        Some(".trunc()")
    ),
    unary!(
        "NINT",
        1,
        "RosyNINT::rosy_nint",
        intrinsics::nint::get_return_type,
        Some(".round()")
    ),
    unary!(
        "TYPE",
        1,
        "RosyTYPE::rosy_type",
        intrinsics::type_fn::get_return_type
    ),
    unary!(
        "TRIM",
        1,
        "RosyTRIM::rosy_trim",
        intrinsics::trim::get_return_type
    ),
    unary!(
        "LTRIM",
        1,
        "RosyLTRIM::rosy_ltrim",
        intrinsics::ltrim::get_return_type
    ),
    unary!(
        "ISRT",
        1,
        "RosyISRT::rosy_isrt",
        intrinsics::isrt::get_return_type
    ),
    unary!(
        "ISRT3",
        1,
        "RosyISRT3::rosy_isrt3",
        intrinsics::isrt3::get_return_type
    ),
    unary!("CM", 1, "RosyCM::rosy_cm", intrinsics::cm::get_return_type),
    unary!(
        "ST",
        1,
        "RosyST::rosy_to_string",
        intrinsics::st::get_return_type,
        None,
        false
    ),
    unary!(
        "LO",
        1,
        "RosyLO::rosy_to_logical",
        intrinsics::lo::get_return_type,
        None,
        false
    ),
    unary!(
        "RE",
        1,
        "RosyREConvert::rosy_re_convert",
        intrinsics::re_convert::get_return_type
    ),
    unary!(
        "VE",
        1,
        "RosyVEConvert::rosy_ve_convert",
        intrinsics::ve_convert::get_return_type
    ),
    unary!(
        "LENGTH",
        1,
        "RosyLENGTH::rosy_length",
        intrinsics::length::get_return_type,
        None,
        false
    ),
    unary!(
        "VARMEM",
        1,
        "RosyVARMEM::rosy_varmem",
        intrinsics::varmem::get_return_type,
        None,
        false
    ),
    unary!(
        "VARPOI",
        1,
        "RosyVARPOI::rosy_varpoi",
        intrinsics::varpoi::get_return_type,
        None,
        false
    ),
    unary!(
        "ERF",
        1,
        "RosyERF::rosy_erf",
        intrinsics::erf::get_return_type
    ),
    unary!(
        "WERF",
        1,
        "RosyWERF::rosy_werf",
        intrinsics::werf::get_return_type
    ),
    unary!(
        "REAL",
        1,
        "RosyREAL::rosy_real",
        intrinsics::real_fn::get_return_type
    ),
    unary!(
        "IMAG",
        1,
        "RosyIMAG::rosy_imag",
        intrinsics::imag_fn::get_return_type
    ),
    unary!(
        "CONJ",
        1,
        "RosyCONJ::rosy_conj",
        intrinsics::conj::get_return_type
    ),
    unary!(
        "CMPLX",
        1,
        "RosyCMPLX::rosy_cmplx",
        intrinsics::cmplx::get_return_type
    ),
    unary!(
        "VMAX",
        1,
        "RosyVMAX::rosy_vmax",
        intrinsics::vmax::get_return_type
    ),
    unary!(
        "VMIN",
        1,
        "RosyVMIN::rosy_vmin",
        intrinsics::vmin::get_return_type
    ),
    binary!(
        "POSITION",
        2,
        "RosyPOSITION::rosy_position",
        intrinsics::position::get_return_type,
        false
    ),
    unary!(
        "LST",
        1,
        "RosyLST::rosy_lst",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
    unary!(
        "LCM",
        1,
        "RosyLCM::rosy_lcm",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
    unary!(
        "LCD",
        1,
        "RosyLCD::rosy_lcd",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
    unary!(
        "LRE",
        1,
        "RosyLRE::rosy_lre",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
    unary!(
        "LLO",
        1,
        "RosyLLO::rosy_llo",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
    unary!(
        "LVE",
        1,
        "RosyLVE::rosy_lve",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
    unary!(
        "LDA",
        1,
        "RosyLDA::rosy_lda",
        intrinsics::mem_size::always_re,
        None,
        false
    ),
];

fn intrinsic_map() -> &'static HashMap<&'static str, &'static Intrinsic> {
    static MAP: OnceLock<HashMap<&'static str, &'static Intrinsic>> = OnceLock::new();
    MAP.get_or_init(|| INTRINSICS.iter().map(|i| (i.name, i)).collect())
}

/// Look up an intrinsic by its ROSY name (`"SIN"`, `"ST"`, …). Case-sensitive.
pub fn lookup_intrinsic(name: &str) -> Option<&'static Intrinsic> {
    intrinsic_map().get(name).copied()
}

/// Convenience: unary result type, or `None` if unknown / not unary / illegal input.
pub fn unary_return_type(name: &str, input: &RosyType) -> Option<RosyType> {
    lookup_intrinsic(name)?.unary_return_type(input)
}

/// Convenience: binary result type.
pub fn binary_return_type(name: &str, lhs: &RosyType, rhs: &RosyType) -> Option<RosyType> {
    lookup_intrinsic(name)?.binary_return_type(lhs, rhs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intrinsic_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for i in INTRINSICS {
            assert!(seen.insert(i.name), "duplicate intrinsic {}", i.name);
        }
    }

    #[test]
    fn lookup_is_case_sensitive() {
        assert!(lookup_intrinsic("SIN").is_some());
        assert!(lookup_intrinsic("sin").is_none());
    }

    #[test]
    fn sin_matches_module_table() {
        let t = RosyType::RE();
        assert_eq!(
            unary_return_type("SIN", &t),
            intrinsics::sin::get_return_type(&t)
        );
        assert_eq!(unary_return_type("SIN", &t), Some(RosyType::RE()));
        assert!(unary_return_type("SIN", &RosyType::ST()).is_none());
    }

    #[test]
    fn add_matches_module_table() {
        let re = RosyType::RE();
        assert_eq!(
            BinaryOp::Add.return_type(&re, &re),
            operators::add::get_return_type(&re, &re)
        );
    }

    #[test]
    fn position_is_binary() {
        let st = RosyType::ST();
        assert_eq!(
            binary_return_type("POSITION", &st, &st),
            Some(RosyType::RE())
        );
        assert!(unary_return_type("POSITION", &st).is_none());
    }
}
