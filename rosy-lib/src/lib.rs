#![cfg_attr(feature = "nightly-simd", feature(portable_simd))]

//! # Rosy Runtime Library
//!
//! The embedded runtime library that ships with every generated Rust project.
//! Contains operator implementations, intrinsic functions, type definitions,
//! MPI support, Taylor series (DA/CD), and the optimizer.
//!
//! ## Type Aliases
//!
//! | Rosy Type | Rust Type | Description |
//! |-----------|-----------|-------------|
//! | `RE` | `f64` | Real number |
//! | `ST` | `String` | String |
//! | `LO` | `bool` | Logical (boolean) |
//! | `CM` | `Complex64` | Complex number |
//! | `VE` | `Vec<f64>` | Vector of reals |
//! | `DA` | [`taylor::DA`] | Differential Algebra (Taylor series) |
//! | `CD` | [`taylor::CD`] | Complex Differential Algebra |
//!
//! ## Sub-modules
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`operators`] | Binary operator dispatch (add, sub, mult, div, etc.) |
//! | [`intrinsics`] | Built-in math functions (sin, sqr, exp, etc.) |
//! | [`core`] | Core I/O, file management, concatenation |
//! | [`taylor`] | DA/CD Taylor series implementation |
//! | `mpi` | MPI parallel context |
//! | [`optimizer`] | FIT loop optimization algorithms |

pub mod core;
pub mod value;
pub mod intrinsics;
#[cfg(feature = "mpi")]
pub mod mpi;
pub mod operators;
pub mod optimizer;
pub mod registry;
pub mod taylor;

pub use core::*;
pub use value::{RosyValue, rosy_dyn_binary};
pub use intrinsics::*;
#[cfg(feature = "mpi")]
pub use mpi::*;
pub use operators::*;
pub use registry::{
    BinaryOp, INTRINSICS, Intrinsic, IntrinsicTyping, UnaryOp, binary_return_type,
    lookup_intrinsic, unary_return_type,
};

pub use taylor::{CD, DA};

pub trait ExpectReExt {
    fn expect_re(&self) -> anyhow::Result<f64>;
}
impl ExpectReExt for f64 {
    fn expect_re(&self) -> anyhow::Result<f64> {
        Ok(*self)
    }
}

pub trait LenExt {
    fn len(&self) -> usize;
}
impl LenExt for f64 {
    fn len(&self) -> usize {
        1
    }
}

/// Anything the generated code might use as a real / index / cast source.
pub trait IntoF64 {
    fn into_f64(self) -> f64;
}
impl IntoF64 for f64 {
    fn into_f64(self) -> f64 {
        self
    }
}
impl IntoF64 for &f64 {
    fn into_f64(self) -> f64 {
        *self
    }
}
impl IntoF64 for RosyValue {
    fn into_f64(self) -> f64 {
        self.as_f64()
    }
}
impl IntoF64 for &RosyValue {
    fn into_f64(self) -> f64 {
        self.as_f64()
    }
}
impl IntoF64 for &mut RosyValue {
    fn into_f64(self) -> f64 {
        self.as_f64()
    }
}

pub trait AsF64 {
    fn as_f64_val(&self) -> f64;
}
impl AsF64 for f64 {
    fn as_f64_val(&self) -> f64 {
        *self
    }
}
impl AsF64 for RosyValue {
    fn as_f64_val(&self) -> f64 {
        self.as_f64()
    }
}
impl<T: AsF64 + ?Sized> AsF64 for &T {
    fn as_f64_val(&self) -> f64 {
        (**self).as_f64_val()
    }
}

#[inline]
pub fn rosy_as_usize(v: &impl AsF64) -> usize {
    v.as_f64_val() as usize
}
#[inline]
pub fn rosy_as_u32(v: &impl AsF64) -> u32 {
    v.as_f64_val() as u32
}
#[inline]
pub fn rosy_as_u64(v: &impl AsF64) -> u64 {
    v.as_f64_val() as u64
}
#[inline]
pub fn rosy_as_i64(v: &impl AsF64) -> i64 {
    v.as_f64_val() as i64
}
#[inline]
pub fn rosy_as_f64(v: &impl AsF64) -> f64 {
    v.as_f64_val()
}

