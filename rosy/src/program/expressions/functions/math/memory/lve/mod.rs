//! # LVE Function (Vector Memory Estimate)
//!
//! Returns the Vector memory size estimate. This is a COSY INFINITY
//! compatibility function — Rosy does not require memory management,
//! but the function is provided so legacy scripts parse correctly.
//!
//! In practice, `LVE(n)` returns `n` since a vector of n components
//! requires n units of allocation.
//!
//! ## Syntax
//!
//! ```text
//! LVE(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | RE |
//!
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```
//! ## COSY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
//! ```

