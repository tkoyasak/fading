use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use xshell::{Shell, cmd};

const CACHE_FILE: &str = ".cache/fading-cli.json";

pub(crate) struct Cache {
    pub commit: String,
    /// YYYYMMDD → entry content
    pub entries: HashMap<String, String>,
}

/// Load the cache, rebuilding from git if the commit has changed or the
/// integrity check fails.
pub(crate) fn load(sh: &Shell, repo_dir: &str, home: &str) -> Result<Cache> {
    let head = cmd!(sh, "git -C {repo_dir} rev-parse HEAD")
        .read()
        .context("Failed to get HEAD commit")?;
    let head = head.trim().to_string();

    let cache_path = Path::new(home).join(CACHE_FILE);

    if let Ok(raw) = std::fs::read_to_string(&cache_path)
        && let Ok(cached) = serde_json::from_str::<serde_json::Value>(&raw)
    {
        let cached_commit = cached["commit"].as_str().unwrap_or("");
        let cached_integrity = cached["integrity"].as_str().unwrap_or("");

        if cached_commit == head
            && let Ok(entries) =
                serde_json::from_value::<BTreeMap<String, String>>(cached["entries"].clone())
        {
            let entries_json =
                serde_json::to_string(&entries).context("Failed to serialize entries")?;
            if compute_integrity(&head, &entries_json) == cached_integrity {
                return Ok(Cache {
                    commit: head,
                    entries: entries.into_iter().collect(),
                });
            }
        }
    }

    // Cache miss or integrity failure — rebuild from git
    let entries = build_from_git(sh, repo_dir)?;
    save(&cache_path, &head, &entries)?;

    Ok(Cache {
        commit: head,
        entries,
    })
}

fn build_from_git(sh: &Shell, repo_dir: &str) -> Result<HashMap<String, String>> {
    let names = cmd!(sh, "git -C {repo_dir} ls-tree --name-only HEAD -- entries/")
        .read()
        .context("Failed to list entries at HEAD")?;

    let mut entries = HashMap::new();
    for name in names.lines().filter(|f| f.ends_with(".md")) {
        let content = cmd!(sh, "git -C {repo_dir} show HEAD:{name}")
            .read()
            .with_context(|| format!("Failed to read {name} at HEAD"))?;
        for (date, text) in crate::parse::parse_entries(&content) {
            entries.insert(date.strftime("%Y%m%d").to_string(), text);
        }
    }
    Ok(entries)
}

fn save(path: &Path, commit: &str, entries: &HashMap<String, String>) -> Result<()> {
    // BTreeMap for deterministic key ordering — required for stable integrity hash
    let sorted: BTreeMap<&str, &str> = entries
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let entries_json = serde_json::to_string(&sorted).context("Failed to serialize entries")?;
    let integrity = compute_integrity(commit, &entries_json);

    let cache = serde_json::json!({
        "commit": commit,
        "integrity": integrity,
        "entries": sorted,
    });

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).context("Failed to create cache dir")?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&cache)?)
        .context("Failed to write cache file")?;
    Ok(())
}

/// SHA-256 of `commit || entries_json` (entries serialized with sorted keys).
fn compute_integrity(commit: &str, entries_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(commit.as_bytes());
    hasher.update(entries_json.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}
