//! # COULSUM
//!
//! The Coulomb sum of a Taylor integrator for charged particles.
//!
//! ## Syntax
//!
//! ```text
//! COULSUM y mq i n wx wy wz want phi radii;
//! ```
//!
//! For particle `i` of `n`, adds up `n_k (r_i - r_k) / |r_i - r_k|^3` over
//! every other particle `k`, each coordinate a series in time, measuring
//! distance in the rest frame of the beam's velocity `W = (wx, wy, wz)`.
//! Particle `p` keeps its six series at `y(6(p-1)+1 ..= 6(p-1)+6)` and its
//! charge factor at `mq(2p)`; the three sums go to `phi(6(i-1)+4 ..= 6(i-1)+6)`.
//! With `want` nonzero, `radii` gets one convergence-radius estimate per
//! pair. The result is bit for bit what the same loop written with series
//! operators gives; see `rosy_lib::coulsum` and `docs/distributed-storage.md`.

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier,
        statements::{
            SourceLocation,
            core::variable_args::{infer_written, mutable_places},
        },
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::*,
};

/// One letter per argument: `i` read, `c` an array of untyped cells it
/// writes into, `v` a list it writes.
const KINDS: &str = "iiiiiiiicv";

#[derive(Debug)]
pub struct CoulsumStatement {
    pub args: Vec<VariableIdentifier>,
}

impl FromRule for CoulsumStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::coulsum,
            "Expected `coulsum` rule, found: {:?}",
            pair.as_rule()
        );
        let mut args = Vec::new();
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::variable_identifier {
                args.push(
                    VariableIdentifier::from_rule(inner)
                        .context("Failed to build a COULSUM argument")?
                        .ok_or_else(|| anyhow!("Expected a variable"))?,
                );
            }
        }
        ensure!(
            args.len() == KINDS.len(),
            "COULSUM takes {} variables, found {}",
            KINDS.len(),
            args.len()
        );
        Ok(Some(CoulsumStatement { args }))
    }
}

impl Transpile for CoulsumStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();
        let cells = mutable_places("COULSUM", &self.args, context, &mut requested_variables)?;
        // Reals are read first, so no borrow of them is alive in the call.
        let real = |k: usize| format!("rosy_lib::rosy_as_f64(&*({}))", cells[k]);
        let serialization = format!(
            "{{ let (__i, __n, __w, __want) = ({}, {}, [{}, {}, {}], {}); rosy_lib::coulsum(&*({}), &*({}), __i, __n, __w, __want, {}, {}).context(\"...in COULSUM\")?; }}",
            real(2), real(3), real(4), real(5), real(6), real(7), cells[0], cells[1], cells[8], cells[9],
        );
        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for CoulsumStatement {
    /// `radii`, if nothing else types it, is a list of reals.
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        Some(infer_written("COULSUM", KINDS, &self.args, resolver, ctx, source_location))
    }
}
