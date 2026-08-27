//! COSY vs Rosy syntax for the current file (`.fox` vs `.rosy`).
//!
//! Thread-local so INCLUDE / LSP can switch per path. Not a process singleton.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

thread_local! {
    static COSY_SYNTAX: Cell<bool> = const { Cell::new(false) };
    static CURRENT_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn set_cosy_syntax(enabled: bool) {
    COSY_SYNTAX.with(|c| c.set(enabled));
}

pub fn is_cosy_syntax() -> bool {
    COSY_SYNTAX.with(Cell::get)
}

pub fn current_source_path() -> Option<PathBuf> {
    CURRENT_PATH.with(|c| c.borrow().clone())
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

/// Run `f` with syntax and source path for `path`, then restore both.
pub fn with_path<T>(path: Option<&Path>, f: impl FnOnce() -> T) -> T {
    let prev_syntax = is_cosy_syntax();
    let prev_path = current_source_path();
    apply_from_path(path);
    CURRENT_PATH.with(|c| *c.borrow_mut() = path.map(Path::to_path_buf));
    let out = f();
    set_cosy_syntax(prev_syntax);
    CURRENT_PATH.with(|c| *c.borrow_mut() = prev_path);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn with_path_sets_and_restores() {
        assert!(current_source_path().is_none());
        with_path(Some(Path::new("/tmp/foo.rosy")), || {
            assert_eq!(
                current_source_path().as_deref(),
                Some(Path::new("/tmp/foo.rosy"))
            );
            with_path(Some(Path::new("/tmp/bar.fox")), || {
                assert!(is_cosy_syntax());
                assert_eq!(
                    current_source_path().as_deref(),
                    Some(Path::new("/tmp/bar.fox"))
                );
            });
            assert!(!is_cosy_syntax());
            assert_eq!(
                current_source_path().as_deref(),
                Some(Path::new("/tmp/foo.rosy"))
            );
        });
        assert!(current_source_path().is_none());
    }
}
