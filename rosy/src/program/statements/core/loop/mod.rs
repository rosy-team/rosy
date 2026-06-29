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
//! ## Rosy Example
//! ```text
#![doc = include_str!("test.rosy")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("rosy_output.txt")]
//! ```
//! ## COSY INFINITY Example
//! ```text
#![doc = include_str!("test.fox")]
//! ```
//! **Output**:
//! ```text
#![doc = include_str!("cosy_output.txt")]
//! ```

use anyhow::{Context, Error, Result, anyhow, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::{CosyParser, *},
    program::{
        expressions::Expr,
        statements::{SourceLocation, Statement},
    },
    resolve::*,
    rosy_lib::RosyType,
    transpile::*,
};
use pest::Parser;

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
            let start_loop_pair = inner.next().context("Missing first token `start_loop`!")?;
            let mut start_loop_inner = start_loop_pair.into_inner();

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
            let end_text = end_pair.as_str().to_string();
            let mut end = Expr::from_rule(end_pair)
                .context("Failed to build `end` expression in `loop` statement!")?
                .ok_or_else(|| {
                    anyhow::anyhow!("Expected expression for `end` in `loop` statement")
                })?;

            // Optional step expression
            let mut step = if let Some(step_pair) = start_loop_inner.next() {
                if step_pair.as_rule() == Rule::expr {
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

            if step.is_none() {
                if let Some((recovered_end, recovered_step)) =
                    recover_trailing_signed_numeric_loop_step(&end_text)
                        .context("Failed to recover signed numeric LOOP step")?
                {
                    end = recovered_end;
                    step = Some(recovered_step);
                }
            }

            (iterator, start, end, step)
        };

        let mut body = Vec::new();
        // Process remaining elements (statements and end)
        while let Some(element) = inner.next() {
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

fn parse_loop_expr_fragment(src: &str) -> Result<Expr> {
    let trimmed = src.trim();
    let mut pairs = CosyParser::parse(Rule::expr, trimmed)
        .with_context(|| format!("Failed to parse LOOP expression fragment `{trimmed}`"))?;
    let pair = pairs
        .next()
        .ok_or_else(|| anyhow::anyhow!("Empty LOOP expression fragment `{trimmed}`"))?;
    Expr::from_rule(pair)?
        .ok_or_else(|| anyhow::anyhow!("Expected expression in LOOP fragment `{trimmed}`"))
}

fn recover_trailing_signed_numeric_loop_step(end_text: &str) -> Result<Option<(Expr, Expr)>> {
    let trimmed = end_text.trim_end();
    let Some(split_at) = trimmed.rfind(char::is_whitespace) else {
        return Ok(None);
    };

    let step_text = trimmed[split_at..].trim_start();
    let end_text = trimmed[..split_at].trim_end();
    if end_text.is_empty() || !is_signed_numeric_literal(step_text) {
        return Ok(None);
    }

    Ok(Some((
        parse_loop_expr_fragment(end_text)?,
        parse_loop_expr_fragment(step_text)?,
    )))
}

fn is_signed_numeric_literal(src: &str) -> bool {
    if !src.starts_with('-') {
        return false;
    }
    let normalized = src.replace('D', "E").replace('d', "e");
    normalized.parse::<f64>().is_ok()
}

impl TranspileableStatement for LoopStatement {
    fn register_typeslot_declaration(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> TypeslotDeclarationResult {
        TypeslotDeclarationResult::NotAVarFuncOrProcedureDecl
    }
    fn wire_inference_edges(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> InferenceEdgeResult {
        let mut inner_ctx = ctx.clone();
        // Loop iterator is always RE
        let iter_slot = TypeSlot::Variable(ctx.scope_path.clone(), self.iterator.clone());
        resolver.insert_slot(
            iter_slot.clone(),
            Some(&RosyType::RE()),
            Some(source_location),
        );
        inner_ctx.variables.insert(self.iterator.clone(), iter_slot);
        InferenceEdgeResult::HasEdges {
            result: resolver.discover_slots(&self.body, &mut inner_ctx),
        }
    }
    fn hydrate_resolved_types(
        &mut self,
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> TypeHydrationResult {
        if let Err(e) = resolver.apply_to_ast(&mut self.body, current_scope) {
            return TypeHydrationResult::Hydrated { result: Err(e) };
        }
        TypeHydrationResult::Hydrated { result: Ok(()) }
    }
}
impl Transpile for LoopStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify the start, end, and step expressions are REs
        let start_type = self.start.type_of(context).map_err(|e| vec![e])?;
        if start_type != RosyType::RE() {
            return Err(vec![anyhow!(
                "Loop start expression must be of type 'RE', found '{}'",
                start_type
            )]);
        }
        let end_type = self.end.type_of(context).map_err(|e| vec![e])?;
        if end_type != RosyType::RE() {
            return Err(vec![anyhow!(
                "Loop end expression must be of type 'RE', found '{}'",
                end_type
            )]);
        }
        if let Some(step_expr) = &self.step {
            let step_type = step_expr.type_of(context).map_err(|e| vec![e])?;
            if step_type != RosyType::RE() {
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

        // Define the iterator variable (allow reuse of existing variable, as COSY does)
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
        let step_value = if let Some(step_expr) = &self.step {
            match step_expr.transpile(context) {
                Ok(output) => {
                    requested_variables.extend(output.requested_variables.iter().cloned());
                    output.as_value().to_string()
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
            String::from("1f64")
        };

        let serialization = format!(
            "{{\n\tlet __rosy_loop_end = {};\n\tlet __rosy_loop_step = {};\n\tensure!(__rosy_loop_step != 0f64, \"LOOP step cannot be zero\");\n\tlet mut {} = {};\n\twhile (__rosy_loop_step > 0f64 && {} <= __rosy_loop_end) || (__rosy_loop_step < 0f64 && {} >= __rosy_loop_end) {{\n{}\n\t\t{} += __rosy_loop_step;\n\t}}\n}}",
            end_output.as_value(),
            step_value,
            self.iterator,
            start_output.as_value(),
            self.iterator,
            self.iterator,
            indent(serialized_statements.join("\n")),
            self.iterator,
        );
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
