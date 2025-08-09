use std::{fmt::Write as FmtWrite, fs, io::Write as IOWrite, path::Path};

use anyhow::{Result, bail};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
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
    fn new(entry: &NaiveDate, now: &DateTime<Local>) -> Self {
        let date = now.date_naive();
        Self {
            id: *entry,
            created: date,
            modified: date,
        }
    }

    fn to_metadata_block(&self) -> String {
        let s = toml::to_string(self).unwrap();
        format!("+++\n{s}+++\n")
    }
}

pub fn generate_monthly_entry(s: Option<String>) -> Result<()> {
    let now = Local::now();
    let entry = if let Some(s) = s {
        NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d")?
    } else {
        now.date_naive()
    };

    let entry_path = format!("entries/{}.md", entry.format("%Y-%m"));
    if Path::new(&entry_path).exists() {
        bail!("'{entry_path}' already exists");
    }

    let mut buf = String::with_capacity(1000);

    let metadata = Metadata::new(&entry, &now).to_metadata_block();
    write!(&mut buf, "{metadata}")?;

    let n = now.num_days_in_month();
    let mut cursor = now.with_day(1).unwrap();
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
