//! # Differential Algebra (DA) Statements
//!
//! Statements for initializing and working with Taylor series (DA/CD) values.
//!
//! ## Initialization & Configuration
//!
//! - **[`da_init`]** — `OV order nvars;` — initialize the DA environment
//! - **[`daeps`]** — `DAEPS eps;` — set DA epsilon
//! - **[`danot`]** — `DANOT order;` — set DA notation order
//! - **[`datrn`]** — `DATRN var;` — DA truncation
//!
//! ## Printing & I/O
//!
//! - **[`daprv`]** — `DAPRV ...;` — print DA values
//! - **[`darev`]** — `DAREV ...;` — reverse-print DA values
//! - **[`darea`]** — `DAREA unit da_var num_vars;` — read a DA vector from file
//! - **[`dapew`]** — `DAPEW unit da_var var_i order_n;` — print order-n part in variable xᵢ
//!
//! ## Coefficient Access
//!
//! - **[`dapee`]** — `DAPEE da_var id result;` — get coefficient by TRANSPORT notation id
//! - **[`dapea`]** — `DAPEA da_var exps_array size result;` — get coefficient by exponent array
//! - **[`dapep`]** — `DAPEP da_var id m result;` — get parameter-dependent component
//! - **[`dacliw`]** — `DACLIW da n linear;` — extract linear (first-order) coefficients
//! - **[`dacqlc`]** — `DACQLC da n hessian linear constant;` — extract quadratic Lie coefficients
//!
//! ## In-Place Arithmetic
//!
//! - **[`dascl`]** — `DASCL da_var scalar;` — scale all coefficients by a factor
//! - **[`dasgn`]** — `DASGN da_var;` — negate all coefficients
//! - **[`dader`]** — `DADER da_var var_index;` — partial derivative w.r.t. variable
//! - **[`daint`]** — `DAINT da_var var_index;` — integration w.r.t. variable
//!
//! ## Filtering & Term Removal
//!
//! - **[`danoro`]** — `DANORO da_var;` — remove odd-order terms
//! - **[`danors`]** — `DANORS da_var threshold;` — remove coefficients below threshold
//!
//! ## Substitution & Algebra
//!
//! - **[`daplu`]** — `DAPLU da_in i C result;` — plug (replace variable xᵢ with constant C)
//! - **[`dadiu`]** — `DADIU i da_in result;` — divide by independent variable xᵢ
//! - **[`dadmu`]** — `DADMU i j da_in result;` — divide by xᵢ then multiply by xⱼ
//!
//! ## Analysis
//!
//! - **[`daest`]** — `DAEST da_var i j result;` — estimate size of j-th order terms
//!
//! ## Evaluation
//!
//! - **[`mtree`]** — `MTREE ...;` — tree representation for fast DA evaluation

pub mod cdf2;
pub mod cdflo;
pub mod cdnf;
pub mod cdnfda;
pub mod cdnfds;
pub mod da_init;
pub mod dacliw;
pub mod dacode;
pub mod dacqlc;
pub mod dader;
pub mod dadiu;
pub mod dadmu;
pub mod daeps;
pub mod daepsm;
pub mod daest;
pub mod dafilt;
pub mod daflo;
pub mod dafset;
pub mod dagmd;
pub mod daint;
pub mod danoro;
pub mod danors;
pub mod danot;
pub mod danotw;
pub mod danow;
pub mod dapea;
pub mod dapee;
pub mod dapep;
pub mod dapew;
pub mod daplu;
pub mod daprv;
pub mod daran;
pub mod darea;
pub mod darev;
pub mod dascl;
pub mod dasgn;
pub mod datrn;
pub mod epsmin;
pub mod mtree;

use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ResolutionRule, ScopeContext, TypeResolver};
use crate::syntax_config;
use crate::transpile::TranspileableExpr;
use rosy_lib::RosyType;

/// Fox `VARIABLE X mem` is otherwise RE; DA/CD statement dests must be cells.
pub(crate) fn wire_da_result_cell(
    dest: &Expr,
    resolver: &mut TypeResolver,
    ctx: &mut ScopeContext,
    stmt: &str,
) {
    let Some(name) = dest.as_bare_variable_name() else {
        return;
    };
    let Some(slot) = ctx.variables.get(name).cloned() else {
        return;
    };
    if let Some(node) = resolver.nodes.get_mut(&slot) {
        if syntax_config::is_cosy_syntax() {
            node.rule = ResolutionRule::InferredFrom {
                recipe: ExprRecipe::Literal(RosyType::ANY()),
                reason: format!("{stmt} dest (COSY cell)"),
            };
            node.resolved = Some(RosyType::ANY());
            node.depends_on.clear();
        } else if node.resolved.is_none() {
            node.rule = ResolutionRule::InferredFrom {
                recipe: ExprRecipe::Literal(RosyType::DA()),
                reason: format!("{stmt} dest"),
            };
        }
    }
}
