//! # DANOTW Statement (DA Weighted Order)
//!
//! Sets per-variable weight factors for DA and CD monomial ordering.
//! Must be called before DAINI. The next `DAINI` call enumerates monomials
//! where `Σ wᵢ·eᵢ ≤ max_order`, then clears the weight vector.
//!
//! ## Syntax
//!
//! ```text
//! DANOTW weights size;
//! ```
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

use anyhow::{Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, add_context_to_all,
    },
};

/// AST node for the `DANOTW weights size;` weighted order statement.
#[derive(Debug)]
pub struct DanotwStatement {
    pub weights: Expr,
    pub size: Expr,
}

impl FromRule for DanotwStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::danotw,
            "Expected `danotw` rule when building DANOTW statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let weights = Expr::from_rule(
            inner
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing weights in DANOTW"))?,
        )
        .map_err(|e| e)?
        .ok_or_else(|| anyhow::anyhow!("Expected weights expression in DANOTW"))?;

        let size = Expr::from_rule(
            inner
                .next()
                .ok_or_else(|| anyhow::anyhow!("Missing size in DANOTW"))?,
        )
        .map_err(|e| e)?
        .ok_or_else(|| anyhow::anyhow!("Expected size expression in DANOTW"))?;

        Ok(Some(DanotwStatement { weights, size }))
    }
}

impl TranspileableStatement for DanotwStatement {
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
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        InferenceEdgeResult::NoEdges
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
    }
}

impl Transpile for DanotwStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let weights_output = self.weights.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling weights in DANOTW".to_string())
        })?;
        requested_variables.extend(weights_output.requested_variables.iter().cloned());

        let size_output = self.size.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling size in DANOTW".to_string())
        })?;
        requested_variables.extend(size_output.requested_variables.iter().cloned());

        let serialization = format!(
            "{{\n\t\t\tlet __danotw_weights = {weights};\n\t\t\tlet __danotw_size = {size} as usize;\n\t\t\tlet __danotw_vec: Vec<u32> = __danotw_weights.iter().take(__danotw_size).map(|&v| v as u32).collect();\n\t\t\ttaylor::set_weight_vector(__danotw_vec)?;\n\t\t}}",
            weights = weights_output.as_value(),
            size = size_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
