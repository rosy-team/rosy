/// Nelder-Mead Simplex optimizer (Algorithm 1 in COSY).
///
/// This is a derivative-free optimization method suitable for general objective
/// functions that don't need to satisfy smoothness criteria. It works by
/// maintaining a simplex of n+1 points in n-dimensional space and iteratively
/// reflecting, expanding, contracting, or shrinking the simplex to find a minimum.
///
/// Reference: Nelder, J.A. and Mead, R. (1965), "A Simplex Method for Function
/// Minimization", The Computer Journal, 7(4), 308-313.

/// Evaluate the sum of squares of objectives (scalar merit function)
fn merit(objectives: &[f64]) -> f64 {
    objectives.iter().map(|o| o * o).sum()
}

/// Run Nelder-Mead simplex optimization.
///
/// Minimizes the sum of squares of the objective values returned by `body`.
pub fn nelder_mead<F>(
    variables: &mut [f64],
    eps: f64,
    max_iter: usize,
    _num_objectives: usize,
    body: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(&mut [f64]) -> anyhow::Result<Vec<f64>>,
{
    let n = variables.len();

    // Nelder-Mead parameters (standard values)
    let alpha = 1.0;  // reflection
    let gamma = 2.0;  // expansion
    let rho = 0.5;    // contraction
    let sigma = 0.5;  // shrink

    // Initialize simplex: n+1 vertices
    // First vertex is the initial point
    let mut simplex: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    simplex.push(variables.to_vec());

    // Create other vertices by perturbing each dimension
    for i in 0..n {
        let mut vertex = variables.to_vec();
        let delta = if vertex[i].abs() > 1e-10 {
            vertex[i] * 0.05  // 5% perturbation
        } else {
            0.00025  // Small absolute perturbation for near-zero values
        };
        vertex[i] += delta;
        simplex.push(vertex);
    }

    // Evaluate all vertices
    let mut values: Vec<f64> = Vec::with_capacity(n + 1);
    for vertex in &mut simplex {
        let objs = body(vertex.as_mut_slice())?;
        values.push(merit(&objs));
    }

    let mut iteration = 0;

    loop {
        // Sort vertices by function value
        let mut indices: Vec<usize> = (0..=n).collect();
        indices.sort_by(|&a, &b| values[a].partial_cmp(&values[b]).unwrap_or(std::cmp::Ordering::Equal));

        // Reorder simplex and values
        let sorted_simplex: Vec<Vec<f64>> = indices.iter().map(|&i| simplex[i].clone()).collect();
        let sorted_values: Vec<f64> = indices.iter().map(|&i| values[i]).collect();
        simplex = sorted_simplex;
        values = sorted_values;

        // Check termination: iteration count
        iteration += 1;
        if iteration >= max_iter {
            break;
        }

        // Check termination: convergence
        // Use range of values (max - min) relative to function scale
        let range = values[n] - values[0];
        let scale = (values[0].abs() + values[n].abs()) * 0.5 + 1e-30;
        if range / scale < eps && range < eps {
            break;
        }

        // Also check if best value is essentially zero
        if values[0] < eps * eps {
            break;
        }

        // Calculate centroid of all points except worst
        let mut centroid = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                centroid[j] += simplex[i][j];
            }
        }
        for j in 0..n {
            centroid[j] /= n as f64;
        }

        // Reflection
        let mut reflected = vec![0.0; n];
        for j in 0..n {
            reflected[j] = centroid[j] + alpha * (centroid[j] - simplex[n][j]);
        }
        let reflected_val = merit(&body(reflected.as_mut_slice())?);

        if reflected_val < values[n - 1] && reflected_val >= values[0] {
            // Accept reflection
            simplex[n] = reflected;
            values[n] = reflected_val;
            continue;
        }

        if reflected_val < values[0] {
            // Try expansion
            let mut expanded = vec![0.0; n];
            for j in 0..n {
                expanded[j] = centroid[j] + gamma * (reflected[j] - centroid[j]);
            }
            let expanded_val = merit(&body(expanded.as_mut_slice())?);

            if expanded_val < reflected_val {
                simplex[n] = expanded;
                values[n] = expanded_val;
            } else {
                simplex[n] = reflected;
                values[n] = reflected_val;
            }
            continue;
        }

        // Contraction
        let mut contracted = vec![0.0; n];
        for j in 0..n {
            contracted[j] = centroid[j] + rho * (simplex[n][j] - centroid[j]);
        }
        let contracted_val = merit(&body(contracted.as_mut_slice())?);

        if contracted_val < values[n] {
            simplex[n] = contracted;
            values[n] = contracted_val;
            continue;
        }

        // Shrink: move all points toward the best point
        for i in 1..=n {
            for j in 0..n {
                simplex[i][j] = simplex[0][j] + sigma * (simplex[i][j] - simplex[0][j]);
            }
            let objs = body(simplex[i].as_mut_slice())?;
            values[i] = merit(&objs);
        }
    }

    // Copy best values back to variables
    variables.copy_from_slice(&simplex[0]);

    // Run body one final time with best values so objectives are set
    body(variables)?;

    Ok(())
}
