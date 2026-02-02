use std::fmt::Write;

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use jiff::{ToSpan, Zoned, civil::Date, tz};
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Generate};

const TIMEZONE: &str = "Asia/Tokyo";
const ENTRIES_DIR: &str = "entries/";
const BRANCH_PREFIX: &str = "entry/";
const BASE_BRANCH: &str = "main";

impl Cmd for Generate {
    fn run(self) -> anyhow::Result<()> {
        let entry = Entry::new()?;
        GitHub::new(entry)?.create_pull_request()
    }
}

/// Returns Asia/Tokyo timezone
fn jst() -> Result<tz::TimeZone> {
    tz::db()
        .get(TIMEZONE)
        .with_context(|| format!("Failed to load {TIMEZONE} timezone"))
}

#[derive(Debug, Clone)]
struct Entry {
    pub id: Date,
    pub path: String,
    pub content: String,
}

impl Entry {
    fn new() -> Result<Self> {
        let today = Zoned::now().with_time_zone(jst()?).date();
        let id = today.first_of_month();
        let path = format!("{ENTRIES_DIR}{}.md", id.strftime("%Y-%m"));

        // Validate path to prevent path traversal
        ensure!(
            path.starts_with(ENTRIES_DIR) && !path.contains(".."),
            "Invalid path: {path}"
        );

        let content = String::with_capacity(1_000);

        let mut entry = Self { id, path, content };
        entry.generate()?;
        Ok(entry)
    }

    fn metadata_block(&self) -> String {
        let id = self.id.strftime("%Y-%m");
        let today = Zoned::now()
            .with_time_zone(jst().expect("timezone should be available"))
            .strftime("%Y-%m-%d");

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
        use std::env::var;
        let repo = var("GITHUB_REPOSITORY").context("GITHUB_REPOSITORY env not found")?;
        let sha = var("GITHUB_SHA").context("GITHUB_SHA env not found")?;

        let id = entry.id.strftime("%Y-%m");
        let branch = format!("{BRANCH_PREFIX}{id}");
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

        self.create_branch(&sh)?;
        self.create_or_update_file(&sh)?;
        self.create_pr(&sh)?;

        Ok(())
    }

    fn create_branch(&self, sh: &Shell) -> Result<()> {
        let repo = &self.repo;
        let branch = &self.branch;
        let sha = &self.sha;

        // Check if branch exists - gh api returns non-zero exit code for 404
        // Note: Do NOT use ignore_status() here, as we need to detect 404 errors
        let branch_exists = cmd!(sh, "gh api repos/{repo}/git/refs/heads/{branch} --jq .ref")
            .read()
            .is_ok();

        if !branch_exists {
            cmd!(
                sh,
                "gh api repos/{repo}/git/refs -X POST -f ref=refs/heads/{branch} -f sha={sha}"
            )
            .run()
            .context("Failed to create branch")?;
        }

        Ok(())
    }

    fn create_or_update_file(&self, sh: &Shell) -> Result<()> {
        let repo = &self.repo;
        let path = &self.path;
        let encoded = &self.encoded;
        let branch = &self.branch;
        let message = &self.message;

        // Note: GitHub Apps can sign commits if no custom author/committer info is provided
        cmd!(
            sh,
            "gh api repos/{repo}/contents/{path} -X PUT -f content={encoded} -f branch={branch} -f message={message}"
        )
        .run()
        .context("Failed to create/update file")
    }

    fn create_pr(&self, sh: &Shell) -> Result<()> {
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
                "gh pr create --repo {repo} --base {BASE_BRANCH} --head {branch} --title {message} --body ''"
            )
            .run()
            .context("Failed to create pull request")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::{ToSpan, civil::date};
    use proptest::prelude::*;
    use serial_test::serial;

    fn test_entry(year: i16, month: i8, day: i8) -> Entry {
        Entry {
            id: Date::new(year, month, day).unwrap(),
            path: format!("{ENTRIES_DIR}{year:04}-{month:02}.md"),
            content: String::new(),
        }
    }

    fn setup_github_env(repo: &str, sha: &str) {
        unsafe {
            std::env::set_var("GITHUB_REPOSITORY", repo);
            std::env::set_var("GITHUB_SHA", sha);
        }
    }

    fn cleanup_github_env() {
        unsafe {
            std::env::remove_var("GITHUB_REPOSITORY");
            std::env::remove_var("GITHUB_SHA");
        }
    }

    fn proptest_config() -> ProptestConfig {
        ProptestConfig::with_cases(1000)
    }

    mod strategies {
        use super::*;

        pub(super) fn jiff_date() -> impl Strategy<Value = Date> {
            let fixed = date(2020, 1, 1);
            (0i64..=4000).prop_map(move |offset| fixed.checked_add(offset.days()).unwrap())
        }

        pub(super) fn first_of_month() -> impl Strategy<Value = Date> {
            jiff_date().prop_map(|date| date.first_of_month())
        }

        pub(super) fn github_repo() -> impl Strategy<Value = String> {
            (
                "[a-z0-9-]{3,20}/[a-z0-9-]{3,30}",
                "[A-Za-z0-9_.-]{3,20}/[A-Za-z0-9_.-]{3,30}",
            )
                .prop_map(|(_, repo)| repo)
        }

