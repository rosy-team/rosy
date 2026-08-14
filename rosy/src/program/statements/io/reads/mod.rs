//! # READS Statement
//!
//! Reads a raw string from a unit (file or console) without type conversion.
//!
//! ## Syntax
//!
//! ```text
//! READS unit variable;
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
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, VariableScope,
        add_context_to_all,
    },
};

/// AST node for `READS unit variable;`.
#[derive(Debug)]
pub struct ReadsStatement {
    pub unit_expr: Expr,
    pub identifier: VariableIdentifier,
}

impl FromRule for ReadsStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::reads,
            "Expected `reads` rule when building READS statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let unit_pair = inner.next().context("Missing unit expression in READS!")?;
        let unit_expr = Expr::from_rule(unit_pair)
            .context("Failed to build unit expression in READS")?
            .ok_or_else(|| anyhow!("Expected unit expression in READS"))?;

        let identifier = VariableIdentifier::from_rule(
            inner
                .next()
                .context("Missing variable identifier in READS!")?,
        )
        .context("...while building variable identifier for READS statement")?
        .ok_or_else(|| anyhow!("Expected variable identifier for READS statement"))?;

        Ok(Some(ReadsStatement {
            unit_expr,
            identifier,
        }))
    }
}

impl TranspileableStatement for ReadsStatement {
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

impl Transpile for ReadsStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Serialize the unit expression
        let unit_serialization = match self.unit_expr.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables.iter().cloned());
                output.as_value()
            }
            Err(e) => {
                for err in add_context_to_all(
                    e,
                    "...while transpiling unit expression for READS".to_string(),
                ) {
                    errors.push(err);
                }
                String::new()
            }
        };

        // Serialize the identifier (assignment target)
        let serialized_variable_identifier = match self.identifier.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables);
                output.serialization
            }
            Err(vec_err) => {
                for err in vec_err {
                    errors.push(err.context(format!(
                        "...while transpiling identifier expression for READS into '{}'",
                        self.identifier.name
                    )));
                }
                String::new()
            }
        };

        if !errors.is_empty() {
            return Err(errors);
        }

        // Determine dereference prefix based on variable scope
        let dereference = match context
            .variables
            .get(&self.identifier.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.identifier.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.identifier.name.clone());
                "*"
            }
        };

        // Emit runtime dispatch: unit 5 = stdin, otherwise file unit
        let serialization = format!(
            "{deref}{dest} = {{\n\
                let __unit = {unit} as u64;\n\
                if __unit == 5 {{\n\
                    let mut __line = String::new();\n\
                    std::io::stdin().read_line(&mut __line).map_err(|e| anyhow::anyhow!(\"READS stdin error: {{}}\", e))?;\n\
                    __line.trim_end_matches('\\n').trim_end_matches('\\r').to_string()\n\
                }} else {{\n\
                    rosy_lib::core::file_io::rosy_reads_string_from_unit(__unit)?\n\
                }}\n\
            }};",
            deref = dereference,
            dest = serialized_variable_identifier,
            unit = unit_serialization,
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
