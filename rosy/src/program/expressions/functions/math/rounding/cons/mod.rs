//! # CONS Function
//!
//! Extracts the constant (scalar) part of a value.
//!
//! ## Syntax
//!
//! ```text
//! CONS(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE    | RE     |
//! | CM    | CM     |
//! | VE    | RE     |
//! | DA    | RE     |
//! | CD    | CM     |
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

