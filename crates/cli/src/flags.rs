use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use jiff::{ToSpan, Zoned, civil::Date};

xflags::xflags! {
    /// CLI for the `fading` language
    cmd fading {

        /// Open an entry in Helix
        cmd open {
            /// today (default), N months back, YYYY-MM, or YYYY-MM-DD
            optional when: When
        }

        /// Create a new monthly entry
        cmd new {
            /// this month (default), N months back, or YYYY-MM
            optional when: When
        }

        /// Show a contribution calendar for the past year
        cmd stats {
            /// today (default), N months back, YYYY-MM, or YYYY-MM-DD
            optional when: When
        }

        /// Download the git bundle from R2
        cmd get {}

        /// Upload the git bundle to R2
        cmd put {}

        /// Send a time notification
        cmd notify {}

        /// Start the language server
        cmd ls {}
    }
}

/// Parsed form of the optional argument shared by `open`, `new`, and `stats`.
///
/// An explicit argument is parsed via [`FromStr`]; its absence is treated as
/// [`When::Now`] by each command. A bare number is always an offset in months,
/// never a year — e.g. `2026` means 2026 months back, not the year 2026.
#[derive(Debug)]
pub(crate) enum When {
    /// No argument — today / the current month.
    Now,
    /// `N` — N months back.
    MonthsBack(i64),
    /// `YYYY-MM` — held as the first day of that month.
    Month(Date),
    /// `YYYY-MM-DD`.
    Day(Date),
}

impl FromStr for When {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            let n = s.parse().with_context(|| format!("Invalid offset: {s}"))?;
            return Ok(When::MonthsBack(n));
        }
        match s.split('-').count() {
            2 => {
                let date = format!("{s}-01")
                    .parse()
                    .map_err(|_| anyhow!("Invalid month: {s} (expected YYYY-MM, e.g. 2026-01)"))?;
                Ok(When::Month(date))
            }
            3 => {
                let date = s.parse().map_err(|_| {
                    anyhow!("Invalid date: {s} (expected YYYY-MM-DD, e.g. 2026-01-15)")
                })?;
                Ok(When::Day(date))
            }
            _ => bail!("Invalid argument: {s} (expected N, YYYY-MM, or YYYY-MM-DD)"),
        }
    }
}

/// Today's date. Used by each command's `When` resolution.
pub(crate) fn today() -> Date {
    Zoned::now().date()
}

/// The date `n` months before today. Used by each command's `When` resolution.
pub(crate) fn months_back(n: i64) -> Result<Date> {
    today()
        .checked_add((-n).months())
        .with_context(|| format!("Failed to go back {n} months"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn parse(s: &str) -> Result<When> {
        s.parse()
    }

    #[test]
    fn digits_are_months_back() {
        assert!(matches!(parse("0").unwrap(), When::MonthsBack(0)));
        assert!(matches!(parse("1").unwrap(), When::MonthsBack(1)));
        assert!(matches!(parse("100").unwrap(), When::MonthsBack(100)));
    }

    #[test]
    fn yyyy_mm_is_month() {
        match parse("2026-04").unwrap() {
            When::Month(d) => assert_eq!(d, date(2026, 4, 1)),
            _ => panic!("expected Month"),
        }
    }

    #[test]
    fn yyyy_mm_dd_is_day() {
        match parse("2026-04-15").unwrap() {
            When::Day(d) => assert_eq!(d, date(2026, 4, 15)),
            _ => panic!("expected Day"),
        }
    }

    #[test]
    fn today_word_is_rejected() {
        assert!(parse("today").is_err());
    }

    #[test]
    fn invalid_month_rejected() {
        assert!(parse("2026-00").is_err());
        assert!(parse("2026-13").is_err());
        assert!(parse("2026-4").is_err()); // not zero-padded
    }

    #[test]
    fn invalid_day_rejected() {
        assert!(parse("2026-04-31").is_err()); // April has 30 days
        assert!(parse("2026-02-30").is_err());
    }

    #[test]
    fn invalid_format_rejected() {
        assert!(parse("not-valid").is_err());
        assert!(parse("2026/04").is_err());
    }
}
