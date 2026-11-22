use std::collections::BTreeMap;
use std::path::Path;
use std::process;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::activity::{Activity, TIME_FORMAT};
use crate::display::{self, bold, dim, duration, glob_match, green, yellow};
use crate::log::Log;
use crate::Filter;

// ---------- recording ----------

pub fn start(path: &Path, project: String, description: String, at: NaiveDateTime) -> Result<()> {
    let mut log = Log::load(path)?;
    begin(&mut log, Activity::new(at, project, description))
}

pub fn continue_(
    path: &Path,
    number: usize,
    project: Option<String>,
    description: Option<String>,
    at: NaiveDateTime,
) -> Result<()> {
    let mut log = Log::load(path)?;
    let Some((p, d)) = recent_pairs(&log).into_iter().nth(number) else {
        bail!("there is no recent activity #{number} (see `tock last`)");
    };
    let activity = Activity::new(at, project.unwrap_or(p), description.unwrap_or(d));
    begin(&mut log, activity)
}

fn begin(log: &mut Log, activity: Activity) -> Result<()> {
    let stopped = stop_running(log, activity.start)?;
    let started = describe(&activity);
    let at = activity.start;
    log.push(activity);
    log.save()?;
    report_stopped(&stopped);
    println!("Started {started} at {}", bold(&at.format(TIME_FORMAT).to_string()));
    Ok(())
}

pub fn stop(path: &Path, at: NaiveDateTime) -> Result<()> {
    let mut log = Log::load(path)?;
    let stopped = stop_running(&mut log, at)?;
    if stopped.is_empty() {
        println!("No activity is running.");
        return Ok(());
    }
    log.save()?;
    report_stopped(&stopped);
    Ok(())
}

fn stop_running(log: &mut Log, at: NaiveDateTime) -> Result<Vec<Activity>> {
    if let Some(a) = log.running().find(|a| a.start > at) {
        bail!(
            "cannot stop {} at {}: it started at {}",
            describe(a),
            at.format(TIME_FORMAT),
            a.start.format(TIME_FORMAT)
        );
    }
    Ok(log
        .running_mut()
        .map(|a| {
            a.end = Some(at);
            a.clone()
        })
        .collect())
}

fn report_stopped(stopped: &[Activity]) {
    for a in stopped {
        println!(
            "Stopped {} after {}",
            describe(a),
            bold(&duration(a.duration(a.start)))
        );
    }
}

pub fn cancel(path: &Path) -> Result<()> {
    let mut log = Log::load(path)?;
    let removed = log.remove_running();
    if removed.is_empty() {
        println!("No activity is running.");
        return Ok(());
    }
    log.save()?;
    for a in &removed {
        println!("Cancelled {}", describe(a));
    }
    Ok(())
}

pub fn change(
    path: &Path,
    project: Option<String>,
    description: Option<String>,
    start: Option<NaiveDateTime>,
) -> Result<()> {
    let mut log = Log::load(path)?;
    let mut changed = Vec::new();
    for a in log.running_mut() {
        if let Some(p) = &project {
            a.project = p.clone();
        }
        if let Some(d) = &description {
            a.description = d.clone();
        }
        if let Some(s) = start {
            a.start = s;
        }
        changed.push(a.clone());
    }
    if changed.is_empty() {
        println!("No activity is running.");
        return Ok(());
    }
    log.save()?;
    for a in &changed {
        println!("Now tracking {} since {}", describe(a), a.start.format(TIME_FORMAT));
    }
    Ok(())
}

// ---------- viewing ----------

pub fn current(path: &Path, now: NaiveDateTime) -> Result<()> {
    let log = Log::load(path)?;
    let running: Vec<Activity> = log.running().cloned().collect();
    if running.is_empty() {
        println!("No activity is running.");
    } else {
        print_activities(&running, false, now);
    }
    Ok(())
}

pub fn status(path: &Path, project: Option<&str>, now: NaiveDateTime) -> Result<()> {
    let log = Log::load(path)?;

    let mut any = false;
    for a in log.running() {
        any = true;
        println!(
            "{} {} since {} ({})",
            green("●"),
            describe(a),
            a.start.format("%H:%M"),
            bold(&duration(a.duration(now)))
        );
    }
    if !any {
        println!("{} No activity is running.", dim("○"));
    }
    println!();

    let today = now.date();
    let week_start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let month_start = today.with_day(1).unwrap();
    let total_since = |from: NaiveDate| -> Duration {
        log.activities()
            .map(|(_, a)| a)
            .filter(|a| a.start.date() >= from && a.start.date() <= today)
            .filter(|a| project.is_none_or(|p| glob_match(p, &a.project)))
            .map(|a| a.duration(now))
            .sum()
    };

    let rows = [
        ("Today", total_since(today)),
        ("This week", total_since(week_start)),
        ("This month", total_since(month_start)),
    ];
    let durations: Vec<String> = rows.iter().map(|(_, d)| duration(*d)).collect();
    let w = durations.iter().map(|d| display::width(d)).max().unwrap_or(0);
    for ((label, _), d) in rows.iter().zip(&durations) {
        println!("{}  {}", display::pad(label, 10), display::pad_left(d, w));
    }
    Ok(())
}

