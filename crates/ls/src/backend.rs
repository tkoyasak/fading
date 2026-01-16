use std::cell::RefCell;

use chrono::{Datelike, Local};
use log::debug;
use papaya::HashMap;
use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};
use tree_sitter::InputEdit;
use tree_sitter_md::{MarkdownParser, MarkdownTree};

const CODE_ACTION_UPDATE_METADATA: CodeActionKind =
    CodeActionKind::new("source.updateMetadata.fading");

thread_local! {
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

#[derive(Clone, Debug)]
struct Document {
    version: Option<i32>,
    changed: bool,
    content: String,
    line_offsets: Vec<usize>,
    tree: Option<MarkdownTree>,
}

impl Document {
    fn new(version: Option<i32>, content: String) -> Self {
        let line_offsets = compute_line_offsets(&content);
        let tree = parse_document(&content, None);
        Self {
            version,
            changed: false,
            content,
            line_offsets,
            tree,
        }
    }
}

fn compute_line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn position_to_byte_offset(line_offsets: &[usize], content: &str, position: Position) -> usize {
    let line = position.line as usize;
    if line >= line_offsets.len() {
        return content.len();
    }
    let line_start = line_offsets[line];
    let line_end = if line + 1 < line_offsets.len() {
        line_offsets[line + 1].saturating_sub(1)
    } else {
        content.len()
    };
    let line_content = &content[line_start..line_end];

    // LSP uses UTF-16 code units for character offset
    let mut utf16_offset = 0u32;
    let mut byte_offset = 0usize;
    for ch in line_content.chars() {
        if utf16_offset >= position.character {
            break;
        }
        utf16_offset += ch.len_utf16() as u32;
        byte_offset += ch.len_utf8();
    }
    line_start + byte_offset
}

fn compute_end_position(start: Position, text: &str) -> tree_sitter::Point {
    let mut row = start.line as usize;
    let mut col = start.character as usize;

    for ch in text.chars() {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += ch.len_utf8();
        }
    }

    tree_sitter::Point::new(row, col)
}

fn parse_document(content: &str, old_tree: Option<&MarkdownTree>) -> Option<MarkdownTree> {
    PARSER.with(|parser| parser.borrow_mut().parse(content.as_bytes(), old_tree))
}

#[derive(Debug)]
pub struct Backend {
    _client: Client,
    documents: HashMap<Uri, Document>,
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
                if let Some(range) = change.range {
                    // Incremental update
                    let start_byte =
                        position_to_byte_offset(&doc.line_offsets, &doc.content, range.start);
                    let old_end_byte =
                        position_to_byte_offset(&doc.line_offsets, &doc.content, range.end);

                    let new_text = &change.text;
                    let new_end_byte = start_byte + new_text.len();

                    // Build new content
                    let mut new_content = String::with_capacity(
                        doc.content.len() - (old_end_byte - start_byte) + new_text.len(),
                    );
                    new_content.push_str(&doc.content[..start_byte]);
                    new_content.push_str(new_text);
                    new_content.push_str(&doc.content[old_end_byte..]);

                    // Create InputEdit for tree-sitter
                    let new_end_position = compute_end_position(range.start, new_text);
                    let input_edit = InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position: tree_sitter::Point::new(
                            range.start.line as usize,
                            range.start.character as usize,
                        ),
                        old_end_position: tree_sitter::Point::new(
                            range.end.line as usize,
                            range.end.character as usize,
                        ),
                        new_end_position,
                    };

                    if let Some(ref mut tree) = doc.tree {
                        tree.edit(&input_edit);
                    }

                    doc.content = new_content;
                    doc.line_offsets = compute_line_offsets(&doc.content);
                } else {
                    // Full update (fallback)
                    doc.content = change.text.clone();
                    doc.line_offsets = compute_line_offsets(&doc.content);
                    doc.tree = None;
                }
            }

            // Re-parse the tree
            doc.tree = parse_document(&doc.content, doc.tree.as_ref());
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
                doc.version = None;
                doc.content = content.clone();
                doc.line_offsets = compute_line_offsets(&doc.content);
                doc.tree = parse_document(&doc.content, None);
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
            Some(doc) => {
                if !doc.changed {
                    return None;
                }

                let mut lines = doc.content.lines();

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
