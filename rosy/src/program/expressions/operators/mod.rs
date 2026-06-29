//! # Operators
//!
//! All binary and unary operators in the Rosy language.
//!
//! - **[`arithmetic`]** — `+`, `-`, `*`, `/`
//! - **[`comparison`]** — `=`, `<>`, `<`, `>`, `<=`, `>=`
//! - **[`logical`]** — `AND`, `OR`
//! - **[`unary`]** — `-x` (negation), `NOT x`
//! - **[`collection`]** — `&` (concatenate), `|` (extract), `%` (DA derivative)
//!
//! The `^` (power) operator is in [`super::functions::math::exponential::pow`].
//!
//! Each operator uses a registry-driven type system defined in
//! [`rosy_lib::operators`](crate::rosy_lib::operators) that serves as the
//! single source of truth for which type combinations are valid.

pub mod arithmetic;
pub mod collection;
pub mod comparison;
pub mod logical;
pub mod unary;
