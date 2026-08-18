---
title: "Enums (defenum)"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "defenum"
  - "tagged union"
  - "tagged unions"
id: core/features/enums
parent: core/features
---

# Enums (defenum)

Calcit enums are tagged unions: each variant has a tag and zero or more typed payload values. `defenum` creates an enum definition; `%::` constructs enum values that retain that definition.

## Quick Recipes

- **Define**: `defenum Shape (:circle 'Number) (:rect 'Number 'Number)`
- **Create**: `%:: Shape :circle 5`
- **Match** (recommended): `match shape ((:circle r) ...) ((:rect w h) ...)`
- **Legacy Fallback**: `tag-match shape ((:circle r) ...) ((:rect w h) ...)`
- **Type Check**: `assert-type shape 'Shape`

## Defining Enums

```cirru
let
    Color $ defenum Color (:red) (:green) (:blue)
    c $ %:: Color :red
  println c
  ; => $ %:: 'Color :red
```

Variants with payloads:

```cirru
let
    Shape $ defenum Shape (:circle 'Number) (:rect 'Number 'Number)
    c $ %:: Shape :circle 5
    r $ %:: Shape :rect 3 4
  println c
  ; => $ %:: 'Shape :circle 5
  println r
  ; => $ %:: 'Shape :rect 3 4
```

## Generic Enums

`defenum` accepts an optional generics list right after the type name. Declare generic slots with quoted symbols, then use the applied named type syntax `(:: 'TypeName ...)` in schemas and assertions.

```cirru
let
    ResultX $ defenum ResultX ([] 'T 'E) (:ok 'T) (:err 'E)
    ok $ %:: ResultX :ok 1
    err $ %:: ResultX :err |oops
  assert-type ok $ :: 'ResultX 'Number 'String
  assert-type err $ :: 'ResultX 'Number 'String
  assert= 1 $ &enum:nth ok 1
  assert= |oops $ &enum:nth err 1
```

At the type level, `(:: 'ResultX 'Number 'String)` means the first slot is bound to `T` and the second slot is bound to `E`.

### Generic Enums with `where` Bounds

After the optional generics list, `defenum` may also take a `where` map to constrain type variables with traits. This is useful when enum payloads must support methods like `.debug`.

```cirru
let
    ShownMaybe $ defenum ShownMaybe ([] 'T)
      {} $ 'T Debug
      :some 'T
      :none
    some-val $ %:: ShownMaybe :some 1
    none-val $ %:: ShownMaybe :none
  assert-type some-val $ :: 'ShownMaybe 'Number
  assert= |1 $ match some-val
    (:some item) (.debug item)
    (:none) |none
  assert= |none $ match none-val
    (:some item) (.debug item)
    (:none) |none
```

The `where` map uses type variables as keys and trait names as values. `%::` checks the variant payloads against those bounds at runtime, so invalid type arguments are rejected when the enum value is created.

## Enums with Struct Payloads

Enum payload slots can reference named structs, not just primitive tags. Use the same applied named type syntax that `defstruct` uses in schemas.

```cirru.no-check
let
    Point $ defstruct Point (:x :number) (:y :number)
    Shape $ defenum Shape (:point Point) (:label :string)
    p $ %{} Point (:x 1) (:y 2)
    shape $ %:: Shape :point p
  assert-type shape Shape
  assert= 1 $ :x (&enum:nth shape 1)
```

Applied generic structs also work in enum payloads:

```cirru
let
    Box $ defstruct Box ([] 'T) (:value 'T)
    Wrapped $ defenum Wrapped ([] 'T)
      :box $ :: 'Box 'T
      :empty
    value $ %:: Wrapped :box
      %{} Box $ :value 1
  assert-type value $ :: 'Wrapped :number
  assert= 1 $ match value
    (:box boxed) (:value boxed)
    (:empty) 0
```

When you annotate an enum payload with a struct type, `%::` validates both the variant tag and the payload value at runtime. Applied struct payload types must use the correct generic arity.

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
      (:circle radius) (* radius radius 3.14159)
      (:rect w h) (* w h)
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
        (:ok msg) (str-spaced |OK: msg)
        (:err msg) (str-spaced |Error: msg)
  println $ describe ok
  ; => OK: success
```

## Pattern Matching with `match` (recommended)

`match` is a **native syntax** (not a macro) that branches on enum variant tags. Because the compiler sees the full branch structure, it can perform:

- **Exhaustiveness checking** — warns at preprocess time if any variant is not covered
- **Variant arity check** — warns if the binding count doesn't match the variant's payload count
- **Binding type inference** — each binding variable automatically receives the payload type from the enum definition

### Basic usage

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    area $ match c
      (:circle radius) (* radius radius 3.14159)
      (:rect w h) (* w h)
  println area
  ; => 78.53975
```

### Multi-line branch bodies

When a branch body needs more than one expression, indent subsequent lines under the branch:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :string) (:err :string)
    r $ %:: ApiResult :err |network-error
    msg $ match r
      (:ok v) (str-spaced |OK: v)
      (:err e) (str-spaced |Error: e)
  println msg
  ; => Error: network-error
```

### Exhaustiveness checking

If you omit a variant and don't have a wildcard `_` branch, the compiler warns:

```cirru.no-check
match c
  (:circle radius) (* radius radius 3.14159)
  ; "⚠" Warning: match on "`Shape`" is not exhaustive. Missing variant (s) : [:rect]
