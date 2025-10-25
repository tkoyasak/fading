use anyhow::Result;

mod compress;
mod generate;
mod vendor;

use crate::{compress::compress_parser, generate::generate_parser, vendor::vendor_from_source};

fn main() {
    if let Err(err) = try_main() {
        for cause in err.chain() {
            eprintln!("error: {cause}");
        }
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    vendor_from_source()?;
    generate_parser()?;
    compress_parser()?;

    Ok(())
}
