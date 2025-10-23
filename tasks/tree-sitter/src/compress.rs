use std::path::PathBuf;

use anyhow::Result;
use xshell::{Shell, cmd};

use crate::flags::Compress;

impl Compress {
    pub fn run(self) -> Result<()> {
        let mut sh = Shell::new()?;

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ts_dir = manifest_dir.join("..").join("..").join("tree-sitter");
        sh.set_current_dir(ts_dir);

        let parser = "src/parser.c";
        cmd!(sh, "zstd -v --ultra -22 --rm {parser}").run_echo()?;

        Ok(())
    }
}
