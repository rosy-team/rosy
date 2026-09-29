//! Random numbers.
//!
//! `RERAN` follows COSY Infinity 10.2 (`reran_` in the 10.2 binary): one
//! saved angle `XRAN`, started at 0.234. Each draw does
//!
//! ```text
//! XRAN = (XRAN + 10) + 1234321/1000000
//! if XRAN > 10000 { XRAN = XRAN - 9999*|cos(XRAN)| }
//! RERAN = 2*(frac(|sin(XRAN)|*1000000) - 0.5)
//! ```
//!
//! `RANSEED s` (s >= 0) sets that angle to `s`, so the next `RERAN` restarts
//! from there. A negative seed draws a fresh angle from the OS.
//!
//! FIT and `DARAN` keep a separate `StdRng` (seed 0 unless `RANSEED` says
//! otherwise). COSY's `DARAN` shares `XRAN`; that path is not wired yet.

use rand::{Rng, SeedableRng, rngs::StdRng};
use std::sync::RwLock;

/// COSY `bran_$XRAN`, initialized to 0.234 in the 10.2 image.
static XRAN: RwLock<f64> = RwLock::new(0.234);

/// Separate stream for FIT and DARAN.
static GLOBAL_RNG: RwLock<Option<StdRng>> = RwLock::new(None);

fn ensure_init(guard: &mut Option<StdRng>) {
    if guard.is_none() {
        *guard = Some(StdRng::seed_from_u64(0));
    }
}

/// `RANSEED`.
///
/// - `seed < 0`: new OS-entropy stream for FIT/`DARAN`, and a new `XRAN`.
/// - `seed >= 0`: FIT/`DARAN` reseed from `seed as u64`, and `XRAN = seed`
///   so the next `RERAN` matches a COSY run that started from that angle.
pub fn set_rng_seed(seed: f64) {
    let (new_rng, angle) = if seed < 0.0 {
        let mut rng = StdRng::from_os_rng();
        let angle = rng.random_range(0.0..10000.0);
        (rng, angle)
    } else {
        (StdRng::seed_from_u64(seed as u64), seed)
    };
    *GLOBAL_RNG.write().unwrap() = Some(new_rng);
    *XRAN.write().unwrap() = angle;
}

/// COSY `RERAN`: one value in `[-1, 1)`.
pub fn rosy_reran() -> f64 {
    let mut xran = XRAN.write().unwrap();
    *xran = (*xran + 10.0) + (1_234_321.0 / 1_000_000.0);
    if *xran > 10_000.0 {
        *xran -= 9_999.0 * xran.cos().abs();
    }
    let scaled = xran.sin().abs() * 1_000_000.0;
    let frac = scaled - scaled.trunc();
    (frac - 0.5) * 2.0
}

/// Uniform `[0, 1)` for the FIT optimizer.
pub fn rng_f64() -> f64 {
    let mut guard = GLOBAL_RNG.write().unwrap();
    ensure_init(&mut guard);
    guard.as_mut().unwrap().random_range(0.0..1.0)
}

/// Uniform `[-1, 1)` for FIT perturbations.
pub fn rng_f64_symmetric() -> f64 {
    let mut guard = GLOBAL_RNG.write().unwrap();
    ensure_init(&mut guard);
    2.0 * guard.as_mut().unwrap().random_range(0.0..1.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reran_matches_cosy_10_2_prefix() {
        *XRAN.write().unwrap() = 0.234;
        // Printed by COSY Infinity 10.2 with no prior RERAN.
        let expect = [
            0.9563653094228357,
            0.2003791886381805,
            0.4825703673996031,
            0.8870908897370100,
            -0.4921502069919370,
        ];
        for e in expect {
            let g = rosy_reran();
            assert!(
                (g - e).abs() < 1e-15,
                "reran {g:.16} vs cosy {e:.16}"
            );
        }
    }
}
