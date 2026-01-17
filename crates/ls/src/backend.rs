use log::debug;
use papaya::HashMap;
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

use crate::document::Document;
use crate::metadata::{CODE_ACTION_UPDATE_METADATA, update_metadata};

#[derive(Debug)]
pub struct Backend {
    _client: Client,
    documents: HashMap<Uri, Document>,
}

impl Backend {
    pub fn new(_client: Client) -> Self {
        Self {
            _client,
            documents: HashMap::new(),
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
                    text_document_sync: Some(TextDocumentSyncCapability::Options(
                        TextDocumentSyncOptions {
                            change: Some(TextDocumentSyncKind::INCREMENTAL),
                            open_close: Some(true),
                            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                                include_text: Some(true),
                            })),
                            ..Default::default()
                        },
                    )),
                    code_action_provider: Some(CodeActionProviderCapability::Options(
                        CodeActionOptions {
                            code_action_kinds: Some(vec![CODE_ACTION_UPDATE_METADATA]),
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
        debug!("fading-ls did open.");
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let content = params.text_document.text;
        let doc = Document::new(Some(version), content);
        self.documents.pin().insert(uri, doc);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        debug!("fading-ls did close.");
        let uri = params.text_document.uri;
        self.documents.pin().remove(&uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("fading-ls did change.");
        let new_version = params.text_document.version;
        let uri = params.text_document.uri;

        self.documents.pin().update(uri, |doc| {
            let mut doc = doc.clone();

            if doc.version.is_some_and(|v| v > new_version) {
                log::warn!(
                    "Out-of-sync: currently at {}, got {}",
                    doc.version.unwrap(),
                    new_version
                );
                return doc;
            }

            for change in &params.content_changes {
                doc.apply_change(change.range, &change.text);
            }

            doc.update(Some(new_version));
            doc
        });
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        debug!("fading-ls did save.");
        if let Some(content) = params.text {
            let uri = params.text_document.uri;
            self.documents.pin().update(uri, |doc| {
                let mut doc = doc.clone();
                doc.reset_content(content.clone());
                doc
            });
        }
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
            && let Some(doc) = self.documents.pin().get(&uri)
            && let Some(edits) = update_metadata(doc)
        {
            let changes = std::collections::HashMap::from([(uri, edits)]);

            let code_action = CodeAction {
                title: "update metadata".to_string(),
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

        assert!(backend.documents.pin().get(&uri).is_some());

        backend
            .did_close(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            })
            .await;

        assert!(backend.documents.pin().get(&uri).is_none());
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

        let guard = backend.documents.pin();
        let doc = guard.get(&uri).unwrap();
        assert_eq!(doc.content, "hello, world");
        assert_eq!(doc.version, Some(2));
        assert!(doc.modified);
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

        let guard = backend.documents.pin();
        let doc = guard.get(&uri).unwrap();
        assert_eq!(doc.content, "new content");
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

        let guard = backend.documents.pin();
        let doc = guard.get(&uri).unwrap();
        assert_eq!(doc.content, "saved content");
        assert_eq!(doc.version, None);
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

        assert!(backend.documents.pin().get(&uri).is_some());

        backend.shutdown().await.unwrap();

        assert!(backend.documents.pin().get(&uri).is_none());
    }
}
