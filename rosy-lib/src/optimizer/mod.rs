pub mod simplex;
pub mod lmdif;
pub mod sa;

/// Result of an optimization run
#[derive(Debug)]
pub struct OptimizationResult {
    /// The optimal variable values found
    pub variables: Vec<f64>,
    /// The final objective value(s)
    pub objectives: Vec<f64>,
    /// Number of iterations performed
    pub iterations: usize,
}

/// Run an optimization using the specified algorithm.
///
/// # Arguments
/// * `variables` - Initial values of the variables to optimize
/// * `eps` - Convergence tolerance
/// * `max_iter` - Maximum number of iterations (0 = execute once, no optimization)
/// * `algorithm` - Algorithm number (1 = Simplex, 3 = Simulated Annealing, 4 = LMDIF)
/// * `body` - Closure that takes variable values and returns objective values
///
/// # Returns
/// The optimized variable values and final objective values
pub fn run_fit<F>(
    variables: &mut [f64],
    eps: f64,
    max_iter: usize,
    algorithm: usize,
    num_objectives: usize,
    mut body: F,
) -> anyhow::Result<()>
where
    F: FnMut(&mut [f64]) -> anyhow::Result<Vec<f64>>,
{
    // If max_iter is 0, execute body once and return
    if max_iter == 0 {
        body(variables)?;
        return Ok(());
    }

    match algorithm {
        1 => simplex::nelder_mead(variables, eps, max_iter, num_objectives, &mut body),
        // Algorithm 2 is not available, rerouted to LMDIF per COSY manual
        2 | 4 => lmdif::lmdif(variables, eps, max_iter, num_objectives, &mut body),
        3 => sa::simulated_annealing(variables, eps, max_iter, num_objectives, &mut body),
        other => {
            anyhow::bail!("Unknown optimization algorithm: {}. Supported: 1 (Simplex), 3 (SA), 4 (LMDIF)", other)
        }
    }
}
