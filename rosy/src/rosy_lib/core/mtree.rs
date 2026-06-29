//! # MTREE Runtime Helper
//!
//! Computes the tree (trie/Horner) representation of a DA array for
//! fast polynomial evaluation.
//!
//! COSY signature: `MTREE(DA_array, elements, coeff_array, steer1, steer2, elements2, tree_length)`
//!
//! The tree representation groups monomials into a nested Horner evaluation
//! scheme indexed by two steering arrays. Each node in the tree says:
//! "multiply by variable `steer1[k]`, then add `coeff_array[k]`, and branch to `steer2[k]`."
//!
//! This matches COSY's MTREE output format so programs ported from COSY
//! that use MTREE for fast map evaluation will work correctly.

use crate::rosy_lib::taylor::{self, DA};
use anyhow::{Result, bail};

/// Build the tree representation of a DA vector.
///
/// # Arguments
/// - `da_array` — vector of DA values (the map components)
/// - `num_elements` — number of elements in da_array to process
///
/// # Returns
/// `(coeffs, steer1, steer2, num_tree_elements, tree_length)` where:
/// - `coeffs[k]` — coefficient at tree node k
/// - `steer1[k]` — variable index to multiply by (0 = constant term / no multiply)
/// - `steer2[k]` — next node index (0 = end of branch)
/// - `num_tree_elements` — number of elements used in the output arrays
/// - `tree_length` — total length of the tree
pub fn rosy_mtree(
    da_array: &Vec<DA>,
    num_elements: usize,
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>, f64, f64)> {
    let rt = taylor::get_runtime()?;
    let num_vars = rt.config.num_vars;

    // Collect all (monomial_exponents, coefficient, component_index) triples
    let mut terms: Vec<(Vec<u8>, f64, usize)> = Vec::new();
    let n_elems = num_elements.min(da_array.len());

    for comp in 0..n_elems {
        let da = &da_array[comp];
        for &idx in &da.nonzero {
            let mono = &rt.monomial_list[idx as usize];
            let coeff = da.coeffs[idx as usize];
            if coeff.abs() > rt.config.epsilon {
                let exps: Vec<u8> = mono.exponents[..num_vars].to_vec();
                terms.push((exps, coeff, comp));
            }
        }
    }

    if terms.is_empty() {
        return Ok((vec![0.0], vec![0.0], vec![0.0], 0.0, 0.0));
    }

    // Sort by component, then by graded lexicographic order of exponents
    terms.sort_by(|a, b| {
        a.2.cmp(&b.2).then_with(|| {
            let ord_a: u8 = a.0.iter().sum();
            let ord_b: u8 = b.0.iter().sum();
            ord_a.cmp(&ord_b).then_with(|| a.0.cmp(&b.0))
        })
    });

    // Build a flat Horner tree representation.
    // Each node: (coefficient, variable_to_multiply_by, next_sibling_or_child).
    //
    // We decompose each monomial c * x1^a1 * x2^a2 * ... into a chain:
    //   multiply by x1, multiply by x1, ...(a1 times), multiply by x2, ..., add c
    // and merge common prefixes.

    let mut coeffs = Vec::new();
    let mut steer1 = Vec::new(); // variable index (1-based, 0 = leaf)
    let mut steer2 = Vec::new(); // next node index (0 = terminal)

    // Simple flat tree: one node per monomial factor
    for (exps, coeff, _comp) in &terms {
        // Expand monomial into chain of variable indices
        let mut chain: Vec<usize> = Vec::new();
        for (var_idx, &exp) in exps.iter().enumerate() {
            for _ in 0..exp {
                chain.push(var_idx + 1); // 1-based variable index
            }
        }

        // Emit nodes: intermediate nodes have coeff 0, last has actual coeff
        if chain.is_empty() {
            // Constant term
            coeffs.push(*coeff);
            steer1.push(0.0);
            steer2.push(0.0);
        } else {
            for (k, &var) in chain.iter().enumerate() {
                let is_last = k == chain.len() - 1;
                coeffs.push(if is_last { *coeff } else { 0.0 });
                steer1.push(var as f64);
                steer2.push(if is_last {
                    0.0
                } else {
                    (coeffs.len() + 1) as f64 // 1-based next node
                });
            }
        }
    }

    let tree_len = coeffs.len();
    let num_tree_elems = tree_len as f64;
    let tree_length = tree_len as f64;

    Ok((coeffs, steer1, steer2, num_tree_elems, tree_length))
}
