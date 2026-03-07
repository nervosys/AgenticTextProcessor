# AQL Stages

## Search & Transform

### find

Search for a pattern in the input.

```
find "pattern"
find /regex/
find "error" ignore_case
find "TODO" whole_word
```

### replace

Replace occurrences of a pattern with a string.

```
replace "old" with "new"
replace "old" with "new" all          # global
replace "old" with "new" ignore_case  # case-insensitive
replace /\d+/ with "NUM"             # regex
```

### delete

Delete lines matching a pattern.

```
delete "pattern"
delete lines matching "pattern"
delete /^#/
```

### insert

Insert text before or after lines matching a pattern.

```
insert "// TODO" before "function"
insert "}" after "class"
```

## Field Processing

### select

Select specific fields (1-indexed).

```
select 1, 3
select fields 1, 2, 5
```

### filter

Filter records by a condition.

```
filter field 2 > 100
filter field 1 contains "error"
filter field 3 equals "NYC"
filter field 1 not_empty
filter line contains "TODO"
```

### set separator

Set the field separator for subsequent stages.

```
set separator ","
set separator "\t"
```

## Ordering & Limiting

### sort

Sort records.

```
sort                          # lexicographic
sort desc                     # descending
sort by field 1               # by specific field
sort by field 2 desc numeric  # numeric descending
```

### unique

Deduplicate records.

```
unique                    # by full line content
unique by field 1         # by specific field
```

### take / first

Take the first N records.

```
take 10
first 5
```

### skip

Skip the first N records.

```
skip 10
```

### last

Take the last N records.

```
last 5
```

## Aggregation

### count

Count lines, optionally matching a pattern.

```
count
count "error"
```

### Aggregate Functions

Aggregate a numeric field.

```
sum field 2
avg field 3
min field 1
max field 4
distinct field 1
freq field 2
```
