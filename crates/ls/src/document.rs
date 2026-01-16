use std::cell::RefCell;

use tower_lsp_server::ls_types::Position;
use tree_sitter::InputEdit;
use tree_sitter_md::{MarkdownParser, MarkdownTree};

thread_local! {
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

#[derive(Clone, Debug)]
pub struct Document {
    pub version: Option<i32>,
    pub changed: bool,
    pub content: String,
    line_offsets: Vec<usize>,
    tree: Option<MarkdownTree>,
}

impl Document {
    pub fn new(version: Option<i32>, content: String) -> Self {
        let line_offsets = compute_line_offsets(&content);
        let tree = parse(&content, None);
        Self {
            version,
            changed: false,
            content,
            line_offsets,
            tree,
        }
    }

    pub fn apply_change(&mut self, range: Option<tower_lsp_server::ls_types::Range>, text: &str) {
        if let Some(range) = range {
            let start_byte = self.position_to_byte_offset(range.start);
            let old_end_byte = self.position_to_byte_offset(range.end);
            let new_end_byte = start_byte + text.len();

            // Build new content
            let mut new_content = String::with_capacity(
                self.content.len() - (old_end_byte - start_byte) + text.len(),
            );
            new_content.push_str(&self.content[..start_byte]);
            new_content.push_str(text);
            new_content.push_str(&self.content[old_end_byte..]);

            // Create InputEdit for tree-sitter
            let new_end_position = compute_end_position(range.start, text);
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

            if let Some(ref mut tree) = self.tree {
                tree.edit(&input_edit);
            }

            self.content = new_content;
            self.line_offsets = compute_line_offsets(&self.content);
        } else {
            // Full update (fallback)
            self.content = text.to_string();
            self.line_offsets = compute_line_offsets(&self.content);
            self.tree = None;
        }
    }

    pub fn reparse(&mut self) {
        self.tree = parse(&self.content, self.tree.as_ref());
    }

    pub fn reset_content(&mut self, content: String) {
        self.version = None;
        self.content = content;
        self.line_offsets = compute_line_offsets(&self.content);
        self.tree = parse(&self.content, None);
    }

    fn position_to_byte_offset(&self, position: Position) -> usize {
        let line = position.line as usize;
        if line >= self.line_offsets.len() {
            return self.content.len();
        }
        let line_start = self.line_offsets[line];
        let line_end = if line + 1 < self.line_offsets.len() {
            self.line_offsets[line + 1].saturating_sub(1)
        } else {
            self.content.len()
        };
        let line_content = &self.content[line_start..line_end];

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

fn parse(content: &str, old_tree: Option<&MarkdownTree>) -> Option<MarkdownTree> {
    PARSER.with(|parser| parser.borrow_mut().parse(content.as_bytes(), old_tree))
}