pub fn list(path: &Path, filter: &Filter, number: Option<usize>, grouped: bool, now: NaiveDateTime) -> Result<()> {
    let log = Log::load(path)?;
    let mut acts = select(&log, filter, now);
    if let Some(n) = number {
        acts.drain(..acts.len().saturating_sub(n));
    }
    if acts.is_empty() {
        println!("No activities found.");
    } else {
        print_activities(&acts, grouped, now);
    }
    Ok(())
}

pub fn report(path: &Path, filter: &Filter, now: NaiveDateTime) -> Result<()> {
    let log = Log::load(path)?;
    let acts = select(&log, filter, now);
    if acts.is_empty() {
        println!("No activities found.");
        return Ok(());
    }

    let mut projects: BTreeMap<&str, (Duration, BTreeMap<&str, Duration>)> = BTreeMap::new();
    for a in &acts {
        let d = a.duration(now);
        let (total, descs) = projects.entry(&a.project).or_default();
        *total += d;
        *descs.entry(&a.description).or_default() += d;
    }
    let grand: Duration = projects.values().map(|(t, _)| *t).sum();

    let label = |d: &str| if d.is_empty() { "(no description)".to_string() } else { d.to_string() };
    let label_w = projects
        .iter()
        .flat_map(|(p, (_, ds))| std::iter::once(display::width(p)).chain(ds.keys().map(|d| 4 + display::width(&label(d)))))
        .max()
        .unwrap_or(0)
        .max(30)
        + 3;
    let dur_w = display::width(&duration(grand));
    let line = |text: &str, d: Duration| {
        let dots = ".".repeat(label_w.saturating_sub(display::width(text) + 1));
        format!("{text} {} {}", dim(&dots), display::pad_left(&duration(d), dur_w))
    };

    for (project, (total, descs)) in &projects {
        println!("{}", bold(&line(project, *total)));
        for (desc, d) in descs {
            println!("{}", line(&format!("    {}", label(desc)), *d));
        }
        println!();
    }
    println!("{}", bold(&line("Total", grand)));
    Ok(())
}

