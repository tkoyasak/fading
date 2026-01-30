use std::fmt::Write;

use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use jiff::{ToSpan, Zoned, civil::Date};
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Generate};

impl Cmd for Generate {
    fn run(self) -> anyhow::Result<()> {
        let entry = Entry::new()?;
        GitHub::new(entry)?.create_pull_request()
    }
}

#[derive(Debug)]
struct Entry {
    pub id: Date,
    pub path: String,
    pub content: String,
}

impl Entry {
    fn new() -> Result<Self> {
        let today = Zoned::now().date();
        let id = today.first_of_month();
        let path = format!("entries/{}.md", id.strftime("%Y-%m"));
        let content = String::with_capacity(1_000);

        let mut entry = Self { id, path, content };
        entry.generate()?;
        Ok(entry)
    }

    fn metadata_block(&self) -> String {
        let id = self.id.strftime("%Y-%m");
        let today = Zoned::now().strftime("%Y-%m-%d");

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

        for date in self
            .id
            .series(1.days())
            .take(self.id.days_in_month() as usize)
        {
            write!(
                &mut self.content,
                "\n###### {}\n\n\n",
                date.strftime("%Y-%m-%d %a")
            )?;
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
struct GitHub {
    repo: String,
    sha: String,
    path: String,
    branch: String,
    message: String,
    encoded: String,
}

impl GitHub {
    fn new(entry: Entry) -> Result<Self> {
        let repo = std::env::var("GITHUB_REPOSITORY")?;
        let sha = std::env::var("GITHUB_SHA")?;

        let id = entry.id.strftime("%Y-%m");
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
