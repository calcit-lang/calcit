---
title: "Enums (defenum)"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "defenum"
  - "tagged union"
  - "tagged unions"
---
# Enums (defenum)

Calcit enums are tagged unions — each variant has a tag (keyword) and zero or more typed payload fields. Under the hood enums are represented as tuples with a class reference.

## Quick Recipes

- **Define**: `defenum Shape (:circle :number) (:rect :number :number)`
- **Create**: `%:: Shape :circle 5`
- **Match** (recommended): `match shape ((:circle r) ...) ((:rect w h) ...)`
- **Match** (legacy): `tag-match shape ((:circle r) ...) ((:rect w h) ...)`
- **Type Check**: `assert-type shape :enum`

## Defining Enums

```cirru
let
    Color $ defenum Color (:red) (:green) (:blue)
    c $ %:: Color :red
  println c
  ; => (%:: :red (:enum Color))
```

Variants with payloads:

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    r $ %:: Shape :rect 3 4
  println c
  ; => (%:: :circle 5 (:enum Shape))
  println r
  ; => (%:: :rect 3 4 (:enum Shape))
```

## Creating Instances

Use `%::` with the enum definition, the variant tag, and then the payload values:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :string) (:err :string)
    ok $ %:: ApiResult :ok |success
    err $ %:: ApiResult :err |network-error
  println ok
  println err
```

## Pattern Matching with `tag-match` (legacy)

> **Note**: Prefer `match` (see below) for new code. `match` provides compile-time exhaustiveness checking.

`tag-match` branches on the variant tag and binds payload values to names:

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    area $ tag-match c
      (:circle radius)
        * radius radius 3.14159
      (:rect w h)
        * w h
  println area
  ; => 78.53975
```

Multi-line branch bodies (required when the body is more than a single call):

```cirru
let
    ApiResult $ defenum ApiResult (:ok :string) (:err :string)
    ok $ %:: ApiResult :ok |success
    describe $ fn (r)
      tag-match r
        (:ok msg)
          str-spaced |OK: msg
        (:err msg)
          str-spaced |Error: msg
  println (describe ok)
  ; => OK: success
```

## Pattern Matching with `match` (recommended)

`match` is a native syntax (not a macro) that branches on enum variant tags with **compile-time exhaustiveness checking**. When the matched value has a known enum type, the preprocessor verifies that all variants are covered:

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    area
      match c
        (:circle radius) (* radius radius 3.14159)
        (:rect w h) (* w h)
  println area
  ; => 78.53975
```

If you omit a variant and don't have a wildcard `_` branch, the compiler warns:

```cirru.no-check
match c
  (:circle radius) (* radius radius 3.14159)
  ; ⚠ Warning: match on `Shape` is not exhaustive. Missing variant(s): [:rect]
```

Use `_` as a wildcard to catch remaining variants:

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    label
      match c
        (:circle _r) |round
        _ |other
  println label
  ; => round
```

### `match` vs `tag-match`

| Feature | `match` | `tag-match` |
|---------|---------|-------------|
| Implementation | Native syntax | Macro (expands to nested `if`) |
| Exhaustiveness | Compile-time warning | None |
| Variant arity check | Yes | No |
| JS output | Direct if-else chain | Nested ternaries |
| Recommended | Yes | Legacy use |

Both syntaxes share the same branch format: each branch is `(pattern body)`.

## Zero-payload Variants

When a variant has no payload, the pattern is just the tag:

```cirru
let
    MaybeInt $ defenum MaybeInt (:some :number) (:none)
    some-val $ %:: MaybeInt :some 42
    none-val $ %:: MaybeInt :none
    extracted $ tag-match some-val
      (:some v)
        * v 2
      (:none) nil
  println extracted
  ; => 84
```

## Checking Enum Origin

Use `&tuple:enum` to verify a tuple belongs to a specific enum:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :number) (:err :string)
    x $ %:: ApiResult :ok 1
  println $ = (&tuple:enum x) ApiResult
  ; => true
```

## Common Patterns

### Result / Either type

```cirru
let
    AppResult $ defenum AppResult (:ok :number) (:err :string)
    compute $ fn (x)
      if (> x 0)
        %:: AppResult :ok (* x 10)
        %:: AppResult :err |negative-input
    handle $ fn (r)
      tag-match r
        (:ok v)
          str-spaced |result: v
        (:err e)
          str-spaced |failed: e
  println $ handle (compute 5)
  ; => result: 50
  println $ handle (compute -1)
  ; => failed: negative-input
```

### Compose enums with functions

```cirru
let
    Status $ defenum Status (:pending) (:done :string) (:failed :string)
    pending $ %:: Status :pending
    done $ %:: Status :done |ok
    is-done $ fn (s)
      tag-match s
        (:done _) true
        (:pending) false
        (:failed _) false
  println (is-done pending)
  ; => false
  println (is-done done)
  ; => true
```

## Type Annotations

Field types in `defenum` declarations participate in type checking:

```cirru.no-run
; (:ok :string) means the :ok variant has one :string payload
defenum ApiResult (:ok :string) (:err :string)

; (:point :number :number) means :point has two :number payloads
defenum Shape (:point :number :number) (:circle :number)

; (:none) means no payload
defenum MaybeInt (:some :number) (:none)
```

Runtime type validation is enforced at instance creation — passing the wrong type to `%::` will raise an error.

## Automatic Tuple-to-Enum Rewrite

When a function parameter is typed as an enum in its schema, the preprocessor automatically rewrites untyped tuple literal (`::`) arguments to typed enum tuple construction (`%::`). This lets you write shorter tuple syntax while still getting full enum type checking.

```cirru
defenum Result0 (:err :string) (:ok)

defn takes-result (r)
  :: :fn $ {} (:return :dynamic)
    :args $ [] 'app.main/Result0
  tag-match r ((:ok) :ok) ((:err msg) msg) $ _ :unknown

; Write an untyped tuple — preprocessor rewrites to enum tuple automatically:
takes-result $ :: :ok
; Equivalent to:
takes-result $ %:: Result0 :ok
```

Requirements for the rewrite to trigger:

- The function must have a schema with `:args` that references an enum type (via `TypeRef` like `'ns/EnumName`, `Enum`, or `Tuple`)
- The argument at the call site must be an untyped tuple literal (`::`)
- The enum definition must be resolvable at preprocess time

If any condition is not met, the argument is left unchanged (no error is raised). This makes the rewrite safe to use alongside existing code.

## Notes

- Enum instances are immutable tuples with a class reference.
- `match` is the recommended pattern matching syntax with exhaustiveness checking.
- `tag-match` is a legacy macro; unmatched tags raise a runtime error without compile-time warning.
- Use `&tuple:nth` to directly access payload values by index (0 = tag, 1+ = payloads).
- Enums vs plain tuples: plain `:: :tag val` tuples have no class; `%:: Enum :tag val` tuples carry their enum class for origin checking.

## See Also

- [Tuples](tuples.md) — raw tagged tuples without a class
- [Records](records.md) — named-field structs with `defstruct`
- [Static Analysis](static-analysis.md) — type checking for enum payloads and type slots
