use std::process;

use anyhow::Result;

mod commit;
mod entry;

use crate::{commit::create_content, entry::generate_entry};

fn main() {
    match try_main() {
        Ok(..) => {}
        Err(err) => {
            for cause in err.chain() {
                println!("::error::{cause:#?}");
            }
            process::exit(1);
        }
    };
}

fn try_main() -> Result<()> {
    let entry = generate_entry()?;
    create_content(entry)?;

    Ok(())
}
