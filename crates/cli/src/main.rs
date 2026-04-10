mod ls;
mod month;
mod new;
mod notify;
mod open;
mod push;
mod sigv4;

xflags::xflags! {
    /// CLI for the `fading` language
    cmd fading {

        /// Start the language server
        cmd ls {}

        /// Create a new monthly entry (default: current month)
        cmd new {
            /// Month: empty/today (current), +N/-N (offset), or YYYY-MM (direct)
            optional month: String
        }

        /// Notify of the time
        cmd notify {}

        /// Open an entry in Helix (default: current month)
        cmd open {
            /// Month to open: empty/today (current), +N/-N (offset), or YYYY-MM (direct)
            optional month: String
        }

        /// Push to Cloudflare (R2 backup and/or KV sync)
        cmd push {
            /// Target: r2, kv (default: both)
            optional target: String
            /// Sync all KV entries (ignore last synced commit)
            optional -f, --full
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
        FadingCmd::New(new) => new.run(),
        FadingCmd::Notify(notify) => notify.run(),
        FadingCmd::Open(open) => open.run(),
        FadingCmd::Push(push) => push.run(),
    };

    if let Err(err) = result {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
