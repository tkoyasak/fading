use std::{env, process};

use anyhow::Result;

mod commit;
mod entry;

use crate::{commit::update_entry, entry::generate_entry};

#[tokio::main]
async fn main() {
    match try_main().await {
        Ok(..) => {}
        Err(err) => {
            println!("::error::{err:#?}");
            process::exit(1);
        }
    };
}

async fn try_main() -> Result<()> {
    let arg = env::args().nth(1);
    let entry = generate_entry(arg)?;
    update_entry(entry).await?;

    Ok(())
}
