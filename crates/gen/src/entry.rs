use std::{fmt::Write as FmtWrite, fs, io::Write as IOWrite, path::Path};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Duration, Local, NaiveDate};
use serde::{Serialize, Serializer};

#[derive(Debug, Serialize)]
struct Metadata {
    #[serde(serialize_with = "serialize_id")]
    id: NaiveDate,
    #[serde(default)]
    created: NaiveDate,
    #[serde(default)]
    modified: NaiveDate,
}

fn serialize_id<S>(id: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = id.format("%Y-%m").to_string();
    serializer.serialize_str(&s)
}

impl Metadata {
    fn new(id: NaiveDate) -> Self {
        let now = Local::now().date_naive();
        Self {
            id,
            created: now,
            modified: now,
        }
    }

    fn to_metadata_block(&self) -> String {
        let s = toml::to_string(self).unwrap();
        format!("+++\n{s}+++\n")
    }
}

pub fn generate_monthly_entry(s: Option<String>) -> Result<()> {
    let entry = if let Some(s) = s {
        NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d")
            .context("failed to parse: date format should be '%Y-%m'")?
    } else {
        Local::now().date_naive()
    };

    let entry_path = format!("entries/{}.md", entry.format("%Y-%m"));
    if Path::new(&entry_path).exists() {
        bail!("'{entry_path}' already exists");
    }

    let mut buf = String::with_capacity(1_000);

    let metadata = Metadata::new(entry).to_metadata_block();
    write!(&mut buf, "{metadata}")?;

    let n = entry.num_days_in_month();
    let mut cursor = entry.with_day(1).unwrap();
    for _ in 0..n {
        let date = cursor.format("%Y-%m-%d %a");
        write!(&mut buf, "\n###### {date}\n\n\n")?;
        cursor += Duration::days(1);
    }

    fs::create_dir_all("entries")?;
    fs::File::create(&entry_path)?.write_all(buf.as_bytes())?;

    println!("Done: generated '{entry_path}'");
    Ok(())
}
