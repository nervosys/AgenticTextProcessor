# LSP Server

The `atp-lsp` crate provides a Language Server Protocol (LSP) server for AQL files.

## Installation

```bash
cargo install --path src/atp-lsp
```

## Features

| Feature         | Description                             |
| --------------- | --------------------------------------- |
| **Completion**  | 27 AQL keywords with descriptions       |
| **Hover**       | Documentation for AQL keywords on hover |
| **Diagnostics** | Real-time AQL syntax validation         |
| **Formatting**  | Pipe normalization and cleanup          |

## VS Code Integration

Add to your VS Code `settings.json`:

```json
{
  "aql.lsp.path": "atp-lsp",
  "aql.lsp.args": []
}
```

Or create a `.vscode/settings.json` in your project:

```json
{
  "[aql]": {
    "editor.formatOnSave": true
  }
}
```

## Protocol

The LSP server communicates over stdin/stdout using the JSON-RPC protocol with standard LSP Content-Length headers.

### Supported Methods

| Method                            | Direction                    |
| --------------------------------- | ---------------------------- |
| `initialize`                      | Request                      |
| `initialized`                     | Notification                 |
| `shutdown` / `exit`               | Request/Notification         |
| `textDocument/didOpen`            | Notification                 |
| `textDocument/didChange`          | Notification                 |
| `textDocument/didClose`           | Notification                 |
| `textDocument/completion`         | Request                      |
| `textDocument/hover`              | Request                      |
| `textDocument/formatting`         | Request                      |
| `textDocument/publishDiagnostics` | Notification (server→client) |

### Completion Keywords

The server provides completions for all AQL keywords: `find`, `replace`, `with`, `delete`, `insert`, `before`, `after`, `lines`, `matching`, `select`, `fields`, `filter`, `sort`, `by`, `field`, `unique`, `take`, `first`, `skip`, `last`, `count`, `set`, `separator`, `ignore_case`, `whole_word`, `invert`, `all`.
