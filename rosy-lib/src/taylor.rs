//! Taylor series implementation for differential algebra.
//!
//! This module provides DA (real) and CD (complex) differential algebra types
//! for automatic differentiation and polynomial manipulation in beam physics simulations.

pub mod monomial;
pub mod config;
pub mod da;
pub mod horner;

pub use monomial::{Monomial, enumerate_monomials};
pub use config::{TaylorConfig, TaylorRuntime, init_taylor, cleanup_taylor, get_config, get_runtime, set_epsilon, set_truncation_order, set_filter_da, get_filter_da, set_weight_vector, dump_addressing_arrays};
pub use da::DACoefficient;
pub use horner::FixedMultiplier;

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
