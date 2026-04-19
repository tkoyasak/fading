use anyhow::{Context, Result, bail};
use jiff::Zoned;
use xshell::{Shell, cmd};

use crate::{Cmd, Open, month::parse_month};

impl Cmd for Open {
    fn run(self, sh: Shell) -> Result<()> {
        let id = parse_month(&self.month)?;
        let path = format!("entries/{id}.md");

        if !sh.path_exists(&path) {
            bail!("File not found: {path}");
        }

        let jump_to_today = matches!(self.month.as_deref(), None | Some("today"));
        let line = jump_to_today.then(|| find_today_line(&sh, &path)).flatten();
        open_in_ghostty(&sh, &path, line)
    }
}

/// Find the 1-indexed line number of today's heading in the file, if present.
fn find_today_line(sh: &Shell, path: &str) -> Option<usize> {
    let today = Zoned::now().strftime("%Y-%m-%d").to_string();
    let prefix = format!("###### {today}");
    let content = sh.read_file(path).ok()?;
    content
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with(&prefix))
        .map(|(i, _)| i + 1)
}

// Open a file in Helix via Ghostty's AppleScript API.
// Requires Ghostty 1.3.0+. See: https://github.com/ghostty-org/ghostty/pull/11208
fn open_in_ghostty(sh: &Shell, rel_path: &str, line: Option<usize>) -> Result<()> {
    let home = sh.current_dir();
    let home_str = home
        .to_str()
        .context("FADING_HOME path is not valid UTF-8")?;
    let abs_path = format!("{home_str}/{rel_path}");

    let hx_running = cmd!(sh, "pgrep -f {abs_path}").read().is_ok();

    let script = if hx_running {
        // Helix is already open with this file — just focus the existing Ghostty window.
        format!(
            r#"tell application "Ghostty"
    activate
    set targetDir to "{home_str}"
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
        // Helix is not running — open a new Ghostty window and launch Helix.
        let hx_target = match line {
            Some(n) => format!("{abs_path}:{n}"),
            None => abs_path.clone(),
        };
        format!(
            r#"tell application "Ghostty"
    activate
    set cfg to new surface configuration
    set initial working directory of cfg to "{home_str}"
    set initial input of cfg to "hx {hx_target}\n"
    set win to new window with configuration cfg
end tell"#
        )
    };

    cmd!(sh, "osascript -e {script}")
        .run()
        .context("Failed to open Ghostty")
}
