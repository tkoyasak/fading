mod crypto;
mod entry;
mod flags;
mod ls;
mod new;
mod notify;
mod open;
mod r2;
mod stats;

pub(crate) use flags::*;

pub struct Ctx {
    pub sh: xshell::Shell,
}

impl Ctx {
    fn new() -> anyhow::Result<Self> {
        let sh = xshell::Shell::new()?;
        let home = if cfg!(debug_assertions) {
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures").to_string()
        } else {
            sh.var("FADING_HOME")?
        };
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
}

pub trait Cmd {
    fn run(self, ctx: Ctx) -> anyhow::Result<()>;
}

fn cmd_run() -> anyhow::Result<()> {
    let ctx = Ctx::new()?;
    let flags = Fading::from_env()?;
    match flags.subcommand {
        FadingCmd::Open(open) => open.run(ctx),
        FadingCmd::New(new) => new.run(ctx),
        FadingCmd::Stats(stats) => stats.run(ctx),
        FadingCmd::Get(get) => get.run(ctx),
        FadingCmd::Put(put) => put.run(ctx),
        FadingCmd::Notify(notify) => notify.run(ctx),
        FadingCmd::Ls(ls) => ls.run(ctx),
    }
}

fn main() {
    if let Err(err) = cmd_run() {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}
