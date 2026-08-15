---
title: "Traits"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "trait call"
  - "trait impl"
  - "assert-traits"
---

# Traits

Calcit provides a lightweight nominal trait system for attaching method implementations to struct/enum definitions and for describing capabilities of built-in values. The historical prototype/class terminology is no longer the design model; see [Polymorphism](polymorphism.md) for the unified dispatch rules.

Keep two concepts separate:

- A **trait impl** has a concrete `deftrait` value as its origin. It participates in `.method` dispatch and can satisfy `assert-traits`, generic `:where` bounds, and `&trait-call`.
- An **inherent method bag** has no trait origin. It remains compatible with legacy dispatch through `.method`, but it does not prove any trait capability.

## Quick Recipes

- **Define Trait**: `deftrait MyTrait .method (:: 'Fn $ {} ...)`
- **Implement Trait**: `defimpl MyImpl MyTrait .method (fn (x) ...)`
- **Attach to Struct**: `impl-traits MyStruct MyImpl`
- **Call Method**: `instance .method` (receiver-first; `.method instance` remains compatible)
- **Check Trait**: `assert-traits instance MyTrait`

## Define a trait

Use `deftrait` to define a trait and its method signatures (including type annotations).

```cirru
deftrait MyFoo $ .foo
  :: 'Fn $ {}
    :generics $ [] 'T
    :args $ [] 'T
    :return 'String
```

## Implement a trait

