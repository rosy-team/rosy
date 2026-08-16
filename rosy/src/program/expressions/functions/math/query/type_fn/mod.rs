//! # TYPE Function
//!
//! Returns the COSY type code of a value as RE.
//!
//! ## Syntax
//!
//! ```text
//! TYPE(expr)
//! ```
//!
//! ## Type Codes
//!
//! | Input | Code |
//! |-------|------|
//! | RE    |  1   |
//! | ST    |  2   |
//! | LO    |  3   |
//! | CM    |  4   |
//! | VE    |  5   |
//! | DA    |  6   |
//! | CD    |  7   |
//! | GR    |  8   |
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

