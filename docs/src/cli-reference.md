# CLI Reference

ATP provides 15 subcommands accessible via the `atp` binary:

## Core Operations

| Command     | Description                              | Alias      |
| ----------- | ---------------------------------------- | ---------- |
| `search`    | Search files for patterns (grep)         | `atp-grep` |
| `transform` | Transform text with substitutions (sed)  | `atp-sed`  |
| `analyze`   | Analyze structured data (awk)            | `atp-awk`  |
| `query`     | Execute AQL queries                      |            |
| `pipeline`  | Run multi-stage pipelines from YAML/JSON |            |

## Inspection

| Command      | Description                        |
| ------------ | ---------------------------------- |
| `ontology`   | Machine-readable capability schema |
| `explain`    | Preview what a command would do    |
| `validate`   | Check pattern validity             |
| `compliance` | FIPS/CMMC compliance report        |

## Interactive

| Command  | Description                      |
| -------- | -------------------------------- |
| `repl`   | Interactive AQL shell            |
| `watch`  | Watch files and re-run on change |
| `stream` | Process stdin in streaming mode  |

## Tooling

| Command       | Description                |
| ------------- | -------------------------- |
| `completions` | Generate shell completions |
| `manpage`     | Generate man pages         |
| `plugins`     | List installed plugins     |

## Global Flags

```
--format <FORMAT>    Output format: json, json-pretty, jsonl, yaml, csv, human
--no-color           Disable colored output
--verbose            Verbose output
--quiet              Suppress non-essential output
```

See individual command pages for details.
