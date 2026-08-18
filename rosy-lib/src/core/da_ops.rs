//! DA coefficient-level operations: DASCL, DASGN, DADER, DAINT, DANORO, DANORS,
//! DAFSET, DAFILT, DARAN, DAGMD, DACODE, DAFLO, CDFLO, DANOW, CDF2, CDNF, CDNFDA, CDNFDS.

use anyhow::{Context, Result, bail, ensure};
use num_complex::Complex64;

use crate::taylor::config::DERIV_INVALID;
use crate::taylor::da::{DA as GenericDA, DACoefficient};
use crate::taylor::{CD, DA, get_filter_da, get_runtime, set_filter_da};

/// DASCL: Scale all coefficients of every DA element in the array by `scalar`.
///
/// Equivalent to `da[i] *= scalar` for all terms i.
pub fn rosy_dascl(da: &mut Vec<DA>, scalar: f64) -> Result<()> {
    for da_el in da.iter_mut() {
        *da_el = (&*da_el * scalar).context("DASCL: failed to scale DA element")?;
    }
    Ok(())
}

/// DASGN: Negate all coefficients of every DA element in the array (sign flip).
///
/// Equivalent to `da[i] *= -1` for all terms i.
pub fn rosy_dasgn(da: &mut Vec<DA>) -> Result<()> {
    for da_el in da.iter_mut() {
        *da_el = -(&*da_el);
    }
    Ok(())
}

/// DADER: Differentiate every DA element in the array w.r.t. variable `var_index` (1-based).
///
/// For each monomial m_k with exponent e_v > 0 in variable v, the derivative contributes
/// `coeff_k * e_v` to the monomial with exponent v decremented by 1.
pub fn rosy_dader(da: &mut Vec<DA>, var_index: usize) -> Result<()> {
    // Acquire tables and release lock before building new DAs
    let (epsilon, _num_vars, deriv_target_v, deriv_exp_v) = {
        let rt = get_runtime().context("DADER requires DA to be initialized (call DAINI first)")?;
        let num_vars = rt.config.num_vars;
        if var_index == 0 || var_index > num_vars {
            bail!(
                "DADER: variable index {} out of range [1, {}]",
                var_index,
                num_vars
            );
        }
        let v = var_index - 1;
        let n = rt.num_monomials;
        let base = v * n;
        let deriv_target_v = rt.deriv_target[base..base + n].to_vec();
        let deriv_exp_v = rt.deriv_exponent[base..base + n].to_vec();
        (rt.config.epsilon, num_vars, deriv_target_v, deriv_exp_v)
    };

    for da_el in da.iter_mut() {
        // Snapshot: (flat_index, coefficient)
        let source: Vec<(usize, f64)> = da_el
            .nonzero
            .iter()
            .map(|&k| (k as usize, da_el.coeffs[k as usize]))
            .collect();

        let mut new_da = DA::zero();

        for (k, coeff_k) in source {
            let target = deriv_target_v[k];
            if target == DERIV_INVALID {
                continue; // exponent of v in monomial k is 0 → derivative is 0
            }
            let exp = deriv_exp_v[k] as f64;
            let new_coeff = coeff_k * exp;
            if new_coeff.abs() > epsilon {
                // Mapping is injective, so each target gets at most one contribution
                new_da.coeffs[target as usize] = new_coeff;
                new_da.nonzero.push(target);
            }
        }

        *da_el = new_da;
    }
    Ok(())
}

