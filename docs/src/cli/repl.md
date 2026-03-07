# REPL

Interactive AQL shell for exploratory text processing.

## Usage

```bash
atp repl [PATHS...]
```

## Features

- Tab completion for AQL keywords
- Command history (persisted across sessions)
- Multi-line input with `\` continuation
- Colored output with syntax highlighting
- Inline help with `help` or `?`

## Commands

| Command | Description                            |
| ------- | -------------------------------------- |
| `help`  | Show available commands and AQL syntax |
| `exit`  | Exit the REPL                          |
| `clear` | Clear the screen                       |
| Any AQL | Execute an AQL query                   |

## Example Session

```
atp> find "TODO"
Found 3 matches in 2 files

atp> find "TODO" | sort | unique
Found 2 unique matches

atp> find "error" ignore_case | count
Count: 42

atp> help
Available AQL stages: find, replace, delete, ...
```
