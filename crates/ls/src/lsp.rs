use chrono::{Local, NaiveDate};
use dashmap::DashMap;
use pulldown_cmark::{Event, MetadataBlockKind, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer};

#[derive(Debug)]
pub struct Backend {
    #[allow(dead_code)]
    client: Client,
    documents: DashMap<String, String>,
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

    fn on_change(&self, uri: &Uri, text: &str) {
        self.documents.insert(uri.to_string(), text.to_owned());
    }

    fn on_remove(&self, uri: &Uri) {
        self.documents.remove(&uri.to_string());
    }

    fn on_update(&self, uri: &Uri) -> Option<TextEdit> {
        match self.documents.get(&uri.to_string()) {
            Some(ref text) => {
                let exts = Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
                let mut parsed = Parser::new_ext(text, exts);

                if let Some(event) = parsed.next()
                    && let Event::Start(tag) = event
                    && tag == Tag::MetadataBlock(MetadataBlockKind::PlusesStyle)
                {
                } else {
                    return None;
                };

                let edit = if let Some(event) = parsed.next()
                    && let Event::Text(s) = event
                    && let Ok(mut metadata) = toml::from_str::<Metadata>(&s)
                {
                    let now = Local::now().date_naive();
                    if metadata.modified != now {
                        metadata.modified = now;
                        let new_text = toml::to_string(&metadata).unwrap();
                        let start = Position::new(1, 0);
                        let end = Position::new(4, 0);
                        let range = Range::new(start, end);
                        TextEdit { range, new_text }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                };

                if let Some(event) = parsed.next()
                    && let Event::End(tag) = event
                    && tag == TagEnd::MetadataBlock(MetadataBlockKind::PlusesStyle)
                {
                } else {
                    return None;
                };

                Some(edit)
            }
            None => None,
        }
    }
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
                        change: Some(TextDocumentSyncKind::FULL),
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
        let _ = params;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.on_remove(&params.text_document.uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let Some(event) = params.content_changes.last() else {
            return;
        };
        self.on_change(&params.text_document.uri, &event.text);
    }

    async fn will_save_wait_until(
        &self,
        params: WillSaveTextDocumentParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        match self.on_update(&params.text_document.uri) {
            Some(edit) => Ok(Some(vec![edit])),
            None => Ok(None),
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.on_remove(&params.text_document.uri);
    }
}
