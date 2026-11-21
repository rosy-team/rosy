//! # Statements whose arguments are variables it reads and writes
//!
//! Shared by the particle store's statements ([`super::pstore`]), `EXTERN`
//! ([`super::extern`]) and `COULSUM` ([`crate::program::statements::math::coulsum`]):
//! each argument is a variable, handed over as a mutable place, and a
//! variable that only such a statement writes takes its type from it.

use anyhow::{Error, Result, anyhow};
use std::collections::BTreeSet;

use crate::{
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ExprRecipe, ResolutionRule, ScopeContext, TypeResolver},
    transpile::*,
};
use rosy_lib::RosyType;

/// Each argument as a `&mut` place in the generated code, in order. The
/// variables a place needs from an enclosing scope go into `requested`.
pub fn mutable_places(
    statement: &str,
    args: &[VariableIdentifier],
    context: &mut TranspilationInputContext,
    requested: &mut BTreeSet<String>,
) -> Result<Vec<String>, Vec<Error>> {
    let mut places = Vec::with_capacity(args.len());
    for arg in args {
        let output = arg.transpile(context).map_err(|e| {
            add_context_to_all(e, format!("...while transpiling an argument of {statement}"))
        })?;
        requested.extend(output.requested_variables.clone());
        let scope = &context
            .variables
            .get(&arg.name)
            .ok_or_else(|| vec![anyhow!("Variable '{}' is not defined in this scope!", arg.name)])?
            .scope;
        places.push(match scope {
            VariableScope::Local => format!("&mut {}", output.serialization),
            VariableScope::Arg => format!("&mut *{}", output.serialization),
            VariableScope::Higher => {
                requested.insert(arg.name.clone());
                format!("&mut *{}", output.serialization)
            }
        });
    }
    Ok(places)
}

/// Types the variables a statement only writes. `kinds` has one letter per
/// argument: `v` is a list it writes, `s` a real it writes; any other letter
/// is left alone. `VARIABLE X 1 n` becomes an array of reals, a bare cell a
/// list.
pub fn infer_written(
    statement: &str,
    kinds: &str,
    args: &[VariableIdentifier],
    resolver: &mut TypeResolver,
    ctx: &mut ScopeContext,
    source_location: SourceLocation,
) -> Result<()> {
    for (arg, kind) in args.iter().zip(kinds.bytes()) {
        if !matches!(kind, b'v' | b's') {
            continue;
        }
        let Some(slot) = ctx.variables.get(&arg.name).cloned() else {
            continue;
        };
        let Some(node) = resolver.nodes.get_mut(&slot) else {
            continue;
        };
        if matches!(node.rule, ResolutionRule::Unresolved) {
            let ty = if kind == b's' {
                RosyType::RE()
            } else if node.extra_dims > 0 {
                RosyType::new(rosy_lib::RosyBaseType::RE, node.extra_dims)
            } else {
                RosyType::VE()
            };
            node.rule = ResolutionRule::InferredFrom {
                recipe: ExprRecipe::Literal(ty),
                reason: format!("{statement} writes it"),
            };
            node.assigned_at = Some(source_location.clone());
        }
    }
    Ok(())
}
