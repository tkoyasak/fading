use tokio::runtime::Runtime;

use crate::{Cmd, Ctx, Ls};

impl Cmd for Ls {
    fn run(self, _ctx: Ctx) -> anyhow::Result<()> {
        Runtime::new()?.block_on(fading_ls::run());
        Ok(())
    }
}
