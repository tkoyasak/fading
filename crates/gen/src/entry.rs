use std::{
    fmt::Write as FmtWrite,
    fs,
    io::Write as IOWrite,
    path::{Path, PathBuf},
};

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

#[derive(Debug)]
struct Entry {
    entry: NaiveDate,
    path: PathBuf,
    metadata: Metadata,
}

impl Entry {
    fn new(arg: Option<String>) -> Result<Self> {
        let entry = if let Some(arg) = arg {
            let s = format!("{arg}-01");
            NaiveDate::parse_from_str(&s, "%Y-%m-%d").with_context(|| {
                format!("Error: failed to parse '{arg}', date format should be '%Y-%m'")
            })?
        } else {
            Local::now().date_naive()
        };

        let path = {
            let s = format!("entries/{}.md", entry.format("%Y-%m"));
            let path = Path::new(&s);
            if path.exists() {
                bail!("Error: '{}' already exists", path.display());
            }
            path.to_path_buf()
        };

        let metadata = Metadata::new(entry);

        Ok(Self {
            entry,
            path,
            metadata,
        })
    }

    fn generate(&self) -> Result<()> {
        let mut buf = String::with_capacity(1_000);

        let metadata = self.metadata.to_metadata_block();
        write!(&mut buf, "{metadata}")?;

        let n = self.entry.num_days_in_month();
        let mut cur = self.entry.with_day(1).unwrap();

        for _ in 0..n {
            let date = cur.format("%Y-%m-%d %a");
            write!(&mut buf, "\n###### {date}\n\n\n")?;

            cur += Duration::days(1);
        }

        fs::create_dir_all("entries")?;
        fs::File::create(&self.path)?.write_all(buf.as_bytes())?;

        println!("Done: generated '{}'", self.path.display());
        Ok(())
    }
}

pub fn generate_monthly_entry(arg: Option<String>) -> Result<()> {
    Entry::new(arg)?.generate()
}
