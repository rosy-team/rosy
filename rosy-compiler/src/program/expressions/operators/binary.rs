//! Shared AST for every infix operator (`+`, `&`, `|`, `^`, …).

use crate::program::expressions::{Expr, ExprKind};
use crate::resolve::{ExprRecipe, ScopeContext, TypeResolver, TypeSlot};
use crate::transpile::{
    TranspilationInputContext, TranspilationOutput, Transpile, TranspileableExpr, ValueKind,
    VariableScope, emit_as_rosy_value_ref, emit_unwrap_rosy_value,
};
use anyhow::{Error, Result, anyhow};
use rosy_lib::{BinaryOp, RosyBaseType, RosyType};
use std::collections::{BTreeSet, HashSet};

#[derive(Debug)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

/// `d_in > 0` and `LIST|i < PIVOT` when one side is a series.
/// COSY compares the constant term.
fn series_const_compare(op: BinaryOp, left: &RosyType, right: &RosyType) -> bool {
    if !matches!(
        op,
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Lte | BinaryOp::Gte | BinaryOp::Eq | BinaryOp::Neq
    ) {
        return false;
    }
    if left.dimensions != 0 || right.dimensions != 0 {
        return false;
    }
    let numeric = |ty: &RosyType| matches!(ty.base_type, RosyBaseType::RE | RosyBaseType::DA);
    numeric(left) && numeric(right) && (*left != RosyType::RE() || *right != RosyType::RE())
}

/// `(Y(I)-Y(K))*(Y(I)-Y(K))`: both operands are the same computed value.
/// Evaluate it once and pass it twice. Only when the operand calls no user
/// FUNCTION or PROCEDURE, so evaluating it once cannot skip a side effect;
/// operators and intrinsics are pure, and the result is bit-identical.
/// PHAD's SIMO pair loop spends three of its 27 DA operations on this.
fn shared_operand_call(
    op: BinaryOp,
    left: &TranspilationOutput,
    right: &TranspilationOutput,
) -> Option<String> {
    if !matches!(left.value_kind, ValueKind::Owned)
        || left.serialization != right.serialization
        || left.serialization.contains("__fn_")
        || left.serialization.contains("__proc_")
    {
        return None;
    }
    Some(format!(
        "{{ let __shared = {}; {}(&__shared, &__shared)? }}",
        left.serialization,
        op.rust_call()
    ))
}

fn collect_concat_leaves<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let ExprKind::Binary(bin) = &expr.inner
        && bin.op == BinaryOp::Concat
    {
        collect_concat_leaves(&bin.left, out);
        collect_concat_leaves(&bin.right, out);
        return;
    }
    out.push(expr);
}

