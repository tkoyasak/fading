//! Language Server for the fading markdown-based diary system.
//!
//! This crate provides an LSP implementation that supports:
//! - Incremental document synchronization with tree-sitter parsing
//! - Automatic metadata (modified date) updates via code actions
//! - Real-time diagnostics for frontmatter validation

use tower_lsp_server::{LspService, Server};

mod backend;
mod diagnostics;
mod document;
mod metadata;

/// Starts the language server.
///
/// Initializes logging, creates the LSP service, and begins serving
/// requests over stdin/stdout.
pub async fn start() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(backend::Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
