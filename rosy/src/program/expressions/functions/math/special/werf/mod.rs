//! # WERF Function (Faddeeva / Complex Error Function w)
//!
//! Computes the Faddeeva function w(z) = exp(-z^2) * erfc(-iz).
//!
//! ## Syntax
//!
//! ```text
//! WERF(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | CM | CM |
//! | CD | CD |
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

