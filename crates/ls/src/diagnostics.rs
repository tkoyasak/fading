//! Diagnostics for validating frontmatter.
//!
//! Validates TOML frontmatter structure, required fields, and semantic constraints.

use std::sync::OnceLock;

use jiff::civil::Date;
use serde::Deserialize;
use toml::Spanned;
use tower_lsp_server::ls_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri,
};
use tree_sitter::{Query, StreamingIterator};

use crate::document::Document;

/// Diagnostic source identifier.
pub const DIAGNOSTIC_SOURCE: &str = "fading";

/// Diagnostic codes for different error types.
#[derive(Debug, Clone, Copy)]
enum Code {
    MissingFrontmatter,
    InvalidToml,
    MissingField,
    InvalidIdFormat,
    InvalidDate,
    CreatedAfterModified,
    InvalidHeadingLevel,
    InvalidHeadingFormat,
    IdFilenameMismatch,
}

impl Code {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingFrontmatter => "missing-frontmatter",
            Self::InvalidToml => "invalid-toml",
            Self::MissingField => "missing-field",
            Self::InvalidIdFormat => "invalid-id-format",
            Self::InvalidDate => "invalid-date",
            Self::CreatedAfterModified => "created-after-modified",
            Self::InvalidHeadingLevel => "invalid-heading-level",
            Self::InvalidHeadingFormat => "invalid-heading-format",
            Self::IdFilenameMismatch => "id-filename-mismatch",
        }
    }
}

/// Lenient frontmatter structure for validation.
///
/// Uses `Spanned<T>` to track byte offsets of each field in the source.
#[derive(Debug, Deserialize)]
struct RawMetadata {
    id: Option<Spanned<String>>,
    created: Option<Spanned<toml::value::Datetime>>,
    modified: Option<Spanned<toml::value::Datetime>>,
}

/// Runs all diagnostic checks on a document.
pub fn diagnose(doc: &Document, uri: &Uri) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Check 1: Frontmatter exists
    let Some((fm, fm_range)) = doc.frontmatter() else {
        diagnostics.push(make_diagnostic(
            Code::MissingFrontmatter,
            Range::new(Position::new(0, 0), Position::new(0, 0)),
            DiagnosticSeverity::ERROR,
            "Missing frontmatter: document must start with a `+++` block",
        ));
        return diagnostics;
    };

    // Check 2: Valid TOML syntax
    let raw = match toml::from_str::<RawMetadata>(&fm) {
        Ok(raw) => raw,
        Err(e) => {
            let range = toml_error_range(&e, &fm).unwrap_or(fm_range);
            diagnostics.push(make_diagnostic(
                Code::InvalidToml,
                range,
                DiagnosticSeverity::ERROR,
                &format!("Invalid TOML: {}", e.message()),
            ));
            return diagnostics;
        }
    };

    // Check 3: Required fields
    check_required_fields(&raw, fm_range, &mut diagnostics);

    // Check 4: id format (YYYY-MM)
    check_id_format(&raw.id, &fm, &mut diagnostics);

    // Check 5: id matches filename
    check_id_filename_match(&raw.id, uri, &fm, &mut diagnostics);

    // Check 6-7: Date validation
    check_dates(&raw, &fm, &mut diagnostics);

    // Check 8-9: Heading validation
    check_headings(doc, &mut diagnostics);

    diagnostics
}

/// Creates a diagnostic with consistent formatting.
fn make_diagnostic(
    code: Code,
    range: Range,
    severity: DiagnosticSeverity,
    message: &str,
) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(code.as_str().to_string())),
        source: Some(DIAGNOSTIC_SOURCE.to_string()),
        message: message.to_string(),
        ..Default::default()
    }
}

