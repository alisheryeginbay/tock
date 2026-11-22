# tock

A small command-line time tracker. Every activity is one line in a plain-text
log you can read, grep and edit by hand.

```
2026-11-22 09:00 - 2026-11-22 10:30 | acme | API design | billable, meeting
2026-11-22 10:30 | internal | Code review
```

Each activity has one project, an optional description and optional tags.
Projects split your time into totals that add up to the overall total. Tags are
labels that cut across projects, like `billable` or `meeting`. Tags are stored
in lowercase and can't contain spaces, commas or pipes.

A line without an end time is a running activity. Write a literal `|` or `\`
inside a field as `\|` or `\\`. Blank lines and lines starting with `#` are
ignored. Lines that can't be parsed are kept as they are, and `tock check`
reports them.

## Install

```sh
cargo install --path .
```

## Log location

`--file PATH`, then `$TOCK_FILE`, then `$XDG_DATA_HOME/tock/log.tock`
(by default `~/.local/share/tock/log.tock`). `tock path` prints the file in use.

## Usage

```sh
tock start -p acme -d "API design"     # start (stops whatever is running)
tock start -p acme -d "API design" -t 9:15 -g billable -g meeting
tock stop                              # or: tock stop -t 17:30
tock change -d "API review"            # edit the running activity (-p/-d/-t)
tock change -g focus --untag meeting   # add or remove tags
tock cancel                            # discard the running activity
tock current                           # what's running

tock last                              # recent project/description pairs
tock continue 2 -g urgent              # restart pair #2 (default 0), keeping its tags

tock status                            # running activity + day/week/month totals
tock list --today                      # grouped by day; --no-grouping, -n N
tock report --last-week -p 'acme*'     # time per project and description
tock report --from 2026-11-01 --to 2026-11-15 --round 15m
tock report --by-tag --month           # time per tag, then per project

tock projects [-c]                     # project names (-c: only running)
tock tags [-c]                         # tag names
tock search 'meet*'                    # match project, description or tag
tock edit [-e "code -w"]               # open the log in $VISUAL/$EDITOR
tock check                             # unparseable lines
tock sanity                            # negative durations and overlaps
tock completions zsh > ~/.zfunc/_tock  # bash, zsh, fish, elvish, powershell
```

Date filters for `list` and `report`: `--today`, `--yesterday`,
`--current-week`, `--last-week`, `--month`, `-d DATE`, or `--from`/`--to`
(inclusive). `-p PATTERN` filters by project and `-g PATTERN` by tag
(`list`, `report` and `status`). `-g` can be repeated, and an activity must
match every tag given. Patterns accept `*` and `?` and ignore case.

`report --by-tag` counts an activity under each of its tags, so per-tag totals
can add up to more than the overall total.

`--round` changes only the output, never the log.

`check` and `sanity` exit with status 1 when they find problems. Colors are
turned off when output isn't a terminal or when `NO_COLOR` is set.
