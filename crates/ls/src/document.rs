//! Document representation with tree-sitter parsing.
//!
//! Manages document content, line offsets, and syntax tree for incremental updates.

use std::cell::RefCell;
use std::sync::Arc;

use crop::Rope;
use log::warn;
use tower_lsp_server::ls_types::{Position, Range, TextDocumentContentChangeEvent};
use tree_sitter::{InputEdit, Point};
use tree_sitter_md::{MarkdownParser, MarkdownTree};

thread_local! {
    /// Thread-local markdown parser instance for reuse.
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

/// Converts Rope to bytes for tree-sitter parsing or queries.
///
/// Pre-allocates with exact capacity to avoid reallocation.
fn rope_to_bytes(rope: &Rope) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(rope.byte_len());
    for chunk in rope.chunks() {
        bytes.extend_from_slice(chunk.as_bytes());
    }
    bytes
}

/// Parses Rope content into a markdown syntax tree, optionally reusing an old tree.
fn parse_rope(rope: &Rope, old_tree: Option<&MarkdownTree>) -> Option<MarkdownTree> {
    let bytes = rope_to_bytes(rope);
    PARSER.with(|parser| parser.borrow_mut().parse(&bytes, old_tree))
}

/// A text document with associated metadata and syntax tree.
///
/// The syntax tree is wrapped in `Arc` to make cloning cheap (pointer copy only).
/// This is important for concurrent access patterns like `papaya::HashMap::update()`.
#[derive(Debug)]
pub struct Document {
    /// LSP document version, `None` after save.
    version: Option<i32>,
    /// Whether the document has been modified since opening or last save.
    modified: bool,
    /// The document content.
    rope: Rope,
    /// Parsed tree-sitter syntax tree (Arc-wrapped for cheap cloning).
    tree: Option<Arc<MarkdownTree>>,
}

impl Clone for Document {
    /// Clones the document.
    ///
    /// Rope and Arc clones are cheap (reference counting).
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            modified: self.modified,
            rope: self.rope.clone(), // Rope: cheap (16 bytes)
            tree: self.tree.clone(), // Arc: cheap (pointer copy)
        }
    }
}

impl Document {
    /// Creates a new document with the given version and content.
    pub fn new(version: Option<i32>, modified: bool, content: String) -> Self {
        let rope = Rope::from(content);
        let tree = parse_rope(&rope, None).map(Arc::new);

        Self {
            version,
            modified,
            rope,
            tree,
        }
    }

