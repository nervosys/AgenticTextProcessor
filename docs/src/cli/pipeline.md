# Pipeline

Run multi-stage processing pipelines defined in YAML or JSON.

## Usage

```bash
atp pipeline <DEFINITION_FILE> [PATHS...] [OPTIONS]
```

## Pipeline Definition (YAML)

```yaml
name: "Clean and count TODOs"
stages:
  - type: search
    pattern: "TODO|FIXME|HACK"
    case_sensitive: false
  - type: sort
  - type: unique
  - type: count
```

## Pipeline Definition (JSON)

```json
{
  "name": "Clean and count TODOs",
  "stages": [
    { "type": "search", "pattern": "TODO|FIXME|HACK", "case_sensitive": false },
    { "type": "sort" },
    { "type": "unique" },
    { "type": "count" }
  ]
}
```

## DSL Syntax

Pipelines can also be defined inline using the DSL:

```bash
# Arrow syntax
atp pipeline --dsl "search 'TODO' -> sort -> unique -> count" src/
```

## Output Schema

```json
{
  "stages": [
    {
      "stage_index": 0,
      "stage_type": "search",
      "duration_ms": 12,
      "records_in": 1000,
      "records_out": 42
    }
  ],
  "final_output": [...],
  "total_duration_ms": 25
}
```
