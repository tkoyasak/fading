use std::io::Read;
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use shiguredo_s3::{ChecksumAlgorithm, ChecksumMode, Client, Config, Credentials, S3Request};
use xshell::cmd;

use crate::{Cmd, Ctx, Get, Put, crypto::decrypt, crypto::encrypt};

const R2_OBJECT_KEY: &str = "fading.bundle";
const ERROR_CODE_REFERENCE: &str =
    "https://developers.cloudflare.com/r2/api/error-codes/#error-code-reference";

struct R2Config {
    bucket: String,
    encryption_key: String,
    client: Client,
}

impl R2Config {
    fn from_ctx(ctx: &Ctx) -> Result<Self> {
        let account_id = ctx.cf_account_id()?;
        let bucket = ctx.r2_bucket()?;
        let access_key_id = ctx.r2_access_key_id()?;
        let secret_access_key = ctx.r2_secret_access_key()?;
        let encryption_key = ctx.encryption_key()?;
        let config = Config::builder()
            .region("auto")
            .credentials_provider(Credentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
                "Static",
            ))
            .endpoint(format!("https://{account_id}.r2.cloudflarestorage.com"))
            .force_path_style(true)
            .build()
            .context("Failed to build S3 config")?;
        Ok(Self {
            bucket,
            encryption_key,
            client: Client::from_conf(config),
        })
    }
}

fn request_url(req: &S3Request) -> String {
    assert!(req.https, "R2 requires HTTPS");
    format!("https://{}:{}{}", req.host, req.port, req.uri)
}

fn r2_error(e: ureq::Error, op: &str) -> anyhow::Error {
    match e {
        ureq::Error::StatusCode(code) => {
            anyhow!("R2 {op} failed: HTTP {code}\nSee: {ERROR_CODE_REFERENCE}")
        }
        e => anyhow::Error::from(e),
    }
}

impl Cmd for Put {
    fn run(self, ctx: Ctx) -> Result<()> {
        let cfg = R2Config::from_ctx(&ctx)?;
        let sh = ctx.sh;

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
            .checksum_algorithm(ChecksumAlgorithm::Crc64Nvme)
            .build_request(SystemTime::now())
            .context("Failed to build PUT request")?;

        let url = request_url(&req);
        let mut builder = ureq::put(&url);
        for (name, value) in &req.headers {
            builder = builder.header(name, value);
        }
        builder
            .send(&req.body[..])
            .map_err(|e| r2_error(e, "upload"))?;

        println!("R2: uploaded {}/{R2_OBJECT_KEY}", cfg.bucket);
        Ok(())
    }
}

impl Cmd for Get {
    fn run(self, ctx: Ctx) -> Result<()> {
        let cfg = R2Config::from_ctx(&ctx)?;
        let sh = ctx.sh;

        let req = cfg
            .client
            .get_object()
            .bucket(&cfg.bucket)
            .key(R2_OBJECT_KEY)
            .checksum_mode(ChecksumMode::Enabled)
            .build_request(SystemTime::now())
            .context("Failed to build GET request")?;

        let url = request_url(&req);
        let mut builder = ureq::get(&url);
        for (name, value) in &req.headers {
            builder = builder.header(name, value);
        }
        let mut resp = builder.call().map_err(|e| r2_error(e, "download"))?;

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
