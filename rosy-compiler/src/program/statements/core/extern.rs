//! # EXTERN
//!
//! Calls a C or Rust function that the program supplies.
//!
//! ## Syntax
//!
//! ```text
//! EXTERN 'file.c'  'function' ['kinds'] a b c ...;
//! EXTERN 'file.rs' 'function' ['kinds'] a b c ...;
//! ```
//!
//! The file is compiled into the program (C) or mounted as a module (Rust);
//! its path is relative to the script. Each variable goes over as an array
//! of doubles (a real is an array of one), and whatever the function
//! changes is written back:
//!
//! ```c
//! void function(long long count, double **values, const long long *lengths);
//! ```
//!
//! ```rust,ignore
//! pub fn function(values: &mut [&mut [f64]]) { ... }
//! ```
//!
//! The optional third string gives each argument's kind, one letter each:
//! `r` (the default) as above, or `d` for an array of cells holding DA
//! values, which go over as blocks of a flag and the coefficients (see
//! `rosy_lib::extern_call`).
//!
//! `ROSY_EXTERN_CFLAGS` and `ROSY_EXTERN_LIBS`, read when the program is
//! built, add compiler flags and libraries. The argument conversion lives in
//! `rosy_lib`'s MPI build, so for now a program using EXTERN needs it.

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use super::variable_args::mutable_places;
use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::*,
};

#[derive(Debug)]
pub struct ExternStatement {
    /// The C or Rust file, relative to the script.
    pub file: String,
    /// The function in it.
    pub function: String,
    /// One letter per argument, `r` or `d`, lower case; empty means all `r`.
    pub kinds: String,
    pub args: Vec<VariableIdentifier>,
}

impl FromRule for ExternStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::extern_stmt,
            "Expected `extern_stmt` rule, found: {:?}",
            pair.as_rule()
        );
        let mut args = Vec::new();
        let mut strings = Vec::new();
        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::string => {
                    let raw = inner.as_str();
                    strings.push(raw[1..raw.len() - 1].to_string());
                }
                Rule::variable_identifier => args.push(
                    VariableIdentifier::from_rule(inner)
                        .context("Failed to build an EXTERN argument")?
                        .ok_or_else(|| anyhow!("Expected a variable"))?,
                ),
                _ => {}
            }
        }
        ensure!(
            strings.len() == 2 || strings.len() == 3,
            "EXTERN takes a C or Rust file and a function name, both quoted, optionally the argument kinds, then variables"
        );
        if let Some(kinds) = strings.get(2) {
            ensure!(
                kinds.chars().all(|c| matches!(c, 'r' | 'R' | 'd' | 'D')),
                "EXTERN argument kinds '{kinds}' may hold only r (reals) and d (DA cells)"
            );
            ensure!(
                kinds.len() <= args.len(),
                "EXTERN argument kinds '{kinds}' name {} arguments but {} are given",
                kinds.len(),
                args.len()
            );
        }
        let ident = |s: &str| {
            !s.is_empty()
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !s.starts_with(|c: char| c.is_ascii_digit())
        };
        ensure!(ident(&strings[1]), "'{}' is not a C function name", strings[1]);
        ensure!(
            !strings[0].contains("*/") && !strings[0].contains('"'),
            "'{}' is not a usable file name",
            strings[0]
        );
        Ok(Some(ExternStatement {
            kinds: strings.get(2).map_or(String::new(), |k| k.to_ascii_lowercase()),
            function: strings[1].clone(),
            file: strings[0].clone(),
            args,
        }))
    }
}

impl Transpile for ExternStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let cells: Vec<String> = mutable_places("EXTERN", &self.args, context, &mut requested_variables)?
            .into_iter()
            .map(|place| format!("{place} as &mut dyn PsCell"))
            .collect();
        // The CLI finds the file through the comment and compiles it in (C)
        // or mounts it as a module (Rust).
        let (file, function, kinds) = (&self.file, &self.function, &self.kinds);
        let serialization = match crate::compiler::embedded::extern_module_name(file) {
            Some(module) => format!(
                "{{ /* rosy-extern-source \"{file}\" */ rosy_lib::extern_call_rust(crate::{module}::{function}, \"{kinds}\", &mut [{}]).context(\"...in EXTERN {function}\")?; }}",
                cells.join(", "),
            ),
            None => format!(
                "{{ /* rosy-extern-source \"{file}\" */ unsafe extern \"C\" {{ fn {function}(count: i64, values: *mut *mut f64, lengths: *const i64); }} rosy_lib::extern_call({function}, \"{kinds}\", &mut [{}]).context(\"...in EXTERN {function}\")?; }}",
                cells.join(", "),
            ),
        };
        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for ExternStatement {
    /// The function decides what it writes, so EXTERN types nothing itself.
    fn wire_inference_edges(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        None
    }
}
