use std::path::PathBuf;

use anyhow::Result;
use xshell::{Shell, cmd};

pub fn generate_parser() -> Result<()> {
    let mut sh = Shell::new()?;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ts_dir = manifest_dir.join("..").join("..").join("tree-sitter");
    sh.set_current_dir(&ts_dir);

    cmd!(sh, "tree-sitter generate ./grammar/grammar.js").run_echo()?;

    sh.remove_path("src/grammar.json")?;
    sh.remove_path("src/node-types.json")?;
    sh.remove_path("src/tree_sitter/alloc.h")?;
    sh.remove_path("src/tree_sitter/array.h")?;

    Ok(())
}
