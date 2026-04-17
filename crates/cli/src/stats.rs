use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use jiff::{ToSpan, Zoned, civil::Date};

use crate::{Cmd, Stats, month::parse_month_arg};

impl Cmd for Stats {
    fn run(self) -> Result<()> {
        let home = std::env::var("FADING_HOME").context("FADING_HOME env is not set")?;
        let entries_dir = Path::new(&home).join("entries");

        let today = Zoned::now().date();
        let month_str = parse_month_arg(&self.month)?;
        let to = if self.month.is_none() {
            today
        } else {
            let last: Date = format!("{month_str}-01").parse().context("date parse")?;
            let last = last
                .checked_add(1.months())
                .context("date arithmetic")?
                .checked_sub(1.days())
                .context("date arithmetic")?;
            last.min(today)
        };
        let from = to.checked_sub(364.days()).context("date arithmetic")?;

        let entries = find_entry_lengths(&entries_dir, from, to)?;
        render_grid(&entries, from, to);
        Ok(())
    }
}

/// Returns a map of date → char count for days with actual content.
fn find_entry_lengths(entries_dir: &Path, from: Date, to: Date) -> Result<HashMap<Date, usize>> {
    let mut map = HashMap::new();

    let mut month = from.first_of_month();
    loop {
        let month_str = format!("{:04}-{:02}", month.year(), month.month());
        let path = entries_dir.join(format!("{month_str}.md"));

        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            for (day, len) in parse_entry_lengths(&content, from, to) {
                map.insert(day, len);
            }
        }

        if month.year() == to.year() && month.month() == to.month() {
            break;
        }
        month = month
            .checked_add(1.months())
            .context("date arithmetic")?
            .first_of_month();
    }

    Ok(map)
}

fn parse_entry_lengths(content: &str, from: Date, to: Date) -> Vec<(Date, usize)> {
    let mut result = Vec::new();
    let mut current_date: Option<Date> = None;
    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        if !frontmatter_done {
            if line == "+++" {
                if !in_frontmatter {
                    in_frontmatter = true;
                } else {
                    frontmatter_done = true;
                }
                continue;
            }
            if in_frontmatter {
                continue;
            }
        }

        if let Some(date) = parse_heading_date(line) {
            if let Some(d) = current_date
                && d >= from
                && d <= to
            {
                let len = entry_char_count(&current_lines);
                if len > 0 {
                    result.push((d, len));
                }
            }
            current_date = Some(date);
            current_lines.clear();
        } else if current_date.is_some() {
            current_lines.push(line);
        }
    }

    if let Some(d) = current_date
        && d >= from
        && d <= to
    {
        let len = entry_char_count(&current_lines);
        if len > 0 {
            result.push((d, len));
        }
    }

    result
}

fn parse_heading_date(line: &str) -> Option<Date> {
    let rest = line.strip_prefix("###### ")?;
    rest.get(..10)?.parse().ok()
}

/// Returns the char count of the entry, or 0 if it's empty/placeholder.
fn entry_char_count(lines: &[&str]) -> usize {
    let joined = lines.join("\n");
    let trimmed = joined.trim();
    if trimmed.is_empty() || trimmed == "<!-- -->" {
        0
    } else {
        trimmed.chars().count()
    }
}

fn render_grid(entries: &HashMap<Date, usize>, from: Date, to: Date) {
    const R: &str = "\x1b[0m";
    const DIM: &str = "\x1b[38;2;130;130;130m";
    // Activity level colors (orange gradient)
    const C1: &str = "\x1b[38;2;190;120;80m";
    const C2: &str = "\x1b[38;2;210;105;60m";
    const C3: &str = "\x1b[38;2;220;90;45m";
    const C4: &str = "\x1b[38;2;225;70;30m";
    // Inactive
    const CDOT: &str = "\x1b[38;2;70;70;70m";

    // Compute quartile thresholds from written days
    let mut lens: Vec<usize> = entries.values().copied().collect();
    lens.sort_unstable();
    let pct = |p: f64| -> usize {
        if lens.is_empty() {
            return 0;
        }
        let i = ((lens.len() as f64 * p) as usize).min(lens.len() - 1);
        lens[i]
    };
    let (p25, p50, p75) = (pct(0.25), pct(0.50), pct(0.75));

    let activity_cell = |len: usize| -> &'static str {
        if len == 0 {
            return "·";
        }
        if len <= p25 {
            "░"
        } else if len <= p50 {
            "▒"
        } else if len <= p75 {
            "▓"
        } else {
            "█"
        }
    };
    let activity_color = |len: usize| -> &'static str {
        if len == 0 {
            return CDOT;
        }
        if len <= p25 {
            C1
        } else if len <= p50 {
            C2
        } else if len <= p75 {
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
                    let len = entries.get(&d).copied().unwrap_or(0);
                    let col = activity_color(len);
                    let ch = activity_cell(len);
                    print!("{col}{ch}{R}");
                }
                None => print!(" "),
            }
        }
        println!();
    }

    // Legend + summary
    let total_days = (from.series(1.days())).take_while(|d| *d <= to).count();
    let written_days = entries.len();
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

    #[test]
    fn test_parse_heading_date() {
        assert_eq!(
            parse_heading_date("###### 2026-01-15 Thu"),
            Some("2026-01-15".parse().unwrap())
        );
        assert_eq!(parse_heading_date("## Not a heading"), None);
        assert_eq!(parse_heading_date("###### invalid-date Thu"), None);
    }

    #[test]
    fn test_entry_char_count() {
        assert_eq!(entry_char_count(&[]), 0);
        assert_eq!(entry_char_count(&[""]), 0);
        assert_eq!(entry_char_count(&["<!-- -->"]), 0);
        assert_eq!(entry_char_count(&["", "<!-- -->", ""]), 0);
        assert_eq!(entry_char_count(&["Hello"]), 5);
        assert_eq!(entry_char_count(&["こんにちは"]), 5);
    }

    #[test]
    fn test_parse_entry_lengths() {
        let content = r#"+++
id = "2026-01"
created = 2026-01-01
modified = 2026-01-15
+++

###### 2026-01-14 Wed

こんにちは．

###### 2026-01-15 Thu

<!-- -->

###### 2026-01-16 Fri

また明日ね．
"#;
        let from = "2026-01-01".parse().unwrap();
        let to = "2026-01-31".parse().unwrap();
        let mut days = parse_entry_lengths(content, from, to);
        days.sort_by_key(|(d, _)| *d);

        assert_eq!(days.len(), 2);
        assert_eq!(days[0].0, "2026-01-14".parse::<Date>().unwrap());
        assert!(days[0].1 > 0);
        assert_eq!(days[1].0, "2026-01-16".parse::<Date>().unwrap());
        assert!(days[1].1 > 0);
        // 2026-01-15 is placeholder, should be absent
        assert!(
            !days
                .iter()
                .any(|(d, _)| *d == "2026-01-15".parse::<Date>().unwrap())
        );
    }
}
