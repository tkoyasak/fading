use anyhow::{Context, Result, bail};
use jiff::{Span, Zoned, civil::Date};
use xshell::{Shell, cmd};

use crate::{Cmd, Open};

fn current_month() -> String {
    Zoned::now().strftime("%Y-%m").to_string()
}

/// Parse month argument and return YYYY-MM string
fn parse_month_arg(month: &Option<String>) -> Result<String> {
    match month {
        None => Ok(current_month()),
        Some(arg) if arg == "today" => Ok(current_month()),
        Some(arg) if arg.starts_with('+') || arg.starts_with('-') => {
            // Offset: +N or -N months
            let offset: i64 = arg
                .parse()
                .with_context(|| format!("Invalid offset: {arg}"))?;
            let target = Zoned::now()
                .checked_add(Span::new().months(offset))
                .with_context(|| format!("Failed to add {offset} months"))?;
            Ok(target.strftime("%Y-%m").to_string())
        }
        Some(arg) => {
            // Direct month specification (YYYY-MM)
            // Validate by parsing as a date
            if format!("{arg}-01").parse::<Date>().is_err() {
                bail!("Invalid month format: {arg} (expected YYYY-MM, e.g., 2025-01)");
            }
            Ok(arg.to_string())
        }
    }
}

impl Cmd for Open {
    fn run(self) -> Result<()> {
        let sh = Shell::new()?;

        // Check if helix is installed
        cmd!(sh, "which hx")
            .read()
            .context("Helix (hx) is not installed or not in PATH")?;

        // Get repository path from environment variable
        let repo_dir = std::env::var("FADING_DIR").context("FADING_DIR env is not set")?;

        // Parse month argument
        let month_str = parse_month_arg(&self.month)?;
        let path = format!("{repo_dir}/entries/{month_str}.md");

        // Check if file exists
        if !std::path::Path::new(&path).exists() {
            bail!("File not found: {path}");
        }

        // Open in Helix
        cmd!(sh, "hx {path}")
            .run()
            .with_context(|| format!("Failed to open {path} in Helix"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    mod strategies {
        use super::*;

        /// Generate valid year (1970-2100)
        pub(super) fn year() -> impl Strategy<Value = u16> {
            1970u16..=2100
        }

        /// Generate valid month (1-12)
        pub(super) fn valid_month() -> impl Strategy<Value = u8> {
            1u8..=12
        }

        /// Generate invalid month (0, 13-255)
        pub(super) fn invalid_month() -> impl Strategy<Value = u8> {
            prop_oneof![Just(0u8), 13u8..=255,]
        }

        /// Generate valid YYYY-MM string
        pub(super) fn valid_month_str() -> impl Strategy<Value = String> {
            (year(), valid_month()).prop_map(|(y, m)| format!("{:04}-{:02}", y, m))
        }

        /// Generate invalid YYYY-MM string
        pub(super) fn invalid_month_str() -> impl Strategy<Value = String> {
            (year(), invalid_month()).prop_map(|(y, m)| format!("{:04}-{:02}", y, m))
        }

        /// Generate month offset (-1000 to +1000)
        pub(super) fn month_offset() -> impl Strategy<Value = i64> {
            -1000i64..=1000
        }

        /// Generate offset string (+N or -N)
        pub(super) fn offset_str() -> impl Strategy<Value = String> {
            month_offset().prop_map(|offset| {
                if offset >= 0 {
                    format!("+{}", offset)
                } else {
                    format!("{}", offset)
                }
            })
        }
    }

    proptest! {
        /// parse_month_arg(None) returns current month
        #[test]
        fn prop_parse_month_none(_seed in 0u32..100) {
            let result = parse_month_arg(&None);
            prop_assert!(result.is_ok());
            let month = result.unwrap();
            prop_assert_eq!(month.len(), 7);
            prop_assert!(month.contains('-'));
        }

        /// parse_month_arg(Some("today")) returns current month
        #[test]
        fn prop_parse_month_today(_seed in 0u32..100) {
            let result = parse_month_arg(&Some("today".to_string()));
            prop_assert!(result.is_ok());
            let month = result.unwrap();
            prop_assert_eq!(month.len(), 7);
        }

        /// parse_month_arg with valid offset succeeds
        #[test]
        fn prop_parse_month_offset(offset_str in strategies::offset_str()) {
            let result = parse_month_arg(&Some(offset_str.clone()));
            // Should succeed for reasonable offsets
            if let Ok(offset) = offset_str.parse::<i64>() {
                if offset.abs() < 12000 {
                    prop_assert!(result.is_ok());
                }
            }
        }

        /// parse_month_arg with valid YYYY-MM succeeds
        #[test]
        fn prop_parse_month_valid(month_str in strategies::valid_month_str()) {
            let result = parse_month_arg(&Some(month_str));
            prop_assert!(result.is_ok());
        }

        /// parse_month_arg with invalid YYYY-MM fails
        #[test]
        fn prop_parse_month_invalid(month_str in strategies::invalid_month_str()) {
            let result = parse_month_arg(&Some(month_str));
            prop_assert!(result.is_err());
        }
    }
}
