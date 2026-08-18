//! # LOOP Statement (Counted Loop)
//!
//! Iterates a variable from a start value to an end value with an optional step.
//!
//! ## Syntax
//!
//! ```text
//! LOOP i start end [step];
//!     <statements>
//! ENDLOOP;
//! ```
//!
//! If `step` is omitted, it defaults to `1`. The loop variable `i` is
//! automatically declared as `RE` within the loop scope.
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/loop.rosy"))]
//! ```

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::Expr,
        statements::{SourceLocation, Statement},
    },
    resolve::*,
    transpile::*,
};
use rosy_lib::RosyType;

/// AST node for the counted `LOOP i start end [step]; ... ENDLOOP;` statement.
#[derive(Debug)]
pub struct LoopStatement {
    pub iterator: String,
    pub start: Expr,
    pub end: Expr,
    pub step: Option<Expr>,
    pub body: Vec<Statement>,
}

impl FromRule for LoopStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::r#loop,
            "Expected `loop` rule when building loop statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let (iterator, start, end, step) = {
            let mut start_loop_inner = inner
                .next()
                .context("Missing first token `start_loop`!")?
                .into_inner();

            let iterator = start_loop_inner
                .next()
                .context("Missing first token `variable_name`!")?
                .as_str()
                .to_string();
            let start_pair = start_loop_inner
                .next()
                .context("Missing second token `start_expr`!")?;
            let start = Expr::from_rule(start_pair)
                .context("Failed to build `start` expression in `loop` statement!")?
                .ok_or_else(|| {
                    anyhow::anyhow!("Expected expression for `start` in `loop` statement")
                })?;
            let end_pair = start_loop_inner
                .next()
                .context("Missing third token `end_expr`!")?;
            let end = Expr::from_rule(end_pair)
                .context("Failed to build `end` expression in `loop` statement!")?
                .ok_or_else(|| {
                    anyhow::anyhow!("Expected expression for `end` in `loop` statement")
                })?;

            // Optional step expression
            let step = if let Some(step_pair) = start_loop_inner.next() {
                if !matches!(step_pair.as_rule(), Rule::semicolon | Rule::end_loop) {
                    Some(
                        Expr::from_rule(step_pair)
                            .context("Failed to build `step` expression in `loop` statement!")?
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Expected expression for `step` in `loop` statement"
                                )
                            })?,
                    )
                } else {
                    None
                }
            } else {
                None
            };

            (iterator, start, end, step)
        };

        let mut body = Vec::new();
        // Process remaining elements (statements and end)
        for element in inner {
            // Skip the end element
            if element.as_rule() == Rule::end_loop {
                break;
            }

            let pair_input = element.as_str();
            if let Some(stmt) = Statement::from_rule(element)
                .with_context(|| format!("Failed to build statement from:\n{}", pair_input))?
            {
                body.push(stmt);
            }
        }

        Ok(Some(LoopStatement {
            iterator,
            start,
            end,
            step,
            body,
        }))
    }
}
impl TranspileableStatement for LoopStatement {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        None
    }
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        let mut inner_ctx = ctx.clone();
        // Loop iterator is always RE
        let iter_slot = TypeSlot::Variable(ctx.scope_path.clone(), self.iterator.clone());
        if !resolver.nodes.contains_key(&iter_slot) {
            resolver.insert_slot(
                iter_slot.clone(),
                Some(&RosyType::RE()),
                Some(source_location),
            );
        }
        inner_ctx.variables.insert(self.iterator.clone(), iter_slot);
        Some(resolver.discover_slots(&self.body, &mut inner_ctx))
    }
    fn hydrate_resolved_types(
        &mut self,
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> Option<Result<()>> {
        if let Err(e) = resolver.apply_to_ast(&mut self.body, current_scope) {
            return Some(Err(e));
        }
        Some(Ok(()))
    }
}
impl Transpile for LoopStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify the start, end, and step expressions are REs
        let as_re = |t: &RosyType| t == &RosyType::RE() || t.is_any();
        let start_type = self.start.type_of(context).map_err(|e| vec![e])?;
        if !as_re(&start_type) {
            return Err(vec![anyhow!(
                "Loop start expression must be of type 'RE', found '{}'",
                start_type
            )]);
        }
        let end_type = self.end.type_of(context).map_err(|e| vec![e])?;
        if !as_re(&end_type) {
            return Err(vec![anyhow!(
                "Loop end expression must be of type 'RE', found '{}'",
                end_type
            )]);
        }
        if let Some(step_expr) = &self.step {
            let step_type = step_expr.type_of(context).map_err(|e| vec![e])?;
            if !as_re(&step_type) {
                return Err(vec![anyhow!(
                    "Loop step expression must be of type 'RE', found '{}'",
                    step_type
                )]);
            }
        }

        // Define and raise the level of any existing variables
        let mut inner_context: TranspilationInputContext = context.clone();
        inner_context.in_loop = true;
        let mut requested_variables = BTreeSet::new();
        let mut serialized_statements = Vec::new();
        let mut errors = Vec::new();

        inner_context.variables.insert(
            self.iterator.clone(),
            ScopedVariableData {
                scope: VariableScope::Local,
                data: VariableData {
                    name: self.iterator.clone(),
                    r#type: RosyType::RE(),
                },
            },
        );

        // Transpile each inner statement
        for stmt in &self.body {
            match stmt.transpile(&mut inner_context) {
                Ok(output) => {
                    serialized_statements.push(output.serialization);
                    requested_variables.extend(output.requested_variables.iter().cloned());
                }
                Err(stmt_errors) => {
                    for e in stmt_errors {
                        errors.push(e.context("...while transpiling statement in loop"));
                    }
                }
            }
        }

        // Serialize the start, end, and step expressions
        let start_output = match self.start.transpile(context) {
            Ok(output) => output,
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context(format!(
                        "...while transpiling start expression for loop with iterator '{}'",
                        self.iterator
                    )));
                }
                return Err(errors);
            }
        };
        requested_variables.extend(start_output.requested_variables.iter().cloned());
        let end_output = match self.end.transpile(context) {
            Ok(output) => output,
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context(format!(
                        "...while transpiling end expression for loop with iterator '{}'",
                        self.iterator
                    )));
                }
                return Err(errors);
            }
        };
        requested_variables.extend(end_output.requested_variables.iter().cloned());
        let step_value: Option<String> = if let Some(step_expr) = &self.step {
            match step_expr.transpile(context) {
                Ok(output) => {
                    requested_variables.extend(output.requested_variables.iter().cloned());
                    let ty = step_expr.type_of(context).unwrap_or_else(|_| RosyType::RE());
                    Some(if ty.is_any() {
                        format!("({}).expect_re()?", output.as_owned(&RosyType::ANY()))
                    } else {
                        output.as_value().to_string()
                    })
                }
                Err(vec_err) => {
                    for e in vec_err {
                        errors.push(e.context(format!(
                            "...while transpiling step expression for loop with iterator '{}'",
                            self.iterator
                        )));
                    }
                    return Err(errors);
                }
            }
        } else {
            None
        };

        let re_val = |out: &TranspilationOutput, ty: &RosyType| {
            if ty.is_any() {
                format!("({}).expect_re()?", out.as_owned(&RosyType::ANY()))
            } else {
                out.as_value()
            }
        };
        let iter = &self.iterator;
        let start = re_val(&start_output, &start_type);
        let end = re_val(&end_output, &end_type);
        let body = indent(serialized_statements.join("\n"));
        let loop_core = if let Some(step) = &step_value {
            format!(
                "let mut {iter}: RE = rosy_as_f64(&({start}));\n\
                 let __{iter}_end: RE = rosy_as_f64(&({end}));\n\
                 let __{iter}_step: RE = rosy_as_f64(&({step}));\n\
                 while (__{iter}_step > 0.0_f64 && {iter} <= __{iter}_end) || (__{iter}_step <= 0.0_f64 && {iter} >= __{iter}_end) {{\n\
                 {body}\n\
                 \t{iter} += __{iter}_step;\n\
                 }}"
            )
        } else {
            format!(
                "let mut {iter}: RE = rosy_as_f64(&({start}));\n\
                 let __{iter}_end: RE = rosy_as_f64(&({end}));\n\
                 while {iter} <= __{iter}_end {{\n\
                 {body}\n\
                 \t{iter} += 1.0_f64;\n\
                 }}"
            )
        };
        // Always brace so `let mut I: RE` cannot shadow a later I in this block.
        let serialization = format!("{{ {loop_core} }}");
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
