//! # Type Resolution
//!
//! Dependency-graph-based type inference pass that runs between AST
//! construction and transpilation. Fills in `Option<RosyType>` fields
//! left as `None` during parsing.
//!
//! ## Algorithm
//!s
//! 1. Walk the AST to discover all "type slots" (variables, function args,
//!    function return types, procedure args)
//! 2. Build a dependency graph between unresolved slots
//! 3. Topologically sort (Kahn's algorithm) and resolve from leaves inward
//! 4. Report cycles as errors

use crate::errors::RosyError;
use crate::program::Program;
use crate::program::expressions::*;
use crate::program::statements::*;
use crate::transpile::{
    ExprFunctionCallResult, InferenceEdgeResult, TypeHydrationResult, TypeslotDeclarationResult,
};
use anyhow::{Result, anyhow};
use rosy_lib::RosyType;
use std::collections::{HashMap, HashSet, VecDeque};

// ─── Type Slot ──────────────────────────────────────────────────────────────

/// A unique identifier for a type slot in the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeSlot {
    /// A variable declaration: (scope_path, variable_name)
    Variable(Vec<String>, String),
    /// A function return type: (scope_path, function_name)
    FunctionReturn(Vec<String>, String),
    /// A function/procedure argument: (scope_path, callable_name, arg_name)
    Argument(Vec<String>, String, String),
}
impl std::fmt::Display for TypeSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeSlot::Variable(scope, name) => {
                if scope.is_empty() {
                    write!(f, "variable '{}'", name)
                } else {
                    write!(f, "variable '{}' (in {})", name, scope.join(" > "))
                }
            }
            TypeSlot::FunctionReturn(scope, name) => {
                if scope.is_empty() {
                    write!(f, "return type of function '{}'", name)
                } else {
                    write!(
                        f,
                        "return type of function '{}' (in {})",
                        name,
                        scope.join(" > ")
                    )
                }
            }
            TypeSlot::Argument(scope, callable, arg) => {
                if scope.is_empty() {
                    write!(f, "argument '{}' of '{}'", arg, callable)
                } else {
                    write!(
                        f,
                        "argument '{}' of '{}' (in {})",
                        arg,
                        callable,
                        scope.join(" > ")
                    )
                }
            }
        }
    }
}

// ─── Resolution Rule ────────────────────────────────────────────────────────

/// Describes *how* to compute a slot's type once all its dependencies are resolved.
#[derive(Debug, Clone)]
pub enum ResolutionRule {
    /// The type is already known from an explicit annotation.
    Explicit(RosyType),
    /// Inferred from an assignment RHS or call-site argument expression.
    InferredFrom {
        recipe: ExprRecipe,
        /// Human-readable explanation of where this inference came from.
        reason: String,
    },
    /// Mirrors another slot exactly (e.g., return type from implicit return var).
    Mirror {
        source: TypeSlot,
        /// Human-readable explanation of why this slot mirrors another.
        reason: String,
    },
    /// No rule has been established yet — the slot is truly unknown.
    /// Will remain unresolved and trigger an error if not replaced.
    Unresolved,
}

// ─── Expression Recipe ──────────────────────────────────────────────────────

/// A lightweight "recipe" for computing the type of an expression.
/// Stores just enough info to re-derive the type once dependencies are resolved.
#[derive(Debug, Clone)]
pub enum ExprRecipe {
    /// A literal type — always known.
    Literal(RosyType),
    /// A variable reference — look up its slot.
    Variable(TypeSlot),
    /// An indexed variable reference — look up its slot and reduce dimensions.
    /// e.g., `A(R)(C)` on a 2D array has num_indices=2, reducing RE** to RE.
    IndexedVariable(TypeSlot, usize),
    /// A binary operator applied to two sub-recipes.
    BinaryOp {
        op: BinaryOpKind,
        left: Box<ExprRecipe>,
        right: Box<ExprRecipe>,
    },
    /// A binary concat of two sub-recipes.
    Concat(Box<ExprRecipe>, Box<ExprRecipe>),
    /// Any type-preserving intrinsic (sin, cos, tan, exp, log, sqrt, etc.) — output type equals input type.
    TypePreserving(Box<ExprRecipe>),
    /// REAL intrinsic — result depends on input type (RE/CM->RE, DA->DA).
    RealFn(Box<ExprRecipe>),
    /// IMAG intrinsic — result depends on input type (RE/CM->RE, DA->DA).
    ImagFn(Box<ExprRecipe>),
    /// Wraps a recipe and adds dimensions to the result type.
    /// Used when inferring a variable's type from an indexed assignment:
    /// e.g., `X[0, 1] := 2` means the RHS is RE, but X should be (RE 2D).
    WithDimensions(Box<ExprRecipe>, usize),
    /// Expression whose type couldn't be determined statically.
    /// The optional string provides context (e.g. undeclared variable name).
    Unknown(Option<String>),
}

