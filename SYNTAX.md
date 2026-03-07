# AQL — ATP Query Language

**A unified, composable syntax for text processing, jointly optimized for AI agents and humans.**

AQL replaces the fragmented regex-heavy syntaxes of `grep`, `sed`, and `awk` with a single keyword-based language. Every query is readable English, unambiguous to parse, and trivially composable via `|` pipes.

---

## Why AQL?

### The Problem with Regex-Based Syntax

| Issue                    | grep / sed / awk                                              | AQL                                                                    |
| ------------------------ | ------------------------------------------------------------- | ---------------------------------------------------------------------- |
| **Delimiter confusion**  | `sed 's/path/to/file/new/'` — where does the pattern end?     | `replace "path/to/file" with "new"` — unambiguous                      |
| **Flag overloading**     | `-i` means *in-place* in sed, *case-insensitive* in grep      | `ignore_case` always means one thing                                   |
| **Escaping hell**        | Shell-escape + regex-escape = compound errors                 | Double-quote strings; regex opt-in with `/…/`                          |
| **Silent failures**      | `sed 's/[unclosed//'` may silently produce nothing            | Parser rejects with `"Expected closing '/'"`                           |
| **Positional semantics** | Meaning depends on position, not names                        | Keywords declare intent explicitly                                     |
| **Multi-syntax**         | `grep X \| sed 's/a/b/' \| awk '{print $3}'` — three syntaxes | `find "X" \| replace "a" with "b" all \| select fields 3` — one syntax |

### Agent Reliability Proof

AQL achieves **higher reliability** than regex-based syntax for AI agents because:

1. **Deterministic parsing**: Every AQL query has exactly one valid parse tree. Regex-based tools have ambiguous delimiter parsing (e.g., `s/a/b/g` with `/` inside the pattern).

2. **Reduced error surface**: AQL has 0 possible delimiter-confusion errors, 0 flag-overloading errors, and 0 escaping-interaction errors. Classic tools have O(n²) potential escaping errors where n is the number of special characters.

3. **Token efficiency**: AQL expresses equivalent operations in fewer distinct syntax rules (1 language vs 3), reducing LLM token prediction errors:

   | Task                    | Classic (tokens, syntaxes)                                | AQL (tokens, syntaxes)                                           |
   | ----------------------- | --------------------------------------------------------- | ---------------------------------------------------------------- |
   | Case-insensitive search | `grep -i 'pat'` (3 tok, grep syntax)                      | `find "pat" ignore_case` (3 tok, AQL)                            |
   | Global replace          | `sed 's/old/new/g'` (1 tok but 4 sub-parts)               | `replace "old" with "new" all` (5 tok, self-describing)          |
   | Pipeline                | `grep X \| sed 's/a/b/' \| awk '{print $1}'` (3 syntaxes) | `find "X" \| replace "a" with "b" \| select fields 1` (1 syntax) |
   | Complex pipeline        | `grep -i err \| awk -F'                                   | ' '{print $3}' \| sort \| uniq -c \| sort -rn \| head -10`       | `find "err" ignore_case \| set separator "\|" \| select fields 3 \| sort \| unique \| count \| sort desc numeric \| take 10` |

4. **Self-validating**: `atp query --validate` instantly checks syntax. No regex-tool equivalent exists.

5. **Self-explaining**: `atp query --explain` produces natural language descriptions. Agents can verify intent before execution.

---

## Quick Reference

```bash
# Literal search (strings in quotes — no escaping needed)
atp query 'find "hello world"'

# Regex search (opt-in with /…/)
atp query 'find /fn\s+\w+/'

# Case-insensitive search
atp query 'find "error" ignore_case'

# Global replace
atp query 'replace "oldName" with "newName" all'

# Pipeline: find → sort → unique → count
atp query 'find "TODO" | sort | unique | count'

# Field processing: set separator, select, filter, sort
atp query 'set separator "," | select fields 1, 3 | filter field 2 > 100 | sort by field 1 desc numeric'

# Explain without executing
atp query --explain 'find "error" | replace "error" with "warning" all'

# Validate syntax
atp query --validate 'find "X" | sort | take 10'
```

---

## Formal Grammar (EBNF)

