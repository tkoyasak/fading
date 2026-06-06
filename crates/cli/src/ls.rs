use crate::{Cmd, Ctx, Ls};

impl Cmd for Ls {
    fn run(self, _ctx: Ctx) -> anyhow::Result<()> {
        fading_ls::run()?;
        Ok(())
    }
}
