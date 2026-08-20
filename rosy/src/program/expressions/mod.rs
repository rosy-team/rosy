//! # Expressions
//!
//! Everything in Rosy that produces a value — operators, functions, literals,
//! and variable references.
//!
//! ## Looking for something?
//!
//! | I want to... | Go to |
//! |--------------|-------|
//! | Use `+`, `-`, `*`, `/`, `&`, `\|`, `^`, `AND`, `OR`, … | **[`operators::binary`]** |
//! | Use `NOT` or unary `-` | **[`operators::unary`]** |
//! | Call `SIN`, `ST`, … (named intrinsics) | [`core::intrinsic_call`] — types in `rosy_lib::registry` |
//! | Write a literal number, string, or boolean | **[`types`]** |
//! | Construct `DA(n)` or `CD(n)` | **[`types::da`]**, **[`types::cd`]** |
//!
//! ## Example
//!
//! ```text
//! x + y * 2           { arithmetic with precedence }
//! 1 & 2 & 3           { vector concatenation }
//! vec|3               { extract 3rd element }
//! SIN(x)              { intrinsic function }
//! ST(42)              { type conversion }
//! DA(1)               { DA variable constructor }
//! ```

pub mod core;
pub mod kind;
pub mod operators;
pub mod string_convert;
pub mod types;

pub use kind::ExprKind;

use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile};
use crate::{
    ast::{FromRule, PRATT_PARSER, Rule},
    resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot},
    transpile::{TranspileableExpr, add_context_to_all},
};
use rosy_lib::RosyType;
use std::collections::HashSet;

use crate::program::expressions::core::intrinsic_call::IntrinsicCallExpr;
use crate::program::expressions::core::var_expr::VarExpr;

use crate::program::expressions::operators::binary::BinaryExpr;
use crate::program::expressions::operators::unary::neg::NegExpr;
use crate::program::expressions::operators::unary::not::NotExpr;
use rosy_lib::BinaryOp;

use crate::program::expressions::types::cd::CDExpr;
use crate::program::expressions::types::da::DAExpr;
use anyhow::{Context, Error, Result, bail};

use crate::program::statements::SourceLocation;

#[derive(Debug)]
pub struct Expr {
    pub inner: ExprKind,
    pub source_location: SourceLocation,
}

fn dispatch_builtin_function(
    pair: pest::iterators::Pair<Rule>,
    loc: SourceLocation,
) -> Result<Expr> {
    let mut inner = pair.into_inner();
    let name_pair = inner
        .next()
        .ok_or_else(|| anyhow::anyhow!("builtin_function missing name"))?;
    let name = name_pair.as_str().trim().to_ascii_uppercase();
    let mut args = Vec::new();
    for arg_pair in inner {
        let expr = Expr::from_rule(arg_pair)
            .with_context(|| format!("Failed to parse argument of `{name}`"))?
            .ok_or_else(|| anyhow::anyhow!("Expected argument expression for `{name}`"))?;
        args.push(expr);
    }

    match name.as_str() {
        "DA" => {
            anyhow::ensure!(
                args.len() == 1,
                "`DA` expects 1 argument, got {}",
                args.len()
            );
            Ok(Expr {
                inner: DAExpr {
                    index: Box::new(args.remove(0)),
                }
                .into(),
                source_location: loc,
            })
        }
        "CD" => {
            anyhow::ensure!(
                args.len() == 1,
                "`CD` expects 1 argument, got {}",
                args.len()
            );
            Ok(Expr {
                inner: CDExpr {
                    index: Box::new(args.remove(0)),
                }
                .into(),
                source_location: loc,
            })
        }
        _ => {
            if rosy_lib::lookup_intrinsic(&name).is_none() {
                bail!("Unknown intrinsic `{name}`");
            }
            Ok(Expr {
                inner: IntrinsicCallExpr { name, args }.into(),
                source_location: loc,
            })
        }
    }
}

fn flatten_atom_arg(pair: pest::iterators::Pair<Rule>) -> pest::iterators::Pair<Rule> {
    let mut inner = pair.into_inner();
    inner.next().expect("atom_arg missing child")
}

