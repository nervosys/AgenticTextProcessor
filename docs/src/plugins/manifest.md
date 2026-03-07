# Plugin Manifest

Plugin manifests are TOML files with the following structure.

## Required Fields

```toml
[plugin]
name = "my-plugin"           # Unique plugin name
version = "1.0.0"            # SemVer version
description = "What it does" # Human-readable description
kind = "stage"               # "stage" or "format"
```

## Optional Fields

```toml
[plugin]
author = "Your Name"         # Plugin author
license = "MIT"              # License identifier
```

## Stage Configuration

Required when `kind = "stage"`:

```toml
[stage]
input = "lines"              # "lines", "records", or "text"
output = "lines"             # "lines", "records", or "text"

[stage.transform]
type = "replace"             # "replace", "filter", "sort", "aggregate", "shell"
# Additional fields depend on transform type
```

### Replace Transform

```toml
[stage.transform]
type = "replace"
pattern = "\\bfoo\\b"
replacement = "bar"
global = true
```

### Filter Transform

```toml
[stage.transform]
type = "filter"
pattern = "^ERROR"
```

### Sort Transform

```toml
[stage.transform]
type = "sort"
field = 2          # Optional: sort by field index
descending = true  # Optional: reverse order
```

### Aggregate Transform

```toml
[stage.transform]
type = "aggregate"
fields = ["count", "sum"]
```

### Shell Transform

```toml
[stage.transform]
type = "shell"
command = "tr '[:lower:]' '[:upper:]'"
```

## Format Configuration

Required when `kind = "format"`:

```toml
[format]
name = "my-format"           # Format name (used with --format)
template = "{file}:{line}"   # Template for each record
header = "Results:\n"        # Optional header
footer = "\nDone."           # Optional footer
separator = "\t"             # Field separator for interpolation
```
