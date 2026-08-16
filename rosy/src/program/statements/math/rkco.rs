//! # RKCO Statement
//!
//! Sets the coefficient arrays used in the COSY eighth-order Runge-Kutta integrator.
//!
//! ## Syntax
//!
//! ```text
//! RKCO c b e a1 a2;
//! ```
//!
//! All five arguments are output VE (vector) variables that receive the Butcher-tableau
//! coefficients for the DOP853 integrator:
//! - `c`  — node coefficients (13 values)
//! - `b`  — 8th-order weights (13 values)
//! - `e`  — error-estimate weights (13 values)
//! - `a1` — coupling matrix rows 2..7, flattened (21 values)
//! - `a2` — coupling matrix rows 8..13, flattened (57 values)
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/rkco.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::core::variable_identifier::VariableIdentifier, statements::SourceLocation,
    },
    resolve::{ScopeContext, TypeResolver},
    transpile::{
        InferenceEdgeResult, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableStatement, TypeHydrationResult, TypeslotDeclarationResult, ValueKind,
        add_context_to_all,
    },
};

/// AST node for `RKCO c b e a1 a2;`.
#[derive(Debug)]
pub struct RkcoStatement {
    pub c_var: VariableIdentifier,
    pub b_var: VariableIdentifier,
    pub e_var: VariableIdentifier,
    pub a1_var: VariableIdentifier,
    pub a2_var: VariableIdentifier,
}

impl FromRule for RkcoStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::rkco,
            "Expected `rkco` rule when building RKCO statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let c_pair = inner.next().context("Missing c parameter in RKCO!")?;
        let c_var = VariableIdentifier::from_rule(c_pair)
            .context("Failed to build c variable identifier in RKCO")?
            .ok_or_else(|| anyhow::anyhow!("Expected c variable identifier in RKCO"))?;

        let b_pair = inner.next().context("Missing b parameter in RKCO!")?;
        let b_var = VariableIdentifier::from_rule(b_pair)
            .context("Failed to build b variable identifier in RKCO")?
            .ok_or_else(|| anyhow::anyhow!("Expected b variable identifier in RKCO"))?;

        let e_pair = inner.next().context("Missing e parameter in RKCO!")?;
        let e_var = VariableIdentifier::from_rule(e_pair)
            .context("Failed to build e variable identifier in RKCO")?
            .ok_or_else(|| anyhow::anyhow!("Expected e variable identifier in RKCO"))?;

        let a1_pair = inner.next().context("Missing a1 parameter in RKCO!")?;
        let a1_var = VariableIdentifier::from_rule(a1_pair)
            .context("Failed to build a1 variable identifier in RKCO")?
            .ok_or_else(|| anyhow::anyhow!("Expected a1 variable identifier in RKCO"))?;

        let a2_pair = inner.next().context("Missing a2 parameter in RKCO!")?;
        let a2_var = VariableIdentifier::from_rule(a2_pair)
            .context("Failed to build a2 variable identifier in RKCO")?
            .ok_or_else(|| anyhow::anyhow!("Expected a2 variable identifier in RKCO"))?;

        Ok(Some(RkcoStatement {
            c_var,
            b_var,
            e_var,
            a1_var,
            a2_var,
        }))
    }
}

impl TranspileableStatement for RkcoStatement {
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

impl Transpile for RkcoStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let c_output = self
            .c_var
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling c in RKCO".to_string()))?;
        requested_variables.extend(c_output.requested_variables.clone());

        let b_output = self
            .b_var
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling b in RKCO".to_string()))?;
        requested_variables.extend(b_output.requested_variables.clone());

        let e_output = self
            .e_var
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling e in RKCO".to_string()))?;
        requested_variables.extend(e_output.requested_variables.clone());

        let a1_output = self
            .a1_var
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling a1 in RKCO".to_string()))?;
        requested_variables.extend(a1_output.requested_variables.clone());

        let a2_output = self
            .a2_var
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling a2 in RKCO".to_string()))?;
        requested_variables.extend(a2_output.requested_variables.clone());

        fn make_lvalue(ser: &str, value_kind: ValueKind, rhs: &str) -> String {
            if value_kind == ValueKind::Owned {
                format!("{ser} = {rhs}")
            } else if let Some(bare) = ser.strip_prefix('&') {
                format!("{bare} = {rhs}")
            } else {
                format!("*{ser} = {rhs}")
            }
        }

        let c_assign = make_lvalue(&c_output.serialization, c_output.value_kind, "rosy_rkco_c");
        let b_assign = make_lvalue(&b_output.serialization, b_output.value_kind, "rosy_rkco_b");
        let e_assign = make_lvalue(&e_output.serialization, e_output.value_kind, "rosy_rkco_e");
        let a1_assign = make_lvalue(
            &a1_output.serialization,
            a1_output.value_kind,
            "rosy_rkco_a1",
        );
        let a2_assign = make_lvalue(
            &a2_output.serialization,
            a2_output.value_kind,
            "rosy_rkco_a2",
        );

        let serialization = format!(
            "{{ let (rosy_rkco_c, rosy_rkco_b, rosy_rkco_e, rosy_rkco_a1, rosy_rkco_a2) = rosy_lib::core::rkco::rosy_rkco()?; {c_assign}; {b_assign}; {e_assign}; {a1_assign}; {a2_assign}; }}"
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
