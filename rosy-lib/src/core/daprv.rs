//! DAPRV and DAREV - DA vector print and read routines.
//!
//! DAPRV writes an array of DA vectors in COSY-format tabular output.
//! DAREV reads an array of DA vectors back from that format.

use anyhow::{Result, Context, bail};

use crate::taylor::{DA, cosy_display_rank, get_config, get_runtime};
use crate::taylor::Monomial;

/// Write an array of DA vectors in COSY INFINITY DAPRV format.
///
/// Arguments:
/// - `array`: the DA vector array (`Vec<DA>`)
/// - `num_components`: number of components to print
/// - `max_vars`: maximum number of variables in the expansion
/// - `current_vars`: current number of main variables
/// - `unit`: output unit number (6 = stdout, otherwise file unit)
pub fn rosy_daprv(
    array: &impl crate::AsDaRef,
    num_components: impl crate::IntoF64,
    _max_vars: impl crate::IntoF64,
    current_vars: impl crate::IntoF64,
    unit: impl crate::AsF64,
) -> Result<()> {
    let array = array.as_da_vec();
    let num_components = crate::rosy_as_usize(&num_components.into_f64());
    let _max_vars = crate::rosy_as_usize(&_max_vars.into_f64());
    let current_vars = crate::rosy_as_usize(&current_vars.into_f64());
    let unit = crate::rosy_as_u64(&unit);
    let output = format_daprv(&array, num_components, _max_vars, current_vars)?;

    if unit == 6 {
        print!("{}", output);
    } else {
        // Write to file
        // Write without the trailing newline that write_to_unit adds
        for line in output.lines() {
            crate::core::file_io::rosy_write_to_unit(unit, line)?;
        }
    }

    Ok(())
}

/// Format DAPRV output in COSY INFINITY-compatible format.
///
/// COSY format (one row per monomial):
///   - No header line
///   - One 14-column G14.7 coefficient field per component
///   - Concatenated exponents after the coefficient fields
///   - Separator: ` ` + 78 dashes + `\n`
/// Printed/parsed exponent slots: COSY uses min(max_vars, 6), padding zeros.
fn daprv_exponent_digits(max_vars: usize) -> usize {
    max_vars.min(crate::taylor::MAX_VARS)
}

/// Format the subset of Fortran `G14.7` used by COSY's DAPRV output.
///
/// In its fixed-point range, Fortran reserves four trailing columns where an
/// exponent would otherwise be printed. Outside that range it uses a
/// normalized `0.xxxxxxxE+xx` mantissa rather than Rust's `1.xxxxxxxe+xx`.
fn format_cosy_g14_7(value: f64) -> String {
    const WIDTH: usize = 14;
    const SIGNIFICANT_DIGITS: i32 = 7;

    if !value.is_finite() {
        return format!("{value:>WIDTH$}");
    }

    let magnitude = value.abs();
    if (0.1..10_000_000.0).contains(&magnitude) {
        let digits_before_decimal = magnitude.log10().floor() as i32 + 1;
        let precision = (SIGNIFICANT_DIGITS - digits_before_decimal).max(0) as usize;
        let fixed = format!("{value:>width$.precision$}", width = WIDTH - 4);
        let result = format!("{fixed}    ");
        if result.len() <= WIDTH {
            return result;
        }
    }

    let (mut mantissa, mut exponent) = if magnitude == 0.0 {
        (0.0, 0)
    } else {
        let exponent = magnitude.log10().floor() as i32 + 1;
        (value / 10_f64.powi(exponent), exponent)
    };

    // Account for a mantissa which rounds across the normalization boundary.
    mantissa = (mantissa * 10_000_000.0).round() / 10_000_000.0;
    if mantissa.abs() >= 1.0 {
        mantissa /= 10.0;
        exponent += 1;
    }

    let exponent_field = format!(
        "E{}{:02}",
        if exponent < 0 { '-' } else { '+' },
        exponent.abs()
    );
    let scientific = format!("{mantissa:.7}{exponent_field}");
    if scientific.len() > WIDTH {
        "*".repeat(WIDTH)
    } else {
        format!("{scientific:>WIDTH$}")
    }
}

