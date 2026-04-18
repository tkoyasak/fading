use std::collections::HashSet;

use anyhow::{Context, Result};
use serde_json::{Value, json};
use xshell::{Shell, cmd};

use crate::{Cmd, Push};

const R2_OBJECT_KEY: &str = "fading.bundle";

fn push_r2(
    sh: &Shell,
    repo_dir: &str,
    account_id: &str,
    bucket: &str,
    access_key_id: &str,
    secret_access_key: &str,
) -> Result<()> {
    let sh = sh.clone().with_current_dir(repo_dir);
    let temp = sh.create_temp_dir().context("Failed to create temp dir")?;
    let bundle_path = temp.path().join(R2_OBJECT_KEY);
    let bundle_path_str = bundle_path
        .to_str()
        .context("Bundle path is not valid UTF-8")?;

    cmd!(sh, "git bundle create {bundle_path_str} --all")
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
    let sh = sh.clone().with_current_dir(repo_dir);
    let head = cmd!(sh, "git rev-parse HEAD")
        .read()
        .context("Failed to get HEAD commit")?;
    let head = head.trim();
    let all_entries = crate::parse::read_all_entries(repo_dir)?;

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

                    let changed = cmd!(sh, "git diff --name-only {last_commit} {head} -- entries/")
                        .read()
                        .context("Failed to get changed files")?;

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

impl Cmd for Push {
    fn run(self) -> Result<()> {
        let sh = Shell::new()?;
        let repo_dir = sh
            .var("FADING_HOME")
            .context("FADING_HOME env is not set")?;

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

        let account_id = sh
            .var("FADING_CLI_CF_ACCOUNT_ID")
            .context("FADING_CLI_CF_ACCOUNT_ID env is not set")?;

        if do_r2 {
            let bucket = sh
                .var("FADING_CLI_R2_BUCKET")
                .context("FADING_CLI_R2_BUCKET env is not set")?;
            let access_key_id = sh
                .var("FADING_CLI_R2_ACCESS_KEY_ID")
                .context("FADING_CLI_R2_ACCESS_KEY_ID env is not set")?;
            let secret_access_key = sh
                .var("FADING_CLI_R2_SECRET_ACCESS_KEY")
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
            let api_token = sh
                .var("FADING_CLI_CF_API_TOKEN")
                .context("FADING_CLI_CF_API_TOKEN env is not set")?;
            let ns_id = sh
                .var("FADING_CLI_KV_NAMESPACE_ID")
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
