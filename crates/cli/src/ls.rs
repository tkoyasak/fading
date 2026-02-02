use tokio::runtime::Runtime;

use crate::flags::{Cmd, Ls};

impl Cmd for Ls {
    fn run(self) -> anyhow::Result<()> {
        Runtime::new()?.block_on(fading_ls::run());
        Ok(())
    }
}