/// DAINT: Integrate every DA element in the array w.r.t. variable `var_index` (1-based).
///
/// For each monomial m_k with coefficient c_k, the integral contributes
/// `c_k / (e_v + 1)` to the monomial with exponent v incremented by 1.
/// Terms that would exceed the truncation order are dropped.
pub fn rosy_daint(da: &mut Vec<DA>, var_index: usize) -> Result<()> {
    // Acquire tables and release lock before building new DAs
    let (epsilon, _num_vars, integ_target_v, deriv_exp_v) = {
        let rt = get_runtime().context("DAINT requires DA to be initialized (call DAINI first)")?;
        let num_vars = rt.config.num_vars;
        if var_index == 0 || var_index > num_vars {
            bail!(
                "DAINT: variable index {} out of range [1, {}]",
                var_index,
                num_vars
            );
        }
        let v = var_index - 1;
        let n = rt.num_monomials;
        let base = v * n;
        let integ_target_v = rt.integ_target[base..base + n].to_vec();
        // deriv_exp_v[t] = exponent of v in monomial t = (e_v_in_source + 1)
        let deriv_exp_v = rt.deriv_exponent[base..base + n].to_vec();
        (rt.config.epsilon, num_vars, integ_target_v, deriv_exp_v)
    };

    for da_el in da.iter_mut() {
        let source: Vec<(usize, f64)> = da_el
            .nonzero
            .iter()
            .map(|&k| (k as usize, da_el.coeffs[k as usize]))
            .collect();

        let mut new_da = DA::zero();

        for (k, coeff_k) in source {
            let target = integ_target_v[k];
            if target == DERIV_INVALID {
                continue; // result would exceed truncation order
            }
            let t = target as usize;
            // The exponent of v in the target monomial equals (e_v_source + 1)
            let new_exp = deriv_exp_v[t] as f64;
            let new_coeff = coeff_k / new_exp;
            if new_coeff.abs() > epsilon {
                // Mapping is injective
                new_da.coeffs[t] = new_coeff;
                new_da.nonzero.push(target);
            }
        }

        *da_el = new_da;
    }
    Ok(())
}

/// DANORO: Remove all odd-order terms from every DA element in the array.
///
/// Keeps only terms whose total monomial order is even (0, 2, 4, …).
pub fn rosy_danoro(da: &mut Vec<DA>) -> Result<()> {
    let monomial_orders = {
        let rt =
            get_runtime().context("DANORO requires DA to be initialized (call DAINI first)")?;
        rt.monomial_orders.clone()
    };

    for da_el in da.iter_mut() {
        let mut i = 0;
        while i < da_el.nonzero.len() {
            let flat = da_el.nonzero[i] as usize;
            if monomial_orders[flat] % 2 == 1 {
                da_el.nonzero.swap_remove(i);
                da_el.coeffs[flat] = 0.0;
                // don't advance i — the swapped-in element needs to be checked
            } else {
                i += 1;
            }
        }
    }
    Ok(())
}

/// DANORS: Remove all coefficients whose absolute value is below `threshold`.
///
/// Keeps only terms with |c| >= threshold.
pub fn rosy_danors(da: &mut Vec<DA>, threshold: f64) -> Result<()> {
    for da_el in da.iter_mut() {
        let mut i = 0;
        while i < da_el.nonzero.len() {
            let flat = da_el.nonzero[i] as usize;
            if da_el.coeffs[flat].abs() < threshold {
                da_el.nonzero.swap_remove(i);
                da_el.coeffs[flat] = 0.0;
            } else {
                i += 1;
            }
        }
    }
    Ok(())
}

/// DAFSET: Set the DA filtering template used by DAFILT.
///
/// Pass `template = 0.0` (scalar 0) to disable filtering. Otherwise the
/// single DA value is used as the template for all components.
/// In Rosy, `template_da` is a `Vec<DA>` with at least one element;
/// pass an empty vec to disable.
pub fn rosy_dafset(template_da: Vec<DA>) -> Result<()> {
    if template_da.is_empty() || (template_da.len() == 1 && template_da[0].is_zero()) {
        set_filter_da(None)
    } else {
        set_filter_da(Some(template_da))
    }
}

