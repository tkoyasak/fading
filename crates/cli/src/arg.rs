use anyhow::{Context, Result, anyhow, bail};
use jiff::{ToSpan, Zoned, civil::Date};

/// Parsed form of the optional argument shared by `open`, `new`, and `stats`.
///
/// Classification is the same for every command; each command resolves the
/// classified value to a concrete date or month on its own.
pub(crate) enum Arg {
    /// No argument — today / the current month.
    Now,
    /// `N` — N months back.
    MonthsBack(i64),
    /// `YYYY-MM` — held as the first day of that month.
    Month(Date),
    /// `YYYY-MM-DD`.
    Day(Date),
}

impl Arg {
    /// Classify the raw argument without resolving it to a date or month.
    pub(crate) fn classify(arg: Option<&str>) -> Result<Self> {
        match arg {
            None => Ok(Arg::Now),
            Some(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
                let n = s.parse().with_context(|| format!("Invalid offset: {s}"))?;
                Ok(Arg::MonthsBack(n))
            }
            Some(s) => match s.split('-').count() {
                2 => {
                    let date = format!("{s}-01").parse().map_err(|_| {
                        anyhow!("Invalid month: {s} (expected YYYY-MM, e.g. 2026-01)")
                    })?;
                    Ok(Arg::Month(date))
                }
                3 => {
                    let date = s.parse().map_err(|_| {
                        anyhow!("Invalid date: {s} (expected YYYY-MM-DD, e.g. 2026-01-15)")
                    })?;
                    Ok(Arg::Day(date))
                }
                _ => bail!("Invalid argument: {s} (expected N, YYYY-MM, or YYYY-MM-DD)"),
            },
        }
    }

    /// Resolve to the date `open` opens to and jumps to.
    pub(crate) fn open_date(&self) -> Result<Date> {
        Ok(match self {
            Arg::Now => today(),
            Arg::MonthsBack(n) => months_back(*n)?.first_of_month(),
            Arg::Month(date) => *date,
            Arg::Day(date) => *date,
        })
    }

    /// Resolve to the `YYYY-MM` month `new` creates. A date is rejected.
    pub(crate) fn new_month(&self) -> Result<String> {
        let date = match self {
            Arg::Now => today(),
            Arg::MonthsBack(n) => months_back(*n)?,
            Arg::Month(date) => *date,
            Arg::Day(_) => bail!("`new` takes a month (YYYY-MM), not a date"),
        };
        Ok(date.strftime("%Y-%m").to_string())
    }

    /// Resolve to the end date of the one-year window `stats` renders.
    pub(crate) fn stats_to(&self) -> Result<Date> {
        let today = today();
        let to = match self {
            Arg::Now => today,
            Arg::MonthsBack(n) => months_back(*n)?.last_of_month(),
            Arg::Month(date) => date.last_of_month(),
            Arg::Day(date) => *date,
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

    fn classify(arg: &str) -> Result<Arg> {
        Arg::classify(Some(arg))
    }

    #[test]
    fn none_is_now() {
        assert!(matches!(Arg::classify(None).unwrap(), Arg::Now));
    }

    #[test]
    fn digits_are_months_back() {
        assert!(matches!(classify("0").unwrap(), Arg::MonthsBack(0)));
        assert!(matches!(classify("1").unwrap(), Arg::MonthsBack(1)));
        assert!(matches!(classify("100").unwrap(), Arg::MonthsBack(100)));
    }

    #[test]
    fn yyyy_mm_is_month() {
        match classify("2026-04").unwrap() {
            Arg::Month(d) => assert_eq!(d, date(2026, 4, 1)),
            _ => panic!("expected Month"),
        }
    }

    #[test]
    fn yyyy_mm_dd_is_day() {
        match classify("2026-04-15").unwrap() {
            Arg::Day(d) => assert_eq!(d, date(2026, 4, 15)),
            _ => panic!("expected Day"),
        }
    }

    #[test]
    fn today_word_is_rejected() {
        assert!(classify("today").is_err());
    }

    #[test]
    fn invalid_month_rejected() {
        assert!(classify("2026-00").is_err());
        assert!(classify("2026-13").is_err());
        assert!(classify("2026-4").is_err()); // not zero-padded
    }

    #[test]
    fn invalid_day_rejected() {
        assert!(classify("2026-04-31").is_err()); // April has 30 days
        assert!(classify("2026-02-30").is_err());
    }

    #[test]
    fn invalid_format_rejected() {
        assert!(classify("not-valid").is_err());
        assert!(classify("2026/04").is_err());
    }

    #[test]
    fn open_date_for_month_is_first_day() {
        assert_eq!(
            classify("2026-04").unwrap().open_date().unwrap(),
            date(2026, 4, 1)
        );
    }

    #[test]
    fn open_date_for_day_is_exact() {
        assert_eq!(
            classify("2026-04-15").unwrap().open_date().unwrap(),
            date(2026, 4, 15)
        );
    }

    #[test]
    fn new_month_for_month() {
        assert_eq!(classify("2026-04").unwrap().new_month().unwrap(), "2026-04");
    }

    #[test]
    fn new_month_rejects_date() {
        assert!(classify("2026-04-15").unwrap().new_month().is_err());
    }

    #[test]
    fn stats_to_for_past_month_is_month_end() {
        // A clearly-past month is not clamped to today.
        assert_eq!(
            classify("2020-02").unwrap().stats_to().unwrap(),
            date(2020, 2, 29)
        );
    }

    #[test]
    fn stats_to_for_past_day_is_exact() {
        assert_eq!(
            classify("2020-02-10").unwrap().stats_to().unwrap(),
            date(2020, 2, 10)
        );
    }
}
