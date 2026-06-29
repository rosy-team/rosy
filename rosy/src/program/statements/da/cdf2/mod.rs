//! # CDF2 Statement (Complex DA exp(:f2:) in Floquet Variables)
//!
//! Applies the linear symplectic rotation exp(:f2:) to a CD vector in Floquet
//! (action-angle) coordinates. For each monomial with exponent pairs (a_k, b_k)
//! for conjugate variable pairs, the coefficient is multiplied by
//! `exp(i * sum_k mu_k * (a_k - b_k))` where mu_k are the phase advances (tunes).
//!
//! ## Syntax
//!
//! ```text
//! CDF2 input tune1 tune2 tune3 result;
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
//!
//! Note: the standalone `test.rosy` exercises CDF2 directly with hand-built
//! CD inputs and runs end-to-end. A `test.fox` cosy-parity fixture is
//! deferred — it would require running the cosy.fox / libcosy normal-form
//! pipeline (DANF / NF) against the COSY 10.2 binary and diffing output.

use anyhow::{Context, Error, Result, ensure};
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

/// AST node for the `CDF2 input tune1 tune2 tune3 result;` statement.
#[derive(Debug)]
pub struct Cdf2Statement {
    pub input: Expr,
    pub tune1: Expr,
    pub tune2: Expr,
    pub tune3: Expr,
    pub result: Expr,
}

impl FromRule for Cdf2Statement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::cdf2,
            "Expected `cdf2` rule when building CDF2 statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let input = Expr::from_rule(inner.next().context("Missing input in CDF2")?)
            .context("Failed to build input expression in CDF2")?
            .ok_or_else(|| anyhow::anyhow!("Expected input expression in CDF2"))?;

        let tune1 = Expr::from_rule(inner.next().context("Missing tune1 in CDF2")?)
            .context("Failed to build tune1 expression in CDF2")?
            .ok_or_else(|| anyhow::anyhow!("Expected tune1 expression in CDF2"))?;

        let tune2 = Expr::from_rule(inner.next().context("Missing tune2 in CDF2")?)
            .context("Failed to build tune2 expression in CDF2")?
            .ok_or_else(|| anyhow::anyhow!("Expected tune2 expression in CDF2"))?;

        let tune3 = Expr::from_rule(inner.next().context("Missing tune3 in CDF2")?)
            .context("Failed to build tune3 expression in CDF2")?
            .ok_or_else(|| anyhow::anyhow!("Expected tune3 expression in CDF2"))?;

        let result = Expr::from_rule(inner.next().context("Missing result in CDF2")?)
            .context("Failed to build result expression in CDF2")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in CDF2"))?;

        Ok(Some(Cdf2Statement {
            input,
            tune1,
            tune2,
            tune3,
            result,
        }))
    }
}

impl TranspileableStatement for Cdf2Statement {
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

impl Transpile for Cdf2Statement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let input_output = self
            .input
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling input in CDF2".to_string()))?;
        requested_variables.extend(input_output.requested_variables.iter().cloned());

        let t1_output = self
            .tune1
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling tune1 in CDF2".to_string()))?;
        requested_variables.extend(t1_output.requested_variables.iter().cloned());

        let t2_output = self
            .tune2
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling tune2 in CDF2".to_string()))?;
        requested_variables.extend(t2_output.requested_variables.iter().cloned());

        let t3_output = self
            .tune3
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling tune3 in CDF2".to_string()))?;
        requested_variables.extend(t3_output.requested_variables.iter().cloned());

        let result_output = self.result.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in CDF2".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.iter().cloned());

        let result_ref = result_output.as_mut_ref();

        let serialization = format!(
            "rosy_lib::core::da_ops::rosy_cdf2({}, {}, {}, {}, {})?;",
            input_output.as_ref(),
            t1_output.as_value(),
            t2_output.as_value(),
            t3_output.as_value(),
            result_ref,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
