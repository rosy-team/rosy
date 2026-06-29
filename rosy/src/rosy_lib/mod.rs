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
pub mod intrinsics;
#[cfg(feature = "mpi")]
pub mod mpi;
pub mod operators;
pub mod optimizer;
pub mod taylor;

pub use core::*;
pub use intrinsics::*;
#[cfg(feature = "mpi")]
pub use mpi::*;
pub use operators::*;

pub use taylor::{CD, DA};
/// Immutable 1-based index. Returns `&T`.
/// Rounds the float index to nearest integer (matching COSY INFINITY's NINT),
/// then validates bounds with a 1-based error message.
#[inline(always)]
pub fn rosy_get<'a, T, C: AsRef<[T]>>(container: &'a C, one_based: f64, var_name: &str) -> &'a T {
    let slice = container.as_ref();
    let idx = one_based.round() as usize;
    slice.get(idx.wrapping_sub(1)).unwrap_or_else(|| {
        panic!(
            "Index {} into '{}' is out of bounds (1-{})",
            idx,
            var_name,
            slice.len()
        )
    })
}

/// Mutable 1-based index for assignment. Returns `&mut T`.
///
/// Unlike the read-side [`rosy_get`], this auto-grows the vector to fit
/// the requested 1-based index, padding new slots with `T::default()`.
/// This matches COSY semantics where `VARIABLE FOO ;` followed by
/// `FOO(1) := X` allocates slot 1 lazily.
///
/// Indices ≤ 0 still panic — those are programmer errors, not omissions.
#[inline(always)]
pub fn rosy_get_mut<'a, T: Default>(
    container: &'a mut Vec<T>,
    one_based: f64,
    var_name: &str,
) -> &'a mut T {
    let idx = one_based.round() as usize;
    if idx == 0 {
        panic!(
            "Index 0 into '{}' is out of bounds — Rosy uses 1-based indexing",
            var_name
        );
    }
    if idx > container.len() {
        container.resize_with(idx, T::default);
    }
    &mut container[idx - 1]
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
    pub fn new(base_type: RosyBaseType, dimensions: usize) -> Self {
        RosyType {
            base_type,
            dimensions,
        }
    }

    #[allow(non_snake_case)]
    pub fn RE() -> Self {
        RosyType {
            base_type: RosyBaseType::RE,
            dimensions: 0,
        }
    }
    #[allow(non_snake_case)]
    pub fn ST() -> Self {
        RosyType {
            base_type: RosyBaseType::ST,
            dimensions: 0,
        }
    }
    #[allow(non_snake_case)]
    pub fn LO() -> Self {
        RosyType {
            base_type: RosyBaseType::LO,
            dimensions: 0,
        }
    }
    #[allow(non_snake_case)]
    pub fn CM() -> Self {
        RosyType {
            base_type: RosyBaseType::CM,
            dimensions: 0,
        }
    }
    #[allow(non_snake_case)]
    pub fn VE() -> Self {
        RosyType {
            base_type: RosyBaseType::VE,
            dimensions: 0,
        }
    }
    #[allow(non_snake_case)]
    pub fn DA() -> Self {
        RosyType {
            base_type: RosyBaseType::DA,
            dimensions: 0,
        }
    }
    #[allow(non_snake_case)]
    pub fn CD() -> Self {
        RosyType {
            base_type: RosyBaseType::CD,
            dimensions: 0,
        }
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
