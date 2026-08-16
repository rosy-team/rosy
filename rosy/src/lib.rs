//! # Rosy
//!
#![doc = concat!("**Version:** `v", env!("CARGO_PKG_VERSION"), "` — [Changelog](https://github.com/rosy-team/rosy/releases)")]
//!
//! A modern transpiler for the Rosy scientific programming language,
//! designed for beam physics and differential algebra applications.
//! Rosy programs are transpiled into self-contained, native Rust executables.
//!
//! ## Language Reference
//! The official Rosy language reference begins in the [`program`] module.
//! Runtime types and DA live in the sibling `rosy_lib` crate.
//!
//! ## More Resources
//! - **[Example programs](https://github.com/rosy-team/rosy/tree/master/examples)** on GitHub
//! - **[Installation & usage](https://github.com/rosy-team/rosy)** in the README

pub mod compiler;
pub mod lsp;
pub mod program;

pub use compiler::ast;
pub use compiler::embedded;
pub use compiler::errors;
pub use compiler::resolve;
pub use compiler::transpile;
pub use program::syntax_config;
