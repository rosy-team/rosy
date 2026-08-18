//! # CPOLVAL Statement
//!
//! Complex-DA polynomial evaluation / composition. CPOLVAL is the
//! complex-coefficient analogue of POLVAL — instead of substituting f64
//! arguments into f64-coefficient DA polynomials, both polynomial and
//! arguments live in the CD (complex-DA) algebra.
//!
//! ## Syntax
//!
//! ```text
//! CPOLVAL L P NP A NA R NR;
//! ```
//!
//! | Arg | Role                                    |
//! |-----|-----------------------------------------|
//! | L   | evaluation mode (1 = Horner, normally 1)|
//! | P   | array of NP CD polynomial vectors       |
//! | NP  | number of polynomials                   |
//! | A   | array of NA CD arguments                |
//! | NA  | number of arguments                     |
//! | R   | result CD-array (output)                |
//! | NR  | number of results                       |
//!
//! Used by the normal-form pipeline (DANF / NF / TS / TP) where
//! resonance / tune / Twiss extraction needs to substitute CD basis
//! variables into CD polynomials, and by COCR / CRCO for the
//! COSY-to-circular-representation transform.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/math/cpolval.rosy"))]
//! ```

use anyhow::{Context, Error, Result, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::expressions::Expr,
    transpile::{
        TranspilationInputContext, TranspilationOutput, Transpile, TranspileableStatement,
        add_context_to_all,
    },
};

/// AST node for `CPOLVAL L P NP A NA R NR;`.
#[derive(Debug)]
pub struct CpolvalStatement {
    pub l_expr: Expr,
    pub p_expr: Expr,
    pub np_expr: Expr,
    pub a_expr: Expr,
    pub na_expr: Expr,
    pub r_expr: Expr,
    pub nr_expr: Expr,
}

impl FromRule for CpolvalStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::cpolval,
            "Expected `cpolval` rule when building CPOLVAL statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        macro_rules! next_expr {
            ($name:literal) => {{
                let p =
                    inner
                        .next()
                        .context(concat!("Missing `", $name, "` parameter in CPOLVAL!"))?;
                Expr::from_rule(p)
                    .context(concat!(
                        "Failed to build `",
                        $name,
                        "` expression in CPOLVAL"
                    ))?
                    .ok_or_else(|| {
                        anyhow::anyhow!(concat!("Expected `", $name, "` expression in CPOLVAL"))
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

        Ok(Some(CpolvalStatement {
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

impl Transpile for CpolvalStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let mut requested_variables = BTreeSet::new();

        let l_out = self
            .l_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling L in CPOLVAL".to_string()))?;
        requested_variables.extend(l_out.requested_variables.iter().cloned());

        let p_out = self
            .p_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling P in CPOLVAL".to_string()))?;
        requested_variables.extend(p_out.requested_variables.iter().cloned());

        let np_out = self
            .np_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling NP in CPOLVAL".to_string()))?;
        requested_variables.extend(np_out.requested_variables.iter().cloned());

        let a_out = self
            .a_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling A in CPOLVAL".to_string()))?;
        requested_variables.extend(a_out.requested_variables.iter().cloned());

        let na_out = self
            .na_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling NA in CPOLVAL".to_string()))?;
        requested_variables.extend(na_out.requested_variables.iter().cloned());

        // R is the output variable — needs a mutable borrow
        let r_out = self
            .r_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling R in CPOLVAL".to_string()))?;
        requested_variables.extend(r_out.requested_variables.clone());

        let nr_out = self
            .nr_expr
            .transpile(context)
            .map_err(|e| add_context_to_all(e, "...while transpiling NR in CPOLVAL".to_string()))?;
        requested_variables.extend(nr_out.requested_variables.iter().cloned());

        // Build a mutable reference to the result variable.
        let r_mut = r_out.as_ref().replacen('&', "&mut ", 1);

        // CPOLVAL only supports CD-typed inputs. Unlike POLVAL (which dispatches
        // by A's base type), there's no need for routing — the generated call
        // always lands in `rosy_polval_cd`. Any non-CD args trip up the runtime
        // signature mismatch, surfacing as a clear compile error in the
        // transpiled Rust.

        // Same alias handling as POLVAL: clone any input arg whose underlying
        // name matches R's, since two `&mut` (or `&` + `&mut`) to the same
        // memory is rejected by Rust's borrow checker.
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
            "rosy_lib::core::polval::rosy_polval_cd({}, {}, rosy_as_usize(&({})), {}, rosy_as_usize(&({})), {}, rosy_as_usize(&({})))?;",
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

impl TranspileableStatement for CpolvalStatement {}
