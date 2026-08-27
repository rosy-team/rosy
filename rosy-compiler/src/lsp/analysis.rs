//! Bridges the rosy transpiler's parser and type resolver into LSP-friendly data.
//!
//! Runs the real rosy pipeline (parse → AST → type resolution → transpilation)
//! on a document and extracts diagnostics, resolved types, and symbol locations.

use crate::{
    ast::Rule,
    errors::RosyError,
    program::Program,
    resolve::{GraphNode, TypeResolver, TypeSlot},
    transpile::{TranspilationInputContext, Transpile},
};
use tower_lsp::lsp_types::*;

/// Result of analyzing a single Rosy document.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    /// Parse and type resolution errors as LSP diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Resolved variable types, keyed by (line, col) of declaration.
    /// Value is the human-readable type string (e.g. "RE", "VE", "DA").
    pub variable_types: Vec<InlayHintData>,
    /// Semantic tokens for syntax highlighting via the LSP.
    pub semantic_tokens: Vec<SemanticTokenData>,
}

/// Data for a single semantic token.
#[derive(Debug)]
pub struct SemanticTokenData {
    pub line: u32,
    pub start_col: u32,
    pub length: u32,
    pub token_type: SemanticTokenType,
}

/// The token types we report to the editor.
/// The index in LEGEND_TOKEN_TYPES must match what we register in capabilities.
#[derive(Debug, Clone, Copy)]
pub enum SemanticTokenType {
    Keyword,
    Function,
    Type,
    Variable,
    Number,
    String,
    Comment,
}

impl SemanticTokenType {
    pub fn index(self) -> u32 {
        match self {
            SemanticTokenType::Keyword => 0,
            SemanticTokenType::Function => 1,
            SemanticTokenType::Type => 2,
            SemanticTokenType::Variable => 3,
            SemanticTokenType::Number => 4,
            SemanticTokenType::String => 5,
            SemanticTokenType::Comment => 6,
        }
    }
}

/// The legend registered with the client. Order must match SemanticTokenType::index().
pub const LEGEND_TOKEN_TYPES: &[tower_lsp::lsp_types::SemanticTokenType] = &[
    tower_lsp::lsp_types::SemanticTokenType::KEYWORD,
    tower_lsp::lsp_types::SemanticTokenType::FUNCTION,
    tower_lsp::lsp_types::SemanticTokenType::TYPE,
    tower_lsp::lsp_types::SemanticTokenType::VARIABLE,
    tower_lsp::lsp_types::SemanticTokenType::NUMBER,
    tower_lsp::lsp_types::SemanticTokenType::STRING,
    tower_lsp::lsp_types::SemanticTokenType::COMMENT,
];

/// Data for a single inlay hint.
#[derive(Debug)]
pub struct InlayHintData {
    /// Position right after the variable name in the declaration.
    pub position: Position,
    /// The resolved type label (e.g. "(RE)", "(CM)", "(DA 2D)").
    pub label: String,
    /// Where the type was inferred from (assignment RHS, function call, etc.)
    /// If present, the inlay hint label part becomes clickable, navigating here.
    pub inferred_from: Option<InferredFromLocation>,
}

/// Where a type was inferred from — used for clickable inlay hint labels.
#[derive(Debug)]
pub struct InferredFromLocation {
    /// LSP position of the source of inference (e.g. the assignment RHS).
    pub line: u32,
    pub col: u32,
    /// Human-readable description of how the type was determined.
    pub reason: String,
}

/// Extract an LSP Position from an anyhow error by downcasting to RosyError.
///
/// Walks the error chain looking for a RosyError with a SourceLocation.
/// Returns a 0-based LSP Position if found, otherwise None.
/// Extract the clean error message from the innermost RosyError in the chain.
fn extract_message_from_anyhow(error: &anyhow::Error) -> String {
    let mut best_msg = None;
    for cause in error.chain() {
        if let Some(rosy_err) = cause.downcast_ref::<RosyError>() {
            best_msg = Some(rosy_err.message.clone());
        }
    }
    best_msg.unwrap_or_else(|| format!("{}", error.root_cause()))
}

