use anyhow::Result;

mod entry;
mod github;

use crate::{entry::generate_entry, github::create_pull_request};

fn main() {
    match try_main() {
        Ok(..) => {}
        Err(err) => {
            for cause in err.chain() {
                println!("::error::{cause:#?}");
            }
            std::process::exit(1);
        }
    };
}

fn try_main() -> Result<()> {
    let entry = generate_entry()?;
    create_pull_request(entry)?;

    Ok(())
}
