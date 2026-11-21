//! # Particle-store intrinsics (`PS*`)
//!
//! Distributed particle storage for MPI runs. Each rank owns a contiguous
//! range of Morton keys; these intrinsics move data between that store and
//! ordinary local arrays, so loop bodies index them as usual.
//!
//! ## Syntax
//!
//! ```text
//! PSLOAD  x y z px py pz f n e count;
//! PSBUILD q wx wy wz relg;
//! PSGET   x y z px py pz f n e ids nown;
//! PSLISTS snumbs start_snumbs nnumbs start_nnumbs cx cy cz nleaves;
//! PSPUT   x y z px py pz e;
//! PSSCALED sx sy sz;
//! PSSIZES nmult nloc depth;
//! PSLEAFLOC index;
//! PSUP    level list count;
//! PSDOWN  level list count;
//! PSMSYNC cells level;
//! PSLSYNC cells level;
//! PSHALO;
//! PSCOST leaf seconds;
//! PSBOUNDS xmin ymin zmin dzero;
//! PSPREV  ids start;
//! PSPOS   ids positions;
//! PSSUM   values count total;
//! PSMAX   values count max;
//! PSALL   x y z px py pz f n e count;
//! PSSLICE x y z px py pz f n e first count;
//! PSWRITE unit lines count;
//! PSREMOVE keep info k removed nremoved ncool;
//! ```
//!
//! `EXTERN` and `COULSUM`, which take their arguments the same way, have
//! their own statements: [`super::extern`] and
//! [`crate::program::statements::math::coulsum`].
//!
//! Every argument is a variable. Every rank must make the same call.
//! See `rosy_lib::pstore` for what each one does.

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use super::variable_args::{infer_written, mutable_places};
use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::*,
};

/// Each intrinsic and what its arguments are: `i` is read, `v` is a list
/// it writes, `s` is a real it writes, `c` is an array of untyped cells.
const INTRINSICS: &[(&str, &str)] = &[
    ("PSLOAD", "iiiiiiiiii"),
    ("PSBUILD", "iiiii"),
    ("PSGET", "vvvvvvvvvvs"),
    ("PSLISTS", "vvvvvvvs"),
    ("PSPUT", "iiiiiii"),
    ("PSSCALED", "vvv"),
    ("PSSIZES", "sss"),
    ("PSLEAFLOC", "v"),
    ("PSUP", "ivs"),
    ("PSDOWN", "ivs"),
    ("PSMSYNC", "ci"),
    ("PSLSYNC", "ci"),
    ("PSHALO", ""),
    ("PSCOST", "ii"),
    ("PSBOUNDS", "ssss"),
    ("PSPREV", "vv"),
    ("PSPOS", "iv"),
    ("PSSUM", "iis"),
    ("PSMAX", "iis"),
    ("PSALL", "vvvvvvvvvs"),
    ("PSSLICE", "vvvvvvvvvss"),
    ("PSWRITE", "iii"),
    ("PSREMOVE", "iiivss"),
];

/// Why an unknown procedure name must not be compiled as a do-nothing call
/// (COSY syntax turns unknown procedures into no-ops so that missing COSY
/// graphics routines don't stop a program): it is the old name of EXTERN,
/// it uses the store's `PS` prefix, or it is a near miss of a store
/// statement, EXTERN or COULSUM. Dropping such a call silently would skip
/// real work (a misspelled PSREMOVE would never remove a particle).
pub fn unknown_statement_error(name: &str) -> Option<String> {
    let upper = name.to_ascii_uppercase();
    if upper == "KERNEL" {
        return Some("KERNEL was renamed to EXTERN: write EXTERN 'file' 'function' ...".to_string());
    }
    let names = INTRINSICS.iter().map(|(n, _)| *n).chain(["EXTERN", "COULSUM"]);
    let closest = names
        .map(|n| (edit_distance(&upper, n), n))
        .min()
        .filter(|(d, _)| *d <= 2 && upper.len() >= 4);
    if upper.starts_with("PS") && upper.len() > 2 || closest.is_some() {
        let hint = closest.map(|(_, n)| format!(" (did you mean {n}?)")).unwrap_or_default();
        return Some(format!(
            "'{name}' is not a Rosy statement{hint}; unknown names this close to the store's statements are errors, not do-nothing calls"
        ));
    }
    None
}

/// Levenshtein distance between two ASCII words.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, &cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod unknown_name_tests {
    use super::unknown_statement_error;

    #[test]
    fn near_misses_of_store_statements_are_errors() {
        assert!(unknown_statement_error("KERNEL").unwrap().contains("renamed to EXTERN"));
        assert!(unknown_statement_error("PSREMOV").unwrap().contains("did you mean PSREMOVE?"));
        assert!(unknown_statement_error("psremove_all").is_some(), "store prefix");
        assert!(unknown_statement_error("EXTRN").unwrap().contains("did you mean EXTERN?"));
        assert!(unknown_statement_error("COLSUM").unwrap().contains("did you mean COULSUM?"));
        // COSY routines Rosy leaves out on purpose stay do-nothing calls.
        for name in ["GRMOVE", "GRDRAW", "GRCHAR", "CLEAR", "GRWDTH", "GRPROJ", "MQ"] {
            assert_eq!(unknown_statement_error(name), None, "{name}");
        }
    }
}

#[derive(Debug)]
pub struct PstoreStatement {
    pub name: String,
    pub args: Vec<VariableIdentifier>,
}

impl FromRule for PstoreStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::pstore,
            "Expected `pstore` rule, found: {:?}",
            pair.as_rule()
        );
        let mut name = String::new();
        let mut args = Vec::new();
        let mut strings = 0usize;
        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::string => strings += 1,
                Rule::pstore_name => name = inner.as_str().to_ascii_uppercase(),
                Rule::variable_identifier => args.push(
                    VariableIdentifier::from_rule(inner)
                        .context("Failed to build a particle-store argument")?
                        .ok_or_else(|| anyhow!("Expected a variable"))?,
                ),
                _ => {}
            }
        }
        ensure!(strings == 0, "{name} takes variables only");
        let expected = INTRINSICS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, kinds)| kinds.len())
            .ok_or_else(|| anyhow!("Unknown particle-store intrinsic {name}"))?;
        ensure!(
            args.len() == expected,
            "{name} takes {expected} variables, found {}",
            args.len()
        );
        Ok(Some(PstoreStatement { name, args }))
    }
}

impl Transpile for PstoreStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::from(["rosy_mpi_context".to_string()]);
        let cells: Vec<String> = mutable_places(&self.name, &self.args, context, &mut requested_variables)?
            .into_iter()
            .map(|place| format!("{place} as &mut dyn PsCell"))
            .collect();
        let serialization = format!(
            "rosy_mpi_context.ps_call(\"{}\", &mut [{}]).context(\"...in {}\")?;",
            self.name,
            cells.join(", "),
            self.name,
        );
        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for PstoreStatement {
    /// A variable that only an intrinsic writes takes its type from here.
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        let kinds = INTRINSICS.iter().find(|(n, _)| *n == self.name)?.1;
        Some(infer_written(&self.name, kinds, &self.args, resolver, ctx, source_location))
    }
}
