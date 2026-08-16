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
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```
//! ## COSY INFINITY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::core::variable_identifier::VariableIdentifier,
    program::statements::SourceLocation,
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

impl TranspileableStatement for ReranStatement {
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
        source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        // RERAN always assigns an RE (f64) value to the target variable
        let slot = match ctx.variables.get(&self.output_var.name) {
            Some(s) => s,
            None => return InferenceEdgeResult::NoEdges,
        };
        if let Some(node) = resolver.nodes.get_mut(slot)
            && node.resolved.is_none()
        {
                node.rule = ResolutionRule::InferredFrom {
                    recipe: ExprRecipe::Literal(RosyType::RE()),
                    reason: format!("inferred from RERAN at {}", source_location),
                };
                node.assigned_at = Some(source_location);
        }
        InferenceEdgeResult::HasEdges { result: Ok(()) }
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
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
            "{deref}{dest} = rosy_lib::core::reran::rosy_reran();",
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