fn format_daprv(
    array: &Vec<DA>,
    num_components: usize,
    max_vars: usize,
    current_vars: usize,
) -> Result<String> {
    let epsilon = get_runtime()
        .context("DAPRV requires DA to be initialized (call OV first)")?
        .config.epsilon;

    let mut output = String::new();

    // Collect all unique monomials across all components
    let mut all_monomials: Vec<Monomial> = Vec::new();
    for i in 0..num_components.min(array.len()) {
        for (m, _) in array[i].coeffs_iter() {
            if !all_monomials.contains(&m) {
                all_monomials.push(m);
            }
        }
    }

    let sort_vars = current_vars.max(1).min(6);
    all_monomials.sort_by_cached_key(|m| {
        (
            m.total_order,
            cosy_display_rank(&m.exponents, sort_vars),
            m.exponents,
        )
    });

    // One row per monomial; each component is one G14.7 field.
    let nv = daprv_exponent_digits(max_vars);
    let components = num_components.min(array.len());
    for monomial in &all_monomials {
        if !(0..components).any(|comp_idx| array[comp_idx].get_coeff(monomial).abs() > epsilon) {
            continue;
        }

        output.push(' ');
        for component in array.iter().take(components) {
            let coeff = component.get_coeff(monomial);
            let coeff = if coeff.abs() <= epsilon { 0.0 } else { coeff };
            output.push_str(&format_cosy_g14_7(coeff));
        }
        output.push(' ');
        for i in 0..nv {
            let exponent = monomial.exponents.get(i).copied().unwrap_or(0);
            output.push(char::from_digit(exponent as u32, 10).unwrap_or('?'));
        }
        output.push('\n');
    }
    output.push_str(&format!(" {}\n", "-".repeat(78)));

    Ok(output)
}

fn is_daprv_separator(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c == '-')
}

fn daprv_exponents(token: &str, nv: usize) -> Monomial {
    let mut exponents = [0u8; crate::taylor::MAX_VARS];
    for (i, ch) in token.chars().enumerate().take(nv) {
        exponents[i] = ch.to_digit(10).unwrap_or(0) as u8;
    }
    Monomial::new(exponents)
}

fn parse_daprv_coefficient(field: &str) -> Result<f64> {
    field
        .trim()
        .replace(['D', 'd'], "E")
        .parse()
        .with_context(|| format!("Invalid DAPRV coefficient field '{field}'"))
}

fn exponent_token_is_digits(token: &str, nv: usize) -> bool {
    !token.is_empty()
        && token.len() <= nv.max(1)
        && token.chars().all(|c| c.is_ascii_digit())
}

/// COSY row: optional leading pad, `components` G14.7 fields, then `nv` digit exponents.
fn is_cosy_daprv_row(line: &str, components: usize, nv: usize) -> bool {
    let line = line.trim_end();
    let body = line.strip_prefix(' ').unwrap_or(line);
    let coeff_end = 14 * components;
    if body.len() < coeff_end {
        return false;
    }
    for i in 0..components {
        let field = &body[i * 14..(i + 1) * 14];
        if field.chars().all(|c| c == '*') {
            continue;
        }
        if parse_daprv_coefficient(field).is_err() {
            return false;
        }
    }
    let rest = body[coeff_end..].trim();
    rest.len() == nv && rest.chars().all(|c| c.is_ascii_digit())
}

/// Legacy Rosy row: whitespace-separated coefficient and exponent token.
fn is_legacy_daprv_row(line: &str, nv: usize) -> bool {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    tokens.len() >= 2
        && parse_daprv_coefficient(tokens[0]).is_ok()
        && exponent_token_is_digits(tokens[1], nv)
}

fn parse_cosy_daprv_row(line: &str, components: usize, nv: usize) -> Result<(Monomial, Vec<f64>)> {
    let line = line.trim_end();
    let body = line.strip_prefix(' ').unwrap_or(line);
    let coeff_end = 14 * components;
    if body.len() < coeff_end {
        bail!("DAPRV row is too short for {components} components: '{line}'");
    }
    let exponent_token = body[coeff_end..]
        .split_whitespace()
        .next()
        .context("DAPRV row is missing its exponent field")?;
    if !exponent_token_is_digits(exponent_token, nv) && exponent_token.len() != nv {
        bail!("DAPRV row has a malformed exponent field '{exponent_token}'");
    }
    let monomial = daprv_exponents(exponent_token, nv);
    let mut coeffs = Vec::with_capacity(components);
    for i in 0..components {
        coeffs.push(parse_daprv_coefficient(&body[i * 14..(i + 1) * 14])?);
    }
    Ok((monomial, coeffs))
}

