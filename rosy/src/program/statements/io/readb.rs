//! # READB Statement (Binary Read)
//!
//! Reads a binary value from a file unit into a variable.
//!
//! ## Syntax
//!
//! ```text
//! READB unit variable;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/readb.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::{Expr, core::variable_identifier::VariableIdentifier},
    transpile::{TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr, TranspileableStatement, add_context_to_all},
};

/// AST node for `READB unit variable;`.
#[derive(Debug)]
pub struct ReadbStatement {
    pub unit: Expr,
    pub identifier: VariableIdentifier,
}

impl FromRule for ReadbStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::readb,
            "Expected `readb` rule when building READB statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner
            .next()
            .context("Missing unit expression in `readb` statement!")?;
        let unit = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in `readb` statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in `readb` statement"))?;

        let identifier = VariableIdentifier::from_rule(
            inner
                .next()
                .context("Missing second token `variable_identifier`!")?,
        )
        .context("...while building variable identifier for READB statement")?
        .ok_or_else(|| anyhow::anyhow!("Expected variable identifier for READB statement"))?;

        Ok(Some(ReadbStatement { unit, identifier }))
    }
}
impl Transpile for ReadbStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Serialize the identifier
        let serialized_variable_identifier = match self.identifier.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables);
                output.serialization
            }
            Err(vec_err) => {
                for err in vec_err {
                    errors.push(err.context(format!(
                        "...while transpiling identifier expression for READB into '{}'",
                        self.identifier.name
                    )));
                }
                String::new()
            }
        };

        // Get the variable type
        let variable_type = match self.identifier.type_of(context) {
            Ok(var_type) => var_type,
            Err(e) => {
                errors.push(e.context(format!(
                    "...while determining type of variable identifier for READB into '{}'",
                    self.identifier.name
                )));
                return Err(errors);
            }
        };

        let serialized_variable_type = variable_type.as_rust_type();

        // Transpile unit expression
        let unit_output = self.unit.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in READB".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        let serialization = format!(
            "{{\n\tlet __rosy_unit = ({}).round() as u64;\n\tlet _readb_data = rosy_lib::core::file_io::rosy_readb_from_unit(__rosy_unit)?;\n\t{} = <{} as rosy_lib::core::file_io::RosyFromBinary>::from_binary(&_readb_data)?;\n}}",
            unit_output.as_value(),
            serialized_variable_identifier,
            serialized_variable_type,
        );

        if errors.is_empty() {
            Ok(TranspilationOutput {
                serialization,
                requested_variables,
                ..Default::default()
            })
        } else {
            Err(errors)
        }
    }
}

impl TranspileableStatement for ReadbStatement {}
