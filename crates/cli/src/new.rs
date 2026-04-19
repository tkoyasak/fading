use std::fmt::Write;

use anyhow::{Context, Result};
use jiff::{ToSpan, Zoned, civil::Date};
use xshell::Shell;

use crate::{Cmd, New, month::parse_month};

impl Cmd for New {
    fn run(self, sh: Shell) -> Result<()> {
        let id = parse_month(&self.month)?;
        let path = format!("entries/{id}.md");

        if sh.path_exists(&path) {
            println!(
                "Entry already exists: {}",
                sh.current_dir().join(&path).display()
            );
            return Ok(());
        }

        let content = generate_entry(&id)?;
        sh.write_file(&path, &content)?;
        println!("Created entry: {}", sh.current_dir().join(&path).display());
        Ok(())
    }
}

fn generate_entry(id: &str) -> Result<String> {
    let start: Date = format!("{id}-01")
        .parse()
        .with_context(|| format!("Invalid month: {id}"))?;

    let today = Zoned::now().strftime("%Y-%m-%d");
    let mut content = String::with_capacity(1_000);

    write!(
        content,
        r#"+++
id = "{id}"
created = {today}
modified = {today}
+++
"#
    )?;

    for date in start.series(1.days()).take(start.days_in_month() as usize) {
        write!(
            content,
            r"
###### {}

<!-- -->
",
            date.strftime("%Y-%m-%d %a")
        )?;
    }

    Ok(content)
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
            let first_day: Date = format!("{month_str}-01").parse().unwrap();
            let expected_days = first_day.days_in_month() as usize;
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
