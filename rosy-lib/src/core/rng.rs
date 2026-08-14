//! Global seeded random number generator.
//!
//! Provides a single, program-wide `StdRng` initialized with seed `0` by default.
//! All randomness in Rosy (RERAN, FIT optimizer, etc.) draws from this RNG,
//! ensuring reproducible results across runs.
//!
//! Use [`set_rng_seed`] (the `RANSEED` statement) to change the seed at runtime.
//! A negative value switches to system-entropy seeding; a positive value is
//! truncated to `u64` and used as a deterministic seed.

use std::sync::RwLock;
use rand::{Rng, SeedableRng, rngs::StdRng};

/// The global RNG, seeded to 0 by default for reproducibility.
static GLOBAL_RNG: RwLock<Option<StdRng>> = RwLock::new(None);

/// Ensure the global RNG is initialized (lazily, on first use).
fn ensure_init(guard: &mut Option<StdRng>) {
    if guard.is_none() {
        *guard = Some(StdRng::seed_from_u64(0));
    }
}

/// Set the global RNG seed.
///
/// - If `seed < 0.0`, the RNG is reseeded from OS entropy (`StdRng::from_os_rng()`).
/// - If `seed >= 0.0`, the value is truncated to `u64` and used as a deterministic seed.
pub fn set_rng_seed(seed: f64) {
    let new_rng = if seed < 0.0 {
        StdRng::from_os_rng()
    } else {
        StdRng::seed_from_u64(seed as u64)
    };
    let mut guard = GLOBAL_RNG.write().unwrap();
    *guard = Some(new_rng);
}

/// Generate a random `f64` in `[-1, 1]` from the global RNG.
/// This is the runtime backing for the `RERAN` statement.
pub fn rosy_reran() -> f64 {
    let mut guard = GLOBAL_RNG.write().unwrap();
    ensure_init(&mut guard);
    guard.as_mut().unwrap().random_range(-1.0..=1.0)
}

/// Generate a random `f64` in `[0, 1)` from the global RNG.
/// Used by the FIT optimizer for acceptance probability tests.
pub fn rng_f64() -> f64 {
    let mut guard = GLOBAL_RNG.write().unwrap();
    ensure_init(&mut guard);
    guard.as_mut().unwrap().random_range(0.0..1.0)
}

/// Generate a random `f64` in `[-1, 1)` from the global RNG.
/// Used by the FIT optimizer for symmetric perturbation.
pub fn rng_f64_symmetric() -> f64 {
    let mut guard = GLOBAL_RNG.write().unwrap();
    ensure_init(&mut guard);
    2.0 * guard.as_mut().unwrap().random_range(0.0..1.0) - 1.0
}
