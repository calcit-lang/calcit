---
title: "Polymorphism"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "trait dispatch"
  - "method dispatch"
  - "impl-traits"
entry_for:
  - "impl-traits"
  - "trait-call"
  - "assert-traits"
---
# Polymorphism

Calcit models polymorphism with traits. Traits define method capabilities and can be attached to struct/enum definitions with `impl-traits`.

For capability-based dispatch via struct/enum-attached impls (used by struct/enum values created from them), see [Traits](traits.md).

Historically, the idea was inspired by JavaScript, and also [borrowed from a trick of Haskell](https://www.well-typed.com/blog/2018/03/oop-in-haskell/) (simulating OOP with immutable data structures). The current model is trait-based.

## Quick Recipes

- **Define Trait**: `deftrait Show .show (:: :fn $ {} ...)`
- **Implement**: `defimpl ShowImpl Show .show (fn (x) ...)`
- **Attach**: `impl-traits MyStruct ShowImpl`
- **Call**: `instance .show` (receiver-first; `.show instance` remains compatible)

## Key terms

- **Trait**: A named capability with method signatures (defined by `deftrait`).

## `Debug` and `Show`

Calcit separates diagnostic rendering from user-facing presentation:

- `Debug` / `.debug` is implemented by every built-in Calcit value. It is the
  faithful diagnostic representation, intended for logging and inspection.
- `Show` / `.show` is deliberately opt-in. A struct or enum only receives it
  after an explicit `defimpl ... calcit.core/Show`; calling `.show` on a
  statically known value without that implementation is a type error.

Use `Debug` for polymorphic helpers that must work with ordinary values. Use
`Show` only when the API promises a curated presentation format.
- **Trait impl**: An impl carrying the exact trait value as origin and providing its complete method set.
- **Inherent method bag**: A compatibility impl without trait origin. It participates only in ordinary `.method` lookup and proves no trait bound.
- **impl-traits**: Attaches one or more trait impl records to a struct/enum definition.
- **assert-traits**: Adds a compile-time hint and performs a nominal runtime check for that exact trait origin.

## Define a trait

```cirru
let
    DemoShow $ deftrait DemoShow
      .show $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
    DemoEq $ deftrait DemoEq
      .eq? $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T 'T
          :return :bool
  [] DemoShow DemoEq
```

Traits are values and can be referenced like normal symbols.

## Implement a trait for a struct/enum definition

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
    Person0 $ defstruct Person (:name :string)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p) (assert-type p Person0)
        str "|foo " $ :name p
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  p .foo
```

`impl-traits` returns a new struct/enum definition with trait implementations attached. You can also attach multiple traits at once:

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
    ShowTrait $ deftrait ShowTrait
      .show $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
    EqTrait $ deftrait EqTrait
      .eq $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
    Person0 $ defstruct Person (:name :string)
    ShowImpl $ defimpl ShowImpl ShowTrait
      .show $ fn (p) (assert-type p Person0)
        str |Person: $ :name p
    EqImpl $ defimpl EqImpl EqTrait
      .eq $ fn (p) (assert-type p Person0)
        str |eq: $ :name p
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p) (assert-type p Person0)
        str |foo: $ :name p
    Person $ impl-traits Person0 ShowImpl EqImpl MyFooImpl
    p $ %{} Person (:name |Alice)
  [] (p .show) (p .foo)
```

## Trait checks and type hints

`assert-traits` marks a local as having a trait and validates it at runtime:

```cirru
let
    MyFoo $ deftrait MyFoo
      .foo $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T
          :return :string
    Person0 $ defstruct Person (:name :string)
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p) (assert-type p Person0)
        str-spaced |foo $ :name p
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  assert-traits p MyFoo
  p .foo
```

If the trait is missing or its selected impl is incomplete, `assert-traits` raises an error. Methods are not combined across unrelated impls: implementing `TraitA/.render` never satisfies `TraitB/.render`.

