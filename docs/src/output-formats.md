# Output Formats

ATP supports six output formats, selectable via `--format`:

## JSON (`--format json`)

Compact JSON. Ideal for machine parsing.

```json
{"atp_version":"0.1.0","command":"search","data":{"total_matches":3},"metadata":{}}
```

## JSON Pretty (`--format json-pretty`)

Human-readable JSON with indentation.

```json
{
  "atp_version": "0.1.0",
  "command": "search",
  "data": {
    "total_matches": 3
  }
}
```

## JSONL (`--format jsonl`)

One JSON object per line. Ideal for streaming and piping.

```
{"file":"a.rs","line":10,"match":"TODO: fix this"}
{"file":"b.rs","line":25,"match":"TODO: refactor"}
```

## YAML (`--format yaml`)

YAML output. Good for configuration and human reading.

```yaml
atp_version: "0.1.0"
command: search
data:
  total_matches: 3
```

## CSV (`--format csv`)

Comma-separated values. Compatible with spreadsheets and data tools.

```
file,line_number,matched_text
src/main.rs,42,TODO
src/lib.rs,10,TODO
```

## Human (`--format human`)

Default terminal display with colors and formatting. Not intended for machine parsing.

```
src/main.rs:42: // TODO: fix this
src/lib.rs:10:  // TODO: refactor
```

## Envelope Schema

All structured formats (JSON, YAML) wrap results in the ATP envelope:

```json
{
  "$schema": "https://atp.nervosys.com/schemas/v0.1.0/<command>.json",
  "atp_version": "0.1.0",
  "command": "<command>",
  "timestamp": "<ISO 8601>",
  "data": { ... },
  "metadata": {
    "duration_ms": 12,
    "provenance": {
      "command_line": "atp search TODO src/",
      "working_directory": "/project",
      "input_hash": "sha256:..."
    }
  }
}
```
