use std::path::PathBuf;

use anyhow::Result;
use xshell::{Shell, cmd};

pub fn vendor_from_source() -> Result<()> {
    let mut sh = Shell::new()?;

    let temp_dir = sh.create_temp_dir()?;
    sh.set_current_dir(temp_dir.path());

    let repo = env!("TREE_SITTER_MARKDOWN_URL");
    let path = temp_dir.path().to_string_lossy().to_string();
    cmd!(sh, "git clone --filter=blob:none --depth=1 --branch=split_parser --no-checkout --no-tags --sparse {repo} {path}").run_echo()?;

    let targets = [
        "LICENSE",
        "common/common.js",
        "common/html_entities.json",
        "tree-sitter-markdown/grammar.js",
        "tree-sitter-markdown/src/scanner.c",
        "tree-sitter-markdown/test/corpus",
    ];
    cmd!(sh, "git sparse-checkout set --no-cone {targets...}").run_echo()?;

    let rev = env!("TREE_SITTER_MARKDOWN_REV");
    cmd!(sh, "git checkout {rev}").run_echo()?;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ts_dir = manifest_dir.join("..").join("..").join("tree-sitter");

    sh.copy_file_to_dir("LICENSE", &ts_dir)?;
    sh.copy_file_to_dir("common/common.js", ts_dir.join("grammar"))?;
    sh.copy_file_to_dir("common/html_entities.json", ts_dir.join("grammar"))?;
    sh.copy_file_to_dir("tree-sitter-markdown/grammar.js", ts_dir.join("grammar"))?;
    sh.copy_file_to_dir("tree-sitter-markdown/src/scanner.c", ts_dir.join("src"))?;

    let paths = sh.read_dir("tree-sitter-markdown/test/corpus")?;
    let test_dir = ts_dir.join("test").join("corpus");
    for path in paths {
        sh.copy_file_to_dir(path, &test_dir)?;
    }

    let mut sh = Shell::new()?;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ts_dir = manifest_dir.join("..").join("..").join("tree-sitter");
    sh.set_current_dir(&ts_dir);

    let grammar_path = "grammar/grammar.js";
    let grammar = sh.read_file(grammar_path)?;
    let old_lines = grammar.lines();
    let mut new_lines = Vec::with_capacity(old_lines.size_hint().0);

    for line in old_lines {
        if line == "const common = require('../common/common');" {
            new_lines.push("const common = require('./common');".to_string());
        } else if line.contains("'markdown'") {
            new_lines.push(line.replace("'markdown'", "'fading'"));
        } else {
            new_lines.push(line.to_string());
        }
    }

    sh.write_file(grammar_path, new_lines.join("\n") + "\n")?;

    let scanner_path = "src/scanner.c";
    let scanner = sh.read_file(scanner_path)?;
    let old_lines = scanner.lines();
    let mut new_lines = Vec::with_capacity(old_lines.size_hint().0);

    for line in old_lines {
        if line.contains("tree_sitter_markdown") {
            new_lines.push(line.replace("tree_sitter_markdown", "tree_sitter_fading"));
        } else {
            new_lines.push(line.to_string());
        }
    }

    sh.write_file(scanner_path, new_lines.join("\n") + "\n")?;

    Ok(())
}
