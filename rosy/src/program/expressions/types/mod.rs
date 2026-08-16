//! # Literals
//!
//! How to write literal values in Rosy.
//!
//! - **[`number`]** — `3.14`, `42`, `-7` (type `RE`)
//! - **[`string`]** — `'hello'` (type `ST`)
//! - **[`boolean`]** — `TRUE`, `FALSE` (type `LO`)
//! - **[`da`]** — `DA(1)` (Differential Algebra variable)
//! - **[`cd`]** — `CD(1)` (Complex DA variable)

pub mod boolean;
pub mod cd;
pub mod da;
pub mod number;
pub mod string;
