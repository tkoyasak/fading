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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tower::{Service, ServiceExt};
    use tower_lsp_server::LspService;
    use tower_lsp_server::jsonrpc::{Request, Response};

    fn initialize_request(id: i64, with_fading_folder: bool) -> Request {
        let folders = if with_fading_folder {
            json!([{"uri": "file:///workspace/fading", "name": "fading"}])
        } else {
            json!([{"uri": "file:///workspace/other", "name": "other"}])
        };

        Request::build("initialize")
            .params(json!({"capabilities": {}, "workspaceFolders": folders}))
            .id(id)
            .finish()
    }

    fn did_open_request(uri: &str, content: &str) -> Request {
        Request::build("textDocument/didOpen")
            .params(json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "fading",
                    "version": 1,
                    "text": content
                }
            }))
            .finish()
    }

    fn did_change_request(uri: &str, version: i32, text: &str) -> Request {
        Request::build("textDocument/didChange")
            .params(json!({
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": text}]
            }))
            .finish()
    }

    fn did_save_request(uri: &str) -> Request {
        Request::build("textDocument/didSave")
            .params(json!({"textDocument": {"uri": uri}}))
            .finish()
    }

    fn did_close_request(uri: &str) -> Request {
        Request::build("textDocument/didClose")
            .params(json!({"textDocument": {"uri": uri}}))
            .finish()
    }

    fn diagnostic_request(id: i64, uri: &str) -> Request {
        Request::build("textDocument/diagnostic")
            .params(json!({"textDocument": {"uri": uri}}))
            .id(id)
            .finish()
    }

    fn code_action_request(id: i64, uri: &str) -> Request {
        Request::build("textDocument/codeAction")
            .params(json!({
                "textDocument": {"uri": uri},
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "context": {"diagnostics": []}
            }))
            .id(id)
            .finish()
    }

    fn code_action_request_with_only(id: i64, uri: &str, only: &[&str]) -> Request {
        Request::build("textDocument/codeAction")
            .params(json!({
                "textDocument": {"uri": uri},
                "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                "context": {"diagnostics": [], "only": only}
            }))
            .id(id)
            .finish()
    }

    async fn send(service: &mut LspService<Backend>, request: Request) -> Option<Response> {
        service.ready().await.unwrap().call(request).await.unwrap()
    }

    #[tokio::test]
    async fn test_initialize_with_fading_folder() {
        let (mut service, _) = LspService::new(Backend::new);

        let request = initialize_request(1, true);
        let response = send(&mut service, request).await.unwrap();

        // Response should contain result (not error)
        let result = response.result().expect("should have result");
        let result_str = result.to_string();
        assert!(
            result_str.contains("fading"),
            "result should contain 'fading'"
        );
        assert!(
            result_str.contains("textDocumentSync"),
            "result should contain 'textDocumentSync'"
        );
    }

    #[tokio::test]
    async fn test_initialize_without_fading_folder() {
        let (mut service, _) = LspService::new(Backend::new);

        let request = initialize_request(1, false);
        let response = send(&mut service, request).await.unwrap();

        // Should return default/empty capabilities
        let result = response.result().expect("should have result");
        let result_str = result.to_string();
        // Default result should not contain "fading" server info
        assert!(
            !result_str.contains(r#""name":"fading""#),
            "result should not contain 'fading' server info"
        );
    }

    #[tokio::test]
    async fn test_document_lifecycle() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        let init = initialize_request(1, true);
        send(&mut service, init).await;

        let uri = "file:///test.md";
        let content = "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n";

        // Open document (notification - no response)
        let open = did_open_request(uri, content);
        assert!(send(&mut service, open).await.is_none());

        // Change document (notification - no response)
        let change = did_change_request(uri, 2, "new content");
        assert!(send(&mut service, change).await.is_none());

        // Save document (notification - no response)
        let save = did_save_request(uri);
        assert!(send(&mut service, save).await.is_none());

        // Close document (notification - no response)
        let close = did_close_request(uri);
        assert!(send(&mut service, close).await.is_none());
    }

    #[tokio::test]
    async fn test_diagnostic_with_valid_document() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        let uri = "file:///test.md";
        let content = "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n";

        // Open document
        send(&mut service, did_open_request(uri, content)).await;

        // Request diagnostics
        let response = send(&mut service, diagnostic_request(2, uri))
            .await
            .unwrap();
        let result = response.result().expect("should have result");

        // Valid document should have empty diagnostics
        assert!(result.to_string().contains("items"));
    }

    #[tokio::test]
    async fn test_diagnostic_with_invalid_document() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        let uri = "file:///test.md";
        let content = "no frontmatter";

        // Open document
        send(&mut service, did_open_request(uri, content)).await;

        // Request diagnostics
        let response = send(&mut service, diagnostic_request(2, uri))
            .await
            .unwrap();
        let result = response.result().expect("should have result");
        let result_str = result.to_string();

        // Invalid document should have diagnostics
        assert!(result_str.contains("missing-frontmatter"));
    }

    #[tokio::test]
    async fn test_diagnostic_unknown_document() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        // Request diagnostics for unknown document
        let response = send(&mut service, diagnostic_request(2, "file:///unknown.md"))
            .await
            .unwrap();
        let result = response.result().expect("should have result");

        // Unknown document should return empty diagnostics
        assert!(result.to_string().contains("items"));
    }

    #[tokio::test]
    async fn test_code_action_with_modified_outdated_document() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        let uri = "file:///test.md";
        // Document with outdated modified date
        let content = "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\n+++\n";

        // Open document
        send(&mut service, did_open_request(uri, content)).await;

        // Modify document to set modified flag
        send(&mut service, did_change_request(uri, 2, content)).await;

        // Request code actions
        let response = send(&mut service, code_action_request(3, uri))
            .await
            .unwrap();
        let result = response.result().expect("should have result");
        let result_str = result.to_string();

        // Should have updateMetadata action
        assert!(result_str.contains("updateMetadata") || result_str.contains("Update metadata"));
    }

    #[tokio::test]
    async fn test_code_action_with_unmodified_document() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        let uri = "file:///test.md";
        let content = "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\n+++\n";

        // Open document (not modified)
        send(&mut service, did_open_request(uri, content)).await;

        // Request code actions without modifying
        let response = send(&mut service, code_action_request(2, uri))
            .await
            .unwrap();
        let result = response.result().expect("should have result");

        // Unmodified document should have no actions (null)
        assert!(result.is_null() || result.to_string() == "[]");
    }

    #[tokio::test]
    async fn test_code_action_filtered_by_only() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        let uri = "file:///test.md";
        let content = "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\n+++\n";

        // Open and modify document
        send(&mut service, did_open_request(uri, content)).await;
        send(&mut service, did_change_request(uri, 2, content)).await;

        // Request with non-matching "only" filter
        let response = send(
            &mut service,
            code_action_request_with_only(3, uri, &["quickfix"]),
        )
        .await
        .unwrap();
        let result = response.result().expect("should have result");

        // Should return null when filter doesn't match
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn test_shutdown_clears_documents() {
        let (mut service, _) = LspService::new(Backend::new);

        // Initialize
        send(&mut service, initialize_request(1, true)).await;

        let uri = "file:///test.md";
        let content = "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n";

        // Open document
        send(&mut service, did_open_request(uri, content)).await;

        // Shutdown
        let shutdown = Request::build("shutdown").id(2).finish();
        let response = send(&mut service, shutdown).await.unwrap();
        assert!(response.result().is_some());
    }
}