fn parse_legacy_daprv_row(line: &str, nv: usize) -> Result<(Monomial, f64)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        bail!("Legacy DAPRV row needs a coefficient and an exponent: '{line}'");
    }
    if !exponent_token_is_digits(tokens[1], nv) {
        bail!("Legacy DAPRV row has a malformed exponent field '{}'", tokens[1]);
    }
    let coeff = parse_daprv_coefficient(tokens[0])?;
    Ok((daprv_exponents(tokens[1], nv), coeff))
}

/// Read an array of DA vectors from COSY DAPRV format.
///
/// Arguments:
/// - `array`: the DA vector array to read into
/// - `num_components`: number of components to read
/// - `max_vars`: maximum number of variables
/// - `current_vars`: current number of main variables  
/// - `unit`: input unit number
pub fn rosy_darev(
    array: &mut impl crate::AsDaDst,
    num_components: impl crate::IntoF64,
    _max_vars: impl crate::IntoF64,
    current_vars: impl crate::IntoF64,
    unit: impl crate::AsF64,
) -> Result<()> {
    let num_components = crate::rosy_as_usize(&num_components.into_f64());
    let _max_vars = crate::rosy_as_usize(&_max_vars.into_f64());
    let _current_vars = crate::rosy_as_usize(&current_vars.into_f64());
    let unit = crate::rosy_as_u64(&unit);
    let dest = array;
    let mut array = dest.load_da_vec();
    // Ensure array is big enough and zeroed
    while array.len() < num_components {
        array.push(DA::zero());
    }
    for i in 0..num_components.min(array.len()) {
        array[i] = DA::zero();
    }

    let nv = daprv_exponent_digits(_max_vars);

    let components = num_components.min(array.len());
    let first_data_line = loop {
        let line = crate::core::file_io::rosy_read_from_unit(unit)
            .context("Failed to read line in DAREV")?;
        if is_daprv_separator(&line) {
            dest.store_da_vec(array);
            return Ok(());
        }
        if !line.trim().is_empty() {
            break line;
        }
    };

    let use_cosy = is_cosy_daprv_row(&first_data_line, components, nv);
    let use_legacy = is_legacy_daprv_row(&first_data_line, nv);
    if !use_cosy && !use_legacy {
        bail!(
            "DAREV could not recognize DAPRV layout in '{}'",
            first_data_line.trim_end()
        );
    }

    if use_legacy && !use_cosy {
        let mut comp_idx = 0;
        let mut line = first_data_line;
        loop {
            if is_daprv_separator(&line) {
                comp_idx += 1;
                if comp_idx >= components {
                    break;
                }
            } else if !line.trim().is_empty() {
                let (monomial, coeff) = parse_legacy_daprv_row(&line, nv)?;
                if coeff.abs() > 1e-15 {
                    array[comp_idx].set_coeff(monomial, coeff);
                }
            }
            line = crate::core::file_io::rosy_read_from_unit(unit)
                .context("Failed to read legacy Rosy DAPRV data in DAREV")?;
        }
    } else {
        let mut line = first_data_line;
        loop {
            if is_daprv_separator(&line) {
                break;
            }
            if !line.trim().is_empty() {
                let (monomial, coeffs) = parse_cosy_daprv_row(&line, components, nv)?;
                for (comp_idx, coeff) in coeffs.into_iter().enumerate() {
                    if coeff.abs() > 1e-15 {
                        array[comp_idx].set_coeff(monomial, coeff);
                    }
                }
            }
            line = crate::core::file_io::rosy_read_from_unit(unit)
                .context("Failed to read COSY DAPRV data in DAREV")?;
        }
    }
    dest.store_da_vec(array);

    Ok(())
}

