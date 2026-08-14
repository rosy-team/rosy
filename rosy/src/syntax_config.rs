//! # Syntax Configuration
//!
//! Global toggle for COSY INFINITY vs. Rosy syntax mode.
//!
//! When `--cosy-syntax` is passed on the CLI, COSY INFINITY syntax rules
//! apply (e.g., VARIABLE declarations require a memory-size argument).
//! The default is Rosy mode.

/// Global syntax configuration for COSY vs Rosy syntax mode.
///
/// When `--cosy-syntax` is passed, COSY INFINITY syntax is enforced:
///   - VARIABLE declarations **require** a memory size as the first expression
///     after the name (it is parsed but discarded).
///   - Additional expressions after the memory size are array dimensions.
///
/// When `--cosy-syntax` is NOT passed (default Rosy mode):
///   - VARIABLE declarations do NOT accept memory sizes.
///   - All expressions after the name are treated as array dimensions.
///   - Types can optionally be annotated with `(RE)`, `(VE)`, etc.
use std::sync::OnceLock;

static COSY_SYNTAX: OnceLock<bool> = OnceLock::new();

/// Set the global syntax mode. Call this once from `main()` before parsing.
pub fn set_cosy_syntax(enabled: bool) {
    COSY_SYNTAX
        .set(enabled)
        .expect("syntax mode was already set");
}

/// Check whether COSY syntax mode is active.
pub fn is_cosy_syntax() -> bool {
    *COSY_SYNTAX.get().unwrap_or(&false)
}
