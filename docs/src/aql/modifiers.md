# AQL Modifiers

Modifiers alter the behavior of search and transform stages.

## Available Modifiers

| Modifier         | Applies To    | Description                              |
| ---------------- | ------------- | ---------------------------------------- |
| `ignore_case`    | find, replace | Case-insensitive matching                |
| `whole_word`     | find, replace | Match whole words only                   |
| `invert`         | find          | Invert match (non-matching lines)        |
| `all` / `global` | replace       | Replace all occurrences (not just first) |
| `multiline`      | find, replace | `^`/`$` match line boundaries            |

## Examples

```
# Case-insensitive search
find "error" ignore_case

# Whole word match (won't match "errors" or "error_code")
find "error" whole_word

# Inverted match (lines NOT containing "debug")
find "debug" invert

# Global replace
replace "foo" with "bar" all

# Combined modifiers
find "error" ignore_case whole_word

# Replace all, case-insensitive
replace "old" with "new" all ignore_case
```