pub fn last(path: &Path, number: usize) -> Result<()> {
    let log = Log::load(path)?;
    let pairs = recent_pairs(&log);
    if pairs.is_empty() {
        println!("No activities recorded yet.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = pairs
        .into_iter()
        .take(number)
        .enumerate()
        .map(|(i, (p, d))| vec![format!("[{i}]"), d, p])
        .collect();
    display::print_table(&["#", "Description", "Project"], &rows, false);
    Ok(())
}

pub fn projects(path: &Path, only_running: bool) -> Result<()> {
    let log = Log::load(path)?;
    let mut names: Vec<&str> = log
        .activities()
        .map(|(_, a)| a)
        .filter(|a| !only_running || a.is_running())
        .map(|a| a.project.as_str())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.dedup();
    for n in names {
        println!("{n}");
    }
    Ok(())
}

pub fn search(path: &Path, term: &str, now: NaiveDateTime) -> Result<()> {
    let log = Log::load(path)?;
    let pattern = format!("*{term}*");
    let mut acts: Vec<Activity> = log
        .activities()
        .map(|(_, a)| a)
        .filter(|a| glob_match(&pattern, &a.project) || glob_match(&pattern, &a.description))
        .cloned()
        .collect();
    acts.sort_by_key(|a| a.start);
    if acts.is_empty() {
        println!("No activities match `{term}`.");
    } else {
        print_activities(&acts, false, now);
    }
    Ok(())
}

// ---------- maintenance ----------

pub fn edit(path: &Path, editor: Option<String>) -> Result<()> {
    let editor = editor
        .or_else(|| std::env::var("VISUAL").ok().filter(|e| !e.is_empty()))
        .or_else(|| std::env::var("EDITOR").ok().filter(|e| !e.is_empty()))
        .unwrap_or_else(|| "vi".into());

    if !path.exists() {
        Log::load(path)?.save()?;
    }
    // Run through the shell so editors given with arguments (e.g. "code -w") work.
    let status = process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("tock")
        .arg(path)
        .status()
        .with_context(|| format!("cannot run editor `{editor}`"))?;
    if !status.success() {
        bail!("editor `{editor}` exited with {status}");
    }

    let invalid = Log::load(path)?.invalid().count();
    if invalid > 0 {
        println!("{} {invalid} line(s) could not be parsed; run `tock check` for details.", yellow("warning:"));
    }
    Ok(())
}

pub fn check(path: &Path) -> Result<()> {
    let log = Log::load(path)?;
    let mut bad = 0;
    for (line, text, error) in log.invalid() {
        bad += 1;
        println!("{} {error}", yellow(&format!("line {line}:")));
        println!("    {}", dim(text));
    }
    if bad == 0 {
        println!("All lines are valid.");
        return Ok(());
    }
    process::exit(1);
}

pub fn sanity(path: &Path, now: NaiveDateTime) -> Result<()> {
    let log = Log::load(path)?;
    let mut acts: Vec<(usize, &Activity)> = log.activities().collect();
    acts.sort_by_key(|(_, a)| a.start);

    let mut issues = 0;
    let mut issue = |msg: String| {
        issues += 1;
        println!("{msg}");
    };

    let mut latest: Option<(usize, NaiveDateTime)> = None;
    for &(line, a) in &acts {
        if a.end.is_some_and(|e| e < a.start) {
            issue(format!("{} ends before it starts\n    {}", yellow(&format!("line {line}:")), dim(&a.to_string())));
        }
        if let Some((prev, prev_end)) = latest
            && a.start < prev_end
        {
            issue(format!(
                "{} overlaps line {prev} until {}\n    {}",
                yellow(&format!("line {line}:")),
                prev_end.format(TIME_FORMAT),
                dim(&a.to_string())
            ));
        }
        let end = a.end.unwrap_or(now).max(a.start);
        if latest.is_none_or(|(_, e)| end > e) {
            latest = Some((line, end));
        }
    }

    let running = acts.iter().filter(|(_, a)| a.is_running()).count();
    if running > 1 {
        issue(format!("{} {running} activities are running at once", yellow("note:")));
    }

    if issues == 0 {
        println!("No problems found.");
        return Ok(());
    }
    process::exit(1);
}

// ---------- helpers ----------

fn describe(a: &Activity) -> String {
    if a.description.is_empty() {
        format!("({})", a.project)
    } else {
        format!("\"{}\" ({})", a.description, a.project)
    }
}

/// Distinct project/description pairs, most recently started first.
fn recent_pairs(log: &Log) -> Vec<(String, String)> {
    let mut acts: Vec<&Activity> = log.activities().map(|(_, a)| a).collect();
    acts.sort_by_key(|a| std::cmp::Reverse(a.start));
    let mut pairs: Vec<(String, String)> = Vec::new();
    for a in acts {
        let pair = (a.project.clone(), a.description.clone());
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    pairs
}

fn date_range(f: &Filter, today: NaiveDate) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    if f.today {
        (Some(today), Some(today))
    } else if f.yesterday {
        let y = today - Duration::days(1);
        (Some(y), Some(y))
    } else if f.current_week {
        (Some(monday), Some(monday + Duration::days(6)))
    } else if f.last_week {
        (Some(monday - Duration::days(7)), Some(monday - Duration::days(1)))
    } else if f.month {
        let first = today.with_day(1).unwrap();
        let next = first.checked_add_months(chrono::Months::new(1)).unwrap();
        (Some(first), Some(next - Duration::days(1)))
    } else if let Some(d) = f.date {
        (Some(d), Some(d))
    } else {
        (f.from, f.to)
    }
}

/// Activities matching the filter, rounded if requested, sorted by start.
fn select(log: &Log, f: &Filter, now: NaiveDateTime) -> Vec<Activity> {
    let (from, to) = date_range(f, now.date());
    let mut acts: Vec<Activity> = log
        .activities()
        .map(|(_, a)| a)
        .filter(|a| from.is_none_or(|d| a.start.date() >= d) && to.is_none_or(|d| a.start.date() <= d))
        .filter(|a| f.project.as_deref().is_none_or(|p| glob_match(p, &a.project)))
        .cloned()
        .map(|mut a| {
            if let Some(r) = f.round {
                a.start = display::round_time(a.start, r);
                a.end = a.end.map(|e| display::round_time(e, r));
            }
            a
        })
        .collect();
    acts.sort_by_key(|a| a.start);
    acts
}

fn print_activities(acts: &[Activity], grouped: bool, now: NaiveDateTime) {
    let header = ["Start", "End", "Description", "Project", "Duration"];
    let row = |a: &Activity| {
        let start = if grouped { a.start.format("%H:%M") } else { a.start.format(TIME_FORMAT) };
        let end = match a.end {
            None => "running".to_string(),
            Some(e) if e.date() == a.start.date() => e.format("%H:%M").to_string(),
            Some(e) => e.format(TIME_FORMAT).to_string(),
        };
        vec![start.to_string(), end, a.description.clone(), a.project.clone(), duration(a.duration(now))]
    };

    if !grouped {
        display::print_table(&header, &acts.iter().map(row).collect::<Vec<_>>(), true);
        return;
    }

    let mut all = vec![header.iter().map(|s| s.to_string()).collect::<Vec<_>>()];
    all.extend(acts.iter().map(row));
    let widths = display::column_widths(&all);
    println!("{}", bold(&display::render_row(&all[0], &widths, true)));

    let mut i = 0;
    while i < acts.len() {
        let day = acts[i].start.date();
        let n = acts[i..].iter().take_while(|a| a.start.date() == day).count();
        let total: Duration = acts[i..i + n].iter().map(|a| a.duration(now)).sum();
        println!();
        println!("{}  {}", bold(&day.format("%A, %Y-%m-%d").to_string()), dim(&duration(total)));
        for r in &all[i + 1..i + 1 + n] {
            println!("{}", display::render_row(r, &widths, true));
        }
        i += n;
    }
}
