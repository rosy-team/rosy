//! # Variables & Identifiers
//!
//! How Rosy resolves variable names, array indexing, and function calls.
//!
//! - **[`var_expr`]** — Variable references and function call disambiguation
//! - **[`variable_identifier`]** — Parsed identifiers with optional indexing/arguments

pub mod intrinsic_call;
pub mod var_expr;
pub mod variable_identifier;
