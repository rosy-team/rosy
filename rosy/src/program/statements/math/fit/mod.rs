//! FIT statement implementation.
//!
//! ## Syntax
//! ```text
//! FIT var1 var2 ...; <statements> ENDFIT eps max algo obj1 obj2 ...;
//! ```
//!
//! The FIT loop is an optimization construct that repeatedly executes its body
//! while an optimizer adjusts the specified variables to minimize the objective
//! value(s). The loop terminates when the optimizer can't improve by more than
//! `eps`, or when `max` iterations are reached. If `max` is 0, the body executes
//! exactly once (no optimization).
//!
//! Algorithms:
//!   1 = Nelder-Mead Simplex
//!   3 = Simulated Annealing (not yet implemented)
//!   4 = LMDIF (Levenberg-Marquardt least squares)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/fit.rosy"))]
//! ```

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::Expr,
        statements::{SourceLocation, Statement},
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableExpr, TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult,
        indent,
    },
};
use rosy_lib::RosyType;

#[derive(Debug)]
pub struct FitStatement {
    /// Variable names to be optimized (the "knobs")
    pub fit_variables: Vec<String>,
    /// Body statements executed each iteration
    pub body: Vec<Statement>,
    /// Convergence tolerance (eps)
    pub eps: Expr,
    /// Maximum number of iterations
    pub max_iter: Expr,
    /// Algorithm number (1=Simplex, 3=SA, 4=LMDIF)
    pub algorithm: Expr,
    /// Objective variable name(s) to minimize
    pub objectives: Vec<String>,
}
impl FromRule for FitStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::fit_statement,
            "Expected `fit_statement` rule when building FIT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // Parse start_fit to get variable names
        let fit_variables = {
            let start_fit = inner.next().context("Missing first token `start_fit`!")?;
            ensure!(
                start_fit.as_rule() == Rule::start_fit,
                "Expected `start_fit`, found: {:?}",
                start_fit.as_rule()
            );

            let start_fit_inner = start_fit.into_inner();
            let mut vars = Vec::new();
            for pair in start_fit_inner {
                if pair.as_rule() == Rule::variable_name {
                    vars.push(pair.as_str().to_string());
                }
            }

            if vars.is_empty() {
                anyhow::bail!("FIT statement requires at least one variable!");
            }

            vars
        };

        // Parse body statements until we hit end_fit
        let mut body = Vec::new();
        let end_fit_pair = loop {
            let element = inner
                .next()
                .ok_or_else(|| anyhow::anyhow!("Expected `end_fit` at end of FIT block!"))?;

            if element.as_rule() == Rule::end_fit {
                break element;
            }

            let pair_input = element.as_str();
            if let Some(stmt) = Statement::from_rule(element)
                .with_context(|| format!("Failed to build statement from:\n{}", pair_input))?
            {
                body.push(stmt);
            }
        };

        // Parse end_fit: ENDFIT eps max algo obj1 obj2 ... ;
        let (eps, max_iter, algorithm, objectives) = {
            let mut end_fit_inner = end_fit_pair.into_inner();

            let eps_pair = end_fit_inner
                .next()
                .context("Missing `eps` expression in ENDFIT!")?;
            let eps = Expr::from_rule(eps_pair)
                .context("Failed to build `eps` expression in ENDFIT!")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `eps` in ENDFIT"))?;

            let max_pair = end_fit_inner
                .next()
                .context("Missing `max` expression in ENDFIT!")?;
            let max_iter = Expr::from_rule(max_pair)
                .context("Failed to build `max` expression in ENDFIT!")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `max` in ENDFIT"))?;

            let algo_pair = end_fit_inner
                .next()
                .context("Missing `algorithm` expression in ENDFIT!")?;
            let algorithm = Expr::from_rule(algo_pair)
                .context("Failed to build `algorithm` expression in ENDFIT!")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `algorithm` in ENDFIT"))?;

            let mut objectives = Vec::new();
            for pair in end_fit_inner {
                if pair.as_rule() == Rule::variable_name {
                    objectives.push(pair.as_str().to_string());
                }
            }

            if objectives.is_empty() {
                anyhow::bail!("ENDFIT requires at least one objective variable!");
            }

            (eps, max_iter, algorithm, objectives)
        };

        Ok(Some(FitStatement {
            fit_variables,
            body,
            eps,
            max_iter,
            algorithm,
            objectives,
        }))
    }
}
impl TranspileableStatement for FitStatement {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> TypeslotDeclarationResult {
        TypeslotDeclarationResult::NotAVarFuncOrProcedureDecl
    }
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        InferenceEdgeResult::HasEdges {
            result: resolver.discover_slots(&self.body, &mut ctx.clone()),
        }
    }
    fn hydrate_resolved_types(
        &mut self,
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> TypeHydrationResult {
        if let Err(e) = resolver.apply_to_ast(&mut self.body, current_scope) {
            return TypeHydrationResult::Hydrated { result: Err(e) };
        }
        TypeHydrationResult::Hydrated { result: Ok(()) }
    }
}
impl Transpile for FitStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        // Verify all fit variables exist and are RE type
        for var_name in &self.fit_variables {
            match context.variables.get(var_name) {
                Some(scoped_var) => {
                    if scoped_var.data.r#type != RosyType::RE() {
                        errors.push(anyhow!(
                            "FIT variable '{}' must be of type (RE), found '{}'",
                            var_name,
                            scoped_var.data.r#type
                        ));
                    }
                }
                None => {
                    errors.push(anyhow!(
                        "FIT variable '{}' is not declared in this scope!",
                        var_name
                    ));
                }
            }
        }

        // Verify all objective variables exist and are RE type
        for obj_name in &self.objectives {
            match context.variables.get(obj_name) {
                Some(scoped_var) => {
                    if scoped_var.data.r#type != RosyType::RE() {
                        errors.push(anyhow!(
                            "ENDFIT objective '{}' must be of type (RE), found '{}'",
                            obj_name,
                            scoped_var.data.r#type
                        ));
                    }
                }
                None => {
                    errors.push(anyhow!(
                        "ENDFIT objective '{}' is not declared in this scope!",
                        obj_name
                    ));
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // Verify eps, max, and algorithm are RE type
        let eps_type = self.eps.type_of(context).map_err(|e| vec![e])?;
        if eps_type != RosyType::RE() {
            return Err(vec![anyhow!(
                "ENDFIT `eps` must be of type (RE), found '{}'",
                eps_type
            )]);
        }
        let max_type = self.max_iter.type_of(context).map_err(|e| vec![e])?;
        if max_type != RosyType::RE() {
            return Err(vec![anyhow!(
                "ENDFIT `max` must be of type (RE), found '{}'",
                max_type
            )]);
        }
        let algo_type = self.algorithm.type_of(context).map_err(|e| vec![e])?;
        if algo_type != RosyType::RE() {
            return Err(vec![anyhow!(
                "ENDFIT `algorithm` must be of type (RE), found '{}'",
                algo_type
            )]);
        }

        // Transpile body statements
        let mut inner_context = context.clone();
        let mut serialized_statements = Vec::new();

        for stmt in &self.body {
            match stmt.transpile(&mut inner_context) {
                Ok(output) => {
                    serialized_statements.push(output.serialization);
                    requested_variables.extend(output.requested_variables);
                }
                Err(stmt_errors) => {
                    for e in stmt_errors {
                        errors.push(e.context("...while transpiling statement in FIT block"));
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // Transpile eps, max, algorithm expressions
        let eps_output = match self.eps.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables.clone());
                output
            }
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context("...while transpiling `eps` in ENDFIT"));
                }
                return Err(errors);
            }
        };
        let max_output = match self.max_iter.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables.clone());
                output
            }
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context("...while transpiling `max` in ENDFIT"));
                }
                return Err(errors);
            }
        };
        let algo_output = match self.algorithm.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables.clone());
                output
            }
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context("...while transpiling `algorithm` in ENDFIT"));
                }
                return Err(errors);
            }
        };

        // Generate the Rust code
        // Strategy: collect fit variables into a Vec, pass to optimizer closure,
        // in the closure body, assign from the vec back to variables, run body,
        // then collect objectives into the return vec.
        let num_objs = self.objectives.len();

        // Build: let mut fit_vars = vec![var1, var2, ...];
        // FIT variables are always RE (Copy), so we can use them directly.
        let vars_init = self.fit_variables.to_vec().join(", ");

        // Build inside closure: assign from slice to variables
        let vars_assign = self
            .fit_variables
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{} = __rosy_fit_vars[{}];", v, i))
            .collect::<Vec<_>>()
            .join("\n");

        // Build inside closure: collect objectives into vec
        // Objective variables are always RE (Copy), so we can use them directly.
        let objs_collect = self.objectives.to_vec().join(", ");

        // Build after optimizer: assign back from optimized vec
        let vars_writeback = self
            .fit_variables
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{} = __rosy_fit_vars[{}];", v, i))
            .collect::<Vec<_>>()
            .join("\n");

        let body_code = serialized_statements.join("\n");

        let serialization = format!(
            "{{\n\
            \tlet mut __rosy_fit_vars: Vec<f64> = vec![{vars_init}];\n\
            \trosy_lib::optimizer::run_fit(\n\
            \t\t&mut __rosy_fit_vars,\n\
            \t\t{eps_val},\n\
            \t\t{max_val} as usize,\n\
            \t\t{algo_val} as usize,\n\
            \t\t{num_objs},\n\
            \t\t|__rosy_fit_vars: &mut [f64]| -> anyhow::Result<Vec<f64>> {{\n\
            {assign}\n\
            {body}\n\
            \t\t\tOk(vec![{objs_collect}])\n\
            \t\t}},\n\
            \t).expect(\"FIT optimization failed\");\n\
            {writeback}\n\
            }}",
            vars_init = vars_init,
            eps_val = eps_output.as_value(),
            max_val = max_output.as_value(),
            algo_val = algo_output.as_value(),
            num_objs = num_objs,
            assign = indent(indent(indent(vars_assign))),
            body = indent(indent(indent(body_code))),
            objs_collect = objs_collect,
            writeback = indent(vars_writeback),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
