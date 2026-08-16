//! Closed expression AST. Replaces `Box<dyn TranspileableExpr>`.

use super::core::intrinsic_call::IntrinsicCallExpr;
use super::core::var_expr::VarExpr;
use super::core::variable_identifier::VariableIdentifier;
use super::operators::arithmetic::add::AddExpr;
use super::operators::arithmetic::div::DivExpr;
use super::operators::arithmetic::mult::MultExpr;
use super::operators::arithmetic::sub::SubExpr;
use super::operators::collection::concat::ConcatExpr;
use super::operators::collection::derive::DeriveExpr;
use super::operators::collection::extract::ExtractExpr;
use super::operators::comparison::eq::EqExpr;
use super::operators::comparison::gt::GtExpr;
use super::operators::comparison::gte::GteExpr;
use super::operators::comparison::lt::LtExpr;
use super::operators::comparison::lte::LteExpr;
use super::operators::comparison::neq::NeqExpr;
use super::operators::logical::and_op::AndExpr;
use super::operators::logical::or_op::OrExpr;
use super::operators::unary::neg::NegExpr;
use super::operators::unary::not::NotExpr;
use super::pow::PowExpr;
use super::types::cd::CDExpr;
use super::types::da::DAExpr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    ExprFunctionCallResult, TranspilationInputContext, TranspilationOutput, Transpile,
    TranspileableExpr,
};
use anyhow::{Error, Result};
use rosy_lib::RosyType;
use std::collections::HashSet;

macro_rules! expr_kind {
    ($($var:ident($ty:ty)),+ $(,)?) => {
        #[derive(Debug)]
        pub enum ExprKind {
            $($var($ty)),+
        }

        $(
            impl From<$ty> for ExprKind {
                fn from(v: $ty) -> Self {
                    Self::$var(v)
                }
            }
        )+

        impl Transpile for ExprKind {
            fn transpile(
                &self,
                context: &mut TranspilationInputContext,
            ) -> Result<TranspilationOutput, Vec<Error>> {
                match self {
                    $(Self::$var(v) => v.transpile(context),)+
                }
            }
        }

        impl TranspileableExpr for ExprKind {
            fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
                match self {
                    $(Self::$var(v) => v.type_of(context),)+
                }
            }
            fn discover_expr_function_calls(
                &self,
                resolver: &mut TypeResolver,
                ctx: &ScopeContext,
            ) -> ExprFunctionCallResult {
                match self {
                    $(Self::$var(v) => v.discover_expr_function_calls(resolver, ctx),)+
                }
            }
            fn build_expr_recipe(
                &self,
                resolver: &TypeResolver,
                ctx: &ScopeContext,
                deps: &mut HashSet<TypeSlot>,
            ) -> ExprRecipe {
                match self {
                    $(Self::$var(v) => v.build_expr_recipe(resolver, ctx, deps),)+
                }
            }
            fn as_bare_variable_name(&self) -> Option<&str> {
                match self {
                    $(Self::$var(v) => v.as_bare_variable_name(),)+
                }
            }
            fn try_inplace_append(
                &self,
                target_var: &str,
                context: &mut TranspilationInputContext,
            ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
                match self {
                    $(Self::$var(v) => v.try_inplace_append(target_var, context),)+
                }
            }
        }
    };
}

expr_kind! {
    Add(AddExpr),
    Sub(SubExpr),
    Mult(MultExpr),
    Div(DivExpr),
    Pow(PowExpr),
    Concat(ConcatExpr),
    Extract(ExtractExpr),
    Derive(DeriveExpr),
    Eq(EqExpr),
    Neq(NeqExpr),
    Lt(LtExpr),
    Gt(GtExpr),
    Lte(LteExpr),
    Gte(GteExpr),
    And(AndExpr),
    Or(OrExpr),
    Neg(NegExpr),
    Not(NotExpr),
    Intrinsic(IntrinsicCallExpr),
    Var(VarExpr),
    Ident(VariableIdentifier),
    Da(DAExpr),
    Cd(CDExpr),
    Number(f64),
    Boolean(bool),
    String(String),
}
