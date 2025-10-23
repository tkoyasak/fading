use std::fmt::Write;

use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};

#[derive(Debug)]
pub struct Entry {
    pub id: NaiveDate,
    pub path: String,
    pub content: String,
}

impl Entry {
    fn new() -> Self {
        let id = Local::now().date_naive().with_day(1).unwrap();
        let path = format!("entries/{}.md", id.format("%Y-%m"));
        let content = String::with_capacity(1_000);

        Self { id, path, content }
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

pub fn generate_entry() -> Result<Entry> {
    let mut entry = Entry::new();
    entry.generate()?;
    Ok(entry)
}
