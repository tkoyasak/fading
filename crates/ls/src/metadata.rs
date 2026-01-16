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
