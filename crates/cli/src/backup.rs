use anyhow::{Context, Result};
use xshell::{Shell, cmd};

use crate::{Backup, Cmd};

const BUNDLE_PATH: &str = "/tmp/fading.bundle";
const R2_OBJECT_KEY: &str = "fading.bundle";

impl Cmd for Backup {
    fn run(self) -> Result<()> {
        // Get repository path and R2 bucket from environment variables
        let repo_dir = std::env::var("FADING_DIR").context("FADING_DIR env is not set")?;
        let bucket =
            std::env::var("FADING_R2_BUCKET").context("FADING_R2_BUCKET env is not set")?;

        let sh = Shell::new()?;

        // Create git bundle of the entire repository
        cmd!(sh, "git -C {repo_dir} bundle create {BUNDLE_PATH} --all")
            .run()
            .context("Failed to create git bundle")?;

        // Upload to Cloudflare R2 (always overwrites latest)
        cmd!(
            sh,
            "wrangler r2 object put {bucket}/{R2_OBJECT_KEY} --file {BUNDLE_PATH}"
        )
        .run()
        .context("Failed to upload bundle to R2")?;

        println!("Backup uploaded to R2: {bucket}/{R2_OBJECT_KEY}");
        Ok(())
    }
}
