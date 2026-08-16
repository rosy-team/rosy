//! # Variable Identifier
//!
//! Parsed representation of a Rosy identifier with optional parenthesized
//! arguments and/or bracket indices.
//!
//! ## Syntax Forms
//!
//! ```text
//! NAME                    { plain identifier }
//! NAME(expr)              { single index or single-arg function }
//! NAME(expr)(expr)        { multi-dimensional indexing }
//! NAME(expr, expr, ...)   { multi-arg function call }
//! NAME[expr, expr]        { bracket indexing (vector element extraction) }
//! ```
//!
//! The distinction between indexing and function call is resolved later
//! by [`super::var_expr::VarExpr::classify`].
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/expressions/core/variable_identifier.rosy"))]
//! ```

use std::collections::BTreeSet;

use crate::ast::Rule;
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::{ast::FromRule, transpile::*};
use anyhow::{Context, Error, Result, ensure};
use rosy_lib::RosyType;
use std::collections::HashSet;

/// A parsed identifier with optional parenthesized arguments and bracket indices.
#[derive(Debug)]
pub struct VariableIdentifier {
    pub name: String,
    /// Each paren group `(expr, ...)` is a `Vec<Expr>`.
    /// `X(I)(J)` → two groups, each with one expr.
    /// `FUNC(a, b)` → one group with two exprs.
    pub paren_groups: Vec<Vec<Expr>>,
    /// Optional bracket indices `[expr, expr, ...]`
    pub bracket_indices: Vec<Expr>,
}

impl VariableIdentifier {
    /// Flatten paren_groups into a single index list for variable indexing.
    /// Only valid when each paren group has exactly one argument (multi-dim indexing).
    pub fn flat_indices(&self) -> Vec<&Expr> {
        let mut indices: Vec<&Expr> = Vec::new();
        for group in &self.paren_groups {
            for expr in group {
                indices.push(expr);
            }
        }
        for expr in &self.bracket_indices {
            indices.push(expr);
        }
        indices
    }

    /// Total number of indexing dimensions (only valid for variable indexing, not function calls).
    ///
    /// Counts every expression across all paren groups plus every bracket index.
    /// Supports both chained `X(I)(J)` (groups of one) and COSY-style `X(I, J)`
    /// (a single group of many).
    pub fn num_index_dimensions(&self) -> usize {
        self.paren_groups.iter().map(|g| g.len()).sum::<usize>() + self.bracket_indices.len()
    }
}

impl FromRule for VariableIdentifier {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<VariableIdentifier>> {
        ensure!(
            pair.as_rule() == Rule::variable_identifier,
            "Expected `variable_identifier` rule when building variable identifier, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let name = inner
            .next()
            .context("Missing variable name in variable identifier!")?
            .as_str()
            .to_string();

        let mut paren_groups = Vec::new();
        let mut bracket_indices = Vec::new();

        for token in inner {
            match token.as_rule() {
                Rule::paren_group => {
                    let mut group = Vec::new();
                    for expr_pair in token.into_inner() {
                        let expr = Expr::from_rule(expr_pair)
                            .context("Failed to build expression in paren group!")?
                            .ok_or_else(|| anyhow::anyhow!("Expected expression in paren group"))?;
                        group.push(expr);
                    }
                    paren_groups.push(group);
                }
                Rule::bracket_index => {
                    for expr_pair in token.into_inner() {
                        let expr = Expr::from_rule(expr_pair)
                            .context("Failed to build expression in bracket index!")?
                            .ok_or_else(|| {
                                anyhow::anyhow!("Expected expression in bracket index")
                            })?;
                        bracket_indices.push(expr);
                    }
                }
                _ => {}
            }
        }

        Ok(Some(VariableIdentifier {
            name,
            paren_groups,
            bracket_indices,
        }))
    }
}

impl TranspileableExpr for VariableIdentifier {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let var_data = context.variables.get(&self.name).ok_or(anyhow::anyhow!(
            "Variable '{}' is not defined in this scope!{}",
            self.name,
            context.variable_hint(&self.name)
        ))?;

        let num_indices = self.num_index_dimensions();
        let mut var_type = var_data.data.r#type;