`defimpl` validates concrete trait implementations when they are created: missing or extra methods and non-callable values are rejected, and native preprocessing compares typed method signatures when metadata is available. Passing a tag as the trait argument remains a legacy way to create an inherent method bag; `cr edit format` reports `W_LEGACY_INHERENT_IMPL` because that value cannot satisfy `assert-traits`, `:where`, or `&trait-call`. The advisory is non-blocking, so compatible `.method` code continues to run.

## Generic `:where` bounds on functions

When a function is generic but still requires trait capabilities, keep type variables in `:generics` and place trait constraints in `:where`.

This mirrors the main reason Rust has `where` clauses: parameter declaration and bound declaration stay separate, and multiple constraints remain readable instead of being packed into the first mention of the type variable.

Top-level definitions use `:schema`:

```cirru.edn
%{} :CodeEntry
  :code $ quote
    defn show-it (x) (x .show)
  :schema $ :: 'Fn
    {}
      :generics $ [] 'T
      :where $ {} ('T 'Show)
      :args $ [] 'T
      :return 'String
```

Local functions use `hint-fn` with the same shape:

```cirru
let
    show-it $ fn (x)
      hint-fn $ {}
        :generics $ [] 'T
        :where $ {} ('T Show)
        :args $ [] 'T
        :return 'String
      x .show
  ; `show-it` requires an explicit Show implementation at its call site.
```

For multiple constraints on the same variable, use a list value:

```cirru.edn
{}
  :where $ {}
    'T $ [] 'Show 'Eq
```

Do not use the old tuple form like `:where $ [] (:: 'Show 'T)`.

## Keep polymorphic relationships visible

`:dynamic` means that static checking has stopped at that slot. Repeating it in an argument and return position does not say that both positions have the same type:

```cirru.no-run
do
  :: :fn $ {}
    :args $ [] :dynamic
    :return :dynamic
  :: :fn $ {}
    :generics $ [] 'T
    :args $ [] 'T
    :return 'T
```

Use a type variable for identity-like relationships, add `:where` when that variable only needs a capability, and keep type arguments on containers such as `:: :list 'T` and `:: :map 'K 'V`. Use a named enum for a finite set of alternatives. Reserve `:dynamic` for boundaries whose shape is genuinely open, such as unvalidated FFI input. `:any` is only a legacy spelling of `:dynamic`, not another polymorphism mechanism.

Agents can check how much information survives without running the program:

```bash
cr analyze check-types --summary-only
cr analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --summary-only
```

If the summary reports debt, rerun `weak-types` without `--summary-only` and scope it with `--ns` or `--ns-prefix`. Each occurrence explains whether it affects generic substitution, callback checking, container element propagation, or compile-time method specialization.

## Built-in traits

Core types provide origin-carrying built-in trait implementations registered consistently by native and JS runtimes. The capability-focused traits available for generic `:where` bounds include:

| Trait | Method | Built-in value categories |
| --- | --- | --- |
| `Compare` | `.compare` | Number, String |
| `Countable` | `.count` | List, Map, Set, String, Struct, Enum |
| `Contains` | `.contains?` | List, Map, Set, String, Struct, Enum |
| `Mappable` | `.map` | List, Map, Set, Option, Result |
| `Debug` | `.debug` | Number, String, Bool, Tag, Symbol, Nil, CirruQuote, List, Map, Set, Fn, Struct, Enum |
| `Show` | `.show` | Explicit user implementations only |
| `Eq` | `.eq?` | The same scalar, collection, struct, and enum categories registered for `Debug` |

`Compare` returns a negative number, zero, or a positive number. It intentionally starts with Number and String; cross-category ordering is not defined.

```cirru
do
  assert= -1 $ 1 .compare 2
  assert= 0 $ |same .compare |same
```

## Option and Result helpers

`Option T` and `Result T E` are generic core enums. Their constructors remain `%some`/`%none` and `%ok`/`%err`; use inferred methods for normal pipelines without losing type relationships:

- Option: `.some?`, `.none?`, `.map`, `.unwrap`, `.unwrap-or`, `.and-then`, `.fold`
- Result: `.ok?`, `.err?`, `.map`, `.map-err`, `.unwrap-or`, `.and-then`

For a statically known receiver, preprocessing resolves these methods and lowers
them to their internal direct implementations. Direct names such as
`option:unwrap-or` and `result:unwrap-or` are core implementation details, not
the public call style.

`optionally` exists only for legacy core/internal `Optional<T>` compatibility. Public function schemas reject `Optional<T>`; new APIs return `Option<T>`, `Result<T,E>`, or `Unit` directly.

Core lookup APIs that no longer need to preserve bootstrapping compatibility use nominal results directly:

- `find`, `find-index`, `find-last`, `find-last-index`, `index-of`, and `last-index-of` return `Option`.
- `get-in` returns `Option<Dynamic>` while preserving a more precise payload type for literal paths when inference can resolve it.
- List/set `max` and `min` return `Option<Number>` so empty collections are explicit.
- String `.find-index` and `str-find-index` return `Option<Number>`; the internal `&str:find-index` primitive retains its `-1` ABI sentinel.
- `get-env` returns `Option<String>`; use `.unwrap-or` for a default.
- `parse-float` returns `Result<Number,String>`, with the invalid source in `:err`.
- Reflection uses `enum-definition: Enum -> Option<EnumDef>`, `struct-definition: Struct -> Option<StructDef>`, and `impl-origin: Impl -> Option<Trait>`.
- `destruct-list`, `destruct-map`, `destruct-set`, and `destruct-str` return named `*Destruct` enums, preserving the familiar `:some`/`:none` branches with checked payloads.
- Public collection methods follow the same contract: Map/Set `.destruct` return their named destruct enums. Struct does not expose `.nth`, because field position is not stable across backends; field-name `get` returns the field's declared type directly.
- `when-let` consumes `Option<T>` and returns `Option<R>`; `update-in` passes `Option<T>` to its updater so a missing leaf is never represented by nil.

Raw JavaScript property reads and native calls are different: they return `JsNullish<JsObject>`. Narrow them with `js-present?`/`js-nullish?`; `nil?`, `some?`, and generic `optionally` do not erase this host boundary. Use `js-nullish->option` only as an explicit conversion after accepting or validating the opaque payload contract.

When a generic payload cannot be inferred, Calcit keeps the nominal wrapper and uses `Dynamic` only for the unknown payload—for example, `find` over a dynamically typed list is still `Option<Dynamic>`, not plain `Dynamic`. This makes migration mistakes visible. Using nullable predicates (`some?`/`nil?`), positional enum access, or raw comparison on that Option reports `W_NOMINAL_ENUM_LEGACY_USE`; switch to Option methods or `tag-match`.

The same operations are available as methods on enum values:

```cirru
do
  assert= (%some 1) $ optionally 1
  assert= (%none) $ optionally nil
  assert= (%some 2) $ find ([] 1 2 3) (fn (x) (> x 1))
  assert= (%ok 1.5) $ parse-float |1.5
  assert= |fallback $ (get-env |__MISSING_ENV__) .unwrap-or |fallback
  assert= 0 $
    %none
    , .unwrap-or 0
  assert= (%ok 4)
    (%ok 2) .and-then $ fn (x)
      %ok $ * x 2
  assert= (%err |failed!)
    (%err |failed) .map-err $ fn (e) (str e |!)
```

## Notes

- There is no inheritance. Behavior sharing is done via traits and `impl-traits`.
- Method calls resolve through attached trait impls first, then built-in implementations.
- `.method` remains intentionally name-based and follows impl precedence; use `&trait-call` when the trait identity itself is part of the contract.
- Use `assert-traits` when a function relies on trait methods and you want early, clear failures.

## Further reading

- Dev log(中文) https://github.com/calcit-lang/calcit/discussions/44
- Dev log in video(中文) https://www.bilibili.com/video/BV1Ky4y137cv