/// DAFILT: Filter `input` through the template set by DAFSET, writing to `result`.
///
/// For each monomial index, a coefficient is kept only if the corresponding
/// monomial is nonzero in the template component (component 0 of the template
/// is used as a universal mask).
pub fn rosy_dafilt(input: &Vec<DA>, result: &mut Vec<DA>) -> Result<()> {
    let filter = get_filter_da()?;
    let epsilon = {
        let rt = get_runtime().context("DAFILT requires DA initialized")?;
        rt.config.epsilon
    };

    match filter {
        None => {
            // No filter set — copy input to result unchanged
            for (r, src) in result.iter_mut().zip(input.iter()) {
                *r = src.clone();
            }
        }
        Some(template) => {
            let mask = &template[0]; // use first template component as the monomial mask
            for (r, src) in result.iter_mut().zip(input.iter()) {
                // Keep only monomials that are nonzero in the mask
                let mut new_da = DA::zero();
                for &k in &src.nonzero {
                    if mask.coeffs[k as usize].abs() > epsilon {
                        new_da.coeffs[k as usize] = src.coeffs[k as usize];
                        new_da.nonzero.push(k);
                    }
                }
                *r = new_da;
            }
        }
    }
    Ok(())
}

/// DARAN: Fill every DA element in `da` with random coefficients in [-1, 1].
///
/// `sparsity` is the fraction of monomials that will be set nonzero
/// (0.0 = all zero, 1.0 = all filled). Uses the global Rosy RNG.
pub fn rosy_daran(da: &mut Vec<DA>, sparsity: f64) -> Result<()> {
    let (num_monomials, epsilon) = {
        let rt = get_runtime().context("DARAN requires DA to be initialized (call DAINI first)")?;
        (rt.num_monomials, rt.config.epsilon)
    };

    let sparsity = sparsity.clamp(0.0, 1.0);

    for da_el in da.iter_mut() {
        *da_el = DA::zero();
        for k in 0..num_monomials {
            if crate::core::rng::rng_f64() < sparsity {
                let val = crate::core::rng::rng_f64_symmetric();
                if val.abs() > epsilon {
                    da_el.coeffs[k] = val;
                    da_el.nonzero.push(k as u32);
                }
            }
        }
    }
    Ok(())
}

/// DAGMD: Compute the Lie derivative ∇g · f (gradient of g dotted with vector field f).
///
/// `result = Σᵢ (∂g/∂xᵢ) * f[i]`
///
/// Arguments:
/// - `g`: single-component DA (scalar field)
/// - `f`: array of DAs (vector field, `dim` components)
/// - `result`: single-component DA output
/// - `dim`: number of components of f
pub fn rosy_dagmd(g: &Vec<DA>, f: &Vec<DA>, result: &mut Vec<DA>, dim: usize) -> Result<()> {
    let (epsilon, num_vars, deriv_targets, deriv_exponents, n) = {
        let rt = get_runtime().context("DAGMD requires DA to be initialized (call DAINI first)")?;
        let num_vars = rt.config.num_vars;
        (
            rt.config.epsilon,
            num_vars,
            rt.deriv_target.clone(),
            rt.deriv_exponent.clone(),
            rt.num_monomials,
        )
    };

    if dim > num_vars {
        bail!(
            "DAGMD: dim ({}) exceeds number of DA variables ({})",
            dim,
            num_vars
        );
    }
    if f.len() < dim {
        bail!("DAGMD: f array has {} elements but dim={}", f.len(), dim);
    }

    // g is a single-component DA; we compute ∂g/∂xᵢ for each i, multiply by f[i], accumulate
    let g0 = g
        .get(0)
        .ok_or_else(|| anyhow::anyhow!("DAGMD: g is empty"))?;

    let mut acc = DA::zero();

    for i in 0..dim {
        // Compute ∂g/∂xᵢ (derivative w.r.t. variable i+1, 0-based: i)
        let v = i;
        let base = v * n;
        let deriv_target_v = &deriv_targets[base..base + n];
        let deriv_exp_v = &deriv_exponents[base..base + n];

        let mut dg_dxi = DA::zero();
        for &k in &g0.nonzero {
            let target = deriv_target_v[k as usize];
            if target == DERIV_INVALID {
                continue;
            }
            let exp = deriv_exp_v[k as usize] as f64;
            let new_coeff = g0.coeffs[k as usize] * exp;
            if new_coeff.abs() > epsilon {
                dg_dxi.coeffs[target as usize] = new_coeff;
                dg_dxi.nonzero.push(target);
            }
        }

        // Multiply dg_dxi by f[i] and add to accumulator
        let fi = &f[i];
        let product = (&dg_dxi * fi).context("DAGMD: multiplication failed")?;
        acc = (&acc + &product).context("DAGMD: addition failed")?;
    }

    if let Some(r) = result.get_mut(0) {
        *r = acc;
    } else {
        result.push(acc);
    }

    Ok(())
}

