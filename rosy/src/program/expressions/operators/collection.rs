//! # Collection & DA Operators
//!
//! | Operator | Symbol | Description |
//! |----------|--------|-------------|
//! | `&` | [`mod@concat`] | Vector/string concatenation |
//! | `\|` | [`extract`] | Element extraction (indexing) |
//! | `%` | [`mod@derive`] | DA partial derivative |

pub mod concat;
pub mod derive;
pub mod extract;
