//! Diagnostics for validating frontmatter.
//!
//! Validates TOML frontmatter structure, required fields, and semantic constraints.

use std::sync::OnceLock;

use chrono::NaiveDate;
use serde::Deserialize;
use toml::Spanned;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};
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
pub fn diagnose(doc: &Document) -> Vec<Diagnostic> {
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
    let raw = match toml::from_str::<RawMetadata>(fm) {
        Ok(raw) => raw,
        Err(e) => {
            let range = toml_error_range(&e, fm).unwrap_or(fm_range);
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
    check_id_format(&raw.id, fm, &mut diagnostics);

    // Check 5-6: Date validation
    check_dates(&raw, fm, &mut diagnostics);

    // Check 7-8: Heading validation
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
fn offset_to_position(fm: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32; // Line 1 is first line of frontmatter content
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
    if raw.id.is_none() {
        diagnostics.push(make_diagnostic(
            Code::MissingField,
            range,
            DiagnosticSeverity::ERROR,
            "Missing required field: `id`",
        ));
    }
    if raw.created.is_none() {
        diagnostics.push(make_diagnostic(
            Code::MissingField,
            range,
            DiagnosticSeverity::ERROR,
            "Missing required field: `created`",
        ));
    }
    if raw.modified.is_none() {
        diagnostics.push(make_diagnostic(
            Code::MissingField,
            range,
            DiagnosticSeverity::ERROR,
            "Missing required field: `modified`",
        ));
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
    let is_valid =
        id_str.len() == 7 && NaiveDate::parse_from_str(&format!("{id_str}-01"), "%Y-%m-%d").is_ok();

    if !is_valid {
        diagnostics.push(make_diagnostic(
            Code::InvalidIdFormat,
            range,
            DiagnosticSeverity::ERROR,
            &format!("Invalid id format: expected YYYY-MM, got `{id_str}`"),
        ));
    }
}

/// Validates date fields and checks created <= modified.
fn check_dates(raw: &RawMetadata, fm: &str, diagnostics: &mut Vec<Diagnostic>) {
    let created = check_and_parse_date(&raw.created, "created", fm, diagnostics);
    let modified = check_and_parse_date(&raw.modified, "modified", fm, diagnostics);

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

/// Parses a Spanned Datetime as a NaiveDate.
fn parse_spanned_date(spanned: &Spanned<toml::value::Datetime>) -> Option<NaiveDate> {
    let d = spanned.get_ref().date.as_ref()?;
    NaiveDate::from_ymd_opt(d.year as i32, d.month as u32, d.day as u32)
}

/// Parses and validates a date field, adding diagnostics if invalid.
fn check_and_parse_date(
    spanned: &Option<Spanned<toml::value::Datetime>>,
    field_name: &str,
    fm: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<NaiveDate> {
    let spanned = spanned.as_ref()?;

    match parse_spanned_date(spanned) {
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
    let source = doc.content().as_bytes();

    let query = heading_query();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut captures = cursor.captures(query, root, source);

    while let Some((query_match, _)) = captures.next() {
        for capture in query_match.captures {
            validate_heading(capture.node, source, diagnostics);
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
/// Uses chrono to parse and validate the date, including checking that the day of week matches.
fn is_valid_heading_date(text: &str) -> bool {
    NaiveDate::parse_from_str(text, "%Y-%m-%d %a").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> Document {
        Document::new(None, content.to_string())
    }

    #[test]
    fn test_missing_frontmatter() {
        let doc = make_doc("# Hello\n\nNo frontmatter.");
        let diags = diagnose(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("missing-frontmatter".to_string()))
        );
    }

    #[test]
    fn test_invalid_toml() {
        let doc = make_doc("+++\ninvalid toml here\n+++\n");
        let diags = diagnose(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("invalid-toml".to_string()))
        );
    }

    #[test]
    fn test_missing_fields() {
        let doc = make_doc("+++\nid = \"2026-01\"\n+++\n");
        let diags = diagnose(&doc);
        // Missing created and modified
        assert_eq!(diags.len(), 2);
        assert!(
            diags
                .iter()
                .all(|d| d.code == Some(NumberOrString::String("missing-field".to_string())))
        );
    }

    #[test]
    fn test_invalid_id_format() {
        let doc =
            make_doc("+++\nid = \"invalid\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n");
        let diags = diagnose(&doc);
        assert!(
            diags
                .iter()
                .any(|d| d.code == Some(NumberOrString::String("invalid-id-format".to_string())))
        );
    }

    #[test]
    fn test_created_after_modified() {
        let doc =
            make_doc("+++\nid = \"2026-01\"\ncreated = 2026-01-15\nmodified = 2026-01-01\n+++\n");
        let diags = diagnose(&doc);
        assert!(
            diags
                .iter()
                .any(|d| d.code
                    == Some(NumberOrString::String("created-after-modified".to_string())))
        );
    }

    #[test]
    fn test_valid_document() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\nContent",
        );
        let diags = diagnose(&doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_invalid_heading_level_h1() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\n# 2026-01-18 Sun\n",
        );
        let diags = diagnose(&doc);
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
        let diags = diagnose(&doc);
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
        let diags = diagnose(&doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_headings() {
        let doc = make_doc(
            "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n\n###### 2026-01-18 Sun\n\nSome content\n\n###### 2026-01-19 Mon\n",
        );
        let diags = diagnose(&doc);
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
}
