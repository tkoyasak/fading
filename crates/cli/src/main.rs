mod crypto;
mod ls;
mod month;
mod new;
mod notify;
mod open;
mod pull;
mod push;
mod sigv4;
mod stats;

xflags::xflags! {
    /// CLI for the `fading` language
    cmd fading {

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

        /// Pull and restore the R2 bundle into the local repository
        cmd pull {}

        /// Push to Cloudflare (R2 backup and/or KV sync)
        cmd push {
            /// Target: r2, kv (default: both)
            optional target: String
            /// Sync all KV entries (ignore last synced commit)
            optional -f, --full
        }

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
        FadingCmd::Ls(ls) => ls.run(sh),
        FadingCmd::New(new) => new.run(sh),
        FadingCmd::Notify(notify) => notify.run(sh),
        FadingCmd::Open(open) => open.run(sh),
        FadingCmd::Pull(pull) => pull.run(sh),
        FadingCmd::Push(push) => push.run(sh),
        FadingCmd::Stats(stats) => stats.run(sh),
    }
}

fn main() {
    if let Err(err) = cmd_run() {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
