use tokio::runtime::Runtime;

use crate::flags::{Cmd, Ls};

impl Cmd for Ls {
    fn run(self) -> anyhow::Result<()> {
        let rt = Runtime::new()?;
        rt.block_on(fading_ls::start());

        Ok(())
    }
}
