//! LSP backend implementation.
//!
//! Handles the core LSP protocol: document synchronization, code actions, etc.

use log::debug;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::{
    CodeActionOptions, CodeActionParams, CodeActionProviderCapability, CodeActionResponse,
    DiagnosticOptions, DiagnosticServerCapabilities, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, InitializeParams, InitializeResult, PositionEncodingKind,
    RelatedFullDocumentDiagnosticReport, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri,
};
use tower_lsp_server::{Client, LanguageServer};

use crate::code_actions::{code_actions, should_clear_modified, supported_action_kinds};
use crate::diagnostics::{DIAGNOSTIC_SOURCE, diagnose};
use crate::document::Document;

/// The LSP backend that manages document state and handles requests.
#[derive(Debug)]
pub struct Backend {
    /// LSP client for sending notifications.
    _client: Client,
    /// Open documents indexed by URI.
    documents: papaya::HashMap<Uri, Document>,
}

impl Backend {
    /// Creates a new backend instance.
    pub fn new(client: Client) -> Self {
        Self {
            _client: client,
            documents: papaya::HashMap::new(),
        }
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        debug!("fading-ls initialize.");
        if params
            .workspace_folders
            .is_some_and(|folders| folders.iter().any(|f| f.name == "fading"))
        {
            Ok(InitializeResult {
                server_info: Some(ServerInfo {
                    name: "fading".to_string(),
                    version: Some(env!("CARGO_PKG_VERSION").to_string()),
                }),
                capabilities: ServerCapabilities {
                    position_encoding: Some(PositionEncodingKind::UTF8),
                    text_document_sync: Some(TextDocumentSyncCapability::Options(
                        TextDocumentSyncOptions {
                            open_close: Some(true),
                            change: Some(TextDocumentSyncKind::INCREMENTAL),
                            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                                include_text: Some(false),
                            })),
                            ..Default::default()
                        },
                    )),
                    code_action_provider: Some(CodeActionProviderCapability::Options(
                        CodeActionOptions {
                            code_action_kinds: Some(supported_action_kinds()),
                            ..Default::default()
                        },
                    )),
                    diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                        DiagnosticOptions {
                            identifier: Some(DIAGNOSTIC_SOURCE.to_string()),
                            inter_file_dependencies: false,
                            workspace_diagnostics: false,
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
            })
        } else {
            Ok(Default::default())
        }
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        debug!("fading-ls shutdown.");
        self.documents.pin().clear();
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("fading-ls did_open: {}", uri.path());
        let version = params.text_document.version;
        let content = params.text_document.text;
        let doc = Document::new(Some(version), false, content);
        self.documents.pin().insert(uri, doc);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = &params.text_document.uri;
        debug!("fading-ls did_close: {}", uri.path());
        self.documents.pin().remove(uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("fading-ls did_change: {}", uri.path());
        let new_version = params.text_document.version;
        let content_changes = params.content_changes;

        self.documents.pin().update(uri, |doc| {
            let mut new_doc = doc.clone();
            new_doc.update(new_version, &content_changes);
            new_doc
        });
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        debug!("fading-ls did_save: {}", uri.path());

        // Clear modified flag if frontmatter.modified is already today
        self.documents.pin().update(uri, |doc| {
            let mut new_doc = doc.clone();
            if should_clear_modified(&new_doc) {
                new_doc.clear_modified();
            }
            new_doc
        });
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        let uri = &params.text_document.uri;
        debug!("fading-ls diagnostic: {}", uri.path());

        let items = self
            .documents
            .pin()
            .get(uri)
            .map(diagnose)
            .unwrap_or_default();

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
                related_documents: None,
            }),
        ))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        debug!("fading-ls code_action: {}", uri.path());

        // Filter by context.only if specified
        if params.context.only.is_some_and(|only| {
            !supported_action_kinds()
                .iter()
                .any(|kind| only.contains(kind))
        }) {
            return Ok(None);
        }

        let actions = self
            .documents
            .pin()
            .get(&uri)
            .and_then(|doc| code_actions(doc, &uri));

        Ok(actions)
    }
}