```

The check fires only when the compiler can infer the enum type of the matched value — for example, when the value is directly constructed with `%::`, or when the function parameter is annotated with an enum type in its schema. When the type cannot be inferred at preprocess time, the match still works at runtime but no compile-time warning is issued.

Use `_` as a wildcard to catch remaining variants:

```cirru
let
    Shape $ defenum Shape (:circle :number) (:rect :number :number)
    c $ %:: Shape :circle 5
    label $ match c
      (:circle _r) |round
      _ |other
  println label
  ; => round
```

### No-match runtime error

If no branch matches at runtime (and no `_` wildcard is present), `match` throws:

```
match: no matching branch for tag :unknown-tag
```

This is an explicit crash, not a silent `nil`. `tag-match` has the same behavior.

### `match` vs `tag-match`

| Feature                | `match`              | `tag-match`                    |
| ---------------------- | -------------------- | ------------------------------ |
| Implementation         | Native syntax        | Macro (expands to nested `if`) |
| Exhaustiveness check   | Compile-time warning | None                           |
| Variant arity check    | Yes                  | No                             |
| Binding type inference | Yes (from defenum)   | No                             |
| JS output              | Direct if-else chain | Nested ternaries               |
| Recommended            | Yes                  | Legacy use                     |

Both syntaxes share the same branch format: each branch is `(pattern body)`.

### Migrating from `tag-match` to `match`

The branch syntax is identical — migration is a single keyword replacement:

```cirru.no-check
; Before $ tag-match

tag-match r
  (:ok v) (str-spaced |ok: v)
  (:err e) (str-spaced |err: e)

; After $ match

match r
  (:ok v) (str-spaced |ok: v)
  (:err e) (str-spaced |err: e)
```

After replacing the keyword, the compiler will report any uncovered variants. Either add the missing branches or append `_ <default>` to keep the wildcard behaviour.

## Zero-payload Variants

When a variant has no payload, the pattern is just the tag wrapped in parentheses:

```cirru
let
    MaybeInt $ defenum MaybeInt (:some :number) (:none)
    some-val $ %:: MaybeInt :some 42
    none-val $ %:: MaybeInt :none
    extracted $ match some-val
      (:some v) (* v 2)
      (:none) nil
  println extracted
  ; => 84
```

## Checking Enum Origin

Use the Option-returning `enum-definition` API to inspect the definition behind an enum value:

```cirru
let
    ApiResult $ defenum ApiResult (:ok :number) (:err :string)
    x $ %:: ApiResult :ok 1
  println $ = (enum-definition x) (%some ApiResult)
  ; => true
```

## Common Patterns

### Result / Either type

```cirru
let
    AppResult $ defenum AppResult (:ok :number) (:err :string)
    compute $ fn (x)
      if (> x 0)
        %:: AppResult :ok $ * x 10
        %:: AppResult :err |negative-input
    handle $ fn (r)
      match r
        (:ok v) (str-spaced |result: v)
        (:err e) (str-spaced |failed: e)
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
      match s
        (:done _) true
        (:pending) false
        (:failed _) false
  println $ is-done pending
  ; => false
  println $ is-done done
  ; => true
```

## Type Annotations

Field types in `defenum` declarations participate in type checking:

```cirru.no-run
; (:ok :string) means the :ok variant has one :string payload

defenum ApiResult (:ok :string) (:err :string)

; (:point :number :number) means :point has two :number payloads

defenum Shape (:point :number :number) (:circle :number)

; generic struct payloads use applied named types

defstruct Box ([] 'T) (:value 'T)

defenum Wrapped ([] 'T)
  :box $ :: 'Box 'T
  :empty

; (:none) means no payload

defenum MaybeInt (:some :number) (:none)
```

Payloads may also reference named struct types (illustrative, cross-file type ref):

```cirru.no-check
; defstruct and defenum from the same snippet context:

defstruct Point (:x :number) (:y :number)

defenum Shape2 (:point Point) (:circle :number)
```

Runtime type validation is enforced at instance creation — passing the wrong type to `%::` will raise an error.

## Automatic Anonymous-to-Named Enum Rewrite

When a function parameter is typed as an enum, the preprocessor can rewrite an anonymous enum (`%:: _`) to the expected named definition.

```cirru
let
    Result0 $ defenum Result0 (:err :string) (:ok)
    takes-result $ fn (r)
      :: :fn $ {} (:return :dynamic)
        :args $ [] 'app.main/Result0
      match r
        (:ok) :ok
        (:err msg) msg
        _ :unknown
  ; The anonymous enum is rewritten to the expected named enum:
  assert= :ok $ takes-result (%:: _ :ok)
  ; Equivalent to:
  assert= :ok $ takes-result (%:: Result0 :ok)
```

Requirements for the rewrite to trigger:

- The function schema must reference a named enum type such as `'ns/EnumName`
- The argument at the call site must be an anonymous enum literal (`%:: _`)
- The enum definition must be resolvable at preprocess time

If any condition is not met, the argument is left unchanged (no error is raised). This makes the rewrite safe to use alongside existing code.

## Notes

- Enum values are immutable and retain their enum definition.
- `match` is the recommended pattern matching syntax with exhaustiveness checking.
- `tag-match` is a legacy macro; keep it for legacy code paths or when you explicitly want the old syntax.
- Use `&enum:nth` to directly access payload values by index (0 = tag, 1+ = payloads).
- `%:: _ :tag ...` constructs an anonymous enum; `%:: EnumDef :tag ...` constructs a named enum.

## See Also

- [Anonymous enums](anonymous-enums.md) — short-lived enum values without `defenum`
- [Structs](structs.md) — named-field structures with `defstruct`
- [Static Analysis](static-analysis.md) — type checking for enum payloads and type slots
