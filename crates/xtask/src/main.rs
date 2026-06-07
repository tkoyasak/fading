mod corpus;
mod fixtures;

xflags::xflags! {
    /// Developer tasks for the `fading` workspace
    cmd xtask {
        /// Download the source corpus from AozoraTxt into the local cache
        cmd corpus {}
        /// Generate the local fixtures under `fixtures/entries` from the cache
        cmd fixtures {}
    }
}

pub trait Cmd {
    fn run(self) -> anyhow::Result<()>;
}

fn cmd_run() -> anyhow::Result<()> {
    let flags = Xtask::from_env()?;
    match flags.subcommand {
        XtaskCmd::Corpus(corpus) => corpus.run(),
        XtaskCmd::Fixtures(fixtures) => fixtures.run(),
    }
}

fn main() {
    if let Err(err) = cmd_run() {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
