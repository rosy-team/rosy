/// Returns a pseudo-random f64 in [-1, 1], matching COSY's RERAN behavior.
/// Draws from the global seeded RNG (see [`super::rng`]).
pub fn rosy_reran() -> f64 {
    super::rng::rosy_reran()
}
