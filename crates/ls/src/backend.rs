//! LSP backend implementation.
//!
//! Handles the core LSP protocol: document synchronization, code actions, etc.

use std::collections::HashMap;
use std::sync::RwLock;

use log::{debug, warn};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionOptions, CodeActionParams, CodeActionProviderCapability,
    CodeActionResponse, DiagnosticOptions, DiagnosticServerCapabilities,
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, FullDocumentDiagnosticReport, InitializeParams,
    InitializeResult, RelatedFullDocumentDiagnosticReport, SaveOptions, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri, WorkspaceEdit,
};
use tower_lsp_server::{Client, LanguageServer};

use crate::diagnostics::{DIAGNOSTC_SOURCE, diagnose};
use crate::document::Document;
use crate::metadata::{CODE_ACTION_UPDATE_METADATA, update_metadata};

/// The LSP backend that manages document state and handles requests.
#[derive(Debug)]
pub struct Backend {
    /// LSP client for sending notifications.
    _client: Client,
    /// Open documents indexed by URI.
    documents: RwLock<HashMap<Uri, Document>>,
}

impl Backend {
    /// Creates a new backend instance.
    pub fn new(client: Client) -> Self {
        Self {
            _client: client,
            documents: RwLock::new(HashMap::new()),
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
                    code_action_provider: Some(CodeActionProviderCapability::Options(
                        CodeActionOptions {
                            code_action_kinds: Some(vec![CODE_ACTION_UPDATE_METADATA]),
                            ..Default::default()
                        },
                    )),
                    diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                        DiagnosticOptions {
                            identifier: Some(DIAGNOSTC_SOURCE.to_string()),
                            inter_file_dependencies: false,
                            workspace_diagnostics: false,
                            ..Default::default()
                        },
                    )),
                    text_document_sync: Some(TextDocumentSyncCapability::Options(
                        TextDocumentSyncOptions {
                            open_close: Some(true),
                            change: Some(TextDocumentSyncKind::INCREMENTAL),
                            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                                include_text: Some(true),
                            })),
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
        self.documents.write().unwrap().clear();
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        debug!("fading-ls did open.");
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let content = params.text_document.text;
        let doc = Document::new(Some(version), content);
        self.documents.write().unwrap().insert(uri, doc);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        debug!("fading-ls did close.");
        let uri = params.text_document.uri;
        self.documents.write().unwrap().remove(&uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("fading-ls did change.");
        let new_version = params.text_document.version;
        let uri = params.text_document.uri;

        let mut docs = self.documents.write().unwrap();
        let Some(doc) = docs.get_mut(&uri) else {
            return;
        };
        if let Some(version) = doc.version()
            && version > new_version
        {
            warn!("Out-of-sync: currently at {version}, got {new_version}");
            return;
        }

        for change in &params.content_changes {
            doc.apply_change(change.range, &change.text);
        }
        doc.update(Some(new_version));
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        debug!("fading-ls did save.");
        if let Some(content) = params.text {
            let uri = params.text_document.uri;
            if let Some(doc) = self.documents.write().unwrap().get_mut(&uri) {
                doc.reset_content(content);
            }
        }
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        debug!("fading-ls diagnostic.");
        let uri = params.text_document.uri;

        let items = self
            .documents
            .read()
            .unwrap()
            .get(&uri)
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
        debug!("fading-ls code action.");
        let uri = params.text_document.uri;

        if params
            .context
            .only
            .is_none_or(|only| only.contains(&CODE_ACTION_UPDATE_METADATA))
            && let Some(edits) = self
                .documents
                .read()
                .unwrap()
                .get(&uri)
                .and_then(update_metadata)
        {
            let changes = HashMap::from([(uri, edits)]);

            let code_action = CodeAction {
                title: "Update metadata".to_string(),
                kind: Some(CODE_ACTION_UPDATE_METADATA),
                is_preferred: Some(true),
                edit: Some(WorkspaceEdit::new(changes)),
                ..Default::default()
            };

            Ok(Some(vec![code_action.into()]))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tower_lsp_server::LspService;
    use tower_lsp_server::ls_types::{
        CodeActionContext, NumberOrString, Position, Range, TextDocumentContentChangeEvent,
        TextDocumentIdentifier, TextDocumentItem, VersionedTextDocumentIdentifier, WorkspaceFolder,
    };

    fn create_test_backend() -> &'static Backend {
        let (service, _) = LspService::new(Backend::new);
        // Leak the service to get a 'static reference for testing
        Box::leak(Box::new(service)).inner()
    }

    fn make_uri(path: &str) -> Uri {
        Uri::from_str(&format!("file://{path}")).unwrap()
    }

    #[tokio::test]
    async fn test_initialize_with_fading_workspace() {
        let backend = create_test_backend();
        let params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: make_uri("/workspace/fading"),
                name: "fading".to_string(),
            }]),
            ..Default::default()
        };

        let result = backend.initialize(params).await.unwrap();
        assert!(result.server_info.is_some());
        assert_eq!(result.server_info.unwrap().name, "fading");
        assert!(result.capabilities.text_document_sync.is_some());
    }

    #[tokio::test]
    async fn test_initialize_without_fading_workspace() {
        let backend = create_test_backend();
        let params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: make_uri("/workspace/other"),
                name: "other".to_string(),
            }]),
            ..Default::default()
        };

        let result = backend.initialize(params).await.unwrap();
        assert!(result.server_info.is_none());
    }

    #[tokio::test]
    async fn test_did_open_and_close() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "hello".to_string(),
                },
            })
            .await;

        assert!(backend.documents.read().unwrap().get(&uri).is_some());

        backend
            .did_close(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            })
            .await;

        assert!(backend.documents.read().unwrap().get(&uri).is_none());
    }

    #[tokio::test]
    async fn test_did_change_incremental() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "hello world".to_string(),
                },
            })
            .await;

        backend
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(0, 5), Position::new(0, 5))),
                    range_length: None,
                    text: ",".to_string(),
                }],
            })
            .await;

        let guard = backend.documents.read().unwrap();
        let doc = guard.get(&uri).unwrap();
        assert_eq!(doc.content(), "hello, world");
        assert_eq!(doc.version(), Some(2));
        assert!(doc.modified());
    }

    #[tokio::test]
    async fn test_did_change_full() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "old".to_string(),
                },
            })
            .await;

        backend
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "new content".to_string(),
                }],
            })
            .await;

        let guard = backend.documents.read().unwrap();
        let doc = guard.get(&uri).unwrap();
        assert_eq!(doc.content(), "new content");
    }

    #[tokio::test]
    async fn test_did_save() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "original".to_string(),
                },
            })
            .await;

        backend
            .did_save(DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                text: Some("saved content".to_string()),
            })
            .await;

        let guard = backend.documents.read().unwrap();
        let doc = guard.get(&uri).unwrap();
        assert_eq!(doc.content(), "saved content");
        assert_eq!(doc.version(), None);
    }

    #[tokio::test]
    async fn test_code_action_returns_update_metadata() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        let content =
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\n+++\n\nContent";

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: content.to_string(),
                },
            })
            .await;

        // Mark as changed
        backend
            .did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: content.to_string(),
                }],
            })
            .await;

        let result = backend
            .code_action(CodeActionParams {
                text_document: TextDocumentIdentifier { uri },
                range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                context: CodeActionContext {
                    diagnostics: vec![],
                    only: None,
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();

        assert!(result.is_some());
        let actions = result.unwrap();
        assert_eq!(actions.len(), 1);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "hello".to_string(),
                },
            })
            .await;

        assert!(backend.documents.read().unwrap().get(&uri).is_some());

        backend.shutdown().await.unwrap();

        assert!(backend.documents.read().unwrap().get(&uri).is_none());
    }

    #[tokio::test]
    async fn test_diagnostic_returns_errors() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        // Document without frontmatter
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "# No frontmatter".to_string(),
                },
            })
            .await;

        let result = backend
            .diagnostic(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();

        match result {
            DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => {
                assert_eq!(report.full_document_diagnostic_report.items.len(), 1);
                assert_eq!(
                    report.full_document_diagnostic_report.items[0]
                        .code
                        .as_ref()
                        .unwrap(),
                    &NumberOrString::String("missing-frontmatter".to_string())
                );
            }
            _ => panic!("Expected full diagnostic report"),
        }
    }

    #[tokio::test]
    async fn test_diagnostic_valid_document() {
        let backend = create_test_backend();
        let uri = make_uri("/test.md");

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\nContent".to_string(),
                },
            })
            .await;

        let result = backend
            .diagnostic(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await
            .unwrap();

        match result {
            DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => {
                assert!(report.full_document_diagnostic_report.items.is_empty());
            }
            _ => panic!("Expected full diagnostic report"),
        }
    }
}
