//! # DADMU Statement (DA Divide by xᵢ then Multiply by xⱼ)
//!
//! Divides a DA vector by xᵢ then multiplies by xⱼ.
//! Terms whose xᵢ-exponent is zero are dropped (contribute 0 to result).
//!
//! ## Syntax
//!
//! ```text
//! DADMU i j da_in result;
//! ```
//!
//! Arguments:
//! 1. `i`      (RE, read)         - 1-based index of variable to divide by
//! 2. `j`      (RE, read)         - 1-based index of variable to multiply by
//! 3. `da_in`  (DA vector, read)  - source DA array
//! 4. `result` (DA vector, write) - result DA array
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/dadmu.rosy"))]
//! ```

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

/// AST node for `DADMU i j da_in result;`.
#[derive(Debug)]
pub struct DadmuStatement {
    pub var_i_expr: Expr,
    pub var_j_expr: Expr,
    pub da_in_expr: Expr,
    pub result_expr: Expr,
}

impl FromRule for DadmuStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::dadmu,
            "Expected `dadmu` rule when building DADMU statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let var_i_pair = inner.next().context("Missing variable i in DADMU!")?;
        let var_i_expr = Expr::from_rule(var_i_pair)
            .context("Failed to build var_i expression in DADMU")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_i expression in DADMU"))?;

        let var_j_pair = inner.next().context("Missing variable j in DADMU!")?;
        let var_j_expr = Expr::from_rule(var_j_pair)
            .context("Failed to build var_j expression in DADMU")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_j expression in DADMU"))?;

        let da_in_pair = inner.next().context("Missing da_in parameter in DADMU!")?;
        let da_in_expr = Expr::from_rule(da_in_pair)
            .context("Failed to build da_in expression in DADMU")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_in expression in DADMU"))?;

        let result_pair = inner.next().context("Missing result parameter in DADMU!")?;
        let result_expr = Expr::from_rule(result_pair)
            .context("Failed to build result expression in DADMU")?
            .ok_or_else(|| anyhow::anyhow!("Expected result expression in DADMU"))?;

        Ok(Some(DadmuStatement {
            var_i_expr,
            var_j_expr,
            da_in_expr,
            result_expr,
        }))
    }
}

impl TranspileableStatement for DadmuStatement {
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

impl Transpile for DadmuStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let var_i_output = self.var_i_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_i in DADMU".to_string())
        })?;
        requested_variables.extend(var_i_output.requested_variables.iter().cloned());

        let var_j_output = self.var_j_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_j in DADMU".to_string())
        })?;
        requested_variables.extend(var_j_output.requested_variables.iter().cloned());

        let da_in_output = self.da_in_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_in in DADMU".to_string())
        })?;
        requested_variables.extend(da_in_output.requested_variables.iter().cloned());

        let result_output = self.result_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling result in DADMU".to_string())
        })?;
        requested_variables.extend(result_output.requested_variables.clone());

        let serialization = format!(
            "rosy_lib::core::daprv::rosy_dadmu({} as usize, {} as usize, {}, {})?;",
            var_i_output.as_value(),
            var_j_output.as_value(),
            da_in_output.as_ref(),
            result_output.as_mut_ref(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
