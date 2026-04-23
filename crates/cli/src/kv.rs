use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use xshell::{Shell, cmd};

use crate::entry::parse_entries;
use crate::{Cmd, Sync};

impl Cmd for Sync {
    fn run(self, sh: Shell) -> Result<()> {
        let account_id = sh.var("FADING_CLI_CF_ACCOUNT_ID")?;
        let api_token = sh.var("FADING_CLI_CF_API_TOKEN")?;
        let ns_id = sh.var("FADING_CLI_KV_NAMESPACE_ID")?;

        let head = cmd!(sh, "git rev-parse HEAD").read()?;
        let head = head.trim();
        let all_entries = read_all_entries(&sh)?;

        let base_url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{account_id}/storage/kv/namespaces/{ns_id}"
        );
        let bearer = format!("Bearer {api_token}");

        let entries_to_sync = if self.full {
            Some(all_as_pairs(&all_entries))
        } else {
            determine_sync_entries(&sh, &all_entries, head, &base_url, &bearer)?
        };

        let Some(entries_to_sync) = entries_to_sync else {
            println!("KV: no changes since last sync ({head})");
            return Ok(());
        };

        // Build bulk payload: entries to sync + __index
        let mut all_keys: Vec<&str> = all_entries.keys().map(|k| k.as_str()).collect();
        all_keys.sort_unstable();
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
}

/// Returns `None` when no changes since last sync (skip entirely).
/// Returns `Some(entries)` for the entries that need syncing.
fn determine_sync_entries<'a>(
    sh: &Shell,
    all_entries: &'a HashMap<String, String>,
    head: &str,
    base_url: &str,
    bearer: &str,
) -> Result<Option<Vec<(&'a str, &'a str)>>> {
    let last_commit = match fetch_last_commit(base_url, bearer) {
        Ok(commit) => commit,
        Err(e) => {
            eprintln!("KV: failed to fetch index, falling back to full sync: {e}");
            return Ok(Some(all_as_pairs(all_entries)));
        }
    };

    if last_commit == head {
        return Ok(None);
    }

    if !is_git_sha(&last_commit) {
        eprintln!("KV: index has invalid commit hash, falling back to full sync");
        return Ok(Some(all_as_pairs(all_entries)));
    }

    let changed = cmd!(sh, "git diff --name-only {last_commit} {head} -- entries/").read()?;
    let changed_months: HashSet<&str> = changed
        .lines()
        .filter_map(|f| f.strip_prefix("entries/"))
        .filter_map(|f| f.strip_suffix(".md"))
        .collect();

    Ok(Some(
        all_entries
            .iter()
            .filter(|(date, _)| {
                // date is YYYYMMDD; changed_months are YYYY-MM
                let month = format!("{}-{}", &date[..4], &date[4..6]);
                changed_months.contains(month.as_str())
            })
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect(),
    ))
}

fn fetch_last_commit(base_url: &str, bearer: &str) -> Result<String> {
    let mut resp = ureq::get(&format!("{base_url}/values/__index"))
        .header("Authorization", bearer)
        .call()
        .context("Failed to fetch index")?;

    let json_str = resp
        .body_mut()
        .read_to_string()
        .context("Failed to read index response")?;

    let index: Value = serde_json::from_str(&json_str).context("Failed to parse index JSON")?;

    Ok(index["commit"].as_str().unwrap_or("").to_string())
}

fn all_as_pairs(entries: &HashMap<String, String>) -> Vec<(&str, &str)> {
    entries
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

fn is_git_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn read_all_entries(sh: &Shell) -> Result<HashMap<String, String>> {
    let names = cmd!(sh, "git ls-tree --name-only HEAD -- entries/").read()?;

    let names: Vec<String> = names
        .lines()
        .filter(|f| f.ends_with(".md"))
        .map(|s| s.to_string())
        .collect();

    let results: Vec<Result<Vec<(String, String)>>> = std::thread::scope(|s| {
        let handles: Vec<_> = names
            .iter()
            .map(|name| {
                let sh = sh.clone();
                s.spawn(move || -> Result<Vec<(String, String)>> {
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
    fn is_git_sha_valid() {
        assert!(is_git_sha("0123456789abcdef0123456789abcdef01234567"));
        assert!(is_git_sha("0123456789ABCDEF0123456789ABCDEF01234567")); // uppercase
        assert!(is_git_sha(&"a".repeat(40)));
    }

    #[test]
    fn is_git_sha_invalid() {
        assert!(!is_git_sha("")); // empty
        assert!(!is_git_sha(&"a".repeat(39))); // too short
        assert!(!is_git_sha(&"a".repeat(41))); // too long
        assert!(!is_git_sha(&"g".repeat(40))); // 'g' is not hex
        assert!(!is_git_sha("not-a-sha-at-all-and-definitely-not-40c")); // contains '-'
    }
}
