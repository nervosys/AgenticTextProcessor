# AQL Syntax Reference

## Formal Grammar (EBNF)

```ebnf
pipeline      = stage { "|" stage } ;
stage         = find | replace | delete | insert
              | select | filter | sort | unique
              | take | skip | last | count
              | aggregate | set_separator ;

find          = "find" pattern { modifier } ;
replace       = "replace" pattern "with" string { modifier } ;
delete        = "delete" [ "lines" ] [ "matching" ] pattern ;
insert        = "insert" string ( "before" | "after" ) pattern ;

select        = "select" [ "fields" ] integer { "," integer } ;
filter        = "filter" condition ;
set_separator = "set" "separator" string ;

sort          = "sort" [ "by" ( "field" integer | "line" ) ]
                [ "asc" | "desc" ] [ "numeric" ] ;
unique        = "unique" [ "by" "field" integer ] ;
take          = ( "take" | "first" ) integer ;
skip          = "skip" integer ;
last          = "last" integer ;
count         = "count" [ pattern ] ;
aggregate     = ( "sum" | "avg" | "min" | "max" | "distinct" | "freq" )
                "field" integer ;

pattern       = string | regex ;
string        = '"' { char } '"' | "'" { char } "'" ;
regex         = "/" { char } "/" ;
modifier      = "ignore_case" | "whole_word" | "invert"
              | "all" | "global" | "multiline" ;

condition     = field_cond | line_cond | compound ;
field_cond    = "field" integer op value ;
line_cond     = "line" ( "contains" | "matches" ) pattern ;
op            = "contains" | "equals" | "!=" | "matches"
              | ">" | "<" | "not_empty" ;
compound      = condition ( "and" | "or" ) condition ;
```

## Patterns

AQL supports two pattern types:

### Literal Strings

Enclosed in double or single quotes. No escaping required:

```
find "hello world"
find 'path/to/file'
```

### Regular Expressions

Enclosed in forward slashes. Standard regex syntax:

```
find /fn\s+\w+\(/
find /^import\s/
```

## Pipes

Stages are composed with `|`:

```
find "TODO" | sort | unique | take 10
```

Each stage receives the output of the previous stage as input. The first stage receives the file contents.
