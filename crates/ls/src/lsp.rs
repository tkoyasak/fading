use std::collections::HashMap;
use std::ops::Deref;

use chrono::{Local, NaiveDate};
use crop::Rope;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer};
use tracing::instrument;

use crate::utils::lsp_range_to_rope_range;

const CODE_ACTION_UPDATE_METADATA: &str = "fading.updateMetadata";

#[derive(Debug)]
pub struct Backend {
    _client: Client,
    documents: DashMap<String, (i32, Rope)>,
}

impl LanguageServer for Backend {
    // https://github.com/zed-industries/zed/blob/6c83a3bcdea1212bc74fc6d46cc6fca869137808/crates/lsp/src/lsp.rs#L597-L837
    #[instrument(skip_all)]
    async fn initialize(&self, _params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "fading-ls".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..Default::default()
                    },
                )),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![CodeActionKind::new(
                            CODE_ACTION_UPDATE_METADATA,
                        )]),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
        })
    }

    #[instrument(skip_all)]
    async fn initialized(&self, _params: InitializedParams) {}

    #[instrument(skip_all)]
    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    #[instrument(skip_all)]
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.to_string();
        let version = params.text_document.version;
        let rope = Rope::from(params.text_document.text);
        self.documents.insert(uri, (version, rope));
    }

    #[instrument(skip_all)]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.as_str();
        self.documents.remove(uri);
    }

    #[instrument(skip_all)]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.as_str();
        self.documents.alter(uri, |_, (mut version, mut rope)| {
            if version < params.text_document.version {
                version = params.text_document.version;
                for change in params.content_changes {
                    if let Some(ref range) = change.range {
                        let range = lsp_range_to_rope_range(&rope, range);
                        rope.replace(range, &change.text);
                    } else {
                        rope = Rope::from(change.text);
                    }
                }
                (version, rope)
            } else {
                (version, rope)
            }
        });
    }

    #[instrument(skip_all)]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            let uri = params.text_document.uri.as_str();
            let version = i32::MIN;
            let rope = Rope::from(text);
            self.documents.alter(uri, |_, _| (version, rope));
        }
    }

    #[instrument(skip_all)]
    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<CodeActionResponse>> {
        if let Some(kinds) = params.context.only
            && kinds
                .iter()
                .any(|kind| kind.as_str() == CODE_ACTION_UPDATE_METADATA)
            && let Some(update) = self.on_update(&params.text_document.uri)
        {
            #[allow(clippy::mutable_key_type)]
            let changes = HashMap::from([(params.text_document.uri, update)]);
            Ok(Some(vec![CodeActionOrCommand::CodeAction(CodeAction {
                title: "Update Metadata".into(),
                kind: Some(CodeActionKind::new(CODE_ACTION_UPDATE_METADATA)),
                edit: Some(WorkspaceEdit::new(changes)),
                ..Default::default()
            })]))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Metadata {
    id: String,
    #[serde(default)]
    created: NaiveDate,
    #[serde(default)]
    modified: NaiveDate,
}

impl Backend {
    pub fn new(_client: Client) -> Self {
        Self {
            _client,
            documents: DashMap::new(),
        }
    }

    fn on_update(&self, uri: &Uri) -> Option<Vec<TextEdit>> {
        match self.documents.get(uri.as_str()) {
            Some(v) => {
                let (_, rope) = v.deref();
                if rope.line_slice(0..1) != "+++\n" {
                    return None;
                }

                let s = rope.line_slice(1..4).to_string();
                let now = Local::now().date_naive();
                let edit = if let Ok(mut metadata) = toml::from_str::<Metadata>(&s)
                    && metadata.modified != now
                {
                    metadata.modified = now;
                    let new_text = toml::to_string(&metadata).unwrap();
                    let start = Position::new(1, 0);
                    let end = Position::new(4, 0);
                    let range = Range::new(start, end);
                    TextEdit { range, new_text }
                } else {
                    return None;
                };

                if rope.line_slice(4..5) != "+++\n" {
                    return None;
                }
                Some(vec![edit])
            }
            None => None,
        }
    }
}
