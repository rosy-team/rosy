//! # CMPLX Function (Convert to Complex)
//!
//! Converts a value to complex type.
//!
//! ## Syntax
//!
//! ```text
//! CMPLX(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | CM |
//! | CM | CM |
//! | DA | CD |
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
//! ## COSY INFINITY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
//! ```

