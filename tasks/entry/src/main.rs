use anyhow::Result;

mod compile;
mod entry;
mod flags;
mod generate;
mod github;

use crate::flags::{Cmd, Task, TaskCmd};

fn main() {
    if let Err(err) = try_main() {
        let prefix = if let Ok(s) = std::env::var("GITHUB_ACTIONS")
            && s == "true"
        {
            "::error::"
        } else {
            ""
        };
        println!("{prefix}{err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let flags = Task::from_env_or_exit();
    match flags.subcommand {
        TaskCmd::Compile(compile) => compile.run(),
        TaskCmd::Generate(generate) => generate.run(),
    }
}
