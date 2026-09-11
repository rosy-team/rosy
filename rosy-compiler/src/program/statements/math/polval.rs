//! # POLVAL Statement
//!
//! Evaluates / composes polynomials stored as DA vectors.
//!
//! ## Syntax
//!
//! ```text
//! POLVAL L P NP A NA R NR;
//! ```
//!
//! | Arg | Role                                    |
//! |-----|-----------------------------------------|
//! | L   | evaluation mode (1 = Horner, normally 1)|
//! | P   | array of NP DA polynomial vectors       |
//! | NP  | number of polynomials                   |
//! | A   | array of NA arguments (RE or DA)        |
//! | NA  | number of arguments                     |
//! | R   | result array (output variable)          |
//! | NR  | number of results                       |
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/polval.rosy"))]
//! ```

use anyhow::{ensure, Context, Error, Result};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ExprRecipe, ResolutionRule, ScopeContext, TypeResolver},
    transpile::{
        add_context_to_all, TranspilationInputContext, TranspilationOutput, Transpile,
        TranspileableExpr, TranspileableStatement,
    },
};
use rosy_lib::RosyBaseType;

/// AST node for `POLVAL L P NP A NA R NR;`.
#[derive(Debug)]
pub struct PolvalStatement {
    pub l_expr: Expr,
    pub p_expr: Expr,
    pub np_expr: Expr,
    pub a_expr: Expr,
    pub na_expr: Expr,
    pub r_expr: Expr,
    pub nr_expr: Expr,
}

impl FromRule for PolvalStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::polval,
            "Expected `polval` rule when building POLVAL statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        macro_rules! next_expr {
            ($name:literal) => {{
                let p =
                    inner
                        .next()
                        .context(concat!("Missing `", $name, "` parameter in POLVAL!"))?;
                Expr::from_rule(p)
                    .context(concat!(
                        "Failed to build `",
                        $name,
                        "` expression in POLVAL"
                    ))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(concat!("Expected `", $name, "` expression in POLVAL"))
                    })?
            }};
        }

        let l_expr = next_expr!("L");
        let p_expr = next_expr!("P");
        let np_expr = next_expr!("NP");
        let a_expr = next_expr!("A");
        let na_expr = next_expr!("NA");
        let r_expr = next_expr!("R");
        let nr_expr = next_expr!("NR");

        Ok(Some(PolvalStatement {
            l_expr,
            p_expr,
            np_expr,
            a_expr,
            na_expr,
            r_expr,
            nr_expr,
        }))
    }
}

impl Transpile for PolvalStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let l_out = self
            .l_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling L in POLVAL".to_string()))?;
        requested_variables.extend(l_out.requested_variables.iter().cloned());

        let p_out = self
            .p_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling P in POLVAL".to_string()))?;
        requested_variables.extend(p_out.requested_variables.iter().cloned());

        let np_out = self
            .np_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling NP in POLVAL".to_string()))?;
        requested_variables.extend(np_out.requested_variables.iter().cloned());

        let a_out = self
            .a_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling A in POLVAL".to_string()))?;
        requested_variables.extend(a_out.requested_variables.iter().cloned());

        let na_out = self
            .na_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling NA in POLVAL".to_string()))?;
        requested_variables.extend(na_out.requested_variables.iter().cloned());

        // R is the output variable — needs a mutable borrow
        let r_out = self
            .r_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling R in POLVAL".to_string()))?;
        requested_variables.extend(r_out.requested_variables.clone());

        let nr_out = self
            .nr_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling NR in POLVAL".to_string()))?;
        requested_variables.extend(nr_out.requested_variables.iter().cloned());

        // Build a mutable reference to the result variable.
        // as_ref() gives either `&expr` (Owned) or the Ref serialization which
        // already starts with `&`. Replace the leading `&` with `&mut `.
        let r_mut = r_out.as_ref().replacen('&', "&mut ", 1);

        // Dispatch based on A's element type:
        //   DA-array  → DA→DA polynomial composition (map-composition path)
        //   VE-array  → batch evaluation across many particles
        //   otherwise → plain RE evaluation
        let a_type = self
            .a_expr
            .type_of(context)
            .map_err(|e| vec![e.context("...while determining type of A in POLVAL")])?;
        let r_type = self
            .r_expr
            .type_of(context)
            .map_err(|e| vec![e.context("...while determining type of R in POLVAL")])?;
        let p_type = self
            .p_expr
            .type_of(context)
            .map_err(|e| vec![e.context("...while determining type of P in POLVAL")])?;

        let polval_fn = if a_type.is_any() || r_type.is_any() || p_type.is_any() {
            "rosy_polval_any"
        } else if a_type.base_type == RosyBaseType::DA && a_type.dimensions > 0 {
            "rosy_polval_da"
        } else if a_type.base_type == RosyBaseType::VE && a_type.dimensions > 0 {
            "rosy_polval_ve"
        } else {
            "rosy_polval_re"
        };

        // COSY's in-place POLVAL allows P, A, and R to alias each other. Rust's
        // borrow checker rejects two `&mut` to the same memory or a `&` + `&mut`
        // overlap, so we clone any input arg whose underlying name matches R's.
        let a_ref = a_out.as_ref();
        let p_ref = p_out.as_ref();
        let r_strip = r_mut.trim_start_matches("&mut ");
        let clone_ref = |r: &str| {
            let inner = r.trim_start_matches('&');
            if inner.starts_with('*') {
                format!("&({}).clone()", inner)
            } else {
                format!("&{}.clone()", inner)
            }
        };
        let a_arg = if a_ref.trim_start_matches('&') == r_strip {
            clone_ref(&a_ref)
        } else {
            a_ref
        };
        let p_arg = if p_ref.trim_start_matches('&') == r_strip {
            clone_ref(&p_ref)
        } else {
            p_ref
        };

        let serialization = format!(
            "rosy_lib::core::polval::{}({}, {}, rosy_as_usize(&({})), {}, rosy_as_usize(&({})), {}, rosy_as_usize(&({})))?;",
            polval_fn,
            l_out.as_value(),
            p_arg,
            np_out.as_value(),
            a_arg,
            na_out.as_value(),
            r_mut,
            nr_out.as_value(),
        );

        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}

impl TranspileableStatement for PolvalStatement {
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        let r_name = self.r_expr.as_bare_variable_name()?;
        let a_name = self.a_expr.as_bare_variable_name()?;
        let r_slot = ctx.variables.get(r_name)?.clone();
        let a_slot = ctx.variables.get(a_name)?.clone();
        if r_slot == a_slot {
            return Some(Ok(()));
        }
        let a_ty = resolver.nodes.get(&a_slot).and_then(|n| n.resolved);
        let Some(node) = resolver.nodes.get_mut(&r_slot) else {
            return Some(Ok(()));
        };
        if !matches!(node.rule, ResolutionRule::Unresolved) {
            return Some(Ok(()));
        }
        if let Some(a_ty) = a_ty {
            node.rule = ResolutionRule::InferredFrom {
                recipe: ExprRecipe::Literal(a_ty),
                reason: "POLVAL result matches argument array".to_string(),
            };
            node.resolved = Some(a_ty);
            node.assigned_at = Some(source_location);
        } else {
            node.rule = ResolutionRule::Mirror {
                source: a_slot.clone(),
                reason: "POLVAL result matches argument array".to_string(),
            };
            node.depends_on.insert(a_slot);
            node.assigned_at = Some(source_location);
        }
        Some(Ok(()))
    }
}
