//! # PLOOP Statement (Parallel Loop)
//!
//! MPI-distributed loop that partitions iterations across ranks.
//! Results are gathered into a specified output variable.
//!
//! ## Syntax
//!
//! ```text
//! PLOOP i start end;
//!     <statements>
//! ENDPLOOP [commut] [output ...];
//! ```
//!
//! The optional leading integer is the communication option, not a variable.
//! COSY defaults it to 1 (allgather) when it is omitted. An empty name list
//! gathers nothing: the option is unused and each rank keeps its own writes.
//! Each listed name is exchanged with that same option. Rosy implements
//! option 1. Options 3 and up are other MPI patterns (gather, scatter, broadcast).
//!
//! ## Partitioning convention (COSY INFINITY compatible)
//!
//! The PLOOP iterator partitions the **last (innermost)** dimension of the
//! output array. Each rank `g` writes into `output[...][g]`. A 1D output (or
//! `VE`) is one `coordinate`. A 2D or 3D output exchanges that rank axis in
//! one bulk call per slab. Deeper arrays still walk cell by cell.
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
    transpile::*,
};
use rosy_lib::RosyType;

/// AST node for the parallel loop `PLOOP ... ENDPLOOP [outputs];`.
#[derive(Debug)]
pub struct PLoopStatement {
    pub iterator: String,
    pub start: Expr,
    pub end: Expr,
    pub body: Vec<Statement>,
    pub commutivityfrom_rule: Option<u8>,
    /// Names gathered after the loop. Empty means each rank keeps its own writes.
    pub outputs: Vec<VariableIdentifier>,
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
        let mut commutivityfrom_rule = None;
        let mut outputs = Vec::new();
        for pair in end_ploop_pair.into_inner() {
            match pair.as_rule() {
                Rule::unit => {
                    commutivityfrom_rule = Some(pair.as_str().parse::<u8>().context(
                        "Failed to parse `commutivityfrom_rule` as u8 in `ploop` statement!",
                    )?);
                }
                Rule::variable_identifier => {
                    let output = VariableIdentifier::from_rule(pair)
                        .context(
                            "Failed to build `output` variable identifier in `ploop` statement!",
                        )?
                        .ok_or_else(|| {
                            anyhow::anyhow!("Expected variable identifier for ploop statement")
                        })?;
                    outputs.push(output);
                }
                Rule::semicolon => {}
                other => bail!("Unexpected token in `end_ploop` statement: {:?}", other),
            }
        }

