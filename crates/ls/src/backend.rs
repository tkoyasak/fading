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
                    document_formatting_provider: Some(OneOf::Left(true)),
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
                let old_version = doc.version.unwrap();
                panic!("Out-of-sync: currently at {old_version}, got {new_version}");
            }

            for change in &params.content_changes {
                doc.apply_change(change.range, &change.text);
            }

            doc.reparse();
            doc.version = Some(new_version);
            doc.changed = true;
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

    async fn formatting(
        &self,
        _params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        debug!("fading-ls formatting.");
        Ok(None)
    }
}
