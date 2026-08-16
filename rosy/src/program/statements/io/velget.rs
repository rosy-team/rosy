//! # VELGET Statement
//!
//! Gets a component of a vector of reals (VE) and assigns it to a variable.
//!
//! ## Syntax
//!
//! ```text
//! VELGET vec_expr component_expr output_var;
//! ```
//!
//! - `vec_expr`       — VE expression (the vector)
//! - `component_expr` — RE expression (1-indexed component number)
//! - `output_var`     — variable that receives the RE result
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/io/velget.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
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

/// AST node for `VELGET vec_expr component_expr output_var;`.
#[derive(Debug)]
pub struct VelgetStatement {
    pub vec_expr: Expr,
    pub component_expr: Expr,
    pub output_var: VariableIdentifier,
}

impl FromRule for VelgetStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::velget,
            "Expected `velget` rule when building VELGET statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        let vec_pair = inner
            .next()
            .context("Missing vector expression in VELGET!")?;
        let vec_expr = Expr::from_rule(vec_pair)
            .context("Failed to build vector expression in VELGET")?
            .ok_or_else(|| anyhow::anyhow!("Expected vector expression in VELGET"))?;

        let component_pair = inner
            .next()
            .context("Missing component expression in VELGET!")?;
        let component_expr = Expr::from_rule(component_pair)
            .context("Failed to build component expression in VELGET")?
            .ok_or_else(|| anyhow::anyhow!("Expected component expression in VELGET"))?;

        let output_pair = inner.next().context("Missing output variable in VELGET!")?;
        let output_var = VariableIdentifier::from_rule(output_pair)
            .context("Failed to build output variable identifier in VELGET")?
            .ok_or_else(|| anyhow::anyhow!("Expected output variable identifier in VELGET"))?;

        Ok(Some(VelgetStatement {
            vec_expr,
            component_expr,
            output_var,
        }))
    }
}
impl TranspileableStatement for VelgetStatement {
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
impl Transpile for VelgetStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let vec_output = self.vec_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling vector expression in VELGET".to_string(),
            )
        })?;
        requested_variables.extend(vec_output.requested_variables.iter().cloned());

        let component_output = self.component_expr.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling component expression in VELGET".to_string(),
            )
        })?;
        requested_variables.extend(component_output.requested_variables.iter().cloned());

        let output_id_output = self.output_var.transpile(context).map_err(|e| {
            add_context_to_all(
                e,
                "...while transpiling output variable in VELGET".to_string(),
            )
        })?;
        requested_variables.extend(output_id_output.requested_variables.clone());

        // Determine whether the output variable needs pointer dereference
        let dereference = match context
            .variables
            .get(&self.output_var.name)
            .ok_or_else(|| {
                vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!",
                    self.output_var.name
                )]
            })?
            .scope
        {
            VariableScope::Local => "",
            VariableScope::Arg => "*",
            VariableScope::Higher => {
                requested_variables.insert(self.output_var.name.clone());
                "*"
            }
        };

        let serialization = format!(
            "{{\n    \
                let __rosy_velget_vec = {vec};\n    \
                let __rosy_velget_component = ({component}) as isize;\n    \
                if __rosy_velget_component < 1 || __rosy_velget_component as usize > __rosy_velget_vec.len() {{\n        \
                    bail!(\"VELGET: component index {{}} is out of bounds for vector of length {{}}\", __rosy_velget_component, __rosy_velget_vec.len());\n    \
                }}\n    \
                {deref}{dest} = __rosy_velget_vec[(__rosy_velget_component - 1) as usize];\n\
            }}",
            component = component_output.as_value(),
            deref = dereference,
            dest = output_id_output.serialization,
            vec = vec_output.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