fn extract_location_from_anyhow(error: &anyhow::Error) -> Option<Position> {
    // Walk the error chain looking for the most specific (innermost) RosyError
    // with a source location. Inner errors are more precise than outer wrappers.
    let mut best = None;
    for cause in error.chain() {
        if let Some(rosy_err) = cause.downcast_ref::<RosyError>()
            && let Some(loc) = &rosy_err.location
        {
            best = Some(Position::new(
                loc.line.saturating_sub(1) as u32,
                loc.col.saturating_sub(1) as u32,
            ));
        }
    }
    best
}

/// Analyze a Rosy source document, returning diagnostics and type information.
///
/// `source_path` is used to resolve INCLUDE directives. Pass `None` for
/// unsaved buffers (INCLUDEs with relative paths will produce diagnostics).
pub fn analyze(source: &str, source_path: Option<&std::path::Path>) -> AnalysisResult {
    let mut result = AnalysisResult {
        semantic_tokens: tokenize_source(source),
        ..Default::default()
    };

    crate::syntax_config::apply_from_path(source_path);
    // Step 1: Parse
    let pairs = match crate::ast::parse_source(source) {
        Ok(pairs) => pairs,
        Err(e) => {
            result.diagnostics.push(pest_error_to_diagnostic(&e));
            return result;
        }
    };

    let program_pair = match pairs.into_iter().next() {
        Some(p) => p,
        None => {
            result.diagnostics.push(Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                severity: Some(DiagnosticSeverity::ERROR),
                message: "Expected a program".to_string(),
                source: Some("rosy".to_string()),
                ..Default::default()
            });
            return result;
        }
    };

    // Step 2: Build AST (resolves INCLUDEs at the AST level)
    let mut ast = match Program::from_rule_with_includes(
        program_pair,
        source_path,
        &mut crate::program::IncludeTracker::default(),
    ) {
        Ok(Some(ast)) => ast,
        Ok(None) => {
            result.diagnostics.push(Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                severity: Some(DiagnosticSeverity::ERROR),
                message: "Failed to build AST: empty program".to_string(),
                source: Some("rosy".to_string()),
                ..Default::default()
            });
            return result;
        }
        Err(e) => {
            let position = extract_location_from_anyhow(&e).unwrap_or(Position::new(0, 0));
            result.diagnostics.push(Diagnostic {
                range: Range::new(position, position),
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("AST construction failed: {e}"),
                source: Some("rosy".to_string()),
                ..Default::default()
            });
            return result;
        }
    };

    // Step 3: Type Resolution
    // The resolver is returned so we can inspect resolved nodes for inlay hints.
    let resolver = match TypeResolver::resolve(&mut ast) {
        Ok((resolver, warnings)) => {
            for w in warnings {
                let position = w
                    .location
                    .as_ref()
                    .map(|loc| {
                        Position::new(
                            loc.line.saturating_sub(1) as u32,
                            loc.col.saturating_sub(1) as u32,
                        )
                    })
                    .unwrap_or(Position::new(0, 0));
                result.diagnostics.push(Diagnostic {
                    range: Range::new(position, position),
                    severity: Some(DiagnosticSeverity::WARNING),
                    message: w.message,
                    source: Some("rosy".to_string()),
                    ..Default::default()
                });
            }
            Some(resolver)
        }
        Err(e) => {
            let position = extract_location_from_anyhow(&e).unwrap_or(Position::new(0, 0));
            result.diagnostics.push(Diagnostic {
                range: Range::new(position, position),
                severity: Some(DiagnosticSeverity::ERROR),
                message: format!("Type resolution failed: {e}"),
                source: Some("rosy".to_string()),
                ..Default::default()
            });
            None
        }
    };

    // Step 4: Extract resolved types for inlay hints from the resolver's graph nodes.
    if let Some(resolver) = resolver {
        for node in resolver.nodes.values() {
            extract_inlay_hint(node, source_path, &mut result.variable_types);
        }
    }

    // Step 5: Transpilation — catches type mismatches, invalid operations,
    // and other errors that only surface when generating Rust code.
    match ast.transpile(&mut TranspilationInputContext::default()) {
        Ok(_) => {}
        Err(errors) => {
            for err in &errors {
                let position = extract_location_from_anyhow(err).unwrap_or(Position::new(0, 0));
                // Extract the clean message from the innermost RosyError,
                // falling back to root_cause Display if no RosyError found.
                let message = extract_message_from_anyhow(err);
                result.diagnostics.push(Diagnostic {
                    range: Range::new(position, position),
                    severity: Some(DiagnosticSeverity::ERROR),
                    message,
                    source: Some("rosy".to_string()),
                    ..Default::default()
                });
            }
        }
    }

    result
}