impl ExprRecipe {
    /// Returns true if this recipe references the given type slot.
    pub fn references_slot(&self, target: &TypeSlot) -> bool {
        match self {
            ExprRecipe::Literal(_) | ExprRecipe::Unknown(_) => false,
            ExprRecipe::Variable(s) | ExprRecipe::IndexedVariable(s, _) => s == target,
            ExprRecipe::BinaryOp { left, right, .. } | ExprRecipe::Concat(left, right) => {
                left.references_slot(target) || right.references_slot(target)
            }
            ExprRecipe::TypePreserving(inner)
            | ExprRecipe::RealFn(inner)
            | ExprRecipe::ImagFn(inner)
            | ExprRecipe::WithDimensions(inner, _) => inner.references_slot(target),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mult,
    Div,
    Extract,
    Derive,
    Pow,
}

impl From<BinaryOpKind> for rosy_lib::BinaryOp {
    fn from(op: BinaryOpKind) -> Self {
        match op {
            BinaryOpKind::Add => Self::Add,
            BinaryOpKind::Sub => Self::Sub,
            BinaryOpKind::Mult => Self::Mult,
            BinaryOpKind::Div => Self::Div,
            BinaryOpKind::Extract => Self::Extract,
            BinaryOpKind::Derive => Self::Derive,
            BinaryOpKind::Pow => Self::Pow,
        }
    }
}

// ─── Dependency Graph Node ──────────────────────────────────────────────────

#[derive(Debug)]
pub struct GraphNode {
    pub slot: TypeSlot,
    /// How to compute this slot's type once dependencies are met.
    pub rule: ResolutionRule,
    /// Slots that this node depends on (must be resolved first).
    pub depends_on: HashSet<TypeSlot>,
    /// The resolved type (filled in during topological traversal).
    pub resolved: Option<RosyType>,
    /// Where this slot was declared (VARIABLE statement source location).
    pub declared_at: Option<SourceLocation>,
    /// Where the assignment that established the type inference rule is.
    pub assigned_at: Option<SourceLocation>,
}

// ─── Scope Context (used during graph construction) ─────────────────────────

/// Tracks what's been declared so far in a scope during the discovery walk.
#[derive(Debug, Clone, Default)]
pub struct ScopeContext {
    pub scope_path: Vec<String>,
    /// Maps variable name → its TypeSlot.
    pub variables: HashMap<String, TypeSlot>,
    /// Maps function name → (return_type_slot, vec of (arg_name, arg_slot)).
    pub functions: HashMap<String, (TypeSlot, Vec<(String, TypeSlot)>)>,
    /// Maps procedure name → vec of (arg_name, arg_slot).
    pub procedures: HashMap<String, Vec<(String, TypeSlot)>>,
}

// ─── Type Resolver ──────────────────────────────────────────────────────────

pub struct TypeResolver {
    /// All nodes in the dependency graph, keyed by their slot.
    pub nodes: HashMap<TypeSlot, GraphNode>,
}

impl TypeResolver {
    pub fn new() -> Self {
        TypeResolver {
            nodes: HashMap::new(),
        }
    }

    // ─── Public entry point ─────────────────────────────────────────────

    /// Run type resolution on a program. Mutates the AST in place.
    /// Returns the resolver (with all resolved graph nodes) and a list of
    /// warning messages (e.g. unused variables). The caller can inspect
    /// `resolver.nodes` for resolved types, declaration locations, etc.
    pub fn resolve(program: &mut Program) -> Result<(TypeResolver, Vec<RosyError>)> {
        let mut resolver = TypeResolver::new();
        let mut ctx = ScopeContext::default();

        // Phase 1: Walk AST, discover all slots and build dependency graph
        resolver.discover_slots(&program.statements, &mut ctx)?;

        // Phase 2: Topological sort + resolve
        let warnings = resolver.topological_resolve()?;

        // Phase 3: Apply resolved types back to the AST
        resolver.apply_to_ast(&mut program.statements, &[])?;

        Ok((resolver, warnings))
    }

    // ─── Graph Infrastructure ───────────────────────────────────────────

    /// Insert a node for a slot. If it has an explicit type, mark it resolved.
    pub fn insert_slot(
        &mut self,
        slot: TypeSlot,
        explicit_type: Option<&RosyType>,
        declared_at: Option<SourceLocation>,
    ) {
        if let Some(t) = explicit_type {
            self.nodes.insert(
                slot.clone(),
                GraphNode {
                    slot,
                    rule: ResolutionRule::Explicit(t.clone()),
                    depends_on: HashSet::new(),
                    resolved: Some(t.clone()),
                    declared_at,
                    assigned_at: None,
                },
            );
        } else {
            // Placeholder — rule and deps will be set by discover_dependencies
            self.nodes.entry(slot.clone()).or_insert_with(|| GraphNode {
                slot,
                rule: ResolutionRule::Unresolved,
                depends_on: HashSet::new(),
                resolved: None,
                declared_at,
                assigned_at: None,
            });
        }
    }

    // ─── Phase 1: Discovery ─────────────────────────────────────────────

    /// Walk the AST, creating graph nodes for every type slot and recording
    /// their dependencies.
    pub fn discover_slots(
        &mut self,
        statements: &[Statement],
        ctx: &mut ScopeContext,
    ) -> Result<()> {
        // First pass: register all declarations so we know what exists
        for stmt in statements {
            self.register_typeslot_declaration(stmt, ctx)?;
        }

        // Second pass: discover dependencies from assignments and call sites
        for stmt in statements {
            self.discover_dependencies(stmt, ctx)?;
        }

        Ok(())
    }

    /// Register a declaration, creating graph nodes for its type slots.
    pub fn register_typeslot_declaration(
        &mut self,
        stmt: &Statement,
        ctx: &mut ScopeContext,
    ) -> Result<()> {
        let TypeslotDeclarationResult::VarFuncOrProcedureDecl { result } = stmt
            .inner
            .register_typeslot_declaration(self, ctx, stmt.source_location.clone())
        else {
            return Ok(()); // not a declaration, skip
        };

        result
    }

    /// Walk statements looking for assignments and call sites to establish dependencies.
    pub fn discover_dependencies(
        &mut self,
        stmt: &Statement,
        ctx: &mut ScopeContext,
    ) -> Result<()> {
        let InferenceEdgeResult::HasEdges { result } =
            stmt.inner
                .wire_inference_edges(self, ctx, stmt.source_location.clone())
        else {
            return Ok(());
        };

        result
    }

    /// Recursively walk an expression tree looking for function calls.
    /// For each one found, wire up call-site argument dependencies.
    pub fn discover_expr_function_calls(&mut self, expr: &Expr, ctx: &ScopeContext) -> Result<()> {
        match expr.inner.discover_expr_function_calls(self, ctx) {
            ExprFunctionCallResult::HasFunctionCalls { result } => result,
            ExprFunctionCallResult::NoFunctionCalls => Ok(()),
        }
    }

    /// For a call site like `F(X, Y)`, if `F` has untyped parameters, add
    /// dependencies from the parameter slots to the argument expressions.
    pub fn discover_call_site_deps(
        &mut self,
        name: &str,
        args: &[Expr],
        is_function: bool,
        ctx: &ScopeContext,
    ) -> Result<()> {
        let param_slots: Option<Vec<(String, TypeSlot)>> = if is_function {
            ctx.functions.get(name).map(|(_, params)| params.clone())
        } else {
            ctx.procedures.get(name).map(|params| params.clone())
        };

        if let Some(params) = param_slots {
            for (i, arg_expr) in args.iter().enumerate() {
                if let Some((_, param_slot)) = params.get(i) {
                    // Only update if the parameter slot is unresolved
                    if let Some(param_node) = self.nodes.get(param_slot) {
                        if param_node.resolved.is_some() {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    // Build recipe for the argument expression
                    let mut deps = HashSet::new();
                    let recipe = self.build_expr_recipe(arg_expr, ctx, &mut deps);

                    let node = self.nodes.get_mut(param_slot).unwrap();
                    node.rule = ResolutionRule::InferredFrom {
                        recipe,
                        reason: format!("inferred from argument {} at call site", i + 1),
                    };
                    node.depends_on = deps;
                }
            }
        }

        Ok(())
    }

    /// Build an ExprRecipe from an AST expression, collecting dependency slots.
    pub fn build_expr_recipe(
        &self,
        expr: &Expr,
        ctx: &ScopeContext,
        deps: &mut HashSet<TypeSlot>,
    ) -> ExprRecipe {
        expr.inner.build_expr_recipe(self, ctx, deps)
    }

    // ─── Phase 2: Topological Resolution ────────────────────────────────

    /// Process nodes whose dependencies are all resolved first, resolve them,
    /// then process their dependents, and so on. One pass — no iteration.
    /// Returns a list of warning messages for unused variables.
    pub fn topological_resolve(&mut self) -> Result<Vec<RosyError>> {
        // Build reverse dependency map: slot → set of slots that depend on it
        let mut dependents: HashMap<TypeSlot, Vec<TypeSlot>> = HashMap::new();
        let mut in_degree: HashMap<TypeSlot, usize> = HashMap::new();

        for (slot, node) in &self.nodes {
            // Only count edges to slots that exist in the graph
            let real_deps: usize = node
                .depends_on
                .iter()
                .filter(|d| self.nodes.contains_key(d))
                .count();
            in_degree.insert(slot.clone(), real_deps);

            for dep in &node.depends_on {
                if self.nodes.contains_key(dep) {
                    dependents
                        .entry(dep.clone())
                        .or_default()
                        .push(slot.clone());
                }
            }
        }

        // Seed the queue with all nodes that have in-degree 0
        let mut queue: VecDeque<TypeSlot> = VecDeque::new();
        for (slot, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(slot.clone());
            }
        }

        let mut resolved_count: usize = 0;
        let warnings: Vec<RosyError> = Vec::new();
        let mut warned_slots: HashSet<TypeSlot> = HashSet::new();
        while let Some(slot) = queue.pop_front() {
            // Resolve this node if not already resolved
            if self.nodes.get(&slot).map_or(true, |n| n.resolved.is_none()) {
                // Check if this is an unused variable (Unresolved rule, nothing depends on it)
                let is_unused = {
                    let node = self.nodes.get(&slot);
                    node.map_or(false, |n| {
                        matches!(n.rule, ResolutionRule::Unresolved)
                            && matches!(n.slot, TypeSlot::Variable(..) | TypeSlot::Argument(..))
                    })
                };

                if is_unused {
                    // Default unresolved variables and arguments to RE (standard COSY behavior).
                    // Fall through to the dependents-decrement block below so any node
                    // depending on this slot (e.g. a function body that references an
                    // uncalled function's argument) still progresses through the queue.
                    let node = self.nodes.get_mut(&slot).unwrap();
                    let default_type = RosyType::RE();
                    node.resolved = Some(default_type.clone());
                    node.rule = ResolutionRule::InferredFrom {
                        recipe: ExprRecipe::Literal(default_type),
                        reason: "untyped variables default to RE".to_string(),
                    };
                    warned_slots.insert(slot.clone());
                } else {
                    self.resolve_node(&slot)?;
                }
            }
            resolved_count += 1;

            // Decrement in-degree for all dependents
            if let Some(deps) = dependents.get(&slot) {
                for dep_slot in deps {
                    if let Some(deg) = in_degree.get_mut(dep_slot) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(dep_slot.clone());
                        }
                    }
                }
            }
        }

        // Any remaining unresolved nodes are cycles or truly unresolvable
        // (exclude slots that were already warned about as unused variables)
        let unresolved: Vec<&GraphNode> = self
            .nodes
            .values()
            .filter(|n| n.resolved.is_none() && !warned_slots.contains(&n.slot))
            .collect();

        if unresolved.is_empty() {
            tracing::debug!(
                "Type resolution complete: resolved {} slot{} successfully",
                resolved_count,
                if resolved_count == 1 { "" } else { "s" }
            );
            return Ok(warnings);
        }

        self.build_resolution_error(&unresolved)
    }

    /// Build a detailed error message for unresolved type slots.
    fn build_resolution_error(&self, unresolved: &[&GraphNode]) -> Result<Vec<RosyError>> {
        // Partition into cycle nodes (have unresolved deps) vs no-info nodes
        let mut cycle_slots: Vec<&TypeSlot> = unresolved
            .iter()
            .filter(|n| {
                n.depends_on
                    .iter()
                    .any(|d| self.nodes.get(d).map_or(false, |dn| dn.resolved.is_none()))
            })
            .map(|n| &n.slot)
            .collect();

        let mut no_info_slots: Vec<&TypeSlot> = unresolved
            .iter()
            .filter(|n| {
                !n.depends_on
                    .iter()
                    .any(|d| self.nodes.get(d).map_or(false, |dn| dn.resolved.is_none()))
            })
            .map(|n| &n.slot)
            .collect();

        // Source-order sorter: prefer `declared_at`, fall back to `assigned_at`
        // (variables introduced by `:=` only carry `assigned_at`). Slots with no
        // location go last, in slot-name order so output stays deterministic.
        let order_key = |slot: &&TypeSlot| -> Option<(usize, usize)> {
            let node = self.nodes.get(*slot)?;
            let loc = node.declared_at.as_ref().or(node.assigned_at.as_ref())?;
            Some((loc.line, loc.col))
        };
        let by_source = |a: &&TypeSlot, b: &&TypeSlot| match (order_key(a), order_key(b)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => format!("{}", a).cmp(&format!("{}", b)),
        };
        cycle_slots.sort_by(by_source);
        no_info_slots.sort_by(by_source);

        let total = unresolved.len();
        let mut msg = format!(
            "\n╭─ Type Resolution Failed ─────────────────────────────────\n│\n│  {} unresolved type{} found:\n│",
            total,
            if total == 1 { "" } else { "s" }
        );

        // Report cycle errors
        if !cycle_slots.is_empty() {
            msg.push_str("\n│  🔄 Circular dependencies detected:");
            msg.push_str("\n│");
            for slot in &cycle_slots {
                let node = self.nodes.get(slot).unwrap();
                let dep_names: Vec<String> = node
                    .depends_on
                    .iter()
                    .filter(|d| self.nodes.get(*d).map_or(false, |n| n.resolved.is_none()))
                    .map(|d| format!("{}", d))
                    .collect();
                msg.push_str(&format!("\n│    ✗ {} depends on:", slot,));
                for dep in &dep_names {
                    msg.push_str(&format!("\n│        → {}", dep));
                }
                // Include source locations if available
                if let Some(loc) = &node.declared_at {
                    msg.push_str(&format!("\n│        📍 Declared at: {}", loc));
                }
                if let Some(loc) = &node.assigned_at {
                    msg.push_str(&format!("\n│        📍 Assigned at: {}", loc));
                }
                // Include the resolution rule reason if available
                if let Some(reason) = Self::rule_reason(&node.rule) {
                    msg.push_str(&format!("\n│        ({})", reason));
                }
            }
            msg.push_str("\n│");
            msg.push_str("\n│    Break the cycle by adding an explicit type annotation");
            msg.push_str("\n│    to at least one of the slots above.");
            msg.push_str("\n│");
        }

        // Report no-info errors
        for slot in &no_info_slots {
            let node = self.nodes.get(slot).unwrap();
            let reason_hint = Self::rule_reason(&node.rule)
                .map(|r| format!("\n\x20   • Attempted: {}", r))
                .unwrap_or_default();
            let hint = match slot {
                TypeSlot::Variable(scope, name) => {
                    let scope_str = if scope.is_empty() {
                        "global scope".to_string()
                    } else {
                        format!("'{}'", scope.join(" > "))
                    };
                    let decl_hint = node
                        .declared_at
                        .as_ref()
                        .map(|loc| format!("\n\x20   • Declared at: {}", loc))
                        .unwrap_or_default();
                    format!(
                        "  ✗ Could not determine the type of variable '{}' (in {})\n\
                         \x20   • It is declared but never assigned a value with a known type.{}{}\n\
                         \x20   • Try assigning it a value (e.g. {} := 0;) or adding an explicit type.\n\
                         \x20   → Add an explicit type: VARIABLE (RE) {} ;",
                        name, scope_str, decl_hint, reason_hint, name, name
                    )
                }
                TypeSlot::FunctionReturn(_, name) => {
                    format!(
                        "  ✗ Could not determine the return type of function '{}'\n\
                         \x20   • The function body doesn't assign a known-type value to '{}'.{}\n\
                         \x20   → Add an explicit return type: FUNCTION (RE) {} ... ;",
                        name, name, reason_hint, name
                    )
                }
                TypeSlot::Argument(_, callable, arg) => {
                    format!(
                        "  ✗ Could not determine the type of argument '{}' of '{}'\n\
                         \x20   • No call site passes a value with a known type for this argument.{}\n\
                         \x20   → Add an explicit type: {} (RE)",
                        arg, callable, reason_hint, arg
                    )
                }
            };
            for line in hint.lines() {
                msg.push_str(&format!("\n│  {}", line));
            }
            msg.push_str("\n│");
        }

        msg.push_str("\n│  The type resolver builds a dependency graph and resolves");
        msg.push_str("\n│  types from leaves inward. If a slot has no path to a");
        msg.push_str("\n│  known type, or is part of a cycle, it cannot be resolved.");
        msg.push_str("\n│");
        msg.push_str("\n╰──────────────────────────────────────────────────────────");
        // Use the location of the first unresolvable slot for diagnostic placement
        let first_loc = no_info_slots
            .iter()
            .chain(cycle_slots.iter())
            .filter_map(|s| self.nodes.get(s)?.declared_at.clone())
            .next();
        Err(RosyError {
            message: msg,
            location: first_loc,
            severity: crate::errors::RosyErrorSeverity::Error,
        }
        .into())
    }

    /// Extract the human-readable reason from a resolution rule, if available.
    fn rule_reason(rule: &ResolutionRule) -> Option<&str> {
        match rule {
            ResolutionRule::InferredFrom { reason, .. } => Some(reason.as_str()),
            ResolutionRule::Mirror { reason, .. } => Some(reason.as_str()),
            _ => None,
        }
    }

    /// Resolve a single node by evaluating its rule.
    fn resolve_node(&mut self, slot: &TypeSlot) -> Result<()> {
        let node = self
            .nodes
            .get(slot)
            .ok_or_else(|| anyhow!("No node for slot {}", slot))?;

        if node.resolved.is_some() {
            return Ok(());
        }

        let rule = node.rule.clone();
        let declared_at = node.declared_at.clone();
        let assigned_at = node.assigned_at.clone();
        let resolved_type = match rule {
            ResolutionRule::Explicit(t) => t,
            ResolutionRule::InferredFrom { recipe, reason } => {
                let self_referential = recipe.references_slot(slot);
                self.evaluate_recipe(&recipe).map_err(|e| {
                    let hint = if self_referential {
                        "\n\n\thint: this variable's type depends on itself — use an explicit type cast (e.g. VE(8))"
                    } else {
                        ""
                    };
                    let msg = format!(
                        "while resolving {}: {}\n\t({}){hint}",
                        slot, e, reason
                    );
                    // Prefer assigned_at (the source of inference), fall back to declared_at
                    let loc = assigned_at.clone().or_else(|| declared_at.clone());
                    anyhow::Error::from(RosyError {
                        message: msg,
                        location: loc,
                        severity: crate::errors::RosyErrorSeverity::Error,
                    })
                })?
            }
            ResolutionRule::Mirror { source, .. } => self
                .nodes
                .get(&source)
                .and_then(|n| n.resolved.clone())
                .ok_or_else(|| {
                    anyhow!(
                        "Mirror source {} not resolved when resolving {}",
                        source,
                        slot
                    )
                })?,
            ResolutionRule::Unresolved => {
                let msg = format!(
                    "No type could be determined for {}\n  💡 Add an explicit type annotation or assign a value with a known type.",
                    slot
                );
                return Err(RosyError {
                    message: msg,
                    location: node.declared_at.clone(),
                    severity: crate::errors::RosyErrorSeverity::Error,
                }
                .into());
            }
        };

        self.nodes.get_mut(slot).unwrap().resolved = Some(resolved_type);
        Ok(())
    }

    /// Evaluate an ExprRecipe using already-resolved slot types.
    pub fn evaluate_recipe(&self, recipe: &ExprRecipe) -> Result<RosyType> {
        match recipe {
            ExprRecipe::Literal(t) => Ok(t.clone()),
            ExprRecipe::Variable(slot) => self
                .nodes
                .get(slot)
                .and_then(|n| n.resolved.clone())
                .ok_or_else(|| anyhow!("Variable slot {} not resolved", slot)),
            ExprRecipe::IndexedVariable(slot, num_indices) => {
                let base = self
                    .nodes
                    .get(slot)
                    .and_then(|n| n.resolved.clone())
                    .ok_or_else(|| anyhow!("Variable slot {} not resolved", slot))?;
                // Cascade: peel min(indices, dimensions), then if remaining
                // index applies to a (VE) (dim=0 VE base), it extracts to RE.
                // Mirrors VariableIdentifier::type_of's cascade exactly.
                let mut remaining = *num_indices;
                let dim_peel = remaining.min(base.dimensions);
                let new_dim = base.dimensions - dim_peel;
                remaining -= dim_peel;
                if remaining == 1 && new_dim == 0 && base.base_type == rosy_lib::RosyBaseType::VE {
                    return Ok(RosyType::RE());
                }
                if remaining > 0 {
                    return Err(anyhow!(
                        "IndexedVariable: too many indices ({} for {} dims)",
                        num_indices,
                        base.dimensions
                    ));
                }
                Ok(RosyType::new(base.base_type, new_dim))
            }
            ExprRecipe::WithDimensions(inner, extra_dims) => {
                let mut t = self.evaluate_recipe(inner)?;
                t.dimensions += extra_dims;
                Ok(t)
            }
            ExprRecipe::BinaryOp { op, left, right } => {
                let left_type = self.evaluate_recipe(left)?;
                let right_type = self.evaluate_recipe(right)?;
                let result = rosy_lib::BinaryOp::from(*op).return_type(&left_type, &right_type);
                result.ok_or_else(|| {
                    anyhow!(
                        "No operator rule for {:?}({}, {})",
                        op,
                        left_type,
                        right_type
                    )
                })
            }
            ExprRecipe::Concat(left, right) => {
                let left_type = self.evaluate_recipe(left)?;
                let right_type = self.evaluate_recipe(right)?;
                rosy_lib::BinaryOp::Concat
                    .return_type(&left_type, &right_type)
                    .ok_or_else(|| anyhow!("No concat rule for {} & {}", left_type, right_type))
            }
            ExprRecipe::TypePreserving(inner) => self.evaluate_recipe(inner),
            ExprRecipe::RealFn(inner) => {
                let input_type = self.evaluate_recipe(inner)?;
                rosy_lib::unary_return_type("REAL", &input_type)
                    .ok_or_else(|| anyhow!("No REAL rule for {}", input_type))
            }
            ExprRecipe::ImagFn(inner) => {
                let input_type = self.evaluate_recipe(inner)?;
                rosy_lib::unary_return_type("IMAG", &input_type)
                    .ok_or_else(|| anyhow!("No IMAG rule for {}", input_type))
            }
            ExprRecipe::Unknown(reason) => {
                let detail = reason
                    .as_deref()
                    .unwrap_or("expression type could not be determined statically");
                Err(anyhow!("{}", detail))
            }
        }
    }

    // ─── Phase 3: Apply Resolved Types ──────────────────────────────────

    /// Walk the AST and fill in all `None` type fields with resolved types.
    pub fn apply_to_ast(
        &self,
        statements: &mut [Statement],
        current_scope: &[String],
    ) -> Result<()> {
        for stmt in statements.iter_mut() {
            let TypeHydrationResult::Hydrated { result } =
                stmt.inner.hydrate_resolved_types(self, current_scope)
            else {
                continue;
            };
            result?;
        }

        Ok(())
    }
}
