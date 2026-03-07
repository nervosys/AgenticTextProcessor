# AQL Examples

## Code Maintenance

```bash
# Find all TODOs and count by file
atp query 'find "TODO" | count' src/

# Find unused imports (lines with "import" that appear only once)
atp query 'find /^import\s/ | sort | unique' src/

# Replace deprecated API calls
atp query 'replace "old_api()" with "new_api()" all' src/ --apply

# Delete commented-out code
atp query 'delete /^\s*\/\//' src/ --apply
```

## Log Analysis

```bash
# Find errors, case-insensitive, last 100
atp query 'find "error" ignore_case | last 100' /var/log/app.log

# Count errors by type
atp query 'find "ERROR" | sort | unique' app.log

# Extract timestamps from error lines
atp query 'find "ERROR" | select fields 1, 2' app.log
```

## CSV Processing

```bash
# Set separator and select columns
atp query 'set separator "," | select fields 1, 3, 5' data.csv

# Filter rows where field 2 > 100
atp query 'set separator "," | filter field 2 > 100' data.csv

# Sort by field 3 descending
atp query 'set separator "," | sort by field 3 desc numeric' data.csv

# Unique values in field 1
atp query 'set separator "," | select fields 1 | unique' data.csv

# Average of field 2
atp query 'set separator "," | avg field 2' data.csv
```

## Multi-Stage Pipelines

```bash
# Complex analysis: find errors, extract fields, sort, deduplicate, top 10
atp query 'find "ERROR" | set separator " " | select fields 3, 5 | sort by field 1 | unique | take 10' logs/

# Clean and normalize: delete blanks, replace tabs, sort, unique
atp query 'delete /^\s*$/ | replace "\t" with " " all | sort | unique' data.txt
```
