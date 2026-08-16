//! COSY vs Rosy syntax for the current compile (`--cosy-syntax`).
//!
//! Thread-local so the LSP can reset it per document. Not a process singleton.

use std::cell::Cell;

thread_local! {
    static COSY_SYNTAX: Cell<bool> = const { Cell::new(false) };
}

pub fn set_cosy_syntax(enabled: bool) {
    COSY_SYNTAX.with(|c| c.set(enabled));
}

pub fn is_cosy_syntax() -> bool {
    COSY_SYNTAX.with(Cell::get)
}
