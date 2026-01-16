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

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Range;

    #[test]
    fn test_compute_line_offsets_empty() {
        assert_eq!(compute_line_offsets(""), vec![0]);
    }

    #[test]
    fn test_compute_line_offsets_single_line() {
        assert_eq!(compute_line_offsets("hello"), vec![0]);
    }

    #[test]
    fn test_compute_line_offsets_multiple_lines() {
        assert_eq!(compute_line_offsets("hello\nworld\n"), vec![0, 6, 12]);
    }

    #[test]
    fn test_compute_end_position_single_line() {
        let start = Position::new(0, 0);
        let point = compute_end_position(start, "hello");
        assert_eq!(point.row, 0);
        assert_eq!(point.column, 5);
    }

    #[test]
    fn test_compute_end_position_with_newline() {
        let start = Position::new(0, 0);
        let point = compute_end_position(start, "hello\nworld");
        assert_eq!(point.row, 1);
        assert_eq!(point.column, 5);
    }

    #[test]
    fn test_compute_end_position_from_middle() {
        let start = Position::new(2, 5);
        let point = compute_end_position(start, "ab\ncd");
        assert_eq!(point.row, 3);
        assert_eq!(point.column, 2);
    }

    #[test]
    fn test_document_new() {
        let doc = Document::new(Some(1), "hello\nworld".to_string());
        assert_eq!(doc.version, Some(1));
        assert!(!doc.changed);
        assert_eq!(doc.content, "hello\nworld");
    }

    #[test]
    fn test_document_position_to_byte_offset_ascii() {
        let doc = Document::new(None, "hello\nworld".to_string());
        // line 0, char 0 -> byte 0
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 0)), 0);
        // line 0, char 5 -> byte 5
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 5)), 5);
        // line 1, char 0 -> byte 6
        assert_eq!(doc.position_to_byte_offset(Position::new(1, 0)), 6);
        // line 1, char 3 -> byte 9
        assert_eq!(doc.position_to_byte_offset(Position::new(1, 3)), 9);
    }

    #[test]
    fn test_document_position_to_byte_offset_utf16() {
        // "あ" is 3 bytes in UTF-8, 1 code unit in UTF-16
        // "𠮷" (U+20BB7) is 4 bytes in UTF-8, 2 code units in UTF-16 (surrogate pair)
        let doc = Document::new(None, "aあb𠮷c".to_string());
        // 'a' at char 0 -> byte 0
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 0)), 0);
        // 'あ' at char 1 -> byte 1
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 1)), 1);
        // 'b' at char 2 -> byte 4 (1 + 3)
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 2)), 4);
        // '𠮷' at char 3 -> byte 5 (1 + 3 + 1)
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 3)), 5);
        // 'c' at char 5 (surrogate pair = 2 code units) -> byte 9 (1 + 3 + 1 + 4)
        assert_eq!(doc.position_to_byte_offset(Position::new(0, 5)), 9);
    }

    #[test]
    fn test_document_position_to_byte_offset_out_of_bounds() {
        let doc = Document::new(None, "hello".to_string());
        // line out of bounds -> content.len()
        assert_eq!(doc.position_to_byte_offset(Position::new(10, 0)), 5);
    }

    #[test]
    fn test_document_apply_change_insert() {
        let mut doc = Document::new(None, "hello world".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(0, 5));
        doc.apply_change(Some(range), ",");
        assert_eq!(doc.content, "hello, world");
    }

    #[test]
    fn test_document_apply_change_delete() {
        let mut doc = Document::new(None, "hello world".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(0, 6));
        doc.apply_change(Some(range), "");
        assert_eq!(doc.content, "helloworld");
    }

    #[test]
    fn test_document_apply_change_replace() {
        let mut doc = Document::new(None, "hello world".to_string());
        let range = Range::new(Position::new(0, 0), Position::new(0, 5));
        doc.apply_change(Some(range), "hi");
        assert_eq!(doc.content, "hi world");
    }

    #[test]
    fn test_document_apply_change_multiline() {
        let mut doc = Document::new(None, "line1\nline2\nline3".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(2, 0));
        doc.apply_change(Some(range), "\n");
        assert_eq!(doc.content, "line1\nline3");
    }

    #[test]
    fn test_document_apply_change_full_update() {
        let mut doc = Document::new(None, "old content".to_string());
        doc.apply_change(None, "new content");
        assert_eq!(doc.content, "new content");
    }

    #[test]
    fn test_document_reset_content() {
        let mut doc = Document::new(Some(5), "old".to_string());
        doc.changed = true;
        doc.reset_content("new".to_string());
        assert_eq!(doc.version, None);
        assert_eq!(doc.content, "new");
    }
}
