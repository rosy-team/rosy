//! # VARPOI Function
//!
//! Returns the current pointer address of an object as RE (f64).
//! In Rosy, this returns the Rust pointer address cast to f64,
//! identical to VARMEM (Rust has no Fortran-style pointer/memory distinction).
//!
//! ## Syntax
//!
//! ```text
//! VARPOI(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | RE |
//! | ST | RE |
//! | LO | RE |
//! | CM | RE |
//! | VE | RE |
//! | DA | RE |
//! | CD | RE |
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

