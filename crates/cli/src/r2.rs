use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::crypto::{decrypt, encrypt};
use crate::{Cmd, Pull, Push};

const R2_OBJECT_KEY: &str = "fading.bundle";

impl Cmd for Push {
    fn run(self, sh: Shell) -> Result<()> {
        let account_id = sh.var("FADING_CLI_CF_ACCOUNT_ID")?;
        let bucket = sh.var("FADING_CLI_R2_BUCKET")?;
        let access_key_id = sh.var("FADING_CLI_R2_ACCESS_KEY_ID")?;
        let secret_access_key = sh.var("FADING_CLI_R2_SECRET_ACCESS_KEY")?;

        let temp = sh.create_temp_dir()?;
        let bundle_path = temp.path().join(R2_OBJECT_KEY);
        let path = bundle_path
            .to_str()
            .context("Bundle path is not valid UTF-8")?;

        cmd!(sh, "git bundle create {path} --all").run()?;

        let key_hex = sh.var("FADING_CLI_ENCRYPTION_KEY")?;
        let body = sh.read_binary_file(&bundle_path)?;
        let encrypted = encrypt(&key_hex, &body)?;
        println!(
            "R2: bundle encrypted ({} B → {} B)",
            body.len(),
            encrypted.len()
        );
        let body = encrypted;

        let datetime = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .strftime("%Y%m%dT%H%M%SZ")
            .to_string();

        let signed = crate::sigv4::sign_r2_put(
            &account_id,
            &access_key_id,
            &secret_access_key,
            &bucket,
            R2_OBJECT_KEY,
            &body,
            &datetime,
        );

        let url =
            format!("https://{account_id}.r2.cloudflarestorage.com/{bucket}/{R2_OBJECT_KEY}");

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
}

impl Cmd for Pull {
    fn run(self, sh: Shell) -> Result<()> {
        let account_id = sh.var("FADING_CLI_CF_ACCOUNT_ID")?;
        let bucket = sh.var("FADING_CLI_R2_BUCKET")?;
        let access_key_id = sh.var("FADING_CLI_R2_ACCESS_KEY_ID")?;
        let secret_access_key = sh.var("FADING_CLI_R2_SECRET_ACCESS_KEY")?;

        let datetime = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .strftime("%Y%m%dT%H%M%SZ")
            .to_string();

        let signed = crate::sigv4::sign_r2_get(
            &account_id,
            &access_key_id,
            &secret_access_key,
            &bucket,
            R2_OBJECT_KEY,
            &datetime,
        );

        let url =
            format!("https://{account_id}.r2.cloudflarestorage.com/{bucket}/{R2_OBJECT_KEY}");

        let mut resp = ureq::get(&url)
            .header("Authorization", &signed.authorization)
            .header("x-amz-date", &signed.x_amz_date)
            .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
            .call()
            .context("Failed to download bundle from R2")?;

        let mut body = Vec::new();
        use std::io::Read;
        resp.body_mut()
            .as_reader()
            .read_to_end(&mut body)
            .context("Failed to read response body")?;

        let key_hex = sh.var("FADING_CLI_ENCRYPTION_KEY")?;
        let decrypted = decrypt(&key_hex, &body)?;
        println!("R2: bundle decrypted ({} B → {} B)", body.len(), decrypted.len());
        let body = decrypted;

        let temp = sh.create_temp_dir()?;
        let bundle_path = temp.path().join(R2_OBJECT_KEY);
        std::fs::write(&bundle_path, &body).context("Failed to write bundle to temp file")?;
        let path = bundle_path
            .to_str()
            .context("Bundle path is not valid UTF-8")?;

        cmd!(sh, "git bundle verify {path}").run()?;
        cmd!(sh, "git fetch {path}").run()?;

        println!("R2: fetched from {bucket}/{R2_OBJECT_KEY}");
        Ok(())
    }
}
