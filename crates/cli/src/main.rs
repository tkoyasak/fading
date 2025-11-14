use anyhow::Result;

mod flags;
mod ls;
mod notify;

use crate::flags::{Cmd, Fading, FadingCmd};

fn main() {
    if let Err(err) = try_main() {
        for cause in err.chain() {
            eprintln!("error: {cause}");
        }
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let flags = Fading::from_env_or_exit();
    match flags.subcommand {
        FadingCmd::Ls(ls) => ls.run(),
        FadingCmd::Notify(notify) => notify.run(),
    }
}
