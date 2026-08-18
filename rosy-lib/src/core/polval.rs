//! POLVAL - polynomial evaluation / composition.
//!
//! `POLVAL L P NP A NA R NR;`
//!
//! Lets the polynomial described by NP DA vectors stored in the array P
//! act on the NA arguments A, and stores the NR results in R.
//!
//! In the normal case L == 1 (Horner evaluation).
//! The current implementation supports RE (f64) arguments (plain polynomial evaluation)
//! and returns an error for other argument types at runtime.

use crate::taylor::{CD, DA};
use anyhow::{Result, bail};

#[cfg(feature = "nightly-simd")]
use std::simd::prelude::*;

#[cfg(feature = "nightly-simd")]
const LANES: usize = 4;

/// Evaluate NP polynomials (stored in `p_array` as DA vectors) at the NA real
/// arguments in `a_array`, writing NR results into `r_array`.
///
/// # Arguments
/// * `_l`       - evaluation mode flag (1 = Horner; currently ignored, always Horner)
/// * `p_array`  - slice of NP DA polynomials
/// * `np`       - number of polynomials to evaluate
/// * `a_array`  - slice of NA real-valued arguments
/// * `na`       - number of arguments
/// * `r_array`  - output vector, must be large enough to hold NR results
/// * `nr`       - number of results to write
pub fn rosy_polval_re(
    _l: impl crate::IntoF64,
    p_array: &impl crate::PolvalDaSrc,
    np: impl crate::IntoF64,
    a_array: &impl crate::PolvalReSrc,
    na: impl crate::IntoF64,
    r_array: &mut impl crate::PolvalReDst,
    nr: impl crate::IntoF64,
) -> Result<()> {
    let p_array = p_array.to_da_vec();
    let a_array = a_array.to_re_vec();
    let np = crate::rosy_as_usize(&np.into_f64());
    let na = crate::rosy_as_usize(&na.into_f64());
    let nr = crate::rosy_as_usize(&nr.into_f64());
    if np < nr {
        bail!("POLVAL: NP ({}) must be >= NR ({})", np, nr);
    }

    let mut out = r_array.load_re_vec();
    while out.len() < nr {
        out.push(0.0);
    }

    for i in 0..nr {
        if i >= p_array.len() {
            bail!("POLVAL: polynomial array too short at index {}", i);
        }
        out[i] = evaluate_da_at_re(&p_array[i], &a_array, na)?;
    }
    r_array.store_re_vec(out);

    Ok(())
}

