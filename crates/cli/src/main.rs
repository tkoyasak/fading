mod ls;
mod notify;
mod open;

xflags::xflags! {
    /// CLI for the `fading` language
    cmd fading {

        /// Start the language server
        cmd ls {}

        /// Notify of the time
        cmd notify {}

        /// Open an entry in Helix (default: current month)
        cmd open {
            /// Month to open: empty/today (current), +N/-N (offset), or YYYY-MM (direct)
            optional month: String
        }
    }
}

pub trait Cmd {
    fn run(self) -> anyhow::Result<()>;
}

fn main() {
    let flags = Fading::from_env_or_exit();

    let result = match flags.subcommand {
        FadingCmd::Ls(ls) => ls.run(),
        FadingCmd::Notify(notify) => notify.run(),
        FadingCmd::Open(open) => open.run(),
    };

    if let Err(err) = result {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
