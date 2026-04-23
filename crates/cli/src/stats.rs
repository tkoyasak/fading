use std::collections::HashMap;

use anyhow::{Context, Result};
use jiff::{ToSpan, Zoned, civil::Date};
use xshell::Shell;

use crate::{Cmd, Stats, month::parse_month};

impl Cmd for Stats {
    fn run(self, sh: Shell) -> Result<()> {
        let id = parse_month(&self.month)?;
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
        render_grid(&char_counts, from, to);
        Ok(())
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
    let mut results = Vec::new();
    let mut current_date: Option<Date> = None;
    let mut chars = 0usize;

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("###### ") {
            if let Some(date) = current_date.take() {
                if chars > 0 && date >= from && date <= to {
                    results.push((date, chars));
                }
                chars = 0;
            }
            current_date = rest.get(..10).and_then(|s| s.parse().ok());
        } else if current_date.is_some() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && trimmed != "<!-- -->" {
                chars += trimmed.chars().count();
            }
        }
    }
    if let Some(date) = current_date
        && chars > 0
        && date >= from
        && date <= to
    {
        results.push((date, chars));
    }
    results
}

fn render_grid(char_counts: &HashMap<Date, usize>, from: Date, to: Date) {
    const R: &str = "\x1b[0m";
    const DIM: &str = "\x1b[38;2;130;130;130m";
    // Activity level colors (orange gradient)
    const C1: &str = "\x1b[38;2;190;120;80m";
    const C2: &str = "\x1b[38;2;210;105;60m";
    const C3: &str = "\x1b[38;2;220;90;45m";
    const C4: &str = "\x1b[38;2;225;70;30m";
    // Inactive
    const CDOT: &str = "\x1b[38;2;70;70;70m";

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

    let activity_cell = |size: usize| -> &'static str {
        if size == 0 {
            return "·";
        }
        if size <= p25 {
            "░"
        } else if size <= p50 {
            "▒"
        } else if size <= p75 {
            "▓"
        } else {
            "█"
        }
    };
    let activity_color = |size: usize| -> &'static str {
        if size == 0 {
            return CDOT;
        }
        if size <= p25 {
            C1
        } else if size <= p50 {
            C2
        } else if size <= p75 {
            C3
        } else {
            C4
        }
    };

    // Start at Monday of the week containing `from`
    let week_start_offset = from.weekday().to_monday_zero_offset() as i64;
    let grid_start = from.checked_sub(week_start_offset.days()).unwrap();

    let mut weeks: Vec<[Option<Date>; 7]> = Vec::new();
    let mut col_start = grid_start;
    loop {
        let mut week = [None; 7];
        for i in 0..7i64 {
            let day = col_start.checked_add(i.days()).unwrap();
            if day >= from && day <= to {
                week[i as usize] = Some(day);
            }
        }
        weeks.push(week);
        if col_start.checked_add(6.days()).unwrap() >= to {
            break;
        }
        col_start = col_start.checked_add(7.days()).unwrap();
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
                    let col = activity_color(size);
                    let ch = activity_cell(size);
                    print!("{col}{ch}{R}");
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
    println!("      {DIM}Less{R} {C1}░{R} {C2}▒{R} {C3}▓{R} {C4}█{R} {DIM}More{R}");
    println!();
    println!("  {DIM}{written_days} / {total_days} days written{R}");
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
        // "こんにちは" = 5 chars, 15 bytes
        let content = "###### 2026-01-01 Thu\n\nこんにちは\n";
        let from = date(2026, 1, 1);
        let to = date(2026, 1, 31);
        let result = count_chars_by_day(content, from, to);
        assert_eq!(result[0].1, 5);
    }
}
