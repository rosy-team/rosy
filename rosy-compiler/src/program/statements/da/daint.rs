//! # DAINT Statement (DA Integration)
//!
//! Integrates a DA vector array in place with respect to a variable index.
//! Terms that would exceed the truncation order are dropped.
//!
//! ## Syntax
//!
//! ```text
//! DAINT da_var var_index;
//! ```
//!
//! Arguments:
//! 1. `da_var`    (DA array, in/out) — DA vector to integrate in place
//! 2. `var_index` (RE, integer)      — 1-based index of the variable to integrate w.r.t.
//!
//! > **COSY note**: In COSY INFINITY, `DAINT` takes 3 arguments `(index, input, result)`
//! > and writes to a separate output variable. Rosy's form is in-place `(da_var, index)`.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/daint.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use rosy_lib::RosyType;
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr,
        TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for `DAINT da_var var_index;`.
#[derive(Debug)]
pub struct DaintStatement {
    pub da_expr: Expr,
    pub index_expr: Expr,
    /// COSY `DAINT index input dest` writes dest. Rosy 2-arg is in-place.
    pub dest_expr: Option<Expr>,
}

impl FromRule for DaintStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daint,
            "Expected `daint` rule when building DAINT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let a = Expr::from_rule(inner.next().context("Missing first DAINT arg")?)?
            .ok_or_else(|| anyhow::anyhow!("Expected first DAINT arg"))?;
        let b = Expr::from_rule(inner.next().context("Missing second DAINT arg")?)?
            .ok_or_else(|| anyhow::anyhow!("Expected second DAINT arg"))?;
        let third = inner
            .next()
            .filter(|p| p.as_rule() != Rule::semicolon)
            .map(|p| Expr::from_rule(p))
            .transpose()?
            .flatten();

        let (index_expr, da_expr, dest_expr) = if third.is_some() {
            (a, b, third)
        } else {
            (b, a, None)
        };

        Ok(Some(DaintStatement {
            da_expr,
            index_expr,
            dest_expr,
        }))
    }
}

impl Transpile for DaintStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_ty = self.da_expr.type_of(context).map_err(|e| vec![e])?;
        let da_output = self.da_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling da_var in DAINT".to_string())
        })?;
        requested_variables.extend(da_output.requested_variables.iter().cloned());

        let index_output = self.index_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling var_index in DAINT".to_string())
        })?;
        requested_variables.extend(index_output.requested_variables.iter().cloned());

        let serialization = if let Some(dest) = &self.dest_expr {
            let dest_out = dest.transpile(context).map_err(|e| {
                add_context_to_all(e, "...while transpiling dest in DAINT".to_string())
            })?;
            requested_variables.extend(dest_out.requested_variables.iter().cloned());
            let dest_ty = dest.type_of(context).unwrap_or_else(|_| RosyType::ANY());
            let store = if dest_ty.is_any() {
                format!("{} = RosyValue::from(__daint)", dest_out.as_value())
            } else if dest_ty.base_type == rosy_lib::RosyBaseType::RE {
                format!("{} = RosyValue::from(__daint).as_f64()", dest_out.as_value())
            } else {
                format!("{} = __daint", dest_out.as_value())
            };
            format!(
                "{{ let mut __daint = {}.clone(); rosy_lib::core::da_ops::rosy_daint(&mut __daint, rosy_as_usize(&({})))?; {store}; }}",
                da_output.as_owned(&da_ty),
                index_output.as_value(),
            )
        } else {
            format!(
                "rosy_lib::core::da_ops::rosy_daint({}, rosy_as_usize(&({})))?;",
                da_output.as_mut_ref(),
                index_output.as_value(),
            )
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DaintStatement {}
