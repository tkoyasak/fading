use std::collections::HashSet;

use anyhow::{Context, Result};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use xshell::{Shell, cmd};

use crate::{Cmd, Push};

const R2_OBJECT_KEY: &str = "fading.bundle";

/// Parse a monthly entry file into (date, content) pairs.
/// Skips the TOML frontmatter and empty days.
fn parse_entries(content: &str) -> Vec<(String, String)> {
    let opts = Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS;
    let mut entries = Vec::new();
    let mut current_date: Option<String> = None;
    let mut content_start: usize = 0;
    let mut in_h6 = false;
    let mut heading_text = String::new();

    for (event, range) in Parser::new_ext(content, opts).into_offset_iter() {
        match event {
            // Skip frontmatter; set content_start to just after the closing +++
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
                // Date is the first 10 chars of the heading text (YYYY-MM-DD)
                current_date = heading_text.get(..10).map(|s| s.to_string());
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

/// Read all entry files at HEAD and return (date, content) pairs sorted by date.
fn parse_all_entries(sh: &Shell, repo_dir: &str) -> Result<Vec<(String, String)>> {
    let names = cmd!(sh, "git -C {repo_dir} ls-tree --name-only HEAD -- entries/")
        .read()
        .context("Failed to list entries at HEAD")?;

    let mut all_entries = Vec::new();
    for name in names.lines().filter(|f| f.ends_with(".md")) {
        let content = cmd!(sh, "git -C {repo_dir} show HEAD:{name}")
            .read()
            .with_context(|| format!("Failed to read {name} at HEAD"))?;
        all_entries.extend(parse_entries(&content));
    }
    Ok(all_entries)
}

fn push_r2(
    sh: &Shell,
    repo_dir: &str,
    account_id: &str,
    bucket: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> Result<()> {
    let temp = sh.create_temp_dir().context("Failed to create temp dir")?;
    let bundle_path = temp.path().join(R2_OBJECT_KEY);
    let bundle_path_str = bundle_path
        .to_str()
        .context("Bundle path is not valid UTF-8")?;

    cmd!(
        sh,
        "git -C {repo_dir} bundle create {bundle_path_str} --all"
    )
    .run()
    .context("Failed to create git bundle")?;

    let body = sh
        .read_binary_file(&bundle_path)
        .context("Failed to read bundle file")?;

    let datetime = jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::UTC)
        .strftime("%Y%m%dT%H%M%SZ")
        .to_string();

    let signed = crate::sigv4::sign_r2_put(
        account_id,
        access_key_id,
        secret_access_key,
        bucket,
        R2_OBJECT_KEY,
        &body,
        &datetime,
    );

    let url = format!("https://{account_id}.r2.cloudflarestorage.com/{bucket}/{R2_OBJECT_KEY}");

    ureq::put(&url)
        .header("Authorization", &signed.authorization)
        .header("x-amz-date", &signed.x_amz_date)
        .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
        .header("Content-Type", "application/octet-stream")
        .send(&body[..])
        .context("Failed to upload bundle to R2")?;

    println!("R2: uploaded {bucket}/{R2_OBJECT_KEY}");
    Ok(())
}

fn push_kv(
    sh: &Shell,
    repo_dir: &str,
    account_id: &str,
    ns_id: &str,
    api_token: &str,
    full: bool,
) -> Result<()> {
    let head = cmd!(sh, "git -C {repo_dir} rev-parse HEAD")
        .read()
        .context("Failed to get HEAD commit")?;
    let head = head.trim();

    let all_entries = parse_all_entries(sh, repo_dir)?;

    let base_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/storage/kv/namespaces/{ns_id}"
    );
    let bearer = format!("Bearer {api_token}");

    let entries_to_sync: Vec<&(String, String)> = if full {
        all_entries.iter().collect()
    } else {
        let index_result = ureq::get(&format!("{base_url}/values/__index"))
            .header("Authorization", &bearer)
            .call();

        match index_result {
            Ok(mut resp) => {
                let json_str = resp.body_mut().read_to_string().unwrap_or_else(|e| {
                    eprintln!("KV: failed to read index response: {e}");
                    String::new()
                });
                if let Ok(index) = serde_json::from_str::<Value>(&json_str) {
                    let last_commit = index["commit"].as_str().unwrap_or("");

                    if last_commit == head {
                        println!("KV: no changes since last sync ({head})");
                        return Ok(());
                    }

                    let changed = cmd!(
                        sh,
                        "git -C {repo_dir} diff --name-only {last_commit} {head} -- entries/"
                    )
                    .read()
                    .context("Failed to get changed files")?;

                    let changed_months: HashSet<&str> = changed
                        .lines()
                        .filter_map(|f| f.strip_prefix("entries/"))
                        .filter_map(|f| f.strip_suffix(".md"))
                        .collect();

                    all_entries
                        .iter()
                        .filter(|(date, _)| changed_months.contains(&date[..7]))
                        .collect()
                } else {
                    all_entries.iter().collect()
                }
            }
            Err(e) => {
                eprintln!("KV: failed to fetch index, falling back to full sync: {e}");
                all_entries.iter().collect()
            }
        }
    };

    // Build bulk payload: entries to sync + __index
    let all_keys: Vec<&str> = all_entries.iter().map(|(k, _)| k.as_str()).collect();
    let index = json!({ "keys": all_keys, "commit": head });
    let index_str = serde_json::to_string(&index)?;

    let mut bulk: Vec<Value> = entries_to_sync
        .iter()
        .map(|(k, v)| json!({ "key": k, "value": v }))
        .collect();
    bulk.push(json!({ "key": "__index", "value": index_str }));

    let bulk_bytes = serde_json::to_vec(&bulk)?;

    ureq::put(&format!("{base_url}/bulk"))
        .header("Authorization", &bearer)
        .header("Content-Type", "application/json")
        .send(&bulk_bytes[..])
        .context("Failed to bulk put KV entries")?;

    println!(
        "KV: {} entries, updated __index ({} keys, commit {})",
        bulk.len() - 1,
        all_keys.len(),
        &head[..8]
    );
    Ok(())
}

impl Cmd for Push {
    fn run(self) -> Result<()> {
        let repo_dir = std::env::var("FADING_DIR").context("FADING_DIR env is not set")?;

        let target = self.target.as_deref();
        let do_r2 = matches!(target, None | Some("r2"));
        let do_kv = matches!(target, None | Some("kv"));

        if !do_r2 && !do_kv {
            anyhow::bail!(
                "Unknown target: '{}' (expected: r2, kv)",
                self.target.unwrap_or_default()
            );
        }

        let sh = Shell::new()?;
        let mut ok = true;

        let account_id = std::env::var("FADING_CLI_CF_ACCOUNT_ID")
            .context("FADING_CLI_CF_ACCOUNT_ID env is not set")?;

        if do_r2 {
            let bucket = std::env::var("FADING_CLI_R2_BUCKET")
                .context("FADING_CLI_R2_BUCKET env is not set")?;
            let access_key_id = std::env::var("FADING_CLI_R2_ACCESS_KEY_ID")
                .context("FADING_CLI_R2_ACCESS_KEY_ID env is not set")?;
            let secret_access_key = std::env::var("FADING_CLI_R2_SECRET_ACCESS_KEY")
                .context("FADING_CLI_R2_SECRET_ACCESS_KEY env is not set")?;
            if let Err(e) = push_r2(
                &sh,
                &repo_dir,
                &account_id,
                &bucket,
                &access_key_id,
                &secret_access_key,
            ) {
                eprintln!("R2 error: {e:?}");
                ok = false;
            }
        }

        if do_kv {
            let api_token = std::env::var("FADING_CLI_CF_API_TOKEN")
                .context("FADING_CLI_CF_API_TOKEN env is not set")?;
            let ns_id = std::env::var("FADING_CLI_KV_NAMESPACE_ID")
                .context("FADING_CLI_KV_NAMESPACE_ID env is not set")?;
            if let Err(e) = push_kv(&sh, &repo_dir, &account_id, &ns_id, &api_token, self.full) {
                eprintln!("KV error: {e:?}");
                ok = false;
            }
        }

        if !ok {
            anyhow::bail!("One or more push operations failed");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entries_skips_empty_days() {
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
        assert_eq!(entries[0].0, "2026-01-02");
        assert_eq!(entries[0].1, "今日は良い日だった。");
    }

    #[test]
    fn test_parse_entries_skips_placeholder() {
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
        assert_eq!(entries[0].0, "2026-01-02");
        assert_eq!(entries[0].1, "今日は良い日だった。");
    }

    #[test]
    fn test_parse_entries_multiple() {
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
        assert_eq!(entries[0].0, "2026-01-01");
        assert_eq!(entries[0].1, "一行目。");
        assert_eq!(entries[1].0, "2026-01-02");
        assert_eq!(entries[1].1, "二行目。\n続き。");
    }
}
