//! # WRITEM Statement
//!
//! Writes the COSY memory contents of a variable into parallel arrays.
//!
//! ## Syntax
//!
//! ```text
//! WRITEM input_var var_info length dp_array int_array da_params;
//! ```
//!
//! ## Arguments (COSY spec: c v c v v v)
//!
//! 1. `input_var`  — any type — the variable being serialized (input, `c`)
//! 2. `var_info`   — VE — receives `[type_code, length, version]` (output, `v`)
//! 3. `length`     — RE — allocation hint (input, `c`)
//! 4. `dp_array`   — VE — receives double-precision payload (output, `v`)
//! 5. `int_array`  — VE — receives integer metadata (output, `v`)
//! 6. `da_params`  — VE — receives DA parameters (output, `v`)
//!
//! WRITEM followed by READM round-trips correctly.

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement, ValueKind, add_context_to_all},
};

/// AST node for `WRITEM input var_info length dp_array int_array da_params;`.
#[derive(Debug)]
pub struct WritemStatement {
    /// arg 1: the variable to serialize (expr, `c`)
    pub input_expr: Expr,
    /// arg 2: output — variable info VE (variable_identifier, `v`)
    pub var_info: VariableIdentifier,
    /// arg 3: length hint (expr, `c`) — parsed for compatibility; actual length is set by procedure
    #[allow(dead_code)]
    pub length_expr: Expr,
    /// arg 4: output — double-precision payload VE (variable_identifier, `v`)
    pub dp_array: VariableIdentifier,
    /// arg 5: output — integer metadata VE (variable_identifier, `v`)
    pub int_array: VariableIdentifier,
    /// arg 6: output — DA parameters VE (variable_identifier, `v`)
    pub da_params: VariableIdentifier,
}

impl FromRule for WritemStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::writem,
            "Expected `writem` rule when building WRITEM statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // arg 1: input expr
        let input_pair = inner.next().context("Missing arg 1 (input) in WRITEM!")?;
        let input_expr = Expr::from_rule(input_pair)
            .context("Failed to parse input expression in WRITEM")?
            .ok_or_else(|| anyhow::anyhow!("Expected input expression in WRITEM"))?;

        // arg 2: var_info variable_identifier
        let var_info_pair = inner
            .next()
            .context("Missing arg 2 (var_info) in WRITEM!")?;
        let var_info = VariableIdentifier::from_rule(var_info_pair)
            .context("Failed to parse var_info identifier in WRITEM")?
            .ok_or_else(|| anyhow::anyhow!("Expected var_info identifier in WRITEM"))?;

        // arg 3: length expr
        let length_pair = inner.next().context("Missing arg 3 (length) in WRITEM!")?;
        let length_expr = Expr::from_rule(length_pair)
            .context("Failed to parse length expression in WRITEM")?
            .ok_or_else(|| anyhow::anyhow!("Expected length expression in WRITEM"))?;

        // arg 4: dp_array variable_identifier
        let dp_pair = inner
            .next()
            .context("Missing arg 4 (dp_array) in WRITEM!")?;
        let dp_array = VariableIdentifier::from_rule(dp_pair)
            .context("Failed to parse dp_array identifier in WRITEM")?
            .ok_or_else(|| anyhow::anyhow!("Expected dp_array identifier in WRITEM"))?;

        // arg 5: int_array variable_identifier
        let int_pair = inner
            .next()
            .context("Missing arg 5 (int_array) in WRITEM!")?;
        let int_array = VariableIdentifier::from_rule(int_pair)
            .context("Failed to parse int_array identifier in WRITEM")?
            .ok_or_else(|| anyhow::anyhow!("Expected int_array identifier in WRITEM"))?;

        // arg 6: da_params variable_identifier
        let da_pair = inner
            .next()
            .context("Missing arg 6 (da_params) in WRITEM!")?;
        let da_params = VariableIdentifier::from_rule(da_pair)
            .context("Failed to parse da_params identifier in WRITEM")?
            .ok_or_else(|| anyhow::anyhow!("Expected da_params identifier in WRITEM"))?;

        Ok(Some(WritemStatement {
            input_expr,
            var_info,
            length_expr,
            dp_array,
            int_array,
            da_params,
        }))
    }
}


impl Transpile for WritemStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Transpile the input expression (the variable being serialized)
        let input_output = match self.input_expr.transpile(context) {
            Ok(o) => {
                requested_variables.extend(o.requested_variables.iter().cloned());
                o
            }
            Err(e) => {
                errors.extend(add_context_to_all(
                    e,
                    "...while transpiling input in WRITEM".to_string(),
                ));
                return Err(errors);
            }
        };

        // Transpile output variable identifiers
        let var_info_output = match self.var_info.transpile(context) {
            Ok(o) => {
                requested_variables.extend(o.requested_variables.iter().cloned());
                o
            }
            Err(e) => {
                errors.extend(add_context_to_all(
                    e,
                    "...while transpiling var_info in WRITEM".to_string(),
                ));
                return Err(errors);
            }
        };

        let dp_output = match self.dp_array.transpile(context) {
            Ok(o) => {
                requested_variables.extend(o.requested_variables.iter().cloned());
                o
            }
            Err(e) => {
                errors.extend(add_context_to_all(
                    e,
                    "...while transpiling dp_array in WRITEM".to_string(),
                ));
                return Err(errors);
            }
        };

        let int_output = match self.int_array.transpile(context) {
            Ok(o) => {
                requested_variables.extend(o.requested_variables.iter().cloned());
                o
            }
            Err(e) => {
                errors.extend(add_context_to_all(
                    e,
                    "...while transpiling int_array in WRITEM".to_string(),
                ));
                return Err(errors);
            }
        };

        let da_output = match self.da_params.transpile(context) {
            Ok(o) => {
                requested_variables.extend(o.requested_variables.iter().cloned());
                o
            }
            Err(e) => {
                errors.extend(add_context_to_all(
                    e,
                    "...while transpiling da_params in WRITEM".to_string(),
                ));
                return Err(errors);
            }
        };

        if !errors.is_empty() {
            return Err(errors);
        }

        // Build lvalue assignments for the four output variables
        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let var_info_assign = make_lvalue(
            &var_info_output.serialization,
            var_info_output.value_kind,
            "_writem_var_info",
        );
        let dp_assign = make_lvalue(&dp_output.serialization, dp_output.value_kind, "_writem_dp");
        let int_assign = make_lvalue(
            &int_output.serialization,
            int_output.value_kind,
            "_writem_int",
        );
        let da_assign = make_lvalue(&da_output.serialization, da_output.value_kind, "_writem_da");

        let input_ref = input_output.as_ref();

        let serialization = format!(
            "{{ \
                let (_writem_var_info, _writem_dp, _writem_int, _writem_da) = \
                    rosy_lib::core::mem_serial::RosyWritem::writem({input_ref}); \
                {var_info_assign}; \
                {dp_assign}; \
                {int_assign}; \
                {da_assign}; \
            }}",
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for WritemStatement {}
