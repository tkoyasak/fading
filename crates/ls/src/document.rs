//! Document representation with tree-sitter parsing.
//!
//! Manages document content, line offsets, and syntax tree for incremental updates.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use crop::Rope;
use log::warn;
use tower_lsp_server::ls_types::{Position, Range, TextDocumentContentChangeEvent};
use tree_sitter::{InputEdit, Point};
use tree_sitter_md::{MarkdownParser, MarkdownTree};

thread_local! {
    /// Thread-local markdown parser instance for reuse.
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

/// Parses Rope content into a markdown syntax tree, optionally reusing an old tree.
fn parse_rope(rope: &Rope, old_tree: Option<&MarkdownTree>) -> Option<MarkdownTree> {
    // Pre-allocate with exact capacity to avoid reallocation
    let mut bytes = Vec::with_capacity(rope.byte_len());
    for chunk in rope.chunks() {
        bytes.extend_from_slice(chunk.as_bytes());
    }
    PARSER.with(|parser| parser.borrow_mut().parse(&bytes, old_tree))
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
    content: Rope,
    /// Parsed tree-sitter syntax tree (Arc-wrapped for cheap cloning).
    tree: Option<Arc<MarkdownTree>>,
    /// Cached frontmatter text and range (invalidated on content change).
    frontmatter: OnceLock<Option<(String, Range)>>,
    /// Cached source bytes for tree-sitter queries (invalidated on content change).
    source_cache: OnceLock<Vec<u8>>,
}

impl Document {
    /// Creates a new document with the given version and content.
    pub fn new(version: Option<i32>, modified: bool, content: String) -> Self {
        let rope = Rope::from(content);
        let tree = parse_rope(&rope, None).map(Arc::new);

        Self {
            version,
            modified,
            content: rope,
            tree,
            frontmatter: OnceLock::new(),
            source_cache: OnceLock::new(),
        }
    }

    /// Applies a batch of text changes and updates the document version.
    ///
    /// This method applies all changes incrementally, then re-parses the syntax tree.
    /// The document is marked as modified after this operation.
    pub fn update(&mut self, new_version: i32, changes: &[TextDocumentContentChangeEvent]) {
        if let Some(version) = self.version
            && version > new_version
        {
            warn!("Out-of-sync: currently at {version}, got {new_version}");
            return;
        }

        for change in changes {
            self.apply_change(change.range, &change.text);
        }
        self.version = Some(new_version);
        self.modified = true;
        self.tree = parse_rope(&self.content, self.tree.as_deref()).map(Arc::new);
        self.frontmatter = OnceLock::new();
        self.source_cache = OnceLock::new();
    }

    /// Applies a text change to the document.
    ///
    /// If `range` is `Some`, performs an incremental update and edits the syntax tree.
    /// If `range` is `None`, replaces the entire content (full sync fallback).
    ///
    /// Uses `Arc::make_mut()` to edit the tree in-place when possible (single reference),
    /// or clone it when other references exist. This enables tree-sitter's incremental parsing.
    fn apply_change(&mut self, range: Option<Range>, text: &str) {
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

            // Replace content using Rope::replace
            self.content.replace(start_byte..old_end_byte, text);
        } else {
            // Full update (fallback)
            self.content = Rope::from(text);
            self.tree = None;
        }
    }

    /// Convert LSP Position (line, UTF-8 byte offset) to tree-sitter Point and byte offset.
    fn find_canonical_position(&self, position: &Position) -> (Point, usize) {
        let line = position.line as usize;
        let line_count = self.content.line_len();

        if line >= line_count {
            return (Point::new(line, 0), self.content.byte_len());
        }

        let line_start = self.content.byte_of_line(line);
        let line_end = if line + 1 < line_count {
            self.content.byte_of_line(line + 1).saturating_sub(1)
        } else {
            self.content.byte_len()
        };

        // With UTF-8 encoding, position.character is a byte offset within the line
        let absolute_byte_offset = (line_start + position.character as usize).min(line_end);
        let relative_byte = absolute_byte_offset - line_start;
        let point = Point::new(line, relative_byte);
        (point, absolute_byte_offset)
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
    pub fn content(&self) -> &Rope {
        &self.content
    }

    /// Returns a reference to the parsed syntax tree.
    pub fn tree(&self) -> Option<&MarkdownTree> {
        self.tree.as_deref()
    }

    /// Extracts the TOML frontmatter content and its range.
    ///
    /// Returns `None` if the document doesn't start with a `plus_metadata` node.
    ///
    /// The result is cached after first extraction to avoid repeated tree traversal.
    pub fn frontmatter(&self) -> Option<(&str, Range)> {
        self.frontmatter
            .get_or_init(|| {
                let tree = self.tree.as_ref()?;
                let root = tree.block_tree().root_node();
                let node = root.child(0)?;

                if node.kind() != "plus_metadata" {
                    return None;
                }

                let end_row = node.end_position().row.saturating_sub(1);
                let start = self.content.byte_of_line(1);
                let end = self.content.byte_of_line(end_row);

                let text: String = self.content.byte_slice(start..end).chunks().collect();

                // Range from line 1 to the closing `+++` line (exclusive)
                let range = Range::new(Position::new(1, 0), Position::new(end_row as u32, 0));

                Some((text, range))
            })
            .as_ref()
            .map(|(text, range)| (text.as_str(), *range))
    }

    /// Returns cached source bytes for tree-sitter queries.
    ///
    /// The bytes are lazily computed and cached. The cache is invalidated on content change.
    pub fn source_bytes(&self) -> &[u8] {
        self.source_cache.get_or_init(|| {
            let mut bytes = Vec::with_capacity(self.content.byte_len());
            for chunk in self.content.chunks() {
                bytes.extend_from_slice(chunk.as_bytes());
            }
            bytes
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Range;

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
        let doc = Document::new(Some(1), false, "hello\nworld".to_string());
        assert_eq!(doc.version(), Some(1));
        assert!(!doc.modified());
        assert_eq!(doc.content(), "hello\nworld");
    }

    #[test]
    fn test_find_canonical_position_ascii() {
        let doc = Document::new(None, false, "hello\nworld".to_string());
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
    fn test_find_canonical_position_utf8() {
        // UTF-8 encoding: "aあb𠮷c"
        // 'a' = 1 byte, 'あ' = 3 bytes, 'b' = 1 byte, '𠮷' = 4 bytes, 'c' = 1 byte
        // Total: byte offsets 0, 1-3, 4, 5-8, 9
        let doc = Document::new(None, false, "aあb𠮷c".to_string());
        // Byte 0 -> 'a' -> Point(0, 0), byte 0
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 0));
        assert_eq!((point.row, point.column, byte), (0, 0, 0));
        // Byte 1 -> 'あ' start -> Point(0, 1), byte 1
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 1));
        assert_eq!((point.row, point.column, byte), (0, 1, 1));
        // Byte 4 -> 'b' -> Point(0, 4), byte 4
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 4));
        assert_eq!((point.row, point.column, byte), (0, 4, 4));
        // Byte 5 -> '𠮷' start -> Point(0, 5), byte 5
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 5));
        assert_eq!((point.row, point.column, byte), (0, 5, 5));
        // Byte 9 -> 'c' -> Point(0, 9), byte 9
        let (point, byte) = doc.find_canonical_position(&Position::new(0, 9));
        assert_eq!((point.row, point.column, byte), (0, 9, 9));
    }

    #[test]
    fn test_find_canonical_position_out_of_bounds() {
        let doc = Document::new(None, false, "hello".to_string());
        // line out of bounds -> Point(10, 0), content.len()
        let (point, byte) = doc.find_canonical_position(&Position::new(10, 0));
        assert_eq!((point.row, point.column, byte), (10, 0, 5));
    }

    #[test]
    fn test_document_apply_change_insert() {
        let mut doc = Document::new(None, false, "hello world".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(0, 5));
        doc.apply_change(Some(range), ",");
        assert_eq!(doc.content(), "hello, world");
    }

    #[test]
    fn test_document_apply_change_delete() {
        let mut doc = Document::new(None, false, "hello world".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(0, 6));
        doc.apply_change(Some(range), "");
        assert_eq!(doc.content(), "helloworld");
    }

    #[test]
    fn test_document_apply_change_replace() {
        let mut doc = Document::new(None, false, "hello world".to_string());
        let range = Range::new(Position::new(0, 0), Position::new(0, 5));
        doc.apply_change(Some(range), "hi");
        assert_eq!(doc.content(), "hi world");
    }

    #[test]
    fn test_document_apply_change_multiline() {
        let mut doc = Document::new(None, false, "line1\nline2\nline3".to_string());
        let range = Range::new(Position::new(0, 5), Position::new(2, 0));
        doc.apply_change(Some(range), "\n");
        assert_eq!(doc.content(), "line1\nline3");
    }

    #[test]
    fn test_document_apply_change_full_update() {
        let mut doc = Document::new(None, false, "old content".to_string());
        doc.apply_change(None, "new content");
        assert_eq!(doc.content(), "new content");
    }

    #[test]
    fn test_document_frontmatter() {
        let doc = Document::new(
            None,
            false,
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\nContent"
                .to_string(),
        );
        let (fm, _) = doc.frontmatter().unwrap();
        assert!(fm.contains("id = \"2026-01\""));
        assert!(fm.contains("created = 2026-01-01"));
        assert!(fm.contains("modified = 2026-01-15"));
    }

    #[test]
    fn test_document_frontmatter_none_without_plus_metadata() {
        let doc = Document::new(None, false, "# Hello\n\nNo frontmatter.".to_string());
        assert!(doc.frontmatter().is_none());
    }

    #[test]
    fn test_document_frontmatter_none_unclosed() {
        let doc = Document::new(None, false, "+++\nid = \"test\"\n# No closing".to_string());
        assert!(doc.frontmatter().is_none());
    }
}
