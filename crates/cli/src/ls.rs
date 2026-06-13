use anyhow::Result;

use crate::{Cmd, Ctx, Ls};

impl Cmd for Ls {
    fn run(self, _ctx: Ctx) -> Result<()> {
        fading_ls::run()?;
        Ok(())
    }
}
