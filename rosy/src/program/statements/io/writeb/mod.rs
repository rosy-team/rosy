//! # WRITEB Statement (Binary Write)
//!
//! Writes values in binary format to a file unit.
//!
//! ## Syntax
//!
//! ```text
//! WRITEB unit expr1 [expr2 ...];
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/writeb.rosy"))]
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

/// AST node for `WRITEB unit expr+;`.
/// WRITEB unit expr+ ;
#[derive(Debug)]
pub struct WritebStatement {
    pub unit: Expr,
    pub exprs: Vec<Expr>,
}

impl FromRule for WritebStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::writeb,
            "Expected `writeb` rule when building WRITEB statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner
            .next()
            .context("Missing unit expression in `writeb` statement!")?;
        let unit = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in `writeb` statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in `writeb` statement"))?;

        let exprs = {
            let mut exprs = Vec::new();
            for expr_pair in inner {
                if expr_pair.as_rule() == Rule::semicolon {
                    break;
                }

                let expr = Expr::from_rule(expr_pair)
                    .context("Failed to build expression in `writeb` statement!")?
                    .ok_or_else(|| anyhow::anyhow!("Expected expression in `writeb` statement"))?;
                exprs.push(expr);
            }
            exprs
        };

        Ok(Some(WritebStatement { unit, exprs }))
    }
}
impl TranspileableStatement for WritebStatement {
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
impl Transpile for WritebStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let unit_output = self.unit.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in WRITEB".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let mut serialized_stmts = Vec::new();
        serialized_stmts.push(format!(
            "let __rosy_unit = ({}).round() as u64;",
            unit_output.as_value()
        ));

        for expr in &self.exprs {
            let output = expr.transpile(context).map_err(|err_vec| {
                add_context_to_all(
                    err_vec,
                    format!(
                        "...while transpiling expression '{:?}' for WRITEB statement",
                        expr
                    ),
                )
            })?;

            requested_variables.extend(output.requested_variables.iter().cloned());

            serialized_stmts.push(format!(
                "rosy_lib::core::file_io::rosy_writeb_to_unit(__rosy_unit, &rosy_lib::core::file_io::RosyToBinary::to_binary({}))?;",
                output.as_ref()
            ));
        }

        Ok(TranspilationOutput {
            serialization: format!("{{ {} }}", serialized_stmts.join("\n")),
            requested_variables,
            ..Default::default()
        })
    }
}