/// DACODE: Decode monomial indices into exponent arrays.
///
/// For each monomial index m (0-based internally, 1-based in ROSY), writes the
/// exponent vector into `result[m]`. The `params` vector should contain at least
/// `[order, num_vars]` for validation against the current DAINI setup.
///
/// `result` is a 2D RE array: `result[m][v]` = exponent of variable v+1 in monomial m.
pub fn rosy_dacode(params: &Vec<f64>, size: usize, result: &mut Vec<Vec<f64>>) -> Result<()> {
    let (num_monomials, num_vars, monomial_list) = {
        let rt =
            get_runtime().context("DACODE requires DA to be initialized (call DAINI first)")?;
        (
            rt.num_monomials,
            rt.config.num_vars,
            rt.monomial_list.clone(),
        )
    };

    // Validate params against current setup
    if params.len() >= 2 {
        let p_order = params[0] as u32;
        let p_vars = params[1] as usize;
        let rt = get_runtime()?;
        if p_order != rt.config.max_order {
            bail!(
                "DACODE: params order ({}) does not match current DA order ({})",
                p_order,
                rt.config.max_order
            );
        }
        if p_vars != num_vars {
            bail!(
                "DACODE: params num_vars ({}) does not match current DA num_vars ({})",
                p_vars,
                num_vars
            );
        }
    }

    let count = size.min(num_monomials).min(result.len());

    for m in 0..count {
        let mono = &monomial_list[m];
        let inner = &mut result[m];
        let vlen = inner.len().min(num_vars);
        for v in 0..vlen {
            inner[v] = mono.exponents[v] as f64;
        }
    }

    Ok(())
}

// ============================================================================
// Generic Lie series flow (shared by DAFLO and CDFLO)
// ============================================================================

/// Compute the Lie derivative L_f(g) = ∇g · f = Σᵢ (∂g/∂xᵢ) · fᵢ
/// for a single scalar DA `g` and vector field `f`.
///
/// Generic over coefficient type T (f64 or Complex64).
fn lie_derivative_scalar<T: DACoefficient>(
    g: &GenericDA<T>,
    f: &[GenericDA<T>],
    dim: usize,
    epsilon: f64,
    deriv_targets: &[u32],
    deriv_exponents: &[u8],
    n: usize,
) -> Result<GenericDA<T>> {
    let mut acc = GenericDA::<T>::zero();

    for i in 0..dim {
        let base = i * n;
        let deriv_target_v = &deriv_targets[base..base + n];
        let deriv_exp_v = &deriv_exponents[base..base + n];

        // Compute ∂g/∂xᵢ
        let mut dg_dxi = GenericDA::<T>::zero();
        for &k in &g.nonzero {
            let target = deriv_target_v[k as usize];
            if target == DERIV_INVALID {
                continue;
            }
            let exp = T::from_usize(deriv_exp_v[k as usize] as usize);
            let new_coeff = g.coeffs[k as usize] * exp;
            if new_coeff.abs() > epsilon {
                dg_dxi.coeffs[target as usize] = new_coeff;
                dg_dxi.nonzero.push(target);
            }
        }

        // Multiply ∂g/∂xᵢ by f[i] and accumulate
        let product = (&dg_dxi * &f[i]).context("Lie derivative: multiplication failed")?;
        acc = (&acc + &product).context("Lie derivative: addition failed")?;
    }

    Ok(acc)
}

