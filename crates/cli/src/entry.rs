use jiff::civil::Date;

/// Parse a fading entry file's contents line by line and count characters per day.
/// Returns (date, char_count) for every written day within `from..=to` that has
/// content, excluding empty entries and `<!-- -->` placeholders.
pub(crate) fn parse_char_counts(content: &str, from: Date, to: Date) -> Vec<(Date, usize)> {
    let mut entries = Vec::new();
    let mut current_date: Option<Date> = None;
    let mut chars: usize = 0;
    let mut in_frontmatter = false;
    let mut first_line = true;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip the TOML frontmatter delimited by `+++` at the top of the file.
        if first_line {
            first_line = false;
            if trimmed == "+++" {
                in_frontmatter = true;
                continue;
            }
        }
        if in_frontmatter {
            if trimmed == "+++" {
                in_frontmatter = false;
            }
            continue;
        }

        // An h6 heading (`###### YYYY-MM-DD ...`) starts a new entry. Exactly six
        // `#` match: a seventh `#` falls where the prefix expects a space.
        if let Some(rest) = line.strip_prefix("###### ") {
            if let Some(date) = current_date.take()
                && chars > 0
            {
                entries.push((date, chars));
            }
            // Out-of-range dates become `None`, so the content below is never counted.
            current_date = rest
                .get(..10)
                .and_then(|s| s.parse().ok())
                .filter(|d| (from..=to).contains(d));
            chars = 0;
            continue;
        }

        // Otherwise it's entry content; count its characters.
        if current_date.is_some() && !trimmed.is_empty() && trimmed != "<!-- -->" {
            chars += trimmed.chars().count();
        }
    }

    if let Some(date) = current_date
        && chars > 0
    {
        entries.push((date, chars));
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    const FRONTMATTER: &str =
        "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-01\n+++\n";

    /// Parse with a range wide enough to keep every test date.
    fn parse(content: &str) -> Vec<(Date, usize)> {
        parse_char_counts(content, date(2000, 1, 1), date(2100, 1, 1))
    }

    #[test]
    fn empty_content_returns_no_entries() {
        assert!(parse("").is_empty());
    }

    #[test]
    fn only_frontmatter_returns_no_entries() {
        assert!(parse(FRONTMATTER).is_empty());
    }

    #[test]
    fn single_entry_with_content() {
        let content = format!("{FRONTMATTER}\n###### 2026-01-01 Thu\n\nhello\n");
        let entries = parse(&content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], (date(2026, 1, 1), 5));
    }

    #[test]
    fn placeholder_entry_excluded() {
        let content = format!("{FRONTMATTER}\n###### 2026-01-01 Thu\n\n<!-- -->\n");
        assert!(parse(&content).is_empty());
    }

    #[test]
    fn empty_entry_excluded() {
        let content =
            format!("{FRONTMATTER}\n###### 2026-01-01 Thu\n\n###### 2026-01-02 Fri\n\nhello\n");
        let entries = parse(&content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, date(2026, 1, 2));
    }

    #[test]
    fn multiple_entries() {
        let content = format!(
            "{FRONTMATTER}\n###### 2026-01-01 Thu\n\nhello\n\n###### 2026-01-02 Fri\n\nworld\n"
        );
        let entries = parse(&content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], (date(2026, 1, 1), 5));
        assert_eq!(entries[1], (date(2026, 1, 2), 5));
    }

    #[test]
    fn counts_unicode_chars_not_bytes() {
        let content = "###### 2026-01-01 Thu\n\nこんにちは\n";
        let entries = parse(content);
        assert_eq!(entries[0].1, 5);
    }

    #[test]
    fn multiple_lines_summed() {
        let content = "###### 2026-01-01 Thu\n\nhello\nworld\n";
        let entries = parse(content);
        assert_eq!(entries[0].1, 10);
    }

    #[test]
    fn works_without_frontmatter() {
        let content = "###### 2026-01-01 Thu\n\nhello\n";
        let entries = parse(content);
        assert_eq!(entries, vec![(date(2026, 1, 1), 5)]);
    }

    #[test]
    fn date_before_range_excluded() {
        let content = "###### 2025-12-31 Wed\n\nhello\n\n###### 2026-01-01 Thu\n\nworld\n";
        let entries = parse_char_counts(content, date(2026, 1, 1), date(2026, 1, 31));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, date(2026, 1, 1));
    }

    #[test]
    fn boundary_dates_included() {
        let content = "###### 2026-01-01 Thu\n\nfirst\n\n###### 2026-01-31 Sat\n\nlast\n";
        let entries = parse_char_counts(content, date(2026, 1, 1), date(2026, 1, 31));
        assert_eq!(entries.len(), 2);
    }
}