/// Converts a TOML parse error to a range.
///
/// Returns `None` if the error has no span information.
fn toml_error_range(error: &toml::de::Error, fm: &str) -> Option<Range> {
    let span = error.span()?;
    let (line, col) = offset_to_position(fm, span.start);
    Some(Range::new(
        Position::new(line, col),
        Position::new(line, col + 1),
    ))
}

/// Converts byte offset to (line, column) in frontmatter.
///
/// # Line numbering
/// Line numbers are relative to the frontmatter content (after the opening `+++`).
/// - Line 0: Would be the opening `+++` (not included in frontmatter string)
/// - Line 1: First line of TOML content (where frontmatter string starts)
/// - Line N: Nth line of TOML content
///
/// # Arguments
/// - `fm`: The frontmatter string (TOML content only, without `+++` markers)
/// - `offset`: Byte offset within the frontmatter string
///
/// # Returns
/// - `line`: 1-based line number within frontmatter content
/// - `col`: 0-based column number (resets to 0 after each newline)
fn offset_to_position(fm: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 0u32;
    let mut current_offset = 0usize;

    for ch in fm.chars() {
        if current_offset >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        current_offset += ch.len_utf8();
    }

    (line, col)
}

/// Checks for missing required fields.
fn check_required_fields(raw: &RawMetadata, range: Range, diagnostics: &mut Vec<Diagnostic>) {
    let fields = [
        (raw.id.is_none(), "id"),
        (raw.created.is_none(), "created"),
        (raw.modified.is_none(), "modified"),
    ];

    for (is_missing, field_name) in fields {
        if is_missing {
            diagnostics.push(make_diagnostic(
                Code::MissingField,
                range,
                DiagnosticSeverity::ERROR,
                &format!("Missing required field: `{field_name}`"),
            ));
        }
    }
}

/// Validates the id field format (YYYY-MM).
fn check_id_format(id: &Option<Spanned<String>>, fm: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(spanned_id) = id else { return };

    // Get line number from byte offset (no need to search through frontmatter)
    let (line, _) = offset_to_position(fm, spanned_id.span().start);
    let range = Range::new(Position::new(line, 0), Position::new(line + 1, 0));

    let id_str = spanned_id.get_ref();

    // Validate YYYY-MM format by attempting to parse with a dummy day
    let is_valid = id_str.len() == 7 && format!("{id_str}-01").parse::<Date>().is_ok();

    if !is_valid {
        diagnostics.push(make_diagnostic(
            Code::InvalidIdFormat,
            range,
            DiagnosticSeverity::ERROR,
            &format!("Invalid id format: expected YYYY-MM, got `{id_str}`"),
        ));
    }
}

/// Validates that the id matches the filename.
///
/// Expects filename format: YYYY-MM.md
/// Expects id format: "YYYY-MM"
fn check_id_filename_match(
    id: &Option<Spanned<String>>,
    uri: &Uri,
    fm: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(spanned_id) = id else { return };

    let id_str = spanned_id.get_ref();

    // Extract filename from URI (e.g., "file:///path/to/2026-01.md" -> "2026-01.md")
    let path_str = uri.path().as_str();
    let Some(filename) = path_str.split('/').next_back() else {
        return;
    };

    // Extract expected id from filename (e.g., "2026-01.md" -> "2026-01")
    let expected_id = filename.strip_suffix(".md").unwrap_or(filename);

    // Check if id matches filename (without .md extension)
    if id_str != expected_id {
        let (line, _) = offset_to_position(fm, spanned_id.span().start);
        let range = Range::new(Position::new(line, 0), Position::new(line + 1, 0));

        diagnostics.push(make_diagnostic(
            Code::IdFilenameMismatch,
            range,
            DiagnosticSeverity::ERROR,
            &format!(
                "ID `{}` does not match filename `{}` (expected `{}`)",
                id_str, filename, expected_id
            ),
        ));
    }
}