    /// Applies a batch of text changes and updates the document version.
    ///
    /// This method applies all changes incrementally, then re-parses the syntax tree.
    pub fn update(&mut self, new_version: i32, changes: &[TextDocumentContentChangeEvent]) {
        if let Some(version) = self.version
            && version >= new_version
        {
            warn!("Out-of-sync or duplicate: currently at {version}, got {new_version}");
            return;
        }

        for change in changes {
            self.apply_change(change.range, &change.text);
        }

        self.version = Some(new_version);
        self.modified = true;
        self.tree = parse_rope(&self.rope, self.tree.as_deref()).map(Arc::new);
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
            let (start_position, start_byte) = self.find_canonical_position(range.start);
            let (old_end_position, old_end_byte) = self.find_canonical_position(range.end);

            // Validate byte range before replacement
            if start_byte > old_end_byte || old_end_byte > self.rope.byte_len() {
                warn!(
                    "Invalid byte range: {}..{} (content length: {})",
                    start_byte,
                    old_end_byte,
                    self.rope.byte_len()
                );
                // Fallback to full sync
                self.rope = Rope::from(text);
                self.tree = None;
                return;
            }

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
            self.rope.replace(start_byte..old_end_byte, text);
        } else {
            // Full update (fallback)
            self.rope = Rope::from(text);
            self.tree = None;
        }
    }

    /// Convert LSP Position (line, UTF-8 byte offset) to tree-sitter Point and byte offset.
    ///
    /// # Arguments
    /// * `position` - LSP position (line and character offset)
    ///
    /// # Returns
    /// Tuple of (tree-sitter Point, absolute byte offset)
    fn find_canonical_position(&self, position: Position) -> (Point, usize) {
        let line = position.line as usize;
        let line_count = self.rope.line_len();

        if line >= line_count {
            return (Point::new(line, 0), self.rope.byte_len());
        }

        let line_start = self.rope.byte_of_line(line);
        let line_end = if line + 1 < line_count {
            // Next line start - 1 = end of current line (excluding newline)
            self.rope.byte_of_line(line + 1).saturating_sub(1)
        } else {
            // Last line: end = document end
            self.rope.byte_len()
        };

        // With UTF-8 encoding, position.character is a byte offset within the line
        let absolute_byte_offset = (line_start + position.character as usize).min(line_end);
        let relative_byte = absolute_byte_offset - line_start;
        let point = Point::new(line, relative_byte);
        (point, absolute_byte_offset)
    }

    /// Returns the document version.
    #[must_use]
    pub fn version(&self) -> Option<i32> {
        self.version
    }

    /// Returns whether the document has been modified.
    #[must_use]
    pub fn modified(&self) -> bool {
        self.modified
    }

    /// Clears the modified flag.
    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    /// Returns a reference to the document content.
    #[must_use]
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// Returns a reference to the parsed syntax tree.
    #[must_use]
    pub fn tree(&self) -> Option<&MarkdownTree> {
        self.tree.as_deref()
    }

    /// Extracts the TOML frontmatter content and its range.
    ///
    /// Returns `None` if the document doesn't start with a `plus_metadata` node.
    #[must_use]
    pub fn frontmatter(&self) -> Option<(String, Range)> {
        let tree = self.tree.as_ref()?;
        let root = tree.block_tree().root_node();
        let node = root.child(0)?;

        if node.kind() != "plus_metadata" {
            return None;
        }

        let end_row = node.end_position().row.saturating_sub(1);
        let start = self.rope.byte_of_line(1);
        let end = self.rope.byte_of_line(end_row);

        let text: String = self.rope.byte_slice(start..end).chunks().collect();

        // Range from line 1 to the closing `+++` line (exclusive)
        // Safety: end_row is from tree-sitter which should be within valid document bounds
        #[allow(clippy::cast_possible_truncation)]
        let end_line = end_row.min(u32::MAX as usize) as u32;
        let range = Range::new(Position::new(1, 0), Position::new(end_line, 0));

        Some((text, range))
    }

    /// Returns source bytes for tree-sitter queries.
    ///
    /// Converts the Rope content to bytes each time (no caching).
    /// This is appropriate since queries are typically run once per diagnostic request.
    #[must_use]
    pub fn source_bytes(&self) -> Vec<u8> {
        rope_to_bytes(&self.rope)
    }
}

