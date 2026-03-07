# Backward Compatibility

ATP provides drop-in replacement binaries for traditional Unix text tools.

## Binaries

| Binary     | Replaces | Description                             |
| ---------- | -------- | --------------------------------------- |
| `atp-grep` | `grep`   | Pattern matching with structured output |
| `atp-sed`  | `sed`    | Stream editing with dry-run default     |
| `atp-awk`  | `awk`    | Data analysis with typed output         |

## atp-grep

Accepts standard grep flags:

```bash
atp-grep -i "pattern" file.txt          # case-insensitive
atp-grep -r "TODO" src/                 # recursive
atp-grep -c "error" logs/*.log          # count matches
atp-grep -l "TODO" src/                 # files with matches only
atp-grep -n "pattern" file.txt          # line numbers
atp-grep -v "debug" file.txt            # invert match
atp-grep -e "pat1" -e "pat2" file.txt   # multiple patterns
atp-grep -A3 -B3 "error" file.txt       # context lines
```

## atp-sed

Accepts standard sed expression syntax:

```bash
atp-sed 's/old/new/g' file.txt          # substitute
atp-sed -i 's/old/new/g' file.txt       # in-place
atp-sed -e 's/a/b/' -e 's/c/d/' f.txt   # multiple expressions
atp-sed '/pattern/d' file.txt           # delete matching lines
```

## atp-awk

Accepts standard awk flags:

```bash
atp-awk -F',' '{print $1, $3}' data.csv    # field separator
atp-awk '/pattern/ {print $0}' file.txt     # pattern + action
atp-awk 'BEGIN {FS=","} {print $1}' f.csv   # BEGIN block
```

## Key Differences

All backward-compatible binaries add:

- **Structured output** with `--format json/yaml/csv`
- **Dry-run by default** for sed operations (use `--apply` or `-i`)
- **Provenance tracking** in JSON output
- **Better error messages** with suggestions
