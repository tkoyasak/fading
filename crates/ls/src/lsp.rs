use chrono::{Local, NaiveDate};
use crop::Rope;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer};

use crate::utils::lsp_range_to_rope_range;

#[derive(Debug)]
pub struct Backend {
    #[allow(dead_code)]
    client: Client,
    documents: DashMap<String, Rope>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "fading-ls".to_owned(),
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        will_save: None,
                        will_save_wait_until: Some(true),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    },
                )),
                ..Default::default()
            },
        })
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let rope = Rope::from(params.text_document.text);
        self.documents
            .insert(params.text_document.uri.to_string(), rope);
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(params.text_document.uri.as_str());
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.documents
            .alter(params.text_document.uri.as_str(), |_, mut rope| {
                for change in params.content_changes {
                    if let Some(ref range) = change.range {
                        let range = lsp_range_to_rope_range(&rope, range);
                        rope.replace(range, &change.text);
                    } else {
                        rope = Rope::from(change.text);
                    }
                }
                rope
            });
    }

    async fn will_save_wait_until(
        &self,
        params: WillSaveTextDocumentParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        let edits = self.on_update(&params.text_document.uri);
        Ok(edits)
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            let rope = Rope::from(text);
            self.documents
                .insert(params.text_document.uri.to_string(), rope);
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
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
        }
    }

    fn on_update(&self, uri: &Uri) -> Option<Vec<TextEdit>> {
        match self.documents.get(uri.as_str()) {
            Some(rope) => {
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
