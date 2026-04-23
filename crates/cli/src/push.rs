use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use jiff::civil::Date;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use xshell::{Shell, cmd};

use crate::crypto::encrypt;
use crate::{Cmd, Push};

const R2_OBJECT_KEY: &str = "fading.bundle";

impl Cmd for Push {
    fn run(self, sh: Shell) -> Result<()> {
        let target = self.target.as_deref();
        let do_r2 = matches!(target, None | Some("r2"));
        let do_kv = matches!(target, None | Some("kv"));

        if !do_r2 && !do_kv {
            anyhow::bail!(
                "Unknown target: '{}' (expected: r2, kv)",
                self.target.unwrap_or_default()
            );
        }
        let mut ok = true;

        let account_id = sh.var("FADING_CLI_CF_ACCOUNT_ID")?;

        if do_r2 {
            let bucket = sh.var("FADING_CLI_R2_BUCKET")?;
            let access_key_id = sh.var("FADING_CLI_R2_ACCESS_KEY_ID")?;
            let secret_access_key = sh.var("FADING_CLI_R2_SECRET_ACCESS_KEY")?;
            if let Err(e) = push_r2(
                &sh,
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
            let api_token = sh.var("FADING_CLI_CF_API_TOKEN")?;
            let ns_id = sh.var("FADING_CLI_KV_NAMESPACE_ID")?;
            if let Err(e) = push_kv(&sh, &account_id, &ns_id, &api_token, self.full) {
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

fn push_r2(
    sh: &Shell,
    account_id: &str,
    bucket: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> Result<()> {
    let temp = sh.create_temp_dir()?;
    let bundle_path = temp.path().join(R2_OBJECT_KEY);
    let path = bundle_path
        .to_str()
        .context("Bundle path is not valid UTF-8")?;

    cmd!(sh, "git bundle create {path} --all").run()?;

    let body = sh.read_binary_file(&bundle_path)?;
    let body = match sh.var("FADING_CLI_ENCRYPTION_KEY") {
        Ok(key_hex) => {
            let encrypted = encrypt(&key_hex, &body)?;
            println!(
                "R2: bundle encrypted ({} B → {} B)",
                body.len(),
                encrypted.len()
            );
            encrypted
        }
        Err(_) => body,
    };

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

fn push_kv(sh: &Shell, account_id: &str, ns_id: &str, api_token: &str, full: bool) -> Result<()> {
    let head = cmd!(sh, "git rev-parse HEAD").read()?;
    let head = head.trim();
    let all_entries = read_all_entries(sh)?;

    let base_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/storage/kv/namespaces/{ns_id}"
    );
    let bearer = format!("Bearer {api_token}");

    let entries_to_sync: Vec<(&str, &str)> = if full {
        all_entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
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

                    let changed =
                        cmd!(sh, "git diff --name-only {last_commit} {head} -- entries/").read()?;

                    let changed_months: HashSet<&str> = changed
                        .lines()
                        .filter_map(|f| f.strip_prefix("entries/"))
                        .filter_map(|f| f.strip_suffix(".md"))
                        .collect();

                    all_entries
                        .iter()
                        .filter(|(date, _)| {
                            // date is YYYYMMDD; changed_months are YYYY-MM
                            let month = format!("{}-{}", &date[..4], &date[4..6]);
                            changed_months.contains(month.as_str())
                        })
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect()
                } else {
                    all_entries
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect()
                }
            }
            Err(e) => {
                eprintln!("KV: failed to fetch index, falling back to full sync: {e}");
                all_entries
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect()
            }
        }
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

fn parse_entries(content: &str) -> Vec<(Date, String)> {
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
