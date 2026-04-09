use std::fmt::Write;

use anyhow::{Context, Result};
use jiff::{ToSpan, Zoned, civil::Date};

use crate::{Cmd, New, month::parse_month_arg};

fn generate_entry(month_str: &str) -> Result<String> {
    let id: Date = format!("{month_str}-01")
        .parse()
        .with_context(|| format!("Invalid month: {month_str}"))?;

    let today = Zoned::now().strftime("%Y-%m-%d");
    let mut content = String::with_capacity(1_000);

    write!(
        content,
        "+++\nid = \"{month_str}\"\ncreated = {today}\nmodified = {today}\n+++\n"
    )?;

    for date in id.series(1.days()).take(id.days_in_month() as usize) {
        write!(content, "\n###### {}\n\n\n", date.strftime("%Y-%m-%d %a"))?;
    }

    Ok(content)
}

impl Cmd for New {
    fn run(self) -> Result<()> {
        let repo_dir = std::env::var("FADING_DIR").context("FADING_DIR env is not set")?;
        let month_str = parse_month_arg(&self.month)?;
        let path = format!("{repo_dir}/entries/{month_str}.md");

        if std::path::Path::new(&path).exists() {
            println!("Entry already exists: {path}");
            return Ok(());
        }

        let content = generate_entry(&month_str)?;
        std::fs::write(&path, content).with_context(|| format!("Failed to write entry: {path}"))?;
        println!("Created entry: {path}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::{ToSpan, civil::date};
    use proptest::prelude::*;

    fn proptest_config() -> ProptestConfig {
        ProptestConfig::with_cases(1000)
    }

    mod strategies {
        use super::*;

        pub(super) fn month_str() -> impl Strategy<Value = String> {
            let fixed = date(2020, 1, 1);
            (0i64..=4000).prop_map(move |offset| {
                let d = fixed.checked_add(offset.days()).unwrap().first_of_month();
                format!("{:04}-{:02}", d.year(), d.month())
            })
        }
    }

    proptest! {
        #![proptest_config(proptest_config())]

        /// Property: Generated content has valid frontmatter and correct day count
        #[test]
        fn prop_generate_entry(month_str in strategies::month_str()) {
            let content = generate_entry(&month_str).unwrap();

            // Frontmatter
            let id_field = format!("id = \"{month_str}\"");
            prop_assert!(content.starts_with("+++\n"));
            prop_assert!(content.contains(&id_field));
            prop_assert!(content.contains("created = "));
            prop_assert!(content.contains("modified = "));

            // Day count
            let id: Date = format!("{month_str}-01").parse().unwrap();
            let expected_days = id.days_in_month() as usize;
            let prefix = format!("###### {month_str}-");
            prop_assert_eq!(content.matches(&prefix).count(), expected_days);

            // First and last day headers
            let first_header = format!("###### {month_str}-01 ");
            let last_header = format!("###### {month_str}-{:02} ", expected_days);
            prop_assert!(content.contains(&first_header));
            prop_assert!(content.contains(&last_header));
        }
    }

    #[test]
    fn test_leap_year_february() {
        let content = generate_entry("2024-02").unwrap();
        assert_eq!(content.matches("###### 2024-02-").count(), 29);
        assert!(content.contains("###### 2024-02-29 "));
    }

    #[test]
    fn test_non_leap_year_february() {
        let content = generate_entry("2023-02").unwrap();
        assert_eq!(content.matches("###### 2023-02-").count(), 28);
        assert!(content.contains("###### 2023-02-28 "));
        assert!(!content.contains("###### 2023-02-29 "));
    }
}
