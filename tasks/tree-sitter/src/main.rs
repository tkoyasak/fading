use anyhow::Result;

use crate::flags::{TreeSitter, TreeSitterCmd};

mod compress;
mod flags;
mod generate;
mod vendor;

fn main() {
    if let Err(err) = try_main() {
        for cause in err.chain() {
            eprintln!("error: {cause}");
        }
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let flags = TreeSitter::from_env_or_exit();
    match flags.subcommand {
        TreeSitterCmd::Compress(compress) => compress.run(),
        TreeSitterCmd::Generate(generate) => generate.run(),
        TreeSitterCmd::Vendor(vendor) => vendor.run(),
    }
}
