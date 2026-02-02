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
    static PARSER: RefCell<MarkdownParser> = RefCell::new(MarkdownParser::default());
}

fn rope_to_bytes(rope: &Rope) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(rope.byte_len());
    for chunk in rope.chunks() {
        bytes.extend_from_slice(chunk.as_bytes());
    }
    bytes
}

fn parse_rope(rope: &Rope, old_tree: Option<&MarkdownTree>) -> Option<MarkdownTree> {
    let bytes = rope_to_bytes(rope);
    PARSER.with(|parser| parser.borrow_mut().parse(&bytes, old_tree))
}

#[derive(Debug)]
pub struct Document {
    version: Option<i32>,
    modified: bool,
    rope: Rope,
    tree: Option<Arc<MarkdownTree>>,
}

impl Clone for Document {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            modified: self.modified,
            rope: self.rope.clone(),
            tree: self.tree.clone(),
        }
    }
}

impl Document {
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

    fn apply_change(&mut self, range: Option<Range>, text: &str) {
        if let Some(range) = range {
            let (start_position, start_byte) = self.find_canonical_position(range.start);
            let (old_end_position, old_end_byte) = self.find_canonical_position(range.end);

            if start_byte > old_end_byte || old_end_byte > self.rope.byte_len() {
                warn!(
                    "Invalid byte range: {}..{} (content length: {})",
                    start_byte,
                    old_end_byte,
                    self.rope.byte_len()
                );
                self.rope = Rope::from(text);
                self.tree = None;
                return;
            }

            let new_end_byte = start_byte + text.len();
            let new_end_position = compute_end_position(start_position, text);

            let input_edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            };

            if let Some(ref mut tree) = self.tree {
                Arc::make_mut(tree).edit(&input_edit);
            }

            self.rope.replace(start_byte..old_end_byte, text);
        } else {
            self.rope = Rope::from(text);
            self.tree = None;
        }
    }

    fn find_canonical_position(&self, position: Position) -> (Point, usize) {
        let line = position.line as usize;
        let line_count = self.rope.line_len();

        if line >= line_count {
            return (Point::new(line, 0), self.rope.byte_len());
        }

        let line_start = self.rope.byte_of_line(line);
        let line_end = if line + 1 < line_count {
            self.rope.byte_of_line(line + 1).saturating_sub(1)
        } else {
            self.rope.byte_len()
        };

        let absolute_byte_offset = (line_start + position.character as usize).min(line_end);
        let relative_byte = absolute_byte_offset - line_start;
        let point = Point::new(line, relative_byte);
        (point, absolute_byte_offset)
    }

    #[must_use]
    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    #[must_use]
    pub fn tree(&self) -> Option<&MarkdownTree> {
        self.tree.as_deref()
    }

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

        #[allow(clippy::cast_possible_truncation)]
        let end_line = end_row.min(u32::MAX as usize) as u32;
        let range = Range::new(Position::new(1, 0), Position::new(end_line, 0));

        Some((text, range))
    }

    #[must_use]
    pub fn source_bytes(&self) -> Vec<u8> {
        rope_to_bytes(&self.rope)
    }
}

fn compute_end_position(start: Point, text: &str) -> Point {
    let newline_count = text.bytes().filter(|&b| b == b'\n').count();

    if newline_count == 0 {
        Point::new(start.row, start.column + text.len())
    } else {
        let last_line_bytes = text.rsplit_once('\n').map_or(0, |(_, last)| last.len());
        Point::new(start.row + newline_count, last_line_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn proptest_config() -> ProptestConfig {
        ProptestConfig::with_cases(1000)
    }

    #[test]
    fn test_find_canonical_position_utf8() {
        let doc = Document::new(None, false, "aあb𠮷c".to_string());
        let (point, byte) = doc.find_canonical_position(Position::new(0, 0));
        assert_eq!((point.row, point.column, byte), (0, 0, 0));
        let (point, byte) = doc.find_canonical_position(Position::new(0, 4));
        assert_eq!((point.row, point.column, byte), (0, 4, 4));
        let (point, byte) = doc.find_canonical_position(Position::new(0, 9));
        assert_eq!((point.row, point.column, byte), (0, 9, 9));
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

    proptest! {
        #![proptest_config(proptest_config())]

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

    }
}
