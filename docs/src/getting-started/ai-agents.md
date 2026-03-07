# For AI Agents

ATP is designed for AI agent integration. Every command produces structured, schema-validated output.

## Discovery Protocol

```bash
# 1. Discover all capabilities (always start here)
atp ontology --format json

# 2. Get schema for a specific command
atp ontology -c search --format json

# 3. List all available commands
atp ontology --list
```

The ontology endpoint returns a complete machine-readable description of ATP's capabilities, including input/output schemas, supported operations, and examples.

## Structured Output

All commands support `--format` with these options:

| Format        | Flag                   | Use Case                   |
| ------------- | ---------------------- | -------------------------- |
| JSON          | `--format json`        | Machine parsing            |
| JSON (pretty) | `--format json-pretty` | Human-readable JSON        |
| JSONL         | `--format jsonl`       | Streaming / line-by-line   |
| YAML          | `--format yaml`        | Configuration files        |
| CSV           | `--format csv`         | Tabular data               |
| Human         | `--format human`       | Terminal display (default) |

## Output Envelope

Every JSON/YAML response follows the ATP envelope schema:

```json
{
  "$schema": "https://atp.nervosys.com/schemas/v0.1.0/search.json",
  "atp_version": "0.1.0",
  "command": "search",
  "timestamp": "2025-01-01T00:00:00Z",
  "data": { ... },
  "metadata": {
    "provenance": {
      "command_line": "atp search TODO src/",
      "working_directory": "/project"
    }
  }
}
```

## Error Handling

Errors are returned as typed JSON with suggestions:

```json
{
  "error": {
    "code": "PATTERN_INVALID",
    "message": "Invalid regex: unclosed group",
    "suggestion": "Check for unmatched parentheses"
  }
}
```

## Dry-Run by Default

Transform operations default to dry-run mode. Use `--apply` to execute changes. This prevents agents from accidentally modifying files.

## Provenance Tracking

Every operation records full provenance — what was done, to which files, with which parameters — enabling audit trails and reproducibility.
