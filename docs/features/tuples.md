---
title: "Anonymous Enums"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "anonymous enum"
  - "legacy tuple migration"
id: core/features/tuples
parent: core/features
---

# Anonymous Enums

Anonymous enums are short-lived tagged values that do not need a named
`defenum` declaration. Use `_` in the definition position:

```cirru
let
    value $ %:: _ :point 10 20
  println value
  ; => $ %:: _ :point 10 20
```

The `_` marker makes the missing definition explicit. The old “tuple” name and
`&tuple:*` native APIs have been removed; diagnostics point to their `enum`
replacements.

## Accessing Values

Use `&enum:nth` when positional access is genuinely appropriate. Index `0` is
the variant tag and payload values begin at index `1`.

```cirru
let
    value $ %:: _ :point 10 20
  assert= :point $ &enum:nth value 0
  assert= 10 $ &enum:nth value 1
  assert= 3 $ &enum:count value
```

Anonymous enum access remains bounds-checked and reports ordinary diagnostics;
it never relies on unchecked indexing.

## Matching

`tag-match` can match an anonymous enum when there is no definition available
for exhaustiveness analysis:

```cirru
let
    value $ %:: _ :point 10 20
  assert= 30 $ tag-match value
    (:point x y) (+ x y)
    _ 0
```

For domain data, prefer a named enum and native `match`, which can validate
variants, payload arity, and exhaustiveness:

```cirru
let
    Shape $ defenum Shape (:point 'Number 'Number) (:none)
    value $ %:: Shape :point 10 20
  assert= 30 $ match value
    (:point x y) (+ x y)
    (:none) 0
```

## Named Conversion

When the expected function parameter type is a named enum, the preprocessor can
rewrite `%:: _ :variant ...` to that named definition. Variant and payload
validation then use the target `defenum` declaration.

## Migration

Use these replacements when upgrading older code:

| Removed spelling | Replacement |
| --- | --- |
| tuple value / `:tuple` | enum value / `:enum` |
| `tuple?` | `enum?` for values, `enum-def?` for definitions |
| `tuple-enum` | `enum-definition` |
| `&tuple:nth` | `&enum:nth` |
| `&tuple:count` | `&enum:count` |
| `&tuple:assoc` | `&enum:assoc` |
| `&tuple:*` | corresponding `&enum:*` API |

Compatibility readers may still load historical snapshots, but newly written
schemas and source should use `Enum`, `EnumDef`, `enum`, and `enum-def` names.

## See Also

- [Enums](enums.md) — named enums declared with `defenum`
- [Structs](records.md) — fixed named fields declared with `defstruct`
- [Static Analysis](static-analysis.md) — enum payload and match checking