fn flatten_glued_op<'a>(
    pair: pest::iterators::Pair<'a, Rule>,
    out: &mut Vec<pest::iterators::Pair<'a, Rule>>,
) {
    for g in pair.into_inner() {
        if g.as_rule() == Rule::atom_arg {
            out.push(flatten_atom_arg(g));
        } else {
            out.push(g);
        }
    }
}

fn flatten_arg_pairs(pair: pest::iterators::Pair<Rule>) -> Vec<pest::iterators::Pair<Rule>> {
    let mut out = Vec::new();
    flatten_arg_node(pair, &mut out);
    out
}

fn flatten_arg_node<'a>(
    pair: pest::iterators::Pair<'a, Rule>,
    out: &mut Vec<pest::iterators::Pair<'a, Rule>>,
) {
    match pair.as_rule() {
        Rule::arg | Rule::bare_arg | Rule::atom_arg | Rule::glued_tail | Rule::spaced_infix => {
            for g in pair.into_inner() {
                flatten_arg_node(g, out);
            }
        }
        Rule::arg_sp => {}
        Rule::glued_op_arg => flatten_glued_op(pair, out),
        _ => out.push(pair),
    }
}

impl FromRule for Expr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Expr>> {
        // Accept either an `expr` pair (walk its children for primaries+
        // infixes) or a bare primary pair (like neg_expr's operand after
        // the `neg_expr = { "-" ~ term }` grammar fix). Collecting into a
        // Vec<Pair> lets us feed both shapes through the same PrattParser
        // call uniformly.
        let src = pair.as_str().to_string();
        let pairs_iter: Vec<pest::iterators::Pair<Rule>> = match pair.as_rule() {
            Rule::expr => pair.into_inner().collect(),
            Rule::arg => {
                let flat = flatten_arg_pairs(pair);
                if flat.is_empty() {
                    anyhow::bail!("empty command arg `{src}`");
                }
                flat
            }
            _ => vec![pair],
        };
        if pairs_iter.is_empty() {
            anyhow::bail!("empty command arg `{src}`");
        }
        let result = PRATT_PARSER
            .map_primary(|primary| {
                let loc = SourceLocation::from_pair(&primary);
                match primary.as_rule() {
                    Rule::neg_expr => {
                        let mut inner = primary.into_inner();
                        let operand_pair = inner.next().ok_or_else(|| {
                            anyhow::anyhow!("Negation expression missing operand")
                        })?;
                        let operand = Expr::from_rule(operand_pair)
                            .context("Failed to parse negation operand")?
                            .ok_or_else(|| anyhow::anyhow!("Expected expression in negation"))?;
                        Ok(Expr {
                            inner: NegExpr {
                                operand: Box::new(operand),
                            }
                            .into(),
                            source_location: loc,
                        })
                    }
                    Rule::not_expr => {
                        // NOT expression: parse the inner operand
                        // The inner can be: boolean, variable_identifier, or expr (parenthesized)
                        let not_loc = loc.clone();
                        let mut inner = primary.into_inner();
                        let operand_pair = inner
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("NOT expression missing operand"))?;

                        // Handle different operand types
                        let operand = match operand_pair.as_rule() {
                            Rule::boolean => {
                                let op_loc = SourceLocation::from_pair(&operand_pair);
                                let b = bool::from_rule(operand_pair)?
                                    .ok_or_else(|| anyhow::anyhow!("Expected boolean"))?;
                                Expr {
                                    inner: b.into(),
                                    source_location: op_loc,
                                }
                            }
                            Rule::variable_identifier => {
                                let op_loc = SourceLocation::from_pair(&operand_pair);
                                let var_expr = VarExpr::from_rule(operand_pair)?
                                    .ok_or_else(|| anyhow::anyhow!("Expected VarExpr"))?;
                                Expr {
                                    inner: var_expr.into(),
                                    source_location: op_loc,
                                }
                            }
                            Rule::expr => Expr::from_rule(operand_pair)?.ok_or_else(|| {
                                anyhow::anyhow!("Failed to parse NOT operand expression")
                            })?,
                            other => {
                                return Err(anyhow::anyhow!(
                                    "Unexpected NOT operand type: {:?}",
                                    other
                                ));
                            }
                        };

                        Ok(Expr {
                            inner: NotExpr {
                                operand: Box::new(operand),
                            }
                            .into(),
                            source_location: not_loc,
                        })
                    }
                    Rule::variable_identifier => {
                        let var_expr = VarExpr::from_rule(primary)?;
                        Ok(Expr {
                            inner: var_expr
                                .ok_or_else(|| anyhow::anyhow!("Expected VarExpr"))?
                                .into(),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::number => {
                        let n = f64::from_rule(primary)?;
                        Ok(Expr {
                            inner: n.ok_or_else(|| anyhow::anyhow!("Expected number"))?.into(),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::boolean => {
                        let b = bool::from_rule(primary)?;
                        Ok(Expr {
                            inner: b.ok_or_else(|| anyhow::anyhow!("Expected boolean"))?.into(),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::string => {
                        let s = String::from_rule(primary)?;
                        Ok(Expr {
                            inner: s.ok_or_else(|| anyhow::anyhow!("Expected string"))?.into(),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::builtin_function => dispatch_builtin_function(primary, loc.clone()),
                    Rule::expr => {
                        // handle parenthesized expressions by recursively parsing
                        Expr::from_rule(primary)
                            .context("Failed to build expression for parenthesized `expr`")?
                            .ok_or_else(|| {
                                anyhow::anyhow!("Expected expression for parenthesized `expr`")
                            })
                    }
                    _ => bail!("Unexpected primary expr: {:?}", primary.as_rule()),
                }
            })
            .map_infix(|left, op, right| {
                let op_loc = SourceLocation::from_pair(&op);
                let bin = match op.as_rule() {
                    Rule::add => BinaryOp::Add,
                    Rule::sub => BinaryOp::Sub,
                    Rule::mult => BinaryOp::Mult,
                    Rule::div => BinaryOp::Div,
                    Rule::pow => BinaryOp::Pow,
                    Rule::concat => BinaryOp::Concat,
                    Rule::extract => BinaryOp::Extract,
                    Rule::derive => BinaryOp::Derive,
                    Rule::eq => BinaryOp::Eq,
                    Rule::neq => BinaryOp::Neq,
                    Rule::lt => BinaryOp::Lt,
                    Rule::gt => BinaryOp::Gt,
                    Rule::lte => BinaryOp::Lte,
                    Rule::gte => BinaryOp::Gte,
                    Rule::and_op => BinaryOp::And,
                    Rule::or_op => BinaryOp::Or,
                    other => bail!("Unexpected infix operator: {:?}", other),
                };
                let left = left.context("...while parsing left-hand side of infix expression")?;
                let right = right.context("...while parsing right-hand side of infix expression")?;
                Ok(Expr {
                    inner: BinaryExpr::new(bin, left, right).into(),
                    source_location: op_loc,
                })
            })
            .parse(pairs_iter.into_iter());

        result.map(Some)
    }
}
impl TranspileableExpr for Expr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        self.inner.type_of(context)
    }
    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> Option<Result<()>> {
        self.inner.discover_expr_function_calls(resolver, ctx)
    }
    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        self.inner.build_expr_recipe(resolver, ctx, deps)
    }
    fn as_bare_variable_name(&self) -> Option<&str> {
        self.inner.as_bare_variable_name()
    }
    fn try_inplace_append(
        &self,
        target_var: &str,
        target_indices: &[String],
        dest: &str,
        context: &mut TranspilationInputContext,
    ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
        self.inner
            .try_inplace_append(target_var, target_indices, dest, context)
    }
}
impl Transpile for Expr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        self.inner.transpile(context).map_err(|err_vec| {
            add_context_to_all(
                err_vec,
                format!("...while transpiling expression: {:?}", self),
            )
        })
    }
}