impl BinaryExpr {
    pub fn new(op: BinaryOp, left: Expr, right: Expr) -> Self {
        Self {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Homogeneous scalar `&` chains: `1 & 2 & 3` → `vec![1, 2, 3]`,
    /// `"a" & "b"` → `format!("{}{}", a, b)`. Mixed types stay pairwise.
    fn try_emit_flat_concat(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
        let mut leaves = Vec::new();
        collect_concat_leaves(&self.left, &mut leaves);
        collect_concat_leaves(&self.right, &mut leaves);
        if leaves.len() < 2 {
            return None;
        }

        let mut types = Vec::with_capacity(leaves.len());
        for leaf in &leaves {
            types.push(leaf.type_of(context).ok()?);
        }
        if types.iter().any(|t| t.is_any()) {
            return None;
        }
        let all_re = types
            .iter()
            .all(|t| t.base_type == RosyBaseType::RE && t.dimensions == 0);
        let all_st = types
            .iter()
            .all(|t| t.base_type == RosyBaseType::ST && t.dimensions == 0);
        if !all_re && !all_st {
            return None;
        }

        let mut parts = Vec::with_capacity(leaves.len());
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();
        for (i, leaf) in leaves.iter().enumerate() {
            match leaf.transpile(context) {
                Ok(output) => {
                    requested_variables.extend(output.requested_variables.iter().cloned());
                    parts.push(if all_re {
                        output.as_value()
                    } else {
                        output.as_ref()
                    });
                }
                Err(e) => {
                    for err in e {
                        errors.push(err.context(format!(
                            "...while transpiling term {} of concatenation",
                            i + 1
                        )));
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Some(Err(errors));
        }

        let serialization = if all_re {
            format!("vec![{}]", parts.join(", "))
        } else {
            format!(
                "format!(\"{}\"{})",
                "{}".repeat(parts.len()),
                parts.iter().map(|p| format!(", {p}")).collect::<String>()
            )
        };
        Some(Ok(TranspilationOutput {
            serialization,
            requested_variables,
            value_kind: ValueKind::Owned,
        }))
    }
}

impl TranspileableExpr for BinaryExpr {
    fn type_of(&self, context: &TranspilationInputContext) -> Result<RosyType> {
        let left_type = self.left.type_of(context)?;
        let right_type = self.right.type_of(context)?;
        if let Some(ty) = self.op.return_type(&left_type, &right_type) {
            return Ok(ty);
        }
        // Mixed RE/DA relations compare the constant term. `return_type`
        // has no arm for that pair, and `LO(...)` / WHILE ask `type_of`
        // before emit.
        if series_const_compare(self.op, &left_type, &right_type) {
            return Ok(RosyType::LO());
        }
        Err(anyhow!(
            "Cannot apply {:?} to types '{}' and '{}'!",
            self.op,
            left_type,
            right_type
        ))
    }

    fn discover_expr_function_calls(
        &self,
        resolver: &mut TypeResolver,
        ctx: &ScopeContext,
    ) -> Option<Result<()>> {
        if let Err(e) = resolver.discover_expr_function_calls(&self.left, ctx) {
            return Some(Err(e));
        }
        Some(resolver.discover_expr_function_calls(&self.right, ctx))
    }

    fn build_expr_recipe(
        &self,
        resolver: &TypeResolver,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        let left = resolver.build_expr_recipe(&self.left, ctx, deps);
        let right = resolver.build_expr_recipe(&self.right, ctx, deps);
        if self.op == BinaryOp::Concat {
            ExprRecipe::Concat(Box::new(left), Box::new(right))
        } else {
            ExprRecipe::BinaryOp {
                op: self.op,
                left: Box::new(left),
                right: Box::new(right),
            }
        }
    }

    fn try_inplace_append(
        &self,
        target_var: &str,
        target_indices: &[String],
        dest: &str,
        context: &mut TranspilationInputContext,
    ) -> Option<Result<TranspilationOutput, Vec<Error>>> {
        if !matches!(self.op, BinaryOp::Concat | BinaryOp::Add) {
            return None;
        }
        let left_ident = match &self.left.inner {
            ExprKind::Var(v) => &v.identifier,
            ExprKind::Ident(id) => id,
            _ => return None,
        };
        if left_ident.name != target_var {
            return None;
        }
        let mut left_indices = Vec::new();
        for index_expr in left_ident.flat_indices() {
            match index_expr.transpile(context) {
                Ok(out) => left_indices.push(out.as_value()),
                Err(e) => return Some(Err(e)),
            }
        }
        if left_indices != target_indices {
            return None;
        }

        let left_type = self.left.type_of(context).ok()?;
        let right_type = self.right.type_of(context).ok()?;

        // `X := X + E` on one DA: add into X. E is evaluated first, so it may
        // read X. Same result as the full add, without a new buffer.
        if self.op == BinaryOp::Add {
            if left_type != RosyType::DA() || right_type != RosyType::DA() {
                return None;
            }
            let right_output = match self.right.transpile(context) {
                Ok(out) => out,
                Err(e) => return Some(Err(e)),
            };
            let mut requested_variables = BTreeSet::new();
            requested_variables.extend(right_output.requested_variables.iter().cloned());
            if matches!(
                context.variables.get(target_var).map(|v| &v.scope),
                Some(VariableScope::Higher)
            ) {
                requested_variables.insert(target_var.to_string());
            }
            // A plain reference (`X := X + Y`) could alias X; keep the full path.
            if !matches!(right_output.value_kind, ValueKind::Owned) {
                return None;
            }
            let rhs = format!("let __rhs: DA = {};", right_output.serialization);
            return Some(Ok(TranspilationOutput {
                serialization: format!("{{ {rhs} ({dest}).add_assign_exact(&__rhs)?; }}"),
                requested_variables,
                ..Default::default()
            }));
        }

        // `X := X & CELL` with a list X and an untyped cell: append in place.
        // The full path copied all of X on every append, so a loop that
        // grows a list chunk by chunk was quadratic (PHAD's `snumbs`).
        if left_type == RosyType::VE() && right_type.is_any() && right_type.dimensions == 0 {
            let right_output = match self.right.transpile(context) {
                Ok(out) => out,
                Err(e) => return Some(Err(e)),
            };
            let mut requested_variables = BTreeSet::new();
            requested_variables.extend(right_output.requested_variables.iter().cloned());
            if matches!(
                context.variables.get(target_var).map(|v| &v.scope),
                Some(VariableScope::Higher)
            ) {
                requested_variables.insert(target_var.to_string());
            }
            return Some(Ok(TranspilationOutput {
                serialization: format!(
                    "{{ let __rhs: RosyValue = {}; rosy_append_ve(&mut {dest}, &__rhs)?; }}",
                    right_output.as_value()
                ),
                requested_variables,
                ..Default::default()
            }));
        }

        let is_push = match (
            left_type.base_type,
            left_type.dimensions,
            right_type.base_type,
            right_type.dimensions,
        ) {
            (RosyBaseType::VE, 0, RosyBaseType::RE, 0) => true,
            (RosyBaseType::VE, 0, RosyBaseType::VE, 0) => false,
            (RosyBaseType::DA, d, RosyBaseType::DA, 0) if d > 0 => true,
            (RosyBaseType::DA, d1, RosyBaseType::DA, d2) if d1 > 0 && d2 > 0 => false,
            (RosyBaseType::CD, d, RosyBaseType::CD, 0) if d > 0 => true,
            (RosyBaseType::CD, d1, RosyBaseType::CD, d2) if d1 > 0 && d2 > 0 => false,
            _ => return None,
        };

        let right_output = match self.right.transpile(context) {
            Ok(out) => out,
            Err(e) => return Some(Err(e)),
        };

        let mut requested_variables = BTreeSet::new();
        requested_variables.extend(right_output.requested_variables.iter().cloned());
        if matches!(
            context.variables.get(target_var).map(|v| &v.scope),
            Some(VariableScope::Higher)
        ) {
            requested_variables.insert(target_var.to_string());
        }

        let needs_clone = matches!(right_type.base_type, RosyBaseType::DA | RosyBaseType::CD);
        let code = if is_push {
            let val = right_output.as_value();
            if needs_clone {
                format!("{{ let __v = {val}.clone(); {dest}.push(__v); }}")
            } else {
                format!("{{ let __v = {val}; {dest}.push(__v); }}")
            }
        } else {
            let val_ref = right_output.as_ref();
            format!("{{ let __v: Vec<_> = ({val_ref}).to_vec(); {dest}.extend_from_slice(&__v); }}")
        };

        Some(Ok(TranspilationOutput {
            serialization: code,
            requested_variables,
            ..Default::default()
        }))
    }
}

impl Transpile for BinaryExpr {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let left_type = self.left.type_of(context).map_err(|e| vec![e])?;
        let right_type = self.right.type_of(context).map_err(|e| vec![e])?;
        if self.op.return_type(&left_type, &right_type).is_none()
            && !series_const_compare(self.op, &left_type, &right_type)
        {
            return Err(vec![anyhow!(
                "Cannot apply {:?} to types '{}' and '{}'!",
                self.op,
                left_type,
                right_type
            )]);
        }

        if self.op == BinaryOp::Concat
            && let Some(flat) = self.try_emit_flat_concat(context)
        {
            return flat;
        }

        let mut errors = Vec::new();
        let mut requested_variables = BTreeSet::new();

        let left_output = match self.left.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling left-hand side"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(left_output.requested_variables.iter().cloned());

        let right_output = match self.right.transpile(context) {
            Ok(output) => output,
            Err(mut e) => {
                for err in e.drain(..) {
                    errors.push(err.context("...while transpiling right-hand side"));
                }
                TranspilationOutput::default()
            }
        };
        requested_variables.extend(right_output.requested_variables.iter().cloned());

        let result_type = self
            .op
            .return_type(&left_type, &right_type)
            .unwrap_or_else(RosyType::LO);
        let scalar = left_type.dimensions == 0 && right_type.dimensions == 0;
        let l = left_output.as_value();
        let r = right_output.as_value();
        let lref = left_output.as_ref();
        let rref = right_output.as_ref();
        let pair = (left_type.base_type, right_type.base_type);

        let serialization = if series_const_compare(self.op, &left_type, &right_type) {
            let as_re = |ty: &RosyType, value: &str, refer: &str| {
                if *ty == RosyType::RE() {
                    value.to_string()
                } else {
                    format!("rosy_as_f64(&{refer})")
                }
            };
            let op = match self.op {
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Lte => "<=",
                BinaryOp::Gte => ">=",
                BinaryOp::Eq => "==",
                BinaryOp::Neq => "!=",
                _ => "==",
            };
            format!(
                "({} {op} {})",
                as_re(&left_type, &l, &lref),
                as_re(&right_type, &r, &rref)
            )
        } else if left_type.is_any() || right_type.is_any() {
            // Reading `LIST|J` off a VE must not box a copy of LIST first.
            let left = if self.op == BinaryOp::Extract && left_type == RosyType::VE() {
                left_output.as_ref()
            } else {
                emit_as_rosy_value_ref(&left_output, &left_type)
            };
            emit_unwrap_rosy_value(
                format!(
                    "rosy_dyn_binary(BinaryOp::{:?}, {}, {})?",
                    self.op,
                    left,
                    emit_as_rosy_value_ref(&right_output, &right_type)
                ),
                &result_type,
            )
        } else if self.op == BinaryOp::Derive {
            format!("RosyDerive::rosy_derive({}, ({}).clone() as i64)?", lref, r)
        } else if self.op == BinaryOp::Extract {
            format!(
                "RosyExtract::rosy_extract({}, {}).context(\"...while trying to extract an element\")?",
                lref, rref
            )
        } else if scalar {
            match (self.op, pair) {
                (BinaryOp::Add, (RosyBaseType::RE, RosyBaseType::RE)) => format!("({l} + {r})"),
                (BinaryOp::Add, (RosyBaseType::LO, RosyBaseType::LO)) => format!("({l} || {r})"),
                (BinaryOp::Sub, (RosyBaseType::RE, RosyBaseType::RE)) => format!("({l} - {r})"),
                (BinaryOp::Mult, (RosyBaseType::RE, RosyBaseType::RE)) => format!("({l} * {r})"),
                (BinaryOp::Mult, (RosyBaseType::LO, RosyBaseType::LO)) => format!("({l} && {r})"),
                (BinaryOp::Div, (RosyBaseType::RE, RosyBaseType::RE)) => format!("({l} / {r})"),
                (BinaryOp::And, (RosyBaseType::LO, RosyBaseType::LO)) => format!("({l} && {r})"),
                (BinaryOp::Or, (RosyBaseType::LO, RosyBaseType::LO)) => format!("({l} || {r})"),
                (
                    BinaryOp::Lt
                    | BinaryOp::Gt
                    | BinaryOp::Lte
                    | BinaryOp::Gte
                    | BinaryOp::Eq
                    | BinaryOp::Neq,
                    (RosyBaseType::RE, RosyBaseType::RE) | (RosyBaseType::ST, RosyBaseType::ST),
                ) => {
                    let sym = match self.op {
                        BinaryOp::Lt => "<",
                        BinaryOp::Gt => ">",
                        BinaryOp::Lte => "<=",
                        BinaryOp::Gte => ">=",
                        BinaryOp::Eq => "==",
                        BinaryOp::Neq => "!=",
                        _ => unreachable!(),
                    };
                    format!("({l} {sym} {r})")
                }
                (BinaryOp::Eq | BinaryOp::Neq, (RosyBaseType::LO, RosyBaseType::LO)) => {
                    let sym = if self.op == BinaryOp::Eq { "==" } else { "!=" };
                    format!("({l} {sym} {r})")
                }
                _ => shared_operand_call(self.op, &left_output, &right_output)
                    .unwrap_or_else(|| format!("{}({lref}, {rref})?", self.op.rust_call())),
            }
        } else {
            shared_operand_call(self.op, &left_output, &right_output)
                .unwrap_or_else(|| format!("{}({lref}, {rref})?", self.op.rust_call()))
        };

        if errors.is_empty() {
            Ok(TranspilationOutput {
                serialization,
                requested_variables,
                value_kind: ValueKind::Owned,
            })
        } else {
            Err(errors)
        }
    }
}
