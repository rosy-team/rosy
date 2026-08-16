//! # Function Call Statement
//!
//! Calls a user-defined function as a statement (return value is discarded).
//! This is the statement-level counterpart to function call expressions.
//!
//! ## Syntax
//!
//! ```text
//! FNAME arg1 [arg2 ...];
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/function_call.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};

use crate::{
    ast::*,
    program::{
        expressions::{Expr, core::var_expr::function_call_transpile_helper},
        statements::SourceLocation,
    },
    resolve::*,
    transpile::*,
};

/// AST node for a function call used as a statement.
#[derive(Debug)]
pub struct FunctionCallStatement {
    pub name: String,
    pub args: Vec<Expr>,
}

impl FromRule for FunctionCallStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::function_call,
            "Expected `function_call` rule when building function call statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let name = inner
            .next()
            .context("Missing function name in function call!")?
            .as_str()
            .to_string();

        let mut args = Vec::new();
        // Collect all remaining arguments (expressions)
        for arg_pair in inner {
            if arg_pair.as_rule() == Rule::semicolon {
                break;
            }

            let expr = Expr::from_rule(arg_pair)
                .context("Failed to build expression in function call!")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression in function call"))?;
            args.push(expr);
        }

        Ok(Some(FunctionCallStatement { name, args }))
    }
}
impl TranspileableStatement for FunctionCallStatement {
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
            result: resolver.discover_call_site_deps(&self.name, &self.args, true, ctx),
        }
    }
    fn hydrate_resolved_types(
        &mut self,
        _resolver: &TypeResolver,
        _current_scope: &[String],
    ) -> TypeHydrationResult {
        TypeHydrationResult::NothingToHydrate
    }
}
impl Transpile for FunctionCallStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        function_call_transpile_helper(&self.name, &self.args, context)
    }
}
