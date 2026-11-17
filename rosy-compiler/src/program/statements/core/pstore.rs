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
//! PSBUILD q zscale;
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
//! PSALL   x y z px py pz f n e count;
//! PSSLICE x y z px py pz f n e first count;
//! PSWRITE unit lines count;
//! KERNEL  'file.c' 'function' ['kinds'] a b c ...;
//! ```
//!
//! `KERNEL` calls a C function the program supplies. The file is compiled
//! and linked into the program; its path is relative to the script. Each
//! variable goes over as an array of doubles (a real is an array of one),
//! and whatever the function changes is written back:
//!
//! ```c
//! void function(long long count, double **values, const long long *lengths);
//! ```
//!
//! An optional third string gives each argument's kind, one letter each:
//! `r` (the default) as above, or `d` for an array of cells holding DA
//! values, which go over as blocks of a flag and the coefficients (see
//! `rosy_lib::kernel_call`).
//!
//! Every argument is a variable. Every rank must make the same call.
//! See `rosy_lib::pstore` for what each one does.

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ExprRecipe, ResolutionRule, ScopeContext, TypeResolver},
    transpile::*,
};
use rosy_lib::RosyType;

/// Each intrinsic and what its arguments are: `i` is read, `v` is a list
/// it writes, `s` is a real it writes, `c` is an array of untyped cells.
const INTRINSICS: &[(&str, &str)] = &[
    ("PSLOAD", "iiiiiiiiii"),
    ("PSBUILD", "ii"),
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
    ("PSALL", "vvvvvvvvvs"),
    ("PSSLICE", "vvvvvvvvvss"),
    ("PSWRITE", "iii"),
    // Not a store call: the Coulomb sum over Taylor series, in rosy-lib.
    ("COULSUM", "iiiiiicv"),
];

#[derive(Debug)]
pub struct PstoreStatement {
    pub name: String,
    pub args: Vec<VariableIdentifier>,
    /// `KERNEL` only: the C file and the function in it.
    pub strings: Vec<String>,
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
        let mut strings = Vec::new();
        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::string => {
                    let raw = inner.as_str();
                    strings.push(raw[1..raw.len() - 1].to_string());
                }
                Rule::pstore_name => name = inner.as_str().to_ascii_uppercase(),
                Rule::variable_identifier => args.push(
                    VariableIdentifier::from_rule(inner)
                        .context("Failed to build a particle-store argument")?
                        .ok_or_else(|| anyhow!("Expected a variable"))?,
                ),
                _ => {}
            }
        }
        if name == "KERNEL" {
            ensure!(
                strings.len() == 2 || strings.len() == 3,
                "KERNEL takes a C file and a function name, both quoted, optionally the argument kinds, then variables"
            );
            if let Some(kinds) = strings.get(2) {
                ensure!(
                    kinds.chars().all(|c| matches!(c, 'r' | 'R' | 'd' | 'D')),
                    "KERNEL argument kinds '{kinds}' may hold only r (reals) and d (DA cells)"
                );
                ensure!(
                    kinds.len() <= args.len(),
                    "KERNEL argument kinds '{kinds}' name {} arguments but {} are given",
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
            return Ok(Some(PstoreStatement { name, args, strings }));
        }
        ensure!(strings.is_empty(), "{name} takes variables only");
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
        Ok(Some(PstoreStatement { name, args, strings }))
    }
}

impl Transpile for PstoreStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let coulsum = self.name == "COULSUM";
        if !coulsum && self.name != "KERNEL" {
            requested_variables.insert("rosy_mpi_context".to_string());
        }
        let mut cells = Vec::new();
        for arg in &self.args {
            let output = arg.transpile(context).map_err(|e| {
                add_context_to_all(e, format!("...while transpiling an argument of {}", self.name))
            })?;
            requested_variables.extend(output.requested_variables.clone());
            let scope = &context
                .variables
                .get(&arg.name)
                .ok_or_else(|| vec![anyhow!("Variable '{}' is not defined in this scope!", arg.name)])?
                .scope;
            let place = match scope {
                VariableScope::Local => format!("&mut {}", output.serialization),
                VariableScope::Arg => format!("&mut *{}", output.serialization),
                VariableScope::Higher => {
                    requested_variables.insert(arg.name.clone());
                    format!("&mut *{}", output.serialization)
                }
            };
            if coulsum {
                cells.push(place);
            } else {
                cells.push(format!("{place} as &mut dyn PsCell"));
            }
        }
        if coulsum {
            // Reals are read first, so no borrow of them is alive in the call.
            let real = |k: usize| format!("rosy_lib::rosy_as_f64(&*({}))", cells[k]);
            let serialization = format!(
                "{{ let (__i, __n, __relg, __want) = ({}, {}, {}, {}); rosy_lib::coulsum(&*({}), &*({}), __i, __n, __relg, __want, {}, {}).context(\"...in COULSUM\")?; }}",
                real(2), real(3), real(4), real(5), cells[0], cells[1], cells[6], cells[7],
            );
            return Ok(TranspilationOutput {
                serialization,
                requested_variables,
                ..Default::default()
            });
        }
        if self.name == "KERNEL" {
            // The CLI finds the file through the comment and compiles it in.
            let (file, function) = (&self.strings[0], &self.strings[1]);
            let kinds = self.strings.get(2).map_or(String::new(), |k| k.to_ascii_lowercase());
            let serialization = format!(
                "{{ /* rosy-kernel-source \"{file}\" */ unsafe extern \"C\" {{ fn {function}(count: i64, values: *mut *mut f64, lengths: *const i64); }} rosy_lib::kernel_call({function}, \"{kinds}\", &mut [{}]).context(\"...in KERNEL {function}\")?; }}",
                cells.join(", "),
            );
            return Ok(TranspilationOutput {
                serialization,
                requested_variables,
                ..Default::default()
            });
        }
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
        let kinds = INTRINSICS.iter().find(|(n, _)| *n == self.name)?.1.as_bytes();
        for (i, arg) in self.args.iter().enumerate() {
            if !matches!(kinds[i], b'v' | b's') {
                continue;
            }
            let Some(slot) = ctx.variables.get(&arg.name).cloned() else {
                continue;
            };
            let Some(node) = resolver.nodes.get_mut(&slot) else {
                continue;
            };
            if matches!(node.rule, ResolutionRule::Unresolved) {
                // `VARIABLE X 1 n` is an array of reals; a bare cell is a list.
                let ty = if kinds[i] == b's' {
                    RosyType::RE()
                } else if node.extra_dims > 0 {
                    RosyType::new(rosy_lib::RosyBaseType::RE, node.extra_dims)
                } else {
                    RosyType::VE()
                };
                node.rule = ResolutionRule::InferredFrom {
                    recipe: ExprRecipe::Literal(ty),
                    reason: format!("{} writes it", self.name),
                };
                node.assigned_at = Some(source_location.clone());
            }
        }
        Some(Ok(()))
    }
}