        Ok(Some(PLoopStatement {
            iterator,
            start,
            end,
            commutivityfrom_rule,
            body,
            outputs,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pest::Parser;

    fn parse_ploop(src: &str) -> PLoopStatement {
        let pair = CosyParser::parse(Rule::ploop, src)
            .unwrap_or_else(|e| panic!("ploop should parse:\n{e}"))
            .next()
            .expect("missing ploop pair");
        PLoopStatement::from_rule(pair)
            .expect("ploop should build")
            .expect("ploop should be present")
    }

    #[test]
    fn endploop_allows_no_gather_and_several_names() {
        let bare = parse_ploop("PLOOP I 1 NP;\nIP := I;\nENDPLOOP;");
        assert!(bare.outputs.is_empty());
        assert!(bare.commutivityfrom_rule.is_none());

        let one = parse_ploop("PLOOP I 1 NP;\nX(I) := 1;\nENDPLOOP X;");
        assert!(one.commutivityfrom_rule.is_none());
        assert_eq!(one.outputs.len(), 1);
        assert_eq!(one.outputs[0].name, "X");

        let many = parse_ploop("PLOOP I 1 NP;\nX(I) := 1;\nENDPLOOP 1 X Y;");
        assert_eq!(many.commutivityfrom_rule, Some(1));
        assert_eq!(
            many.outputs
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            vec!["X", "Y"]
        );
    }

    #[test]
    fn fox_ploop_gather_is_one_call_per_array() {
        use crate::ast;
        use crate::program::{IncludeTracker, Program, syntax_config};
        use crate::resolve::TypeResolver;
        use crate::transpile::{TranspilationInputContext, Transpile};
        use std::path::Path;

        let src = r#"
BEGIN ;
VARIABLE NP 1 ;
NP := 4 ;
VARIABLE COL 1 NP ;
VARIABLE GRID 1 4 NP ;
VARIABLE CUBE 1 2 3 NP ;
PLOOP I 1 NP ;
    COL(I) := 1 ;
ENDPLOOP COL ;
PLOOP I 1 NP ;
    GRID(1,I) := 1 ;
ENDPLOOP GRID ;
PLOOP I 1 NP ;
    CUBE(1,1,I) := 1 ;
ENDPLOOP CUBE ;
END ;
"#;
        let code = syntax_config::with_path(Some(Path::new("t.fox")), || {
            let program = ast::parse_source(src).unwrap().next().expect("program");
            let mut ast = Program::from_rule_with_includes(
                program,
                Some(Path::new("t.fox")),
                &mut IncludeTracker::default(),
            )
            .unwrap()
            .expect("ast");
            TypeResolver::resolve(&mut ast).expect("resolve");
            ast.transpile(&mut TranspilationInputContext::default())
                .unwrap_or_else(|errs| panic!("{errs:?}"))
                .serialization
        });
        assert_eq!(code.matches(".coordinate(").count(), 1, "{code}");
        assert_eq!(code.matches("coordinate_rows(").count(), 1, "{code}");
        assert_eq!(code.matches("coordinate_grid(").count(), 1, "{code}");
        assert!(!code.contains("__ploop_dim_"), "{code}");
    }
}
impl TranspileableStatement for PLoopStatement {
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
        let iter_slot = TypeSlot::Variable(ctx.scope_path.clone(), self.iterator.clone());
        resolver.insert_slot(
            iter_slot.clone(),
            Some(&RosyType::RE()),
            Some(source_location),
        );
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
impl Transpile for PLoopStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // RE, or an untyped fox cell (ANY) that holds a real at runtime.
        // LOOP already unwraps ANY. PLOOP has to, or `PLOOP I 1 NP` dies
        // after `NP` is passed into a procedure.
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

        // Define the iterator
        let mut inner_context = context.clone();
        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();
        // Reuse an existing variable (COSY behavior). A parent local is
        // captured. Leave its scope alone so the body dereferences `&mut`.
        let prior = inner_context.variables.get(&self.iterator).cloned();
        if prior
            .as_ref()
            .is_some_and(|v| v.scope == VariableScope::Higher)
        {
            requested_variables.insert(self.iterator.clone());
        }
        if prior.is_none() {
            inner_context.variables.insert(
                self.iterator.clone(),
                ScopedVariableData {
                    scope: VariableScope::Local,
                    data: VariableData {
                        name: self.iterator.clone(),
                        r#type: RosyType::RE(),
                        declared_dims: None,
                    },
                },
            );
        }

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

        let iterator_declaration_serialization = {
            requested_variables.insert("rosy_mpi_context".to_string());
            // COSY writes the iteration index into the named variable and
            // leaves it there. A fresh `let` would shadow that binding, so
            // the body would see the index while the real variable stayed 0.
            let bind = context.rust_ident(&self.iterator);
            let value = "rosy_mpi_context.get_group_num(&mut __ploop_end)? + 1.0f64";
            let set = if let Some(existing) = context.variables.get(&self.iterator) {
                let rhs = if existing.data.r#type.is_any() {
                    format!("RosyValue::from({value})")
                } else {
                    value.to_string()
                };
                let star = if matches!(existing.scope, VariableScope::Arg | VariableScope::Higher) {
                    "*"
                } else {
                    ""
                };
                format!("{star}{bind} = {rhs};")
            } else {
                format!("let mut {bind} = {value};")
            };
            // `ploop_enter` records the nesting that PROOT reads.
            format!(
                "let mut __ploop_end: f64 = {};\n\t{}\n\trosy_mpi_context.ploop_enter(__ploop_end);",
                end_output.as_re(&end_type),
                set,
            )
        };
        // Each listed name is gathered on its own. An empty list is a bare
        // ENDPLOOP: the body still runs on this rank, and nothing is exchanged.
        // The last (innermost) dimension is the rank axis, matching COSY.
        // 1D is one `coordinate`. 2D and 3D are one bulk exchange per slab.
        // A loop of `coordinate` per cell deadlocks the runtime in probes:
        // PHAD's long-range arrays are tens of thousands of cells.
        let commut = self.commutivityfrom_rule.unwrap_or(1);
        let mut coordination_parts = Vec::new();
        for output in &self.outputs {
            let output_type = output.type_of(context).map_err(|e| vec![e])?;
            if output_type.dimensions < 1 && output_type != RosyType::VE() {
                return Err(vec![anyhow!(
                    "Output variable '{}' for a PLOOP must be an array type, found '{}'",
                    output.name,
                    output_type
                )]);
            }
            let output_serialization = match output.transpile(context) {
                Ok(transpiled) => {
                    requested_variables.extend(transpiled.requested_variables);
                    transpiled.serialization
                }
                Err(vec_err) => {
                    for e in vec_err {
                        errors.push(e.context(format!(
                            "...while transpiling output variable identifier '{}'",
                            output.name
                        )));
                    }
                    return Err(errors);
                }
            };
            let rank = if output_type == RosyType::VE() {
                1
            } else {
                output_type.dimensions
            };
            let outer_loop_count = rank.saturating_sub(1);
            let bulk = match outer_loop_count {
                0 => "coordinate",
                1 => "coordinate_rows",
                2 => "coordinate_grid",
                _ => "",
            };
            if !bulk.is_empty() {
                // Arg and Higher names are already `&mut`, so reborrow them.
                let star = match context.variables.get(&output.name).map(|v| &v.scope) {
                    Some(VariableScope::Arg | VariableScope::Higher) => "*",
                    _ => "",
                };
                coordination_parts.push(format!(
                    "rosy_mpi_context.{bulk}(&mut {star}{output_serialization}, {commut}u8, &mut __ploop_end)?;",
                ));
                continue;
            }
            // Rank 4+ still walks cell by cell. PHAD's gathers are 1D..3D.
            let mut accessor = output_serialization;
            let mut opens = String::new();
            for i in 0..outer_loop_count {
                let idx = format!("__ploop_dim_{}", i);
                let n = format!("__ploop_n_{}", i);
                // Same trip count on every rank. A short Vec must still enter
                // the gather or the longer ranks block in `coordinate` forever.
                opens.push_str(&format!(
                    "let {n} = rosy_mpi_context.sync_extent({accessor}.len())?;\n\t{accessor}.resize_with({n}, Default::default);\n\tfor {idx} in 0..{n} {{\n\t",
                ));
                accessor = format!("{}[{}]", accessor, idx);
            }
            let inner_call = format!(
                "rosy_mpi_context.coordinate(&mut {}, {}u8, &mut __ploop_end)?;",
                accessor, commut,
            );
            let closes = "}".repeat(outer_loop_count);
            coordination_parts.push(format!("{}{}\n\t{}", opens, inner_call, closes));
        }
        let coordination_serialization = coordination_parts.join("\n\t");
        let serialization = format!(
            "{{\n\t{}\n\n{}\n\n\t{}\n\trosy_mpi_context.ploop_exit();\n}}",
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
