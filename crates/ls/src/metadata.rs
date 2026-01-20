//! Frontmatter metadata handling.
//!
//! Provides code actions to automatically update the `modified` date in TOML frontmatter.

use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use tower_lsp_server::ls_types::{CodeActionKind, TextEdit};

use crate::document::Document;

/// Code action kind for updating metadata.
pub const CODE_ACTION_UPDATE_METADATA: CodeActionKind =
    CodeActionKind::new("source.updateMetadata.fading");

/// TOML frontmatter structure.
///
/// Only these three fields are recognized; any extra fields are stripped on update.
#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    /// Entry identifier (e.g., "2026-01").
    id: String,
    /// Date when the entry was created.
    created: toml::value::Date,
    /// Date when the entry was last modified.
    modified: toml::value::Date,
}

/// Generates text edits to update the `modified` date to today.
///
/// Returns `None` if:
/// - The document hasn't been modified this session
/// - The document has no valid frontmatter
/// - The `modified` date is already today
pub fn update_metadata(doc: &Document) -> Option<Vec<TextEdit>> {
    if !doc.modified() {
        return None;
    }

    let (fm, range) = doc.frontmatter()?;
    let mut metadata = toml::from_str::<Metadata>(fm).ok()?;

    let d = Local::now().date_naive();
    let today = toml::value::Date {
        year: d.year() as u16,
        month: d.month() as u8,
        day: d.day() as u8,
    };

    if metadata.modified == today {
        return None;
    }

    metadata.modified = today;
    let new_text = toml::to_string(&metadata).unwrap();

    Some(vec![TextEdit::new(range, new_text)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Position;

    fn make_doc(content: &str, modified: bool) -> Document {
        let mut doc = Document::new(None, content.to_string());
        if modified {
            doc.apply_change(None, content);
            doc.update(None);
        }
        doc
    }

    #[test]
    fn test_update_metadata_not_changed() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-01\n+++\n",
            false,
        );
        assert!(update_metadata(&doc).is_none());
    }

    #[test]
    fn test_update_metadata_no_frontmatter() {
        let doc = make_doc("# Hello\n\nNo frontmatter here.", true);
        assert!(update_metadata(&doc).is_none());
    }

    #[test]
    fn test_update_metadata_invalid_frontmatter_no_closing() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-01\n# No closing",
            true,
        );
        assert!(update_metadata(&doc).is_none());
    }

    #[test]
    fn test_update_metadata_invalid_toml() {
        let doc = make_doc("+++\ninvalid toml\nno equals\nhere\n+++\n", true);
        assert!(update_metadata(&doc).is_none());
    }

    #[test]
    fn test_update_metadata_missing_modified() {
        let doc = make_doc("+++\nid = \"2026-01\"\ncreated = 2026-01-01\n+++\n", true);
        assert!(update_metadata(&doc).is_none());
    }

    #[test]
    fn test_update_metadata_updates_modified_date() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\n+++\n\nContent",
            true,
        );

        let result = update_metadata(&doc);
        assert!(result.is_some());

        let edits = result.unwrap();
        assert_eq!(edits.len(), 1);

        let edit = &edits[0];
        assert_eq!(edit.range.start, Position::new(1, 0));
        assert_eq!(edit.range.end, Position::new(4, 0));

        // Check that the new text contains today's date
        let today = Local::now().date_naive();
        let expected_date = format!("{}-{:02}-{:02}", today.year(), today.month(), today.day());
        assert!(edit.new_text.contains(&expected_date));
    }

    #[test]
    fn test_update_metadata_already_today() {
        let today = Local::now().date_naive();
        let content = format!(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = {}-{:02}-{:02}\n+++\n",
            today.year(),
            today.month(),
            today.day()
        );
        let doc = make_doc(&content, true);

        // Should return None because modified is already today
        assert!(update_metadata(&doc).is_none());
    }

    #[test]
    fn test_update_metadata_strips_extra_fields() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\ntags = [\"rust\", \"lsp\"]\n+++\n",
            true,
        );

        let result = update_metadata(&doc);
        assert!(result.is_some());

        let edit = &result.unwrap()[0];
        // Extra fields should be stripped
        assert!(!edit.new_text.contains("tags"));
        assert!(edit.new_text.contains("id = \"2026-01\""));
    }
}
