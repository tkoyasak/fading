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

pub const DIAGNOSTIC_SOURCE: &str = "fading";

pub fn diagnose(doc: &Document, uri: &Uri) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let Some((fm, fm_range)) = doc.frontmatter() else {
        diagnostics.push(make_diagnostic(
            Code::MissingFrontmatter,
            Range::new(Position::new(0, 0), Position::new(0, 0)),
            DiagnosticSeverity::ERROR,
            "Missing frontmatter: document must start with a `+++` block",
        ));
        return diagnostics;
    };

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

    let ctx = DiagnosticContext {
        doc,
        uri,
        fm: &fm,
        raw: &raw,
        fm_range,
    };

    for provider in get_providers() {
        diagnostics.extend(provider.check(&ctx));
    }

    diagnostics
}

/// Context passed to diagnostic providers.
struct DiagnosticContext<'a> {
    doc: &'a Document,
    uri: &'a Uri,
    fm: &'a str,
    raw: &'a RawMetadata,
    fm_range: Range,
}

/// Trait for diagnostic providers.
trait DiagnosticProvider {
    fn check(&self, ctx: &DiagnosticContext) -> Vec<Diagnostic>;
}

fn get_providers() -> Vec<Box<dyn DiagnosticProvider>> {
    vec![
        Box::new(RequiredFieldsProvider),
        Box::new(IdFormatProvider),
        Box::new(IdFilenameProvider),
        Box::new(DateProvider),
        Box::new(HeadingProvider),
    ]
}

/// Checks for missing required fields (id, created, modified).
struct RequiredFieldsProvider;

impl DiagnosticProvider for RequiredFieldsProvider {
    fn check(&self, ctx: &DiagnosticContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let fields = [
            (ctx.raw.id.is_none(), "id"),
            (ctx.raw.created.is_none(), "created"),
            (ctx.raw.modified.is_none(), "modified"),
        ];

        for (is_missing, field_name) in fields {
            if is_missing {
                diagnostics.push(make_diagnostic(
                    Code::MissingField,
                    ctx.fm_range,
                    DiagnosticSeverity::ERROR,
                    &format!("Missing required field: `{field_name}`"),
                ));
            }
        }

        diagnostics
    }
}

/// Validates id field is in YYYY-MM format.
struct IdFormatProvider;

impl DiagnosticProvider for IdFormatProvider {
    fn check(&self, ctx: &DiagnosticContext) -> Vec<Diagnostic> {
        let Some(spanned_id) = &ctx.raw.id else {
            return vec![];
        };

        let (line, _) = offset_to_position(ctx.fm, spanned_id.span().start);
        let range = Range::new(Position::new(line, 0), Position::new(line + 1, 0));
        let id_str = spanned_id.get_ref();
        let is_valid = id_str.len() == 7 && format!("{id_str}-01").parse::<Date>().is_ok();

        if !is_valid {
            vec![make_diagnostic(
                Code::InvalidIdFormat,
                range,
                DiagnosticSeverity::ERROR,
                &format!("Invalid id format: expected YYYY-MM, got `{id_str}`"),
            )]
        } else {
            vec![]
        }
    }
}

/// Validates id matches filename (e.g., "2026-01.md" → id = "2026-01").
struct IdFilenameProvider;

impl DiagnosticProvider for IdFilenameProvider {
    fn check(&self, ctx: &DiagnosticContext) -> Vec<Diagnostic> {
        let Some(spanned_id) = &ctx.raw.id else {
            return vec![];
        };

        let id_str = spanned_id.get_ref();
        let path_str = ctx.uri.path().as_str();
        let Some(filename) = path_str.split('/').next_back() else {
            return vec![];
        };
        let expected_id = filename.strip_suffix(".md").unwrap_or(filename);

        if id_str != expected_id {
            let (line, _) = offset_to_position(ctx.fm, spanned_id.span().start);
            let range = Range::new(Position::new(line, 0), Position::new(line + 1, 0));

            vec![make_diagnostic(
                Code::IdFilenameMismatch,
                range,
                DiagnosticSeverity::ERROR,
                &format!(
                    "ID `{}` does not match filename `{}` (expected `{}`)",
                    id_str, filename, expected_id
                ),
            )]
        } else {
            vec![]
        }
    }
}

/// Validates date fields (YYYY-MM-DD format) and created ≤ modified.
struct DateProvider;

impl DiagnosticProvider for DateProvider {
    fn check(&self, ctx: &DiagnosticContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let created = validate_date_field(&ctx.raw.created, "created", ctx.fm, &mut diagnostics);
        let modified = validate_date_field(&ctx.raw.modified, "modified", ctx.fm, &mut diagnostics);

        if let (Some(c), Some(m)) = (created, modified)
            && c > m
            && let Some(spanned_created) = &ctx.raw.created
        {
            let (line, _) = offset_to_position(ctx.fm, spanned_created.span().start);
            diagnostics.push(make_diagnostic(
                Code::CreatedAfterModified,
                Range::new(Position::new(line, 0), Position::new(line + 1, 0)),
                DiagnosticSeverity::WARNING,
                &format!("Creation date ({c}) is after modification date ({m})"),
            ));
        }

        diagnostics
    }
}

/// Validates headings are h6 (######) with "YYYY-MM-DD Day" format.
struct HeadingProvider;

impl DiagnosticProvider for HeadingProvider {
    fn check(&self, ctx: &DiagnosticContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let Some(tree) = ctx.doc.tree() else {
            return diagnostics;
        };

        let root = tree.block_tree().root_node();
        let source = ctx.doc.source_bytes();

        let query = heading_query();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut captures = cursor.captures(query, root, source.as_slice());

        while let Some((query_match, _)) = captures.next() {
            for capture in query_match.captures {
                validate_heading(capture.node, source.as_slice(), &mut diagnostics);
            }
        }

        diagnostics
    }
}

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

#[derive(Debug, Deserialize)]
struct RawMetadata {
    id: Option<Spanned<String>>,
    created: Option<Spanned<toml::value::Datetime>>,
    modified: Option<Spanned<toml::value::Datetime>>,
}

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

fn toml_error_range(error: &toml::de::Error, fm: &str) -> Option<Range> {
    let span = error.span()?;
    let (line, col) = offset_to_position(fm, span.start);
    Some(Range::new(
        Position::new(line, col),
        Position::new(line, col + 1),
    ))
}

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

#[allow(clippy::cast_possible_wrap)]
fn parse_date(spanned: &Spanned<toml::value::Datetime>) -> Option<Date> {
    let d = spanned.get_ref().date.as_ref()?;
    Date::new(d.year as i16, d.month as i8, d.day as i8).ok()
}

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

fn heading_query() -> &'static Query {
    HEADING_QUERY.get_or_init(|| {
        let language = tree_sitter_md::LANGUAGE.into();
        Query::new(&language, "(atx_heading) @heading").expect("Failed to compile heading query")
    })
}

fn validate_heading(node: tree_sitter::Node, source: &[u8], diagnostics: &mut Vec<Diagnostic>) {
    let line = node.start_position().row as u32;
    let range = Range::new(Position::new(line, 0), Position::new(line + 1, 0));

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
        let doc =
            make_doc("+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-15\n+++\n");
        let diags = diagnose(&doc, &test_uri("2026-01"));
        assert!(!has_diagnostic_code(&diags, "id-filename-mismatch"));
    }

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

    }
}
