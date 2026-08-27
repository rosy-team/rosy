//! # Rosy Compiler
//!
#![doc = concat!("**Version:** `v", env!("CARGO_PKG_VERSION"), "` — [Changelog](https://github.com/rosy-team/rosy/releases)")]
//!
//! Parse, resolve, and transpile the Rosy scientific programming language
//! into Rust. Runtime types and DA live in the sibling `rosy_lib` crate.
//!
//! ## Language Reference
//! The official Rosy language reference begins in the [`program`] module.
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

/// VS Code language-configuration.json generated from the Pest grammar.
pub const VSCODE_LANGUAGE_CONFIGURATION: &str =
    include_str!(concat!(env!("OUT_DIR"), "/vscode_language_configuration.json"));

/// Tree-sitter highlights.scm generated from the Pest grammar.
pub const TREE_SITTER_HIGHLIGHTS: &str =
    include_str!(concat!(env!("OUT_DIR"), "/highlights.scm"));
