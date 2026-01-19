//! Document representation with tree-sitter parsing.
//!
//! Manages document content, line offsets, and syntax tree for incremental updates.

use std::cell::RefCell;
use std::sync::Arc;

use tower_lsp_server::ls_types::{Position, Range};
use tree_sitter::{InputEdit, Point};
use tree_sitter_md::{MarkdownParser, MarkdownTree};

thread_local! {
    /// Thread-local markdown parser instance for reuse.
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

/// A text document with associated metadata and syntax tree.
///
/// The syntax tree is wrapped in `Arc` to make cloning cheap (pointer copy only).
/// This is important for concurrent access patterns like `papaya::HashMap::update()`.
#[derive(Debug, Clone)]
pub struct Document {
    /// LSP document version, `None` after save.
    version: Option<i32>,
    /// Whether the document has been modified during this session.
    modified: bool,
    /// The document content.
    content: String,
    /// Byte offsets for the start of each line.
    line_offsets: Vec<usize>,
    /// Parsed tree-sitter syntax tree (Arc-wrapped for cheap cloning).
    tree: Option<Arc<MarkdownTree>>,
}

impl Document {
    /// Creates a new document with the given version and content.
    pub fn new(version: Option<i32>, content: String) -> Self {
        let line_offsets = compute_line_offsets(&content);
        let tree = parse(&content, None).map(Arc::new);
        Self {
            version,
            modified: false,
            content,
            line_offsets,
            tree,
        }
    }

    /// Applies a text change to the document.
    ///
    /// If `range` is `Some`, performs an incremental update and edits the syntax tree.
    /// If `range` is `None`, replaces the entire content (full sync fallback).
    ///
    /// Uses `Arc::make_mut()` to edit the tree in-place when possible (single reference),
    /// or clone it when other references exist. This enables tree-sitter's incremental parsing.
    pub fn apply_change(&mut self, range: Option<Range>, text: &str) {
        if let Some(range) = range {
            let (start_position, start_byte) = self.find_canonical_position(&range.start);
            let (old_end_position, old_end_byte) = self.find_canonical_position(&range.end);
            let new_end_byte = start_byte + text.len();
            let new_end_position = compute_end_position(start_position, text);

            // Create InputEdit for tree-sitter incremental parsing
            let input_edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            };

            // Edit tree using Arc::make_mut (clones only if other refs exist)
            if let Some(ref mut tree) = self.tree {
                Arc::make_mut(tree).edit(&input_edit);
            }

            // Build new content
            let mut new_content = String::with_capacity(
                self.content.len() - (old_end_byte - start_byte) + text.len(),
            );
            new_content.push_str(&self.content[..start_byte]);
            new_content.push_str(text);
            new_content.push_str(&self.content[old_end_byte..]);

            self.content = new_content;
            self.line_offsets = compute_line_offsets(&self.content);
        } else {
            // Full update (fallback)
            self.content = text.to_string();
            self.line_offsets = compute_line_offsets(&self.content);
            self.tree = None;
        }
    }

    /// Re-parses the syntax tree and updates the version.
    ///
    /// Call this after applying changes to finalize the document state.
    pub fn update(&mut self, version: Option<i32>) {
        self.tree = parse(&self.content, self.tree.as_deref()).map(Arc::new);
        self.version = version;
        self.modified = true;
    }

    /// Resets the document content, typically after a save.
    ///
    /// Clears the version and re-parses from scratch.
    pub fn reset_content(&mut self, content: String) {
        self.version = None;
        self.content = content;
        self.line_offsets = compute_line_offsets(&self.content);
        self.tree = parse(&self.content, None).map(Arc::new);
    }

    /// Extracts the TOML frontmatter content (without the `+++` delimiters).
    ///
    /// Returns `None` if the document doesn't start with a `plus_metadata` node.
    pub fn frontmatter(&self) -> Option<&str> {
        let tree = self.tree.as_ref()?;
        let root = tree.block_tree().root_node();
        let node = root.child(0)?;

        if node.kind() != "plus_metadata" {
            return None;
        }

        let start = *self.line_offsets.get(1)?;
        let end = *self
            .line_offsets
            .get(node.end_position().row.saturating_sub(1))?;
        self.content.get(start..end)
    }

    /// Returns the document version.
    pub fn version(&self) -> Option<i32> {
        self.version
    }

    /// Returns whether the document has been modified.
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Returns a reference to the document content.
    pub fn content(&self) -> &str {
        self.content.as_ref()
    }

    /// Returns a reference to the parsed syntax tree.
    pub fn tree(&self) -> Option<&MarkdownTree> {
        self.tree.as_deref()
    }

    /// Convert LSP Position (line, UTF-16 character) to tree-sitter Point and byte offset.
    fn find_canonical_position(&self, position: &Position) -> (Point, usize) {
        let line = position.line as usize;
        if line >= self.line_offsets.len() {
            return (Point::new(line, 0), self.content.len());
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

        let point = Point::new(line, byte_offset);
        let absolute_byte_offset = line_start + byte_offset;
        (point, absolute_byte_offset)
    }
}

/// Computes the end position after inserting text at a given start position.
fn compute_end_position(start: Point, text: &str) -> Point {
    let mut row = start.row;
    let mut col = start.column;

    for ch in text.chars() {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += ch.len_utf8();
        }
    }

    Point::new(row, col)
}