/// True when this location belongs on the document we analyzed.
///
/// INCLUDE / MODULE splice other files into one program. those nodes have
/// `file` set to the included path. local nodes use the open path, or `None`
/// for unsaved buffers. never paint a foreign file's line/col onto this buffer.
fn location_belongs_to_document(
    loc: &crate::program::statements::SourceLocation,
    source_path: Option<&std::path::Path>,
) -> bool {
    match (&loc.file, source_path) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(node_file), Some(current)) => paths_refer_to_same_file(node_file, current),
    }
}

fn paths_refer_to_same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Extract an inlay hint from a resolved graph node, if applicable.
fn extract_inlay_hint(
    node: &GraphNode,
    source_path: Option<&std::path::Path>,
    hints: &mut Vec<InlayHintData>,
) {
    // Don't show hints for explicitly annotated types — the user already wrote them
    if matches!(node.rule, crate::resolve::ResolutionRule::Explicit(_)) {
        return;
    }

    let Some(resolved_type) = &node.resolved else {
        return;
    };

    let Some(declared_at) = &node.declared_at else {
        return;
    };

    if !location_belongs_to_document(declared_at, source_path) {
        return;
    }

    let snippet = &declared_at.snippet;
    let snippet_upper = snippet.to_uppercase();

    // Determine what kind of slot this is, extract name, and compute position
    let hint_col = match &node.slot {
        TypeSlot::Variable(_, _name) => {
            // Skip the implicit return variable inside function bodies —
            // it duplicates the FunctionReturn slot's hint.
            if snippet_upper.starts_with("FUNCTION") {
                return;
            }
            // VARIABLE _here_ name — place after VARIABLE keyword
            declared_at.col + "VARIABLE".len()
        }
        TypeSlot::FunctionReturn(_, _) => {
            // FUNCTION _here_ NAME — place right after FUNCTION keyword
            declared_at.col + "FUNCTION".len()
        }
        TypeSlot::Argument(_, _, name) => {
            // FUNCTION (RE) NAME arg1 _here_ — place after the arg name
            if let Some(offset) = snippet_upper.find(&name.to_uppercase()) {
                declared_at.col + offset + name.len()
            } else {
                return;
            }
        }
    };

    // Build inference info with a source location so the user can jump to
    // exactly where the type was determined.
    let (reason, loc) = match &node.rule {
        crate::resolve::ResolutionRule::Explicit(_) => (None, None),
        crate::resolve::ResolutionRule::InferredFrom { reason, .. } => (
            Some(reason.clone()),
            node.assigned_at.as_ref().or(node.declared_at.as_ref()),
        ),
        crate::resolve::ResolutionRule::Mirror { reason, .. } => (
            Some(reason.clone()),
            node.assigned_at.as_ref().or(node.declared_at.as_ref()),
        ),
        crate::resolve::ResolutionRule::Unresolved => (
            Some("could not be inferred".to_string()),
            node.declared_at.as_ref(),
        ),
    };

    let inferred_from = match (reason, loc) {
        (Some(reason), Some(loc)) => Some(InferredFromLocation {
            line: loc.line.saturating_sub(1) as u32,
            col: loc.col.saturating_sub(1) as u32,
            reason: format!("{} (line {}, col {})", reason, loc.line, loc.col),
        }),
        (Some(reason), None) => Some(InferredFromLocation {
            line: declared_at.line.saturating_sub(1) as u32,
            col: declared_at.col.saturating_sub(1) as u32,
            reason,
        }),
        _ => None,
    };

    hints.push(InlayHintData {
        // SourceLocation uses 1-based line/col, LSP uses 0-based
        position: Position::new(
            declared_at.line.saturating_sub(1) as u32,
            hint_col.saturating_sub(1) as u32,
        ),
        label: format!("{resolved_type}"),
        inferred_from,
    });
}

// ─── Semantic Tokenization ──────────────────────────────────────────────────