```ebnf
pipeline      = stage { "|" stage } ;
stage         = find | replace | delete | insert
              | select | filter | sort | unique
              | take | skip | last | count
              | aggregate | set_separator ;

(* ─── Search & Transform ─── *)
find          = "find" pattern { modifier } ;
replace       = "replace" pattern "with" string { modifier } ;
delete        = "delete" [ "lines" ] [ "matching" ] pattern ;
insert        = "insert" string ( "before" | "after" ) pattern ;

(* ─── Field Processing ─── *)
select        = "select" [ "fields" ] integer { "," integer } ;
filter        = "filter" condition ;
set_separator = "set" "separator" string ;

(* ─── Ordering & Limiting ─── *)
sort          = "sort" [ "by" ( "field" integer | "line" ) ]
                [ "asc" | "desc" ] [ "numeric" ] ;
unique        = "unique" [ "by" "field" integer ] ;
take          = ( "take" | "first" ) integer ;
skip          = "skip" integer ;
last          = ( "last" | "tail" ) integer ;

(* ─── Aggregation ─── *)
count         = "count" [ pattern ] ;
aggregate     = "aggregate" agg_func ;
agg_func      = "count"
              | ( "sum" | "avg" | "min" | "max" | "distinct" | "freq" )
                [ "field" ] integer ;

(* ─── Patterns ─── *)
pattern       = string                   (* literal match *)
              | "/" regex_chars "/"       (* regex match *)
              ;
string        = '"' { char } '"'         (* double-quoted *)
              | "'" { char } "'"         (* single-quoted *)
              ;

(* ─── Modifiers ─── *)
modifier      = "ignore_case" | "whole_word" | "invert"
              | "all" | "global" | "multiline"
              | "context" integer | "max" integer
              | "only_match"
              | "in" ( string | "lines" integer "to" integer | "all" )
              | "between" pattern "and" pattern ;

(* ─── Conditions ─── *)
condition     = simple_cond { ( "and" | "or" ) simple_cond } ;
simple_cond   = "field" integer field_op
              | "line" line_op ;
field_op      = "contains" string
              | "equals" string
              | "matches" ( string | "/" regex_chars "/" )
              | "not_empty"
              | ">" number
              | "<" number
              | "!=" string ;
line_op       = "contains" string
              | "matches" ( string | "/" regex_chars "/" ) ;

(* ─── Primitives ─── *)
integer       = digit { digit } ;
number        = integer [ "." integer ] ;
```

---

## Stage Reference

### `find` — Search for Lines (grep)

```
find PATTERN [MODIFIERS]
```

| Modifier                    | Description                               |
| --------------------------- | ----------------------------------------- |
| `ignore_case`               | Case-insensitive matching                 |
| `whole_word`                | Match whole words only                    |
| `invert` / `not`            | Invert match (lines NOT matching)         |
| `multiline`                 | Multiline regex mode                      |
| `only_match`                | Output only the matched text              |
| `context N`                 | Include N lines of context around matches |
| `max N`                     | Maximum number of matches                 |
| `in "*.rs"`                 | Restrict to files matching glob           |
| `in lines 10 to 20`         | Restrict to line range                    |
| `between "START" and "END"` | Restrict to lines between patterns        |

**Examples:**

```bash
find "TODO"                          # literal search
find /fn\s+\w+/                      # regex search
find "error" ignore_case max 5       # case-insensitive, max 5 results
find "unsafe" whole_word in "*.rs"   # whole word in Rust files
find "debug" not                     # lines NOT containing "debug"
find "secret" between "BEGIN" and "END"  # only in marked sections
```

### `replace` — Substitute Text (sed s///)

```
replace PATTERN with "REPLACEMENT" [MODIFIERS]
```

| Modifier              | Description                              |
| --------------------- | ---------------------------------------- |
| `all` / `global`      | Replace all occurrences (not just first) |
| `ignore_case`         | Case-insensitive matching                |
| `between "S" and "E"` | Only replace within pattern range        |
| `in lines N to M`     | Only replace within line range           |

**Examples:**

```bash
replace "old_name" with "new_name" all       # global replace
replace /(\w+)_id/ with "${1}Id" all         # regex with capture groups
replace "TODO" with "DONE" all ignore_case   # case-insensitive
replace "X" with "Y" between "START" and "END"  # scoped replace
```

