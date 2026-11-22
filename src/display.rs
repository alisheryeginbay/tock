//! Formatting helpers shared by the commands.

use std::io::IsTerminal;
use std::sync::OnceLock;

use chrono::{Duration, NaiveDateTime};
use unicode_width::UnicodeWidthStr;

pub fn duration(d: Duration) -> String {
    let mins = d.num_minutes();
    let sign = if mins < 0 { "-" } else { "" };
    let mins = mins.abs();
    if mins >= 60 {
        format!("{sign}{}h {:02}m", mins / 60, mins % 60)
    } else {
        format!("{sign}{mins}m")
    }
}

pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn pad(s: &str, w: usize) -> String {
    format!("{s}{}", " ".repeat(w.saturating_sub(width(s))))
}

pub fn pad_left(s: &str, w: usize) -> String {
    format!("{}{s}", " ".repeat(w.saturating_sub(width(s))))
}

fn color_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
}

fn paint(code: &str, s: &str) -> String {
    if color_enabled() { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
}

pub fn bold(s: &str) -> String {
    paint("1", s)
}

pub fn dim(s: &str) -> String {
    paint("2", s)
}

pub fn green(s: &str) -> String {
    paint("32", s)
}

pub fn yellow(s: &str) -> String {
    paint("33", s)
}

/// Column widths for a set of rows (all rows must have the same length).
pub fn column_widths(rows: &[Vec<String>]) -> Vec<usize> {
    let cols = rows.first().map_or(0, |r| r.len());
    (0..cols).map(|c| rows.iter().map(|r| width(&r[c])).max().unwrap_or(0)).collect()
}

/// Renders a row left-aligned, optionally right-aligning the last column (durations).
pub fn render_row(row: &[String], widths: &[usize], right_last: bool) -> String {
    let last = row.len().saturating_sub(1);
    let line = row
        .iter()
        .zip(widths)
        .enumerate()
        .map(|(i, (cell, &w))| if i == last && right_last { pad_left(cell, w) } else { pad(cell, w) })
        .collect::<Vec<_>>()
        .join("  ");
    line.trim_end().to_string()
}

pub fn print_table(header: &[&str], rows: &[Vec<String>], right_last: bool) {
    let mut all = vec![header.iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    all.extend(rows.iter().cloned());
    let widths = column_widths(&all);
    println!("{}", bold(&render_row(&all[0], &widths, right_last)));
    for row in &all[1..] {
        println!("{}", render_row(row, &widths, right_last));
    }
}

/// Case-insensitive wildcard match supporting `*` and `?`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Rounds a time to the nearest multiple of `minutes`.
pub fn round_time(t: NaiveDateTime, minutes: i64) -> NaiveDateTime {
    if minutes <= 1 {
        return t;
    }
    let secs = t.and_utc().timestamp();
    let step = minutes * 60;
    let rounded = (secs + step / 2).div_euclid(step) * step;
    t + Duration::seconds(rounded - secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(duration(Duration::minutes(0)), "0m");
        assert_eq!(duration(Duration::minutes(43)), "43m");
        assert_eq!(duration(Duration::minutes(125)), "2h 05m");
        assert_eq!(duration(Duration::minutes(-90)), "-1h 30m");
    }

    #[test]
    fn matches_globs() {
        assert!(glob_match("e*t?ng", "eating"));
        assert!(glob_match("*meet*", "Team Meeting"));
        assert!(glob_match("cl*", "Client"));
        assert!(!glob_match("cl?", "client"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn rounds_times() {
        let t = |s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap();
        assert_eq!(round_time(t("2026-11-22 09:07"), 15), t("2026-11-22 09:00"));
        assert_eq!(round_time(t("2026-11-22 09:08"), 15), t("2026-11-22 09:15"));
        assert_eq!(round_time(t("2026-11-22 09:31"), 60), t("2026-11-22 10:00"));
    }
}
