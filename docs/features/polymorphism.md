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

For capability-based dispatch via struct/enum-attached impls (used by records/tuples created from them), see [Traits](traits.md).

Historically, the idea was inspired by JavaScript, and also [borrowed from a trick of Haskell](https://www.well-typed.com/blog/2018/03/oop-in-haskell/) (simulating OOP with immutable data structures). The current model is trait-based.

## Quick Recipes

- **Define Trait**: `deftrait Show .show (:: :fn $ {} ...)`
- **Implement**: `defimpl ShowImpl Show .show (fn (x) ...)`
- **Attach**: `impl-traits MyStruct ShowImpl`
- **Call**: `.show instance`

## Key terms

- **Trait**: A named capability with method signatures (defined by `deftrait`).
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
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p)
        str "|foo " $ :name p
    Person0 $ defstruct Person (:name :string)
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  .foo p
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
      .show $ fn (p)
        str |Person: $ :name p
    EqImpl $ defimpl EqImpl EqTrait
      .eq $ fn (p)
        str |eq: $ :name p
    MyFooImpl $ defimpl MyFooImpl MyFoo
      .foo $ fn (p)
        str |foo: $ :name p
    Person $ impl-traits Person0 ShowImpl EqImpl MyFooImpl
    p $ %{} Person (:name |Alice)
  [] (.show p) (.foo p)
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
      .foo $ fn (p)
        str-spaced |foo $ :name p
    Person $ impl-traits Person0 MyFooImpl
    p $ %{} Person (:name |Alice)
  assert-traits p MyFoo
  .foo p
```

If the trait is missing or its selected impl is incomplete, `assert-traits` raises an error. Methods are not combined across unrelated impls: implementing `TraitA/.render` never satisfies `TraitB/.render`.

`defimpl` validates concrete trait implementations when they are created: missing or extra methods and non-callable values are rejected, and native preprocessing compares typed method signatures when metadata is available. Passing a tag as the trait argument remains a legacy way to create an inherent method bag; `cr edit format` reports `W_LEGACY_INHERENT_IMPL` because that value cannot satisfy `assert-traits`, `:where`, or `&trait-call`. The advisory is non-blocking, so compatible `.method` code continues to run.

## Generic `:where` bounds on functions

When a function is generic but still requires trait capabilities, keep type variables in `:generics` and place trait constraints in `:where`.

This mirrors the main reason Rust has `where` clauses: parameter declaration and bound declaration stay separate, and multiple constraints remain readable instead of being packed into the first mention of the type variable.

Top-level definitions use `:schema`:

```cirru.no-run
%{} :CodeEntry
  :code $ quote
    defn show-it (x) (.show x)
  :schema $ :: :fn
    {}
      :generics $ [] 'T
      :where $ {} ('T Show)
      :args $ [] 'T
      :return :string
```

Local functions use `hint-fn` with the same shape:

```cirru
let
    show-it $ fn (x)
      hint-fn $ {}
        :generics $ [] 'T
        :where $ {} ('T Show)
        :args $ [] 'T
        :return :string
      .show x
  show-it 1
```

For multiple constraints on the same variable, use a list value:

```cirru
:where $ {}
  'T $ [] Show Eq
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
cr analyze weak-types --only schema-dynamic,code-dynamic --intent unresolved --summary-only
```

If the summary reports debt, rerun `weak-types` without `--summary-only` and scope it with `--ns` or `--ns-prefix`. Each occurrence explains whether it affects generic substitution, callback checking, container element propagation, or compile-time method specialization.

## Built-in traits

Core types provide origin-carrying built-in trait implementations registered consistently by native and JS runtimes. The capability-focused traits available for generic `:where` bounds include:

| Trait | Method | Built-in value categories |
| --- | --- | --- |
| `Compare` | `.compare` | Number, String |
| `Countable` | `.count` | List, Map, Set, String, Record, Tuple/enum |
| `Contains` | `.contains?` | List, Map, Set, String, Record, Tuple/enum |
| `Mappable` | `.map` | List, Map, Set, Option, Result |
| `Show` | `.show` | Number, String, Bool, Tag, Symbol, Nil, CirruQuote, List, Map, Set, Fn, Record, Tuple |
| `Eq` | `.eq?` | The same scalar/collection/record/tuple categories registered for `Show` |

`Compare` returns a negative number, zero, or a positive number. It intentionally starts with Number and String; cross-category ordering is not defined.

```cirru
do
  assert= -1 $ .compare 1 2
  assert= 0 $ .compare |same |same
```

## Option and Result helpers

`Option T` and `Result T E` are generic core enums. Their constructors remain `%some`/`%none` and `%ok`/`%err`; the following helpers make normal pipelines explicit without losing type relationships:

- Option: `option:some?`, `option:none?`, `option:map`, `option:unwrap-or`, `option:and-then`
- Result: `result:ok?`, `result:err?`, `result:map`, `result:map-err`, `result:unwrap-or`, `result:and-then`

The same operations are available as methods on enum values:

```cirru
do
  assert= 0 $ .unwrap-or (%none) 0
  assert= (%ok 4)
    .and-then (%ok 2)
      fn (x)
        %ok $ * x 2
  assert= (%err |failed!)
    .map-err (%err |failed)
      fn (e) (str e |!)
```

## Notes

- There is no inheritance. Behavior sharing is done via traits and `impl-traits`.
- Method calls resolve through attached trait impls first, then built-in implementations.
- `.method` remains intentionally name-based and follows impl precedence; use `&trait-call` when the trait identity itself is part of the contract.
- Use `assert-traits` when a function relies on trait methods and you want early, clear failures.

## Further reading

- Dev log(中文) https://github.com/calcit-lang/calcit/discussions/44
- Dev log in video(中文) https://www.bilibili.com/video/BV1Ky4y137cv
