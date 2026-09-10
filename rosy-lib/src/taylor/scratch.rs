//! COSY-compatible `SCRLEN` bump arena for DA operator temps.
//!
//! Size is in f64 words (default 50000). Named DA values stay on the heap.
//! Operator temps use this arena when they fit; otherwise that op allocates
//! a heap `Vec` so callers are not forced to size `SCRLEN`.
//!
//! The queried/set size is never grown automatically (COSY programs read it).
//! Live bump pointers also must not be invalidated by a realloc.

use std::cell::UnsafeCell;

use anyhow::{Result, bail};

/// COSY 10.2 default (`LSCR := -1; SCRLEN LSCR` with no prior set).
pub const DEFAULT_SCRLEN: usize = 50000;

struct ScratchState {
    words: Vec<f64>,
    cursor: usize,
}

impl ScratchState {
    fn new() -> Self {
        Self {
            words: vec![0.0; DEFAULT_SCRLEN],
            cursor: 0,
        }
    }
}

thread_local! {
    static SCRATCH: UnsafeCell<ScratchState> = UnsafeCell::new(ScratchState::new());
}

fn with_scratch<R>(f: impl FnOnce(&mut ScratchState) -> R) -> R {
    SCRATCH.with(|cell| f(unsafe { &mut *cell.get() }))
}

/// Restores the bump cursor when dropped (nested ops stack).
pub struct ScratchFrame {
    saved: usize,
}

impl ScratchFrame {
    pub fn enter() -> Self {
        with_scratch(|s| Self { saved: s.cursor })
    }
}

impl Drop for ScratchFrame {
    fn drop(&mut self) {
        with_scratch(|s| s.cursor = self.saved);
    }
}

/// Current arena length in words.
#[inline]
pub fn current_scrlen() -> usize {
    with_scratch(|s| s.words.len())
}

/// Bump-allocate `n` words, or `None` if they would not fit.
///
/// The pointer is valid until the matching [`ScratchFrame`] drops or
/// `rosy_scrlen` resizes. Prefer [`ScratchScope::alloc`], which falls back
/// to the heap instead of returning `None`.
pub fn try_scratch_alloc(n: usize) -> Option<*mut f64> {
    if n == 0 {
        return Some(std::ptr::null_mut());
    }
    with_scratch(|s| {
        if s.cursor + n > s.words.len() {
            return None;
        }
        let p = unsafe { s.words.as_mut_ptr().add(s.cursor) };
        s.cursor += n;
        Some(p)
    })
}

/// Frame plus any heap fallbacks for this operator.
///
/// Drop restores the bump cursor and frees overflow `Vec`s. Nested ops each
/// own a scope, so inner heap temps do not outlive their call.
pub struct ScratchScope {
    overflow: Vec<Vec<f64>>,
    _frame: ScratchFrame,
}

impl ScratchScope {
    pub fn enter() -> Self {
        Self {
            overflow: Vec::new(),
            _frame: ScratchFrame::enter(),
        }
    }

    /// `n` words from the bump arena, or a fresh heap buffer if it would not fit.
    pub fn alloc(&mut self, n: usize) -> *mut f64 {
        if let Some(p) = try_scratch_alloc(n) {
            return p;
        }
        self.overflow.push(vec![0.0; n]);
        // Inner heap buffer is stable across later `overflow` pushes.
        self.overflow.last_mut().unwrap().as_mut_ptr()
    }
}

/// Enter a frame and bump-reserve `n` words when they fit.
///
/// Misses are ignored: accounting-only callers still `pool_alloc`, and
/// storage callers should use [`ScratchScope`].
pub fn scratch_reserve(n: usize) -> ScratchFrame {
    let frame = ScratchFrame::enter();
    let _ = try_scratch_alloc(n);
    frame
}

fn set_scrlen(n: usize) -> Result<()> {
    with_scratch(|s| {
        if s.words.len() != n {
            s.words = vec![0.0; n];
        }
        s.cursor = 0;
        Ok(())
    })
}

/// Single COSY `SCRLEN` in-out operation.
///
/// `*c < 0` writes the current size into `c`. Otherwise NINT(`*c`) becomes
/// the new size and is written back into `c`.
pub fn rosy_scrlen(c: &mut f64) -> Result<()> {
    if *c < 0.0 {
        *c = current_scrlen() as f64;
        return Ok(());
    }
    let rounded = c.round();
    if !rounded.is_finite() || rounded < 0.0 {
        bail!("SCRLEN size must be a finite non-negative number, got {c}");
    }
    // Guard against accidental `usize::MAX`-sized allocations from huge REs.
    if rounded > 1_000_000_000.0 {
        bail!("SCRLEN {rounded} exceeds maximum 1000000000");
    }
    let n = rounded as usize;
    set_scrlen(n)?;
    *c = n as f64;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        let mut c = DEFAULT_SCRLEN as f64;
        rosy_scrlen(&mut c).unwrap();
    }

    #[test]
    fn default_query_is_50000() {
        reset();
        let mut c = -1.0;
        rosy_scrlen(&mut c).unwrap();
        assert_eq!(c, 50000.0);
    }

    #[test]
    fn set_then_query() {
        reset();
        let mut c = 12345.0;
        rosy_scrlen(&mut c).unwrap();
        assert_eq!(c, 12345.0);
        let mut q = -1.0;
        rosy_scrlen(&mut q).unwrap();
        assert_eq!(q, 12345.0);
        reset();
    }

    #[test]
    fn nint_1_7_sets_2() {
        reset();
        let mut c = 1.7;
        rosy_scrlen(&mut c).unwrap();
        assert_eq!(c, 2.0);
        let mut q = -1.0;
        rosy_scrlen(&mut q).unwrap();
        assert_eq!(q, 2.0);
        reset();
    }

    #[test]
    fn zero_is_valid() {
        reset();
        let mut c = 0.0;
        rosy_scrlen(&mut c).unwrap();
        assert_eq!(c, 0.0);
        let mut q = -1.0;
        rosy_scrlen(&mut q).unwrap();
        assert_eq!(q, 0.0);
        reset();
    }

    #[test]
    fn overflow_falls_back_to_heap() {
        reset();
        let mut c = 1.0;
        rosy_scrlen(&mut c).unwrap();
        let mut scope = ScratchScope::enter();
        assert!(try_scratch_alloc(2).is_none());
        let p = scope.alloc(2);
        assert!(!p.is_null());
        unsafe {
            *p = 1.0;
            *p.add(1) = 2.0;
            assert_eq!(*p, 1.0);
            assert_eq!(*p.add(1), 2.0);
        }
        let mut q = -1.0;
        rosy_scrlen(&mut q).unwrap();
        assert_eq!(q, 1.0, "heap fallback must not grow the queried SCRLEN");
        reset();
    }

    #[test]
    fn zero_scrlen_allocates_on_heap() {
        reset();
        let mut c = 0.0;
        rosy_scrlen(&mut c).unwrap();
        let mut scope = ScratchScope::enter();
        let p = scope.alloc(4);
        unsafe {
            std::slice::from_raw_parts_mut(p, 4).fill(3.0);
            assert_eq!(*p, 3.0);
        }
        reset();
    }
}