/// Tokenize Rosy source text for semantic highlighting.
/// Scans the source directly (not the AST) so it works even on broken files.
/// Uses the auto-generated ROSY_KEYWORD_LIST to recognize keywords.
fn tokenize_source(source: &str) -> Vec<SemanticTokenData> {
    let mut tokens = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx as u32;
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            let b = bytes[i];

            // Skip whitespace
            if b.is_ascii_whitespace() {
                i += 1;
                continue;
            }

            // Comments: { ... } with nesting
            if b == b'{' {
                let start = i;
                let mut depth = 1;
                i += 1;
                // Comments can span lines but we only handle single-line here.
                // Multi-line comments will get the first line highlighted.
                while i < len && depth > 0 {
                    if bytes[i] == b'{' {
                        depth += 1;
                    } else if bytes[i] == b'}' {
                        depth -= 1;
                    }
                    i += 1;
                }
                tokens.push(SemanticTokenData {
                    line: line_num,
                    start_col: start as u32,
                    length: (i - start) as u32,
                    token_type: SemanticTokenType::Comment,
                });
                continue;
            }

            // Strings: '...' or "..."
            if b == b'\'' || b == b'"' {
                let quote = b;
                let start = i;
                i += 1;
                while i < len {
                    if bytes[i] == quote {
                        // Handle '' escape in single-quoted strings
                        if quote == b'\'' && i + 1 < len && bytes[i + 1] == b'\'' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                tokens.push(SemanticTokenData {
                    line: line_num,
                    start_col: start as u32,
                    length: (i - start) as u32,
                    token_type: SemanticTokenType::String,
                });
                continue;
            }

            // Numbers
            if b.is_ascii_digit() || (b == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
                let start = i;
                if b == b'-' {
                    i += 1;
                }
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i < len && bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                    i += 1;
                    while i < len && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                // Scientific notation
                if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
                    i += 1;
                    if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                        i += 1;
                    }
                    while i < len && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                tokens.push(SemanticTokenData {
                    line: line_num,
                    start_col: start as u32,
                    length: (i - start) as u32,
                    token_type: SemanticTokenType::Number,
                });
                continue;
            }

            // Identifiers / keywords
            if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];
                let upper = word.to_uppercase();

                // Check if it's a type annotation
                let token_type = if matches!(
                    upper.as_str(),
                    "RE" | "ST" | "LO" | "CM" | "VE" | "DA" | "CD"
                ) {
                    // If followed by `(`, it's a function call; otherwise it's a type
                    let rest = &line[i..].trim_start();
                    if rest.starts_with('(') {
                        SemanticTokenType::Function
                    } else {
                        SemanticTokenType::Type
                    }
                } else if upper == "TRUE" || upper == "FALSE" {
                    SemanticTokenType::Keyword
                } else if INTRINSIC_FUNCTIONS.contains(&upper.as_str()) {
                    SemanticTokenType::Function
                } else if ROSY_KEYWORD_LIST.iter().any(|(kw, _)| *kw == upper) {
                    SemanticTokenType::Keyword
                } else {
                    SemanticTokenType::Variable
                };

                tokens.push(SemanticTokenData {
                    line: line_num,
                    start_col: start as u32,
                    length: (i - start) as u32,
                    token_type,
                });
                continue;
            }

            // Skip operators and punctuation (not semantically highlighted)
            i += 1;
        }
    }

    tokens
}

/// Convert a pest parse error into an LSP diagnostic.
fn pest_error_to_diagnostic(error: &pest::error::Error<Rule>) -> Diagnostic {
    let (line, col): (usize, usize) = match error.line_col {
        pest::error::LineColLocation::Pos((line, col)) => (line, col),
        pest::error::LineColLocation::Span((line, col), _) => (line, col),
    };

    Diagnostic {
        range: Range::new(
            Position::new(line.saturating_sub(1) as u32, col.saturating_sub(1) as u32),
            Position::new(line.saturating_sub(1) as u32, col as u32),
        ),
        severity: Some(DiagnosticSeverity::ERROR),
        message: format!("{error}"),
        source: Some("rosy".to_string()),
        ..Default::default()
    }
}

// Include generated data from rosy.pest + module docs at build time.
include!(concat!(env!("OUT_DIR"), "/keywords_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/hover_generated.rs"));

