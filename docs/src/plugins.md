# Plugin System

ATP supports stage and format plugins loaded from TOML manifests.

## Overview

Plugins extend ATP with:

- **Stage plugins**: Custom processing stages for AQL pipelines
- **Format plugins**: Custom output format templates

Plugins are discovered from `~/.atp/plugins/` (or `%USERPROFILE%\.atp\plugins\` on Windows).

## Quick Start

1. Create a plugin manifest (TOML file)
2. Place it in the plugin directory
3. Use it in AQL queries or format output

```bash
# List installed plugins
atp plugins

# Use a format plugin
atp search "TODO" src/ --format my-custom-format
```

See [Writing Plugins](./plugins/writing.md) and [Plugin Manifest](./plugins/manifest.md).
