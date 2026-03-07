# CI/CD

ATP uses GitHub Actions for continuous integration and release automation.

## CI Pipeline (`.github/workflows/ci.yml`)

Runs on every push to `main`/`develop` and on pull requests to `main`.

### Jobs

| Job        | Description                             |
| ---------- | --------------------------------------- |
| `check`    | `cargo check --workspace`               |
| `test`     | Test matrix: Ubuntu, Windows, macOS     |
| `fmt`      | `cargo fmt --check`                     |
| `clippy`   | `cargo clippy -- -D warnings`           |
| `doc`      | `cargo doc --no-deps`                   |
| `wasm`     | Build for `wasm32-unknown-unknown`      |
| `msrv`     | Verify MSRV (Rust 1.75.0)               |
| `security` | `cargo audit` for known vulnerabilities |

## Release Pipeline (`.github/workflows/release.yml`)

Triggered by version tags (`v*.*.*`).

### Build Matrix

| Target                      | OS      |
| --------------------------- | ------- |
| `x86_64-unknown-linux-gnu`  | Ubuntu  |
| `x86_64-unknown-linux-musl` | Ubuntu  |
| `aarch64-unknown-linux-gnu` | Ubuntu  |
| `x86_64-pc-windows-msvc`    | Windows |
| `aarch64-pc-windows-msvc`   | Windows |
| `x86_64-apple-darwin`       | macOS   |
| `aarch64-apple-darwin`      | macOS   |

### Artifacts

Each target produces:
- `atp` — main CLI binary
- `atp-grep` — grep compatibility binary
- `atp-sed` — sed compatibility binary
- `atp-awk` — awk compatibility binary
- `atp-lsp` — LSP server binary

All binaries are compressed and uploaded to GitHub Releases.

### WASM Release

A separate job builds the WASM package with `wasm-pack` and attaches `atp-wasm.tar.gz` to the release.

## Publishing to crates.io

```bash
# Publish in dependency order
cargo publish -p atp-core
cargo publish -p atp-cli
cargo publish -p atp-tui
cargo publish -p atp-gui
cargo publish -p atp-wasm
cargo publish -p atp-lsp
```
