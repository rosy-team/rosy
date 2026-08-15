//! # CMPLX Function (Convert to Complex)
//!
//! Converts a value to complex type.
//!
//! ## Syntax
//!
//! ```text
//! CMPLX(expr)
//! ```
//!
//! ## Type Compatibility
//!
//! | Input | Result |
//! |-------|--------|
//! | RE | CM |
//! | CM | CM |
//! | DA | CD |
//! | CD | CD |
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

/// AST node for the `CMPLX(expr)` intrinsic function.
#[derive(Debug)]
pub struct CmplxExpr {
    pub expr: Box<Expr>,
}

impl FromRule for CmplxExpr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        anyhow::ensure!(
            pair.as_rule() == Rule::cmplx_fn,
            "Expected cmplx_fn rule, got {:?}",
            pair.as_rule()
        );
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .context("Missing inner expression for `CMPLX`!")?;
        let expr = Box::new(
            Expr::from_rule(expr_pair)
                .context("Failed to build expression for `CMPLX`")?
                .ok_or_else(|| anyhow::anyhow!("Expected expression for `CMPLX`"))?,
        );
        Ok(Some(CmplxExpr { expr }))
    }
}
impl Transpile for CmplxExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let inner_output = self.expr.transpile(context)?;

        let serialization = format!("RosyCMPLX::rosy_cmplx({})?", inner_output.as_ref());

        Ok(TranspilationOutput {
            serialization,
            requested_variables: inner_output.requested_variables,
            value_kind: ValueKind::Owned,
        })
    }
}
impl TranspileableExpr for CmplxExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        
        let inner_type = self
            .expr
            .type_of(context)
            .context("Failed to determine type of inner expression in CMPLX")?;

        rosy_lib::unary_return_type("CMPLX", &inner_type)
            .ok_or_else(|| anyhow::anyhow!("CMPLX not supported for type: {:?}", inner_type))
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
        // CMPLX has non-uniform output types (RE/CM->CM, DA/CD->CD).
        // Defer to type_of() which calls get_return_type().
        ExprRecipe::Unknown(None)
    }
}
