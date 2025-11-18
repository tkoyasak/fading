use chrono::Local;
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Notify};

impl Cmd for Notify {
    fn run(self) -> anyhow::Result<()> {
        let sh = Shell::new()?;

        let title = "\"fading\"";
        let date = Local::now().format("%Y-%m-%d");
        let body = format!("\"Time to be fading! {date}\"");
        cmd!(
            sh,
            "osascript -e 'display notification '{body}' with title '{title}"
        )
        .run()?;

        Ok(())
    }
}
