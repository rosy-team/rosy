//! # Type Query & Sort Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | `TYPE(x)` | Returns a numeric code identifying the type of *x* |
//! | `ISRT(v)` | Sort a vector in ascending order |
//! | `ISRT3(v, u)` | Sort vector *v* and apply the same permutation to *u* |

pub mod isrt;
pub mod isrt3;
pub mod type_fn;
