//! # OS Statement
//!
//! Triggers a system call, executing a shell command.
//!
//! ## Syntax
//!
//! ```text
//! OS expr;
//! ```
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/os_call.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr,
        TranspileableStatement, add_context_to_all,
    },
};

/// AST node for `OS expr;`.
#[derive(Debug)]
pub struct OsCallStatement {
    pub cmd_expr: Expr,
}

impl FromRule for OsCallStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::os_call,
            "Expected `os_call` rule when building OS statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let cmd_pair = inner.next().context("Missing command expression in OS!")?;
        let cmd_expr = Expr::from_rule(cmd_pair)
            .context("Failed to build command expression in OS")?
            .ok_or_else(|| anyhow::anyhow!("Expected command expression in OS"))?;

        Ok(Some(OsCallStatement { cmd_expr }))
    }
}
impl Transpile for OsCallStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let cmd_output = self.cmd_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling command expression in OS".to_string(),
            )
        })?;
        requested_variables.extend(cmd_output.requested_variables.iter().cloned());

        let cmd_ty = self
            .cmd_expr
            .type_of(context)
            .map_err(|e| vec![e.context("...while typing OS command")])?;
        // ANY holds the command string after `outputpath & 'file' & SI(I)`.
        let cmd_string = if cmd_ty.is_any() {
            format!("RosyST::rosy_to_string(&{})", cmd_output.as_value())
        } else {
            format!("({}).to_string()", cmd_output.as_ref())
        };

        let serialization = format!(
            "{{\n    let __os_cmd: String = {cmd_string};\n    if cfg!(windows) {{\n        std::process::Command::new(\"cmd\").args(&[\"/C\", &__os_cmd]).status().ok();\n    }} else {{\n        std::process::Command::new(\"sh\").args(&[\"-c\", &__os_cmd]).status().ok();\n    }}\n}}"
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for OsCallStatement {}
