//! # READM Statement
//!
//! Reads arrays (produced by WRITEM) back into a Rosy variable.
//!
//! ## Syntax
//!
//! ```text
//! READM output_var var_info length dp_array int_array da_params;
//! ```
//!
//! ## Arguments (COSY spec: v c c c c c)
//!
//! 1. `output_var` — any type — the variable being populated (output, `v`)
//! 2. `var_info`   — VE — `[type_code, length, version]` (input, `c`)
//! 3. `length`     — RE — number of entries in dp_array (input, `c`)
//! 4. `dp_array`   — VE — double-precision payload (input, `c`)
//! 5. `int_array`  — VE — integer metadata (input, `c`)
//! 6. `da_params`  — VE — DA parameters (input, `c`)
//!
//! READM is the inverse of WRITEM and round-trips correctly.
//!
//! ## Fixes applied (vs original implementation)
//! - Bounds check before `var_info[1]` is no longer needed in transpiler code
//!   since `readm()` impls now read `var_info[1]` internally with a bounds check.
//! - Type code validation: each `RosyReadm` impl checks the type code in var_info\[0\].
//! - DA/CD config compatibility: checked inside `RosyReadm` impls.
//! - `length_expr` requested_variables are now propagated (fix #5).
//! - The `length` argument is passed as `_length` since the authoritative length
//!   is taken from `var_info[1]` by the impls; passing it avoids the double-eval
//!   issue that existed when the transpiler computed `{var_info_s}[1]` twice.

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::{Expr, core::variable_identifier::VariableIdentifier},
        statements::SourceLocation,
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableExpr, TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult,
        ValueKind, add_context_to_all,
    },
};

/// AST node for `READM output_var var_info length dp_array int_array da_params;`.
#[derive(Debug)]
pub struct ReadmStatement {
    /// arg 1: output variable (variable_identifier, `v`)
    pub output_var: VariableIdentifier,
    /// arg 2: var_info input (expr, `c`)
    pub var_info_expr: Expr,
    /// arg 3: length (expr, `c`)
    pub length_expr: Expr,
    /// arg 4: dp_array (expr, `c`)
    pub dp_array_expr: Expr,
    /// arg 5: int_array (expr, `c`)
    pub int_array_expr: Expr,
    /// arg 6: da_params (expr, `c`)
    pub da_params_expr: Expr,
}

impl FromRule for ReadmStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::readm,
            "Expected `readm` rule when building READM statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // arg 1: output variable
        let out_pair = inner
            .next()
            .context("Missing arg 1 (output_var) in READM!")?;
        let output_var = VariableIdentifier::from_rule(out_pair)
            .context("Failed to parse output variable in READM")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable in READM"))?;

        // arg 2: var_info
        let vi_pair = inner.next().context("Missing arg 2 (var_info) in READM!")?;
        let var_info_expr = Expr::from_rule(vi_pair)
            .context("Failed to parse var_info expression in READM")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_info expression in READM"))?;

        // arg 3: length
        let len_pair = inner.next().context("Missing arg 3 (length) in READM!")?;
        let length_expr = Expr::from_rule(len_pair)
            .context("Failed to parse length expression in READM")?
            .ok_or_else(|| anyhow::anyhow!("Expected length expression in READM"))?;

        // arg 4: dp_array
        let dp_pair = inner.next().context("Missing arg 4 (dp_array) in READM!")?;
        let dp_array_expr = Expr::from_rule(dp_pair)
            .context("Failed to parse dp_array expression in READM")?
            .ok_or_else(|| anyhow::anyhow!("Expected dp_array expression in READM"))?;

        // arg 5: int_array
        let int_pair = inner
            .next()
            .context("Missing arg 5 (int_array) in READM!")?;
        let int_array_expr = Expr::from_rule(int_pair)
            .context("Failed to parse int_array expression in READM")?
            .ok_or_else(|| anyhow::anyhow!("Expected int_array expression in READM"))?;

        // arg 6: da_params
        let da_pair = inner
            .next()
            .context("Missing arg 6 (da_params) in READM!")?;
        let da_params_expr = Expr::from_rule(da_pair)
            .context("Failed to parse da_params expression in READM")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_params expression in READM"))?;

        Ok(Some(ReadmStatement {
            output_var,
            var_info_expr,
            length_expr,
            dp_array_expr,
            int_array_expr,
            da_params_expr,
        }))
    }
}

impl TranspileableStatement for ReadmStatement {
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

impl Transpile for ReadmStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Transpile output variable (we need its lvalue and its Rust type)
        let out_output = match self.output_var.transpile(context) {
            Ok(o) => {
                requested_variables.extend(o.requested_variables.iter().cloned());
                o
            }
            Err(e) => {
                errors.extend(add_context_to_all(
                    e,
                    "...while transpiling output_var in READM".to_string(),
                ));
                return Err(errors);
            }
        };

        // Get the Rust type of the output variable so we can call the right impl
        let var_type = match self.output_var.type_of(context) {
            Ok(t) => t,
            Err(e) => {
                errors.push(e.context("...while determining type of output_var in READM"));
                return Err(errors);
            }
        };
        let rust_type = var_type.as_rust_type();

        // Helper: transpile a single expression, accumulating errors and requested_variables.
        let mut macro_transpile = |expr: &Expr,
                                   label: &str,
                                   errors: &mut Vec<Error>,
                                   reqs: &mut BTreeSet<String>|
         -> Option<String> {
            match expr.transpile(context) {
                Ok(o) => {
                    reqs.extend(o.requested_variables.iter().cloned());
                    Some(o.as_value())
                }
                Err(e) => {
                    errors.extend(add_context_to_all(
                        e,
                        format!("...while transpiling {label} in READM"),
                    ));
                    None
                }
            }
        };

        let var_info_s = macro_transpile(
            &self.var_info_expr,
            "var_info",
            &mut errors,
            &mut requested_variables,
        );
        // Fix #5: propagate length_expr requested_variables
        let length_s = macro_transpile(
            &self.length_expr,
            "length",
            &mut errors,
            &mut requested_variables,
        );
        let dp_s = macro_transpile(
            &self.dp_array_expr,
            "dp_array",
            &mut errors,
            &mut requested_variables,
        );
        let int_s = macro_transpile(
            &self.int_array_expr,
            "int_array",
            &mut errors,
            &mut requested_variables,
        );
        let da_s = macro_transpile(
            &self.da_params_expr,
            "da_params",
            &mut errors,
            &mut requested_variables,
        );

        if !errors.is_empty() {
            return Err(errors);
        }

        let var_info_s = var_info_s.unwrap();
        let length_s = length_s.unwrap();
        let dp_s = dp_s.unwrap();
        let int_s = int_s.unwrap();
        let da_s = da_s.unwrap();

        // Build lvalue assignment for output variable
        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let out_assign = make_lvalue(
            &out_output.serialization,
            out_output.value_kind,
            "_readm_result",
        );

        // Fix #1/#4: var_info is stored in a temp binding `_readm_var_info` so it
        // is evaluated exactly once, and the bounds check happens inside readm().
        // The `length` argument is also evaluated once and passed through.
        let serialization = format!(
            "{{ \
                let _readm_var_info = {var_info_s}; \
                let _readm_result = <{rust_type} as rosy_lib::core::mem_serial::RosyReadm>::readm(\
                    &_readm_var_info, {length_s}, &{dp_s}, &{int_s}, &{da_s}\
                )?; \
                {out_assign}; \
            }}",
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
