mod generate;

xflags::xflags! {
    /// CLI for the `fading` entries
    cmd task {

        /// Generate a monthly entry
        cmd generate {}
    }
}

pub trait Cmd {
    fn run(self) -> anyhow::Result<()>;
}

fn main() {
    let flags = Task::from_env_or_exit();

    let result = match flags.subcommand {
        TaskCmd::Generate(generate) => generate.run(),
    };

    if let Err(err) = result {
        let prefix = if std::env::var("GITHUB_ACTIONS").is_ok_and(|s| s == "true") {
            "::error::"
        } else {
            ""
        };
        eprintln!("{prefix}{err:?}");
        std::process::exit(1);
    }
}
