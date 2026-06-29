//! # VARMEM Function
//!
//! Returns the current memory address of an object as a real number.
//! Since Rosy transpiles to Rust (not Fortran), true COSY memory addresses
//! are meaningless. VARMEM returns the actual Rust pointer address cast to f64.
//!
//! ## Syntax
//!
//! ```text
//! VARMEM(expr)
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
//! ## COSY Example
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

/// AST node for the `VARMEM(expr)` system function.
#[derive(Debug)]
pub struct VarmemExpr {
    pub expr: Box<Expr>,
}

impl FromRule for VarmemExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::varmem,
            "Expected varmem rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `VARMEM`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `VARMEM`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `VARMEM`"))?,
        );
        Ok(Some(VarmemExpr { expr }))
    }
}
impl Transpile for VarmemExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Transpile the inner expression
        let inner_output = self.expr.transpile(context)?;

        // Generate the transpiled code — as_ref() already produces &expr, matching &self signature
        let serialization = format!("RosyVARMEM::rosy_varmem({})", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for VarmemExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        use crate::rosy_lib::intrinsics::varmem;

        // Get the type of the inner expression
        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in VARMEM")?;

        // Use the VARMEM registry to get the return type
        varmem::get_return_type(&inner_type)
            .ok_or_else(|| anyhow::anyhow!("VARMEM not supported for type: {:?}", inner_type))
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
