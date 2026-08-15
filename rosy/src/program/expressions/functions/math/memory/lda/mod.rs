//! # LDA Function (DA Memory Estimate)
//!
//! Returns the Differential Algebra memory size estimate. This is a COSY
//! INFINITY compatibility function — takes a VE of `(order & num_vars)`
//! and returns an estimated DA memory size as `RE`.
//!
//! ## Syntax
//!
//! ```text
//! LDA(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | VE | RE |
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

