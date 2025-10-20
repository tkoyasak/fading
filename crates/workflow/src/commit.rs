//! NOTE: Using GitHub App tokens (like GITHUB_TOKEN in GitHub Actions)
//! automatically creates verified commits when using the GitHub API.
//! This is because GitHub can cryptographically verify that the commit
//! originated from the authenticated GitHub App/bot.
//!
//! https://github.blog/engineering/platform-security/commit-signing-support-for-bots-and-other-github-apps/
//! https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification#signature-verification-for-bots

use std::env;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use xshell::{Shell, cmd};

use crate::entry::Entry;

#[derive(Debug)]
struct GitHubContext {
    repo: String,
    sha: String,
    path: String,
    branch: String,
    message: String,
    encoded: String,
}

impl GitHubContext {
    fn new(entry: Entry) -> Result<Self> {
        let repo = env::var("GITHUB_REPOSITORY")?;
        let sha = env::var("GITHUB_SHA")?;

        let id = entry.id.format("%Y-%m");
        let branch = format!("entry/{id}");
        let message = format!("cron: generated entry for {id}");
        let encoded = STANDARD.encode(entry.content.as_bytes());

        Ok(GitHubContext {
            repo: repo.to_string(),
            sha: sha.to_string(),
            path: entry.path,
            branch,
            message,
            encoded,
        })
    }

    pub fn create_pull_request(&self) -> Result<()> {
        let sh = Shell::new()?;

        // Create a new reference.
        {
            let repo = &self.repo;
            let branch = &self.branch;
            let sha = &self.sha;
            cmd!(
                sh,
                "gh api repos/{repo}/git/refs -X POST -f ref=refs/heads/{branch} -f sha={sha}"
            )
            .run()
            .context("Failed to create branch.")?;
        }

        // Creates a new file.
        {
            let repo = &self.repo;
            let path = &self.path;
            let message = &self.message;
            let encoded = &self.encoded;
            let branch = &self.branch;
            cmd!(
                sh,
                "gh api repos/{repo}/contents/{path} -X PUT -f content={encoded} -f branch={branch} -f message={message}"
            )
            .run()
            .context("Failed to create file.")?;
        }

        // Create a pull request on GitHub.
        {
            let repo = &self.repo;
            let branch = &self.branch;
            let message = &self.message;
            cmd!(
                sh,
                "gh pr create --repo {repo} --base main --head {branch} --title {message} --body ''"
            )
            .run()
            .context("Failed to create pull request.")?;
        }

        Ok(())
    }
}

pub fn create_content(entry: Entry) -> Result<()> {
    GitHubContext::new(entry)?.create_pull_request()
}
