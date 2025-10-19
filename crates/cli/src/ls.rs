use anyhow::Result;
use tokio::runtime::Runtime;

use crate::flags::Ls;

impl Ls {
    pub fn run(self) -> Result<()> {
        let rt = Runtime::new()?;
        rt.block_on(fading_ls::start());
        Ok(())
    }
}
