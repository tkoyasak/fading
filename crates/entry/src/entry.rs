use std::fmt::Write;

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate};

#[derive(Debug)]
pub struct Entry {
    pub id: NaiveDate,
    pub path: String,
    pub content: String,
}

impl Entry {
    fn new(arg: Option<String>) -> Result<Self> {
        let id = if let Some(arg) = arg {
            let s = format!("{arg}-01");
            NaiveDate::parse_from_str(&s, "%Y-%m-%d").with_context(|| {
                format!("failed to parse '{arg}', date format should be '%Y-%m'")
            })?
        } else {
            Local::now().date_naive().with_day(1).unwrap()
        };

        let path = format!("entries/{}.md", id.format("%Y-%m"));

        let content = String::with_capacity(1_000);

        Ok(Self { id, path, content })
    }

    fn metadata_block(&self) -> String {
        let id = self.id.format("%Y-%m").to_string();
        let today = Local::now().format("%Y-%m-%d").to_string();

        format!(
            r#"+++
id = "{id}"
created = "{today}"
modified = "{today}"
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

pub fn generate_entry(arg: Option<String>) -> Result<Entry> {
    let mut entry = Entry::new(arg)?;
    entry.generate()?;
    Ok(entry)
}
