//! Taylor series implementation for differential algebra.
//!
//! This module provides DA (real) and CD (complex) differential algebra types
//! for automatic differentiation and polynomial manipulation in beam physics simulations.

pub mod config;
pub mod da;
pub mod horner;
pub mod monomial;

pub use config::{
    TaylorConfig, TaylorRuntime, cleanup_taylor, dump_addressing_arrays, get_config, get_filter_da,
    get_runtime, init_taylor, set_epsilon, set_filter_da, set_truncation_order, set_weight_vector,
};
pub use da::DACoefficient;
pub use horner::FixedMultiplier;
pub use monomial::{Monomial, cosy_display_rank, enumerate_monomials};

// Core generic differential algebra type
use num_complex::Complex64;

/// Real differential algebra (f64 coefficients) - traditional DA
pub type DA = da::DA<f64>;

/// Complex differential algebra (Complex64 coefficients) - replaces CD
pub type CD = da::DA<Complex64>;

/// Maximum number of variables supported.
///
/// Set to 16 to handle typical beam physics cases:
/// - 6D phase space (x, px, y, py, z, pz)
/// - Additional coupling/parameter variables
pub const MAX_VARS: usize = 6;

/// Default epsilon for coefficient truncation.
pub const DEFAULT_EPSILON: f64 = 1e-15;
