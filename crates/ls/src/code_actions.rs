//! Code actions for fading documents.
//!
//! Provides code actions to automatically update the `modified` date in TOML frontmatter.

use chrono::{Datelike, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use tower_lsp_server::ls_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Range, TextEdit, Uri, WorkspaceEdit,
};

use crate::document::Document;

/// Trait for code action providers.
///
/// Each code action type should implement this trait to provide its own actions.
trait CodeActionProvider {
    /// Provides code actions for a document.
    fn provide(&self, doc: &Document, uri: &Uri) -> Option<Vec<CodeActionOrCommand>>;

    /// Returns the action kind for this provider.
    fn action_kind(&self) -> CodeActionKind;
}

/// Provider for metadata update code actions.
struct MetadataProvider;

impl CodeActionProvider for MetadataProvider {
    fn provide(&self, doc: &Document, uri: &Uri) -> Option<Vec<CodeActionOrCommand>> {
        // Only offer code action if document has been modified
        if !doc.modified() {
            return None;
        }

        let (fm, range) = doc.frontmatter()?;
        let edits = update_metadata(&fm, range)?;

        // Construct WorkspaceEdit with URI -> edits mapping
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

/// Returns all registered code action providers.
fn get_providers() -> Vec<Box<dyn CodeActionProvider>> {
    vec![
        Box::new(MetadataProvider),
        // Future: Box::new(FormatProvider), etc.
    ]
}

/// Returns all supported action kinds.
///
/// This is used by the LSP backend to advertise capabilities and filter requests.
pub fn supported_action_kinds() -> Vec<CodeActionKind> {
    get_providers()
        .into_iter()
        .map(|p| p.action_kind())
        .collect()
}

/// Returns available code actions for a document.
///
/// Collects actions from all registered providers.
///
/// Currently supports:
/// - Update metadata: Updates the `modified` date to today if the document has been modified
///
/// # Returns
/// - `None` if no actions are available from any provider
/// - `Some(actions)` with code actions from all providers
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

/// Checks if the document's modified flag should be cleared.
///
/// Returns true if the frontmatter's modified date is already today,
/// indicating that no further updates are needed.
///
/// # Returns
/// - `true`: Modified date is already today, safe to clear the modified flag
/// - `false`: Modified date is not today or frontmatter is invalid
pub fn should_clear_modified(doc: &Document) -> bool {
    doc.frontmatter()
        .and_then(|(fm, _)| toml::from_str::<Metadata>(&fm).ok())
        .is_some_and(|metadata| metadata.modified == today())
}

// ===== Private types and functions =====

/// TOML frontmatter structure.
///
/// Only these three fields are recognized; any extra fields are stripped on update.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Metadata {
    /// Entry identifier (e.g., "2026-01").
    id: String,
    /// Date when the entry was created.
    created: toml::value::Date,
    /// Date when the entry was last modified.
    modified: toml::value::Date,
}

/// Converts `chrono::NaiveDate` to `toml::value::Date`.
///
/// # Panics
///
/// Panics if the year is negative or exceeds `u16::MAX` (65535).
/// In practice, this should never happen for dates in reasonable ranges (1000-9999).
///
/// # Examples
///
/// ```ignore
/// let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
/// let toml_date = naive_date_to_toml(date);
/// assert_eq!(toml_date.year, 2026);
/// ```
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn naive_date_to_toml(date: NaiveDate) -> toml::value::Date {
    // SAFETY: chrono guarantees month is 1-12 and day is 1-31
    // year is truncated but should be in valid range for fading dates
    toml::value::Date {
        year: date.year() as u16,
        month: date.month() as u8,
        day: date.day() as u8,
    }
}

/// Converts `toml::value::Date` to `chrono::NaiveDate`.
///
/// Returns `None` if the date is invalid (e.g., Feb 30, month 13).
///
/// # Examples
///
/// ```ignore
/// let toml_date = toml::value::Date { year: 2026, month: 2, day: 30 };
/// assert!(toml_date_to_naive(toml_date).is_none()); // Feb 30 is invalid
/// ```
#[allow(
    dead_code,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
fn toml_date_to_naive(date: toml::value::Date) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(date.year as i32, date.month as u32, date.day as u32)
}

/// Returns the current date as `toml::value::Date`.
fn today() -> toml::value::Date {
    naive_date_to_toml(Local::now().date_naive())
}

