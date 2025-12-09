use chrono::{Datelike, Local};
use log::debug;
use papaya::HashMap;
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

const CODE_ACTION_UPDATE_METADATA: CodeActionKind =
    CodeActionKind::new("source.updateMetadata.fading");

#[derive(Debug)]
pub struct Backend {
    _client: Client,
    documents: HashMap<Uri, (Option<i32>, bool, String)>,
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
                            change: Some(TextDocumentSyncKind::FULL),
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
        self.documents
            .pin()
            .insert(uri, (Some(version), false, content));
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

        self.documents.pin().update(uri, |(version, _, _)| {
            if version.is_none_or(|old_version| old_version <= new_version) {
                let content = params.content_changes[0].text.clone();
                (Some(new_version), true, content)
            } else {
                let old_version = version.unwrap();
                panic!("Out-of-sync: currently at {old_version}, got {new_version}");
            }
        });
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        debug!("fading-ls did save.");
        if let Some(content) = params.text {
            let uri = params.text_document.uri;
            self.documents
                .pin()
                .update(uri, |(_, changed, _)| (None, *changed, content.clone()));
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
            && let Some(update) = self.on_update(&uri)
        {
            let changes = std::collections::HashMap::from([(uri, update)]);

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
            documents: HashMap::new(),
        }
    }

    fn on_update(&self, uri: &Uri) -> Option<Vec<TextEdit>> {
        match self.documents.pin().get(uri) {
            Some((_, changed, content)) => {
                if !(*changed) {
                    return None;
                }

                let mut lines = content.lines();

                if lines.next() != Some("+++") {
                    return None;
                }

                let s = format!("{}\n{}\n{}\n", lines.next()?, lines.next()?, lines.next()?);

                if lines.next() != Some("+++") {
                    return None;
                }

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
