use tokio::runtime::Runtime;

use crate::{Cmd, Ls};

impl Cmd for Ls {
    fn run(self, _sh: xshell::Shell) -> anyhow::Result<()> {
        Runtime::new()?.block_on(fading_ls::run());
        Ok(())
    }
}
