use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use tower_lsp_server::ls_types::{CodeActionKind, Position, Range, TextEdit};

use crate::document::Document;

pub const CODE_ACTION_UPDATE_METADATA: CodeActionKind =
    CodeActionKind::new("source.updateMetadata.fading");

#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    id: String,
    created: toml::value::Date,
    modified: toml::value::Date,
}

pub fn update_metadata(doc: &Document) -> Option<Vec<TextEdit>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str, changed: bool) -> Document {
        let mut doc = Document::new(None, content.to_string());
        doc.changed = changed;
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
        let today = chrono::Local::now().date_naive();
        let expected_date = format!("{}-{:02}-{:02}", today.year(), today.month(), today.day());
        assert!(edit.new_text.contains(&expected_date));
    }

    #[test]
    fn test_update_metadata_already_today() {
        let today = chrono::Local::now().date_naive();
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
}