/// DATRN: Transform independent variables x_i with a_i*x_i + c_i for i = m1..=m2
///
/// Arguments:
/// - `input`: the input DA vector array (`Vec<DA>`)
/// - `scales`: array of scale factors a_i (one per variable; 1-based indexing used via m1/m2)
/// - `shifts`: array of translation factors c_i
/// - `m1`: start index (1-based)
/// - `m2`: end index (1-based, inclusive)
/// - `output`: DA vector array to write results into
pub fn rosy_datrn(
    input: &Vec<DA>,
    scales: &Vec<f64>,
    shifts: &Vec<f64>,
    m1: usize,
    m2: usize,
    output: &mut Vec<DA>,
) -> Result<()> {
    use crate::taylor::MAX_VARS;

    let config = get_config().context("DATRN requires DA to be initialized (call OV first)")?;
    let num_vars = config.num_vars;

    // Build substitution DAs: for each variable i (1-based), build the DA for the new expression.
    // Variables outside [m1, m2] are identity: new_x_i = x_i.
    // Variables inside [m1, m2] become: new_x_i = a_i * x_i + c_i.
    let mut substitutions: Vec<DA> = Vec::with_capacity(num_vars);
    for var_idx in 1..=num_vars {
        if var_idx >= m1 && var_idx <= m2 {
            // Index into scales/shifts arrays (0-based offset from m1)
            let arr_idx = var_idx - m1;
            let a_i = if arr_idx < scales.len() { scales[arr_idx] } else { 1.0 };
            let c_i = if arr_idx < shifts.len() { shifts[arr_idx] } else { 0.0 };

            // Build: a_i * x_i + c_i
            let x_i = DA::variable(var_idx)
                .with_context(|| format!("DATRN: failed to create DA variable {}", var_idx))?;
            let scaled = (&x_i * a_i)
                .with_context(|| format!("DATRN: failed to scale DA variable {}", var_idx))?;
            let shifted = (scaled + DA::from_coeff(c_i))
                .with_context(|| format!("DATRN: failed to shift DA variable {}", var_idx))?;
            substitutions.push(shifted);
        } else {
            // Identity substitution: new_x_i = x_i
            let x_i = DA::variable(var_idx)
                .with_context(|| format!("DATRN: failed to create identity DA variable {}", var_idx))?;
            substitutions.push(x_i);
        }
    }

    // Resize output to match input
    output.resize_with(input.len(), DA::zero);

    // For each DA in input, perform polynomial composition
    for (comp_idx, da_in) in input.iter().enumerate() {
        let mut result = DA::zero();

        // Iterate over each term c * x_1^e1 * x_2^e2 * ... in the input DA
        for (monomial, coeff) in da_in.coeffs_iter() {
            if coeff.abs() <= config.epsilon {
                continue;
            }

            // Evaluate monomial at substituted variables:
            // Monomial contribution = coeff * prod_i (substitutions[i])^exponents[i]
            let mut term = DA::from_coeff(coeff);
            for var_0idx in 0..num_vars.min(MAX_VARS) {
                let exp = monomial.exponents[var_0idx] as usize;
                if exp == 0 {
                    continue;
                }
                // Raise substitution[var_0idx] to the power `exp`
                let mut power = DA::from_coeff(1.0);
                for _ in 0..exp {
                    power = (&power * &substitutions[var_0idx])
                        .with_context(|| format!("DATRN: failed to multiply DA powers for var {}", var_0idx + 1))?;
                }
                term = (&term * &power)
                    .with_context(|| format!("DATRN: failed to multiply term by power for var {}", var_0idx + 1))?;
            }

            // Accumulate into result
            result = (result + term)
                .with_context(|| "DATRN: failed to accumulate result DA".to_string())?;
        }

        output[comp_idx] = result;
    }

    Ok(())
}

