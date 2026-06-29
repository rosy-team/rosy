//! # PLOOP Statement (Parallel Loop)
//!
//! MPI-distributed loop that partitions iterations across ranks.
//! Results are gathered into a specified output variable.
//!
//! ## Syntax
//!
//! ```text
//! PLOOP i start end [commut];
//!     <statements>
//! ENDPLOOP output;
//! ```
//!
//! ## Partitioning convention (COSY INFINITY compatible)
//!
//! The PLOOP iterator partitions the **last (innermost)** dimension of the
//! output array. Each rank `g` writes into `output[...][g]` and the gather
//! broadcasts each inner Vec independently. For 1D outputs (or `VE`) this is
//! a no-op wrapper — `coordinate()` runs directly on the value.
//!
//! ## Example (1D — most common)
//!
//! ```text
//! VARIABLE (VE) results;
//! PLOOP I 1 100;
//!     results := results & (I * 2);
//! ENDPLOOP results;
//! ```
//!
//! ## Example (2D — per-rank column)
//!
//! ```text
//! { Each rank fills its column of a 4-row × NP-col matrix. }
//! VARIABLE (RE 4 NP) X;
//! PLOOP P 1 NP;
//!     LOOP J 1 4;
//!         X(J, P) := 100*P + J;
//!     ENDLOOP;
//! ENDPLOOP X;
//! ```

use anyhow::{Context, Error, Result, anyhow, bail, ensure};
use std::collections::BTreeSet;

use crate::{
    ast::*,
    program::{
        expressions::{Expr, core::variable_identifier::VariableIdentifier},
        statements::{SourceLocation, Statement},
    },
    resolve::*,
    rosy_lib::RosyType,
    transpile::*,
};

/// AST node for the parallel loop `PLOOP ... ENDPLOOP output;`.
#[derive(Debug)]
pub struct PLoopStatement {
    pub iterator: String,
    pub start: Expr,
    pub end: Expr,
    pub body: Vec<Statement>,
    pub commutivityfrom_rule: Option<u8>,
    pub output: VariableIdentifier,
}

impl FromRule for PLoopStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::ploop,
            "Expected `ploop` rule when building ploop statement, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();
        let (iterator, start, end) = {
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

            (iterator, start, end)
        };

        let mut body = Vec::new();
        // Process remaining elements (statements and end)
        let end_ploop_pair = loop {
            let element = inner.next().ok_or(anyhow::anyhow!(
                "Expected `end_ploop` statement at end of `ploop`!"
            ))?;

            // Skip the end element
            if element.as_rule() == Rule::end_ploop {
                break element;
            }

            let pair_input = element.as_str();
            if let Some(stmt) = Statement::from_rule(element)
                .with_context(|| format!("Failed to build statement from:\n{}", pair_input))?
            {
                body.push(stmt);
            }
        };
        let (commutivityfrom_rule, output) = {
            let mut end_ploop_inner = end_ploop_pair.into_inner();

            let first_pair = end_ploop_inner
                .next()
                .context("Missing first token in `end_ploop` statement!")?;
            let second_pair = end_ploop_inner
                .next()
                .context("Missing second token in `end_ploop` statement!")?;

            match (first_pair.as_rule(), second_pair.as_rule()) {
                (Rule::unit, Rule::variable_identifier) => {
                    let commutivityfrom_rule = first_pair.as_str().parse::<u8>().context(
                        "Failed to parse `commutivityfrom_rule` as u8 in `ploop` statement!",
                    )?;
                    let output = VariableIdentifier::from_rule(second_pair)
                        .context(
                            "Failed to build `output` variable identifier in `ploop` statement!",
                        )?
                        .ok_or_else(|| {
                            anyhow::anyhow!("Expected variable identifier for ploop statement")
                        })?;

                    (Some(commutivityfrom_rule), output)
                }
                (Rule::variable_identifier, Rule::semicolon) => {
                    let output = VariableIdentifier::from_rule(first_pair)
                        .context(
                            "Failed to build `output` variable identifier in `ploop` statement!",
                        )?
                        .ok_or_else(|| {
                            anyhow::anyhow!("Expected variable identifier for ploop statement")
                        })?;

                    (None, output)
                }
                _ => bail!("Expected `variable_identifier` in `end_ploop` statement!"),
            }
        };

        Ok(Some(PLoopStatement {
            iterator,
            start,
            end,
            commutivityfrom_rule,
            body,
            output,
        }))
    }
}
impl TranspileableStatement for PLoopStatement {
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
        let iter_slot = TypeSlot::Variable(ctx.scope_path.clone(), self.iterator.clone());
        resolver.insert_slot(
            iter_slot.clone(),
            Some(&RosyType::RE()),
            Some(source_location),
        );
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
impl Transpile for PLoopStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Verify the start and end expressions are REs
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

