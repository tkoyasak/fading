use std::collections::HashMap;

use anyhow::{Context, Result};
use jiff::{ToSpan, Zoned, civil::Date};
use xshell::Shell;

use crate::{Cmd, Ctx, Stats, month::parse_month};

impl Cmd for Stats {
    fn run(self, ctx: Ctx) -> Result<()> {
        let sh = ctx.sh;
        let id = parse_month(self.month.as_deref())?;
        let today = Zoned::now().date();
        let to = if self.month.is_none() {
            today
        } else {
            let last: Date = format!("{id}-01").parse().context("date parse")?;
            let last = last
                .checked_add(1.months())
                .context("date arithmetic")?
                .checked_sub(1.days())
                .context("date arithmetic")?;
            last.min(today)
        };
        let from = to.checked_sub(364.days()).context("date arithmetic")?;

        let char_counts = find_char_counts(&sh, from, to)?;
        render_grid(&char_counts, from, to)
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
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("thread panicked")))
            })
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

fn activity_level(size: usize, p25: usize, p50: usize, p75: usize) -> u8 {
    if size == 0 {
        0
    } else if size <= p25 {
        1
    } else if size <= p50 {
        2
    } else if size <= p75 {
        3
    } else {
        4
    }
}

fn render_grid(char_counts: &HashMap<Date, usize>, from: Date, to: Date) -> Result<()> {
    const R: &str = "\x1b[0m";
    const DIM: &str = "\x1b[38;2;130;130;130m";
    // Activity level colors (orange gradient)
    const C: [&str; 4] = [
        "\x1b[38;2;190;120;80m",
        "\x1b[38;2;210;105;60m",
        "\x1b[38;2;220;90;45m",
        "\x1b[38;2;225;70;30m",
    ];
    // Inactive
    const CDOT: &str = "\x1b[38;2;70;70;70m";
    const CELLS: [&str; 5] = ["·", "░", "▒", "▓", "█"];

    println!(
        "  {DIM}{} –> {}{R}",
        from.strftime("%Y-%m-%d"),
        to.strftime("%Y-%m-%d")
    );
    println!();

    // Compute quartile thresholds from written days
    let mut values: Vec<usize> = char_counts.values().copied().collect();
    values.sort_unstable();
    let pct = |p: f64| -> usize {
        if values.is_empty() {
            return 0;
        }
        let i = ((values.len() as f64 * p) as usize).min(values.len() - 1);
        values[i]
    };
    let (p25, p50, p75) = (pct(0.25), pct(0.50), pct(0.75));

    let cell = |size: usize| CELLS[activity_level(size, p25, p50, p75) as usize];
    let color = |size: usize| {
        let level = activity_level(size, p25, p50, p75);
        if level == 0 {
            CDOT
        } else {
            C[(level - 1) as usize]
        }
    };

    // Start at Monday of the week containing `from`
    let week_start_offset = from.weekday().to_monday_zero_offset() as i64;
    let grid_start = from
        .checked_sub(week_start_offset.days())
        .context("grid start date arithmetic")?;

    let mut weeks: Vec<[Option<Date>; 7]> = Vec::new();
    let mut col_start = grid_start;
    loop {
        let mut week = [None; 7];
        for i in 0..7i64 {
            let day = col_start
                .checked_add(i.days())
                .context("day offset arithmetic")?;
            if day >= from && day <= to {
                week[i as usize] = Some(day);
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
    // Two trailing empty weeks so a month name at the last column is never truncated.
    weeks.push([None; 7]);
    weeks.push([None; 7]);

    // Month labels row — place 3-char abbr at the week column containing the 1st of each month.
    let mut label_row = vec![' '; weeks.len()];
    for (i, week) in weeks.iter().enumerate() {
        if let Some(d) = week.iter().find_map(|d| d.filter(|d| d.day() == 1)) {
            for (j, ch) in month_abbr(d.month()).chars().enumerate() {
                label_row[i + j] = ch;
            }
        }
    }
    print!("      ");
    for ch in &label_row {
        print!("{DIM}{ch}{R}");
    }
    println!();

    // Day rows: Mon(0)..Sun(6), all labeled
    let day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    for row in 0..7usize {
        print!("  {DIM}{}{R} ", day_labels[row]);
        for week in &weeks {
            match week[row] {
                Some(d) => {
                    let size = char_counts.get(&d).copied().unwrap_or(0);
                    print!("{}{}{R}", color(size), cell(size));
                }
                None => print!(" "),
            }
        }
        println!();
    }

    // Legend + summary
    let total_days = (from.series(1.days())).take_while(|d| *d <= to).count();
    let written_days = char_counts.len();
    println!();
    println!(
        "      {DIM}Less{R} {}░{R} {}▒{R} {}▓{R} {}█{R} {DIM}More{R}",
        C[0], C[1], C[2], C[3]
    );
    println!();
    println!("  {DIM}{written_days} / {total_days} days written{R}");
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

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

    #[test]
    fn activity_level_zero() {
        assert_eq!(activity_level(0, 10, 20, 30), 0);
    }

    #[test]
    fn activity_level_boundaries() {
        assert_eq!(activity_level(10, 10, 20, 30), 1); // exactly p25
        assert_eq!(activity_level(11, 10, 20, 30), 2); // between p25 and p50
        assert_eq!(activity_level(20, 10, 20, 30), 2); // exactly p50
        assert_eq!(activity_level(21, 10, 20, 30), 3); // between p50 and p75
        assert_eq!(activity_level(30, 10, 20, 30), 3); // exactly p75
        assert_eq!(activity_level(31, 10, 20, 30), 4); // above p75
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