/// Generic Lie series flow: computes exp(L_f)(ic) for time step 1.
///
/// For each component i of the initial condition, iterates:
///   term_0 = ic\[i\]
///   term_k = (1/k) · L_f(term_{k-1})
///   result\[i\] = Σ term_k
///
/// Converges to machine accuracy for polynomial vector fields;
/// DA truncation guarantees termination for any smooth f.
fn flow_impl<T: DACoefficient>(
    rhs: &[GenericDA<T>],
    ic: &[GenericDA<T>],
    result: &mut Vec<GenericDA<T>>,
    dim: usize,
) -> Result<()> {
    let (epsilon, num_vars, deriv_targets, deriv_exponents, n) = {
        let rt = get_runtime().context("Flow requires DA to be initialized (call DAINI first)")?;
        (
            rt.config.epsilon,
            rt.config.num_vars,
            rt.deriv_target.clone(),
            rt.deriv_exponent.clone(),
            rt.num_monomials,
        )
    };

    if dim > num_vars {
        bail!(
            "Flow: dim ({}) exceeds number of DA variables ({})",
            dim,
            num_vars
        );
    }
    if rhs.len() < dim {
        bail!("Flow: rhs array has {} elements but dim={}", rhs.len(), dim);
    }
    if ic.len() < dim {
        bail!(
            "Flow: initial condition has {} elements but dim={}",
            ic.len(),
            dim
        );
    }

    const MAX_ITER: usize = 200;

    for i in 0..dim {
        let mut term = ic[i].clone();
        let mut sum = term.clone();

        for k in 1..=MAX_ITER {
            // term_k = (1/k) · L_f(term_{k-1})
            let lie = lie_derivative_scalar(
                &term,
                &rhs[..dim],
                dim,
                epsilon,
                &deriv_targets,
                &deriv_exponents,
                n,
            )?;
            let scale = T::one() / T::from_usize(k);
            term = (&lie * scale).context("Flow: scaling failed")?;

            // Check convergence: max |coefficient| in the new term
            let term_norm: f64 = term
                .nonzero
                .iter()
                .map(|&idx| term.coeffs[idx as usize].abs())
                .fold(0.0f64, f64::max);

            if term_norm < epsilon {
                break;
            }

            sum = (&sum + &term).context("Flow: accumulation failed")?;
        }

        if let Some(r) = result.get_mut(i) {
            *r = sum;
        } else {
            result.push(sum);
        }
    }

    Ok(())
}

/// DAFLO: Compute the real DA flow of x' = f(x) for time step 1.
///
/// Arguments:
/// - `rhs`: array of DAs representing the vector field f (dim components)
/// - `ic`: initial condition DA array (dim components)
/// - `result`: output DA array (dim components)
/// - `dim`: dimension of the system
pub fn rosy_daflo(rhs: &Vec<DA>, ic: &Vec<DA>, result: &mut Vec<DA>, dim: usize) -> Result<()> {
    flow_impl(rhs, ic, result, dim)
}

/// CDFLO: Compute the complex DA flow of x' = f(x) for time step 1.
///
/// Same as DAFLO but with complex DA (CD) coefficients.
pub fn rosy_cdflo(rhs: &Vec<CD>, ic: &Vec<CD>, result: &mut Vec<CD>, dim: usize) -> Result<()> {
    flow_impl(rhs, ic, result, dim)
}

/// DANOW: Compute the order-weighted max norm of a DA variable.
///
/// For each monomial of order k with coefficient c, computes |c| * weight^k.
/// Returns the maximum over all monomials.
pub fn rosy_danow(da: &impl crate::AsDaRef, weight: impl crate::AsF64, result: &mut impl crate::SetF64) -> Result<()> {
    let da = da.as_da_vec();
    let weight = weight.as_f64_val();
    let mut best = 0.0;
    if let Some(da) = da.first() {
        for (monomial, coeff) in da.coeffs_iter() {
            let order = monomial.total_order as f64;
            let weighted = coeff.abs() * weight.powf(order);
            if weighted > best {
                best = weighted;
            }
        }
    }
    result.set_f64(best);
    Ok(())
}

