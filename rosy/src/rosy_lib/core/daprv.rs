//! DAPRV and DAREV - DA vector print and read routines.
//!
//! DAPRV writes an array of DA vectors in COSY-format tabular output.
//! DAREV reads an array of DA vectors back from that format.

use anyhow::{Context, Result, bail};

use crate::rosy_lib::core::display::RosyDisplay;
use crate::rosy_lib::taylor::Monomial;
use crate::rosy_lib::taylor::da::DACoefficient;
use crate::rosy_lib::taylor::{DA, cosy_display_rank, get_config};

/// Write an array of DA vectors in COSY INFINITY DAPRV format.
///
/// Arguments:
/// - `array`: the DA vector array (`Vec<DA>`)
/// - `num_components`: number of components to print
/// - `max_vars`: maximum number of variables in the expansion
/// - `current_vars`: current number of main variables
/// - `unit`: output unit number (6 = stdout, otherwise file unit)
pub fn rosy_daprv(
    array: &Vec<DA>,
    num_components: usize,
    max_vars: usize,
    current_vars: usize,
    unit: u64,
) -> Result<()> {
    let output = format_daprv(array, num_components, max_vars, current_vars)?;

    if unit == 6 {
        print!("{}", output);
    } else {
        // Write to file
        // Write without the trailing newline that write_to_unit adds
        for line in output.lines() {
            crate::rosy_lib::core::file_io::rosy_write_to_unit(unit, line)?;
        }
    }

    Ok(())
}

/// Format DAPRV output in COSY-compatible format.
fn format_daprv(
    array: &Vec<DA>,
    num_components: usize,
    max_vars: usize,
    current_vars: usize,
) -> Result<String> {
    let mut output = String::new();

    // Collect all unique monomials from all components
    let mut all_monomials: Vec<Monomial> = Vec::new();
    for i in 0..num_components.min(array.len()) {
        for (m, _) in array[i].coeffs_iter() {
            if !all_monomials.contains(&m) {
                all_monomials.push(m);
            }
        }
    }

    let sort_vars = current_vars.max(1).min(max_vars).min(6);
    all_monomials.sort_by_cached_key(|m| {
        (
            m.total_order,
            cosy_display_rank(&m.exponents, sort_vars),
            m.exponents,
        )
    });

    // If no monomials, print a zero entry
    if all_monomials.is_empty() {
        all_monomials.push(Monomial::constant());
    }

    // Print header
    output.push_str(&format!("  I  COEFFICIENT          "));
    for comp in 1..=num_components.min(array.len()) {
        output.push_str(&format!("     {:>2}             ", comp));
    }
    output.push_str("ORDER EXPONENTS\n");

    // Print each monomial row
    for (idx, monomial) in all_monomials.iter().enumerate() {
        let order = monomial.total_order;
        let exp_str = build_exp_str(&monomial.exponents, max_vars);

        output.push_str(&format!("{:>3}  ", idx + 1));

        // Print coefficient for each component
        for i in 0..num_components.min(array.len()) {
            let coeff = array[i].get_coeff(monomial);
            output.push_str(&format!("{} ", coeff.rosy_display()));
        }

        output.push_str(&format!("{:>5}   {}\n", order, exp_str));
    }

    // Print separator
    let sep_len = 30 + num_components.min(array.len()) * 24;
    output.push_str(&"-".repeat(sep_len.min(132)));
    output.push('\n');

    Ok(output)
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
    array: &mut Vec<DA>,
    num_components: usize,
    max_vars: usize,
    _current_vars: usize,
    unit: u64,
) -> Result<()> {
    // Read lines from the file
    let mut lines = Vec::new();

    // Read the header line
    let _header = crate::rosy_lib::core::file_io::rosy_read_from_unit(unit)
        .context("Failed to read header line in DAREV")?;

    // Read coefficient lines until we hit the separator
    loop {
        let line = crate::rosy_lib::core::file_io::rosy_read_from_unit(unit)
            .context("Failed to read line in DAREV")?;

        // Check if this is a separator line (all dashes)
        if line.trim().chars().all(|c| c == '-') && !line.trim().is_empty() {
            break;
        }

        lines.push(line);
    }

    // Ensure array is big enough
    while array.len() < num_components {
        array.push(DA::zero());
    }

    // Zero out the components we're reading into
    for i in 0..num_components.min(array.len()) {
        array[i] = DA::zero();
    }

    // Parse each line
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse the line: index, coefficients, order, exponents
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() < 2 + num_components {
            continue; // Skip malformed lines
        }

        // First token is the index (1-based), skip it
        // Next num_components tokens are coefficients
        // Then order
        // Then exponents
        let mut coeffs = Vec::new();
        for i in 0..num_components {
            if let Ok(coeff) = tokens[1 + i].parse::<f64>() {
                coeffs.push(coeff);
            } else {
                coeffs.push(0.0);
            }
        }

        // Order is after the coefficients
        let order_idx = 1 + num_components;
        if order_idx >= tokens.len() {
            continue;
        }

        // Exponents start after order
        let exp_start = order_idx + 1;
        let mut exponents = [0u8; 6];
        for i in 0..max_vars.min(6) {
            if exp_start + i < tokens.len() {
                if let Ok(exp) = tokens[exp_start + i].parse::<u8>() {
                    exponents[i] = exp;
                }
            }
        }

        let monomial = Monomial::new(exponents);

        // Set coefficients for each component
        for (i, &coeff) in coeffs.iter().enumerate() {
            if i < array.len() && coeff.abs() > 1e-15 {
                array[i].set_coeff(monomial, coeff);
            }
        }
    }

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
    use crate::rosy_lib::taylor::MAX_VARS;

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
            let a_i = if arr_idx < scales.len() {
                scales[arr_idx]
            } else {
                1.0
            };
            let c_i = if arr_idx < shifts.len() {
                shifts[arr_idx]
            } else {
                0.0
            };

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
            let x_i = DA::variable(var_idx).with_context(|| {
                format!("DATRN: failed to create identity DA variable {}", var_idx)
            })?;
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
                    power = (&power * &substitutions[var_0idx]).with_context(|| {
                        format!(
                            "DATRN: failed to multiply DA powers for var {}",
                            var_0idx + 1
                        )
                    })?;
                }
                term = (&term * &power).with_context(|| {
                    format!(
                        "DATRN: failed to multiply term by power for var {}",
                        var_0idx + 1
                    )
                })?;
            }

            // Accumulate into result
            result = (result + term)
                .with_context(|| "DATRN: failed to accumulate result DA".to_string())?;
        }

        output[comp_idx] = result;
    }

    Ok(())
}

