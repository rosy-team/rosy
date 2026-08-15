//! # Expressions
//!
//! Everything in Rosy that produces a value — operators, functions, literals,
//! and variable references.
//!
//! ## Looking for something?
//!
//! | I want to... | Go to |
//! |--------------|-------|
//! | Use `+`, `-`, `*`, `/` | **[`operators::arithmetic`]** |
//! | Compare with `=`, `<`, `>`, etc. | **[`operators::comparison`]** |
//! | Use `&` (concat), `\|` (extract), `%` (derive) | **[`operators::collection`]** |
//! | Use `AND`, `OR` | **[`operators::logical`]** |
//! | Use `NOT` or unary `-` | **[`operators::unary`]** |
//! | Call `SIN`, `COS`, `TAN`, ... | **[`functions::math::trig`]** |
//! | Call `EXP`, `LOG`, `SQRT`, `SQR`, `^` | **[`functions::math::exponential`]** |
//! | Call `CMPLX`, `CONJ`, `REAL`, `IMAG` | **[`functions::math::complex`]** |
//! | Call `ABS`, `INT`, `NINT`, `NORM`, `CONS` | **[`functions::math::rounding`]** |
//! | Call `VMIN`, `VMAX` | **[`functions::math::vector`]** |
//! | Call `TYPE`, `ISRT`, `ISRT3` | **[`functions::math::query`]** |
//! | Convert types with `ST()`, `CM()`, `RE()`, `LO()`, `VE()` | **[`functions::conversion`]** |
//! | Use `LENGTH`, `TRIM`, `LTRIM`, `POSITION` | **[`functions::sys`]** |
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
pub mod functions;
pub mod operators;
pub mod types;

use crate::transpile::{TranspilationInputContext, TranspilationOutput, Transpile};
use crate::{
    ast::{FromRule, PRATT_PARSER, Rule},
    resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot},
    transpile::{ExprFunctionCallResult, TranspileableExpr, add_context_to_all},
};
use rosy_lib::RosyType;
use std::collections::HashSet;

use crate::program::expressions::core::intrinsic_call::IntrinsicCallExpr;
use crate::program::expressions::core::var_expr::VarExpr;

use crate::program::expressions::functions::math::exponential::pow::PowExpr;

use crate::program::expressions::operators::arithmetic::add::AddExpr;
use crate::program::expressions::operators::arithmetic::div::DivExpr;
use crate::program::expressions::operators::arithmetic::mult::MultExpr;
use crate::program::expressions::operators::arithmetic::sub::SubExpr;
use crate::program::expressions::operators::collection::concat::ConcatExpr;
use crate::program::expressions::operators::collection::derive::DeriveExpr;
use crate::program::expressions::operators::collection::extract::ExtractExpr;
use crate::program::expressions::operators::comparison::eq::EqExpr;
use crate::program::expressions::operators::comparison::gt::GtExpr;
use crate::program::expressions::operators::comparison::gte::GteExpr;
use crate::program::expressions::operators::comparison::lt::LtExpr;
use crate::program::expressions::operators::comparison::lte::LteExpr;
use crate::program::expressions::operators::comparison::neq::NeqExpr;
use crate::program::expressions::operators::logical::and_op::AndExpr;
use crate::program::expressions::operators::logical::or_op::OrExpr;
use crate::program::expressions::operators::unary::neg::NegExpr;
use crate::program::expressions::operators::unary::not::NotExpr;

use crate::program::expressions::types::cd::CDExpr;
use crate::program::expressions::types::da::DAExpr;
use anyhow::{Context, Error, Result, bail};

use crate::program::statements::SourceLocation;

#[derive(Debug)]
pub struct Expr {
    pub inner: Box<dyn TranspileableExpr>,
    pub source_location: SourceLocation,
}

fn unary_intrinsic(
    name: &'static str,
    pair: pest::iterators::Pair<Rule>,
    loc: SourceLocation,
) -> Result<Expr> {
    Ok(Expr {
        inner: Box::new(IntrinsicCallExpr::from_unary_pair(name, pair)?),
        source_location: loc,
    })
}

fn binary_intrinsic(
    name: &'static str,
    pair: pest::iterators::Pair<Rule>,
    loc: SourceLocation,
) -> Result<Expr> {
    Ok(Expr {
        inner: Box::new(IntrinsicCallExpr::from_binary_pair(name, pair)?),
        source_location: loc,
    })
}