/// Intrinsic functions — these get `FUNC($0)` snippet insertion.
/// Everything else in the keyword list gets plain keyword completion.
const INTRINSIC_FUNCTIONS: &[&str] = &[
    "ABS", "ACOS", "ASIN", "ATAN", "CD", "CM", "CMPLX", "CONJ", "CONS", "COS", "COSH", "DA", "ERF",
    "EXP", "IMAG", "INT", "ISRT", "ISRT3", "LCD", "LCM", "LDA", "LENGTH", "LLO", "LO", "LOG",
    "LRE", "LST", "LTRIM", "LVE", "NINT", "NORM", "RE", "REAL", "SIN", "SINH", "SQR", "SQRT", "ST",
    "TAN", "TANH", "TRIM", "TYPE", "VARMEM", "VARPOI", "VE", "VMAX", "VMIN", "WERF",
];

/// Build completion items from the auto-generated keyword list.
/// Keywords are extracted from `keyword_raw` in rosy.pest at compile time,
/// so adding a new construct to the grammar automatically updates completions.
pub fn rosy_keywords() -> Vec<CompletionItem> {
    let base_url = "https://rosy-team.github.io/rosy/rosy_compiler";

    let mut items: Vec<CompletionItem> = ROSY_KEYWORD_LIST
        .iter()
        .map(|(label, detail)| {
            let is_function = INTRINSIC_FUNCTIONS.contains(label);

            CompletionItem {
                label: label.to_string(),
                kind: Some(if is_function {
                    CompletionItemKind::FUNCTION
                } else {
                    CompletionItemKind::KEYWORD
                }),
                detail: Some(detail.to_string()),
                insert_text: if is_function {
                    Some(format!("{label}($0)"))
                } else {
                    None
                },
                insert_text_format: if is_function {
                    Some(InsertTextFormat::SNIPPET)
                } else {
                    None
                },
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("{detail}\n\n[Documentation]({base_url}/)"),
                })),
                ..Default::default()
            }
        })
        .collect();

    // Boolean constants (not in keyword_raw since they're expression-level)
    for (label, detail) in [("TRUE", "Boolean true"), ("FALSE", "Boolean false")] {
        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some(detail.to_string()),
            ..Default::default()
        });
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlay_hints_skip_explicit_types() {
        let source = "BEGIN;\n    FUNCTION (RE) COMPUTE x (RE) y (RE);\n        VARIABLE temp;\n        temp := x * y;\n        COMPUTE := temp + 10;\n    ENDFUNCTION;\n    PROCEDURE RUN;\n        VARIABLE is_true;\n        VARIABLE (LO) is_false;\n        is_true := TRUE;\n    ENDPROCEDURE;\n    RUN;\nEND;";
        let result = analyze(source, None);
        eprintln!("Hints:");
        for h in &result.variable_types {
            eprintln!(
                "  line={} col={} label={:?}",
                h.position.line, h.position.character, h.label
            );
        }
        let labels: Vec<&str> = result
            .variable_types
            .iter()
            .map(|h| h.label.as_str())
            .collect();
        // temp (inferred RE) and is_true (inferred LO) should have hints
        assert!(labels.contains(&"(RE)"), "temp should get an (RE) hint");
        assert!(labels.contains(&"(LO)"), "is_true should get an (LO) hint");
        // Explicitly typed variables should NOT have hints
        assert_eq!(
            result.variable_types.len(),
            2,
            "Only inferred types should produce hints, got: {:?}",
            labels
        );
    }

    #[test]
    fn inlay_hints_skip_included_file_slots() {
        let dir = std::env::temp_dir().join(format!(
            "rosy-inlay-include-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let child = dir.join("child.rosy");
        let parent = dir.join("parent.rosy");
        std::fs::write(
            &child,
            "BEGIN;\n    VARIABLE from_child;\n    from_child := 1;\nEND;\n",
        )
        .unwrap();
        std::fs::write(
            &parent,
            "BEGIN;\n    INCLUDE 'child.rosy';\n    VARIABLE from_parent;\n    from_parent := 2;\nEND;\n",
        )
        .unwrap();

        let source = std::fs::read_to_string(&parent).unwrap();
        let result = analyze(&source, Some(&parent));
        let labels: Vec<&str> = result
            .variable_types
            .iter()
            .map(|h| h.label.as_str())
            .collect();
        assert_eq!(
            result.variable_types.len(),
            1,
            "included VARIABLE should not paint onto the parent buffer, got: {:?}",
            labels
        );
        assert_eq!(result.variable_types[0].label, "(RE)");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
