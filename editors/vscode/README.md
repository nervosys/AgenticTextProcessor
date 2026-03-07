# ATP — AQL Language Support for VS Code

Provides syntax highlighting and LSP (Language Server Protocol) support for
**AQL** (ATP Query Language) files (`.aql`).

## Features

- **Syntax highlighting** — keywords, operators, strings, regex, numbers,
  control flow (`let`, `if/then/else/end`, `def/call`, `group by`).
- **Language Server** — completions, hover documentation, diagnostics, and
  formatting via the `atp-lsp` binary.
- **File association** — `.aql` files auto-detected.

## Requirements

- Install `atp` (`cargo install --path src/atp-cli`) to get the `atp-lsp` binary.
- Or set `atp.lspPath` in VS Code settings to the full path.

## Extension Settings

| Setting            | Default     | Description                         |
| ------------------ | ----------- | ----------------------------------- |
| `atp.lspPath`      | `"atp-lsp"` | Path to the atp-lsp binary          |
| `atp.enableLsp`    | `true`      | Enable the AQL Language Server      |
| `atp.trace.server` | `"off"`     | Trace communication with the server |

## Example `.aql` file

```aql
# Find all TODO comments, sorted and de-duplicated
find "TODO" ignore_case | sort | unique | count
```
