use anyhow::{Context, Result, bail};
use xshell::{Shell, cmd};

use crate::{Cmd, Open, month::parse_month_arg};

// Open a file in Helix via Ghostty's AppleScript API.
// Requires Ghostty 1.3.0+. See: https://github.com/ghostty-org/ghostty/pull/11208
fn open_in_ghostty(path: &str, working_dir: &str) -> Result<()> {
    let sh = Shell::new()?;

    // Check if hx is already editing this file
    let hx_running = cmd!(sh, "pgrep -f {path}").read().is_ok();

    let script = if hx_running {
        // Focus the existing terminal in the working directory
        format!(
            r#"tell application "Ghostty"
    activate
    set targetDir to "{working_dir}"
    set found to false
    repeat with win in windows
        repeat with tb in tabs of win
            repeat with term in terminals of tb
                if working directory of term is targetDir then
                    activate window win
                    focus term
                    set found to true
                    exit repeat
                end if
            end repeat
            if found then exit repeat
        end repeat
        if found then exit repeat
    end repeat
end tell"#
        )
    } else {
        // Open a new Ghostty window and run hx after shell initialization
        format!(
            r#"tell application "Ghostty"
    activate
    set cfg to new surface configuration
    set initial working directory of cfg to "{working_dir}"
    set initial input of cfg to "hx {path}\n"
    set win to new window with configuration cfg
end tell"#
        )
    };

    cmd!(sh, "osascript -e {script}")
        .run()
        .context("Failed to open Ghostty")
}

impl Cmd for Open {
    fn run(self) -> Result<()> {
        // Get repository path from environment variable
        let repo_dir = std::env::var("FADING_DIR").context("FADING_DIR env is not set")?;

        // Parse month argument
        let month_str = parse_month_arg(&self.month)?;
        let path = format!("{repo_dir}/entries/{month_str}.md");

        // Check if file exists
        if !std::path::Path::new(&path).exists() {
            bail!("File not found: {path}");
        }

        // Open in Helix via Ghostty
        open_in_ghostty(&path, &repo_dir)
    }
}
