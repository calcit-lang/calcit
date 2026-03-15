# Traits

Calcit provides a lightweight trait system for attaching method implementations to struct/enum definitions (and using them from constructed instances and built-in types).

It complements the “class-like” polymorphism described in [Polymorphism](polymorphism.md):

- Struct/enum **classes** are about “this concrete type has these methods”.
- **Traits** are about “this value supports this capability (set of methods)”.

## Quick Recipes

- **Define Trait**: `deftrait MyTrait .method (:: :fn $ {} ...)`
- **Implement Trait**: `defimpl MyImpl MyTrait .method (fn (x) ...)`
- **Attach to Struct**: `impl-traits MyStruct MyImpl`
- **Call Method**: `.method instance`
- **Check Trait**: `assert-traits instance MyTrait`

## Define a trait

Use `deftrait` to define a trait and its method signatures (including type annotations).

```cirru
deftrait MyFoo
  .foo $ :: :fn $ {}
    :generics $ [] 'T
    :args $ [] 'T
    :return :string
```

## Implement a trait

Use `defimpl` to create an impl record for a trait.

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn $ {}
        :generics $ [] 'T
        :args $ [] 'T
        :return :string
    Person0 $ defstruct Person (:name :string)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p)
        str-spaced |foo (:name p)
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  .foo p
```

### Impl-related syntax (cheatsheet)

**1) `defimpl` argument order (breaking change)**

```
defimpl ImplName Trait ...
```

- First argument is the **impl record name**.
- Second argument is the **trait value** (symbol) or a **tag**.

Examples:

```cirru
do
  ; Form 1: symbol names for impl and trait
  let
      PersonA0 $ defstruct PersonA (:name :string)
      MyFooA $ deftrait MyFooA
        .foo $ :: :fn $ {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
      MyFooImplA $ defimpl MyFooImplA MyFooA
        .foo $ fn (p) (str-spaced |foo (:name p))
      PersonA $ impl-traits PersonA0 MyFooImplA
      p $ %{} PersonA (:name |Alice)
    .foo p
  ; Form 2: tag keywords for impl and trait (no deftrait needed)
  let
      PersonB0 $ defstruct PersonB (:name :string)
      MyFooImplB $ defimpl :MyFooImplB :MyFooB
        .foo $ fn (p) (str-spaced |bar (:name p))
      PersonB $ impl-traits PersonB0 MyFooImplB
      p $ %{} PersonB (:name |Bob)
    .foo p
```

**2) Method pair forms**

Prefer dot-style keys (`.foo`). Legacy tag keys (`:foo`) are still accepted for compatibility.

```cirru
; Both forms are accepted and equivalent:
do
  let
      MyFoo $ deftrait MyFoo
        .foo $ :: :fn $ {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
      Person0 $ defstruct Person (:name :string)
      ; Form 1: preferred .method keys
      ImplA $ defimpl ImplA MyFoo
        .foo (fn (p) (str |A: (:name p)))
      PersonA $ impl-traits Person0 ImplA
      pa $ %{} PersonA (:name |Alice)
    .foo pa
  let
      MyFoo $ deftrait MyFoo
        .foo $ :: :fn $ {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
      Person0 $ defstruct Person (:name :string)
      ; Form 2: legacy tag keys (compatible)
      ImplB $ defimpl ImplB MyFoo
        :: :foo (fn (p) (str |B: (:name p)))
      PersonB $ impl-traits Person0 ImplB
      pb $ %{} PersonB (:name |Bob)
    .foo pb
```

**3) Tag-based impl (no concrete trait value)**

If you need a pure marker and don’t want to bind to a real trait value, use tags:

```cirru
defimpl :MyMarkerImpl :MyMarker
  .dummy nil
```

This is also a safe replacement for the old self-referential pattern
`defimpl X X`, which can cause recursion in new builds.

Implementation notes:

- `defimpl` creates an “impl record” that stores the trait as its origin.
- This origin is used by `&trait-call` to match the correct implementation when method names overlap.

## Attach impls to struct/enum definitions

`impl-traits` attaches impl records to a **struct/enum type**. For user values, later impls override earlier impls for the same method name ("last-wins").

Constraints:

- `impl-traits` only accepts **struct/enum** values.
- Record/tuple instances must be created from a struct/enum that already has impls attached (`%{}` or `%::`).

Syntax:

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn $ {}
        :generics $ [] 'T
        :args $ [] 'T
        :return :string
    ImplA $ defimpl ImplA MyFoo
      .foo $ fn (p) (str |A: (:name p))
    ImplB $ defimpl :ImplB :ImplB-trait
      .bar $ fn (p) (str |B: (:name p))
    StructDef0 $ defstruct StructDef (:name :string)
    StructDef $ impl-traits StructDef0 ImplA ImplB
    x $ %{} StructDef (:name |test)
  .foo x
```

### Public vs internal API boundary

- Prefer public API in app/library code: `deftrait`, `defimpl`, `impl-traits`, `.method`, `&trait-call`.
- Treat internal `&...` helpers as runtime-level details; they may change more frequently and are not the stable user contract.

```cirru
do
  ; struct example
  let
      MyFoo $ deftrait MyFoo
        .foo $ :: :fn $ {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
      MyFooImpl $ defimpl MyFooImpl MyFoo
        .foo $ fn (p) (str-spaced |foo (:name p))
      Person0 $ defstruct Person (:name :string)
      Person $ impl-traits Person0 MyFooImpl
      p $ %{} Person (:name |Alice)
    .foo p
  ; enum example
  let
      ResultTrait $ deftrait ResultTrait
        .describe $ :: :fn $ {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
      ResultImpl $ defimpl ResultImpl ResultTrait
        .describe $ fn (x)
          tag-match x
            (:ok v) (str |ok: v)
            (:err v) (str |err: v)
      Result0 $ defenum Result0 (:ok :string) (:err :string)
      MyResult $ impl-traits Result0 ResultImpl
      r $ %:: MyResult :ok |done
    .describe r
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

`&trait-call` matches by the impl record's trait origin, not just by trait name text. This avoids accidental dispatch when two different trait values share the same printed name.

Example with two traits sharing the same method name:

```cirru
let
    MyZapA $ deftrait MyZapA
      .zap $ :: :fn $ {}
        :generics $ [] 'T
        :args $ [] 'T
        :return :string
    MyZapB $ deftrait MyZapB
      .zap $ :: :fn $ {}
        :generics $ [] 'T
        :args $ [] 'T
        :return :string
    MyZapAImpl $ defimpl MyZapAImpl MyZapA
      .zap $ fn (_x) |zapA
    MyZapBImpl $ defimpl MyZapBImpl MyZapB
      .zap $ fn (_x) |zapB
    Person0 $ defstruct Person (:name :string)
    Person $ impl-traits Person0 MyZapAImpl MyZapBImpl
    p $ %{} Person (:name |Alice)
  ; .zap follows normal dispatch (last-wins for user impls)
  .zap p
  ; explicitly pick a trait’s implementation
  &trait-call MyZapA :zap p
  &trait-call MyZapB :zap p
```

## Debugging / introspection

Two helpers are useful when debugging trait + method dispatch:

- `&methods-of` returns a list of available method names (strings, including the leading dot).
- `&inspect-methods` prints impl records and methods to stderr, and returns the value unchanged.
- `&impl:origin` returns the trait origin stored on an impl record (or nil).

```cirru
let
    xs $ [] 1 2
  &methods-of xs
  &inspect-methods xs "|list"
```

You can also inspect impl origins directly when validating trait dispatch:

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn ('T) ('T) :string
    Shape0 $ defenum Shape (:point :number :number)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (t) (str |shape: (&tuple:nth t 0))
    Shape $ impl-traits Shape0 MyFooImpl
    some-tuple $ %:: Shape :point 10 20
    impls $ &tuple:impls some-tuple
  any? impls $ fn (impl)
    = (&impl:origin impl) MyFoo
```

## Checking trait requirements

`assert-traits` checks at runtime that a value implements a trait (i.e. it provides all required methods). It returns the value unchanged if the check passes.

Notes:

- `assert-traits` is syntax (expanded to `&assert-traits`) and its first argument must be a **local**.
- For built-in values (list/map/set/string/number/...), `assert-traits` only validates default implementations. It **does not** extend methods at runtime.
- Static analysis and runtime checks may diverge for built-ins due to limited compile-time information; this mismatch is currently allowed.

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn $ {}
        :generics $ [] 'T
        :args $ [] 'T
        :return :string
    Person0 $ defstruct Person (:name :string)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p) (str-spaced |foo (:name p))
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  assert-traits p MyFoo
  .foo p
```

### Examples (verified with `cr eval`)

```bash
cargo run --bin cr -- demos/compact.cirru eval 'let ((xs ([] 1 2 3))) (assert= xs (assert-traits xs calcit.core/Len)) (.len xs)'
```

Expected output:

```text
3
```

```bash
cargo run --bin cr -- demos/compact.cirru eval 'let ((xs ([] 1 2 3))) (assert= xs (assert-traits xs calcit.core/Mappable)) (.map xs inc)'
```

Expected output:

```text
([] 2 3 4)
```
