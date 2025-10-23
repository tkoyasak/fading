use std::path::PathBuf;

use anyhow::Result;
use xshell::{Shell, cmd};

use crate::flags::Generate;

impl Generate {
    pub fn run(self) -> Result<()> {
        let mut sh = Shell::new()?;

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ts_dir = manifest_dir.join("..").join("..").join("tree-sitter");
        sh.set_current_dir(ts_dir);

        cmd!(sh, "tree-sitter generate ./grammar/grammar.js").run_echo()?;

        Ok(())
    }
}