        // Apply indices in cascade: each index peels one declared dimension
        // first, then if any indices remain and the base type is VE with
        // dimensions=0, one final index extracts the VE element to RE.
        // This supports both `M[i, j]` (multi-dim array indexing) and
        // `M[i][j]` (chained access on (VE n) where M[i] is VE then [j] is RE).
        let mut remaining = num_indices;
        let dim_peel = remaining.min(var_type.dimensions);
        var_type.dimensions -= dim_peel;
        remaining -= dim_peel;

        if remaining > 0 {
            if var_type.base_type == rosy_lib::RosyBaseType::VE
                && var_type.dimensions == 0
                && remaining == 1
            {
                return Ok(RosyType::RE());
            }
            return Err(anyhow::anyhow!(
                "Variable '{}' does not have enough dimensions to index into it (tried to index {} times, but it only has {} dimensions{})!",
                self.name,
                num_indices,
                var_data.data.r#type.dimensions,
                if var_data.data.r#type.base_type == rosy_lib::RosyBaseType::VE {
                    " (+1 implicit VE element)"
                } else {
                    ""
                }
            ));
        }

        Ok(var_type)
    }
    fn build_expr_recipe(
        &self,
        _resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        if let Some(slot) = ctx.variables.get(&self.name) {
            deps.insert(slot.clone());
            let num_indices = self.num_index_dimensions();
            if num_indices > 0 {
                ExprRecipe::IndexedVariable(slot.clone(), num_indices)
            } else {
                ExprRecipe::Variable(slot.clone())
            }
        } else {
            let name_upper = self.name.to_uppercase();
            let hint = ctx
                .variables
                .keys()
                .find(|c| c.to_uppercase() == name_upper && *c != &self.name)
                .map(|c| format!(" (did you mean '{}'? Rosy is case-sensitive)", c))
                .unwrap_or_default();
            ExprRecipe::Unknown(Some(format!("undeclared variable '{}'{}", self.name, hint)))
        }
    }
}

impl Transpile for VariableIdentifier {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Check that the variable exists and that the
        //  dimensions are correct
        let _ = self.type_of(context).map_err(|err| {
            vec![err.context(format!(
                "...while checking the type of variable {}",
                self.name
            ))]
        })?;

        // Serialize the indices
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();

        // Collect all transpiled index expressions first so we can
        // build bounds-checked access for each dimension.
        let flat = self.flat_indices();
        let mut transpiled_indices: Vec<String> = Vec::new();

        for (i, index_expr) in flat.iter().enumerate() {
            let i = i + 1;
            let name = &self.name;

            // Check that the type is RE
            let index_expr_type = index_expr.type_of(context).map_err(|err| {
                vec![err.context(format!(
                    "...while checking the type for index expression {i} of {name}"
                ))]
            })?;
            let expected_type = RosyType::RE();
            if index_expr_type != expected_type {
                return Err(vec![anyhow::anyhow!(
                    "Indexing expression {i} when indexing {name} was {index_expr_type}, when it should be {expected_type}!"
                )]);
            }

            // Transpile it
            match index_expr.transpile(context) {
                Ok(output) => {
                    transpiled_indices.push(output.as_value());
                    requested_variables.extend(output.requested_variables);
                }
                Err(vec_err) => {
                    for err in vec_err {
                        errors.push(err.context(format!(
                            "...while transpiling index expression to {}",
                            self.name
                        )));
                    }
                }
            }
        }

        // Finally, serialize the entire variable
        if VariableScope::Higher
            == context
                .variables
                .get(&self.name)
                .ok_or(vec![anyhow::anyhow!(
                    "Variable '{}' is not defined in this scope!{}",
                    self.name,
                    context.variable_hint(&self.name)
                )])?
                .scope
        {
            requested_variables.insert(self.name.clone());
        }

        // Build the serialization: either bare name (no indices) or
        // nested rosy_get() calls that return &T with 1-based bounds checking.
        let serialization = if transpiled_indices.is_empty() {
            self.name.clone()
        } else {
            let mut result = format!("&{}", self.name);
            for idx_expr in &transpiled_indices {
                result = format!(
                    "rosy_get({result}, {expr}, \"{name}\")",
                    result = result,
                    expr = idx_expr,
                    name = self.name,
                );
            }
            result
        };
        if errors.is_empty() {
            Ok(TranspilationOutput {
                serialization,
                requested_variables,
                ..Default::default()
            })
        } else {
            Err(errors)
        }
    }
}