impl FromRule for Expr {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Expr>> {
        // Accept either an `expr` pair (walk its children for primaries+
        // infixes) or a bare primary pair (like neg_expr's operand after
        // the `neg_expr = { "-" ~ term }` grammar fix). Collecting into a
        // Vec<Pair> lets us feed both shapes through the same PrattParser
        // call uniformly.
        let pairs_iter: Vec<pest::iterators::Pair<Rule>> = if pair.as_rule() == Rule::expr {
            pair.into_inner().collect()
        } else {
            vec![pair]
        };
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
                            inner: Box::new(NegExpr {
                                operand: Box::new(operand),
                            }),
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
                                    inner: Box::new(b),
                                    source_location: op_loc,
                                }
                            }
                            Rule::variable_identifier => {
                                let op_loc = SourceLocation::from_pair(&operand_pair);
                                let var_expr = VarExpr::from_rule(operand_pair)?
                                    .ok_or_else(|| anyhow::anyhow!("Expected VarExpr"))?;
                                Expr {
                                    inner: Box::new(var_expr),
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
                            inner: Box::new(NotExpr {
                                operand: Box::new(operand),
                            }),
                            source_location: not_loc,
                        })
                    }
                    Rule::variable_identifier => {
                        let var_expr = VarExpr::from_rule(primary)?;
                        Ok(Expr {
                            inner: Box::new(
                                var_expr.ok_or_else(|| anyhow::anyhow!("Expected VarExpr"))?,
                            ),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::number => {
                        let n = f64::from_rule(primary)?;
                        Ok(Expr {
                            inner: Box::new(n.ok_or_else(|| anyhow::anyhow!("Expected number"))?),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::boolean => {
                        let b = bool::from_rule(primary)?;
                        Ok(Expr {
                            inner: Box::new(b.ok_or_else(|| anyhow::anyhow!("Expected boolean"))?),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::string => {
                        let s = String::from_rule(primary)?;
                        Ok(Expr {
                            inner: Box::new(s.ok_or_else(|| anyhow::anyhow!("Expected string"))?),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::cm => unary_intrinsic("CM", primary, loc.clone()),
                    Rule::st => unary_intrinsic("ST", primary, loc.clone()),
                    Rule::lo => unary_intrinsic("LO", primary, loc.clone()),
                    Rule::da => {
                        let da_expr = DAExpr::from_rule(primary)?;
                        Ok(Expr {
                            inner: Box::new(
                                da_expr.ok_or_else(|| anyhow::anyhow!("Expected DAExpr"))?,
                            ),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::cd_intrinsic => {
                        let cd_expr = CDExpr::from_rule(primary)?;
                        Ok(Expr {
                            inner: Box::new(
                                cd_expr.ok_or_else(|| anyhow::anyhow!("Expected CDExpr"))?,
                            ),
                            source_location: loc.clone(),
                        })
                    }
                    Rule::position => binary_intrinsic("POSITION", primary, loc.clone()),
                    Rule::length => unary_intrinsic("LENGTH", primary, loc.clone()),
                    Rule::sin => unary_intrinsic("SIN", primary, loc.clone()),
                    Rule::cos_fn => unary_intrinsic("COS", primary, loc.clone()),
                    Rule::tan_fn => unary_intrinsic("TAN", primary, loc.clone()),
                    Rule::asin_fn => unary_intrinsic("ASIN", primary, loc.clone()),
                    Rule::acos_fn => unary_intrinsic("ACOS", primary, loc.clone()),
                    Rule::atan_fn => unary_intrinsic("ATAN", primary, loc.clone()),
                    Rule::sinh_fn => unary_intrinsic("SINH", primary, loc.clone()),
                    Rule::cosh_fn => unary_intrinsic("COSH", primary, loc.clone()),
                    Rule::tanh_fn => unary_intrinsic("TANH", primary, loc.clone()),
                    Rule::sqr => unary_intrinsic("SQR", primary, loc.clone()),
                    Rule::sqrt_fn => unary_intrinsic("SQRT", primary, loc.clone()),
                    Rule::exp_fn => unary_intrinsic("EXP", primary, loc.clone()),
                    Rule::log_fn => unary_intrinsic("LOG", primary, loc.clone()),
                    Rule::vmax => unary_intrinsic("VMAX", primary, loc.clone()),
                    Rule::vmin => unary_intrinsic("VMIN", primary, loc.clone()),
                    Rule::lst => unary_intrinsic("LST", primary, loc.clone()),
                    Rule::lcm => unary_intrinsic("LCM", primary, loc.clone()),
                    Rule::lcd => unary_intrinsic("LCD", primary, loc.clone()),
                    Rule::lre => unary_intrinsic("LRE", primary, loc.clone()),
                    Rule::llo => unary_intrinsic("LLO", primary, loc.clone()),
                    Rule::lve => unary_intrinsic("LVE", primary, loc.clone()),
                    Rule::lda => unary_intrinsic("LDA", primary, loc.clone()),
                    Rule::abs_fn => unary_intrinsic("ABS", primary, loc.clone()),
                    Rule::norm_fn => unary_intrinsic("NORM", primary, loc.clone()),
                    Rule::cons_fn => unary_intrinsic("CONS", primary, loc.clone()),
                    Rule::int_fn => unary_intrinsic("INT", primary, loc.clone()),
                    Rule::nint_fn => unary_intrinsic("NINT", primary, loc.clone()),
                    Rule::type_fn => unary_intrinsic("TYPE", primary, loc.clone()),
                    Rule::trim_fn => unary_intrinsic("TRIM", primary, loc.clone()),
                    Rule::ltrim_fn => unary_intrinsic("LTRIM", primary, loc.clone()),
                    Rule::isrt_fn => unary_intrinsic("ISRT", primary, loc.clone()),
                    Rule::isrt3_fn => unary_intrinsic("ISRT3", primary, loc.clone()),
                    Rule::cmplx_fn => unary_intrinsic("CMPLX", primary, loc.clone()),
                    Rule::conj_fn => unary_intrinsic("CONJ", primary, loc.clone()),
                    Rule::real_fn => unary_intrinsic("REAL", primary, loc.clone()),
                    Rule::imag_fn => unary_intrinsic("IMAG", primary, loc.clone()),
                    Rule::re_fn => unary_intrinsic("RE", primary, loc.clone()),
                    Rule::ve_fn => unary_intrinsic("VE", primary, loc.clone()),
                    Rule::varmem => unary_intrinsic("VARMEM", primary, loc.clone()),
                    Rule::varpoi => unary_intrinsic("VARPOI", primary, loc.clone()),
                    Rule::erf_fn => unary_intrinsic("ERF", primary, loc.clone()),
                    Rule::werf_fn => unary_intrinsic("WERF", primary, loc.clone()),
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
                match op.as_rule() {
                    Rule::add => {
                        let left = left
                            .context("...while transpiling left-hand side of `add` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `add` expression")?;
                        Ok(Expr {
                            inner: Box::new(AddExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::sub => {
                        let left = left
                            .context("...while transpiling left-hand side of `sub` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `sub` expression")?;
                        Ok(Expr {
                            inner: Box::new(SubExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::mult => {
                        let left = left
                            .context("...while transpiling left-hand side of `mult` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `mult` expression")?;
                        Ok(Expr {
                            inner: Box::new(MultExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::div => {
                        let left = left
                            .context("...while transpiling left-hand side of `div` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `div` expression")?;
                        Ok(Expr {
                            inner: Box::new(DivExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::pow => {
                        let left = left.context("...while transpiling base of `pow` expression")?;
                        let right =
                            right.context("...while transpiling exponent of `pow` expression")?;
                        Ok(Expr {
                            inner: Box::new(PowExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::concat => {
                        let left = left.context(
                            "...while transpiling left-hand side of `concat` expression",
                        )?;
                        let right = right.context(
                            "...while transpiling right-hand side of `concat` expression",
                        )?;
                        Ok(Expr {
                            inner: Box::new(ConcatExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::extract => {
                        let left =
                            left.context("...while transpiling object of `extract` expression")?;
                        let right =
                            right.context("...while transpiling index of `extract` expression")?;
                        Ok(Expr {
                            inner: Box::new(ExtractExpr {
                                object: Box::new(left),
                                index: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::derive => {
                        let left =
                            left.context("...while transpiling object of `derive` (%) expression")?;
                        let right = right
                            .context("...while transpiling index of `derive` (%) expression")?;
                        Ok(Expr {
                            inner: Box::new(DeriveExpr {
                                object: Box::new(left),
                                index: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::eq => {
                        let left =
                            left.context("...while transpiling left-hand side of `eq` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `eq` expression")?;
                        Ok(Expr {
                            inner: Box::new(EqExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::neq => {
                        let left = left
                            .context("...while transpiling left-hand side of `neq` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `neq` expression")?;
                        Ok(Expr {
                            inner: Box::new(NeqExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::lt => {
                        let left =
                            left.context("...while transpiling left-hand side of `lt` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `lt` expression")?;
                        Ok(Expr {
                            inner: Box::new(LtExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::gt => {
                        let left =
                            left.context("...while transpiling left-hand side of `gt` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `gt` expression")?;
                        Ok(Expr {
                            inner: Box::new(GtExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::lte => {
                        let left = left
                            .context("...while transpiling left-hand side of `lte` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `lte` expression")?;
                        Ok(Expr {
                            inner: Box::new(LteExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::gte => {
                        let left = left
                            .context("...while transpiling left-hand side of `gte` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `gte` expression")?;
                        Ok(Expr {
                            inner: Box::new(GteExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::and_op => {
                        let left = left
                            .context("...while transpiling left-hand side of `AND` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `AND` expression")?;
                        Ok(Expr {
                            inner: Box::new(AndExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    Rule::or_op => {
                        let left =
                            left.context("...while transpiling left-hand side of `OR` expression")?;
                        let right = right
                            .context("...while transpiling right-hand side of `OR` expression")?;
                        Ok(Expr {
                            inner: Box::new(OrExpr {
                                left: Box::new(left),
                                right: Box::new(right),
                            }),
                            source_location: op_loc.clone(),
                        })
                    }
                    _ => bail!("Unexpected infix operator: {:?}", op.as_rule()),
                }
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
    ) -> ExprFunctionCallResult {
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
        context: &mut TranspilationInputContext,
    ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
        self.inner.try_inplace_append(target_var, context)
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
