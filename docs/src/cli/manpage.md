# Man Pages

Generate Unix man pages for ATP commands.

## Usage

```bash
# Print man page to stdout
atp manpage

# Generate man pages to a directory
atp manpage --dir /usr/local/share/man/man1
```

## Options

| Flag           | Description                         |
| -------------- | ----------------------------------- |
| `--dir <PATH>` | Output directory for man page files |

When `--dir` is specified, man pages are generated for `atp` and all subcommands. Without `--dir`, the main `atp` man page is printed to stdout for piping to `man -l -`.
