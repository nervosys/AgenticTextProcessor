# Query (AQL)

Execute queries using the Agentic Query Language (AQL).

## Usage

```bash
atp query '<AQL_EXPRESSION>' [PATHS...] [OPTIONS]
```

## Options

| Flag          | Description                             |
| ------------- | --------------------------------------- |
| `--apply`     | Apply changes for replace/delete stages |
| `--explain`   | Show execution plan without running     |
| `--recursive` | Process directories recursively         |

## Examples

```bash
# Find pattern
atp query 'find "TODO"' src/

# Case-insensitive find
atp query 'find "error" ignore_case' logs/

# Pipeline: find → sort → unique → count
atp query 'find "import" | sort | unique | count' src/

# Replace
atp query 'replace "old_api" with "new_api"' src/ --apply

# Filter and select
atp query 'find "," | select 1, 3 | sort by field 1' data.csv
```

See [AQL Language](../aql.md) for full syntax reference.
