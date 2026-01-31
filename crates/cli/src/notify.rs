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
        /// format_notification_body should always produce quoted string with date
        #[test]
        fn prop_format_notification_body(date in strategies::date_string()) {
            let body = format_notification_body(&date);
            prop_assert!(body.starts_with("\"Time to be fading! "));
            prop_assert!(body.ends_with("\""));
            prop_assert!(body.contains(&date));
        }

        /// Any valid date string should work
        #[test]
        fn prop_format_with_valid_dates(date in strategies::date_string()) {
            let body = format_notification_body(&date);
            let expected = format!("\"Time to be fading! {}\"", date);
            prop_assert_eq!(body, expected);
        }
    }

    #[test]
    fn test_format_notification_body() {
        let body = format_notification_body("2025-01-31");
        assert_eq!(body, "\"Time to be fading! 2025-01-31\"");
    }

    #[test]
    fn test_format_notification_body_different_date() {
        let body = format_notification_body("1999-12-25");
        assert_eq!(body, "\"Time to be fading! 1999-12-25\"");
    }

    #[test]
    fn test_current_date_format() {
        let date = Zoned::now().strftime("%Y-%m-%d").to_string();
        // Should be YYYY-MM-DD format
        assert_eq!(date.len(), 10);
        assert_eq!(date.chars().nth(4), Some('-'));
        assert_eq!(date.chars().nth(7), Some('-'));

        // Should parse as a valid date
        assert!(date.parse::<Date>().is_ok());
    }

    #[test]
    fn test_notify_struct_exists() {
        let _notify = Notify;
        // Notify is a unit struct
    }

    #[test]
    fn test_shell_creation() {
        use xshell::Shell;
        let result = Shell::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_quoted_strings() {
        let title = "\"fading\"";
        assert!(title.starts_with('"'));
        assert!(title.ends_with('"'));
        assert_eq!(title.len(), 8); // "fading"
    }
}
