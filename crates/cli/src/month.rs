use anyhow::{Context, Result, bail};
use jiff::{Span, Zoned, civil::Date};

/// Parse month argument and return YYYY-MM string.
pub(crate) fn parse_month(month: &Option<String>) -> Result<String> {
    match month {
        None => Ok(current_month()),
        Some(arg) if arg == "today" => Ok(current_month()),
        Some(arg) if arg.bytes().all(|b| b.is_ascii_digit()) => {
            // Offset: N months back
            let n: i64 = arg
                .parse()
                .with_context(|| format!("Invalid offset: {arg}"))?;
            let target = Zoned::now()
                .checked_add(Span::new().months(-n))
                .with_context(|| format!("Failed to subtract {n} months"))?;
            Ok(target.strftime("%Y-%m").to_string())
        }
        Some(arg) => {
            // Direct month specification (YYYY-MM)
            if format!("{arg}-01").parse::<Date>().is_err() {
                bail!("Invalid month format: {arg} (expected YYYY-MM, e.g., 2025-01)");
            }
            Ok(arg.to_string())
        }
    }
}

fn current_month() -> String {
    Zoned::now().strftime("%Y-%m").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_none_returns_current_month() {
        let result = parse_month(&None).unwrap();
        assert_eq!(result.len(), 7);
        assert!(result.chars().nth(4) == Some('-'));
    }

    #[test]
    fn parse_today_returns_current_month() {
        let result = parse_month(&Some("today".to_string())).unwrap();
        assert_eq!(result, parse_month(&None).unwrap());
    }

    #[test]
    fn parse_offset_zero_returns_current_month() {
        assert_eq!(
            parse_month(&Some("0".to_string())).unwrap(),
            parse_month(&None).unwrap()
        );
    }

    #[test]
    fn parse_offset_succeeds() {
        assert!(parse_month(&Some("1".to_string())).is_ok());
        assert!(parse_month(&Some("12".to_string())).is_ok());
        assert!(parse_month(&Some("100".to_string())).is_ok());
    }

    #[test]
    fn parse_valid_month() {
        assert_eq!(
            parse_month(&Some("2026-04".to_string())).unwrap(),
            "2026-04"
        );
        assert_eq!(
            parse_month(&Some("2024-02".to_string())).unwrap(),
            "2024-02"
        );
        assert_eq!(
            parse_month(&Some("2026-12".to_string())).unwrap(),
            "2026-12"
        );
        assert_eq!(
            parse_month(&Some("2026-01".to_string())).unwrap(),
            "2026-01"
        );
    }

    #[test]
    fn parse_invalid_month_number() {
        assert!(parse_month(&Some("2026-00".to_string())).is_err());
        assert!(parse_month(&Some("2026-13".to_string())).is_err());
    }

    #[test]
    fn parse_invalid_format() {
        assert!(parse_month(&Some("not-valid".to_string())).is_err());
        assert!(parse_month(&Some("2026/04".to_string())).is_err());
        assert!(parse_month(&Some("2026-4".to_string())).is_err());
    }
}
