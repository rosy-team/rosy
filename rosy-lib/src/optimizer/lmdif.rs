/// LMDIF-style optimizer (Algorithm 4 in COSY).
///
/// This is a Levenberg-Marquardt least-squares optimizer. It minimizes
/// the sum of squares of the objective functions by using finite-difference
/// approximations to the Jacobian matrix.
///
/// When there are fewer objectives than variables, it uses a damped
/// gradient descent approach instead.

/// Run LMDIF optimization.
///
/// Minimizes sum of squares of objectives using Levenberg-Marquardt method
/// with finite-difference Jacobian.
pub fn lmdif<F>(
    variables: &mut [f64],
    eps: f64,
    max_iter: usize,
    num_objectives: usize,
    body: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(&mut [f64]) -> anyhow::Result<Vec<f64>>,
{
    let nv = variables.len();
    let nf = num_objectives;

    // Initial evaluation
    let mut residuals = body(variables)?;
    let mut cost = sum_of_squares(&residuals);

    // Levenberg-Marquardt parameter
    let mut lambda = 1e-4;
    let lambda_up = 11.0;
    let lambda_down = 9.0;

    // Step size for finite differences
    let diff_step = (f64::EPSILON).sqrt();

    let mut best_vars = variables.to_vec();
    let mut best_cost = cost;

    for _iteration in 0..max_iter {
        // Check convergence
        if cost < eps * eps {
            break;
        }

        // Compute Jacobian via forward finite differences
        let mut jacobian = vec![vec![0.0; nv]; nf];
        for j in 0..nv {
            let orig = variables[j];
            let h = if orig.abs() > 1e-10 {
                diff_step * orig.abs()
            } else {
                diff_step
            };
            variables[j] = orig + h;
            let perturbed = body(variables)?;
            variables[j] = orig;

            for i in 0..nf {
                jacobian[i][j] = (perturbed[i] - residuals[i]) / h;
            }
        }

        // Compute J^T * J and J^T * r
        let mut jtj = vec![vec![0.0; nv]; nv];
        let mut jtr = vec![0.0; nv];

        for i in 0..nf {
            for j in 0..nv {
                jtr[j] += jacobian[i][j] * residuals[i];
                for k in 0..nv {
                    jtj[j][k] += jacobian[i][j] * jacobian[i][k];
                }
            }
        }

        // Check gradient convergence (use tighter criterion)
        let grad_norm: f64 = jtr.iter().map(|g| g * g).sum::<f64>().sqrt();
        if grad_norm < eps * eps {
            break;
        }

        // Solve (J^T*J + lambda*I) * delta = -J^T*r
        // Use identity-based damping for better behavior with underdetermined systems
        let mut damped = jtj.clone();
        for j in 0..nv {
            damped[j][j] += lambda * (jtj[j][j].max(1.0));
        }

        // Solve the linear system using simple Gaussian elimination
        let delta = match solve_linear_system(&damped, &jtr.iter().map(|x| -x).collect::<Vec<_>>()) {
            Some(d) => d,
            None => {
                // Singular matrix - increase lambda and retry
                lambda *= lambda_up;
                continue;
            }
        };

        // Try the step
        let mut trial = variables.to_vec();
        for j in 0..nv {
            trial[j] += delta[j];
        }

        let trial_residuals = body(trial.as_mut_slice())?;
        let trial_cost = sum_of_squares(&trial_residuals);

        if trial_cost < cost {
            // Accept step
            variables.copy_from_slice(&trial);
            residuals = trial_residuals;
            cost = trial_cost;
            lambda = (lambda / lambda_down).max(1e-12);

            // Track best
            if cost < best_cost {
                best_vars.copy_from_slice(variables);
                best_cost = cost;
            }

            // Check cost convergence
            if cost < eps * eps {
                break;
            }
        } else {
            // Reject step, increase damping
            lambda *= lambda_up;
            if lambda > 1e16 {
                // Lambda too large, we're stuck — restore best and stop
                break;
            }
        }
    }

    // Restore best variables found
    variables.copy_from_slice(&best_vars);

    // Run body one final time with best values so objectives are set
    body(variables)?;

    Ok(())
}

fn sum_of_squares(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum()
}

/// Solve Ax = b using Gaussian elimination with partial pivoting.
/// Returns None if the matrix is singular.
fn solve_linear_system(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    
    // Augmented matrix
    let mut aug: Vec<Vec<f64>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = a[i].clone();
        row.push(b[i]);
        aug.push(row);
    }

    // Forward elimination with partial pivoting
    for col in 0..n {
        // Find pivot
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..n {
            if aug[row][col].abs() > max_val {
                max_val = aug[row][col].abs();
                max_row = row;
            }
        }

        if max_val < 1e-15 {
            return None; // Singular
        }

        // Swap rows
        if max_row != col {
            aug.swap(col, max_row);
        }

        // Eliminate below
        let pivot = aug[col][col];
        for row in (col + 1)..n {
            let factor = aug[row][col] / pivot;
            for j in col..=n {
                let val = aug[col][j];
                aug[row][j] -= factor * val;
            }
        }
    }

    // Back substitution
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = aug[i][n];
        for j in (i + 1)..n {
            sum -= aug[i][j] * x[j];
        }
        if aug[i][i].abs() < 1e-15 {
            return None; // Singular
        }
        x[i] = sum / aug[i][i];
    }

    Some(x)
}
