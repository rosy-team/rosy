//! # READ Statement
//!
//! Reads a value from a unit (file or console) into a variable.
//!
//! ## Syntax
//!
//! ```text
//! READ unit variable;
//! ```
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

use anyhow::{Context, Error, Result, anyhow, ensure};
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
        add_context_to_all,
    },
};

/// AST node for the `READ unit variable;` statement.
#[derive(Debug)]
pub struct ReadStatement {
    pub unit: Expr,
    pub identifier: VariableIdentifier,
}

impl FromRule for ReadStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::read,
            "Expected `read` rule when building read statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner
            .next()
            .context("Missing unit expression in `read` statement!")?;
        let unit = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in `read` statement!")?
            .ok_or_else(|| anyhow::anyhow!("Expected unit expression in `read` statement"))?;

        let identifier = VariableIdentifier::from_rule(
            inner
                .next()
                .context("Missing second token `variable_identifier`!")?,
        )
        .context("...while building variable identifier for read statement")?
        .ok_or_else(|| anyhow::anyhow!("Expected variable identifier for read statement"))?;

        Ok(Some(ReadStatement { unit, identifier }))
    }
}
impl TranspileableStatement for ReadStatement {
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
impl Transpile for ReadStatement {
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
                        "...while transpiling identifier expression for READ into '{}'",
                        self.identifier.name
                    )));
                }
                String::new() // dummy value to collect more errors
            }
        };

        // Get the variable type and ensure it's compatible with READ
        let variable_type = match self.identifier.type_of(context) {
            Ok(var_type) => var_type,
            Err(e) => {
                errors.push(e.context(format!(
                    "...while determining type of variable identifier for READ into '{}'",
                    self.identifier.name
                )));
                return Err(errors); // Cannot continue without the type
            }
        };
        if !crate::rosy_lib::intrinsics::from_st::can_be_obtained(&variable_type) {
            errors.push(anyhow!(
                "Cannot READ into variable '{}' of type {}!",
                self.identifier.name,
                variable_type
            ));
            return Err(errors); // Cannot continue if the type is incompatible
        }

        // Serialize the variable type
        let serialized_variable_type = variable_type.as_rust_type();

        // Transpile unit expression
        let unit_output = self.unit.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling unit expression in READ".to_string(),
            )
        })?;
        requested_variables.extend(unit_output.requested_variables.iter().cloned());

        // Runtime dispatch on unit value
        let serialization = format!(
            "{{ let __rosy_unit = ({}).round() as i64; \
            if __rosy_unit == 5 {{ \
                {dest} = rosy_lib::intrinsics::from_st::from_stdin::<{typ}>().context(\"Failed to READ into {name}\")?; \
            }} else {{ \
                let __rosy_read_str = rosy_lib::core::file_io::rosy_read_from_unit(__rosy_unit as u64)?; \
                {dest} = <{typ} as rosy_lib::intrinsics::from_st::RosyFromST>::rosy_from_st(__rosy_read_str)\
                    .context(\"Failed to parse value from file unit into {name}\")?; \
            }} }}",
            unit_output.as_value(),
            dest = serialized_variable_identifier,
            typ = serialized_variable_type,
            name = self.identifier.name,
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
