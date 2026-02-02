use jiff::Zoned;
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Notify};

fn format_notification_body(date: &str) -> String {
    format!("\"Time to be fading! {date}\"")
}

impl Cmd for Notify {
    fn run(self) -> anyhow::Result<()> {
        let sh = Shell::new()?;

        let title = "\"fading\"";
        let date = Zoned::now().strftime("%Y-%m-%d").to_string();
        let body = format_notification_body(&date);
        cmd!(
            sh,
            "osascript -e 'display notification '{body}' with title '{title}"
        )
        .run()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::Date;
    use proptest::prelude::*;

    mod strategies {
        use super::*;

        /// Generate valid date string in YYYY-MM-DD format
        pub(super) fn date_string() -> impl Strategy<Value = String> {
            (1970u16..=2100, 1u8..=12, 1u8..=28)
                .prop_map(|(y, m, d)| format!("{:04}-{:02}-{:02}", y, m, d))
        }
    }

    proptest! {
        #[test]
        fn prop_format_notification_body(date in strategies::date_string()) {
            let body = format_notification_body(&date);
            prop_assert!(body.starts_with("\"Time to be fading! "));
            prop_assert!(body.ends_with("\""));
            prop_assert!(body.contains(&date));
        }
    }

    #[test]
    fn test_current_date_format() {
        let date = Zoned::now().strftime("%Y-%m-%d").to_string();
        assert_eq!(date.len(), 10);
        assert_eq!(date.chars().nth(4), Some('-'));
        assert_eq!(date.chars().nth(7), Some('-'));
        assert!(date.parse::<Date>().is_ok());
    }

    #[test]
    fn test_shell_creation() {
        use xshell::Shell;
        let result = Shell::new();
        assert!(result.is_ok());
    }
}