/// DAPLU: Replace independent variable xi by constant C in a DA vector.
///
/// For each term c·x₁^e₁·…·xᵢ^eᵢ·…·xₙ^eₙ, the result accumulates
/// c·C^eᵢ into the monomial with the i-th exponent set to zero.
///
/// Arguments:
/// - `da_in`:   source DA array
/// - `var_idx`: 1-based index of the variable to substitute
/// - `c`:       constant value to substitute for xᵢ
/// - `result`:  output DA array
pub fn rosy_daplu(
    da_in: &impl crate::AsDaRef,
    var_idx: impl crate::IntoF64,
    c: impl crate::AsF64,
    result: &mut impl crate::AsDaDst,
) -> Result<()> {
    use rustc_hash::FxHashMap;
    let da_in = da_in.as_da_vec();
    let var_idx = crate::rosy_as_usize(&var_idx.into_f64());
    let c = c.as_f64_val();
    let mut out = result.load_da_vec();

    let config = get_config().context("DAPLU requires DA to be initialized (call OV first)")?;
    let var_0idx = var_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DAPLU: var_idx must be >= 1, got {}", var_idx))?;
    if var_0idx >= config.num_vars {
        bail!("DAPLU: var_idx {} out of range [1, {}]", var_idx, config.num_vars);
    }

    out.resize_with(da_in.len(), DA::zero);

    for (comp_idx, da) in da_in.iter().enumerate() {
        let mut accum: FxHashMap<Monomial, f64> = FxHashMap::default();

        for (monomial, coeff) in da.coeffs_iter() {
            if coeff.abs() <= config.epsilon {
                continue;
            }
            let e_v = monomial.exponents[var_0idx] as i32;
            let contribution = coeff * c.powi(e_v);
            if contribution.abs() <= config.epsilon {
                continue;
            }
            let mut new_exps = monomial.exponents;
            new_exps[var_0idx] = 0;
            let new_mono = Monomial::new(new_exps);
            *accum.entry(new_mono).or_insert(0.0) += contribution;
        }

        out[comp_idx] = DA::from_coeffs(accum);
    }
    result.store_da_vec(out);

    Ok(())
}

/// DADIU: Divide a DA vector by independent variable xi.
///
/// For each term whose xi-exponent ≥ 1, the result is the term with that
/// exponent decremented by 1. Terms without xi as a factor are dropped (return 0).
///
/// Arguments:
/// - `var_idx`: 1-based index of the variable to divide by
/// - `da_in`:   source DA array
/// - `result`:  output DA array
pub fn rosy_dadiu(
    var_idx: impl crate::IntoF64,
    da_in: &impl crate::AsDaRef,
    result: &mut impl crate::AsDaDst,
) -> Result<()> {
    use rustc_hash::FxHashMap;
    let var_idx = crate::rosy_as_usize(&var_idx.into_f64());
    let da_in = da_in.as_da_vec();
    let mut out = result.load_da_vec();

    let config = get_config().context("DADIU requires DA to be initialized (call OV first)")?;
    let var_0idx = var_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DADIU: var_idx must be >= 1, got {}", var_idx))?;
    if var_0idx >= config.num_vars {
        bail!("DADIU: var_idx {} out of range [1, {}]", var_idx, config.num_vars);
    }

    out.resize_with(da_in.len(), DA::zero);

    for (comp_idx, da) in da_in.iter().enumerate() {
        let mut accum: FxHashMap<Monomial, f64> = FxHashMap::default();

        for (monomial, coeff) in da.coeffs_iter() {
            if coeff.abs() <= config.epsilon {
                continue;
            }
            let e_v = monomial.exponents[var_0idx];
            if e_v == 0 {
                continue; // term not divisible by xi — dropped
            }
            let mut new_exps = monomial.exponents;
            new_exps[var_0idx] = e_v - 1;
            let new_mono = Monomial::new(new_exps);
            *accum.entry(new_mono).or_insert(0.0) += coeff;
        }

        out[comp_idx] = DA::from_coeffs(accum);
    }
    result.store_da_vec(out);

    Ok(())
}

