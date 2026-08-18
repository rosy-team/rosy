//! # Variable Declaration Statement
//!
//! Declares one or more variables with a specified type and optional
//! array dimensions.
//!
//! ## Syntax
//!
//! ```text
//! VARIABLE (type) name1 [name2 ...];          { scalar }
//! VARIABLE (type ** n) name;                    { n-dimensional array }
//! ```
//!
//! ## Supported Types
//!
//! `RE`, `ST`, `LO`, `CM`, `VE`, `DA`, `CD`
//!
//! ## Example
//! ```text
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/constructs/statements/core/var_decl.rosy"))]
//! ```

use std::collections::BTreeSet;

use anyhow::{Context, Error, Result, anyhow, ensure};

use crate::{
    ast::*,
    program::{expressions::Expr, statements::SourceLocation},
    resolve::{ScopeContext, TypeResolver, TypeSlot},
    syntax_config,
    transpile::*,
};
use rosy_lib::{RosyBaseType, RosyType};

#[derive(Debug)]
pub struct VariableDeclarationData {
    pub name: String,
    pub r#type: Option<RosyType>,
    pub dimension_exprs: Vec<Expr>,
}
impl VariableDeclarationData {
    /// Helper to unwrap the type or return a descriptive error.
    pub fn require_type(&self) -> Result<RosyType, Error> {
        self.r#type.ok_or_else(|| {
            anyhow!(
                "Could not infer a type for '{}' - please specify the type explicitly",
                self.name
            )
        })
    }
}
impl Transpile for VariableDeclarationData {
    // note that this transpiles as the default value for the type
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        let resolved_type = self.require_type().map_err(|e| vec![e])?;

        let base_value = match resolved_type.base_type {
            RosyBaseType::RE => "0.0",
            RosyBaseType::ST => "\"\".to_string()",
            RosyBaseType::LO => "false",
            RosyBaseType::CM => "Complex64::new(0.0, 0.0)",
            RosyBaseType::VE => "vec![]",
            RosyBaseType::DA => "DA::zero()",
            RosyBaseType::CD => "CD::zero()",
            RosyBaseType::ANY => "RosyValue::RE(0.0)",
        }
        .to_string();

        let mut requested_variables = BTreeSet::new();
        let mut errors = Vec::new();
        let serialization = if self.dimension_exprs.is_empty() {
            // No explicit dimension expressions, but the resolved type may
            // still have dimensions (e.g. inferred from `X(I)(J) := expr`).
            // Wrap the base value in empty vecs for each inferred dimension.
            if resolved_type.dimensions > 0 {
                let mut result = base_value;
                for _ in 0..resolved_type.dimensions {
                    result = format!("vec![{}; 0]", result);
                }
                result
            } else {
                base_value
            }
        } else {
            let mut result = base_value;
            for dim in self.dimension_exprs.iter().rev() {
                // ensure the type compiles down to a RE
                if let Err(e) = dim.type_of(context).and_then(|t| {
                    let expected_type = RosyType::RE();
                    if t == expected_type {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!(
                            "Array dimension expression must be of type {expected_type}, found {t}"
                        ))
                    }
                }) {
                    errors.push(e.context("...while checking array dimension expression type"));
                    continue;
                }

                // transpile each dimension expression
                match dim.transpile(context) {
                    Ok(output) => {
                        result = format!("vec![{}; {} as usize]", result, output.as_value());
                        requested_variables.extend(output.requested_variables);
                    }
                    Err(dim_errors) => {
                        for e in dim_errors {
                            errors.push(
                                e.context("...while transpiling array dimension expression!"),
                            );
                        }
                    }
                }
            }
            result
        };

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

#[derive(Debug)]
pub struct VarDeclStatement {
    pub data: VariableDeclarationData,
}

