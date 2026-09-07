//! Binary-only CLI. The compiler lives in `rosy_compiler`.

pub(crate) mod compile;
pub(crate) mod setup;
pub(crate) mod test;

use std::path::Path;

/// Format a path for terminal output using the host's native separators.
pub(crate) fn display_path(path: impl AsRef<Path>) -> String {
    let s = path.as_ref().display().to_string();
    if cfg!(windows) {
        s.replace('/', "\\")
    } else {
        s
    }
}

pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const CYAN: &str = "\x1b[36m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const RESET: &str = "\x1b[0m";
