use std::process;

use anyhow::Result;

mod commit;
mod entry;

use crate::{commit::create_commit, entry::generate_entry};

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
    let entry = generate_entry()?;
    create_commit(entry).await?;

    Ok(())
}
