use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write as _};

use anyhow::{Context, Result, bail};
use jiff::{ToSpan, civil::Date};
use xshell::Shell;

use crate::{Cmd, Ctx, Stats, When, flags};

impl Cmd for Stats {
    fn run(self, ctx: Ctx) -> Result<()> {
        let sh = ctx.sh;
        let to = self.when.unwrap_or_default().stats_to()?;
        let from = to.checked_sub(364.days()).context("date arithmetic")?;

        let char_counts = find_char_counts(&sh, from, to)?;
        render_grid(&char_counts, from, to)
    }
}

impl When {
    /// Resolve to the end date of the one-year window `stats` renders.
    fn stats_to(&self) -> Result<Date> {
        let today = flags::today();
        let to = match self {
            When::Now => today,
            When::MonthsBack(n) => flags::months_back(*n)?.last_of_month(),
            When::Month(date) => date.last_of_month(),
            When::Day(date) => *date,
        };
        Ok(to.min(today))
    }
}

fn find_char_counts(sh: &Shell, from: Date, to: Date) -> Result<HashMap<Date, usize>> {
    let mut paths = Vec::new();
    let mut cursor = from.first_of_month();
    loop {
        let (y, m) = (cursor.year(), cursor.month());
        paths.push(format!("entries/{y:04}-{m:02}.md"));
        if y == to.year() && m == to.month() {
            break;
        }
        cursor = cursor
            .checked_add(1.months())
            .context("date arithmetic")?
            .first_of_month();
    }

    let results: Vec<Result<Vec<(Date, usize)>>> = std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .iter()
            .map(|path| {
                let sh = sh.clone();
                let path = path.clone();
                s.spawn(move || -> Result<Vec<(Date, usize)>> {
                    if !sh.path_exists(&path) {
                        return Ok(vec![]);
                    }
                    let content = sh.read_file(&path)?;
                    Ok(count_chars_by_day(&content, from, to))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| bail!("thread panicked")))
            .collect()
    });

    let mut map = HashMap::new();
    for result in results {
        map.extend(result?);
    }
    Ok(map)
}

fn count_chars_by_day(content: &str, from: Date, to: Date) -> Vec<(Date, usize)> {
    crate::entry::parse_entries(content)
        .into_iter()
        .filter(|(date, _)| *date >= from && *date <= to)
        .filter_map(|(date, text)| {
            let chars: usize = text
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && *l != "<!-- -->")
                .map(|l| l.chars().count())
                .sum();
            (chars > 0).then_some((date, chars))
        })
        .collect()
}

fn render_grid(char_counts: &HashMap<Date, usize>, from: Date, to: Date) -> Result<()> {
    let grid = Grid::new(char_counts, from, to)?;
    let mut out = io::stdout().lock();
    write!(out, "{grid}").context("failed to write to stdout")?;
    Ok(())
}

/// The contribution calendar for one window, renderable via [`fmt::Display`].
struct Grid<'a> {
    char_counts: &'a HashMap<Date, usize>,
    scale: Scale,
    weeks: Vec<Week>,
    from: Date,
    to: Date,
}

impl<'a> Grid<'a> {
    fn new(char_counts: &'a HashMap<Date, usize>, from: Date, to: Date) -> Result<Self> {
        Ok(Self {
            scale: Scale::from_counts(char_counts),
            weeks: build_weeks(from, to)?,
            char_counts,
            from,
            to,
        })
    }
}

impl fmt::Display for Grid<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Header
        writeln!(
            f,
            "  {} –> {}",
            self.from.strftime("%Y-%m-%d"),
            self.to.strftime("%Y-%m-%d")
        )?;
        writeln!(f)?;

        // Month labels row
        f.write_str("      ")?;
        for ch in month_label_row(&self.weeks) {
            write!(f, "{ch}")?;
        }
        writeln!(f)?;

        // Day rows: Mon(0)..Sun(6), all labeled
        let day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        for (row, label) in day_labels.iter().enumerate() {
            write!(f, "  {label} ")?;
            for week in &self.weeks {
                match week[row] {
                    Some(d) => {
                        let size = self.char_counts.get(&d).copied().unwrap_or(0);
                        f.write_str(self.scale.cell(size))?;
                    }
                    None => f.write_str(" ")?,
                }
            }
            writeln!(f)?;
        }

        // Legend + summary
        let total_days = self
            .from
            .series(1.days())
            .take_while(|d| *d <= self.to)
            .count();
        let written_days = self.char_counts.len();
        writeln!(f)?;
        writeln!(
            f,
            "      Less {} {} {} {} More",
            CELLS[1], CELLS[2], CELLS[3], CELLS[4]
        )?;
        writeln!(f)?;
        writeln!(f, "  {written_days} / {total_days} days written")?;
        Ok(())
    }
}

