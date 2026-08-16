//! # QUIT Statement
//!
//! Terminates execution of the program.
//!
//! ## Syntax
//!
//! ```text
//! QUIT c;
//! ```
//!
//! ## Semantics
//!
//! - `QUIT 0;` — clean exit via `std::process::exit(0)`
//! - `QUIT 1;` — triggers system traceback via `panic!`
//! - `QUIT n;` — exits with code `n` cast to i32
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/quit.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*, program::expressions::Expr, program::statements::SourceLocation, resolve::*,
    transpile::*,
};

/// AST node for `QUIT c;`.
#[derive(Debug)]
pub struct QuitStatement {
    pub code_expr: Expr,
}

impl FromRule for QuitStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::quit,
            "Expected `quit` rule when building QUIT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let code_pair = inner.next().context("Missing code expression in QUIT!")?;
        let code_expr = Expr::from_rule(code_pair)
            .context("Failed to build code expression in QUIT")?
            .ok_or_else(|| anyhow::anyhow!("Expected code expression in QUIT"))?;

        Ok(Some(QuitStatement { code_expr }))
    }
}
impl TranspileableStatement for QuitStatement {
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
impl Transpile for QuitStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let code_output = self.code_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling code expression in QUIT".to_string(),
            )
        })?;
        requested_variables.extend(code_output.requested_variables.iter().cloned());

        let serialization = format!(
            r#"{{
    let __quit_code = {} as i32;
    if __quit_code == 1 {{
        panic!("QUIT with traceback requested");
    }} else {{
        std::process::exit(__quit_code);
    }}
}}"#,
            code_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
