use jiff::civil::Date;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Parse fading entries from markdown content.
/// Returns (date, content) pairs, excluding empty entries and `<!-- -->` placeholders.
pub(crate) fn parse_entries(content: &str) -> Vec<(Date, String)> {
    let opts = Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    let mut entries = Vec::new();
    let mut current_date: Option<Date> = None;
    let mut content_start: usize = 0;
    let mut in_h6 = false;
    let mut heading_text = String::new();

    for (event, range) in Parser::new_ext(content, opts).into_offset_iter() {
        match event {
            Event::End(TagEnd::MetadataBlock(_)) => {
                content_start = range.end;
            }
            Event::Start(Tag::Heading {
                level: HeadingLevel::H6,
                ..
            }) => {
                if let Some(date) = current_date.take() {
                    let trimmed = content[content_start..range.start].trim();
                    if !trimmed.is_empty() && trimmed != "<!-- -->" {
                        entries.push((date, trimmed.to_string()));
                    }
                }
                in_h6 = true;
                heading_text.clear();
            }
            Event::Text(text) if in_h6 => {
                heading_text.push_str(&text);
            }
            Event::End(TagEnd::Heading(_)) if in_h6 => {
                in_h6 = false;
                current_date = heading_text.get(..10).and_then(|s| s.parse().ok());
                content_start = range.end;
            }
            _ => {}
        }
    }

    if let Some(date) = current_date {
        let trimmed = content[content_start..].trim();
        if !trimmed.is_empty() && trimmed != "<!-- -->" {
            entries.push((date, trimmed.to_string()));
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    const FRONTMATTER: &str =
        "+++\nid = \"2026-01\"\ncreated = 2026-01-01\nmodified = 2026-01-01\n+++\n";

    #[test]
    fn empty_content_returns_no_entries() {
        assert!(parse_entries("").is_empty());
    }

    #[test]
    fn only_frontmatter_returns_no_entries() {
        assert!(parse_entries(FRONTMATTER).is_empty());
    }

    #[test]
    fn single_entry_with_content() {
        let content = format!("{FRONTMATTER}\n###### 2026-01-01 Thu\n\nhello\n");
        let entries = parse_entries(&content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, date(2026, 1, 1));
        assert_eq!(entries[0].1, "hello");
    }

    #[test]
    fn placeholder_entry_excluded() {
        let content = format!("{FRONTMATTER}\n###### 2026-01-01 Thu\n\n<!-- -->\n");
        assert!(parse_entries(&content).is_empty());
    }

    #[test]
    fn empty_entry_excluded() {
        let content =
            format!("{FRONTMATTER}\n###### 2026-01-01 Thu\n\n###### 2026-01-02 Fri\n\nhello\n");
        let entries = parse_entries(&content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, date(2026, 1, 2));
    }

    #[test]
    fn multiple_entries() {
        let content = format!(
            "{FRONTMATTER}\n###### 2026-01-01 Thu\n\nhello\n\n###### 2026-01-02 Fri\n\nworld\n"
        );
        let entries = parse_entries(&content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, "hello");
        assert_eq!(entries[1].1, "world");
    }
}
