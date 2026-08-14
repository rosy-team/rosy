//! # Mathematical Functions
//!
//! All built-in math functions in the Rosy language, grouped by category.
//!
//! - **[`trig`]** — `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `SINH`, `COSH`, `TANH`
//! - **[`exponential`]** — `EXP`, `LOG`, `SQR`, `SQRT`, `^` (power)
//! - **[`complex`]** — `CMPLX`, `CONJ`, `REAL`, `IMAG`
//! - **[`rounding`]** — `ABS`, `INT`, `NINT`, `NORM`, `CONS`
//! - **[`vector`]** — `VMIN`, `VMAX`
//! - **[`query`]** — `TYPE`, `ISRT`, `ISRT3`
//! - **[`memory`]** — `LST`, `LCM`, `LCD` (COSY compatibility, always return 0)

pub mod complex;
pub mod exponential;
pub mod memory;
pub mod query;
pub mod rounding;
pub mod special;
pub mod trig;
pub mod vector;