/// DADMU: Divide a DA vector by xi then multiply by xj.
///
/// For each term whose xi-exponent ≥ 1, the result is the term with the
/// xi-exponent decremented and the xj-exponent incremented.
/// Terms not divisible by xi are dropped (return 0).
///
/// Arguments:
/// - `var_i`:  1-based index of the variable to divide by
/// - `var_j`:  1-based index of the variable to multiply by
/// - `da_in`:  source DA array
/// - `result`: output DA array
pub fn rosy_dadmu(var_i: usize, var_j: usize, da_in: &Vec<DA>, result: &mut Vec<DA>) -> Result<()> {
    use rustc_hash::FxHashMap;

    let config = get_config().context("DADMU requires DA to be initialized (call OV first)")?;
    let i_0idx = var_i
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DADMU: var_i must be >= 1, got {}", var_i))?;
    let j_0idx = var_j
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DADMU: var_j must be >= 1, got {}", var_j))?;
    if i_0idx >= config.num_vars {
        bail!("DADMU: var_i {} out of range [1, {}]", var_i, config.num_vars);
    }
    if j_0idx >= config.num_vars {
        bail!("DADMU: var_j {} out of range [1, {}]", var_j, config.num_vars);
    }

    result.resize_with(da_in.len(), DA::zero);

    for (comp_idx, da) in da_in.iter().enumerate() {
        let mut accum: FxHashMap<Monomial, f64> = FxHashMap::default();

        for (monomial, coeff) in da.coeffs_iter() {
            if coeff.abs() <= config.epsilon {
                continue;
            }
            let e_i = monomial.exponents[i_0idx];
            if e_i == 0 {
                continue; // not divisible by xi — dropped
            }
            let mut new_exps = monomial.exponents;
            new_exps[i_0idx] = e_i - 1;
            new_exps[j_0idx] = new_exps[j_0idx].saturating_add(1);
            // total_order is unchanged (div by xi cancels mul by xj)
            let new_mono = Monomial::new(new_exps);
            *accum.entry(new_mono).or_insert(0.0) += coeff;
        }

        result[comp_idx] = DA::from_coeffs(accum);
    }

    Ok(())
}

/// DACLIW: Extract the linear (first-order) coefficients of a DA.
///
/// The result array `linear[i]` receives the coefficient of xᵢ₊₁ (1-based)
/// in the first DA component. When order-weighted DA is in use, the weighted
/// linear coefficients are extracted.
///
/// Arguments:
/// - `da`:     source DA array (first component used)
/// - `n`:      number of linear coefficients to extract
/// - `linear`: output vector of size n
pub fn rosy_dacliw(
    da: &impl crate::AsDaRef,
    n: impl crate::IntoF64,
    linear: &mut impl crate::PolvalReDst,
) -> Result<()> {
    let config = get_config().context("DACLIW requires DA to be initialized (call OV first)")?;
    let da = da.as_da_vec();
    let n = crate::rosy_as_usize(&n.into_f64());
    let zero;
    let da_ref = match da.first() {
        Some(d) => d,
        None => {
            zero = DA::zero();
            &zero
        }
    };

    let mut out = linear.load_re_vec();
    out.resize(n, 0.0);

    for i in 0..n {
        if i < config.num_vars {
            let mono = Monomial::variable(i);
            out[i] = da_ref.get_coeff(&mono);
        } else {
            out[i] = 0.0;
        }
    }
    linear.store_re_vec(out);

    Ok(())
}

