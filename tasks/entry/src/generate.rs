use std::fmt::Write;

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{Datelike, Duration, Local, NaiveDate};
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Generate};

impl Cmd for Generate {
    fn run(self) -> anyhow::Result<()> {
        let entry = Entry::new()?;
        Context::new(entry)?.create_pull_request()
    }
}

#[derive(Debug)]
struct Entry {
    pub id: NaiveDate,
    pub path: String,
    pub content: String,
}

impl Entry {
    fn new() -> Result<Self> {
        let id = Local::now().date_naive().with_day(1).unwrap();
        let path = format!("entries/{}.md", id.format("%Y-%m"));
        let content = String::with_capacity(1_000);

        let mut entry = Self { id, path, content };
        entry.generate()?;
        Ok(entry)
    }

    fn metadata_block(&self) -> String {
        let id = self.id.format("%Y-%m");
        let today = Local::now().format("%Y-%m-%d");

        format!(
            r#"+++
id = "{id}"
created = {today}
modified = {today}
+++
"#
        )
    }

    fn generate(&mut self) -> Result<()> {
        let metadata = self.metadata_block();
        write!(&mut self.content, "{metadata}")?;

        let n = self.id.num_days_in_month();
        let mut cur = self.id;

        for _ in 0..n {
            let date = cur.format("%Y-%m-%d %a");
            write!(&mut self.content, "\n###### {date}\n\n\n")?;

            cur += Duration::days(1);
        }

        Ok(())
    }
}

// GitHub Apps can use bots to sign commits. If a commit has a bot signature that is
// cryptographically verifiable, GitHub marks the commit as verified.
//
// Signature verification for bots will only work if the request is verified and authenticated as
// the GitHub App or bot and contains no custom author information, custom committer information,
// and no custom signature information, such as Commits API. [^1] [^2]
//
// [^1]: <https://docs.github.com/en/authentication/managing-commit-signature-verification/about-commit-signature-verification#signature-verification-for-bots>
// [^2]: <https://github.blog/engineering/platform-security/commit-signing-support-for-bots-and-other-github-apps>

#[derive(Debug)]
struct Context {
    repo: String,
    sha: String,
    path: String,
    branch: String,
    message: String,
    encoded: String,
}

impl Context {
    fn new(entry: Entry) -> Result<Self> {
        let repo = std::env::var("GITHUB_REPOSITORY")?;
        let sha = std::env::var("GITHUB_SHA")?;

        let id = entry.id.format("%Y-%m");
        let branch = format!("entry/{id}");
        let message = format!("cron: generated entry for {id}");

        let encoded = STANDARD.encode(entry.content);

        Ok(Self {
            repo: repo.to_string(),
            sha: sha.to_string(),
            path: entry.path,
            branch,
            message,
            encoded,
        })
    }

    fn create_pull_request(&self) -> Result<()> {
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
            .run()?;
        }

        // Create a new file.
        {
            let repo = &self.repo;
            let path = &self.path;
            let encoded = &self.encoded;
            let branch = &self.branch;
            let message = &self.message;
            cmd!(
                sh,
                "gh api repos/{repo}/contents/{path} -X PUT -f content={encoded} -f branch={branch} -f message={message}"
            )
            .run()?;
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
            .run()?;
        }

        Ok(())
    }
}
