use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::crypto::{decrypt, encrypt};
use crate::sigv4::R2SignRequest;
use crate::{Cmd, Pull, Push};

const R2_OBJECT_KEY: &str = "fading.bundle";

struct R2Config {
    account_id: String,
    bucket: String,
    access_key_id: String,
    secret_access_key: String,
    encryption_key: String,
}

impl R2Config {
    fn from_env(sh: &Shell) -> Result<Self> {
        Ok(Self {
            account_id: sh.var("FADING_CLI_CF_ACCOUNT_ID")?,
            bucket: sh.var("FADING_CLI_R2_BUCKET")?,
            access_key_id: sh.var("FADING_CLI_R2_ACCESS_KEY_ID")?,
            secret_access_key: sh.var("FADING_CLI_R2_SECRET_ACCESS_KEY")?,
            encryption_key: sh.var("FADING_CLI_ENCRYPTION_KEY")?,
        })
    }

    fn url(&self) -> String {
        format!(
            "https://{}.r2.cloudflarestorage.com/{}/{R2_OBJECT_KEY}",
            self.account_id, self.bucket
        )
    }

    fn sign_request<'a>(&'a self, datetime: &'a str) -> R2SignRequest<'a> {
        R2SignRequest {
            account_id: &self.account_id,
            access_key_id: &self.access_key_id,
            secret_access_key: &self.secret_access_key,
            bucket: &self.bucket,
            key: R2_OBJECT_KEY,
            datetime,
        }
    }
}

fn datetime_now() -> String {
    jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::UTC)
        .strftime("%Y%m%dT%H%M%SZ")
        .to_string()
}

impl Cmd for Push {
    fn run(self, sh: Shell) -> Result<()> {
        let cfg = R2Config::from_env(&sh)?;

        let temp = sh.create_temp_dir()?;
        let bundle_path = temp.path().join(R2_OBJECT_KEY);
        let path = bundle_path
            .to_str()
            .context("Bundle path is not valid UTF-8")?;

        cmd!(sh, "git bundle create {path} --all").run()?;

        let body = sh.read_binary_file(&bundle_path)?;
        let encrypted = encrypt(&cfg.encryption_key, &body)?;
        println!(
            "R2: bundle encrypted ({} B → {} B)",
            body.len(),
            encrypted.len()
        );
        let body = encrypted;

        let datetime = datetime_now();
        let signed = crate::sigv4::sign_r2_put(&cfg.sign_request(&datetime), &body)?;
        let url = cfg.url();

        ureq::put(&url)
            .header("Authorization", &signed.authorization)
            .header("x-amz-date", &signed.x_amz_date)
            .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
            .header("Content-Type", "application/octet-stream")
            .send(&body[..])
            .context("Failed to upload bundle to R2")?;

        println!("R2: uploaded {}/{R2_OBJECT_KEY}", cfg.bucket);
        Ok(())
    }
}

impl Cmd for Pull {
    fn run(self, sh: Shell) -> Result<()> {
        let cfg = R2Config::from_env(&sh)?;

        let datetime = datetime_now();
        let signed = crate::sigv4::sign_r2_get(&cfg.sign_request(&datetime))?;
        let url = cfg.url();

        let mut resp = ureq::get(&url)
            .header("Authorization", &signed.authorization)
            .header("x-amz-date", &signed.x_amz_date)
            .header("x-amz-content-sha256", &signed.x_amz_content_sha256)
            .call()
            .context("Failed to download bundle from R2")?;

        const MAX_BUNDLE_SIZE: u64 = 500 * 1024 * 1024;

        let mut body = Vec::new();
        use std::io::Read;
        resp.body_mut()
            .as_reader()
            .take(MAX_BUNDLE_SIZE)
            .read_to_end(&mut body)
            .context("Failed to read response body")?;

        let decrypted = decrypt(&cfg.encryption_key, &body)?;
        println!(
            "R2: bundle decrypted ({} B → {} B)",
            body.len(),
            decrypted.len()
        );
        let body = decrypted;

        let temp = sh.create_temp_dir()?;
        let bundle_path = temp.path().join(R2_OBJECT_KEY);
        std::fs::write(&bundle_path, &body).context("Failed to write bundle to temp file")?;
        let path = bundle_path
            .to_str()
            .context("Bundle path is not valid UTF-8")?;

        cmd!(sh, "git bundle verify {path}").run()?;
        cmd!(sh, "git fetch {path}").run()?;

        println!("R2: fetched from {}/{R2_OBJECT_KEY}", cfg.bucket);
        Ok(())
    }
}