/// Build exponent string for DAPRV display.
fn build_exp_str(exponents: &[u8], num_vars: usize) -> String {
    let mut result = String::new();
    for i in 0..num_vars.min(exponents.len()) {
        if i % 2 == 0 {
            result.push_str(&format!("{:>2}", exponents[i]));
        } else {
            result.push_str(&format!("{:>2} ", exponents[i]));
        }
    }
    result.trim_end().to_string()
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
pub fn rosy_daplu(da_in: &Vec<DA>, var_idx: usize, c: f64, result: &mut Vec<DA>) -> Result<()> {
    use crate::rosy_lib::taylor::MAX_VARS;
    use rustc_hash::FxHashMap;

    let config = get_config().context("DAPLU requires DA to be initialized (call OV first)")?;
    let var_0idx = var_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DAPLU: var_idx must be >= 1, got {}", var_idx))?;
    if var_0idx >= config.num_vars {
        bail!(
            "DAPLU: var_idx {} out of range [1, {}]",
            var_idx,
            config.num_vars
        );
    }

    result.resize_with(da_in.len(), DA::zero);

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

        result[comp_idx] = DA::from_coeffs(accum);
    }

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
pub fn rosy_dadiu(var_idx: usize, da_in: &Vec<DA>, result: &mut Vec<DA>) -> Result<()> {
    use rustc_hash::FxHashMap;

    let config = get_config().context("DADIU requires DA to be initialized (call OV first)")?;
    let var_0idx = var_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DADIU: var_idx must be >= 1, got {}", var_idx))?;
    if var_0idx >= config.num_vars {
        bail!(
            "DADIU: var_idx {} out of range [1, {}]",
            var_idx,
            config.num_vars
        );
    }

    result.resize_with(da_in.len(), DA::zero);

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

        result[comp_idx] = DA::from_coeffs(accum);
    }

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
        bail!(
            "DADMU: var_i {} out of range [1, {}]",
            var_i,
            config.num_vars
        );
    }
    if j_0idx >= config.num_vars {
        bail!(
            "DADMU: var_j {} out of range [1, {}]",
            var_j,
            config.num_vars
        );
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
pub fn rosy_dacliw(da: &Vec<DA>, n: usize, linear: &mut Vec<f64>) -> Result<()> {
    let config = get_config().context("DACLIW requires DA to be initialized (call OV first)")?;

    let da_ref = da.first().context("DACLIW: DA vector is empty")?;

    linear.resize(n, 0.0);

    for i in 0..n {
        if i < config.num_vars {
            let mono = Monomial::variable(i);
            linear[i] = da_ref.get_coeff(&mono);
        } else {
            linear[i] = 0.0;
        }
    }

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
    use crate::rosy_lib::taylor::MAX_VARS;

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