/// Generates text edits to update the `modified` date to today.
///
/// Takes the frontmatter text and its range directly.
///
/// # Returns
/// - `Some(edits)`: Metadata needs updating, text edits provided
/// - `None`: Modified date is already today, no update needed, or TOML parse error
fn update_metadata(fm: &str, range: Range) -> Option<Vec<TextEdit>> {
    let mut metadata = toml::from_str::<Metadata>(fm).ok()?;
    let today = today();

    if metadata.modified == today {
        return None;
    }

    metadata.modified = today;
    let new_text = toml::to_string(&metadata).ok()?;

    Some(vec![TextEdit::new(range, new_text)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use proptest::prelude::*;
    use tower_lsp_server::ls_types::Position;

    // ===== Helper functions =====

    fn make_doc(content: &str) -> Document {
        Document::new(None, false, content.to_string())
    }

    fn make_modified_doc(content: &str) -> Document {
        Document::new(None, true, content.to_string())
    }

    fn valid_frontmatter_today() -> String {
        let today = today();
        format!(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = {}-{:02}-{:02}\n+++\n\nHello.",
            today.year, today.month, today.day
        )
    }

    fn valid_frontmatter_past() -> String {
        "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2020-01-01\n+++\n\nHello."
            .to_string()
    }

    fn uri() -> Uri {
        "file:///test.md".parse().unwrap()
    }

    // ===== code_actions() tests =====

    #[test]
    fn code_actions_returns_none_when_not_modified() {
        let doc = make_doc(&valid_frontmatter_past());
        assert!(code_actions(&doc, &uri()).is_none());
    }

    #[test]
    fn code_actions_returns_none_when_no_frontmatter() {
        let doc = make_modified_doc("No frontmatter here.");
        assert!(code_actions(&doc, &uri()).is_none());
    }

    #[test]
    fn code_actions_returns_none_when_already_today() {
        let doc = make_modified_doc(&valid_frontmatter_today());
        assert!(code_actions(&doc, &uri()).is_none());
    }

    #[test]
    fn code_actions_returns_some_when_modified_and_outdated() {
        let doc = make_modified_doc(&valid_frontmatter_past());
        let actions = code_actions(&doc, &uri());
        assert!(actions.is_some());
        assert_eq!(actions.unwrap().len(), 1);
    }

    // ===== should_clear_modified() tests =====

    #[test]
    fn should_clear_modified_true_when_today() {
        let doc = make_doc(&valid_frontmatter_today());
        assert!(should_clear_modified(&doc));
    }

    #[test]
    fn should_clear_modified_false_when_not_today() {
        let doc = make_doc(&valid_frontmatter_past());
        assert!(!should_clear_modified(&doc));
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

    // ===== Private function tests =====

    /// Proptest configuration: run 1000 test cases for better coverage
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
        pub(super) fn naive_date() -> impl Strategy<Value = NaiveDate> {
            let fixed = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
            (0i64..=4000).prop_map(move |offset| fixed + Duration::days(offset))
        }

        /// Strategy: Generates toml::value::Date from NaiveDate
        pub(super) fn toml_date() -> impl Strategy<Value = toml::value::Date> {
            naive_date().prop_map(naive_date_to_toml)
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

        /// Property: Date type conversion is lossless
        ///
        /// For any valid date, converting `chrono::NaiveDate` → `toml::value::Date` → `chrono::NaiveDate`
        /// preserves the original value.
        #[test]
        fn prop_date_type_conversion_is_lossless(naive_date in strategies::naive_date()) {
            let toml_date = naive_date_to_toml(naive_date);
            let converted_back = toml_date_to_naive(toml_date).unwrap();

            prop_assert_eq!(naive_date, converted_back);
        }

        /// Property: Metadata TOML serialization is reversible
        ///
        /// For any valid Metadata with arbitrary dates, serializing to TOML and deserializing
        /// produces an identical structure.
        #[test]
        fn prop_metadata_toml_serialization_roundtrip(metadata in strategies::metadata(None)) {
            let toml_str = toml::to_string(&metadata).unwrap();
            let deserialized = toml::from_str::<Metadata>(&toml_str).unwrap();

            prop_assert_eq!(deserialized, metadata);
        }

        /// Property: update_metadata always sets modified to today
        ///
        /// When modified date is not today, update_metadata returns Some(TextEdit) that:
        /// - Preserves id and created fields
        /// - Changes modified to today's date
        /// - Maintains valid TOML structure
        #[test]
        fn prop_update_metadata_changes_modified_to_today(metadata in strategies::metadata(Some(false))) {
            let toml_str = toml::to_string(&metadata).unwrap();
            let range = Range::new(Position::new(1, 0), Position::new(4, 0));

            let edits = update_metadata(&toml_str, range).unwrap();
            prop_assert_eq!(edits.len(), 1);

            let edit = &edits[0];
            prop_assert_eq!(edit.range, range);

            let updated = toml::from_str::<Metadata>(&edit.new_text).unwrap();
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
            let range = Range::new(Position::new(1, 0), Position::new(4, 0));

            prop_assert!(update_metadata(&toml_str, range).is_none());
            prop_assert_eq!(metadata.modified, today());
        }

        /// Property: update_metadata returns None for invalid TOML
        ///
        /// When given invalid TOML that cannot be deserialized to Metadata,
        /// update_metadata gracefully returns None instead of panicking.
        #[test]
        fn prop_update_metadata_returns_none_for_invalid_toml(invalid in strategies::invalid_toml()) {
            let range = Range::new(Position::new(1, 0), Position::new(4, 0));

            let result = update_metadata(&invalid, range);
            prop_assert!(result.is_none());
        }
    }
}
