//! # TYPE Function
//!
//! Returns the COSY type code of a value as RE.
//!
//! ## Syntax
//!
//! ```text
//! TYPE(expr)
//! ```
//!
//! ## Type Codes
//!
//! | Input | Code |
//! |-------|------|
//! | RE    |  1   |
//! | ST    |  2   |
//! | LO    |  3   |
//! | CM    |  4   |
//! | VE    |  5   |
//! | DA    |  6   |
//! | CD    |  7   |
//! | GR    |  8   |
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

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// AST node for the `TYPE(expr)` intrinsic function.
#[derive(Debug)]
pub struct TypeFnExpr {
    pub expr: Box<Expr>,
}

impl FromRule for TypeFnExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::type_fn,
            "Expected type_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `TYPE`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `TYPE`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `TYPE`"))?,
        );
        Ok(Some(TypeFnExpr { expr }))
    }
}

impl Transpile for TypeFnExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_output = self.expr.transpile(context)?;

        let serialization = format!("RosyTYPE::rosy_type({})?", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}

impl TranspileableExpr for TypeFnExpr {
    fn type_of(&self, _context: &TranspilationInputContext) -> Result<RosyType> {
        // TYPE always returns RE regardless of input
        Ok(RosyType::RE())
    }

    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        ExprFunctionCallResult::HasFunctionCalls {
            result: resolver.discover_expr_function_calls(&self.expr, ctx),
        }
    }
    fn build_expr_recipe(
        &self,
        _resolver: &TypeResolver,
        _ctx: &ScopeContext,
        _deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        ExprRecipe::Literal(RosyType::RE())
    }
}
