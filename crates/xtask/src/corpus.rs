use std::path::PathBuf;

use anyhow::Result;
use xshell::{Shell, cmd};

use crate::{Cmd, Corpus};

impl Cmd for Corpus {
    /// Download each source work via `gh` into the local corpus cache.
    fn run(self) -> Result<()> {
        let sh = Shell::new()?;
        let dir = corpus_dir();
        let accept = "Accept: application/vnd.github.raw";
        for path in work_paths() {
            let endpoint = format!("repos/{REPO}/contents/{path}");
            let raw = cmd!(sh, "gh api {endpoint} -H {accept}").read()?;
            let name = path.rsplit('/').next().unwrap_or(path);
            sh.write_file(dir.join(name), &raw)?;
            println!("fetched {name}");
        }
        Ok(())
    }
}

/// Source repository hosting the works listed in `works.txt`.
const REPO: &str = "levelevel/AozoraTxt";

/// Work paths, one per line. Edit `works.txt` (not this file) to swap sources;
/// `cargo` rebuilds when it changes.
const WORKS: &str = include_str!("../works.txt");

/// Local cache of fetched source texts (git-ignored), shared with `fixtures`.
pub fn corpus_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/corpus"))
}

/// Work paths, skipping blank lines and `#` comments.
fn work_paths() -> impl Iterator<Item = &'static str> {
    WORKS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_paths_are_ruby_less() {
        let paths: Vec<_> = work_paths().collect();
        assert!(!paths.is_empty());
        assert!(paths.iter().all(|p| p.contains("_txt_utf8_")));
    }
}
