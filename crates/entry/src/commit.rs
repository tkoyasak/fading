//! NOTE: Using GitHub App tokens (like GITHUB_TOKEN in GitHub Actions)
//! automatically creates verified commits when using the GitHub API.
//! This is because GitHub can cryptographically verify that the commit
//! originated from the authenticated GitHub App/bot.
//!
//! https://github.blog/engineering/platform-security/commit-signing-support-for-bots-and-other-github-apps/
//! https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification#signature-verification-for-bots

use std::env;

use anyhow::{Result, bail};
use octocrab::{Octocrab, params::repos::Reference};

use crate::entry::Entry;

#[derive(Debug)]
struct GitHubClient {
    octocrab: Octocrab,
    owner: String,
    repo: String,
    main_sha: String,
}

impl GitHubClient {
    fn new() -> Result<Self> {
        let token = env::var("GH_TOKEN")?;
        let octocrab = Octocrab::builder().personal_token(token).build()?;

        let repositry = env::var("GITHUB_REPOSITORY")?;
        let (owner, repo) = repositry.split_once('/').unwrap();

        let main_sha = env::var("GITHUB_SHA")?;

        Ok(GitHubClient {
            octocrab,
            owner: owner.to_string(),
            repo: repo.to_string(),
            main_sha: main_sha.to_string(),
        })
    }

    pub async fn commit_changes(&self, entry: Entry) -> Result<()> {
        let branch_name = entry.id.format("%Y-%m").to_string();

        if self
            .octocrab
            .repos(&self.owner, &self.repo)
            .get_content()
            .path(&entry.path)
            .send()
            .await
            .is_ok()
        {
            bail!("'{}' already exists", &entry.path);
        }

        self.octocrab
            .repos(&self.owner, &self.repo)
            .create_ref(&Reference::Branch(branch_name.clone()), &self.main_sha)
            .await?;

        self.octocrab
            .repos(&self.owner, &self.repo)
            .create_file(
                &entry.path,
                format!("generated entry for {branch_name}"),
                &entry.content,
            )
            .branch(&branch_name)
            .send()
            .await?;

        self.octocrab
            .pulls(&self.owner, &self.repo)
            .create(
                format!("cron: generated entry for {branch_name}"),
                &branch_name,
                "main",
            )
            .send()
            .await?;

        Ok(())
    }
}

pub async fn update_entry(entry: Entry) -> Result<()> {
    GitHubClient::new()?.commit_changes(entry).await
}
