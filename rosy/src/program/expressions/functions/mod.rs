//! # Built-in Functions
//!
//! Named expression intrinsics (`SIN`, `ST`, …) parse as [`super::core::intrinsic_call`].
//! Types and emit live in `rosy_lib::registry`. Cases: `rosy/tests/constructs/expressions/functions/`.
//!
//! The only leftover AST here is [`math::exponential::pow`] (`^` is infix, not a call)
//! and [`conversion::string_convert`] (shared emit for `WRITE`).

pub mod conversion;
pub mod math;
