# Contributing

## Development Setup

```bash
git clone https://github.com/nervosys/AgenticTextProcessor
cd AgenticTextProcessor
cargo build
cargo test --workspace
```

## Code Style

- `cargo fmt` — Format all code
- `cargo clippy` — Lint all code
- MSRV: Rust 1.75.0

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific crate tests
cargo test -p atp-core
cargo test -p atp-lsp

# Run integration tests
cargo test -p atp-core --test integration

# Run benchmarks
cargo bench -p atp-core
```

## Project Structure

| Crate      | Purpose                                |
| ---------- | -------------------------------------- |
| `atp-core` | Core library — engines, AQL, telemetry |
| `atp-cli`  | CLI binary and subcommand handlers     |
| `atp-tui`  | Terminal UI                            |
| `atp-gui`  | Desktop GUI                            |
| `atp-wasm` | WebAssembly bindings                   |
| `atp-lsp`  | LSP server for AQL files               |

## Adding a New Engine Feature

1. Implement the feature in `atp-core/src/engine/`
2. Add output types to `atp-core/src/output.rs` if needed
3. Expose via CLI in `atp-cli/src/commands/`
4. Add WASM binding in `atp-wasm/src/lib.rs` if applicable
5. Write unit tests in the module
6. Add integration tests in `atp-core/tests/integration.rs`
7. Add benchmarks in `atp-core/benches/engines.rs`

## Adding an AQL Stage

1. Add the variant to `AqlStage` enum in `atp-core/src/engine/aql.rs`
2. Implement parsing in the `parse_stage` function
3. Implement execution in the `execute_stage` function
4. Add `describe()` output for `--explain`
5. Add LSP keyword to `atp-lsp/src/server.rs`
6. Add tests

## License

AGPL-3.0-only — see [LICENSE](https://github.com/nervosys/AgenticTextProcessor/blob/master/LICENSE). Commercial licensing available — contact [opensource@nervosys.ai](mailto:opensource@nervosys.ai).
