//! DAREA, DAPEW, DAPEE, DAPEA, DAPEP, DAEST — DA power expansion and file I/O routines.
//!
//! - **DAREA**: Read a single DA vector from a file unit.
//! - **DAPEW**: Print DA terms filtered by variable index and order.
//! - **DAPEE**: Extract DA coefficient by TRANSPORT notation id.
//! - **DAPEA**: Extract DA coefficient by explicit exponent array.
//! - **DAPEP**: Extract parameter-dependent component of a DA.
//! - **DAEST**: Estimate the size (summation norm) of j-th order terms.

use anyhow::{Context, Result};

use crate::core::display::RosyDisplay;
use crate::taylor::{DA, MAX_VARS, get_runtime};
use crate::taylor::Monomial;

// ============================================================================
// Helpers
// ============================================================================

/// Decode a TRANSPORT notation id to a fixed-size exponent array.
///
/// Each decimal digit of `id` is the exponent for the corresponding variable
/// (leftmost digit = variable 1).  For example:
/// - `id = 133` → `[1, 3, 3, 0, 0, 0]`  (x₁¹ · x₂³ · x₃³)
/// - `id = 10`  → `[1, 0, 0, 0, 0, 0]`  (x₁¹)
fn decode_transport_id(id: u64) -> [u8; MAX_VARS] {
    let mut exponents = [0u8; MAX_VARS];
    let s = id.to_string();
    for (i, ch) in s.chars().enumerate() {
        if i >= MAX_VARS {
            break;
        }
        exponents[i] = ch.to_digit(10).unwrap_or(0) as u8;
    }
    exponents
}

// ============================================================================
// DAREA
// ============================================================================

/// Read a single DA vector from a file unit (inverse of one-component DAPRV).
///
/// The file must contain data in DAPRV format written by [`crate::core::daprv::rosy_daprv`].
///
/// Arguments:
/// - `unit`: unit number to read from
/// - `da`: DA array — element 0 is overwritten with the data read
/// - `num_vars`: number of independent variables
pub fn rosy_darea(unit: u64, da: &mut Vec<DA>, num_vars: usize) -> Result<()> {
    use crate::core::file_io::rosy_read_from_unit;

    // Ensure the output array has at least one element and is zeroed
    while da.is_empty() {
        da.push(DA::zero());
    }
    da[0] = DA::zero();

    let nv = num_vars.min(MAX_VARS);

    // Read COSY DAPRV format: coefficient lines until separator (all dashes)
    loop {
        let line = rosy_read_from_unit(unit).context("Failed to read data line in DAREA")?;
        let trimmed = line.trim();

        // Separator line ends the block
        if trimmed.chars().all(|c| c == '-') && !trimmed.is_empty() {
            break;
        }
        if trimmed.is_empty() {
            continue;
        }

        // COSY DAPRV format: `{coeff:17.12}     {concatenated_exps}`
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() < 2 {
            continue;
        }

        let coeff: f64 = tokens[0].parse().unwrap_or(0.0);
        if coeff.abs() <= 1e-15 {
            continue;
        }

        // Exponents are concatenated single digits, e.g. "10" = x1=1, x2=0
        let mut exponents = [0u8; MAX_VARS];
        for (i, ch) in tokens[1].chars().enumerate().take(nv) {
            exponents[i] = ch.to_digit(10).unwrap_or(0) as u8;
        }

        let monomial = Monomial::new(exponents);
        da[0].set_coeff(monomial, coeff);
    }

    Ok(())
}

// ============================================================================
// DAPEW
// ============================================================================

/// Print the terms of a DA vector that have exponent `order_n` in variable `var_i`.
///
/// If `var_i == 0` all terms of total order `order_n` are printed.
///
/// Arguments:
/// - `unit`: output unit (6 = stdout)
/// - `da`: DA array — operates on element 0
/// - `var_i`: 1-based variable index (0 = filter by total order)
/// - `order_n`: the target order/exponent value
pub fn rosy_dapew(unit: u64, da: &Vec<DA>, var_i: usize, order_n: u32) -> Result<()> {
    use crate::core::file_io::rosy_write_to_unit;

    if da.is_empty() {
        return Ok(());
    }

    let num_vars = get_runtime()
        .context("DAPEW requires DA initialized")?
        .config.num_vars;

    let da0 = &da[0];
    let mut output = String::new();

    // COSY DAPEW header format
    output.push_str(&format!(
        "        ORDER{:>4}  IN COLUMN{:>5}\n",
        order_n, var_i
    ));
    output.push_str("     I  COEFFICIENT          ORDER EXPONENTS\n");

    let mut terms: Vec<(Monomial, f64)> = da0
        .coeffs_iter()
        .into_iter()
        .filter(|(monomial, _coeff)| {
            if var_i > 0 && var_i <= MAX_VARS {
                monomial.exponents[var_i - 1] as u32 == order_n
            } else {
                monomial.total_order as u32 == order_n
            }
        })
        .collect();

    // Sort by total order ascending then lexicographic
    terms.sort_by(|(a, _), (b, _)| {
        a.total_order
            .cmp(&b.total_order)
            .then_with(|| a.exponents.cmp(&b.exponents))
    });

    for (idx, (monomial, coeff)) in terms.iter().enumerate() {
        let order = monomial.total_order;
        let exp_parts: Vec<String> = (0..num_vars.min(MAX_VARS))
            .map(|i| format!("{:>2}", monomial.exponents[i]))
            .collect();
        let exp_str = exp_parts.join(" ");
        output.push_str(&format!(
            "  {:>4}  {} {:>5}   {}\n",
            idx + 1,
            coeff.rosy_display(),
            order,
            exp_str
        ));
    }

    // COSY separator: 5 spaces + 35 dashes
    output.push_str("     ");
    output.push_str(&"-".repeat(35));
    output.push('\n');

    if unit == 6 {
        print!("{}", output);
    } else {
        for line in output.lines() {
            rosy_write_to_unit(unit, line)?;
        }
    }

    Ok(())
}

