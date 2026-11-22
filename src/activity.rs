//! A single tracked activity and its one-line text representation.
//!
//! Line format:
//!   running:  `2026-11-22 09:00 | project | description`
//!   finished: `2026-11-22 09:00 - 2026-11-22 10:30 | project | description`
//!   tagged:   `2026-11-22 09:00 | project | description | billable, meeting`
//! A literal `|` or `\` inside a field is written as `\|` or `\\`.
//! Tags are lowercase and may not contain whitespace, `,` or `|`.

use std::fmt;

use chrono::{Duration, NaiveDateTime, Timelike};

pub const TIME_FORMAT: &str = "%Y-%m-%d %H:%M";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub start: NaiveDateTime,
    pub end: Option<NaiveDateTime>,
    pub project: String,
    pub description: String,
    pub tags: Vec<String>,
}

impl Activity {
    pub fn new(start: NaiveDateTime, project: String, description: String, tags: Vec<String>) -> Self {
        let mut a = Activity { start: truncate(start), end: None, project, description, tags: Vec::new() };
        a.add_tags(tags);
        a
    }

    /// Adds tags that aren't already present, keeping their order.
    pub fn add_tags(&mut self, tags: impl IntoIterator<Item = String>) {
        for tag in tags {
            if !self.tags.contains(&tag) {
                self.tags.push(tag);
            }
        }
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
        if fields.len() > 4 {
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
        let tags = match fields.get(3) {
            Some(t) => t
                .split(',')
                .filter(|t| !t.trim().is_empty())
                .map(parse_tag)
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let mut a = Activity { start, end, project, description, tags: Vec::new() };
        a.add_tags(tags);
        Ok(a)
    }
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.start.format(TIME_FORMAT))?;
        if let Some(end) = self.end {
            write!(f, " - {}", end.format(TIME_FORMAT))?;
        }
        write!(f, " | {}", escape(&self.project))?;
        if !self.description.is_empty() || !self.tags.is_empty() {
            write!(f, " | {}", escape(&self.description))?;
        }
        if !self.tags.is_empty() {
            write!(f, " | {}", self.tags.join(", "))?;
        }
        Ok(())
    }
}

/// Drops seconds so stored times match the minute precision of the file.
pub fn truncate(t: NaiveDateTime) -> NaiveDateTime {
    t.with_second(0).and_then(|t| t.with_nanosecond(0)).unwrap_or(t)
}

/// Normalizes a tag to lowercase and rejects characters the file format can't hold.
pub fn parse_tag(s: &str) -> Result<String, String> {
    let tag = s.trim().trim_start_matches('#').to_lowercase();
    if tag.is_empty() {
        return Err("tag is empty".into());
    }
    if tag.chars().any(|c| c.is_whitespace() || c == ',' || c == '|') {
        return Err(format!("invalid tag `{}` (no spaces, commas or pipes)", s.trim()));
    }
    Ok(tag)
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
    fn round_trips_tags() {
        let line = "2026-11-22 09:00 | work |  | billable, deep-work";
        let a = Activity::parse(line).unwrap();
        assert_eq!(a.description, "");
        assert_eq!(a.tags, ["billable", "deep-work"]);
        assert_eq!(a.to_string(), line);

        let b = Activity::parse("2026-11-22 09:00 | work | x | Billable,billable, #Meeting").unwrap();
        assert_eq!(b.tags, ["billable", "meeting"]);
        assert_eq!(b.to_string(), "2026-11-22 09:00 | work | x | billable, meeting");
    }

    #[test]
    fn rejects_garbage() {
        assert!(Activity::parse("2026-11-22 09:00 | x | y | two words").is_err());
        assert!(Activity::parse("2026-11-22 09:00 | x | y | a | b").is_err());
        assert!(Activity::parse("hello").is_err());
        assert!(Activity::parse("2026-13-01 09:00 | x").is_err());
        assert!(Activity::parse("2026-11-22 09:00 |  | x").is_err());
    }
}
