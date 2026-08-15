//! # Intrinsic call
//!
//! Generic AST node for a named intrinsic (`SIN(x)`, `POSITION(a, b)`, …).
//! Type rules and emit names come from [`rosy_lib::lookup_intrinsic`].

use crate::ast::{FromRule, Rule};
use crate::program::expressions::Expr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr, ValueKind,
};
use anyhow::{Context as AnyhowContext, Error, Result, bail};
use rosy_lib::RosyType;
use std::collections::{BTreeSet, HashSet};

/// A registry-backed intrinsic call.
#[derive(Debug)]
pub struct IntrinsicCallExpr {
    pub name: String,
    pub args: Vec<Expr>,
}

impl IntrinsicCallExpr {
    /// Parse a pest pair whose single inner child is the operand expression.
    pub fn from_unary_pair(name: &str, pair: pest::iterators::Pair<Rule>) -> Result<Self> {
        let mut inner = pair.into_inner();
        let expr_pair = inner
            .next()
            .with_context(|| format!("Missing inner expression for `{name}`!"))?;
        let expr = Expr::from_rule(expr_pair)
            .with_context(|| format!("Failed to build expression for `{name}`"))?
            .ok_or_else(|| anyhow::anyhow!("Expected expression for `{name}`"))?;
        Ok(Self {
            name: name.to_string(),
            args: vec![expr],
        })
    }

    /// Parse a pest pair with two inner expression children.
    pub fn from_binary_pair(name: &str, pair: pest::iterators::Pair<Rule>) -> Result<Self> {
        let mut inner = pair.into_inner();
        let lhs_pair = inner
            .next()
            .with_context(|| format!("Missing first argument for `{name}`!"))?;
        let rhs_pair = inner
            .next()
            .with_context(|| format!("Missing second argument for `{name}`!"))?;
        let lhs = Expr::from_rule(lhs_pair)
            .with_context(|| format!("Failed to build first argument for `{name}`"))?
            .ok_or_else(|| anyhow::anyhow!("Expected first argument for `{name}`"))?;
        let rhs = Expr::from_rule(rhs_pair)
            .with_context(|| format!("Failed to build second argument for `{name}`"))?
            .ok_or_else(|| anyhow::anyhow!("Expected second argument for `{name}`"))?;
        Ok(Self {
            name: name.to_string(),
            args: vec![lhs, rhs],
        })
    }
}

impl Transpile for IntrinsicCallExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let spec = rosy_lib::lookup_intrinsic(&self.name).ok_or_else(|| {
            vec![anyhow::anyhow!("Unknown intrinsic `{}`", self.name)]
        })?;

        if self.args.len() != spec.arity {
            return Err(vec![anyhow::anyhow!(
                "`{}` expects {} argument(s), got {}",
                self.name,
                spec.arity,
                self.args.len()
            )]);
        }

        let mut requested = BTreeSet::new();
        let mut arg_outs = Vec::with_capacity(self.args.len());
        let mut arg_types = Vec::with_capacity(self.args.len());
        for arg in &self.args {
            let ty = arg.type_of(context).map_err(|e| vec![e])?;
            let out = arg.transpile(context)?;
            requested.extend(out.requested_variables.iter().cloned());
            arg_types.push(ty);
            arg_outs.push(out);
        }

        let q = if spec.fallible { "?" } else { "" };
        let serialization = match (spec.arity, spec.native_re, arg_types.first()) {
            (1, Some(native), Some(ty)) if *ty == RosyType::RE() => {
                format!("{}{}", arg_outs[0].as_value(), native)
            }
            (1, _, _) => format!("{}({}){q}", spec.rust_call, arg_outs[0].as_ref()),
            _ => {
                let refs: Vec<String> = arg_outs.iter().map(|o| o.as_ref()).collect();
                format!("{}({}){q}", spec.rust_call, refs.join(", "))
            }
        };

        Ok(TranspilationOutput {
            serialization,
            requested_variables: requested,
            value_kind: ValueKind::Owned,
        })
    }
}

impl TranspileableExpr for IntrinsicCallExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let spec = rosy_lib::lookup_intrinsic(&self.name)
            .ok_or_else(|| anyhow::anyhow!("Unknown intrinsic `{}`", self.name))?;
        if self.args.len() != spec.arity {
            bail!(
                "`{}` expects {} argument(s), got {}",
                self.name,
                spec.arity,
                self.args.len()
            );
        }
        match spec.arity {
            1 => {
                let inner_type = self.args[0]
                    .type_of(context)
                    .with_context(|| format!("Failed to type operand of `{}`", self.name))?;
                spec.unary_return_type(&inner_type).ok_or_else(|| {
                    anyhow::anyhow!("`{}` not supported for type: {:?}", self.name, inner_type)
                })
            }
            2 => {
                let lhs = self.args[0].type_of(context)?;
                let rhs = self.args[1].type_of(context)?;
                spec.binary_return_type(&lhs, &rhs).ok_or_else(|| {
                    anyhow::anyhow!(
                        "`{}` not supported for types: {:?}, {:?}",
                        self.name,
                        lhs,
                        rhs
                    )
                })
            }
            n => bail!("Intrinsic `{}` has unsupported arity {n}", self.name),
        }
    }

    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> ExprFunctionCallResult {
        for arg in &self.args {
            if let Err(e) = resolver.discover_expr_function_calls(arg, ctx) {
                return ExprFunctionCallResult::HasFunctionCalls { result: Err(e) };
            }
        }
        ExprFunctionCallResult::HasFunctionCalls { result: Ok(()) }
    }

    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        match self.args.len() {
            1 => {
                let inner = resolver.build_expr_recipe(&self.args[0], ctx, deps);
                ExprRecipe::UnaryIntrinsic {
                    name: self.name.clone(),
                    inner: Box::new(inner),
                }
            }
            2 if self.name == "POSITION" => ExprRecipe::Literal(RosyType::RE()),
            _ => ExprRecipe::Unknown(Some(format!(
                "cannot infer type of {} with {} args",
                self.name,
                self.args.len()
            ))),
        }
    }
}
