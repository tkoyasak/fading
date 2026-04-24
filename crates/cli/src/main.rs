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

pub struct Ctx {
    pub sh: xshell::Shell,
}

impl Ctx {
    fn new() -> anyhow::Result<Self> {
        let sh = xshell::Shell::new()?;
        let home = sh.var("FADING_HOME")?;
        let sh = sh.with_current_dir(home);
        Ok(Self { sh })
    }

    pub fn cf_account_id(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_CF_ACCOUNT_ID")?)
    }

    pub fn r2_bucket(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_R2_BUCKET")?)
    }

    pub fn r2_access_key_id(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_R2_ACCESS_KEY_ID")?)
    }

    pub fn r2_secret_access_key(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_R2_SECRET_ACCESS_KEY")?)
    }

    pub fn encryption_key(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_ENCRYPTION_KEY")?)
    }

    pub fn cf_api_token(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_CF_API_TOKEN")?)
    }

    pub fn kv_namespace_id(&self) -> anyhow::Result<String> {
        Ok(self.sh.var("FADING_CLI_KV_NAMESPACE_ID")?)
    }
}

pub trait Cmd {
    fn run(self, ctx: Ctx) -> anyhow::Result<()>;
}

fn cmd_run() -> anyhow::Result<()> {
    let ctx = Ctx::new()?;
    let flags = Fading::from_env_or_exit();
    match flags.subcommand {
        FadingCmd::Get(get) => get.run(ctx),
        FadingCmd::Ls(ls) => ls.run(ctx),
        FadingCmd::New(new) => new.run(ctx),
        FadingCmd::Notify(notify) => notify.run(ctx),
        FadingCmd::Open(open) => open.run(ctx),
        FadingCmd::Push(push) => push.run(ctx),
        FadingCmd::Put(put) => put.run(ctx),
        FadingCmd::Stats(stats) => stats.run(ctx),
    }
}

fn main() {
    if let Err(err) = cmd_run() {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