pub trait RosyIndexable {
    type Out;
    fn rosy_index(&self, idx: usize, name: &str) -> &Self::Out;
}

impl<T> RosyIndexable for Vec<T> {
    type Out = T;
    fn rosy_index(&self, idx: usize, name: &str) -> &T {
        self.get(idx.wrapping_sub(1)).unwrap_or_else(|| {
            panic!(
                "Index {} into '{}' is out of bounds (1-{})",
                idx,
                name,
                self.len()
            )
        })
    }
}

impl<T> RosyIndexable for [T] {
    type Out = T;
    fn rosy_index(&self, idx: usize, name: &str) -> &T {
        self.get(idx.wrapping_sub(1)).unwrap_or_else(|| {
            panic!(
                "Index {} into '{}' is out of bounds (1-{})",
                idx,
                name,
                self.len()
            )
        })
    }
}

impl RosyIndexable for f64 {
    type Out = f64;
    fn rosy_index(&self, idx: usize, name: &str) -> &f64 {
        if idx != 1 {
            panic!("Index {idx} into scalar '{name}'");
        }
        self
    }
}

impl<T: RosyIndexable + ?Sized> RosyIndexable for &T {
    type Out = T::Out;
    fn rosy_index(&self, idx: usize, name: &str) -> &T::Out {
        (**self).rosy_index(idx, name)
    }
}

impl<T: RosyIndexable + ?Sized> RosyIndexable for &mut T {
    type Out = T::Out;
    fn rosy_index(&self, idx: usize, name: &str) -> &T::Out {
        (**self).rosy_index(idx, name)
    }
}

impl RosyIndexable for RosyValue {
    type Out = f64;
    fn rosy_index(&self, idx: usize, name: &str) -> &f64 {
        match self {
            RosyValue::VE(v) => v.rosy_index(idx, name),
            RosyValue::RE(v) if idx == 1 => v,
            other => panic!(
                "cannot index {} '{}' at {}",
                other.kind_name(),
                name,
                idx
            ),
        }
    }
}

#[inline(always)]
pub fn rosy_get<'a, C: RosyIndexable + ?Sized>(
    container: &'a C,
    one_based: impl IntoF64,
    var_name: &str,
) -> &'a C::Out {
    container.rosy_index(one_based.into_f64().round() as usize, var_name)
}

pub trait RosyMutIndexable {
    type Out: Default;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut Self::Out;
}

impl<T: Default> RosyMutIndexable for Vec<T> {
    type Out = T;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut T {
        if idx == 0 {
            panic!("Index 0 into '{name}' is out of bounds — Rosy uses 1-based indexing");
        }
        if idx > self.len() {
            self.resize_with(idx, T::default);
        }
        &mut self[idx - 1]
    }
}

impl RosyMutIndexable for f64 {
    type Out = f64;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut f64 {
        if idx != 1 {
            panic!("Index {idx} into scalar '{name}'");
        }
        self
    }
}

impl RosyMutIndexable for RosyValue {
    type Out = f64;
    fn rosy_index_mut(&mut self, idx: usize, name: &str) -> &mut f64 {
        if idx == 0 {
            panic!("Index 0 into '{name}' is out of bounds — Rosy uses 1-based indexing");
        }
        match self {
            RosyValue::VE(v) => {
                if idx > v.len() {
                    v.resize(idx, 0.0);
                }
                &mut v[idx - 1]
            }
            RosyValue::RE(v) if idx == 1 => v,
            RosyValue::RE(_) => panic!("Index {idx} into scalar '{name}'"),
            RosyValue::ST(_) => panic!("cannot mutably index '{name}' (ST)"),
            RosyValue::LO(_) => panic!("cannot mutably index '{name}' (LO)"),
            RosyValue::CM(_) => panic!("cannot mutably index '{name}' (CM)"),
            RosyValue::DA(_) => panic!("cannot mutably index '{name}' (DA)"),
            RosyValue::CD(_) => panic!("cannot mutably index '{name}' (CD)"),
        }
    }
}

#[inline(always)]
pub fn rosy_get_mut<'a, C: RosyMutIndexable + ?Sized>(
    container: &'a mut C,
    one_based: impl IntoF64,
    var_name: &str,
) -> &'a mut C::Out {
    container.rosy_index_mut(one_based.into_f64().round() as usize, var_name)
}

