# Analyze (awk)

Analyze structured and semi-structured data with field extraction, filtering, and aggregation.

## Usage

```bash
atp analyze [PATHS...] [OPTIONS]
```

## Options

| Flag                 | Description                             |
| -------------------- | --------------------------------------- |
| `--separator <SEP>`  | Field separator (default: whitespace)   |
| `-f, --fields <F>`   | Fields to select (1-indexed, comma-sep) |
| `--header`           | First line is a header row              |
| `--condition <COND>` | Filter condition (e.g., `$3 > 100`)     |
| `--agg <FUNC>`       | Aggregation: count, sum, avg, min, max  |
| `--recursive`        | Process directories recursively         |

## Examples

```bash
# Basic CSV analysis
atp analyze data.csv --separator ','

# Select fields 1 and 3
atp analyze data.csv -f 1,3 --separator ','

# Filter rows
atp analyze data.csv --separator ',' --condition '$2 > 25'

# Count records
atp analyze data.csv --separator ',' --agg count

# With header row
atp analyze data.csv --separator ',' --header

# JSON output
atp analyze data.csv --separator ',' --format json
```

## Output Schema

```json
{
  "total_records": 100,
  "matched_records": 42,
  "aggregations": {
    "count": 42,
    "sum": 1234.5
  },
  "records": [
    {
      "fields": ["Alice", "30", "NYC"],
      "source_file": "data.csv",
      "source_line": 2
    }
  ]
}
```
