//! # Rounding & Absolute Value Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | `ABS(x)` | Absolute value |
//! | `INT(x)` | Truncate to integer (toward zero) |
//! | `NINT(x)` | Round to nearest integer |
//! | `NORM(x)` | Euclidean norm |
//! | `CONS(x)` | Constant part (zeroth-order coefficient of DA) |

pub mod abs;
pub mod cons;
pub mod int_fn;
pub mod nint;
pub mod norm;
