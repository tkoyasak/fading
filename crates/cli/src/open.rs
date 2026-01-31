use anyhow::{Context, Result, bail};
use jiff::{Span, Zoned, civil::Date};
use xshell::{Shell, cmd};

use crate::flags::{Cmd, Open};

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

        // Get repository path from environment variable or use current directory
        let repo_dir = std::env::var("FADING_REPO").unwrap_or_else(|_| ".".to_string());

        // Parse month argument
        let month_str = parse_month_arg(&self.month)?;
        let path = format!("{repo_dir}/entries/{month_str}.md");

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

        /// current_month() always returns YYYY-MM format with valid month
        #[test]
        fn prop_current_month_format(_seed in 0u32..100) {
            let month = current_month();
            prop_assert_eq!(month.len(), 7);
            prop_assert!(month.contains('-'));

            // Verify it's a valid date
            let date_str = format!("{month}-01");
            prop_assert!(date_str.parse::<Date>().is_ok());
        }

        /// Valid YYYY-MM strings should parse as dates
        #[test]
        fn prop_valid_month_format(month_str in strategies::valid_month_str()) {
            let date_str = format!("{month_str}-01");
            prop_assert!(date_str.parse::<Date>().is_ok());
        }

        /// Invalid YYYY-MM strings (wrong month) should fail to parse
        #[test]
        fn prop_invalid_month_format(month_str in strategies::invalid_month_str()) {
            let date_str = format!("{month_str}-01");
            prop_assert!(date_str.parse::<Date>().is_err());
        }

        /// Offset strings (+N, -N) should parse as i64
        #[test]
        fn prop_offset_parsing(offset_str in strategies::offset_str()) {
            let parsed = offset_str.parse::<i64>();
            prop_assert!(parsed.is_ok());
        }

        /// Month arithmetic should work for reasonable offsets
        #[test]
        fn prop_month_arithmetic(offset in strategies::month_offset()) {
            let now = Zoned::now();
            let result = now.checked_add(Span::new().months(offset));

            // Should succeed for reasonable offsets
            // jiff may fail for very large offsets that overflow
            if offset.abs() < 12000 {  // ~1000 years
                prop_assert!(result.is_ok());
            }
        }

        /// Valid year range should be reasonable
        #[test]
        fn prop_year_range(year in strategies::year()) {
            prop_assert!(year >= 1970);
            prop_assert!(year <= 2100);
        }

        /// Valid month range is 1-12
        #[test]
        fn prop_valid_month_range(month in strategies::valid_month()) {
            prop_assert!(month >= 1);
            prop_assert!(month <= 12);
        }

        /// Invalid month range is 0 or 13+
        #[test]
        fn prop_invalid_month_range(month in strategies::invalid_month()) {
            prop_assert!(month == 0 || month >= 13);
        }
    }

    #[test]
    fn test_current_month_is_valid() {
        let month = current_month();
        let date_str = format!("{month}-01");
        assert!(date_str.parse::<Date>().is_ok());
    }

    #[test]
    fn test_specific_valid_months() {
        assert!(format!("2025-01-01").parse::<Date>().is_ok());
        assert!(format!("2025-12-01").parse::<Date>().is_ok());
        assert!(format!("1999-06-01").parse::<Date>().is_ok());
    }

    #[test]
    fn test_specific_invalid_months() {
        assert!(format!("2025-13-01").parse::<Date>().is_err());
        assert!(format!("2025-00-01").parse::<Date>().is_err());
    }

    #[test]
    fn test_specific_offset_parsing() {
        assert_eq!("+1".parse::<i64>(), Ok(1));
        assert_eq!("-12".parse::<i64>(), Ok(-12));
        assert!("++1".parse::<i64>().is_err());
        assert!("abc".parse::<i64>().is_err());
    }

    #[test]
    fn test_parse_month_arg_none() {
        let result = parse_month_arg(&None);
        assert!(result.is_ok());
        let month = result.unwrap();
        assert_eq!(month.len(), 7);
    }

    #[test]
    fn test_parse_month_arg_today() {
        let result = parse_month_arg(&Some("today".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_month_arg_valid_offset() {
        assert!(parse_month_arg(&Some("+1".to_string())).is_ok());
        assert!(parse_month_arg(&Some("-12".to_string())).is_ok());
        assert!(parse_month_arg(&Some("+0".to_string())).is_ok());
    }

    #[test]
    fn test_parse_month_arg_invalid_offset() {
        assert!(parse_month_arg(&Some("+abc".to_string())).is_err());
        assert!(parse_month_arg(&Some("-xyz".to_string())).is_err());
    }

    #[test]
    fn test_parse_month_arg_valid_direct() {
        assert!(parse_month_arg(&Some("2025-01".to_string())).is_ok());
        assert!(parse_month_arg(&Some("2025-12".to_string())).is_ok());
    }

    #[test]
    fn test_parse_month_arg_invalid_direct() {
        assert!(parse_month_arg(&Some("2025-13".to_string())).is_err());
        assert!(parse_month_arg(&Some("2025-00".to_string())).is_err());
        assert!(parse_month_arg(&Some("abcd-ef".to_string())).is_err());
    }
}
