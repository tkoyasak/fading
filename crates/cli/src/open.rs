use std::io::IsTerminal;

use anyhow::{Context, Result, bail};
use jiff::civil::Date;
use xshell::{Shell, cmd};

use crate::{Cmd, Ctx, Open, When, flags};

impl Cmd for Open {
    fn run(self, ctx: Ctx) -> Result<()> {
        let sh = ctx.sh;
        let date = self.when.unwrap_or_default().open_date()?;
        let id = date.strftime("%Y-%m");
        let rel_path = format!("entries/{id}.md");

        if !sh.path_exists(&rel_path) {
            bail!("File not found: {rel_path}");
        }

        let line = find_date_line(&sh, &rel_path, date);

        let home = sh.current_dir();
        let home_str = home
            .to_str()
            .context("FADING_HOME path is not valid UTF-8")?;
        let abs_path = format!("{home_str}/{rel_path}");

        if std::io::stdin().is_terminal() {
            // Invoked from an interactive shell — open Helix right here in the
            // current window, inheriting this terminal.
            open_here(&sh, &abs_path, line)
        } else {
            // No controlling terminal (e.g. launched from `notify`) — open a
            // new Ghostty window and launch Helix there.
            open_in_ghostty(&sh, home_str, &abs_path, line)
        }
    }
}

impl When {
    /// Resolve to the date `open` opens to and jumps to.
    fn open_date(&self) -> Result<Date> {
        Ok(match self {
            When::Now => flags::today(),
            When::MonthsBack(n) => flags::months_back(*n)?.first_of_month(),
            When::Month(date) => *date,
            When::Day(date) => *date,
        })
    }
}

/// Find the 1-indexed line number of the given date's heading in the file, if present.
fn find_date_line(sh: &Shell, path: &str, date: Date) -> Option<usize> {
    let prefix = format!("###### {}", date.strftime("%Y-%m-%d"));
    let content = sh.read_file(path).ok()?;
    content
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with(&prefix))
        .map(|(i, _)| i + 1)
}

/// Open the entry in Helix in the current terminal, inheriting this terminal.
fn open_here(sh: &Shell, abs_path: &str, line: Option<usize>) -> Result<()> {
    let target = match line {
        Some(n) => format!("{abs_path}:{n}"),
        None => abs_path.to_string(),
    };
    cmd!(sh, "hx {target}").run().context("Failed to launch Helix")
}

// Open a file in Helix via Ghostty's AppleScript API.
// Requires Ghostty 1.3.0+. See: https://github.com/ghostty-org/ghostty/pull/11208
fn open_in_ghostty(sh: &Shell, home_str: &str, abs_path: &str, line: Option<usize>) -> Result<()> {
    let hx_pattern = format!("hx {abs_path}");
    let hx_running = cmd!(sh, "pgrep -f {hx_pattern}").read().is_ok();

    let home_esc = escape_applescript(home_str);
    let abs_esc = escape_applescript(abs_path);
    let script = if hx_running {
        // Helix is already open with this file — just focus the existing Ghostty window.
        format!(
            r#"tell application "Ghostty"
    activate
    set targetDir to "{home_esc}"
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
            Some(n) => format!("{abs_esc}:{n}"),
            None => abs_esc.clone(),
        };
        format!(
            r#"tell application "Ghostty"
    activate
    set cfg to new surface configuration
    set initial working directory of cfg to "{home_esc}"
    set initial input of cfg to "hx {hx_target}\n"
    set win to new window with configuration cfg
end tell"#
        )
    };

    cmd!(sh, "osascript -e {script}")
        .run()
        .context("Failed to open Ghostty")
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use crate::When;
    use jiff::civil::date;

    #[test]
    fn open_date_for_month_is_first_day() {
        let when: When = "2026-04".parse().unwrap();
        assert_eq!(when.open_date().unwrap(), date(2026, 4, 1));
    }

    #[test]
    fn open_date_for_day_is_exact() {
        let when: When = "2026-04-15".parse().unwrap();
        assert_eq!(when.open_date().unwrap(), date(2026, 4, 15));
    }
}
