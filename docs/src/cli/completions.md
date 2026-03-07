# Shell Completions

Generate shell completion scripts for ATP.

## Usage

```bash
atp completions <SHELL>
```

## Supported Shells

| Shell      | Command                      |
| ---------- | ---------------------------- |
| Bash       | `atp completions bash`       |
| Zsh        | `atp completions zsh`        |
| Fish       | `atp completions fish`       |
| PowerShell | `atp completions powershell` |
| Elvish     | `atp completions elvish`     |

## Installation

### Bash

```bash
atp completions bash > ~/.local/share/bash-completion/completions/atp
```

### Zsh

```bash
atp completions zsh > ~/.zfunc/_atp
# Add to .zshrc: fpath+=~/.zfunc; autoload -Uz compinit; compinit
```

### Fish

```bash
atp completions fish > ~/.config/fish/completions/atp.fish
```

### PowerShell

```powershell
atp completions powershell | Out-String | Invoke-Expression
# Or add to $PROFILE for persistence
```
