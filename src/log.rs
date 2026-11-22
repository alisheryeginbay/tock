//! The log file: one activity per line, in recording order.
//!
//! Lines that fail to parse are kept verbatim so that rewriting the file never
//! destroys hand edits; `tock check` reports them.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::activity::Activity;

pub enum Line {
    Activity(Activity),
    Invalid { text: String, error: String },
    Blank(String),
}

pub struct Log {
    path: PathBuf,
    pub lines: Vec<Line>,
}

impl Log {
    pub fn load(path: &Path) -> Result<Log> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
        };

        let lines = content
            .lines()
            .map(|text| {
                let trimmed = text.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    Line::Blank(text.to_string())
                } else {
                    match Activity::parse(text) {
                        Ok(a) => Line::Activity(a),
                        Err(error) => Line::Invalid { text: text.to_string(), error },
                    }
                }
            })
            .collect();

        Ok(Log { path: path.to_path_buf(), lines })
    }

    /// Writes the whole log atomically (temp file + rename).
    pub fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let mut out = String::new();
        for line in &self.lines {
            match line {
                Line::Activity(a) => out.push_str(&a.to_string()),
                Line::Invalid { text, .. } | Line::Blank(text) => out.push_str(text),
            }
            out.push('\n');
        }

        let tmp = self.path.with_extension("tock-tmp");
        let mut f = fs::File::create(&tmp).with_context(|| format!("cannot write {}", tmp.display()))?;
        f.write_all(out.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, &self.path).with_context(|| format!("cannot write {}", self.path.display()))?;
        Ok(())
    }

    pub fn push(&mut self, activity: Activity) {
        self.lines.push(Line::Activity(activity));
    }

    /// All parsed activities with their 1-based line numbers.
    pub fn activities(&self) -> impl Iterator<Item = (usize, &Activity)> {
        self.lines.iter().enumerate().filter_map(|(i, l)| match l {
            Line::Activity(a) => Some((i + 1, a)),
            _ => None,
        })
    }

    pub fn running(&self) -> impl Iterator<Item = &Activity> {
        self.activities().map(|(_, a)| a).filter(|a| a.is_running())
    }

    pub fn running_mut(&mut self) -> impl Iterator<Item = &mut Activity> {
        self.lines.iter_mut().filter_map(|l| match l {
            Line::Activity(a) if a.is_running() => Some(a),
            _ => None,
        })
    }

    pub fn remove_running(&mut self) -> Vec<Activity> {
        let mut removed = Vec::new();
        self.lines.retain(|l| match l {
            Line::Activity(a) if a.is_running() => {
                removed.push(a.clone());
                false
            }
            _ => true,
        });
        removed
    }

    pub fn invalid(&self) -> impl Iterator<Item = (usize, &str, &str)> {
        self.lines.iter().enumerate().filter_map(|(i, l)| match l {
            Line::Invalid { text, error } => Some((i + 1, text.as_str(), error.as_str())),
            _ => None,
        })
    }
}