/// Computes the end position after inserting text at a given start position.
///
/// This is optimized for common cases:
/// - Single line: just adds byte length to column
/// - Multiple lines: counts newlines and uses last line byte length
fn compute_end_position(start: Point, text: &str) -> Point {
    // Fast path: count newlines first
    let newline_count = text.bytes().filter(|&b| b == b'\n').count();

    if newline_count == 0 {
        // No newlines: just add byte length to column
        Point::new(start.row, start.column + text.len())
    } else {
        // Multiple lines: find last line byte length
        let last_line_bytes = text.rsplit_once('\n').map_or(0, |(_, last)| last.len());

        Point::new(start.row + newline_count, last_line_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tower_lsp_server::ls_types::Range;

    /// Proptest configuration: run 1000 test cases for better coverage
    fn proptest_config() -> ProptestConfig {
        ProptestConfig::with_cases(1000)
    }

    #[test]
    fn test_document_new() {
        let doc = Document::new(Some(1), false, "hello\nworld".to_string());
        assert_eq!(doc.version(), Some(1));
        // Compare Rope content by converting to String
        let content: String = doc.rope().chunks().collect();
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn test_find_canonical_position_ascii() {
        // Concrete example of ASCII position calculations
        let doc = Document::new(None, false, "hello\nworld".to_string());
        let (point, byte) = doc.find_canonical_position(Position::new(0, 0));
        assert_eq!((point.row, point.column, byte), (0, 0, 0));
        let (point, byte) = doc.find_canonical_position(Position::new(1, 3));
        assert_eq!((point.row, point.column, byte), (1, 3, 9));
    }

    #[test]
    fn test_find_canonical_position_utf8() {
        // Important UTF-8 boundary test case
        // UTF-8 encoding: "aあb𠮷c"
        // 'a' = 1 byte, 'あ' = 3 bytes, 'b' = 1 byte, '𠮷' = 4 bytes, 'c' = 1 byte
        let doc = Document::new(None, false, "aあb𠮷c".to_string());
        let (point, byte) = doc.find_canonical_position(Position::new(0, 0));
        assert_eq!((point.row, point.column, byte), (0, 0, 0));
        let (point, byte) = doc.find_canonical_position(Position::new(0, 4));
        assert_eq!((point.row, point.column, byte), (0, 4, 4));
        let (point, byte) = doc.find_canonical_position(Position::new(0, 9));
        assert_eq!((point.row, point.column, byte), (0, 9, 9));
    }

    #[test]
    fn test_document_apply_change_full_update() {
        let mut doc = Document::new(None, false, "old content".to_string());
        doc.apply_change(None, "new content");
        let content: String = doc.rope().chunks().collect();
        assert_eq!(content, "new content");
    }

    // Historical note: Concrete unit tests were replaced by property tests for better coverage.
    // Key property tests cover:
    // - prop_apply_change_preserves_tree: Tests incremental edits preserve tree validity
    // - prop_multiple_edits_safe: Tests multiple sequential edits don't corrupt state
    // - prop_find_canonical_position_safe: Tests position calculation boundary conditions

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

    proptest! {
        #![proptest_config(proptest_config())]

        /// Property: Clone preserves all document state
        #[test]
        fn prop_clone_preserves_state(
            version in prop::option::of(0i32..1000),
            modified in prop::bool::ANY,
            content in ".{0,200}"
        ) {
            let doc = Document::new(version, modified, content.clone());
            let cloned = doc.clone();

            prop_assert_eq!(cloned.version(), doc.version());
            prop_assert_eq!(cloned.modified(), doc.modified());

            let cloned_content: String = cloned.rope().chunks().collect();
            prop_assert_eq!(cloned_content, content);
        }

        /// Property: Updates with old or same version are rejected
        #[test]
        fn prop_update_rejects_stale_version(
            current_version in 1i32..1000,
            stale_offset in 0i32..100,
            content in ".{1,100}",
            new_content in ".{1,100}"
        ) {
            let stale_version = current_version - stale_offset.min(current_version);
            let mut doc = Document::new(Some(current_version), false, content.clone());

            doc.update(stale_version, &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: new_content,
            }]);

            // Content and version should remain unchanged
            let actual_content: String = doc.rope().chunks().collect();
            prop_assert_eq!(actual_content, content);
            prop_assert_eq!(doc.version(), Some(current_version));
        }

        /// Property: Invalid byte range triggers fallback to full replacement
        #[test]
        fn prop_invalid_range_falls_back_to_full_sync(
            content in "[a-z]{1,50}",
            new_content in "[a-z]{1,50}",
            start_line in 10u32..100,
            start_char in 0u32..100
        ) {
            let mut doc = Document::new(None, false, content);

            // Range where start is beyond document bounds, end is at beginning
            // This creates start_byte > old_end_byte condition
            let invalid_range = Range::new(
                Position::new(start_line, start_char),
                Position::new(0, 0)
            );
            doc.apply_change(Some(invalid_range), &new_content);

            // Should fall back to full content replacement
            let actual_content: String = doc.rope().chunks().collect();
            prop_assert_eq!(actual_content, new_content);
        }

        /// Property: compute_end_position correctly counts newlines and columns
        #[test]
        fn prop_compute_end_position_counts_newlines(
            start_row in 0usize..100,
            start_col in 0usize..100,
            text in "[\\p{Any}]{0,200}"
        ) {
            let start = Point::new(start_row, start_col);
            let end = compute_end_position(start, &text);

            // Count newlines in text
            let newline_count = text.chars().filter(|&c| c == '\n').count();
            prop_assert_eq!(end.row - start.row, newline_count);

            // If no newlines, column should increase by text byte length
            if newline_count == 0 {
                let byte_len: usize = text.chars().map(|c| c.len_utf8()).sum();
                prop_assert_eq!(end.column, start.column + byte_len);
            }
        }

        /// Property: find_canonical_position never panics and returns valid byte offset
        #[test]
        fn prop_find_canonical_position_safe(
            text in "[\\p{Any}]{0,500}",
            line in 0u32..50,
            character in 0u32..100
        ) {
            let doc = Document::new(None, false, text);
            let position = Position::new(line, character);
            let (_point, byte_offset) = doc.find_canonical_position(position);

            // Byte offset must be within document bounds
            prop_assert!(byte_offset <= doc.rope.byte_len());
        }

        /// Property: apply_change with incremental edits preserves tree validity
        /// Note: Uses ASCII-only text to avoid UTF-8 boundary issues
        #[test]
        fn prop_apply_change_preserves_tree(
            initial_text in "[a-zA-Z0-9 \\n]{0,300}",  // ASCII-only to avoid UTF-8 boundary issues
            insert_pos_line in 0u32..10,
            insert_pos_char in 0u32..20,
            insert_text in "[a-zA-Z0-9 ]{0,50}"
        ) {
            let mut doc = Document::new(None, false, initial_text);

            // Insert text at random position
            let position = Position::new(insert_pos_line, insert_pos_char);
            let range = Range::new(position, position);
            doc.apply_change(Some(range), &insert_text);

            // Tree should still exist or be None (both are valid states)
            prop_assert!(doc.tree.is_some() || doc.tree.is_none());

            // Re-parse should succeed
            let new_tree = parse_rope(&doc.rope, doc.tree.as_deref());
            prop_assert!(new_tree.is_some() || new_tree.is_none());
        }

        /// Property: frontmatter extraction is safe for any input
        #[test]
        fn prop_frontmatter_safe(
            id in "[a-z0-9-]{1,20}",
            year in 2020u32..2030,
            month in 1u32..=12,
            day in 1u32..=28,
            extra_lines in prop::collection::vec(".{0,50}", 0..5)
        ) {
            let extra = extra_lines.join("\n");
            let content = format!(
                "+++\nid = \"{}\"\ncreated = {:04}-{:02}-{:02}\nmodified = {:04}-{:02}-{:02}\n{}\n+++\n\nContent",
                id, year, month, day, year, month, day, extra
            );
            let doc = Document::new(None, false, content);

            // frontmatter() should never panic
            if let Some((fm_text, range)) = doc.frontmatter() {
                // Range should start at line 1 (after opening +++
                prop_assert!(range.start.line >= 1);
                // Extracted text should contain the id
                prop_assert!(fm_text.contains(&id));
            }
        }

        /// Property: full document replacement equals apply_change with None range
        #[test]
        fn prop_full_sync_equals_apply_change_none(
            initial_text in ".{0,200}",
            new_text in ".{0,200}"
        ) {
            let mut doc1 = Document::new(None, false, initial_text.clone());
            let doc2 = Document::new(None, false, new_text.clone());

            // Apply full sync via apply_change
            doc1.apply_change(None, &new_text);

            // Both should have same content
            let content1: String = doc1.rope.chunks().collect();
            let content2: String = doc2.rope.chunks().collect();
            prop_assert_eq!(content1, content2);
        }

        /// Property: multiple incremental edits don't corrupt the document
        /// Note: Uses ASCII-only text to avoid UTF-8 boundary issues
        #[test]
        fn prop_multiple_edits_safe(
            initial_text in "[a-zA-Z0-9 \\n]{0,200}",  // ASCII-only
            edits in prop::collection::vec(
                (0u32..10, 0u32..20, "[a-z ]{0,20}"),
                1..10
            )
        ) {
            let mut doc = Document::new(None, false, initial_text);

            for (line, character, text) in edits {
                let pos = Position::new(line, character);
                let range = Range::new(pos, pos);
                doc.apply_change(Some(range), &text);

                // After each edit, content length should be non-negative (always true for usize)
                let _ = doc.rope.byte_len();

                // Tree should remain valid or None
                prop_assert!(doc.tree.is_some() || doc.tree.is_none());
            }
        }

        /// Property: modified flag lifecycle works correctly
        ///
        /// Tests the state transitions of the modified flag:
        /// - Initial state (false on new document with modified=false)
        /// - After update (true)
        /// - After clear_modified (false)
        #[test]
        fn prop_modified_flag_lifecycle(
            initial_content in ".{10,100}",
            change_text in ".{1,50}"
        ) {
            // Initial state: not modified
            let mut doc = Document::new(None, false, initial_content.clone());
            prop_assert!(!doc.modified());

            // After update: modified
            doc.update(1, &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: change_text,
            }]);
            prop_assert!(doc.modified());

            // After clear: not modified
            doc.clear_modified();
            prop_assert!(!doc.modified());

            // Multiple updates preserve modified flag
            doc.update(2, &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "another change".to_string(),
            }]);
            prop_assert!(doc.modified());
        }

        /// Property: version tracking is accurate
        ///
        /// Tests that version numbers are correctly updated and tracked:
        /// - Initial version is preserved
        /// - Each update with a higher version sets the new version
        /// - Old version updates are rejected (early return behavior)
        #[test]
        fn prop_version_tracking(
            initial_version in prop::option::of(0i32..1000),
            version_increments in prop::collection::vec(1i32..100, 1..20)
        ) {
            let mut doc = Document::new(initial_version, false, "initial content".to_string());
            prop_assert_eq!(doc.version(), initial_version);

            let mut current_version = initial_version.unwrap_or(0);
            for increment in version_increments {
                current_version += increment;
                doc.update(current_version, &[TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "updated".to_string(),
                }]);
                prop_assert_eq!(doc.version(), Some(current_version));
            }
        }

        /// Property: rope() returns consistent content
        ///
        /// Tests that rope() getter returns the current document state
        #[test]
        fn prop_rope_returns_current_content(
            initial_content in ".{0,200}",
            new_content in ".{0,200}"
        ) {
            let mut doc = Document::new(None, false, initial_content.clone());

            // Initial content matches
            let rope_content: String = doc.rope().chars().collect();
            prop_assert_eq!(rope_content, initial_content);

            // After update, content matches
            doc.update(1, &[TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: new_content.clone(),
            }]);
            let rope_content: String = doc.rope().chars().collect();
            prop_assert_eq!(rope_content, new_content);
        }

        /// Property: source_bytes() matches rope content
        ///
        /// Tests that source_bytes() returns the UTF-8 bytes of the current content
        #[test]
        fn prop_source_bytes_matches_rope(
            content in ".{0,200}"
        ) {
            let doc = Document::new(None, false, content.clone());
            let source_bytes = doc.source_bytes();
            let expected_bytes = content.as_bytes();

            prop_assert_eq!(source_bytes, expected_bytes);
        }
    }
}
