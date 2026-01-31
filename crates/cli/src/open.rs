use anyhow::{Context, Result, bail};
use jiff::{Span, Zoned};
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Open};

impl Cmd for Open {
    fn run(self) -> Result<()> {
        let sh = Shell::new()?;

        // Get repository path from environment variable or use current directory
        let repo_dir = std::env::var("FADING_REPO").unwrap_or_else(|_| ".".to_string());

        // Parse month argument
        let month_str = match &self.month {
            None => {
                // Current month
                Zoned::now().strftime("%Y-%m").to_string()
            }
            Some(arg) if arg == "today" => {
                // Current month
                Zoned::now().strftime("%Y-%m").to_string()
            }
            Some(arg) if arg.starts_with('+') => {
                // Offset: +N months
                let offset: i64 = arg[1..]
                    .parse()
                    .with_context(|| format!("Invalid offset: {arg}"))?;
                let target = Zoned::now()
                    .checked_add(Span::new().months(offset))
                    .with_context(|| format!("Failed to add {offset} months"))?;
                target.strftime("%Y-%m").to_string()
            }
            Some(arg) if arg.starts_with('-') => {
                // Offset: -N months
                let offset: i64 = arg[1..]
                    .parse()
                    .with_context(|| format!("Invalid offset: {arg}"))?;
                let target = Zoned::now()
                    .checked_sub(Span::new().months(offset))
                    .with_context(|| format!("Failed to subtract {offset} months"))?;
                target.strftime("%Y-%m").to_string()
            }
            Some(arg) => {
                // Direct month specification (YYYY-MM)
                // Basic validation: should match YYYY-MM format
                if !arg.chars().all(|c: char| c.is_ascii_digit() || c == '-') || arg.len() != 7 {
                    bail!("Invalid month format: {arg} (expected YYYY-MM)");
                }
                arg.to_string()
            }
        };

        let path = format!("{repo_dir}/entries/{month_str}.md");

        // Open in Helix
        cmd!(sh, "hx {path}")
            .run()
            .with_context(|| format!("Failed to open {path} in Helix"))?;

        Ok(())
    }
}