### `delete` — Remove Lines (sed d)

```
delete [lines] [matching] PATTERN
```

**Examples:**

```bash
delete "^#"                          # delete comment lines
delete lines matching /^\s*$/        # delete blank lines
```

### `insert` — Insert Lines (sed i/a)

```
insert "TEXT" before PATTERN
insert "TEXT" after PATTERN
```

**Examples:**

```bash
insert "// TODO: review" before "unsafe"
insert "// end section" after /^}/
```

### `select` — Pick Fields (awk print)

```
select [fields] N, M, ...
```

Fields are 1-based. Separator is whitespace by default; change with `set separator`.

**Examples:**

```bash
select fields 1, 3             # select first and third fields
select 2                       # select just field 2
set separator "," | select 1, 3   # CSV: select columns 1 and 3
```

### `filter` — Conditional Filtering (awk conditions)

```
filter CONDITION
```

| Condition                 | Description                   |
| ------------------------- | ----------------------------- |
| `field N contains "text"` | Field contains substring      |
| `field N equals "text"`   | Field equals value exactly    |
| `field N matches /regex/` | Field matches regex           |
| `field N > NUMBER`        | Numeric greater-than          |
| `field N < NUMBER`        | Numeric less-than             |
| `field N != "text"`       | Field not equal to value      |
| `field N not_empty`       | Field is not empty            |
| `line contains "text"`    | Full line contains substring  |
| `line matches /regex/`    | Full line matches regex       |
| `COND and COND`           | Both conditions must be true  |
| `COND or COND`            | Either condition must be true |

**Examples:**

```bash
filter field 3 > 100                               # numeric filter
filter field 1 contains "error"                     # substring filter
filter field 2 not_empty and field 3 > 0            # compound filter
filter line matches /^(ERROR|WARN)/                 # regex line filter
```

### `sort` — Order Lines

```
sort [by field N | by line] [asc | desc] [numeric]
```

**Examples:**

```bash
sort                             # alphabetical
sort desc                        # reverse alphabetical
sort by field 2 numeric          # numeric sort by field 2
sort by field 1 desc numeric     # descending numeric by field 1
```

### `unique` — Deduplicate

```
unique [by field N]
```

### `take` / `first` — Take First N

```
take N
first 10
```

### `skip` — Skip First N

```
skip N
```

### `last` / `tail` — Take Last N

```
last N
tail 5
```

### `count` — Count Lines

```
count [PATTERN]
```

### `aggregate` — Compute Aggregations

```
aggregate FUNCTION
```

| Function             | Description                     |
| -------------------- | ------------------------------- |
| `count`              | Count records                   |
| `sum [field] N`      | Sum of field N                  |
| `avg [field] N`      | Average of field N              |
| `min [field] N`      | Minimum of field N              |
| `max [field] N`      | Maximum of field N              |
| `distinct [field] N` | List distinct values of field N |
| `freq [field] N`     | Frequency table of field N      |

### `set separator` — Change Field Separator

```
set separator "SEPARATOR"
```

Changes the separator for all subsequent `select`, `filter`, `sort by field`, and `aggregate` stages.

**Examples:**

```bash
set separator ","    # CSV
set separator "|"    # pipe-delimited
set separator "\t"   # tab-delimited
```

---

## Keyword Aliases

AQL accepts multiple aliases for most keywords to be forgiving:

| Canonical     | Aliases                                    |
| ------------- | ------------------------------------------ |
| `find`        | `search`                                   |
| `delete`      | `remove`                                   |
| `insert`      | `add`                                      |
| `select`      | `pick`                                     |
| `filter`      | `where`                                    |
| `sort`        | `order`                                    |
| `unique`      | `uniq`, `dedup`, `deduplicate`             |
| `take`        | `head`, `first`                            |
| `last`        | `tail`                                     |
| `skip`        | —                                          |
| `aggregate`   | `agg`                                      |
| `ignore_case` | `ignorecase`, `nocase`, `case_insensitive` |
| `whole_word`  | `wholeword`, `word`                        |
| `invert`      | `not`                                      |
| `all`         | `global`, `every`                          |
| `contains`    | `has`                                      |
| `equals`      | `eq`, `is`                                 |
| `separator`   | `sep`, `delim`, `delimiter`                |
| `avg`         | `average`, `mean`                          |
| `distinct`    | —                                          |
| `freq`        | `frequency`                                |
| `context`     | `ctx`                                      |