/// Batch-evaluate NP polynomials at multiple particles simultaneously.
///
/// Each element of `a_array` is a VE (`Vec<f64>`) of particle values for one
/// coordinate axis. For example, `a_array\[0\]` holds x-values for all particles,
/// `a_array\[1\]` holds px-values, etc.
///
/// Results are written the same way: `r_array\[i\]` will hold the i-th result
/// component for all particles.
///
/// With `nightly-simd`: processes 4 particles per monomial using f64x4 SIMD.
pub fn rosy_polval_ve(
    _l: f64,
    p_array: &[DA],
    np: usize,
    a_array: &[Vec<f64>],
    na: usize,
    r_array: &mut Vec<Vec<f64>>,
    nr: usize,
) -> Result<()> {
    if np < nr {
        bail!("POLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if a_array.len() < na {
        bail!(
            "POLVAL: argument array has {} elements but NA={}",
            a_array.len(),
            na
        );
    }

    let num_particles = if na > 0 { a_array[0].len() } else { 0 };

    while r_array.len() < nr {
        r_array.push(Vec::new());
    }

    for i in 0..nr {
        if i >= p_array.len() {
            bail!("POLVAL: polynomial array too short at index {}", i);
        }
        r_array[i].resize(num_particles, 0.0);
        evaluate_poly_batch(&p_array[i], a_array, na, num_particles, &mut r_array[i]);
    }

    Ok(())
}

/// Evaluate a single DA polynomial at all particles, writing results into `out`.
///
/// Iterates monomials once, processing particles in SIMD chunks of 4.
#[inline]
fn evaluate_poly_batch(
    poly: &DA,
    a_array: &[Vec<f64>],
    na: usize,
    num_particles: usize,
    out: &mut [f64],
) {
    // Zero the output
    out.iter_mut().for_each(|v| *v = 0.0);

    for (monomial, coeff) in poly.coeffs_iter().into_iter() {
        let exponents = &monomial.exponents;

        // Collect active variables (non-zero exponents) for this monomial
        let mut active_vars: [(usize, u8); 6] = [(0, 0); 6];
        let mut num_active = 0;
        for (var_idx, &exp) in exponents.iter().enumerate() {
            if exp != 0 && var_idx < na {
                active_vars[num_active] = (var_idx, exp);
                num_active += 1;
                if num_active >= 6 {
                    break;
                }
            }
        }

        // Constant monomial (no variables) — just add coefficient to all particles
        if num_active == 0 {
            for j in 0..num_particles {
                out[j] += coeff;
            }
            continue;
        }

        #[cfg(feature = "nightly-simd")]
        {
            let chunks = num_particles / LANES;
            let coeff_v = Simd::<f64, LANES>::splat(coeff);

            for c in 0..chunks {
                let base = c * LANES;
                let mut term = coeff_v;

                for a in 0..num_active {
                    let (var_idx, exp) = active_vars[a];
                    let vals = Simd::<f64, LANES>::from_slice(&a_array[var_idx][base..]);
                    term *= simd_powi(vals, exp);
                }

                let current = Simd::<f64, LANES>::from_slice(&out[base..]);
                (current + term).copy_to_slice(&mut out[base..base + LANES]);
            }

            // Scalar remainder
            for j in (chunks * LANES)..num_particles {
                let mut term = coeff;
                for a in 0..num_active {
                    let (var_idx, exp) = active_vars[a];
                    term *= scalar_powi(a_array[var_idx][j], exp);
                }
                out[j] += term;
            }
        }

        #[cfg(not(feature = "nightly-simd"))]
        {
            for j in 0..num_particles {
                let mut term = coeff;
                for a in 0..num_active {
                    let (var_idx, exp) = active_vars[a];
                    term *= scalar_powi(a_array[var_idx][j], exp);
                }
                out[j] += term;
            }
        }
    }
}

/// SIMD power: compute vals^exp for small exponents via repeated multiply.
#[cfg(feature = "nightly-simd")]
#[inline(always)]
fn simd_powi(vals: Simd<f64, LANES>, exp: u8) -> Simd<f64, LANES> {
    match exp {
        0 => Simd::<f64, LANES>::splat(1.0),
        1 => vals,
        2 => vals * vals,
        3 => vals * vals * vals,
        4 => {
            let v2 = vals * vals;
            v2 * v2
        }
        5 => {
            let v2 = vals * vals;
            v2 * v2 * vals
        }
        6 => {
            let v2 = vals * vals;
            v2 * v2 * v2
        }
        _ => {
            // General case via repeated squaring
            let mut result = Simd::<f64, LANES>::splat(1.0);
            let mut base = vals;
            let mut e = exp;
            while e > 0 {
                if e & 1 == 1 {
                    result *= base;
                }
                base *= base;
                e >>= 1;
            }
            result
        }
    }
}

/// Scalar power for small exponents.
#[inline(always)]
fn scalar_powi(val: f64, exp: u8) -> f64 {
    match exp {
        0 => 1.0,
        1 => val,
        2 => val * val,
        3 => val * val * val,
        4 => {
            let v2 = val * val;
            v2 * v2
        }
        5 => {
            let v2 = val * val;
            v2 * v2 * val
        }
        6 => {
            let v2 = val * val;
            v2 * v2 * v2
        }
        _ => val.powi(exp as i32),
    }
}

/// Substitute NA Taylor-series arguments into NP Taylor-series polynomials,
/// producing NR Taylor-series results — the COSY map-composition path.
///
/// Each `p_array[i]` is a polynomial in canonical DA variables (e.g. δx, δp_x …);
/// each `a_array[j]` is itself a Taylor series in the *same* canonical variables
/// (typically a saved map's j-th component). The output `r_array[i]` is the
/// composition `p_array[i] ∘ a_array`, truncated automatically to the current
/// truncation order via DA's overloaded `*` and `+`.
///
/// COSY's `ANM N M O` lowers to `POLVAL 1 N TWOND MM NV O TWOND` where MM is N's
/// map padded with identity DAs for non-physical slots — see libcosy/physics/map_ops.rosy.
pub fn rosy_polval_da(
    _l: f64,
    p_array: &[DA],
    np: usize,
    a_array: &[DA],
    na: usize,
    r_array: &mut Vec<DA>,
    nr: usize,
) -> Result<()> {
    if np < nr {
        bail!("POLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if a_array.len() < na {
        bail!(
            "POLVAL: argument array has {} elements but NA={}",
            a_array.len(),
            na
        );
    }

    while r_array.len() < nr {
        r_array.push(DA::zero());
    }

    for i in 0..nr {
        if i >= p_array.len() {
            bail!("POLVAL: polynomial array too short at index {}", i);
        }
        r_array[i] = evaluate_da_at_da(&p_array[i], a_array, na)?;
    }

    Ok(())
}

/// Compose a single DA polynomial with NA Taylor-series substitutions.
///
/// For each monomial `c · x_1^{e_1} · … · x_na^{e_na}` of `poly` we form
/// `c · args[0]^{e_1} · … · args[na-1]^{e_na}` (DA × DA), then sum across
/// monomials. DA `*` and `+` return `Result` (truncation-buffer overflow is
/// rare but surfaceable), so we propagate with `?` rather than panic.
fn evaluate_da_at_da(poly: &DA, args: &[DA], na: usize) -> Result<DA> {
    let mut result = DA::zero();

    for (monomial, coeff) in poly.coeffs_iter().into_iter() {
        let exponents = &monomial.exponents;
        let mut term = DA::from_coeff(coeff);

        for (var_idx, &exp) in exponents.iter().enumerate() {
            if exp == 0 {
                continue;
            }
            if var_idx >= na || var_idx >= args.len() {
                bail!(
                    "POLVAL: variable index {} out of range (NA={})",
                    var_idx + 1,
                    na
                );
            }
            let pow = da_powi(&args[var_idx], exp)?;
            term = (term * pow)?;
        }

        result = (result + term)?;
    }

    Ok(result)
}

/// CPOLVAL — complex-DA polynomial composition. Companion to `rosy_polval_da`
/// where every coefficient algebra is over `Complex64` instead of `f64`.
///
/// DANF / NF / TS / TP and the COSY-to-circular conversion procedures (COCR /
/// CRCO) all chain through this — once a normal-form transformation is
/// constructed it lives in CD-space, and the resonance / tune / Twiss
/// extraction relies on substituting CD bases into CD polynomials.
///
/// Algorithmically identical to `rosy_polval_da` — only the coefficient
/// type differs. The DA struct is generic over `T: DACoefficient` and
/// `Complex64` implements that trait, so the same `*` / `+` / `clone()`
/// API works through the type alias.
pub fn rosy_polval_cd(
    _l: f64,
    p_array: &[CD],
    np: usize,
    a_array: &[CD],
    na: usize,
    r_array: &mut Vec<CD>,
    nr: usize,
) -> Result<()> {
    if np < nr {
        bail!("CPOLVAL: NP ({}) must be >= NR ({})", np, nr);
    }
    if a_array.len() < na {
        bail!(
            "CPOLVAL: argument array has {} elements but NA={}",
            a_array.len(),
            na
        );
    }

    while r_array.len() < nr {
        r_array.push(CD::zero());
    }

    for i in 0..nr {
        if i >= p_array.len() {
            bail!("CPOLVAL: polynomial array too short at index {}", i);
        }
        r_array[i] = evaluate_cd_at_cd(&p_array[i], a_array, na)?;
    }

    Ok(())
}

/// Compose a single CD polynomial with NA Taylor-series substitutions over
/// the complex-DA algebra. Mirrors `evaluate_da_at_da` exactly — see its
/// header for the per-monomial substitution logic.
fn evaluate_cd_at_cd(poly: &CD, args: &[CD], na: usize) -> Result<CD> {
    let mut result = CD::zero();

    for (monomial, coeff) in poly.coeffs_iter().into_iter() {
        let exponents = &monomial.exponents;
        let mut term = CD::from_coeff(coeff);

        for (var_idx, &exp) in exponents.iter().enumerate() {
            if exp == 0 {
                continue;
            }
            if var_idx >= na || var_idx >= args.len() {
                bail!(
                    "CPOLVAL: variable index {} out of range (NA={})",
                    var_idx + 1,
                    na
                );
            }
            let pow = cd_powi(&args[var_idx], exp)?;
            term = (term * pow)?;
        }

        result = (result + term)?;
    }

    Ok(result)
}

/// CD-typed exponentiation by repeated squaring. Mirrors `da_powi`.
#[inline]
pub fn cd_powi(base: &CD, exp: u8) -> Result<CD> {
    Ok(match exp {
        0 => CD::from_coeff(num_complex::Complex64::new(1.0, 0.0)),
        1 => base.clone(),
        2 => (base.clone() * base)?,
        3 => {
            let v2 = (base.clone() * base)?;
            (v2 * base)?
        }
        4 => {
            let v2 = (base.clone() * base)?;
            (v2.clone() * &v2)?
        }
        5 => {
            let v2 = (base.clone() * base)?;
            let v4 = (v2.clone() * &v2)?;
            (v4 * base)?
        }
        6 => {
            let v2 = (base.clone() * base)?;
            let v3 = (v2 * base)?;
            (v3.clone() * &v3)?
        }
        _ => {
            let mut result = CD::from_coeff(num_complex::Complex64::new(1.0, 0.0));
            let mut current = base.clone();
            let mut e = exp;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * &current)?;
                }
                current = (current.clone() * &current)?;
                e >>= 1;
            }
            result
        }
    })
}

