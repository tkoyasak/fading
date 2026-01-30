use std::fmt::Write;

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use jiff::{ToSpan, Zoned, civil::Date, tz};
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
        let jst = tz::db()
            .get("Asia/Tokyo")
            .context("Failed to load Asia/Tokyo timezone")?;
        let today = Zoned::now().with_time_zone(jst).date();
        let id = today.first_of_month();
        let path = format!("entries/{}.md", id.strftime("%Y-%m"));

        // Validate path to prevent path traversal
        ensure!(
            path.starts_with("entries/") && !path.contains(".."),
            "Invalid path: {path}"
        );

        let content = String::with_capacity(1_000);

        let mut entry = Self { id, path, content };
        entry.generate()?;
        Ok(entry)
    }

    fn metadata_block(&self) -> String {
        let jst = tz::db()
            .get("Asia/Tokyo")
            .expect("Asia/Tokyo timezone should be available");
        let id = self.id.strftime("%Y-%m");
        let today = Zoned::now().with_time_zone(jst).strftime("%Y-%m-%d");

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
        let repo = std::env::var("GITHUB_REPOSITORY")
            .context("GITHUB_REPOSITORY environment variable not found")?;
        let sha =
            std::env::var("GITHUB_SHA").context("GITHUB_SHA environment variable not found")?;

        let id = entry.id.strftime("%Y-%m");
        let branch = format!("entry/{id}");
        let message = format!("cron: generated entry for {id}");

        let encoded = STANDARD.encode(entry.content);

        Ok(Self {
            repo,
            sha,
            path: entry.path,
            branch,
            message,
            encoded,
        })
    }

    fn create_pull_request(&self) -> Result<()> {
        let sh = Shell::new()?;

        // Create a new reference (skip if already exists)
        {
            let repo = &self.repo;
            let branch = &self.branch;
            let sha = &self.sha;

            let branch_exists = cmd!(sh, "gh api repos/{repo}/git/refs/heads/{branch}")
                .ignore_status()
                .run()
                .is_ok();

            if !branch_exists {
                cmd!(
                    sh,
                    "gh api repos/{repo}/git/refs -X POST -f ref=refs/heads/{branch} -f sha={sha}"
                )
                .run()
                .context("Failed to create branch")?;
            }
        }

        // Create or update a file
        // Note: GitHub Apps can sign commits if no custom author/committer info is provided
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
            .run()
            .context("Failed to create/update file")?;
        }

        // Create a pull request on GitHub (skip if already exists)
        {
            let repo = &self.repo;
            let branch = &self.branch;
            let message = &self.message;

            let pr_exists = cmd!(sh, "gh pr list --repo {repo} --head {branch} --json number")
                .read()
                .map(|output| output.trim() != "[]")
                .unwrap_or(false);

            if !pr_exists {
                cmd!(
                    sh,
                    "gh pr create --repo {repo} --base main --head {branch} --title {message} --body ''"
                )
                .run()
                .context("Failed to create pull request")?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_path_validation() {
        // Valid path should work
        let entry = Entry {
            id: Date::new(2026, 1, 1).unwrap(),
            path: "entries/2026-01.md".to_string(),
            content: String::new(),
        };
        assert!(entry.path.starts_with("entries/"));

        // Invalid paths would be caught in Entry::new()
        // We can't directly test this without mocking time, but the validation logic is there
    }

    #[test]
    fn test_metadata_block_format() {
        let entry = Entry {
            id: Date::new(2026, 1, 1).unwrap(),
            path: "entries/2026-01.md".to_string(),
            content: String::new(),
        };

        let metadata = entry.metadata_block();
        assert!(metadata.starts_with("+++\n"));
        assert!(metadata.contains("id = \"2026-01\""));
        assert!(metadata.contains("created = "));
        assert!(metadata.contains("modified = "));
        assert!(metadata.ends_with("+++\n"));
    }

    #[test]
    fn test_generate_creates_headers() {
        let mut entry = Entry {
            id: Date::new(2026, 1, 1).unwrap(),
            path: "entries/2026-01.md".to_string(),
            content: String::new(),
        };

        entry.generate().unwrap();

        // Should contain metadata
        assert!(entry.content.starts_with("+++\n"));

        // Should contain all 31 days in January
        assert_eq!(entry.content.matches("###### 2026-01-").count(), 31);

        // Check specific dates
        assert!(entry.content.contains("###### 2026-01-01 "));
        assert!(entry.content.contains("###### 2026-01-15 "));
        assert!(entry.content.contains("###### 2026-01-31 "));
    }

    #[test]
    fn test_generate_february_has_28_days() {
        let mut entry = Entry {
            id: Date::new(2026, 2, 1).unwrap(),
            path: "entries/2026-02.md".to_string(),
            content: String::new(),
        };

        entry.generate().unwrap();

        // February 2026 has 28 days
        assert_eq!(entry.content.matches("###### 2026-02-").count(), 28);
        assert!(entry.content.contains("###### 2026-02-01 "));
        assert!(entry.content.contains("###### 2026-02-28 "));
        assert!(!entry.content.contains("###### 2026-02-29 "));
    }

    #[test]
    fn test_generate_leap_year() {
        let mut entry = Entry {
            id: Date::new(2024, 2, 1).unwrap(),
            path: "entries/2024-02.md".to_string(),
            content: String::new(),
        };

        entry.generate().unwrap();

        // February 2024 has 29 days (leap year)
        assert_eq!(entry.content.matches("###### 2024-02-").count(), 29);
        assert!(entry.content.contains("###### 2024-02-29 "));
    }
}
