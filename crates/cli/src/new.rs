use std::fmt::Write;

use anyhow::{Context, Result, bail};
use jiff::{ToSpan, Zoned, civil::Date};

use crate::{Cmd, Ctx, New, When};

impl When {
    /// Resolve to the `YYYY-MM` month `new` creates. A date is rejected.
    fn new_month(&self) -> Result<String> {
        let date = match self {
            When::Now => crate::flags::today(),
            When::MonthsBack(n) => crate::flags::months_back(*n)?,
            When::Month(date) => *date,
            When::Day(_) => bail!("`new` takes a month (YYYY-MM), not a date"),
        };
        Ok(date.strftime("%Y-%m").to_string())
    }
}

impl Cmd for New {
    fn run(self, ctx: Ctx) -> Result<()> {
        let sh = ctx.sh;
        let id = self.when.unwrap_or(When::Now).new_month()?;
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

    #[test]
    fn test_frontmatter() {
        let content = generate_entry("2026-04").unwrap();
        assert!(content.starts_with("+++\n"));
        assert!(content.contains("id = \"2026-04\""));
        assert!(content.contains("created = "));
        assert!(content.contains("modified = "));
    }

    #[test]
    fn test_thirty_day_month() {
        let content = generate_entry("2026-04").unwrap();
        assert_eq!(content.matches("###### 2026-04-").count(), 30);
        assert!(content.contains("###### 2026-04-01 "));
        assert!(content.contains("###### 2026-04-30 "));
    }

    #[test]
    fn test_thirty_one_day_month() {
        let content = generate_entry("2026-01").unwrap();
        assert_eq!(content.matches("###### 2026-01-").count(), 31);
        assert!(content.contains("###### 2026-01-01 "));
        assert!(content.contains("###### 2026-01-31 "));
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

    #[test]
    fn new_month_for_month() {
        let when: When = "2026-04".parse().unwrap();
        assert_eq!(when.new_month().unwrap(), "2026-04");
    }

    #[test]
    fn new_month_rejects_date() {
        let when: When = "2026-04-15".parse().unwrap();
        assert!(when.new_month().is_err());
    }
}