/// Compute base^exp for small u8 exponents using exponentiation-by-squaring.
#[inline]
pub fn da_powi(base: &DA, exp: u8) -> Result<DA> {
    Ok(match exp {
        0 => DA::from_coeff(1.0),
        1 => base.clone(),
        2 => (base.clone() * base)?,
        3 => {
            let v2 = (base.clone() * base)?;
            (v2 * base)?
        }
        4 => {
            let v2 = (base.clone() * base)?;
            (v2.clone() * &v2)?
        }
        5 => {
            let v2 = (base.clone() * base)?;
            let v4 = (v2.clone() * &v2)?;
            (v4 * base)?
        }
        6 => {
            let v2 = (base.clone() * base)?;
            let v3 = (v2 * base)?;
            (v3.clone() * &v3)?
        }
        _ => {
            let mut result = DA::from_coeff(1.0);
            let mut current = base.clone();
            let mut e = exp;
            while e > 0 {
                if e & 1 == 1 {
                    result = (result * &current)?;
                }
                current = (current.clone() * &current)?;
                e >>= 1;
            }
            result
        }
    })
}

/// Evaluate a single DA polynomial at the given real-valued point.
///
/// For each monomial c * x1^e1 * x2^e2 * ... we substitute the values from
/// `args` (1-based variable indices mapped to 0-based slice positions) and
/// sum all contributions.
fn evaluate_da_at_re(poly: &DA, args: &[f64], na: usize) -> Result<f64> {
    let mut result = 0.0_f64;

    for (monomial, coeff) in poly.coeffs_iter().into_iter() {
        let exponents = &monomial.exponents;
        let mut term = coeff;

        for (var_idx, &exp) in exponents.iter().enumerate() {
            if exp == 0 {
                continue;
            }
            if var_idx >= na || var_idx >= args.len() {
                bail!(
                    "POLVAL: variable index {} out of range (NA={})",
                    var_idx + 1,
                    na
                );
            }
            term *= args[var_idx].powi(exp as i32);
        }

        result += term;
    }

    Ok(result)
}
