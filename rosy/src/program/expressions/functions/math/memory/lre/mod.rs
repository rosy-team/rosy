//! # LRE Function (Real Memory Estimate)
//!
//! Returns the Real memory size estimate. This is a COSY INFINITY
//! compatibility function — Rosy does not require memory management,
//! but the function is provided so legacy scripts parse correctly.
//!
//! In practice, `LRE(n)` returns `1` since a real always occupies 1 unit.
//!
//! ## Syntax
//!
//! ```text
//! LRE(expr)
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