        pub(super) fn commit_sha() -> impl Strategy<Value = String> {
            "[0-9a-f]{40}".prop_map(|s| s)
        }

        pub(super) fn entry() -> impl Strategy<Value = Entry> {
            first_of_month().prop_map(|date| {
                let mut entry = Entry {
                    id: date,
                    path: format!("{ENTRIES_DIR}{:04}-{:02}.md", date.year(), date.month()),
                    content: String::new(),
                };
                entry.generate().unwrap();
                entry
            })
        }
    }

    proptest! {
        #![proptest_config(proptest_config())]

        /// Property: Entry path format and security
        #[test]
        fn prop_entry_path_format(date in strategies::jiff_date()) {
            let entry = test_entry(date.year(), date.month(), date.day());
            let expected_path = format!("{ENTRIES_DIR}{:04}-{:02}.md", date.year(), date.month());

            prop_assert!(entry.path.starts_with(ENTRIES_DIR));
            prop_assert!(!entry.path.contains(".."));
            prop_assert!(!entry.path.contains("//"));
            prop_assert!(!entry.path.starts_with('/'));
            prop_assert_eq!(&entry.path, &expected_path);
        }

        /// Property: Metadata block format is valid
        ///
        /// For any valid date, the metadata block should:
        /// - Be wrapped in +++ delimiters
        /// - Contain id, created, and modified fields
        /// - Have correct id format (YYYY-MM)
        #[test]
        fn prop_metadata_block_format(date in strategies::jiff_date()) {
            let entry = test_entry(date.year(), date.month(), date.day());
            let metadata = entry.metadata_block();

            prop_assert!(metadata.starts_with("+++\n"));
            prop_assert!(metadata.ends_with("+++\n"));
            prop_assert!(metadata.contains("id = "));
            prop_assert!(metadata.contains("created = "));
            prop_assert!(metadata.contains("modified = "));

            let expected_id = format!("id = \"{:04}-{:02}\"", date.year(), date.month());
            prop_assert!(metadata.contains(&expected_id));
        }

        /// Property: Generated content has correct day count and boundaries
        #[test]
        fn prop_generate_correct_day_count(first_of_month in strategies::first_of_month()) {
            let mut entry = test_entry(first_of_month.year(), first_of_month.month(), 1);
            entry.generate().unwrap();

            let expected_days = first_of_month.days_in_month();
            let pattern = format!("###### {:04}-{:02}-", first_of_month.year(), first_of_month.month());
            let actual_days = entry.content.matches(&pattern).count();

            prop_assert_eq!(actual_days, expected_days as usize);
            prop_assert!(entry.content.starts_with("+++\n"));

            // Verify first and last day headers exist
            let first_header = format!("###### {:04}-{:02}-01 ", first_of_month.year(), first_of_month.month());
            let last_header = format!("###### {:04}-{:02}-{:02} ", first_of_month.year(), first_of_month.month(), expected_days);
            prop_assert!(entry.content.contains(&first_header));
            prop_assert!(entry.content.contains(&last_header));
        }

        /// Property: GitHub struct fields have correct format and branch name
        #[test]
        #[serial]
        fn prop_github_fields_format(
            entry in strategies::entry(),
            repo in strategies::github_repo(),
            sha in strategies::commit_sha()
        ) {
            setup_github_env(&repo, &sha);

            let github = GitHub::new(entry.clone()).unwrap();
            let expected_suffix = format!("{:04}-{:02}", entry.id.year(), entry.id.month());

            // Verify all fields are populated correctly
            prop_assert_eq!(&github.repo, &repo);
            prop_assert_eq!(&github.sha, &sha);
            prop_assert!(github.path.starts_with(ENTRIES_DIR));
            prop_assert!(github.branch.starts_with(BRANCH_PREFIX));
            prop_assert!(github.branch.ends_with(&expected_suffix));
            prop_assert!(github.message.starts_with("cron: "));
            prop_assert!(!github.encoded.is_empty());
            prop_assert!(STANDARD.decode(&github.encoded).is_ok());

            cleanup_github_env();
        }

    }

    // ===== Edge case tests =====

    #[test]
    fn test_leap_year_february() {
        let mut entry = test_entry(2024, 2, 1);
        entry.generate().unwrap();

        assert_eq!(entry.content.matches("###### 2024-02-").count(), 29);
        assert!(entry.content.contains("###### 2024-02-29 "));
    }

    #[test]
    fn test_non_leap_year_february() {
        let mut entry = test_entry(2023, 2, 1);
        entry.generate().unwrap();

        assert_eq!(entry.content.matches("###### 2023-02-").count(), 28);
        assert!(entry.content.contains("###### 2023-02-28 "));
        assert!(!entry.content.contains("###### 2023-02-29 "));
    }

    #[test]
    fn test_jst_timezone_loads() {
        let tz = jst().unwrap();
        assert_eq!(tz.iana_name(), Some(TIMEZONE));
    }

    #[test]
    #[serial]
    fn test_github_new_requires_env_vars() {
        cleanup_github_env();

        let entry = Entry::new().unwrap();
        let result = GitHub::new(entry);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("GITHUB_REPOSITORY")
        );
    }
}
