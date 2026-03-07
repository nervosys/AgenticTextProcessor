# Watch Mode

Monitor files for changes and automatically re-run commands.

## Usage

```bash
atp watch <COMMAND> [PATHS...] [OPTIONS]
```

## Options

| Flag              | Description                       |
| ----------------- | --------------------------------- |
| `--debounce <MS>` | Debounce interval in milliseconds |
| `--recursive`     | Watch directories recursively     |

## Examples

```bash
# Watch for TODO changes
atp watch 'search "TODO"' src/

# Watch CSV file for analysis
atp watch 'analyze --separator ","' data.csv

# Watch with custom debounce
atp watch 'query "find \"error\" | count"' logs/ --debounce 1000
```

The watch command uses the `notify` crate for efficient filesystem monitoring across all platforms.
