//! # Type Conversion Functions
//!
//! Convert values between Rosy base types.
//!
//! - **[`re_convert`]** — `RE(expr)` — convert to real
//! - **[`string_convert`]** — `ST(expr)` — convert to string
//! - **[`complex_convert`]** — `CM(expr)` — convert to complex
//! - **[`logical_convert`]** — `LO(expr)` — convert to logical
//! - **[`ve_convert`]** — `VE(expr)` — convert to vector

pub mod complex_convert;
pub mod logical_convert;
pub mod re_convert;
pub mod string_convert;
pub mod ve_convert;
