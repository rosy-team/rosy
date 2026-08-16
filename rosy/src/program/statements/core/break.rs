//! BREAK statement implementation.
//!
//! Syntax: `BREAK;`
//!
//! Exits the innermost enclosing WHILE or LOOP block immediately.
//! Only valid inside WHILE or LOOP contexts - not valid inside PLOOP,
//! and PROCEDURE/FUNCTION definitions create scope boundaries that
//! reset loop context.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/break.rosy"))]
//! ```

use anyhow::{Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{ast::*, transpile::*};

#[derive(Debug)]
pub struct BreakStatement;

impl FromRule for BreakStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::break_statement,
            "Expected `break_statement` rule when building break statement, found: {:?}",
            pair.as_rule()
        );

        Ok(Some(BreakStatement))
    }
}
impl Transpile for BreakStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        if !context.in_loop {
            return Err(vec![anyhow!(
                "BREAK can only be used inside a WHILE or LOOP block"
            )]);
        }

        Ok(TranspilationOutput {
            serialization: "break;".to_string(),
            requested_variables: BTreeSet::new(),
            ..Default::default()
        })
    }
}

impl TranspileableStatement for BreakStatement {}