Use `defimpl` to create a nominal impl value for a trait.

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    Person0 $ defstruct Person (:name 'String)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p) (assert-type p Person0)
        str-spaced |foo $ :name p
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  p .foo
```

### Impl-related syntax (cheatsheet)

**1) `defimpl` argument order (breaking change)**

```
defimpl ImplName Trait ...
```

- First argument is the **impl value name**.
- Second argument is the concrete **trait value** (normally a symbol). A tag is accepted only for the legacy method-bag form described below.

Examples:

```cirru
let
    PersonA0 $ defstruct PersonA (:name 'String)
    MyFooA $ deftrait MyFooA
      .foo $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    MyFooImplA $ defimpl MyFooImplA MyFooA
      .foo $ fn (p) (assert-type p PersonA0)
        str-spaced |foo $ :name p
    PersonA $ impl-traits PersonA0 MyFooImplA
    p $ %{} PersonA (:name |Alice)
  p .foo
```

**2) Method pair forms**

Prefer dot-style keys (`.foo`). Legacy tag keys (`:foo`) are still accepted for compatibility.

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    Person0 $ defstruct Person (:name 'String)
    ImplB $ defimpl ImplB MyFoo
      :: :foo $ fn (p) (assert-type p Person0)
        str |B: $ :name p
    PersonB $ impl-traits Person0 ImplB
    pb $ %{} PersonB (:name |Bob)
  pb .foo
```

**3) Legacy tag-based method bags (compatibility only)**

Passing a tag instead of a concrete trait value creates an originless/inherent method bag:

```cirru.no-check
defimpl :MyMarkerImpl :MyMarker $ .dummy
  fn (_x) nil
```

This form is retained so older `.method` dispatch keeps working. It does **not** implement a nominal trait and therefore cannot satisfy `assert-traits`, a generic `:where` bound, or `&trait-call`. `cr edit format` reports the non-blocking `W_LEGACY_INHERENT_IMPL` migration advisory. New code should define a real trait and pass its symbol:

```cirru
let
    MyMarker $ deftrait MyMarker (.dummy 'Fn)
    MyMarkerImpl $ defimpl MyMarkerImpl MyMarker
      .dummy $ fn (_x) nil
  , MyMarkerImpl
```

This also replaces the old self-referential pattern `defimpl X X`, which can recurse while the definition is being initialized.

Implementation notes:

- With a concrete trait argument, `defimpl` creates an impl that stores that exact trait value as its origin.
- The impl must provide exactly the trait's declared method set; method values must be callable, and native preprocessing checks declared signatures when signature metadata is available.
- Trait identity is nominal at runtime. Two independently evaluated traits do not become equal merely because their printed names and method sets match.
- This origin is used by `assert-traits` and `&trait-call`; methods from multiple unrelated impls are never merged to satisfy one trait.

## Attach impls to struct/enum definitions

`impl-traits` attaches impl records to a **struct/enum type**. For user values, later impls override earlier impls for the same method name ("last-wins").

Constraints:

- `impl-traits` only accepts **struct/enum** values.
- Struct/enum values must be created from a definition that already has impls attached (`%{}` or `%::`).

Syntax:

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    StructDef0 $ defstruct StructDef (:name 'String)
    ImplA $ defimpl ImplA MyFoo
      .foo $ fn (p) (assert-type p StructDef0)
        str |A: $ :name p
    MyBar $ deftrait MyBar (.bar 'Fn)
    ImplB $ defimpl ImplB MyBar
      .bar $ fn (p) (assert-type p StructDef0)
        str |B: $ :name p
    StructDef $ impl-traits StructDef0 ImplA ImplB
    x $ %{} StructDef (:name |test)
  x .foo
```

### Public vs internal API boundary

- Prefer public API in app/library code: `deftrait`, `defimpl`, `impl-traits`, `.method`, `&trait-call`.
- Treat internal `&...` helpers as runtime-level details; they may change more frequently and are not the stable user contract.

```cirru.no-check
do (; struct example)
  let
      MyFoo $ deftrait MyFoo
        .foo $ :: 'Fn
          {}
            :generics $ [] 'T
            :args $ [] 'T
            :return 'String
      Person0 $ defstruct Person (:name 'String)
      MyFooImpl $ defimpl MyFooImpl MyFoo
        .foo $ fn (p) (assert-type p Person0)
          str-spaced |foo $ :name p
      Person $ impl-traits Person0 MyFooImpl
      p $ %{} Person (:name |Alice)
    p .foo
  ; enum example
  let
      ResultTrait $ deftrait ResultTrait
        .describe $ :: 'Fn
          {}
            :generics $ [] 'T
            :args $ [] 'T
            :return 'String
      ResultImpl $ defimpl ResultImpl ResultTrait
        .describe $ fn (x)
          match x
            (:ok v) (str |ok: v)
            (:err v) (str |err: v)
      Result0 $ defenum Result0 (:ok 'String) (:err 'String)
      MyResult $ impl-traits Result0 ResultImpl
      r $ %:: MyResult :ok |done
    r .describe
```

### Static analysis boundary

For preprocess to resolve impls and inline methods, keep struct/enum definitions and `impl-traits` at **top-level `ns/def`**. If they are created inside `defn`/`defmacro` bodies, preprocess only sees dynamic values and method dispatch cannot be specialized.

When running `warn-dyn-method`, preprocess emits extra diagnostics for:

- `.method` call sites that have multiple trait candidates with the same method name.
- `impl-traits` used inside function/macro bodies (non-top-level attachment).

## Docs as tests

Key trait docs examples are mirrored by executable smoke cases in `calcit/test-doc-smoke.cirru`, including:

- `defimpl` argument order (`ImplName` then `Trait`)
- `assert-traits` local-first requirement
- `impl-traits` only accepting struct/enum definitions

## Method call vs explicit trait call

Normal method invocation uses `.method` dispatch. If multiple traits provide the same method name, `.method` resolves by impl precedence.

When you want to **disambiguate** (or bypass `.method` resolution), use `&trait-call`.

### `&trait-call`

Usage: `&trait-call Trait :method receiver & args`

`&trait-call` matches by the impl value's trait origin, not just by trait name text. This avoids accidental dispatch when two different trait values share the same printed name.

Example with two traits sharing the same method name:

```cirru
let
    MyZapA $ deftrait MyZapA
      .zap $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    MyZapB $ deftrait MyZapB
      .zap $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    MyZapAImpl $ defimpl MyZapAImpl MyZapA
      .zap $ fn (_x) |zapA
    MyZapBImpl $ defimpl MyZapBImpl MyZapB
      .zap $ fn (_x) |zapB
    Person0 $ defstruct Person (:name 'String)
    Person $ impl-traits Person0 MyZapAImpl MyZapBImpl
    p $ %{} Person (:name |Alice)
  ; .zap follows normal dispatch $ last-wins for user impls
  p .zap
  ; explicitly pick a "trait’s" implementation
  &trait-call MyZapA :zap p
  &trait-call MyZapB :zap p
```

## Debugging / introspection

Two helpers are useful when debugging trait + method dispatch:

- `&methods-of` returns a list of available method names (strings, including the leading dot).
- `&inspect-methods` prints impl records and methods to stderr, and returns the value unchanged.
- `impl-origin` returns the trait origin as `Option<Trait>`; inherent method bags return `%none`.

```cirru
let
    xs $ [] 1 2
  &methods-of xs
  &inspect-methods xs |list
```

You can also inspect impl origins directly when validating trait dispatch:

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    Shape0 $ defenum Shape (:point 'Number 'Number)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (t)
        str |shape: $ &enum:nth t 0
    Shape $ impl-traits Shape0 MyFooImpl
    shape $ %:: Shape :point 10 20
    impls $ &enum:impls shape
  any? impls $ fn (impl)
    = (impl-origin impl) (%some MyFoo)
```

## Checking trait requirements

`assert-traits` checks at runtime that a value contains one complete impl whose origin is the requested trait. It returns the value unchanged if the check passes. A same-named method from another trait or an inherent method bag is not sufficient.

Notes:

- `assert-traits` is syntax (expanded to `&assert-traits`) and its first argument must be a **local**.
- Built-in values (list/map/set/string/number/...) expose the same origin-carrying core impls to native and JS. `assert-traits` only validates them; it does **not** extend methods at runtime.
- Static preprocessing uses the evaluated impl metadata when available and a small core bootstrap map during initialization. Runtime validation remains nominal.

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return 'String
    Person0 $ defstruct Person (:name 'String)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p) (assert-type p Person0)
        str-spaced |foo $ :name p
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  assert-traits p MyFoo
  p .foo
```

### Examples (verified with `cr eval`)

```bash
cargo run --bin cr -- calcit.cirru eval 'let ((xs ([] 1 2 3))) (assert= xs (assert-traits xs calcit.core/Len)) (xs .len)'
```

Expected output:

```text
3
```

```bash
cargo run --bin cr -- calcit.cirru eval 'let ((xs ([] 1 2 3))) (assert= xs (assert-traits xs calcit.core/Mappable)) (xs .map inc)'
```

Expected output:

```text
([] 2 3 4)
```
