# Search (grep)

Search files for patterns using regular expressions, literal strings, or glob patterns.

## Usage

```bash
atp search <PATTERN> [PATHS...] [OPTIONS]
```

## Options

| Flag                    | Description                            |
| ----------------------- | -------------------------------------- |
| `-i, --ignore-case`     | Case-insensitive matching              |
| `-L, --literal`         | Treat pattern as literal string        |
| `-g, --glob`            | Treat pattern as glob pattern          |
| `-v, --invert`          | Invert match (show non-matching lines) |
| `-c, --context <N>`     | Lines of context around matches        |
| `-n, --max-matches <N>` | Maximum number of matches              |
| `--extra-patterns <P>`  | Additional patterns (OR-combined)      |
| `--recursive`           | Search directories recursively         |
| `--include <GLOB>`      | Only search files matching glob        |
| `--exclude <GLOB>`      | Skip files matching glob               |

## Examples

```bash
# Basic search
atp search "TODO" src/

# Case-insensitive with context
atp search -i "error" logs/ --context 3

# Literal string search
atp search -L "foo.bar()" src/

# JSON output
atp search "TODO" . --format json

# Multiple patterns
atp search "TODO" src/ --extra-patterns "FIXME,HACK"
```

## Output Schema

```json
{
  "pattern": "TODO",
  "pattern_type": "Regex",
  "case_sensitive": true,
  "total_matches": 5,
  "files_with_matches": 3,
  "matches": [
    {
      "file": "src/main.rs",
      "line_number": 42,
      "column_start": 8,
      "column_end": 12,
      "line_content": "    // TODO: refactor this",
      "matched_text": "TODO",
      "context_before": [],
      "context_after": [],
      "byte_offset": 1024
    }
  ]
}
```
