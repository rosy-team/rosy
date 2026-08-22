//! # DA — Differential Algebra Constructor
//!
//! Creates a DA (Differential Algebra / Taylor series) value from
//! a variable index.
//!
//! ## Syntax
//!
//! ```text
//! DA(n)          { creates DA identity for variable n }
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/types/da.rosy"))]
//! ```

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::{
    ast::{FromRule, Rule},
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr, ValueKind,
    },
};
use anyhow::{Context, Error};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `DA(n)` constructor expression.
#[derive(Debug)]
pub struct DAExpr {
    pub index: Box<Expr>,
}

impl FromRule for DAExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> anyhow::Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::builtin_function,
            "Expected builtin_function (DA), got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let name = inner
            .next()
            .map(|p| p.as_str().trim().to_ascii_uppercase())
            .unwrap_or_default();
        anyhow::ensure!(name == "DA", "Expected DA constructor, got `{name}`");
        let expr_pair = inner.next().context("Missing inner expression for `DA`!")?;
        let index = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `DA`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `DA`"))?,
        );
        Ok(Some(DAExpr { index }))
    }
}
impl TranspileableExpr for DAExpr {
    fn type_of(&self, _context: &TranspilationInputContext) -> anyhow::Result<RosyType> {
        Ok(RosyType::DA())
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> Option<anyhow::Result<()>> {
        Some(resolver.discover_expr_function_calls(&self.index, ctx))
    }
    fn build_expr_recipe(
        &self,
        _resolver: &TypeResolver,
        _ctx: &ScopeContext,
        _deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        ExprRecipe::Literal(RosyType::DA())
    }
}
impl Transpile for DAExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Transpile the index expression
        let index_output = self.index.transpile(context).map_err(|errs| {
            errs.into_iter()
                .map(|e| e.context("...while transpiling DA index expression"))
                .collect::<Vec<_>>()
        })?;

        // Use DA::variable(usize) to create a DA differential variable
        let serialization = format!("DA::variable(rosy_as_usize(&({})))?", index_output.as_value());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: index_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
