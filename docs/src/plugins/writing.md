# Writing Plugins

## Stage Plugins

Stage plugins add custom processing stages to AQL pipelines.

### Transform Types

| Type        | Description                          |
| ----------- | ------------------------------------ |
| `replace`   | Regex substitution on record content |
| `filter`    | Keep records matching a pattern      |
| `sort`      | Sort records by content or field     |
| `aggregate` | Reduce records to a summary          |
| `shell`     | Pipe records through a shell command |

### Example: Replace Stage Plugin

```toml
[plugin]
name = "normalize-whitespace"
version = "1.0.0"
description = "Normalize all whitespace to single spaces"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "replace"
pattern = "\\s+"
replacement = " "
global = true
```

### Example: Filter Stage Plugin

```toml
[plugin]
name = "only-errors"
version = "1.0.0"
description = "Keep only lines containing ERROR"
kind = "stage"

[stage]
input = "lines"
output = "lines"

[stage.transform]
type = "filter"
pattern = "ERROR"
```

## Format Plugins

Format plugins define custom output templates.

### Example: Format Plugin

```toml
[plugin]
name = "github-markdown"
version = "1.0.0"
description = "Output matches as GitHub markdown checkboxes"
kind = "format"

[format]
name = "github-md"
template = "- [ ] {file}:{line}: {content}"
header = "# Matches\n"
footer = "\n---\nTotal: {count} matches"
separator = "\t"
```

Use with: `atp search "TODO" src/ --format github-md`
