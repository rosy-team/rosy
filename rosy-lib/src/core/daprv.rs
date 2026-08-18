//! DAPRV and DAREV - DA vector print and read routines.
//!
//! DAPRV writes an array of DA vectors in COSY-format tabular output.
//! DAREV reads an array of DA vectors back from that format.

use anyhow::{Result, Context, bail};

use crate::taylor::{DA, get_config, get_runtime};
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
    array: &Vec<DA>,
    num_components: usize,
    _max_vars: usize,
    current_vars: usize,
    unit: u64,
) -> Result<()> {
    let output = format_daprv(array, num_components, _max_vars, current_vars)?;

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
/// COSY format (per component block):
///   - No header line
///   - Each non-zero term: `{coeff:17.12}     {exponents_concatenated}\n`
///   - Separator: ` ` + 78 dashes + `\n`
fn format_daprv(
    array: &Vec<DA>,
    num_components: usize,
    _max_vars: usize,
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

    // Sort by total order ascending, then reverse-lexicographic on exponents
    all_monomials.sort_by(|m1, m2| {
        m1.total_order.cmp(&m2.total_order)
            .then_with(|| {
                for i in (0..m1.exponents.len()).rev() {
                    match m1.exponents[i].cmp(&m2.exponents[i]) {
                        std::cmp::Ordering::Equal => continue,
                        ord => return ord,
                    }
                }
                std::cmp::Ordering::Equal
            })
    });

    // One block per component (single-column COSY format)
    let nv = current_vars.min(6);
    for comp_idx in 0..num_components.min(array.len()) {
        for monomial in &all_monomials {
            let coeff = array[comp_idx].get_coeff(monomial);
            if coeff.abs() <= epsilon {
                continue;
            }
            let exp_str: String = monomial.exponents[..nv]
                .iter()
                .map(|&e| char::from_digit(e as u32, 10).unwrap_or('?'))
                .collect();
            output.push_str(&format!("{:17.12}     {}\n", coeff, exp_str));
        }
        output.push_str(&format!(" {}\n", "-".repeat(78)));
    }

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
    _max_vars: usize,
    current_vars: usize,
    unit: u64,
) -> Result<()> {
    // Ensure array is big enough and zeroed
    while array.len() < num_components {
        array.push(DA::zero());
    }
    for i in 0..num_components.min(array.len()) {
        array[i] = DA::zero();
    }

    let nv = current_vars.min(6);

    // Read one block per component; each block ends with a separator line (all dashes)
    for comp_idx in 0..num_components.min(array.len()) {
        loop {
            let line = crate::core::file_io::rosy_read_from_unit(unit)
                .context("Failed to read line in DAREV")?;
            let trimmed = line.trim();

            // Separator line (all dashes) ends this component's block
            if trimmed.chars().all(|c| c == '-') && !trimmed.is_empty() {
                break;
            }
            if trimmed.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = trimmed.split_whitespace().collect();
            if tokens.len() < 2 {
                continue;
            }

            let coeff: f64 = tokens[0].parse().unwrap_or(0.0);
            // Exponents are concatenated single digits per variable, e.g. "10" = x1=1, x2=0
            let mut exponents = [0u8; 6];
            for (i, ch) in tokens[1].chars().enumerate().take(nv) {
                exponents[i] = ch.to_digit(10).unwrap_or(0) as u8;
            }
            let monomial = Monomial::new(exponents);

            if coeff.abs() > 1e-15 {
                array[comp_idx].set_coeff(monomial, coeff);
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
pub fn rosy_daplu(da_in: &Vec<DA>, var_idx: usize, c: f64, result: &mut Vec<DA>) -> Result<()> {
    use rustc_hash::FxHashMap;
    

    let config = get_config().context("DAPLU requires DA to be initialized (call OV first)")?;
    let var_0idx = var_idx
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("DAPLU: var_idx must be >= 1, got {}", var_idx))?;
    if var_0idx >= config.num_vars {
        bail!("DAPLU: var_idx {} out of range [1, {}]", var_idx, config.num_vars);
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
        bail!("DADIU: var_idx {} out of range [1, {}]", var_idx, config.num_vars);
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
    let da_ref = da.first().context("DACLIW: DA vector is empty")?;

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