impl FromRule for VarDeclStatement {
    fn from_rule(pair: pest::iterators::Pair<Rule>) -> Result<Option<Self>> {
        ensure!(
            pair.as_rule() == Rule::var_decl,
            "Expected `var_decl` rule when building variable declaration, found: {:?}",
            pair.as_rule()
        );

        let mut inner = pair.into_inner();

        // Peek at the next token to see if it's a type or a variable name
        let first = inner
            .next()
            .context("Missing tokens in variable declaration!")?;

        let (r#type, mut dimension_exprs, name) = if first.as_rule() == Rule::r#type {
            // Type is present: parse type, then name
            let (r#type, dimension_exprs) = build_type(first)
                .context("...while building variable type in variable declaration!")?;
            let name = inner
                .next()
                .context("Missing variable name after type in variable declaration!")?
                .as_str()
                .to_string();
            (Some(r#type), dimension_exprs, name)
        } else {
            // No type: first token is the variable name
            let name = first.as_str().to_string();
            (None, Vec::new(), name)
        };

        // Collect all memory_size expressions from the grammar
        let mut memory_sizes: Vec<Expr> = Vec::new();
        for mem_pair in inner {
            if mem_pair.as_rule() == Rule::memory_size {
                let mem_inner = mem_pair
                    .into_inner()
                    .next()
                    .context("Missing expression in memory_size")?;
                let expr = Expr::from_rule(mem_inner)
                    .context("...while building memory size expression in variable declaration!")?;
                if let Some(expr) = expr {
                    memory_sizes.push(expr);
                }
            }
        }

        // Apply syntax-mode-specific validation and semantics
        if syntax_config::is_cosy_syntax() {
            // ── COSY mode ──
            // Memory size is REQUIRED as the first expression after the name.
            // It is parsed and discarded (Rosy handles memory automatically).
            // Any additional expressions are array dimensions.
            if memory_sizes.is_empty() {
                anyhow::bail!(
                    "COSY syntax mode requires a memory size in VARIABLE declarations.\n\
                     Expected: VARIABLE {name} <memory_size> ;\n\
                     Hint: If you intended to use Rosy syntax, remove the `--cosy-syntax` flag."
                );
            }
            // First memory_size is allocation hint (discarded), rest are array dimensions
            if memory_sizes.len() > 1 {
                dimension_exprs.extend(memory_sizes.into_iter().skip(1));
            }
        } else {
            // ── Rosy mode (default) ──
            // Nothing is allowed after the variable name — dimensions go
            // inside the type parentheses, and memory sizes don't exist.
            if !memory_sizes.is_empty() {
                if let Some(ty) = &r#type {
                    let type_name = ty.base_type;
                    anyhow::bail!(
                        "In Rosy syntax, array dimensions must be declared inside the type annotation.\n\
                         Found: VARIABLE ({type_name:?}) {name} <{n} trailing expr(s)> ;\n\
                         Expected: VARIABLE ({type_name:?} <dims...>) {name} ;\n\
                         Hint: Move the dimensions into the parentheses, e.g. VARIABLE (RE 3 5) MY_ARRAY ;",
                        n = memory_sizes.len(),
                    );
                } else {
                    anyhow::bail!(
                        "In Rosy syntax, nothing is allowed after the variable name.\n\
                         Found: VARIABLE {name} <{n} trailing expr(s)> ;\n\
                         Expected: VARIABLE {name} ;  or  VARIABLE (TYPE) {name} ;\n\n\
                         Hint: If you need an array, declare dimensions in the type:\n> VARIABLE (RE 3 5) {name} ;",
                        n = memory_sizes.len(),
                    );
                }
            }
        }

        let data = VariableDeclarationData {
            name,
            r#type,
            dimension_exprs,
        };

        Ok(Some(VarDeclStatement { data }))
    }
}
impl TranspileableStatement for VarDeclStatement {
    fn register_typeslot_declaration(
        &self,
        resolver: &mut TypeResolver,
        ctx: &mut ScopeContext,
        source_location: SourceLocation,
    ) -> Option<Result<()>> {
        let slot = TypeSlot::Variable(ctx.scope_path.clone(), self.data.name.clone());

        resolver.insert_slot(
            slot.clone(),
            self.data.r#type.as_ref(),
            Some(source_location),
        );
        ctx.variables.insert(self.data.name.clone(), slot);

        Some(Ok(()))
    }
    fn wire_inference_edges(
        &self,
        _resolver: &mut TypeResolver,
        _ctx: &mut ScopeContext,
        _source_location: SourceLocation,
    ) -> Option<Result<()>> {
        None
    }
    fn hydrate_resolved_types(
        &mut self,
        resolver: &TypeResolver,
        current_scope: &[String],
    ) -> Option<Result<()>> {
        if self.data.r#type.is_none() {
            let slot = TypeSlot::Variable(current_scope.to_vec(), self.data.name.clone());
            if let Some(node) = resolver.nodes.get(&slot)
                && let Some(t) = &node.resolved
            {
                let mut resolved = *t;
                if !self.data.dimension_exprs.is_empty() {
                    resolved.dimensions = self.data.dimension_exprs.len();
                }
                self.data.r#type = Some(resolved);
            }
        }
        Some(Ok(()))
    }
}
impl Transpile for VarDeclStatement {
    fn transpile(
        &self,
        context: &mut TranspilationInputContext,
    ) -> Result<TranspilationOutput, Vec<Error>> {
        // Unused variables have no resolved type — skip them silently
        if self.data.r#type.is_none() {
            return Ok(TranspilationOutput::default());
        }

        let resolved_type = self.data.require_type().map_err(|e| {
            vec![e.context(format!(
                "...while transpiling variable declaration for '{}'",
                self.data.name
            ))]
        })?;

        // Insert the declaration. A previous entry whose scope is `Higher`
        // came from an enclosing function/procedure — shadowing it locally
        // is allowed, just like in most lexically-scoped languages. A
        // previous `Local` or `Arg` entry is a true re-declaration in this
        // very scope and is still rejected.
        let previous = context.variables.insert(
            self.data.name.clone(),
            ScopedVariableData {
                scope: VariableScope::Local,
                data: VariableData {
                    name: self.data.name.clone(),
                    r#type: resolved_type,
                },
            },
        );
        if let Some(prev) = previous
            && prev.scope != VariableScope::Higher
        {
            return Err(vec![anyhow!(
                "Variable '{}' is already defined in this scope!",
                self.data.name
            )]);
        }

        let data_output = self.data.transpile(context)?;
        let data_default_serialization = data_output.serialization;
        let requested_variables = data_output.requested_variables;

        let serialization = format!(
            "let mut {}: {} = {};",
            self.data.name,
            resolved_type.as_rust_type(),
            data_default_serialization
        );
        Ok(TranspilationOutput {
            serialization,
            requested_variables,
            ..Default::default()
        })
    }
}
