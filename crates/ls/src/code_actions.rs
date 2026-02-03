//! Code actions for fading documents.
//!
//! Provides code actions to automatically update the `modified` date in TOML frontmatter.

use jiff::{Zoned, civil::Date};
use serde::{Deserialize, Serialize};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, TextEdit, Uri, WorkspaceEdit,
};

use crate::document::Document;

pub fn supported_action_kinds() -> Vec<CodeActionKind> {
    get_providers()
        .into_iter()
        .map(|p| p.action_kind())
        .collect()
}

pub fn code_actions(doc: &Document, uri: &Uri) -> Option<Vec<CodeActionOrCommand>> {
    let mut actions = Vec::new();

    for provider in get_providers() {
        if let Some(mut provider_actions) = provider.provide(doc, uri) {
            actions.append(&mut provider_actions);
        }
    }

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

pub fn should_clear_modified(doc: &Document) -> bool {
    doc.frontmatter()
        .and_then(|(fm, _)| toml::from_str::<Metadata>(&fm).ok())
        .is_some_and(|metadata| metadata.modified == today())
}

fn get_providers() -> Vec<Box<dyn CodeActionProvider>> {
    vec![Box::new(MetadataProvider)]
}

trait CodeActionProvider {
    fn provide(&self, doc: &Document, uri: &Uri) -> Option<Vec<CodeActionOrCommand>>;
    fn action_kind(&self) -> CodeActionKind;
}

struct MetadataProvider;

impl CodeActionProvider for MetadataProvider {
    fn provide(&self, doc: &Document, uri: &Uri) -> Option<Vec<CodeActionOrCommand>> {
        if !doc.modified() {
            return None;
        }

        let (fm, range) = doc.frontmatter()?;
        let new_text = update_metadata(&fm)?;
        let edits = vec![TextEdit::new(range, new_text)];

        let changes = std::collections::HashMap::from([(uri.clone(), edits)]);
        let code_action = CodeAction {
            title: "Update metadata".to_string(),
            kind: Some(self.action_kind()),
            is_preferred: Some(true),
            edit: Some(WorkspaceEdit::new(changes)),
            ..Default::default()
        };

        Some(vec![code_action.into()])
    }

    fn action_kind(&self) -> CodeActionKind {
        CodeActionKind::new("source.updateMetadata.fading")
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Metadata {
    id: String,
    created: toml::value::Date,
    modified: toml::value::Date,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn date_to_toml(date: Date) -> toml::value::Date {
    toml::value::Date {
        year: date.year() as u16,
        month: date.month() as u8,
        day: date.day() as u8,
    }
}

#[allow(dead_code, clippy::cast_possible_wrap)]
fn toml_to_date(date: toml::value::Date) -> Option<Date> {
    Date::new(date.year as i16, date.month as i8, date.day as i8).ok()
}

fn today() -> toml::value::Date {
    date_to_toml(Zoned::now().date())
}

fn update_metadata(fm: &str) -> Option<String> {
    let mut metadata = toml::from_str::<Metadata>(fm).ok()?;
    let today = today();

    if metadata.modified == today {
        return None;
    }

    metadata.modified = today;
    toml::to_string(&metadata).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::{ToSpan, civil::date};
    use proptest::prelude::*;
    use std::str::FromStr;

    fn make_doc(content: &str) -> Document {
        Document::new(None, false, content.to_string())
    }

    fn make_modified_doc(content: &str) -> Document {
        Document::new(None, true, content.to_string())
    }

    fn uri() -> Uri {
        "file:///test.md".parse().unwrap()
    }

    #[test]
    fn code_actions_returns_none_when_no_frontmatter() {
        let doc = make_modified_doc("No frontmatter here.");
        assert!(code_actions(&doc, &uri()).is_none());
    }

    #[test]
    fn should_clear_modified_false_when_no_frontmatter() {
        let doc = make_doc("No frontmatter.");
        assert!(!should_clear_modified(&doc));
    }

    #[test]
    fn should_clear_modified_false_when_invalid_toml() {
        let doc = make_doc("+++\ninvalid toml {{{\n+++\n");
        assert!(!should_clear_modified(&doc));
    }

    fn proptest_config() -> ProptestConfig {
        ProptestConfig::with_cases(1000)
    }

    mod strategies {
        use super::*;

        /// Strategy: Generates valid YYYY-MM format IDs
        ///
        /// Generates entry IDs in the range 1000-01 to 9999-12 to test edge cases.
        pub(super) fn id() -> impl Strategy<Value = String> {
            (1000i32..=9999, 01u32..=12).prop_map(|(y, m)| format!("{y:04}-{m:02}"))
        }

        /// Strategy: Generates valid dates using day offset
        ///
        /// Produces dates from 2020-01-01 to ~2030 by adding day offsets.
        /// More efficient than regex-based approach (never generates invalid dates like Feb 30).
        pub(super) fn jiff_date() -> impl Strategy<Value = Date> {
            let fixed = date(2020, 1, 1);
            (0i64..=4000).prop_map(move |offset| fixed.checked_add(offset.days()).unwrap())
        }

        /// Strategy: Generates toml::value::Date from jiff::civil::Date
        pub(super) fn toml_date() -> impl Strategy<Value = toml::value::Date> {
            jiff_date().prop_map(date_to_toml)
        }

        /// Strategy: Generates dates that are NOT today
        pub(super) fn not_today() -> impl Strategy<Value = toml::value::Date> {
            let today = today();
            toml_date().prop_filter("not today", move |date| *date != today)
        }

        /// Strategy: Generates Metadata with controllable modified date
        ///
        /// - `Some(true)`: Forces modified = today (for testing no-op case)
        /// - `Some(false)`: Forces modified ≠ today (for testing update case)
        /// - `None`: Random modified date (for general testing)
        pub(super) fn metadata(modified_at_today: Option<bool>) -> impl Strategy<Value = Metadata> {
            match modified_at_today {
                Some(true) => {
                    // Generate metadata with modified = today
                    (id(), toml_date())
                        .prop_map(|(id, created)| Metadata {
                            id,
                            created,
                            modified: today(),
                        })
                        .boxed()
                }
                Some(false) => {
                    // Generate metadata with modified ≠ today
                    (id(), toml_date(), not_today())
                        .prop_map(|(id, created, modified)| Metadata {
                            id,
                            created,
                            modified,
                        })
                        .boxed()
                }
                None => {
                    // Random modified date
                    (id(), toml_date(), toml_date())
                        .prop_map(|(id, created, modified)| Metadata {
                            id,
                            created,
                            modified,
                        })
                        .boxed()
                }
            }
        }

        /// Strategy: Generates invalid TOML strings that cannot be deserialized to Metadata
        pub(super) fn invalid_toml() -> impl Strategy<Value = String> {
            prop_oneof![
                // Syntax errors with fixed examples
                Just("invalid syntax {{{".to_string()),
                Just("id = unclosed string".to_string()),
                Just("[broken\ntoml".to_string()),
                // Missing required fields (generated dynamically)
                (id(), toml_date()).prop_map(|(id, created)| {
                    format!(
                        "id = \"{}\"\ncreated = {}-{:02}-{:02}",
                        id, created.year, created.month, created.day
                    )
                }),
                (id(), toml_date()).prop_map(|(id, modified)| {
                    format!(
                        "id = \"{}\"\nmodified = {}-{:02}-{:02}",
                        id, modified.year, modified.month, modified.day
                    )
                }),
                (toml_date(), toml_date()).prop_map(|(created, modified)| {
                    format!(
                        "created = {}-{:02}-{:02}\nmodified = {}-{:02}-{:02}",
                        created.year,
                        created.month,
                        created.day,
                        modified.year,
                        modified.month,
                        modified.day
                    )
                }),
                // Type mismatches (generated dynamically)
                (0i32..10000, toml_date(), toml_date()).prop_map(|(num, c, m)| {
                    format!(
                        "id = {}\ncreated = {}-{:02}-{:02}\nmodified = {}-{:02}-{:02}",
                        num, c.year, c.month, c.day, m.year, m.month, m.day
                    )
                }),
                (id(), toml_date()).prop_map(|(id, m)| {
                    format!(
                        "id = \"{}\"\ncreated = \"not-a-date\"\nmodified = {}-{:02}-{:02}",
                        id, m.year, m.month, m.day
                    )
                }),
                // Invalid date values (fixed examples)
                Just("id = \"2026-01\"\ncreated = 2026-02-30\nmodified = 2026-01-01".to_string()),
                Just("id = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-13-01".to_string()),
                // Empty or random text
                Just("".to_string()),
                Just("   \n\n  ".to_string()),
                "[a-zA-Z0-9\\s]{0,50}",
            ]
        }
    }

    proptest! {
        #![proptest_config(proptest_config())]

        /// Property: update_metadata always sets modified to today
        ///
        /// When modified date is not today, update_metadata returns Some(String) that:
        /// - Preserves id and created fields
        /// - Changes modified to today's date
        /// - Maintains valid TOML structure
        #[test]
        fn prop_update_metadata_changes_modified_to_today(metadata in strategies::metadata(Some(false))) {
            let toml_str = toml::to_string(&metadata).unwrap();

            let new_text = update_metadata(&toml_str).unwrap();

            let updated = toml::from_str::<Metadata>(&new_text).unwrap();
            prop_assert_eq!(updated.id, metadata.id);
            prop_assert_eq!(updated.created, metadata.created);
            prop_assert_eq!(updated.modified, today());
            prop_assert_ne!(updated.modified, metadata.modified);
        }

        /// Property: update_metadata is idempotent when modified is today
        ///
        /// When modified date is already today, update_metadata returns None
        /// (no changes needed).
        #[test]
        fn prop_update_metadata_noop_when_already_today(metadata in strategies::metadata(Some(true))) {
            let toml_str = toml::to_string(&metadata).unwrap();

            prop_assert!(update_metadata(&toml_str).is_none());
            prop_assert_eq!(metadata.modified, today());
        }

        /// Property: update_metadata returns None for invalid TOML
        ///
        /// When given invalid TOML that cannot be deserialized to Metadata,
        /// update_metadata gracefully returns None instead of panicking.
        #[test]
        fn prop_update_metadata_returns_none_for_invalid_toml(invalid in strategies::invalid_toml()) {
            let result = update_metadata(&invalid);
            prop_assert!(result.is_none());
        }

        /// Property: code_actions respects modified flag
        ///
        /// code_actions should only return actions when document is modified
        #[test]
        fn prop_code_actions_respects_modified_flag(
            metadata in strategies::metadata(Some(false))
        ) {
            let toml_str = toml::to_string(&metadata).unwrap();
            let content = format!("+++\n{}\n+++\n\nContent", toml_str);
            let uri = Uri::from_str("file:///test.md").unwrap();

            // Not modified → no actions
            let doc_not_modified = Document::new(None, false, content.clone());
            prop_assert!(code_actions(&doc_not_modified, &uri).is_none());

            // Modified → has actions (if metadata is outdated)
            let doc_modified = Document::new(None, true, content);
            let actions = code_actions(&doc_modified, &uri);
            prop_assert!(actions.is_some());
        }

        /// Property: should_clear_modified is consistent
        ///
        /// should_clear_modified returns true iff frontmatter.modified == today
        #[test]
        fn prop_should_clear_modified_consistency(
            metadata in strategies::metadata(None)
        ) {
            let toml_str = toml::to_string(&metadata).unwrap();
            let content = format!("+++\n{}\n+++\n\nContent", toml_str);
            let doc = Document::new(None, false, content);

            let result = should_clear_modified(&doc);
            let expected = metadata.modified == today();

            prop_assert_eq!(result, expected);
        }
    }
}