        // Define the iterator
        let mut inner_context = context.clone();
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();
        // Allow reuse of existing variable (COSY behavior)
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
        let mut serialized_statements = Vec::new();
        for stmt in &self.body {
            match stmt.transpile(&mut inner_context) {
                Ok(output) => {
                    serialized_statements.push(output.serialization);
                    requested_variables.extend(output.requested_variables);
                }
                Err(stmt_errors) => {
                    for e in stmt_errors {
                        errors.push(e.context("...while transpiling statement in loop"));
                    }
                }
            }
        }

        // Serialize the start and end expressions
        let _start_output = match self.start.transpile(context) {
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
        requested_variables.extend(_start_output.requested_variables.iter().cloned());
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

        // Check the type of the output array
        let output_type = self.output.type_of(context).map_err(|e| vec![e])?;
        if output_type.dimensions < 1 && output_type != RosyType::VE() {
            return Err(vec![anyhow!(
                "Output variable '{}' for a PLOOP must be an array type, found '{}'",
                self.output.name,
                output_type
            )]);
        }

        // Serialize the output identifier
        let output_serialization = match self.output.transpile(context) {
            Ok(output) => {
                requested_variables.extend(output.requested_variables);
                output.serialization
            }
            Err(vec_err) => {
                for e in vec_err {
                    errors.push(e.context(format!(
                        "...while transpiling output variable identifier '{}'",
                        self.output.name
                    )));
                }
                return Err(errors);
            }
        };

        let iterator_declaration_serialization = {
            requested_variables.insert("rosy_mpi_context".to_string());
            format!(
                "let mut __ploop_end: f64 = {};\n\tlet mut {} = rosy_mpi_context.get_group_num(&mut __ploop_end)? + 1.0f64;",
                end_output.as_value(),
                self.iterator,
            )
        };
        // Partition the OUTPUT array's last (innermost) dimension across MPI
        // ranks, matching COSY INFINITY's convention. For a 2D output declared
        // `(RE D1 NP) X`, each rank g writes into `X[i][g]` for every `i`, and
        // the gather broadcasts every inner Vec independently.
        //
        // For 1D arrays / VE, no outer loops are emitted — `coordinate()` runs
        // directly on the value, identical to the pre-1.2 behavior.
        let coordination_serialization = {
            let rank = if output_type == RosyType::VE() {
                1
            } else {
                output_type.dimensions
            };
            let outer_loop_count = rank.saturating_sub(1);
            let commut = self.commutivityfrom_rule.unwrap_or(1);

            let mut accessor = output_serialization.clone();
            let mut opens = String::new();
            for i in 0..outer_loop_count {
                let idx = format!("__ploop_dim_{}", i);
                opens.push_str(&format!("for {} in 0..{}.len() {{\n\t", idx, accessor));
                accessor = format!("{}[{}]", accessor, idx);
            }
            let inner_call = format!(
                "rosy_mpi_context.coordinate(&mut {}, {}u8, &mut __ploop_end)?;",
                accessor, commut,
            );
            let closes = "}".repeat(outer_loop_count);
            if outer_loop_count == 0 {
                inner_call
            } else {
                format!("{}{}\n\t{}", opens, inner_call, closes)
            }
        };
        let serialization = format!(
            "{{\n\t{}\n\n{}\n\n\t{}\n}}",
            iterator_declaration_serialization,
            indent(serialized_statements.join("\n")),
            coordination_serialization
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
