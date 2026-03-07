# Quick Start

## Search for a Pattern (grep)

```bash
# Search current directory recursively
atp search "TODO" .

# Case-insensitive, with context
atp search -i "error" src/ --context 3

# JSON output for machine consumption
atp search "TODO" . --format json
```

## Transform Text (sed)

```bash
# Preview substitution (dry-run by default)
atp transform 's/old/new/g' file.txt

# Apply changes
atp transform 's/old/new/g' file.txt --apply

# Multiple files
atp transform 's/foo/bar/g' src/ --recursive
```

## Analyze Data (awk)

```bash
# CSV analysis
atp analyze data.csv --separator ','

# Select specific fields
atp analyze data.csv -f 1,3 --separator ','

# With aggregation
atp analyze data.csv --agg count --separator ','
```

## AQL Query

```bash
# Find lines containing a pattern
atp query 'find "TODO"' src/

# Pipeline: find, sort, deduplicate, take top 10
atp query 'find "error" ignore_case | sort | unique | take 10' logs/

# Replace across files
atp query 'replace "old_api" with "new_api"' src/ --apply
```

## Explain Before Executing

```bash
# See what a command would do
atp explain 'find "TODO" | count' src/

# Validate patterns
atp validate "^[a-z]+$"
```
