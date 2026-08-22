//! COSY vs Rosy syntax for the current file (`.fox` vs `.rosy`).
//!
//! Thread-local so INCLUDE / LSP can switch per path. Not a process singleton.

use std::cell::Cell;
use std::path::Path;

thread_local! {
    static COSY_SYNTAX: Cell<bool> = const { Cell::new(false) };
}

pub fn set_cosy_syntax(enabled: bool) {
    COSY_SYNTAX.with(|c| c.set(enabled));
}

pub fn is_cosy_syntax() -> bool {
    COSY_SYNTAX.with(Cell::get)
}

pub fn is_fox_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("fox"))
}

/// Set syntax from `path` (`.fox` → COSY). Missing path stays Rosy.
pub fn apply_from_path(path: Option<&Path>) {
    set_cosy_syntax(path.is_some_and(is_fox_path));
}

/// Run `f` with syntax for `path`, then restore the previous mode.
pub fn with_path<T>(path: Option<&Path>, f: impl FnOnce() -> T) -> T {
    let prev = is_cosy_syntax();
    apply_from_path(path);
    let out = f();
    set_cosy_syntax(prev);
    out
}
