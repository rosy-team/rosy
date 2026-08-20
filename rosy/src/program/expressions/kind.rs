//! Closed expression AST. Replaces `Box<dyn TranspileableExpr>`.

use super::core::intrinsic_call::IntrinsicCallExpr;
use super::core::var_expr::VarExpr;
use super::core::variable_identifier::VariableIdentifier;
use super::operators::binary::BinaryExpr;
use super::operators::unary::neg::NegExpr;
use super::operators::unary::not::NotExpr;
use super::types::cd::CDExpr;
use super::types::da::DAExpr;
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr,
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
            ) -> Option<Result<()>> {
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
                target_indices: &[String],
                dest: &str,
                context: &mut TranspilationInputContext,
            ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
                match self {
                    $(Self::$var(v) => v.try_inplace_append(target_var, target_indices, dest, context),)+
                }
            }
        }
    };
}

expr_kind! {
    Binary(BinaryExpr),
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
