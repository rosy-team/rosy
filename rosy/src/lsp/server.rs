//! LSP server implementation for Rosy.
//!
//! Provides diagnostics, completion, hover, and inlay hints by running
//! the real rosy parser and type resolver on each document change.

use std::collections::HashMap;
use std::sync::Mutex;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use super::analysis;

/// Per-document state cached between requests.
struct DocumentState {
    /// The latest source text.
    text: String,
    /// The latest analysis result.
    analysis: analysis::AnalysisResult}

pub struct RosyLanguageServer {
    client: Client,
    documents: Mutex<HashMap<Url, DocumentState>>}

impl RosyLanguageServer {
    pub fn new(client: Client) -> Self {
        crate::syntax_config::set_cosy_syntax(false);

        RosyLanguageServer {
            client,
            documents: Mutex::new(HashMap::new())}
    }

    /// Run analysis on a document and publish diagnostics.
    async fn on_change(&self, uri: Url, text: String) {
        let path = uri.to_file_path().ok();
        let result = analysis::analyze(&text, path.as_deref());

        let diagnostics = result.diagnostics.clone();

        self.documents.lock().unwrap().insert(
            uri.clone(),
            DocumentState {
                text,
                analysis: result},
        );

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for RosyLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // Full document sync — client sends entire text on each change
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                // Completion for keywords and intrinsic functions
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![" ".to_string()]),
                    ..Default::default()
                }),
                // Hover for type info and documentation links
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                // Inlay hints for variable types
                inlay_hint_provider: Some(OneOf::Left(true)),
                // Semantic tokens for syntax highlighting via the real parser
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: analysis::LEGEND_TOKEN_TYPES.to_vec(),
                                token_modifiers: vec![]},
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "rosy-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    // ─── Document Sync ─────────────────────────────────────────────────

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // Full sync mode: first content change is the entire document
        if let Some(change) = params.content_changes.into_iter().next() {
            self.on_change(params.text_document.uri, change.text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        // Re-analyze from disk on save if the client didn't include text
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.on_change(uri, text).await;
        } else if let Ok(path) = uri.to_file_path()
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            self.on_change(uri, text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .lock()
            .unwrap()
            .remove(&params.text_document.uri);
        // Clear diagnostics for closed document
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }

    // ─── Completion ────────────────────────────────────────────────────

    async fn completion(&self, _params: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(analysis::rosy_keywords())))
    }

    // ─── Hover ─────────────────────────────────────────────────────────

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let docs = self.documents.lock().unwrap();
        let Some(doc) = docs.get(uri) else {
            return Ok(None);
        };

        let Some(word) = word_at_position(&doc.text, position) else {
            return Ok(None);
        };

        let upper = word.to_uppercase();

        // All hover docs are auto-generated from module doc comments at build time.
        // Check type annotations first, then keywords/constructs.
        let hover_text = analysis::ROSY_TYPE_HOVER
            .iter()
            .find(|(name, _, _)| *name == upper)
            .map(|(_, markdown, _)| markdown.to_string())
            .or_else(|| {
                analysis::ROSY_HOVER_DOCS
                    .iter()
                    .find(|(kw, _, _, _, _)| *kw == upper)
                    .map(|(kw, title, desc, url, is_stmt)| {
                        let kind_label = if *is_stmt { "Statement" } else { "Expression" };
                        let desc_line = if desc.is_empty() {
                            String::new()
                        } else {
                            format!("\n\n{desc}")
                        };
                        format!(
                            "**{kw}** \u{2014} {title}\n\n*{kind_label}*{desc_line}\n\n[Documentation]({url})"
                        )
                    })
            });

        Ok(hover_text.map(|text| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: text}),
            range: None}))
    }

    // ─── Inlay Hints ───────────────────────────────────────────────────

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri.clone();
        let docs = self.documents.lock().unwrap();
        let Some(doc) = docs.get(&uri) else {
            return Ok(None);
        };

        let hints: Vec<InlayHint> = doc
            .analysis
            .variable_types
            .iter()
            .filter(|h| {
                h.position.line >= params.range.start.line
                    && h.position.line <= params.range.end.line
            })
            .map(|h| {
                // Double-click inserts the type annotation at the hint position
                let edit = TextEdit {
                    range: Range::new(h.position, h.position),
                    new_text: format!(" {}", h.label)};

                InlayHint {
                    position: h.position,
                    label: InlayHintLabel::String(h.label.clone()),
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: Some(vec![edit]),
                    tooltip: h.inferred_from.as_ref().map(|loc| {
                        InlayHintTooltip::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!("**{}** — {}", h.label, loc.reason)})
                    }),
                    padding_left: Some(true),
                    padding_right: Some(false),
                    data: None}
            })
            .collect();

        Ok(Some(hints))
    }

    // ─── Semantic Tokens ───────────────────────────────────────────────

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let docs = self.documents.lock().unwrap();
        let Some(doc) = docs.get(&params.text_document.uri) else {
            return Ok(None);
        };

        // LSP semantic tokens use delta encoding:
        // each token is relative to the previous one.
        let mut lsp_tokens = Vec::new();
        let mut prev_line = 0u32;
        let mut prev_start = 0u32;

        for token in &doc.analysis.semantic_tokens {
            let delta_line = token.line - prev_line;
            let delta_start = if delta_line == 0 {
                token.start_col - prev_start
            } else {
                token.start_col
            };

            lsp_tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type.index(),
                token_modifiers_bitset: 0});

            prev_line = token.line;
            prev_start = token.start_col;
        }

        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: lsp_tokens})))
    }
}

/// Extract the word at a given position in the source text.
fn word_at_position(text: &str, position: Position) -> Option<String> {
    let line = text.lines().nth(position.line as usize)?;
    let col = position.character as usize;

    if col > line.len() {
        return None;
    }

    // Find word boundaries
    let bytes = line.as_bytes();
    let mut start = col;
    let mut end = col;

    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }

    Some(line[start..end].to_string())
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
