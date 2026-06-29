//! # CONS Function
//!
//! Extracts the constant (scalar) part of a value.
//!
//! ## Syntax
//!
//! ```text
//! CONS(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE    | RE     |
//! | CM    | CM     |
//! | VE    | RE     |
//! | DA    | RE     |
//! | CD    | CM     |
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
use crate::rosy_lib::RosyType;
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result};
use std::collections::HashSet;

/// AST node for the `CONS(expr)` intrinsic function.
#[derive(Debug)]
pub struct ConsExpr {
    pub expr: Box<Expr>,
}

impl FromRule for ConsExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::cons_fn,
            "Expected cons_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `CONS`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `CONS`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `CONS`"))?,
        );
        Ok(Some(ConsExpr { expr }))
    }
}

impl Transpile for ConsExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_type = self.expr.type_of(context).map_err(|e| vec![e])?;

        let inner_output = self.expr.transpile(context)?;

        let serialization = if inner_type == RosyType::RE() {
            inner_output.as_value()
        } else {
            format!("RosyCONS::rosy_cons({})?", inner_output.as_ref())
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}

impl TranspileableExpr for ConsExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use crate::rosy_lib::intrinsics::cons;

        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in CONS")?;

        cons::get_return_type(&inner_type)
            .ok_or_else(|| anyhow::anyhow!("CONS not supported for type: {:?}", inner_type))
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
        // CONS has non-uniform type mapping (VE->RE, DA->RE, RE->RE, CM->CM),
        // so we cannot represent it with a type-preserving recipe.
        ExprRecipe::Unknown(None)
    }
}
