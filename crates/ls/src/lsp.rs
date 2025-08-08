use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use crop::Rope;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer};
use tracing::instrument;

use crate::utils::lsp_range_to_rope_range;

const CODE_ACTION_UPDATE_METADATA: &str = "source.updateMetadata.fading";

#[derive(Debug)]
pub struct Backend {
    _client: Client,
    documents: DashMap<String, (Option<i32>, bool, Rope)>,
}

impl LanguageServer for Backend {
    // https://github.com/zed-industries/zed/blob/6c83a3bcdea1212bc74fc6d46cc6fca869137808/crates/lsp/src/lsp.rs#L597-L837
    #[instrument(skip_all)]
    async fn initialize(&self, _params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "fading ls".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
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
                document_formatting_provider: Some(OneOf::Left(true)),
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
        self.documents.insert(uri, (Some(version), false, rope));
    }

    #[instrument(skip_all)]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.as_str();
        self.documents.remove(uri);
    }

    #[instrument(skip_all)]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let new_version = params.text_document.version;
        let uri = params.text_document.uri.as_str();

        self.documents.alter(uri, |_, (mut version, _, mut rope)| {
            if version.is_none_or(|old_version| old_version <= new_version) {
                version = Some(new_version);

                for change in params.content_changes {
                    if let Some(range) = change.range {
                        let range = lsp_range_to_rope_range(&rope, &range);
                        rope.replace(range, &change.text);
                    } else {
                        rope = Rope::from(change.text);
                    }
                }

                (version, true, rope)
            } else {
                let old_version = version.unwrap();
                panic!("Out-of-sync: currently at {old_version}, get {new_version}");
            }
        });
    }

    #[instrument(skip_all)]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            let uri = params.text_document.uri.as_str();
            let rope = Rope::from(text);
            self.documents
                .alter(uri, |_, (_, date, _)| (None, date, rope));
        }
    }

    #[allow(clippy::mutable_key_type)]
    #[instrument(skip_all)]
    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<CodeActionResponse>> {
        let code_action_kind = CodeActionKind::new(CODE_ACTION_UPDATE_METADATA);
        let uri = params.text_document.uri;

        if let Some(kinds) = params.context.only
            && kinds.contains(&code_action_kind)
            && let Some(update) = self.on_update(&uri)
        {
            let changes = HashMap::from([(uri, update)]);

            let code_action = CodeAction {
                title: "Update Metadata".to_string(),
                kind: Some(code_action_kind),
                edit: Some(WorkspaceEdit::new(changes)),
                ..Default::default()
            };

            Ok(Some(vec![code_action.into()]))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip_all)]
    async fn formatting(
        &self,
        _params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        Ok(None)
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
                let (_, changed, rope) = v.value();

                if !(*changed)
                    || rope.line_len() <= 5
                    || rope.line_slice(0..1) != "+++\n"
                    || rope.line_slice(4..5) != "+++\n"
                {
                    return None;
                }

                let s = rope.line_slice(1..4).to_string();
                let today = Local::now().date_naive();
                if let Ok(mut metadata) = toml::from_str::<Metadata>(&s)
                    && metadata.modified != today
                {
                    let start = Position::new(1, 0);
                    let end = Position::new(4, 0);
                    let range = Range::new(start, end);

                    metadata.modified = today;
                    let new_text = toml::to_string(&metadata).unwrap();

                    Some(vec![TextEdit::new(range, new_text)])
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