/// Computes byte offsets for the start of each line.
fn compute_line_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Parses content into a markdown syntax tree, optionally reusing an old tree.
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
        let start = Point::new(0, 0);
        let point = compute_end_position(start, "hello");
        assert_eq!(point.row, 0);
        assert_eq!(point.column, 5);
    }

    #[test]
    fn test_compute_end_position_with_newline() {
        let start = Point::new(0, 0);
        let point = compute_end_position(start, "hello\nworld");
        assert_eq!(point.row, 1);
        assert_eq!(point.column, 5);
    }

    #[test]
    fn test_compute_end_position_from_middle() {
        let start = Point::new(2, 5);
        let point = compute_end_position(start, "ab\ncd");
        assert_eq!(point.row, 3);
        assert_eq!(point.column, 2);
    }

    #[test]
    fn test_document_new() {
        let doc = Document::new(Some(1), "hello\nworld".to_string());
        assert_eq!(doc.version(), Some(1));
        assert!(!doc.modified());
        assert_eq!(doc.content(), "hello\nworld");
    }

    #[test]
    fn test_find_canonical_position_ascii() {
        let doc = Document::new(None, "hello\nworld".to_string());
        // line 0, char 0 -> Point(0, 0), byte 0
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 0));
        assert_eq!((point.row, point.column, byte), (0, 0, 0));
        // line 0, char 5 -> Point(0, 5), byte 5
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 5));
        assert_eq!((point.row, point.column, byte), (0, 5, 5));
        // line 1, char 0 -> Point(1, 0), byte 6
        let (point, byte) = doc.find_canonical_position(&Position::new(1, 0));
        assert_eq!((point.row, point.column, byte), (1, 0, 6));
        // line 1, char 3 -> Point(1, 3), byte 9
        let (point, byte) = doc.find_canonical_position(&Position::new(1, 3));
        assert_eq!((point.row, point.column, byte), (1, 3, 9));
    }

    #[test]
    fn test_find_canonical_position_utf16() {
        // "あ" is 3 bytes in UTF-8, 1 code unit in UTF-16
        // "𠮷" (U+20BB7) is 4 bytes in UTF-8, 2 code units in UTF-16 (surrogate pair)
        let doc = Document::new(None, "aあb𠮷c".to_string());
        // 'a' at char 0 -> Point(0, 0), byte 0
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 0));
        assert_eq!((point.row, point.column, byte), (0, 0, 0));
        // 'あ' at char 1 -> Point(0, 1), byte 1
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 1));
        assert_eq!((point.row, point.column, byte), (0, 1, 1));
        // 'b' at char 2 -> Point(0, 4), byte 4 (1 + 3)
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 2));
        assert_eq!((point.row, point.column, byte), (0, 4, 4));
        // '𠮷' at char 3 -> Point(0, 5), byte 5 (1 + 3 + 1)
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 3));
        assert_eq!((point.row, point.column, byte), (0, 5, 5));
        // 'c' at char 5 (surrogate pair = 2 code units) -> Point(0, 9), byte 9 (1 + 3 + 1 + 4)
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 5));
        assert_eq!((point.row, point.column, byte), (0, 9, 9));
    }

    #[test]
    fn test_find_canonical_position_out_of_bounds() {
        let doc = Document::new(None, "hello".to_string());
        // line out of bounds -> Point(10, 0), content.len()
        let (point, byte) = doc.find_canonical_position(&Position::new(10, 0));
        assert_eq!((point.row, point.column, byte), (10, 0, 5));
    }

    #[test]
    fn test_document_apply_change_insert() {
        let mut doc = Document::new(None, "hello world".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(0, 5));
        doc.apply_change(Some(range), ",");
        assert_eq!(doc.content(), "hello, world");
    }

    #[test]
    fn test_document_apply_change_delete() {
        let mut doc = Document::new(None, "hello world".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(0, 6));
        doc.apply_change(Some(range), "");
        assert_eq!(doc.content(), "helloworld");
    }

    #[test]
    fn test_document_apply_change_replace() {
        let mut doc = Document::new(None, "hello world".to_string());
        let range = Range::new(Position::new(0, 0), Position::new(0, 5));
        doc.apply_change(Some(range), "hi");
        assert_eq!(doc.content(), "hi world");
    }

    #[test]
    fn test_document_apply_change_multiline() {
        let mut doc = Document::new(None, "line1\nline2\nline3".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(2, 0));
        doc.apply_change(Some(range), "\n");
        assert_eq!(doc.content(), "line1\nline3");
    }

    #[test]
    fn test_document_apply_change_full_update() {
        let mut doc = Document::new(None, "old content".to_string());
        doc.apply_change(None, "new content");
        assert_eq!(doc.content(), "new content");
    }

    #[test]
    fn test_document_reset_content() {
        let mut doc = Document::new(Some(5), "old".to_string());
        doc.apply_change(None, "old");
        doc.update(Some(5)); // sets modified = true
        doc.reset_content("new".to_string());
        assert_eq!(doc.version(), None);
        assert!(doc.modified()); // modified is preserved
        assert_eq!(doc.content(), "new");
    }

    #[test]
    fn test_document_frontmatter() {
        let doc = Document::new(
            None,
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\nContent"
                .to_string(),
        );
        let fm = doc.frontmatter().unwrap();
        assert!(fm.contains("id = \"2026-01\""));
        assert!(fm.contains("created = 2026-01-01"));
        assert!(fm.contains("modified = 2026-01-15"));
    }

    #[test]
    fn test_document_frontmatter_none_without_plus_metadata() {
        let doc = Document::new(None, "# Hello\n\nNo frontmatter.".to_string());
        assert!(doc.frontmatter().is_none());
    }

    #[test]
    fn test_document_frontmatter_none_unclosed() {
        let doc = Document::new(None, "+++\nid = \"test\"\n# No closing".to_string());
        assert!(doc.frontmatter().is_none());
    }
}