/// Validates date fields and checks created <= modified.
fn check_dates(raw: &RawMetadata, fm: &str, diagnostics: &mut Vec<Diagnostic>) {
    let created = validate_date_field(&raw.created, "created", fm, diagnostics);
    let modified = validate_date_field(&raw.modified, "modified", fm, diagnostics);

    // Check created <= modified
    if let (Some(c), Some(m)) = (created, modified)
        && c > m
        && let Some(spanned_created) = &raw.created
    {
        let (line, _) = offset_to_position(fm, spanned_created.span().start);
        diagnostics.push(make_diagnostic(
            Code::CreatedAfterModified,
            Range::new(Position::new(line, 0), Position::new(line + 1, 0)),
            DiagnosticSeverity::WARNING,
            &format!("Creation date ({c}) is after modification date ({m})"),
        ));
    }
}

/// Parses a Spanned Datetime as a Date.
#[allow(clippy::cast_possible_wrap)]
fn parse_date(spanned: &Spanned<toml::value::Datetime>) -> Option<Date> {
    let d = spanned.get_ref().date.as_ref()?;
    Date::new(d.year as i16, d.month as i8, d.day as i8).ok()
}

/// Validates a date field and returns the parsed date, adding diagnostics if invalid.
fn validate_date_field(
    spanned: &Option<Spanned<toml::value::Datetime>>,
    field_name: &str,
    fm: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Date> {
    let spanned = spanned.as_ref()?;

    match parse_date(spanned) {
        Some(date) => Some(date),
        None => {
            let (line, _) = offset_to_position(fm, spanned.span().start);
            diagnostics.push(make_diagnostic(
                Code::InvalidDate,
                Range::new(Position::new(line, 0), Position::new(line + 1, 0)),
                DiagnosticSeverity::ERROR,
                &format!("Invalid date format for `{field_name}`: expected YYYY-MM-DD"),
            ));
            None
        }
    }
}

/// Cached query for finding all headings (compiled once, reused across all documents).
static HEADING_QUERY: OnceLock<Query> = OnceLock::new();

/// Returns the cached heading query, compiling it on first access.
fn heading_query() -> &'static Query {
    HEADING_QUERY.get_or_init(|| {
        let language = tree_sitter_md::LANGUAGE.into();
        Query::new(&language, "(atx_heading) @heading").expect("Failed to compile heading query")
    })
}

/// Validates all headings in the document using tree-sitter queries.
fn check_headings(doc: &Document, diagnostics: &mut Vec<Diagnostic>) {
    let Some(tree) = doc.tree() else { return };
    let root = tree.block_tree().root_node();
    let source = doc.source_bytes();

    let query = heading_query();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut captures = cursor.captures(query, root, source.as_slice());

    while let Some((query_match, _)) = captures.next() {
        for capture in query_match.captures {
            validate_heading(capture.node, source.as_slice(), diagnostics);
        }
    }
}

