//! # RERAN Statement
//!
//! Returns a random number between -1 and 1.
//!
//! ## Syntax
//!
//! ```text
//! RERAN result_var;
//! ```
//!
//! - `result_var` — variable that receives the RE random value
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/reran.rosy"))]
//! ```

use anyhow::{ensure, Context, Error, Result};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ExprRecipe, ResolutionRule, ScopeContext, TypeResolver},
    transpile::*,
};
use rosy_lib::RosyType;

#[derive(Debug)]
pub struct ReranStatement {
    pub output_var: VariableIdentifier,
}

impl FromRule for ReranStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::reran,
            "Expected `reran` rule when building RERAN statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let output_pair = inner.next().context("Missing output variable in RERAN!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in RERAN")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in RERAN"))?;

        Ok(Some(ReranStatement { output_var }))
    }
}

impl Transpile for ReranStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in RERAN".to_string(),
            )
        })?;
        requested_variables.extend(output_id_output.requested_variables.clone());

        let dereference = match context
            .variables
            .get(&self.output_var.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.output_var.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.output_var.name.clone());
                "*"
            }
        };

        // Generate random f64 in [-1, 1] using thread_rng
        let serialization = format!(
            "{deref}{dest} = rosy_lib::core::rng::rosy_reran();",
            deref = dereference,
            dest = output_id_output.serialization,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for ReranStatement {
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        let var_slot = ctx.variables.get(&self.output_var.name)?.clone();
        if let Some(node) = resolver.nodes.get_mut(&var_slot) {
            if matches!(node.rule, ResolutionRule::Unresolved) {
                node.rule = ResolutionRule::InferredFrom {
                    recipe: ExprRecipe::Literal(RosyType::RE()),
                    reason: "RERAN writes RE".to_string(),
                };
                node.assigned_at = Some(source_location);
            }
        }
        Some(Ok(()))
    }
}
