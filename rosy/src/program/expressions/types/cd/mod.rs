//! # CD — Complex Differential Algebra Constructor
//!
//! Creates a CD (Complex Differential Algebra / complex Taylor series)
//! value from a variable index.
//!
//! ## Syntax
//!
//! ```text
//! CD(n)          { creates CD identity for variable n }
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

use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::{
    ast::{FromRule, Rule},
    program::expressions::Expr,
    transpile::{
        ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableExpr, ValueKind,
    },
};
use anyhow::{Context, Error};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `CD(n)` constructor expression.
#[derive(Debug)]
pub struct CDExpr {
    pub index: Box<Expr>,
}

impl FromRule for CDExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> anyhow::Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::builtin_function,
            "Expected builtin_function (CD), got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let name = inner
            .next()
            .map(|p| p.as_str().trim().to_ascii_uppercase())
            .unwrap_or_default();
        anyhow::ensure!(name == "CD", "Expected CD constructor, got `{name}`");
        let expr_pair = inner.next().context("Missing inner expression for `CD`!")?;
        let index = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `CD`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `CD`"))?,
        );
        Ok(Some(CDExpr { index }))
    }
}
impl TranspileableExpr for CDExpr {
    fn type_of(&self, _context: &TranspilationInputContext) -> anyhow::Result<RosyType> {
        Ok(RosyType::CD())
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.index, ctx),
        }
    }
    fn build_expr_recipe(
        &self,
        _resolver: &TypeResolver,
        _ctx: &ScopeContext,
        _deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        ExprRecipe::Literal(RosyType::CD())
    }
}
impl Transpile for CDExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Transpile the index expression
        let index_output = self.index.transpile(context).map_err(|errs| {
            errs.into_iter()
                .map(|e| e.context("...while transpiling CD index expression"))
                .collect::<Vec<_>>()
        })?;

        // Use CD::variable(usize) to create a CD differential variable
        let serialization = format!("CD::variable({} as usize)?", index_output.as_value());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: index_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