---

## Classic Syntax → AQL Translation Table

### grep → AQL

| grep                  | AQL                            |
| --------------------- | ------------------------------ |
| `grep 'pattern' file` | `find "pattern"`               |
| `grep -i 'pat'`       | `find "pat" ignore_case`       |
| `grep -v 'pat'`       | `find "pat" invert`            |
| `grep -w 'pat'`       | `find "pat" whole_word`        |
| `grep -o 'pat'`       | `find "pat" only_match`        |
| `grep -c 'pat'`       | `find "pat" \| count`          |
| `grep -l 'pat'`       | `find "pat"` (files in output) |
| `grep -m 5 'pat'`     | `find "pat" max 5`             |
| `grep -E 'a\|b'`      | `find /a\|b/`                  |
| `grep -F 'literal'`   | `find "literal"`               |
| `grep -A 3 'pat'`     | `find "pat" context 3`         |

### sed → AQL

| sed                    | AQL                                            |
| ---------------------- | ---------------------------------------------- |
| `sed 's/old/new/'`     | `replace "old" with "new"`                     |
| `sed 's/old/new/g'`    | `replace "old" with "new" all`                 |
| `sed 's/old/new/gi'`   | `replace "old" with "new" all ignore_case`     |
| `sed '/pat/d'`         | `delete "pat"`                                 |
| `sed '10,20s/a/b/g'`   | `replace "a" with "b" all in lines 10 to 20`   |
| `sed '/S/,/E/s/a/b/g'` | `replace "a" with "b" all between "S" and "E"` |

### awk → AQL

| awk                             | AQL                             |
| ------------------------------- | ------------------------------- |
| `awk '{print $1, $3}'`          | `select fields 1, 3`            |
| `awk -F',' '{print $1}'`        | `set separator "," \| select 1` |
| `awk '$3 > 100'`                | `filter field 3 > 100`          |
| `awk '/pat/{print}'`            | `find "pat"`                    |
| `awk '{sum+=$2}END{print sum}'` | `aggregate sum field 2`         |
| `awk '{cnt++}END{print cnt}'`   | `count`                         |
| `awk '!seen[$0]++'`             | `unique`                        |
| `sort \| uniq -c`               | `sort \| unique \| count`       |

---

## CLI Usage

```bash
# Execute a query
atp query 'QUERY' [FILES...] [OPTIONS]

# Aliases: atp q, atp aql, atp run
atp q 'find "error" | count' src/

# Options
atp query --explain 'QUERY'    # Explain without executing
atp query --validate 'QUERY'   # Validate syntax only
atp query --include '*.rs' 'QUERY' src/   # File glob filter
atp query --exclude 'test*' 'QUERY' src/  # Exclude pattern
atp query -d 3 'QUERY' src/              # Max depth 3
```

---

## Design Rationale

### Why keywords instead of flags?

Flags (`-i`, `-g`, `-v`) require memorizing per-tool meanings. Keywords (`ignore_case`, `all`, `invert`) are self-documenting and language-independent. An agent generating `find "X" ignore_case` will always produce correct output, whereas an agent generating `grep -i` might confuse it with `sed -i` (in-place).

### Why literal strings by default?

Most searches are for literal text, not regex. `find "hello.world"` searches for the literal dot. `find /hello.world/` uses regex (`.` matches any character). This matches what most users actually want and eliminates the most common source of regex errors.

### Why `|` for composition?

The pipe operator is the most familiar composition mechanism in computing. AQL pipelines read left-to-right, each stage transforming the data before passing it on. Unlike shell pipes, AQL pipes carry **typed data** (file, line number, content, fields) rather than raw text.

### Why `between` as a modifier, not a stage?

`replace "X" with "Y" between "START" and "END"` keeps all lines but only modifies those in range — matching sed's address-range behavior. If `between` were a filter stage, lines outside the range would be lost. The modifier preserves context while scoping operations.
