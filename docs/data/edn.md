---
title: "Cirru Extensible Data Notation"
scope: "core"
kind: "reference"
category: "data"
aliases:
  - "cirru edn"
  - "edn notation"
  - "data notation"
entry_for:
  - "cr cirru parse-edn"
---
# Cirru Extensible Data Notation

Cirru EDN is Calcit's typed data interchange format, inspired by [Clojure EDN](https://github.com/edn-format/edn). Use it when Calcit-specific identity matters; use JSON only when interoperating with JSON systems.

The two runtime APIs are:

- `parse-cirru-edn text [type-options]`
- `format-cirru-edn value`

Map and Set iteration order is not a semantic guarantee. The formatter applies a stable order for readable, reproducible output; callers must not use that order as application data.

## Choosing Cirru EDN or JSON

| Calcit value | Cirru EDN | JSON |
| --- | --- | --- |
| Nil, Bool, Number, String, List | Preserved | Preserved |
| Tag, Symbol | Preserved | Encoded as a JSON string; identity is lost |
| Map | Arbitrary EDN keys preserved | Keys must be Tag or String; parsed object keys become Tag |
| Set | Preserved | Encoded as an array; Set identity is lost |
| Buffer | Preserved | Encoded as a `0x...` string; Buffer identity is lost |
| Tuple / enum value | Tag, fields, and enum name preserved | Encoded as an array; tuple and enum identity is lost |
| Record | Record name and fields preserved | Encoded as an object; struct identity is lost |
| Cirru quote | Preserved | Encoded as nested strings and arrays |
| Ref | Encoded as `atom`; parsing creates a new Ref | Unsupported |
| AnyRef, function, trait/impl, mutable builder | Not portable; do not use as interchange data | Unsupported |

`json-stringify` rejects unsupported values and Maps with non-Tag/non-String keys instead of silently inventing a representation. It also rejects non-finite numbers. `json-parse` maps JSON arrays to List and JSON objects to Maps whose keys are Tags.

## Restoring declared record and enum identity

Cirru EDN always retains the printed record or enum name. To reconnect parsed values to the exact `defstruct` or `defenum` object (including attached traits), pass an options Map keyed by that name:

```cirru
let
    Person $ defstruct Person (:name 'String)
    encoded $ format-cirru-edn $ %{} Person (:name |Ada)
  parse-cirru-edn encoded $ {}
    :Person $ %{} Person (:name |)
```

Without this options Map, parsing still produces a structurally equivalent Record or Tuple, but it cannot recover declaration-attached trait implementations from text alone.

## Literals

For literals, if written in text syntax, we need to add `do` to make sure it's a line:

```cirru
do nil
```

for a number:

```cirru
do 1
```

for a symbol:

```cirru
do 's
```

Tags use a leading colon:

```cirru
do :k
```

## String escaping

for a string:

```cirru
do |demo
```

or wrap with double quotes to support special characters like spaces:

```cirru
do "|demo string"
```

`\n` `\t` `\"` `\\` are supported.

## Data structures

for a list:

```cirru
[] 1 2 3
```

or nested list inside list:

```cirru
[] 1 2
  [] 3 4
```

HashSet for unordered elements:

```cirru
#{} :a :b :c
```

HashMap:

```cirru
{}
  :a 1
  :b 2
```

also can be nested:

```cirru
{}
  :a 1
  :c $ {}
    :d 3
```

Records retain their type name and fields:

```cirru
let
    A $ defstruct A (:a 'Number)
  ; Then create an instance in Calcit
  %{} A
    :a 1
```

Enums use `%::`, while ordinary tagged tuples use `::`:

```cirru.no-run
%:: :Result :ok 1
:: :point 10 20
```

## Quotes

Quoted Cirru is preserved as syntax data. This is used by runtime snapshots (`calcit.cirru`, legacy `compact.cirru`):

```cirru
quote $ def a 1
```

at runtime, it's represented with tuples:

```cirru
:: 'quote $ [] |def |a |1
```

which means you can eval:

```bash
$ cr eval "println $ format-cirru-edn $ :: 'quote $ [] |def |a |1"

quote $ def a 1

took 0.027ms: nil
```

and also:

```bash
$ cr eval 'parse-cirru-edn "|quote $ def a 1"'
took 0.011ms: (:: 'quote ([] |def |a |1))
```

The runtime display may resemble a tuple, but Cirru EDN gives `quote` dedicated parsing and formatting semantics.

## Buffers

Buffers can be created using the `&buffer` function with hex values:

```cirru
&buffer 0x03 0x55 0x77 0xff 0x00
```

## Comments

Comment expressions start with `;`. They occupy nodes in the Cirru syntax tree and are ignored while EDN data is decoded.

Some usages:

```cirru
[] 1 2 3 (; comment) 4 (; comment)
```

```cirru
{}
  ; comment
  :a 1
```

Comments must still obey Cirru indentation because they are syntax-tree nodes, not lexer comments.
