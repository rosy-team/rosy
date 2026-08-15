//! # LENGTH Function
//!
//! Returns the length/size of a value. For vectors, returns the number of
//! elements. For strings, returns the character count. For scalars, returns 1.
//!
//! ## Syntax
//!
//! ```text
//! LENGTH(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | RE |
//! | ST | RE |
//! | LO | RE |
//! | CM | RE |
//! | VE | RE |
//! | DA | RE |
//! | CD | RE |
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

/// AST node for the `LENGTH(expr)` system function.
#[derive(Debug)]
pub struct LengthExpr {
    pub expr: Box<Expr>,
}

impl FromRule for LengthExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::length,
            "Expected length rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `LENGTH`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `LENGTH`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `LENGTH`"))?,
        );
        Ok(Some(LengthExpr { expr }))
    }
}
impl Transpile for LengthExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Transpile the inner expression
        let inner_output = self.expr.transpile(context)?;

        // Generate the transpiled code
        let serialization = format!("RosyLENGTH::rosy_length({})", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for LengthExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        
        // Get the type of the inner expression
        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in LENGTH")?;

        // Use the LENGTH registry to get the return type
        rosy_lib::unary_return_type("LENGTH", &inner_type)
            .ok_or_else(|| anyhow::anyhow!("LENGTH not supported for type: {:?}", inner_type))
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