/// DACQLC: Extract coefficients up to second order from a DA.
///
/// Decomposes the first DA component as:  xᵀHx/2 + Lx + c
///
/// - `hessian[i][j]` = ∂²f/(∂xᵢ∂xⱼ) = coeff(xᵢxⱼ) for i≠j, 2·coeff(xᵢ²) for i=j
/// - `linear[i]`     = coeff of xᵢ₊₁
/// - `*constant`     = constant term
///
/// Arguments:
/// - `da`:       source DA array (first component used)
/// - `n`:        size of the linear and Hessian arrays
/// - `hessian`:  n×n output matrix
/// - `linear`:   n-element output vector
/// - `constant`: output scalar (constant term)
pub fn rosy_dacqlc(
    da: &Vec<DA>,
    n: usize,
    hessian: &mut Vec<Vec<f64>>,
    linear: &mut Vec<f64>,
    constant: &mut f64,
) -> Result<()> {
    use crate::taylor::MAX_VARS;

    let config = get_config().context("DACQLC requires DA to be initialized (call OV first)")?;

    let da_ref = da.first().context("DACQLC: DA vector is empty")?;

    // Constant term
    *constant = da_ref.get_coeff(&Monomial::constant());

    // Linear terms
    linear.resize(n, 0.0);
    for i in 0..n {
        if i < config.num_vars {
            linear[i] = da_ref.get_coeff(&Monomial::variable(i));
        } else {
            linear[i] = 0.0;
        }
    }

    // Quadratic (Hessian) terms
    hessian.resize_with(n, || vec![0.0; n]);
    for row in hessian.iter_mut() {
        row.resize(n, 0.0);
    }

    for i in 0..n.min(config.num_vars) {
        for j in 0..n.min(config.num_vars) {
            let coeff = if i == j {
                let mut exps = [0u8; MAX_VARS];
                exps[i] = 2;
                let mono = Monomial::new(exps);
                2.0 * da_ref.get_coeff(&mono)
            } else {
                let mut exps = [0u8; MAX_VARS];
                exps[i] = 1;
                exps[j] = 1;
                let mono = Monomial::new(exps);
                da_ref.get_coeff(&mono)
            };
            hessian[i][j] = coeff;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    use crate::taylor::{cleanup_taylor, init_taylor};

    #[test]
    fn exponent_digits_use_max_vars_capped_at_six() {
        assert_eq!(daprv_exponent_digits(2), 2);
        assert_eq!(daprv_exponent_digits(6), 6);
        assert_eq!(daprv_exponent_digits(8), 6);
    }

    #[test]
    fn g14_7_matches_cosy_fixed_and_scientific_fields() {
        assert_eq!(format_cosy_g14_7(0.9999439), " 0.9999439    ");
        assert_eq!(format_cosy_g14_7(-0.0001785528), "-0.1785528E-03");
        assert_eq!(format_cosy_g14_7(-0.01059182), "-0.1059182E-01");
        assert_eq!(format_cosy_g14_7(0.000001891319), " 0.1891319E-05");
        assert_eq!(format_cosy_g14_7(0.0), " 0.0000000E+00");
    }

    #[test]
    #[serial]
    fn format_daprv_puts_components_in_cosy_columns() {
        cleanup_taylor();
        init_taylor(2, 2).unwrap();

        let monomial = Monomial::variable(0);
        let mut map = vec![DA::zero(), DA::zero(), DA::zero(), DA::zero(), DA::zero()];
        for (component, coefficient) in map.iter_mut().zip([
            0.9999439,
            -0.0001785528,
            -0.01059182,
            0.000001891319,
            0.0,
        ]) {
            component.set_coeff(monomial.clone(), coefficient);
        }

        let out = format_daprv(&map, 5, 6, 2).unwrap();
        assert_eq!(
            out,
            concat!(
                "  0.9999439    -0.1785528E-03-0.1059182E-01 0.1891319E-05 0.0000000E+00 100000\n",
                " ------------------------------------------------------------------------------\n"
            )
        );

        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn format_daprv_pads_exponent_slots_when_max_vars_exceeds_current() {
        cleanup_taylor();
        init_taylor(2, 2).unwrap();

        let da = DA::variable(1).unwrap();
        let out = format_daprv(&vec![da], 1, 6, 2).unwrap();
        let exp_col: Vec<&str> = out
            .lines()
            .filter(|l| !l.trim().chars().all(|c| c == '-'))
            .filter_map(|l| l.split_whitespace().nth(1))
            .collect();

        assert_eq!(exp_col, vec!["100000"]);
        cleanup_taylor();
    }

    #[test]
    #[serial]
    fn darev_reads_the_same_padded_exponent_width() {
        cleanup_taylor();
        init_taylor(2, 2).unwrap();

        let nv = daprv_exponent_digits(6);
        let token = "100000";
        let mut exponents = [0u8; crate::taylor::MAX_VARS];
        for (i, ch) in token.chars().enumerate().take(nv) {
            exponents[i] = ch.to_digit(10).unwrap_or(0) as u8;
        }
        assert_eq!(nv, 6);
        assert_eq!(exponents[0], 1);
        assert!(exponents[1..].iter().all(|&e| e == 0));

        cleanup_taylor();
    }

    #[test]
    fn layout_detection_prefers_cosy_columns_over_line_length() {
        let cosy = "  0.9999439    -0.1785528E-03-0.1059182E-01 0.1891319E-05 0.0000000E+00 100000";
        assert!(is_cosy_daprv_row(cosy, 5, 6));
        assert!(!is_legacy_daprv_row(cosy, 6));

        let legacy = "  0.9999439 100000";
        assert!(!is_cosy_daprv_row(legacy, 5, 6));
        assert!(is_legacy_daprv_row(legacy, 6));

        let garbage = "hello world";
        assert!(!is_cosy_daprv_row(garbage, 5, 6));
        assert!(!is_legacy_daprv_row(garbage, 6));
    }
}
