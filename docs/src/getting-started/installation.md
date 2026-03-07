# Installation

## From Source

```bash
# Clone the repository
git clone https://github.com/nervosys/AgenticTextProcessor
cd AgenticTextProcessor

# Build all binaries
cargo build --release

# Install CLI
cargo install --path atp-cli

# Install TUI
cargo install --path atp-tui

# Install GUI
cargo install --path atp-gui

# Install LSP server
cargo install --path atp-lsp
```

Binaries produced: `atp`, `atp-grep`, `atp-sed`, `atp-awk`, `atp-tui`, `atp-gui`, `atp-lsp`.

## Shell Completions

Generate completions for your shell:

```bash
# Bash
atp completions bash > ~/.local/share/bash-completion/completions/atp

# Zsh
atp completions zsh > ~/.zfunc/_atp

# Fish
atp completions fish > ~/.config/fish/completions/atp.fish

# PowerShell
atp completions powershell > $PROFILE.CurrentUserAllHosts
```

## Man Pages

Generate man pages:

```bash
# Single page to stdout
atp manpage

# All pages to a directory
atp manpage --dir /usr/local/share/man/man1
```

## WASM (Browser)

```bash
# Build the WASM package
cd atp-wasm
wasm-pack build --target web
```

## Requirements

- Rust 1.75.0+ (MSRV)
- For GUI: platform-specific graphics backend (OpenGL/Vulkan/Metal)