/// One grid column: Monday..Sunday, `None` for days outside the window.
type Week = [Option<Date>; 7];

/// Build the weekly columns spanning `from..=to`, starting on the Monday of the
/// week containing `from`. Two trailing empty weeks are appended so a month
/// label at the last column is never truncated.
fn build_weeks(from: Date, to: Date) -> Result<Vec<Week>> {
    let week_start_offset = from.weekday().to_monday_zero_offset() as i64;
    let mut col_start = from
        .checked_sub(week_start_offset.days())
        .context("grid start date arithmetic")?;

    let mut weeks = Vec::new();
    loop {
        let mut week: Week = [None; 7];
        for (i, slot) in week.iter_mut().enumerate() {
            let day = col_start
                .checked_add((i as i64).days())
                .context("day offset arithmetic")?;
            if day >= from && day <= to {
                *slot = Some(day);
            }
        }
        weeks.push(week);
        let week_end = col_start
            .checked_add(6.days())
            .context("week end arithmetic")?;
        if week_end >= to {
            break;
        }
        col_start = col_start
            .checked_add(7.days())
            .context("next week arithmetic")?;
    }
    weeks.push([None; 7]);
    weeks.push([None; 7]);
    Ok(weeks)
}

/// A character per week column, labeling each column that holds the 1st of a
/// month with its 3-letter abbreviation (blank-padded elsewhere).
fn month_label_row(weeks: &[Week]) -> Vec<char> {
    let mut row = vec![' '; weeks.len()];
    for (i, week) in weeks.iter().enumerate() {
        if let Some(d) = week.iter().find_map(|d| d.filter(|d| d.day() == 1)) {
            for (j, ch) in month_abbr(d.month()).chars().enumerate() {
                row[i + j] = ch;
            }
        }
    }
    row
}

fn month_abbr(month: i8) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

/// Pre-styled cell per activity level 0..=4 (color + glyph + reset baked in).
const CELLS: [&str; 5] = [
    "\x1b[38;2;70;70;70m·\x1b[0m",
    "\x1b[38;2;190;120;80m░\x1b[0m",
    "\x1b[38;2;210;105;60m▒\x1b[0m",
    "\x1b[38;2;220;90;45m▓\x1b[0m",
    "\x1b[38;2;225;70;30m█\x1b[0m",
];

/// Activity scale derived from quartiles of the written days' character counts.
struct Scale {
    p25: usize,
    p50: usize,
    p75: usize,
}

impl Scale {
    fn from_counts(char_counts: &HashMap<Date, usize>) -> Self {
        if char_counts.is_empty() {
            return Self {
                p25: 0,
                p50: 0,
                p75: 0,
            };
        }

        let mut values: Vec<usize> = char_counts.values().copied().collect();
        values.sort_unstable();

        let pct = |p: f64| -> usize {
            let i = ((values.len() as f64 * p) as usize).min(values.len() - 1);
            values[i]
        };

        Self {
            p25: pct(0.25),
            p50: pct(0.50),
            p75: pct(0.75),
        }
    }

