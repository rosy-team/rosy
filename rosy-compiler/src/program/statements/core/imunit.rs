//! # IMUNIT Statement
//!
//! Returns the imaginary unit *i* as a CM (Complex64) value.
//!
//! ## Syntax
//!
//! ```text
//! IMUNIT v;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/imunit.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver},
    transpile::*,
};

#[derive(Debug)]
pub struct ImunitStatement {
    pub identifier: VariableIdentifier,
}

impl FromRule for ImunitStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::imunit,
            "Expected `imunit` rule when building IMUNIT statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let expr_pair = inner
            .next()
            .context("Missing variable expression in IMUNIT!")?;
        let identifier = VariableIdentifier::from_rule(expr_pair)
            .context("Failed to build variable identifier in IMUNIT")?
            .ok_or_else(|| anyhow::anyhow!("Expected variable identifier in IMUNIT"))?;

        Ok(Some(ImunitStatement { identifier }))
    }
}

impl Transpile for ImunitStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let output = self.identifier.transpile(context).map_err(|e| {
            add_context_to_all(e, "...while transpiling identifier in IMUNIT".to_string())
        })?;
        requested_variables.extend(output.requested_variables.clone());

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

        let dest_ty = context
            .variables
            .get(&self.identifier.name)
            .map(|v| v.data.r#type)
            .unwrap_or_else(rosy_lib::RosyType::CM);
        let rhs = if dest_ty.is_any() {
            "RosyValue::from(num_complex::Complex64::new(0.0, 1.0))"
        } else {
            "num_complex::Complex64::new(0.0, 1.0)"
        };
        let serialization = format!("{}{} = {};", dereference, output.serialization, rhs);

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for ImunitStatement {
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        if !crate::syntax_config::is_cosy_syntax() {
            return Some(Ok(()));
        }
        let Some(slot) = ctx.variables.get(&self.identifier.name) else {
            return Some(Ok(()));
        };
        if let Some(node) = resolver.nodes.get_mut(slot) {
            let keep = node
                .resolved
                .map(|t| t == rosy_lib::RosyType::CM() || t.is_any())
                .unwrap_or(false);
            if !keep {
                node.resolved = Some(rosy_lib::RosyType::ANY());
            }
        }
        Some(Ok(()))
    }
}
