//! Memory statistics helpers for MEMALL / MEMFRE.
//!
//! Wraps the `memory_stats` crate so that the generated main.rs only needs to
//! call `rosy_lib` functions rather than depending on `memory_stats` directly.

/// Returns the current physical memory usage of the process in bytes.
/// Falls back to `0.0` on platforms where the query is not supported.
pub fn rosy_memall() -> f64 {
    memory_stats::memory_stats()
        .map(|s| s.physical_mem as f64)
        .unwrap_or(0.0)
}

/// Returns an approximation of "available" memory: `isize::MAX` minus current
/// physical usage.  Falls back to `f64::MAX` when the query is not supported.
pub fn rosy_memfre() -> f64 {
    memory_stats::memory_stats()
        .map(|s| (isize::MAX as f64) - s.physical_mem as f64)
        .unwrap_or(f64::MAX)
}
