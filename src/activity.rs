//! A single tracked activity and its one-line text representation.
//!
//! Line format:
//!   running:  `2026-11-22 09:00 | project | description`
//!   finished: `2026-11-22 09:00 - 2026-11-22 10:30 | project | description`
//! A literal `|` or `\` inside a field is written as `\|` or `\\`.

use std::fmt;

use chrono::{Duration, NaiveDateTime, Timelike};

pub const TIME_FORMAT: &str = "%Y-%m-%d %H:%M";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub project: String,
    pub description: String,
}

impl Activity {
    pub fn new(start: NaiveDateTime, project: String, description: String) -> Self {
        Activity { start: truncate(start), end: None, project, description }
    }

    pub fn is_running(&self) -> bool {
        self.end.is_none()
    }

    /// Duration of the activity; running activities are measured up to `now`.
    pub fn duration(&self, now: NaiveDateTime) -> Duration {
        self.end.unwrap_or(now) - self.start
    }

    pub fn parse(line: &str) -> Result<Activity, String> {
        let fields = split_fields(line);
        if fields.len() < 2 {
            return Err("expected `TIME | PROJECT [| DESCRIPTION]`".into());
        }
        if fields.len() > 3 {
            return Err("too many fields (escape literal pipes as `\\|`)".into());
        }

        let times = fields[0].trim();
        let (start, end) = match times.split_once(" - ") {
            Some((s, e)) => (parse_time(s)?, Some(parse_time(e)?)),
            None => (parse_time(times)?, None),
        };

        let project = fields[1].trim().to_string();
        if project.is_empty() {
            return Err("project name is empty".into());
        }
        let description = fields.get(2).map(|d| d.trim().to_string()).unwrap_or_default();

        Ok(Activity { start, end, project, description })
    }
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.start.format(TIME_FORMAT))?;
        if let Some(end) = self.end {
            write!(f, " - {}", end.format(TIME_FORMAT))?;
        }
        write!(f, " | {}", escape(&self.project))?;
        if !self.description.is_empty() {
            write!(f, " | {}", escape(&self.description))?;
        }
        Ok(())
    }
}

/// Drops seconds so stored times match the minute precision of the file.
pub fn truncate(t: NaiveDateTime) -> NaiveDateTime {
    t.with_second(0).and_then(|t| t.with_nanosecond(0)).unwrap_or(t)
}

fn parse_time(s: &str) -> Result<NaiveDateTime, String> {
    let s = s.trim();
    NaiveDateTime::parse_from_str(s, TIME_FORMAT)
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").map(truncate))
        .map_err(|_| format!("invalid time `{s}` (expected YYYY-MM-DD HH:MM)"))
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|")
}

/// Splits on unescaped `|` and resolves `\|` / `\\` escapes.
fn split_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if matches!(chars.peek(), Some('|') | Some('\\')) => {
                fields.last_mut().unwrap().push(chars.next().unwrap());
            }
            '|' => fields.push(String::new()),
            _ => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, TIME_FORMAT).unwrap()
    }

    #[test]
    fn parses_running_and_finished() {
        let a = Activity::parse("2026-11-22 09:00 | work | coding").unwrap();
        assert_eq!(a.start, t("2026-11-22 09:00"));
        assert!(a.is_running());
        assert_eq!(a.project, "work");
        assert_eq!(a.description, "coding");

        let b = Activity::parse("2026-11-22 09:00 - 2026-11-22 10:15 | work").unwrap();
        assert_eq!(b.end, Some(t("2026-11-22 10:15")));
        assert_eq!(b.description, "");
        assert_eq!(b.duration(t("2030-01-01 00:00")), Duration::minutes(75));
    }

    #[test]
    fn round_trips_escapes() {
        let line = r"2026-11-22 09:00 - 2026-11-22 10:00 | client\|work | C:\\Users\\me";
        let a = Activity::parse(line).unwrap();
        assert_eq!(a.project, "client|work");
        assert_eq!(a.description, r"C:\Users\me");
        assert_eq!(a.to_string(), line);
    }

    #[test]
    fn rejects_garbage() {
        assert!(Activity::parse("hello").is_err());
        assert!(Activity::parse("2026-13-01 09:00 | x").is_err());
        assert!(Activity::parse("2026-11-22 09:00 |  | x").is_err());
    }
}
