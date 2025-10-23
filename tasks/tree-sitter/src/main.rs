use anyhow::Result;

use crate::flags::{Xtask, XtaskCmd};

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
    let flags = Xtask::from_env_or_exit();
    match flags.subcommand {
        XtaskCmd::Compress(compress) => compress.run(),
        XtaskCmd::Generate(generate) => generate.run(),
        XtaskCmd::Vendor(vendor) => vendor.run(),
    }
}