/// CDF2: Apply exp(:f2:) to a CD vector in Floquet variables.
///
/// For each monomial with exponent pairs (a_k, b_k) for conjugate variable pairs,
/// multiplies the coefficient by exp(i * sum_k mu_k * (a_k - b_k))
/// where mu_k are the phase advances (tunes).
pub fn rosy_cdf2(
    input: &Vec<CD>,
    tune1: f64,
    tune2: f64,
    tune3: f64,
    result: &mut Vec<CD>,
) -> Result<()> {
    let tunes = [tune1, tune2, tune3];
    let rt = get_runtime().context("CDF2 requires DA to be initialized (call DAINI first)")?;
    let num_vars = rt.config.num_vars;

    let n_pairs = num_vars / 2;
    ensure!(
        n_pairs <= 3,
        "CDF2: at most 3 conjugate pairs (6 variables) supported, got {} variables",
        num_vars
    );

    for (idx, cd_in) in input.iter().enumerate() {
        if idx >= result.len() {
            break;
        }
        let mut cd_out = CD::zero();

        for (monomial, coeff) in cd_in.coeffs_iter() {
            // Compute net phase: sum_k mu_k * (e_{2k} - e_{2k+1})
            let mut net_phase = 0.0;
            for k in 0..n_pairs {
                let e_pos = monomial.exponents[2 * k] as f64;
                let e_neg = monomial.exponents[2 * k + 1] as f64;
                net_phase += tunes[k] * (e_pos - e_neg);
            }

            // Multiply coefficient by exp(i * net_phase)
            let rotation = Complex64::new(net_phase.cos(), net_phase.sin());
            cd_out.set_coeff(monomial, coeff * rotation);
        }

        result[idx] = cd_out;
    }

    Ok(())
}

