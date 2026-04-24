use std::io::Read;

use anyhow::{Context, Result};
use shiguredo_s3::{Credential, S3Client, S3Config, S3Request};
use xshell::{Shell, cmd};

use crate::crypto::{decrypt, encrypt};
use crate::{Cmd, Pull, Push};

const R2_OBJECT_KEY: &str = "fading.bundle";

struct R2Config {
    bucket: String,
    encryption_key: String,
    client: S3Client,
}

impl R2Config {
    fn from_env(sh: &Shell) -> Result<Self> {
        let account_id = sh.var("FADING_CLI_CF_ACCOUNT_ID")?;
        let bucket = sh.var("FADING_CLI_R2_BUCKET")?;
        let access_key_id = sh.var("FADING_CLI_R2_ACCESS_KEY_ID")?;
        let secret_access_key = sh.var("FADING_CLI_R2_SECRET_ACCESS_KEY")?;
        let encryption_key = sh.var("FADING_CLI_ENCRYPTION_KEY")?;
        let config = S3Config::builder()
            .region("auto")
            .credential(Credential::new(access_key_id, secret_access_key))
            .endpoint(format!("https://{account_id}.r2.cloudflarestorage.com"))
            .use_path_style(true)
            .build()
            .context("Failed to build S3 config")?;
        Ok(Self {
            bucket,
            encryption_key,
            client: S3Client::new(config),
        })
    }
}

fn request_url(req: &S3Request) -> String {
    let scheme = if req.https { "https" } else { "http" };
    format!("{scheme}://{}:{}{}", req.host, req.port, req.uri)
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

        let req = cfg
            .client
            .put_object()
            .bucket(&cfg.bucket)
            .key(R2_OBJECT_KEY)
            .body(encrypted)
            .content_type("application/octet-stream")
            .checksum_algorithm("CRC64NVME")
            .build_request()
            .context("Failed to build PUT request")?;

        let url = request_url(&req);
        let mut builder = ureq::put(&url);
        for (name, value) in &req.headers {
            builder = builder.header(name, value);
        }
        builder
            .send(&req.body[..])
            .context("Failed to upload bundle to R2")?;

        println!("R2: uploaded {}/{R2_OBJECT_KEY}", cfg.bucket);
        Ok(())
    }
}

impl Cmd for Pull {
    fn run(self, sh: Shell) -> Result<()> {
        let cfg = R2Config::from_env(&sh)?;

        let req = cfg
            .client
            .get_object()
            .bucket(&cfg.bucket)
            .key(R2_OBJECT_KEY)
            .checksum_mode("ENABLED")
            .build_request()
            .context("Failed to build GET request")?;

        let url = request_url(&req);
        let mut builder = ureq::get(&url);
        for (name, value) in &req.headers {
            builder = builder.header(name, value);
        }
        let mut resp = builder
            .call()
            .context("Failed to download bundle from R2")?;

        const MAX_BUNDLE_SIZE: u64 = 500 * 1024 * 1024;
        let mut body = Vec::new();
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

        let temp = sh.create_temp_dir()?;
        let bundle_path = temp.path().join(R2_OBJECT_KEY);
        sh.write_file(&bundle_path, &decrypted)?;
        let path = bundle_path
            .to_str()
            .context("Bundle path is not valid UTF-8")?;

        cmd!(sh, "git bundle verify {path}").run()?;
        cmd!(sh, "git fetch {path}").run()?;

        println!("R2: fetched from {}/{R2_OBJECT_KEY}", cfg.bucket);
        Ok(())
    }
}
