use chrono::{Datelike, Local};
use crop::Rope;
use dashmap::DashMap;
use log::debug;
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer};

use crate::utils::lsp_range_to_rope_range;

const CODE_ACTION_UPDATE_METADATA: CodeActionKind =
    CodeActionKind::new("source.updateMetadata.fading");

#[derive(Debug)]
pub struct Backend {
    _client: Client,
    documents: DashMap<String, (Option<i32>, bool, Rope)>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        debug!("fading-ls initialize.");
        if params.workspace_folders.is_some_and(|folders| {
            folders.iter().any(|folder| {
                folder
                    .uri
                    .as_str()
                    .rsplit_once('/')
                    .is_some_and(|(_, name)| name == "fading")
            })
        }) {
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

    async fn initialized(&self, _params: InitializedParams) {
        debug!("fading-ls initialized.");
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        debug!("fading-ls shutdown.");
        self.documents.clear();
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        debug!("fading-ls did open.");
        let uri = params.text_document.uri.to_string();
        let version = params.text_document.version;
        let rope = Rope::from(params.text_document.text);
        self.documents.insert(uri, (Some(version), false, rope));
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        debug!("fading-ls did close.");
        let uri = params.text_document.uri.as_str();
        self.documents.remove(uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("fading-ls did change.");
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
                panic!("Out-of-sync: currently at {old_version}, got {new_version}");
            }
        });
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        debug!("fading-ls did save.");
        if let Some(text) = params.text {
            let uri = params.text_document.uri.as_str();
            let rope = Rope::from(text);
            self.documents
                .alter(uri, |_, (_, changed, _)| (None, changed, rope));
        }
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<CodeActionResponse>> {
        debug!("fading-ls code action.");
        let uri = params.text_document.uri;

        if let Some(update) = self.on_update(&uri) {
            #[expect(clippy::mutable_key_type)]
            let changes = std::collections::HashMap::from([(uri, update)]);

            let code_action = CodeAction {
                title: "Update Metadata".to_string(),
                kind: Some(CODE_ACTION_UPDATE_METADATA),
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

#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    id: String,
    created: toml::value::Date,
    modified: toml::value::Date,
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
                let today = toml::value::Date {
                    year: today.year() as u16,
                    month: today.month() as u8,
                    day: today.day() as u8,
                };

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
