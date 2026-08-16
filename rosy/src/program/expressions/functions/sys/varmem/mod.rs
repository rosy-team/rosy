//! # VARMEM Function
//!
//! Returns the current memory address of an object as a real number.
//! Since Rosy transpiles to Rust (not Fortran), true COSY memory addresses
//! are meaningless. VARMEM returns the actual Rust pointer address cast to f64.
//!
//! ## Syntax
//!
//! ```text
//! VARMEM(expr)
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

