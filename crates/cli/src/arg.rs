use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail};
use jiff::{ToSpan, Zoned, civil::Date};

/// Parsed form of the optional argument shared by `open`, `new`, and `stats`.
///
/// An explicit argument is parsed via [`FromStr`]; its absence is treated as
/// [`When::Now`] by each command.
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

impl When {
    /// Resolve to the date `open` opens to and jumps to.
    pub(crate) fn open_date(&self) -> Result<Date> {
        Ok(match self {
            When::Now => today(),
            When::MonthsBack(n) => months_back(*n)?.first_of_month(),
            When::Month(date) => *date,
            When::Day(date) => *date,
        })
    }

    /// Resolve to the `YYYY-MM` month `new` creates. A date is rejected.
    pub(crate) fn new_month(&self) -> Result<String> {
        let date = match self {
            When::Now => today(),
            When::MonthsBack(n) => months_back(*n)?,
            When::Month(date) => *date,
            When::Day(_) => bail!("`new` takes a month (YYYY-MM), not a date"),
        };
        Ok(date.strftime("%Y-%m").to_string())
    }

    /// Resolve to the end date of the one-year window `stats` renders.
    pub(crate) fn stats_to(&self) -> Result<Date> {
        let today = today();
        let to = match self {
            When::Now => today,
            When::MonthsBack(n) => months_back(*n)?.last_of_month(),
            When::Month(date) => date.last_of_month(),
            When::Day(date) => *date,
        };
        Ok(to.min(today))
    }
}

fn today() -> Date {
    Zoned::now().date()
}

fn months_back(n: i64) -> Result<Date> {
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

    #[test]
    fn open_date_for_month_is_first_day() {
        assert_eq!(
            parse("2026-04").unwrap().open_date().unwrap(),
            date(2026, 4, 1)
        );
    }

    #[test]
    fn open_date_for_day_is_exact() {
        assert_eq!(
            parse("2026-04-15").unwrap().open_date().unwrap(),
            date(2026, 4, 15)
        );
    }

    #[test]
    fn new_month_for_month() {
        assert_eq!(parse("2026-04").unwrap().new_month().unwrap(), "2026-04");
    }

    #[test]
    fn new_month_rejects_date() {
        assert!(parse("2026-04-15").unwrap().new_month().is_err());
    }

    #[test]
    fn stats_to_for_past_month_is_month_end() {
        // A clearly-past month is not clamped to today.
        assert_eq!(
            parse("2020-02").unwrap().stats_to().unwrap(),
            date(2020, 2, 29)
        );
    }

    #[test]
    fn stats_to_for_past_day_is_exact() {
        assert_eq!(
            parse("2020-02-10").unwrap().stats_to().unwrap(),
            date(2020, 2, 10)
        );
    }
}
