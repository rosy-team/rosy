//! # Language Server
//!
//! LSP server for Rosy, providing diagnostics, completion, hover,
//! inlay type hints, and semantic token highlighting.
//!
//! Launched via `rosy lsp` — communicates over stdin/stdout using
//! the Language Server Protocol.

pub mod analysis;
pub mod server;

/// Run the LSP server on stdin/stdout. This function blocks until the client disconnects.
pub async fn run() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = tower_lsp::LspService::new(server::RosyLanguageServer::new);
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
