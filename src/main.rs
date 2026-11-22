mod activity;
mod commands;
mod display;
mod log;

use std::path::PathBuf;

use anyhow::{bail, Result};
use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
use clap::{ArgGroup, Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

/// A simple command-line time tracker backed by a plain-text log.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Log file [default: $XDG_DATA_HOME/tock/log.tock]
    #[arg(short, long, global = true, env = "TOCK_FILE")]
    file: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start a new activity (stops any running one)
    Start {
        /// Project name
        #[arg(short, long)]
        project: String,
        /// What you are doing
        #[arg(short, long, default_value = "")]
        description: String,
        #[command(flatten)]
        time: TimeArg,
    },
    /// Stop the running activity
    Stop {
        #[command(flatten)]
        time: TimeArg,
    },
    /// Restart a recent activity (see `tock last`)
    #[command(visible_alias = "cont")]
    Continue {
        /// Index from `tock last`
        #[arg(default_value_t = 0)]
        number: usize,
        /// Use a different project
        #[arg(short, long)]
        project: Option<String>,
        /// Use a different description
        #[arg(short, long)]
        description: Option<String>,
        #[command(flatten)]
        time: TimeArg,
    },
    /// Discard the running activity without recording it
    Cancel,
    /// Change the running activity
    #[command(group(ArgGroup::new("changes").required(true).multiple(true)))]
    Change {
        /// New project
        #[arg(short, long, group = "changes")]
        project: Option<String>,
        /// New description
        #[arg(short, long, group = "changes")]
        description: Option<String>,
        /// New start time (HH:MM or "YYYY-MM-DD HH:MM")
        #[arg(short, long, group = "changes", value_parser = parse_time)]
        time: Option<NaiveDateTime>,
    },
    /// Show running activities
    Current,
    /// Show today's, this week's and this month's totals
    Status {
        /// Only count projects matching this pattern (* and ? allowed)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// List activities
    #[command(visible_alias = "ls")]
    List {
        #[command(flatten)]
        filter: Filter,
        /// Only show the last N activities
        #[arg(short, long)]
        number: Option<usize>,
        /// Don't group activities by day
        #[arg(long)]
        no_grouping: bool,
    },
    /// Summarize time per project and description
    Report {
        #[command(flatten)]
        filter: Filter,
    },
    /// Show recently used project/description pairs
    Last {
        /// How many to show
        #[arg(short, long, default_value_t = 10)]
        number: usize,
    },
    /// List project names
    Projects {
        /// Only projects with a running activity
        #[arg(short, long)]
        current: bool,
    },
    /// Find activities by project or description (* and ? allowed)
    Search {
        term: String,
    },
    /// Open the log in an editor
    Edit {
        /// Editor command [default: $VISUAL, $EDITOR or vi]
        #[arg(short, long)]
        editor: Option<String>,
    },
    /// Report lines in the log that can't be parsed
    Check,
    /// Report activities with negative durations or overlaps
    Sanity,
    /// Print the log file location
    Path,
    /// Print a shell completion script
    Completions {
        shell: Shell,
    },
}

#[derive(Args)]
pub struct TimeArg {
    /// Use this time instead of now (HH:MM or "YYYY-MM-DD HH:MM")
    #[arg(short, long, value_parser = parse_time)]
    pub time: Option<NaiveDateTime>,
}

#[derive(Args)]
#[command(group(ArgGroup::new("range").multiple(false)))]
pub struct Filter {
    /// Only today
    #[arg(long, group = "range")]
    pub today: bool,
    /// Only yesterday
    #[arg(long, group = "range")]
    pub yesterday: bool,
    /// Only this week (Mon–Sun)
    #[arg(long, group = "range", alias = "current_week")]
    pub current_week: bool,
    /// Only last week (Mon–Sun)
    #[arg(long, group = "range", alias = "last_week")]
    pub last_week: bool,
    /// Only this month
    #[arg(long, group = "range")]
    pub month: bool,
    /// Only this day (YYYY-MM-DD)
    #[arg(short, long, group = "range")]
    pub date: Option<NaiveDate>,
    /// From this day on (YYYY-MM-DD, inclusive)
    #[arg(long, conflicts_with = "range")]
    pub from: Option<NaiveDate>,
    /// Up to this day (YYYY-MM-DD, inclusive)
    #[arg(long, conflicts_with = "range")]
    pub to: Option<NaiveDate>,
    /// Only projects matching this pattern (* and ? allowed)
    #[arg(short, long)]
    pub project: Option<String>,
    /// Round start and end times for display, e.g. 15m or 1h
    #[arg(long, value_parser = parse_round)]
    pub round: Option<i64>,
}

fn parse_time(s: &str) -> Result<NaiveDateTime, String> {
    let s = s.trim();
    if let Ok(t) = NaiveTime::parse_from_str(s, "%H:%M") {
        return Ok(Local::now().date_naive().and_time(t));
    }
    NaiveDateTime::parse_from_str(s, activity::TIME_FORMAT)
        .map_err(|_| format!("invalid time `{s}` (expected HH:MM or \"YYYY-MM-DD HH:MM\")"))
}

fn parse_round(s: &str) -> Result<i64, String> {
    let err = || format!("invalid rounding `{s}` (expected e.g. 15m or 1h)");
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: i64 = num.parse().map_err(|_| err())?;
    match unit {
        "m" if n > 0 => Ok(n),
        "h" if n > 0 => Ok(n * 60),
        _ => Err(err()),
    }
}

fn default_file() -> Result<PathBuf> {
    let data = match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".local/share"),
            None => bail!("cannot determine log location; set TOCK_FILE or pass --file"),
        },
    };
    Ok(data.join("tock/log.tock"))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {e:#}", display::yellow("error:"));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Command::Completions { shell } = cli.command {
        clap_complete::generate(shell, &mut Cli::command(), "tock", &mut std::io::stdout());
        return Ok(());
    }

    let path = match cli.file {
        Some(p) => p,
        None => default_file()?,
    };
    let now = activity::truncate(Local::now().naive_local());

    use commands as c;
    match cli.command {
        Command::Start { project, description, time } => {
            c::start(&path, project, description, time.time.unwrap_or(now))
        }
        Command::Stop { time } => c::stop(&path, time.time.unwrap_or(now)),
        Command::Continue { number, project, description, time } => {
            c::continue_(&path, number, project, description, time.time.unwrap_or(now))
        }
        Command::Cancel => c::cancel(&path),
        Command::Change { project, description, time } => c::change(&path, project, description, time),
        Command::Current => c::current(&path, now),
        Command::Status { project } => c::status(&path, project.as_deref(), now),
        Command::List { filter, number, no_grouping } => c::list(&path, &filter, number, !no_grouping, now),
        Command::Report { filter } => c::report(&path, &filter, now),
        Command::Last { number } => c::last(&path, number),
        Command::Projects { current } => c::projects(&path, current),
        Command::Search { term } => c::search(&path, &term, now),
        Command::Edit { editor } => c::edit(&path, editor),
        Command::Check => c::check(&path),
        Command::Sanity => c::sanity(&path, now),
        Command::Path => {
            println!("{}", path.display());
            Ok(())
        }
        Command::Completions { .. } => unreachable!(),
    }
}
