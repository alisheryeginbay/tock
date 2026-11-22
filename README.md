# tock

A small command-line time tracker. Every activity is one line in a plain-text
log you can read, grep and edit by hand.

```
2026-11-22 09:00 - 2026-11-22 10:30 | acme | API design
2026-11-22 10:30 | internal | Code review
```

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
tock start -p acme -d "API design" -t 9:15
tock stop                              # or: tock stop -t 17:30
tock change -d "API review"            # edit the running activity (-p/-d/-t)
tock cancel                            # discard the running activity
tock current                           # what's running

tock last                              # recent project/description pairs
tock continue 2                        # restart pair #2 (default 0)

tock status                            # running activity + day/week/month totals
tock list --today                      # grouped by day; --no-grouping, -n N
tock report --last-week -p 'acme*'     # time per project and description
tock report --from 2026-11-01 --to 2026-11-15 --round 15m

tock projects [-c]                     # project names (-c: only running)
tock search 'meet*'                    # match project or description
tock edit [-e "code -w"]               # open the log in $VISUAL/$EDITOR
tock check                             # unparseable lines
tock sanity                            # negative durations and overlaps
tock completions zsh > ~/.zfunc/_tock  # bash, zsh, fish, elvish, powershell
```

Date filters for `list` and `report`: `--today`, `--yesterday`,
`--current-week`, `--last-week`, `--month`, `-d DATE`, or `--from`/`--to`
(inclusive). Project patterns accept `*` and `?` and ignore case.
`--round` changes only the output, never the log.

`check` and `sanity` exit with status 1 when they find problems. Colors are
turned off when output isn't a terminal or when `NO_COLOR` is set.
