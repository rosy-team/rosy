//! # WRITE Statement
//!
//! Writes formatted text to a unit (file or console).
//!
//! ## Syntax
//!
//! ```text
//! WRITE unit expr1 [expr2 ...];
//! ```
//!
//! Unit `6` writes to standard output. Each expression is converted
//! to its string representation and printed.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/write.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, string_convert::string_convert_transpile_helper},
    program::statements::SourceLocation,
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for the `WRITE unit expr+;` statement.
#[derive(Debug)]
pub struct WriteStatement {
    pub unit: Expr,
    pub exprs: Vec<Expr>,
}

impl FromRule for WriteStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::write,
            "Expected `write` rule when building write statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner
            .next()
            .context("Missing unit expression in `write` statement!")?;
        let unit = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in `write` statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in `write` statement"))?;

        let exprs = {
            let mut exprs = Vec::new();
            for expr_pair in inner {
                if expr_pair.as_rule() == Rule::semicolon {
                    break;
                }

                let expr = Expr::from_rule(expr_pair)
                    .context("Failed to build expression in `write` statement!")?
                    .ok_or_else(|| anyhow::anyhow!("Expected expression in `write` statement"))?;
                exprs.push(expr);
            }
            exprs
        };

        Ok(Some(WriteStatement { unit, exprs }))
    }
}
impl Transpile for WriteStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        // Transpile the unit expression
        let unit_output = self.unit.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in WRITE".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let mut serialized_exprs = Vec::new();
        for expr in &self.exprs {
            let TranspilationOutput {
                serialization: serialized_expr,
                requested_variables: expr_requested_variables,
                ..
            } = string_convert_transpile_helper(expr, context).map_err(|err_vec| {
                add_context_to_all(
                    err_vec,
                    format!(
                        "...while transpiling expression '{:?}' for WRITE statement",
                        expr
                    ),
                )
            })?;

            serialized_exprs.push(serialized_expr);
            requested_variables.extend(expr_requested_variables);
        }

        // Each WRITE argument is printed on its own line, matching COSY INFINITY semantics.
        let individual_writes: Vec<String> = serialized_exprs
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                format!(
                    "let __rosy_write_arg_{i} = {expr}; \
                     if __rosy_unit == 6 {{ println!(\"{{}}\", __rosy_write_arg_{i}); }} \
                     else {{ rosy_lib::core::file_io::rosy_write_to_unit(__rosy_unit as u64, &__rosy_write_arg_{i})?; }}"
                )
            })
            .collect();

        let serialization = format!(
            "{{ let __rosy_unit = rosy_as_i64(&({})); {} }}",
            unit_output.as_ref(),
            individual_writes.join(" ")
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for WriteStatement {
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        if let Err(e) = resolver.discover_expr_function_calls(&self.unit, ctx) {
            return Some(Err(e.context(
                "...while discovering function call dependencies in WRITE unit expression",
            )));
        }
        for expr in &self.exprs {
            if let Err(e) = resolver.discover_expr_function_calls(expr, ctx) {
                return Some(Err(e.context(
                    "...while discovering function call dependencies in WRITE statement",
                )));
            }
        }
        Some(Ok(()))
    }
}
