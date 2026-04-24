mod crypto;
mod entry;
mod kv;
mod ls;
mod month;
mod new;
mod notify;
mod open;
mod r2;
mod stats;

xflags::xflags! {
    /// CLI for the `fading` language
    cmd fading {

        /// Download the git bundle from R2 and fetch into the local repository
        cmd get {}

        /// Start the language server
        cmd ls {}

        /// Create a new monthly entry (default: current month)
        cmd new {
            /// Month: today (current), N (N months back), or YYYY-MM (direct)
            optional month: String
        }

        /// Notify of the time
        cmd notify {}

        /// Open an entry in Helix (default: current month)
        cmd open {
            /// Month to open: today (current), N (N months back), or YYYY-MM (direct)
            optional month: String
        }

        /// Sync entries to KV
        cmd push {
            /// Sync all entries (ignore last synced commit)
            optional -f, --full
        }

        /// Upload the git bundle to R2
        cmd put {}

        /// Show a contribution calendar for the past year up to the given month
        cmd stats {
            /// Month: today (current), N (N months back), or YYYY-MM (direct)
            optional month: String
        }
    }
}

pub trait Cmd {
    fn run(self, sh: xshell::Shell) -> anyhow::Result<()>;
}

fn cmd_run() -> anyhow::Result<()> {
    let sh = {
        let sh = xshell::Shell::new()?;
        let home = sh.var("FADING_HOME")?;
        sh.with_current_dir(home)
    };

    let flags = Fading::from_env_or_exit();
    match flags.subcommand {
        FadingCmd::Get(get) => get.run(sh),
        FadingCmd::Ls(ls) => ls.run(sh),
        FadingCmd::New(new) => new.run(sh),
        FadingCmd::Notify(notify) => notify.run(sh),
        FadingCmd::Open(open) => open.run(sh),
        FadingCmd::Push(push) => push.run(sh),
        FadingCmd::Put(put) => put.run(sh),
        FadingCmd::Stats(stats) => stats.run(sh),
    }
}

fn main() {
    if let Err(err) = cmd_run() {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
