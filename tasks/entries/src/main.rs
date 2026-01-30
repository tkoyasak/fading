use anyhow::Result;

mod flags;
mod generate;

use crate::flags::{Cmd, Task, TaskCmd};

fn main() {
    if let Err(err) = try_main() {
        let prefix = if std::env::var("GITHUB_ACTIONS").is_ok_and(|s| s == "true") {
            "::error::"
        } else {
            ""
        };
        eprintln!("{prefix}{err:?}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let flags = Task::from_env()?;
    match flags.subcommand {
        TaskCmd::Generate(generate) => generate.run(),
    }
}
