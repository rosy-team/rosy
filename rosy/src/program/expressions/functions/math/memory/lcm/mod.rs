//! # LCM Function (Complex Memory Estimate)
//!
//! Returns the complex-number memory size estimate. This is a COSY INFINITY
//! compatibility function — always returns `2.0` (a CM always occupies 2 RE words).
//!
//! ## Syntax
//!
//! ```text
//! LCM(expr)
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
//! ## COSY INFINITY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
//! ```