// ============================================================================
// DAPEE
// ============================================================================

/// Return the coefficient of a DA vector identified by a TRANSPORT notation id.
///
/// The id encodes variable exponents as decimal digits (leftmost = variable 1).
/// Example: `id = 133` → monomial x₁¹ · x₂³ · x₃³.
///
/// Arguments:
/// - `da`: DA array — operates on element 0
/// - `id`: TRANSPORT notation integer
/// - `result`: written with the extracted coefficient
pub fn rosy_dapee(da: &impl crate::AsDaRef, id: impl crate::AsF64, result: &mut impl crate::SetF64) -> Result<()> {
    let da = da.as_da_vec();
    let id = crate::rosy_as_u64(&id);
    if da.is_empty() {
        result.set_f64(0.0);
        return Ok(());
    }
    let exponents = decode_transport_id(id);
    let monomial = Monomial::new(exponents);
    result.set_f64(da[0].get_coeff(&monomial));
    Ok(())
}

// ============================================================================
// DAPEA
// ============================================================================

/// Return the coefficient of a DA vector identified by an explicit exponent array.
///
/// Arguments:
/// - `da`: DA array — operates on element 0
/// - `exps`: array of exponents (RE values, cast to u8)
/// - `size`: number of exponents to use (at most MAX_VARS)
/// - `result`: written with the extracted coefficient
pub fn rosy_dapea(da: &Vec<DA>, exps: &Vec<f64>, size: usize, result: &mut f64) -> Result<()> {
    if da.is_empty() {
        *result = 0.0;
        return Ok(());
    }
    let mut exponents = [0u8; MAX_VARS];
    for i in 0..size.min(MAX_VARS) {
        if i < exps.len() {
            exponents[i] = exps[i] as u8;
        }
    }
    let monomial = Monomial::new(exponents);
    *result = da[0].get_coeff(&monomial);
    Ok(())
}

// ============================================================================
// DAPEP
// ============================================================================

/// Extract a parameter-dependent component from a DA vector.
///
/// Collects all terms whose first `m` variable exponents match the pattern
/// encoded by `id` (TRANSPORT notation), then strips those exponents and
/// returns the residual polynomial in the result DA.
///
/// Example: DA = 2·x₁ + 3·x₁·x₂ + 4·x₂, id=1, m=1 →
///          result = 2 + 3·x₂  (terms with x₁¹, x₁ factor stripped).
///
/// Arguments:
/// - `da`: DA array — operates on element 0
/// - `id`: TRANSPORT notation id for the first `m` variables
/// - `m`: number of main variables to match
/// - `result`: DA array written with the extracted component (element 0)
pub fn rosy_dapep(
    da: &impl crate::AsDaRef,
    id: impl crate::AsF64,
    m: impl crate::IntoF64,
    result: &mut impl crate::AsDaDst,
) -> Result<()> {
    let da = da.as_da_vec();
    let id = crate::rosy_as_u64(&id);
    let m = crate::rosy_as_usize(&m.into_f64());
    let mut out = result.load_da_vec();
    while out.is_empty() {
        out.push(DA::zero());
    }
    out[0] = DA::zero();

    if da.is_empty() {
        result.store_da_vec(out);
        return Ok(());
    }

    let target = decode_transport_id(id);

    for (monomial, coeff) in da[0].coeffs_iter() {
        // Check whether the first m variables match the target exponents
        let mut matches = true;
        for i in 0..m.min(MAX_VARS) {
            if monomial.exponents[i] != target[i] {
                matches = false;
                break;
            }
        }

        if matches {
            // Strip the matched exponents from the first m variables
            let mut new_exps = monomial.exponents;
            for i in 0..m.min(MAX_VARS) {
                new_exps[i] = 0;
            }
            let new_monomial = Monomial::new(new_exps);
            let existing = out[0].get_coeff(&new_monomial);
            out[0].set_coeff(new_monomial, existing + coeff);
        }
    }
    result.store_da_vec(out);

    Ok(())
}

// ============================================================================
// DAEST
// ============================================================================

/// Estimate the size of j-th order terms of a DA vector (summation norm).
///
/// - If `i > 0`: sums |coeff| for all terms where the exponent of variable i equals j.
/// - If `i == 0`: sums |coeff| for all terms of total order j.
///
/// Arguments:
/// - `da`: DA array — operates on element 0
/// - `i`: variable index (1-based; 0 = aggregate over all variables at order j)
/// - `j`: target order
/// - `result`: written with the summation norm estimate
pub fn rosy_daest(da: &Vec<DA>, i: usize, j: u32, result: &mut f64) -> Result<()> {
    *result = 0.0;
    if da.is_empty() {
        return Ok(());
    }

    for (monomial, coeff) in da[0].coeffs_iter() {
        let matches = if i > 0 && i <= MAX_VARS {
            monomial.exponents[i - 1] as u32 == j
        } else {
            monomial.total_order as u32 == j
        };

        if matches {
            *result += coeff.abs();
        }
    }

    Ok(())
}