/// CDNF: Apply 1/(1-exp(:f2:)) to a CD vector — homological equation solver.
///
/// For each monomial, divides by the resonance denominator (1 - exp(i*net_phase)).
/// Terms with small denominators (resonant terms) are zeroed out.
pub fn rosy_cdnf(
    input: &Vec<CD>,
    tune1: f64,
    tune2: f64,
    tune3: f64,
    resonances: &Vec<f64>,
    res_dims: &Vec<f64>,
    n_resonances: usize,
    result: &mut Vec<CD>,
) -> Result<()> {
    let tunes = [tune1, tune2, tune3];
    let rt = get_runtime().context("CDNF requires DA to be initialized (call DAINI first)")?;
    let num_vars = rt.config.num_vars;
    let n_pairs = num_vars / 2;
    let epsilon = rt.config.epsilon;

    for (idx, cd_in) in input.iter().enumerate() {
        if idx >= result.len() {
            break;
        }
        let mut cd_out = CD::zero();
        // Which conjugate pair does this component belong to?
        let comp_pair = idx / 2;
        let comp_sign = if idx % 2 == 0 { 1.0 } else { -1.0 };

        for (monomial, coeff) in cd_in.coeffs_iter() {
            // Compute net phase: sum_k mu_k * (e_{2k} - e_{2k+1})
            let mut net_phase = 0.0;
            for k in 0..n_pairs {
                let e_pos = monomial.exponents[2 * k] as f64;
                let e_neg = monomial.exponents[2 * k + 1] as f64;
                net_phase += tunes[k] * (e_pos - e_neg);
            }

            // The denominator for the j-th component:
            // exp(i * net_phase) - exp(i * comp_sign * mu_j)
            let comp_mu = if comp_pair < n_pairs {
                tunes[comp_pair]
            } else {
                0.0
            };
            let eig_phase = comp_sign * comp_mu;

            let numerator_phase = Complex64::new(net_phase.cos(), net_phase.sin());
            let eigenvalue = Complex64::new(eig_phase.cos(), eig_phase.sin());
            let denominator = numerator_phase - eigenvalue;

            // Check for resonance: if denominator is too small, zero out
            // Resonances are stored flat: each resonance is n_pairs entries long
            let is_resonant = denominator.norm() < epsilon || {
                let mut resonant = false;
                let mut offset = 0usize;
                for r in 0..n_resonances {
                    let dim = if r < res_dims.len() {
                        res_dims[r] as usize
                    } else {
                        n_pairs
                    };
                    let mut matches = true;
                    for k in 0..dim.min(n_pairs) {
                        let diff =
                            monomial.exponents[2 * k] as i64 - monomial.exponents[2 * k + 1] as i64;
                        if offset + k < resonances.len() && diff != resonances[offset + k] as i64 {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        resonant = true;
                        break;
                    }
                    offset += dim;
                }
                resonant
            };

            if is_resonant {
                // Resonant term — zero in the output (stays in the normal form)
                continue;
            }

            let new_coeff = -coeff / denominator;
            cd_out.set_coeff(monomial, new_coeff);
        }

        result[idx] = cd_out;
    }

    Ok(())
}

/// CDNFDA: Apply Cj operator for non-symplectic normal forms.
///
/// Like CDNF but eigenvalues have |lambda| != 1. Uses separate moduli and arguments.
/// denominator = prod_k(lambda_k^{e_k}) - lambda_coord
pub fn rosy_cdnfda(
    input: &Vec<CD>,
    moduli: &Vec<f64>,
    arguments: &Vec<f64>,
    coord: usize,
    total: usize,
    epsilon: f64,
    result: &mut Vec<CD>,
) -> Result<()> {
    let rt = get_runtime().context("CDNFDA requires DA to be initialized (call DAINI first)")?;
    let num_vars = rt.config.num_vars;

    ensure!(
        coord >= 1 && coord <= total,
        "CDNFDA: coordinate number {} out of range 1..{}",
        coord,
        total
    );

    // Eigenvalue for the target coordinate
    let coord_idx = coord - 1;
    let lambda_coord = if coord_idx < moduli.len() && coord_idx < arguments.len() {
        let r = moduli[coord_idx];
        let theta = arguments[coord_idx];
        Complex64::new(r * theta.cos(), r * theta.sin())
    } else {
        Complex64::new(1.0, 0.0)
    };

    for (idx, cd_in) in input.iter().enumerate() {
        if idx >= result.len() {
            break;
        }
        let mut cd_out = CD::zero();

        for (monomial, coeff) in cd_in.coeffs_iter() {
            // Compute product of eigenvalues raised to exponent powers
            let mut prod = Complex64::new(1.0, 0.0);
            for k in 0..total.min(num_vars).min(moduli.len()).min(arguments.len()) {
                let exp = monomial.exponents[k] as f64;
                if exp != 0.0 {
                    let r = moduli[k];
                    let theta = arguments[k];
                    let lambda_k = Complex64::new(r * theta.cos(), r * theta.sin());
                    prod *= lambda_k.powf(exp);
                }
            }

            let denominator = prod - lambda_coord;

            if denominator.norm() < epsilon {
                continue; // Resonant — zero out
            }

            let new_coeff = -coeff / denominator;
            cd_out.set_coeff(monomial, new_coeff);
        }

        result[idx] = cd_out;
    }

    Ok(())
}

/// CDNFDS: Apply Sj operator for spin normal forms.
///
/// Like CDNFDA but the target eigenvalue is lambda_spin = exp(i * spin_argument).
pub fn rosy_cdnfds(
    input: &Vec<CD>,
    moduli: &Vec<f64>,
    arguments: &Vec<f64>,
    spin_arg: f64,
    total: usize,
    epsilon: f64,
    result: &mut Vec<CD>,
) -> Result<()> {
    let rt = get_runtime().context("CDNFDS requires DA to be initialized (call DAINI first)")?;
    let num_vars = rt.config.num_vars;

    let lambda_spin = Complex64::new(spin_arg.cos(), spin_arg.sin());

    for (idx, cd_in) in input.iter().enumerate() {
        if idx >= result.len() {
            break;
        }
        let mut cd_out = CD::zero();

        for (monomial, coeff) in cd_in.coeffs_iter() {
            let mut prod = Complex64::new(1.0, 0.0);
            for k in 0..total.min(num_vars).min(moduli.len()).min(arguments.len()) {
                let exp = monomial.exponents[k] as f64;
                if exp != 0.0 {
                    let r = moduli[k];
                    let theta = arguments[k];
                    let lambda_k = Complex64::new(r * theta.cos(), r * theta.sin());
                    prod *= lambda_k.powf(exp);
                }
            }

            let denominator = prod - lambda_spin;

            if denominator.norm() < epsilon {
                continue;
            }

            let new_coeff = -coeff / denominator;
            cd_out.set_coeff(monomial, new_coeff);
        }

        result[idx] = cd_out;
    }

    Ok(())
}
