//! # IMAG Function
//!
//! Determines the imaginary part of a value.
//!
//! ## Syntax
//!
//! ```text
//! IMAG(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE    | RE     |
//! | CM    | RE     |
//! | DA    | DA     |
//! | CD    | DA     |
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

