use std::collections::HashMap;

use anyhow::Result;
use jiff::civil::Date;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use xshell::{Shell, cmd};

/// Parse a monthly entry file into `(Date, content)` pairs.
/// Skips the TOML frontmatter and empty/placeholder days.
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

/// Read and parse all entry files from git HEAD in parallel.
/// Returns a map of YYYYMMDD → entry content.
pub(crate) fn read_all_entries(repo_dir: &str) -> Result<HashMap<String, String>> {
    let sh = Shell::new()?.with_current_dir(repo_dir);
    let names = cmd!(sh, "git ls-tree --name-only HEAD -- entries/").read()?;

    let names: Vec<String> = names
        .lines()
        .filter(|f| f.ends_with(".md"))
        .map(|s| s.to_string())
        .collect();

    read_files_parallel(repo_dir, &names)
}

fn read_files_parallel(repo_dir: &str, names: &[String]) -> Result<HashMap<String, String>> {
    let results: Vec<Result<Vec<(String, String)>>> = std::thread::scope(|s| {
        let handles: Vec<_> = names
            .iter()
            .map(|name| {
                s.spawn(move || -> Result<Vec<(String, String)>> {
                    let sh = Shell::new()?.with_current_dir(repo_dir);
                    let content = match cmd!(sh, "git show HEAD:{name}").read() {
                        Ok(c) => c,
                        Err(_) => return Ok(vec![]), // file absent at HEAD
                    };
                    Ok(parse_entries(&content)
                        .into_iter()
                        .map(|(date, text)| (date.strftime("%Y%m%d").to_string(), text))
                        .collect())
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("thread panicked")))
            })
            .collect()
    });

    let mut entries = HashMap::new();
    for result in results {
        for (date, text) in result? {
            entries.insert(date, text);
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skips_empty_days() {
        let content = r#"+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-01
+++

###### 2026-01-01 Thu


###### 2026-01-02 Fri

今日は良い日だった。

###### 2026-01-03 Sat

"#;
        let entries = parse_entries(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "2026-01-02".parse::<Date>().unwrap());
        assert_eq!(entries[0].1, "今日は良い日だった。");
    }

    #[test]
    fn test_skips_placeholder() {
        let content = r#"+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-01
+++

###### 2026-01-01 Thu

<!-- -->

###### 2026-01-02 Fri

今日は良い日だった。

###### 2026-01-03 Sat

<!-- -->
"#;
        let entries = parse_entries(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "2026-01-02".parse::<Date>().unwrap());
        assert_eq!(entries[0].1, "今日は良い日だった。");
    }

    #[test]
    fn test_multiple_entries() {
        let content = r#"+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-01
+++

###### 2026-01-01 Thu

一行目。

###### 2026-01-02 Fri

二行目。
続き。
"#;
        let entries = parse_entries(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "2026-01-01".parse::<Date>().unwrap());
        assert_eq!(entries[0].1, "一行目。");
        assert_eq!(entries[1].0, "2026-01-02".parse::<Date>().unwrap());
        assert_eq!(entries[1].1, "二行目。\n続き。");
    }
}
