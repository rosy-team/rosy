//! # DAINI Statement (DA Initialization)
//!
//! Initializes the Differential Algebra (Taylor series) environment with
//! a specified computation order and number of variables.
//!
//! ## Syntax
//!
//! ```text
//! DAINI order nvars; { note - Rosy doesn't need the 3rd or 4th args }
//! ```
//!
//! Must be called before any DA or CD operations.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/da/da_init.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};

use crate::{
    ast::*,
    program::expressions::Expr,
    syntax_config,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
    },
};

/// AST node for the `DAINI order nvars [output_unit num_monomials_out];` statement.
#[derive(Debug)]
pub struct DAInitStatement {
    pub order: Expr,
    pub number_of_variables: Expr,
    /// Optional 3rd argument: output unit for debug dump of addressing arrays.
    /// When nonzero, prints the monomial index → exponents mapping.
    pub output_unit: Option<Expr>,
    /// Optional 4th argument: variable to receive the total number of monomials.
    /// COSY writes back C(order+nvars, nvars) into this variable.
    pub num_monomials_out: Option<Expr>,
}

impl FromRule for DAInitStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::daini,
            "Expected `daini` rule when building DA init, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // Parse the first expression (order)
        let order_pair = inner
            .next()
            .context("Missing order parameter in DAINI statement!")?;
        let order_expr = Expr::from_rule(order_pair)
            .context("Failed to build order expression in DAINI statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected expression for order in DAINI statement"))?;

        // Parse the second expression (number of variables)
        let num_vars_pair = inner
            .next()
            .context("Missing number of variables parameter in DAINI statement!")?;
        let num_vars_expr = Expr::from_rule(num_vars_pair)
            .context("Failed to build number of variables expression in DAINI statement!")?
            .ok_or_else(|| {
                anyhow::anyhow!("Expected expression for number of variables in DAINI statement")
            })?;

        // Parse optional 3rd argument (output unit for debug dump)
        let mut output_unit = None;
        let mut num_monomials_out = None;

        if let Some(third_pair) = inner.next().filter(|p| p.as_rule() == Rule::expr) {
            let third_expr = Expr::from_rule(third_pair)
                .context("Failed to build 3rd expression in DAINI statement!")?;
            output_unit = third_expr;

            // Parse 4th argument: either `daini_nm_zero` (literal 0, no writeback)
            // or `expr` (variable to receive monomial count)
            if let Some(fourth_pair) = inner.next() {
                match fourth_pair.as_rule() {
                    Rule::daini_nm_zero => {
                        // Literal 0 — no writeback needed
                    }
                    Rule::expr => {
                        let fourth_expr = Expr::from_rule(fourth_pair)
                            .context("Failed to build 4th expression in DAINI statement!")?;
                        num_monomials_out = fourth_expr;
                    }
                    _ => anyhow::bail!(
                        "Unexpected rule in DAINI 4th argument: {:?}",
                        fourth_pair.as_rule()
                    ),
                }
            } else if syntax_config::is_cosy_syntax() {
                anyhow::bail!(
                    "COSY syntax mode requires all 4 arguments in DAINI statements.\n\
                     Expected: DAINI <order> <nvars> <output_unit> <num_monomials> ;\n\
                     Hint: If you intended to use Rosy syntax, remove the `--cosy-syntax` flag."
                );
            }
        } else if syntax_config::is_cosy_syntax() {
            anyhow::bail!(
                "COSY syntax mode requires all 4 arguments in DAINI statements.\n\
                 Expected: DAINI <order> <nvars> <output_unit> <num_monomials> ;\n\
                 Hint: If you intended to use Rosy syntax, remove the `--cosy-syntax` flag."
            );
        }

        Ok(Some(DAInitStatement {
            order: order_expr,
            number_of_variables: num_vars_expr,
            output_unit,
            num_monomials_out,
        }))
    }
}
impl Transpile for DAInitStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Transpile the order expression
        let order_output = self.order.transpile(context).map_err(|errs| {
            errs.into_iter()
                .map(|e| e.context("...while transpiling order expression in DAINI"))
                .collect::<Vec<_>>()
        })?;

        // Transpile the number of variables expression
        let num_vars_output = self
            .number_of_variables
            .transpile(context)
            .map_err(|errs| {
                errs.into_iter()
                    .map(|e| {
                        e.context("...while transpiling number of variables expression in DAINI")
                    })
                    .collect::<Vec<_>>()
            })?;

        let mut requested_variables = order_output.requested_variables.clone();
        requested_variables.extend(num_vars_output.requested_variables.iter().cloned());

        // Base: init DA and capture monomial count
        let mut serialization = format!(
            "taylor::cleanup_taylor();\n\t\tlet __daini_nm = taylor::init_taylor({} as u32, {} as usize)?;",
            order_output.as_value(),
            num_vars_output.as_value()
        );

        // Arg 3: debug dump of addressing arrays if nonzero
        if let Some(ref unit_expr) = self.output_unit {
            let unit_o = unit_expr.transpile(context).map_err(|errs| {
                errs.into_iter()
                    .map(|e| e.context("...while transpiling output unit in DAINI"))
                    .collect::<Vec<_>>()
            })?;
            requested_variables.extend(unit_o.requested_variables.iter().cloned());
            serialization.push_str(&format!(
                "\n\t\tif ({} as i64) != 0 {{ taylor::dump_addressing_arrays()?; }}",
                unit_o.as_value()
            ));
        }

        // Arg 4: write back num monomials
        if let Some(ref nm_expr) = self.num_monomials_out {
            let nm_o = nm_expr.transpile(context).map_err(|errs| {
                errs.into_iter()
                    .map(|e| e.context("...while transpiling num_monomials_out in DAINI"))
                    .collect::<Vec<_>>()
            })?;
            requested_variables.extend(nm_o.requested_variables.iter().cloned());
            serialization.push_str(&format!("\n\t\t{} = __daini_nm as f64;", nm_o.as_value()));
        }

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for DAInitStatement {}
