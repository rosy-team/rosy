//! # MTREE Statement
//!
//! Computes the tree representation of a DA array for fast polynomial evaluation.
//!
//! ## Syntax
//!
//! ```text
//! MTREE da_array elements coeff_array steer1 steer2 elements2 tree_length;
//! ```
//!
//! - `da_array`    — input DA vector (RE ** 1 containing DA values, or accessed as DA array)
//! - `elements`    — number of elements in the DA array (RE)
//! - `coeff_array` — variable for output coefficient array (RE ** 1)
//! - `steer1`      — variable for steering array 1 (RE ** 1)
//! - `steer2`      — variable for steering array 2 (RE ** 1)
//! - `elements2`   — variable for number of tree elements (RE)
//! - `tree_length` — variable for total tree length (RE)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/mtree.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        ValueKind, add_context_to_all,
    },
};

#[derive(Debug)]
pub struct MtreeStatement {
    pub da_array_expr: Expr,
    pub elements_expr: Expr,
    pub coeff_array_expr: Expr,
    pub steer1_expr: Expr,
    pub steer2_expr: Expr,
    pub elements2_expr: Expr,
    pub tree_length_expr: Expr,
}

impl FromRule for MtreeStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::mtree,
            "Expected `mtree` rule when building MTREE statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let da_array_pair = inner
            .next()
            .context("Missing DA array parameter in MTREE!")?;
        let da_array_expr = Expr::from_rule(da_array_pair)
            .context("Failed to build DA array expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected DA array expression in MTREE"))?;

        let elements_pair = inner
            .next()
            .context("Missing elements parameter in MTREE!")?;
        let elements_expr = Expr::from_rule(elements_pair)
            .context("Failed to build elements expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected elements expression in MTREE"))?;

        let coeff_pair = inner
            .next()
            .context("Missing coeff_array parameter in MTREE!")?;
        let coeff_array_expr = Expr::from_rule(coeff_pair)
            .context("Failed to build coeff_array expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected coeff_array expression in MTREE"))?;

        let steer1_pair = inner.next().context("Missing steer1 parameter in MTREE!")?;
        let steer1_expr = Expr::from_rule(steer1_pair)
            .context("Failed to build steer1 expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected steer1 expression in MTREE"))?;

        let steer2_pair = inner.next().context("Missing steer2 parameter in MTREE!")?;
        let steer2_expr = Expr::from_rule(steer2_pair)
            .context("Failed to build steer2 expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected steer2 expression in MTREE"))?;

        let elements2_pair = inner
            .next()
            .context("Missing elements2 parameter in MTREE!")?;
        let elements2_expr = Expr::from_rule(elements2_pair)
            .context("Failed to build elements2 expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected elements2 expression in MTREE"))?;

        let tree_length_pair = inner
            .next()
            .context("Missing tree_length parameter in MTREE!")?;
        let tree_length_expr = Expr::from_rule(tree_length_pair)
            .context("Failed to build tree_length expression in MTREE")?
            .ok_or_else(|| anyhow::anyhow!("Expected tree_length expression in MTREE"))?;

        Ok(Some(MtreeStatement {
            da_array_expr,
            elements_expr,
            coeff_array_expr,
            steer1_expr,
            steer2_expr,
            elements2_expr,
            tree_length_expr,
        }))
    }
}

impl Transpile for MtreeStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let da_array_output = self.da_array_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling DA array in MTREE".to_string())
        })?;
        requested_variables.extend(da_array_output.requested_variables.iter().cloned());

        let elements_output = self.elements_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling elements in MTREE".to_string())
        })?;
        requested_variables.extend(elements_output.requested_variables.iter().cloned());

        let coeff_output = self.coeff_array_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling coeff_array in MTREE".to_string())
        })?;
        requested_variables.extend(coeff_output.requested_variables.clone());

        let steer1_output = self.steer1_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling steer1 in MTREE".to_string())
        })?;
        requested_variables.extend(steer1_output.requested_variables.clone());

        let steer2_output = self.steer2_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling steer2 in MTREE".to_string())
        })?;
        requested_variables.extend(steer2_output.requested_variables.clone());

        let elements2_output = self.elements2_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling elements2 in MTREE".to_string())
        })?;
        requested_variables.extend(elements2_output.requested_variables.clone());

        let tree_length_output = self.tree_length_expr.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling tree_length in MTREE".to_string())
        })?;
        requested_variables.extend(tree_length_output.requested_variables.clone());

        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let coeff_assign = make_lvalue(
            &coeff_output.serialization,
            coeff_output.value_kind,
            "rosy_mtree_c",
        );
        let steer1_assign = make_lvalue(
            &steer1_output.serialization,
            steer1_output.value_kind,
            "rosy_mtree_s1",
        );
        let steer2_assign = make_lvalue(
            &steer2_output.serialization,
            steer2_output.value_kind,
            "rosy_mtree_s2",
        );
        let elements2_assign = make_lvalue(
            &elements2_output.serialization,
            elements2_output.value_kind,
            "rosy_mtree_ne",
        );
        let tree_length_assign = make_lvalue(
            &tree_length_output.serialization,
            tree_length_output.value_kind,
            "rosy_mtree_tl",
        );

        let serialization = format!(
            "{{ let (rosy_mtree_c, rosy_mtree_s1, rosy_mtree_s2, rosy_mtree_ne, rosy_mtree_tl) = rosy_lib::core::mtree::rosy_mtree({da_array}, rosy_as_usize(&({elements})))?; {coeff_assign}; {steer1_assign}; {steer2_assign}; {elements2_assign}; {tree_length_assign}; }}",
            da_array = da_array_output.as_ref(),
            elements = elements_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for MtreeStatement {}