    /// The pre-styled cell for a day of `size` chars, banded as 0 = no activity
    /// then the p25/p50/p75 quartiles (levels 1..=4).
    fn cell(&self, size: usize) -> &'static str {
        if size == 0 {
            CELLS[0]
        } else if size <= self.p25 {
            CELLS[1]
        } else if size <= self.p50 {
            CELLS[2]
        } else if size <= self.p75 {
            CELLS[3]
        } else {
            CELLS[4]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn stats_to_for_past_month_is_month_end() {
        // A clearly-past month is not clamped to today.
        let when: When = "2020-02".parse().unwrap();
        assert_eq!(when.stats_to().unwrap(), date(2020, 2, 29));
    }

    #[test]
    fn stats_to_for_past_day_is_exact() {
        let when: When = "2020-02-10".parse().unwrap();
        assert_eq!(when.stats_to().unwrap(), date(2020, 2, 10));
    }

    #[test]
    fn empty_content_returns_empty() {
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        assert!(count_chars_by_day("", from, to).is_empty());
    }

    #[test]
    fn single_day_counted() {
        let content = "###### 2026-01-01 Thu\n\nhello\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], (date(2026, 1, 1), 5));
    }

    #[test]
    fn placeholder_not_counted() {
        let content = "###### 2026-01-01 Thu\n\n<!-- -->\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        assert!(count_chars_by_day(content, from, to).is_empty());
    }

    #[test]
    fn date_before_range_excluded() {
        let content = "###### 2025-12-31 Wed\n\nhello\n\n###### 2026-01-01 Thu\n\nworld\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, date(2026, 1, 1));
    }

    #[test]
    fn counts_unicode_chars_not_bytes() {
        let content = "###### 2026-01-01 Thu\n\nこんにちは\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result[0].1, 5);
    }

    #[test]
    fn multiple_lines_summed() {
        let content = "###### 2026-01-01 Thu\n\nhello\nworld\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result[0].1, 10);
    }

    #[test]
    fn zero_char_day_excluded() {
        let content = "###### 2026-01-01 Thu\n\n###### 2026-01-02 Fri\n\nhello\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, date(2026, 1, 2));
    }

    #[test]
    fn boundary_dates_included() {
        let content = "###### 2026-01-01 Thu\n\nfirst\n\n###### 2026-01-31 Sat\n\nlast\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result.len(), 2);
    }

    fn scale(p25: usize, p50: usize, p75: usize) -> Scale {
        Scale { p25, p50, p75 }
    }

    #[test]
    fn scale_cell_band_boundaries() {
        let s = scale(10, 20, 30);
        assert_eq!(s.cell(0), CELLS[0]); // no activity
        assert_eq!(s.cell(10), CELLS[1]); // exactly p25
        assert_eq!(s.cell(11), CELLS[2]); // between p25 and p50
        assert_eq!(s.cell(20), CELLS[2]); // exactly p50
        assert_eq!(s.cell(21), CELLS[3]); // between p50 and p75
        assert_eq!(s.cell(30), CELLS[3]); // exactly p75
        assert_eq!(s.cell(31), CELLS[4]); // above p75
    }

    #[test]
    fn scale_from_empty_counts_is_zero() {
        let s = Scale::from_counts(&HashMap::new());
        assert_eq!((s.p25, s.p50, s.p75), (0, 0, 0));
    }

    #[test]
    fn grid_renders_header_and_summary() {
        let counts = HashMap::from([(date(2026, 1, 1), 5)]);
        let out = Grid::new(&counts, date(2026, 1, 1), date(2026, 1, 7))
            .unwrap()
            .to_string();
        assert!(out.contains("2026-01-01 –> 2026-01-07"));
        assert!(out.contains("1 / 7 days written"));
    }

    #[test]
    fn build_weeks_single_week_plus_trailing_empties() {
        // Mon 2026-01-05 .. Sun 2026-01-11
        let weeks = build_weeks(date(2026, 1, 5), date(2026, 1, 11)).unwrap();
        assert_eq!(weeks.len(), 3); // 1 real week + 2 trailing empty
        assert_eq!(weeks[0][0], Some(date(2026, 1, 5)));
        assert_eq!(weeks[0][6], Some(date(2026, 1, 11)));
        assert!(weeks[1].iter().all(Option::is_none));
        assert!(weeks[2].iter().all(Option::is_none));
    }

    #[test]
    fn month_label_row_places_abbr_at_first_of_month() {
        // 2026-01-01 (Thu) — its week column is the first, so "Jan" starts at col 0.
        let weeks = build_weeks(date(2026, 1, 1), date(2026, 1, 7)).unwrap();
        let row = month_label_row(&weeks);
        assert_eq!(&row[0..3], ['J', 'a', 'n']);
    }

    #[test]
    fn month_abbr_all_valid() {
        let expected = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        for (i, name) in expected.iter().enumerate() {
            assert_eq!(month_abbr((i + 1) as i8), *name);
        }
    }

    #[test]
    fn month_abbr_invalid() {
        assert_eq!(month_abbr(0), "???");
        assert_eq!(month_abbr(13), "???");
    }
}
