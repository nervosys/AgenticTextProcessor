# AQL — ATP Query Language

AQL is a unified, composable syntax for text processing, jointly optimized for AI agents and humans.

AQL replaces the fragmented regex-heavy syntaxes of `grep`, `sed`, and `awk` with a single keyword-based language. Every query is readable English, unambiguous to parse, and trivially composable via `|` pipes.

## Why AQL?

| Issue                   | grep / sed / awk                           | AQL                                           |
| ----------------------- | ------------------------------------------ | --------------------------------------------- |
| **Delimiter confusion** | `sed 's/path/to/file/new/'`                | `replace "path/to/file" with "new"`           |
| **Flag overloading**    | `-i` means different things in sed vs grep | `ignore_case` always means one thing          |
| **Escaping hell**       | Shell + regex compound escaping            | Double-quote strings; regex opt-in with `/…/` |
| **Silent failures**     | May silently produce nothing               | Parser rejects with clear error messages      |
| **Multi-syntax**        | Three different syntaxes to learn          | One syntax for everything                     |

## Quick Reference

```bash
# Literal search
atp query 'find "hello world"'

# Regex search
atp query 'find /fn\s+\w+/'

# Case-insensitive
atp query 'find "error" ignore_case'

# Global replace
atp query 'replace "old" with "new" all'

# Pipeline
atp query 'find "TODO" | sort | unique | count'

# Field processing
atp query 'set separator "," | select fields 1, 3 | sort by field 1'

# Explain
atp query --explain 'find "error" | count'
```

See [Syntax Reference](./aql/syntax.md), [Stages](./aql/stages.md), [Modifiers](./aql/modifiers.md), and [Examples](./aql/examples.md).