pub type RE = f64;
pub type ST = String;
pub type LO = bool;
pub type CM = num_complex::Complex64;
pub type VE = Vec<f64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RosyType {
    pub base_type: RosyBaseType,
    pub dimensions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RosyBaseType {
    RE,
    ST,
    LO,
    CM,
    VE,
    DA,
    CD,
    ANY,
}
impl std::fmt::Display for RosyBaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RosyBaseType::RE => write!(f, "RE"),
            RosyBaseType::ST => write!(f, "ST"),
            RosyBaseType::LO => write!(f, "LO"),
            RosyBaseType::CM => write!(f, "CM"),
            RosyBaseType::VE => write!(f, "VE"),
            RosyBaseType::DA => write!(f, "DA"),
            RosyBaseType::CD => write!(f, "CD"),
            RosyBaseType::ANY => write!(f, "ANY"),
        }
    }
}
impl std::fmt::Display for RosyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.dimensions == 0 {
            write!(f, "({})", self.base_type)
        } else {
            write!(f, "({} {}D)", self.base_type, self.dimensions)
        }
    }
}
impl RosyType {
    pub const fn new(base_type: RosyBaseType, dimensions: usize) -> Self {
        RosyType { base_type, dimensions }
    }

    #[allow(non_snake_case)]
    pub const fn RE() -> Self { Self::new(RosyBaseType::RE, 0) }
    #[allow(non_snake_case)]
    pub const fn ST() -> Self { Self::new(RosyBaseType::ST, 0) }
    #[allow(non_snake_case)]
    pub const fn LO() -> Self { Self::new(RosyBaseType::LO, 0) }
    #[allow(non_snake_case)]
    pub const fn CM() -> Self { Self::new(RosyBaseType::CM, 0) }
    #[allow(non_snake_case)]
    pub const fn VE() -> Self { Self::new(RosyBaseType::VE, 0) }
    #[allow(non_snake_case)]
    pub const fn DA() -> Self { Self::new(RosyBaseType::DA, 0) }
    #[allow(non_snake_case)]
    pub const fn CD() -> Self { Self::new(RosyBaseType::CD, 0) }
    #[allow(non_snake_case)]
    pub const fn ANY() -> Self { Self::new(RosyBaseType::ANY, 0) }

    pub fn is_any(&self) -> bool {
        self.base_type == RosyBaseType::ANY
    }

    /// Returns true if this type implements Copy in Rust (cheap to duplicate).
    /// RE (f64), LO (bool), CM (Complex64) are Copy at dimension 0.
    /// All array types (dimensions > 0) are non-Copy (Vec<...>).
    pub fn is_copy(&self) -> bool {
        if self.dimensions > 0 {
            return false; // arrays are Vec<...>, not Copy
        }
        matches!(
            self.base_type,
            RosyBaseType::RE | RosyBaseType::LO | RosyBaseType::CM
        )
    }

    pub fn as_rust_type(&self) -> String {
        let base = match self.base_type {
            RosyBaseType::RE => "f64",
            RosyBaseType::ST => "String",
            RosyBaseType::LO => "bool",
            RosyBaseType::CM => "num_complex::Complex64",
            RosyBaseType::VE => "Vec<f64>",
            RosyBaseType::DA => "DA",
            RosyBaseType::CD => "CD",
            RosyBaseType::ANY => "RosyValue",
        }
        .to_string();

        if self.dimensions == 0 {
            base
        } else {
            let mut result = base;
            for _ in 0..self.dimensions {
                result = format!("Vec<{}>", result);
            }
            result
        }
    }
}
impl TryFrom<&str> for RosyBaseType {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<RosyBaseType, Self::Error> {
        match value {
            "RE" => Ok(RosyBaseType::RE),
            "ST" => Ok(RosyBaseType::ST),
            "LO" => Ok(RosyBaseType::LO),
            "CM" => Ok(RosyBaseType::CM),
            "VE" => Ok(RosyBaseType::VE),
            "DA" => Ok(RosyBaseType::DA),
            "CD" => Ok(RosyBaseType::CD),
            _ => Err(anyhow::anyhow!("Can't convert {} to a Rosy type", value)),
        }
    }
}