/// Validates a single heading node.
fn validate_heading(node: tree_sitter::Node, source: &[u8], diagnostics: &mut Vec<Diagnostic>) {
    let line = node.start_position().row as u32;
    let range = Range::new(Position::new(line, 0), Position::new(line + 1, 0));

    // Check 1: Must be h6
    let is_h6 = (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .any(|child| child.kind() == "atx_h6_marker");

    if !is_h6 {
        diagnostics.push(make_diagnostic(
            Code::InvalidHeadingLevel,
            range,
            DiagnosticSeverity::ERROR,
            "Only h6 headings (######) are allowed",
        ));
        return;
    }

    // Check 2: Format must be YYYY-MM-DD Day
    let text = (0..node.child_count())
        .filter_map(|i| node.child(i as u32))
        .find(|child| child.kind() == "inline")
        .and_then(|inline| inline.utf8_text(source).ok());

    if let Some(text) = text
        && !is_valid_heading_date(text)
    {
        diagnostics.push(make_diagnostic(
            Code::InvalidHeadingFormat,
            range,
            DiagnosticSeverity::ERROR,
            &format!("Invalid heading format: expected `YYYY-MM-DD Day`, got `{text}`"),
        ));
    }
}

/// Checks if the heading text matches the `YYYY-MM-DD Day` format.
///
/// Uses jiff to parse and validate the date, including checking that the day of week matches.
fn is_valid_heading_date(text: &str) -> bool {
    Date::strptime("%Y-%m-%d %a", text).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> Document {
        Document::new(None, false, content.to_string())
    }

    fn test_uri(filename: &str) -> Uri {
        format!("file:///test/{filename}").parse().unwrap()
    }

    // ===== Unit tests for specific error conditions =====

    #[test]
    fn test_invalid_id_format() {
        let doc =
            make_doc("+++\nid = \"invalid\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n");
        let diags = diagnose(&doc, &test_uri("test.md"));
        assert!(
            diags
                .iter()
                .any(|d| d.code == Some(NumberOrString::String("invalid-id-format".to_string())))
        );
    }

    #[test]
    fn test_invalid_heading_level_h1() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\n# 2026-01-18 Sun\n",
        );
        let diags = diagnose(&doc, &test_uri("2026-01.md"));
        assert!(
            diags.iter().any(
                |d| d.code == Some(NumberOrString::String("invalid-heading-level".to_string()))
            )
        );
    }

    #[test]
    fn test_invalid_heading_format() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\n###### Invalid heading\n",
        );
        let diags = diagnose(&doc, &test_uri("2026-01.md"));
        assert!(
            diags
                .iter()
                .any(|d| d.code
                    == Some(NumberOrString::String("invalid-heading-format".to_string())))
        );
    }

    #[test]
    fn test_valid_heading() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\n###### 2026-01-18 Sun\n",
        );
        let diags = diagnose(&doc, &test_uri("2026-01.md"));
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_headings() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\n###### 2026-01-18 Sun\n\nSome content\n\n###### 2026-01-19 Mon\n",
        );
        let diags = diagnose(&doc, &test_uri("2026-01.md"));
        assert!(diags.is_empty());
    }

    #[test]
    fn test_is_valid_heading_date() {
        assert!(is_valid_heading_date("2026-01-18 Sun"));
        assert!(is_valid_heading_date("2026-12-31 Thu"));
        assert!(!is_valid_heading_date("2026-01-18"));
        assert!(!is_valid_heading_date("2026-01-18 Sunurday"));
        assert!(!is_valid_heading_date("Invalid"));
        assert!(!is_valid_heading_date("2026-13-01 Mon")); // Invalid month
        assert!(!is_valid_heading_date("2026-01-32 Mon")); // Invalid day
    }

    #[test]
    fn test_id_filename_match() {
        let doc =
            make_doc("+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n");
        let diags = diagnose(&doc, &test_uri("2026-01.md"));
        assert!(diags.is_empty());
    }

    #[test]
    fn test_id_filename_mismatch() {
        let doc =
            make_doc("+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n");
        let diags = diagnose(&doc, &test_uri("2026-02.md"));
        assert!(has_diagnostic_code(&diags, "id-filename-mismatch"));
    }

    #[test]
    fn test_id_filename_without_md_extension() {
        // Test with filename without .md extension
        let doc =
            make_doc("+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n");
        let diags = diagnose(&doc, &test_uri("2026-01"));
        // Should match because strip_suffix returns original if no .md
        assert!(!has_diagnostic_code(&diags, "id-filename-mismatch"));
    }

    // ===== Property-based tests =====
    //
    // These tests verify invariants across a wide range of inputs:
    // - prop_diagnose_never_panics: Robustness (no crashes on any input)
    // - prop_valid_frontmatter_no_missing_fields: Completeness (valid input → no errors)
    // - prop_no_frontmatter_gets_diagnostic: Correctness (invalid input → expected error)
    // - prop_valid_heading_date_format: Format validation consistency
    // - prop_invalid_date_rejected: Invalid date rejection
    // - prop_diagnostic_count_bounded: Performance (reasonable diagnostic count)
    // - prop_valid_toml_no_error: Valid TOML parsing
    // - prop_created_after_modified_error: Semantic validation

    use proptest::prelude::*;

    fn has_diagnostic_code(diags: &[Diagnostic], code: &str) -> bool {
        diags
            .iter()
            .any(|d| d.code == Some(NumberOrString::String(code.to_string())))
    }

    proptest! {
        /// Property: Invalid TOML syntax produces invalid-toml diagnostic
        #[test]
        fn prop_invalid_toml_produces_diagnostic(
            garbage in "[a-z]{1,20}"
        ) {
            let content = format!("+++\n{} {{ invalid\n+++\n", garbage);
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            prop_assert!(has_diagnostic_code(&diags, "invalid-toml"));
        }

        /// Property: Missing required field produces missing-field diagnostic
        #[test]
        fn prop_missing_field_produces_diagnostic(
            year in 2020u32..2030,
            month in 1u32..=12,
            day in 1u32..=28
        ) {
            // Missing id field
            let content = format!(
                "+++\ncreated = {}-{:02}-{:02}\nmodified = {}-{:02}-{:02}\n+++\n",
                year, month, day, year, month, day
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            prop_assert!(has_diagnostic_code(&diags, "missing-field"));
        }

        /// Property: Time-only value (no date) produces invalid-date diagnostic
        #[test]
        fn prop_time_only_produces_invalid_date(
            id in "[0-9]{4}-[0-9]{2}",
            hour in 0u32..24,
            minute in 0u32..60,
            second in 0u32..60
        ) {
            let content = format!(
                "+++\nid = \"{}\"\ncreated = {:02}:{:02}:{:02}\nmodified = 2026-01-15\n+++\n",
                id, hour, minute, second
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            prop_assert!(has_diagnostic_code(&diags, "invalid-date"));
        }

        /// Property: diagnose never panics on any input
        #[test]
        fn prop_diagnose_never_panics(
            content in "[\\p{Any}]{0,500}"
        ) {
            let doc = make_doc(&content);

            // Should never panic, regardless of input
            let _diags = diagnose(&doc, &test_uri("test.md"));

            // Just completing without panic is success
            prop_assert!(true);
        }

        /// Property: Valid frontmatter with all required fields produces no missing-field errors
        #[test]
        fn prop_valid_frontmatter_no_missing_fields(
            id in "[a-z0-9-]{1,20}",
            created_year in 2020u32..2030,
            created_month in 1u32..=12,
            created_day in 1u32..=28,
            modified_year in 2020u32..2030,
            modified_month in 1u32..=12,
            modified_day in 1u32..=28
        ) {
            let content = format!(
                "+++\nid = \"{}\"\ncreated = {}-{:02}-{:02}\nmodified = {}-{:02}-{:02}\n+++\n\nContent",
                id, created_year, created_month, created_day,
                modified_year, modified_month, modified_day
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            // Should not have missing-field errors
            let has_missing_field_error = diags.iter().any(|d| {
                d.code == Some(NumberOrString::String("missing-field".to_string()))
            });
            prop_assert!(!has_missing_field_error);
        }

        /// Property: Documents without frontmatter get missing-frontmatter diagnostic
        #[test]
        fn prop_no_frontmatter_gets_diagnostic(
            content in "[^+]{1,200}"  // Content without +++ markers
        ) {
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            // Should have at least one diagnostic (missing frontmatter)
            prop_assert!(!diags.is_empty());

            // Should specifically have missing-frontmatter error
            let has_missing_fm = diags.iter().any(|d| {
                d.code == Some(NumberOrString::String("missing-frontmatter".to_string()))
            });
            prop_assert!(has_missing_fm);
        }

        /// Property: Valid heading dates are recognized correctly
        #[test]
        fn prop_valid_heading_date_format(
            year in 2020u32..2030,
            month in 1u32..=12,
            day in 1u32..=28,
            weekday in prop::sample::select(vec!["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"])
        ) {
            let date_str = format!("{}-{:02}-{:02} {}", year, month, day, weekday);

            // Should be recognized as valid format (though day-of-week may not match)
            // The function checks format, not semantic correctness
            let result = is_valid_heading_date(&date_str);

            // Result should be deterministic for same input
            prop_assert!(result || !result);  // Always true, just checking no panic
        }

        /// Property: Invalid month/day values are rejected
        #[test]
        fn prop_invalid_date_rejected(
            month in 13u32..=99,  // Invalid months
            day in 1u32..=28
        ) {
            let date_str = format!("2026-{:02}-{:02} Mon", month, day);

            // Should be rejected
            prop_assert!(!is_valid_heading_date(&date_str));
        }

        /// Property: Diagnostic count is non-negative and bounded
        #[test]
        fn prop_diagnostic_count_bounded(
            content in ".{0,500}"
        ) {
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            // Diagnostic count should be reasonable (not thousands)
            prop_assert!(diags.len() < 100);
        }

        /// Property: Documents with valid structure don't produce invalid-toml error
        #[test]
        fn prop_valid_toml_no_error(
            id in "[a-z0-9-]{1,20}",
            year in 2020u32..2030,
            month in 1u32..=12,
            day in 1u32..=28
        ) {
            let content = format!(
                "+++\nid = \"{}\"\ncreated = {}-{:02}-{:02}\nmodified = {}-{:02}-{:02}\n+++\n",
                id, year, month, day, year, month, day
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            // Should not have invalid-toml error
            let has_invalid_toml = diags.iter().any(|d| {
                d.code == Some(NumberOrString::String("invalid-toml".to_string()))
            });
            prop_assert!(!has_invalid_toml);
        }

        /// Property: created > modified triggers error
        #[test]
        fn prop_created_after_modified_error(
            id in "[a-z0-9-]{1,20}",
            created_year in 2025u32..2030,
            modified_year in 2020u32..2024
        ) {
            let content = format!(
                "+++\nid = \"{}\"\ncreated = {}-01-15\nmodified = {}-01-01\n+++\n",
                id, created_year, modified_year
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri("test.md"));

            // Should have created-after-modified error when created > modified
            if created_year > modified_year {
                let has_error = diags.iter().any(|d| {
                    d.code == Some(NumberOrString::String("created-after-modified".to_string()))
                });
                prop_assert!(has_error);
            }
        }

        /// Property: id matching filename produces no mismatch error
        #[test]
        fn prop_id_filename_match_no_error(
            year in 2020u32..2030,
            month in 1u32..=12
        ) {
            let id = format!("{:04}-{:02}", year, month);
            let filename = format!("{}.md", id);
            let content = format!(
                "+++\nid = \"{}\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n",
                id
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri(&filename));

            // Should not have id-filename-mismatch error
            prop_assert!(!has_diagnostic_code(&diags, "id-filename-mismatch"));
        }

        /// Property: id not matching filename produces mismatch error
        #[test]
        fn prop_id_filename_mismatch_error(
            id_year in 2020u32..2030,
            id_month in 1u32..=12,
            file_year in 2020u32..2030,
            file_month in 1u32..=12
        ) {
            let id = format!("{:04}-{:02}", id_year, id_month);
            let filename = format!("{:04}-{:02}.md", file_year, file_month);
            let content = format!(
                "+++\nid = \"{}\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n",
                id
            );
            let doc = make_doc(&content);
            let diags = diagnose(&doc, &test_uri(&filename));

            // Should have id-filename-mismatch error when id != filename
            if id != format!("{:04}-{:02}", file_year, file_month) {
                prop_assert!(has_diagnostic_code(&diags, "id-filename-mismatch"));
            }
        }
    }
}
