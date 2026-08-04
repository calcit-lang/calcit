---
title: "Static Type Analysis"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "type check"
  - "type warning"
  - "assert-type"
  - "compile-time checks"
  - "weak types"
entry_for:
  - "assert-type"
  - "cr analyze check-types"
  - "cr analyze weak-types"
id: core/features/static-analysis
related:
  - core/run/library-quality
  - core/run/upgrade
---

# Static Type Analysis

Calcit includes a built-in static type analysis system that performs compile-time checks to catch common errors before runtime. This system operates during the preprocessing phase and provides warnings for type mismatches and other potential issues.

## Quick Recipes

- **Assert Type**: `assert-type total 'Number`
- **Local `fn` Hint**: `hint-fn $ {} (:args ([] 'Number)) (:return 'Number)`
- **Top-level `defn` Schema**: `cr edit schema app.main/add --code "quote $ :: 'Fn $ {} (:args ([] 'Number 'Number)) (:return 'Number)"`
- **Top-level value Schema**: `cr edit schema 'app.main/*enabled?' --code "quote $ :: 'Ref 'Bool"`
- **Return Type**: `hint-fn $ {} (:return 'String)`
- **Compact Hint**: `defn my-fn (x) 'String ...`
- **Check Traits**: `assert-traits x MyTrait`
- **Ignore Warning**: `&core:ignore-type-warning`

## Overview

The static analysis system provides:

- **Type inference** - Automatically infers types from literals and expressions
- **Type annotations** - Optional type hints for function parameters and return values
- **Compile-time warnings** - Catches errors before code execution
- **Composable runtime assertions** - `assert-type` and `assert-traits` can validate values at runtime and return original values for chaining

## Static Project Reports

Use the CLI reports when you need to understand type quality without running the application:

```bash
# Coverage by definition; unknown code is reported as none, never as full
cr analyze check-types --ns app.main

# All weak type locations
cr analyze weak-types --ns app.main

# Focus only on unresolved type debt
cr analyze weak-types --ns app.main --intent unresolved

# Focus on nil migration debt while excluding declared Unit returns
cr analyze weak-types --ns app.main --only code-nil --intent unresolved,declared-optional

# Inspect explicitly permitted JS FFI dynamic boundaries
cr analyze weak-types --ns app.main --intent intentional-js-ffi

# Machine-readable definition rows and Snapshot paths
cr analyze check-types --ns app.main --format json
cr analyze weak-types --ns app.main --intent unresolved --format json

# Keep aggregate counts but omit definition rows (especially useful for agents)
cr analyze check-types --ns app.main --summary-only --format json
cr analyze weak-types --ns app.main --intent unresolved --summary-only --format json

# Validate only one definition's examples
cr analyze check-examples --ns app.main --def calculate-total

# Explain one expression using inferred and expected types
cr query type-at app.main/calculate-total --path code@3.2 --format json
```

`check-types` treats nested dynamic slots such as bare `:ref`, `:list`, or `:map` as partial coverage and includes actionable `[W_SCHEMA_DYNAMIC]` entries in `schema_issues`. When partial/none definitions exist, human output adds an `agent-note` and JSON emits `W_TYPE_COVERAGE_GAPS`. `weak-types --format json` reports the exact Snapshot/schema path plus an `impact` and `suggestion` for every occurrence; unresolved dynamic debt emits `W_DYNAMIC_TYPE_DEBT`, while unresolved or compatibility-Optional nil debt emits `W_NIL_TYPE_DEBT`. Definitions marked with the explicit `:js-ffi` feature remain classified as intentional boundaries rather than ordinary unresolved dynamic debt.

An explicit function schema feature such as `:features $ #{} :js-ffi` classifies dynamic schema/code occurrences as `intentional-js-ffi`. It does not hide them: the report keeps the locations visible while separating them from unresolved dynamic types. The feature does not classify `nil`, because an FFI capability does not imply that every nullable branch is intentional.

For `code-nil`, the report uses the declared return contract only at structurally proven return positions. A final `nil` under a returned `do`, or either returned branch of `if`, is `declared-unit` for a `Unit` return and `declared-optional` for an `Optional<T>` return. Other nil literals stay `unresolved`; in particular, an earlier `do` step does not inherit the enclosing return contract. `declared-unit` records a legitimate no-value result and is excluded from nil migration debt. `declared-optional` remains visible as compatibility debt so application APIs can move toward `Option` or `Result`.

For one definition, `cr query context '<ns/def>' --format json` embeds the same distinction in its diagnostics and returns the definition revision together with Snapshot paths.

For one expression, `cr query type-at '<ns/def>' --path code@... --format json` preprocesses only static program metadata and returns inferred type, expected type, typed bindings, confidence, method candidates, and diagnostics. It does not run the application entry. Paths use the same stable Snapshot coordinates returned by structural query commands.

Both analysis commands run as static Snapshot readers: they load configured modules and core metadata but do not preprocess or execute the application entry. With `--format json`, stdout is one versioned JSON envelope containing a stable scope revision, filters, summary, and definition-level rows; startup/command messages stay on stderr.

Use `--summary-only` when only aggregate counts are needed. Human output stops after the aggregate section; JSON keeps `data.summary` and the scope revision while returning an empty `data.definitions` array. `defstruct`, `defenum`, and `deftrait` carry type information in their declarations, so they are classified as data declarations instead of receiving a false top-level `schema-dynamic` finding.

`check-examples` reports pass/fail and elapsed time without printing the final example value, which can be a very large function, record, or component tree. Output explicitly produced by an example is still shown.

## Type Annotations

Built-in types use **quoted symbols**: write `'String`, `'Number`, `'List`, `'Fn`, and `'Dynamic`. This keeps type syntax distinct from ordinary keyword/tag data. Lowercase tags such as `:string`, `:number`, and `:dynamic` remain load-compatible, but `cr edit format` rewrites type positions to the symbol form. It does not rewrite ordinary tags such as enum variants, record keys, `:return` schema keys, or `:kind` values.

### Function Parameter Types

Function parameters should be annotated with function schema:

- top-level `defn` / `defmacro`: prefer `:schema`
- local `fn`: use `hint-fn` with `:args` / `:rest`

For namespace-level definitions, `:schema` is stored on the definition entry and is typically edited with `cr edit schema`, rather than written inline in the function body.

`cr edit schema` accepts exactly one AST node and therefore requires the CLI code/data boundary: use `quote 'String` for a primitive leaf or `quote $ :: 'Ref 'Bool` for a parameterized type expression. A top-level value backed by `defstruct` or `defenum` uses its fully qualified nominal type, for example `cr edit schema app.schema/store --code "quote 'app.schema/Store"`; the qualification lets the stored schema preserve identity without relying on the editing namespace. The `quote` belongs to CLI transport and is not stored inside `:schema`. Callable payloads use the canonical wrapped form `:: 'Fn $ {} ...` or `:: 'Macro $ {} ...`; raw `{} (:kind :fn)` maps and bare parameterized types such as `'Ref` are rejected with a corrective error. Parameterized value schemas use the same type grammar as function arguments, for example `:: 'Ref 'Bool`, `:: 'List 'String`, or `:: 'Map 'Tag 'Number`.

The preprocessor propagates a named function's schema into its parameter bindings. This means field access, method dispatch, generic return inference, and return checks inside the body use the declared types instead of falling back to `:dynamic`. A `:rest` schema is preserved as a variadic element type both for calls and when the function is passed as a higher-order callback.

`assert-type` is still useful, but mainly for local variables, intermediate values, and explicit checks inside the function body.

Runnable Example:

```cirru
let
    calculate-total $ fn (items)
      hint-fn $ {}
        :args $ [] :list
        :return :number
      reduce items 0 $ fn (acc item)
        hint-fn $ {}
          :args $ [] :number :number
          :return :number
        + acc item
  calculate-total $ [] 1 2 3
```

### Return Type Annotations

There are two ways to specify return types:

#### 1. Local `fn` Hint (`hint-fn`)

Use `hint-fn` with schema map at the start of a local function body:

When a function is already bound, the two-argument form refines that local binding for all later expressions in the same lexical scope:

```cirru
let
    add-one $ fn (x) + x 1
  hint-fn add-one $ {}
    :args $ [] :number
    :return :number
  assert-type (add-one 1) :number
```

This refinement is static metadata; it does not execute the function. Put it before calls that should use the signature. The one-argument form below remains the preferred form inside the function itself.

A body hint may declare only part of the signature. Omitted `:args` slots are aligned with the function's real parameters and remain `:dynamic`; they are not interpreted as a zero-argument function. This lets a return-only hint improve downstream inference without inventing parameter constraints.

Legacy clause syntax such as `(hint-fn (return-type ...))`, `(generics ...)`, and `(type-vars ...)` is no longer supported and now fails during preprocessing.

Generic trait bounds use the same schema map. Follow the Rust-style split: declare variables in `:generics`, then put trait constraints in `:where`.

```cirru
let
    get-name $ fn (user)
      hint-fn $ {}
        :args $ [] :dynamic
        :return :string
      , |demo
  get-name nil
```

```cirru
let
    print-it $ fn (x)
      hint-fn $ {}
        :generics $ [] 'T
        :where $ {} ('T Show)
        :args $ [] 'T
        :return 'String
      x .show
  print-it 1
```

Do not use the old tuple/list form such as `:where $ [] (:: 'Show 'T)`.

#### 2. Compact Hint (Trailing Label)

For `defn` and `fn`, you can place a type label immediately after the parameters:

```cirru
let
    add $ fn (a b) :number (+ a b)
  add 10 20
```

For namespace-level `defn` / `defmacro`, parameter and return metadata should still live in `:schema`.

### Multiple Annotations

```cirru
let
    add $ fn (a b) :number
      hint-fn $ {}
        :args $ [] :number :number
        :return :number
      let
          total $ + a b
        assert-type total :number
        , total
  assert= 3 $ add 1 2
```

## Supported Types

| Canonical syntax | Calcit Type |
| ---------------- | ----------- |
| `'Unit` | Nil / unit |
| `'Bool` | Boolean |
| `'Number` | Number |
| `'String` | String |
| `'Symbol` | Symbol |
| `'Tag` | Tag (Keyword) |
| `'List` | List |
| `'Map` | Hash Map |
| `'Set` | Set |
| `'Tuple` | Tuple (general) |
| `'Fn` | Function |
| `'Ref` | Atom / Ref |
| `'Dynamic` | Unknown/unresolved type; static checks are disabled at this boundary |

`:any` is a legacy alias for `:dynamic`; both are accepted as input and formatter output is `'Dynamic`.

### Complex Types

#### Optional Types

Represent values that can be `nil`. Use the `:: 'Optional <type>` syntax:

```cirru
let
    greet $ fn (name)
      hint-fn $ {}
        :args $ [] (:: :optional :string)
        :return :string
      str "|Hello " $ or name |Guest
  greet nil
```

#### Variadic Types

Represent variable arguments in `&` parameters:

```cirru
let
    sum $ fn (& xs)
      hint-fn $ {} (:rest :number) (:return :number)
      reduce xs 0 &+
  sum 1 2 3
```

A variadic function can satisfy a fixed-arity callback when its required parameters and rest element type accept every argument promised by the callback. Callback parameter matching is contravariant, while callback return matching is covariant; for example, a callback accepting `optional<T>` can be used where the caller only supplies `T`.

`:any` is accepted only as a compatibility spelling and is parsed as `:dynamic`. Schema serialization, generated metadata, type queries, and diagnostics use `:dynamic`; new code should not introduce `:any`.

Do not strengthen a schema beyond the runtime contract merely to silence a report. Instead, preserve the relationship that the code actually promises: use a declared type variable when input and output share a type, a trait plus `:where` when only capabilities matter, a parameterized collection/ref for homogeneous values, and a named enum for finite heterogeneous alternatives. Retain `:dynamic` only when the type is genuinely unavailable, such as an unresolved JS FFI/global-state or macro boundary, and keep that boundary explicit and narrow.

### Preserving Polymorphism Instead of Repeating Dynamic

This schema loses the fact that the result has the same type as the input:

```cirru.no-run
:: :fn $ {}
  :args $ [] :dynamic
  :return :dynamic
```

Use a declared type variable:

```cirru.no-run
:: :fn $ {}
  :generics $ [] 'T
  :args $ [] 'T
  :return 'T
```

If the body only needs a capability, add a trait bound rather than replacing `'T` with dynamic:

```cirru.no-run
:: :fn $ {}
  :generics $ [] 'T
  :where $ {} ('T Show)
  :args $ [] 'T
  :return :string
```

The same rule applies inside containers and callbacks: `:: :list 'T` preserves a homogeneous item relationship, while bare `:list` means `list<dynamic>`; a complete `:: :fn` callback schema preserves argument/return checking, while bare `:fn` does not.

#### Record and Enum Types

Use the name defined by `defstruct` or `defenum`:

```cirru
let
    User $ defstruct User (:name :string)
    get-name $ fn (u)
      hint-fn $ {}
        :args $ [] 'User
        :return :string
      get u :name
  get-name $ %{} User (:name |Alice)
```

## Built-in Type Checks

### Function Arity Checking

The system validates that function calls have the correct number of arguments:

```cirru
defn greet (name age) (str "|Hello " name "|, you are " age)

; Error: expects 2 args but got 1

; greet |Alice
```

### Record Field Access

Validates that record fields exist:

```cirru
defstruct User (:name :string) (:age :number)

defn get-user-email (user) (.-email user) (; Warning: field 'email' not found in record User) (; Available fields: name, age)
```

### Tuple Index Bounds

Checks tuple index access at compile time:

```cirru.no-check
let
    point $ %:: :Point 10 20 30
  &tuple:nth point 5 ; Warning: index 5 out of bounds, tuple has 4 elements
```

### Enum Variant Validation

Validates enum construction and pattern matching:

```cirru.no-check
defenum Result (:Ok :number) (:Error :string)

; Warning: variant 'Failure' not found in enum Result

%:: Result :Failure "|something went wrong"

; Available variants: Ok, Error

; Warning: variant 'Ok' expects 1 payload but got 2

%:: Result :Ok 42 |extra
```

### Method Call Validation

Checks that methods exist for the receiver type:

```cirru
defn process-list (xs) (; .unknown-method xs) (println "|demo code") (; "Warning: unknown method .unknown-method for :list") (; Available methods: .map, .filter, .count, ...)
```

### Recur Arity Checking

Validates that `recur` calls have the correct number of arguments:

```cirru
defn factorial (n acc)
  if (<= n 1) acc $ recur (dec n) (* n acc)
  ; Warning: recur expects 2 args but got 3
  ; recur (dec n) (* n acc) 999
```

**Note**: Recur arity checking automatically skips:

- Functions with variadic parameters (`&` rest args)
- Functions with optional parameters (`?` markers)
- Macro-generated functions (e.g., from `loop` macro)
- `calcit.core` namespace functions

## Type Inference

The system infers types from various sources:

### Literal Types

```cirru
let
    ; inferred as :number
    x 42
    ; inferred as :string
    y |hello
    ; inferred as :bool
    z true
    ; inferred as :nil
    w nil
  [] x y z w
```

### Function Return Types

```cirru
let
    ; inferred as :list
    numbers $ range 10
    ; inferred as :number
    n $ &list:first numbers
  [] n numbers
```

### Record and Struct Types

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 10) (:y 20)
    x-val $ :x p
  ; x-val inferred as :number from field type
  assert= x-val 10
```

## Type Assertions

Use `assert-type` to explicitly check local values during preprocessing:

```cirru
let
    transform-fn $ fn (x) (* x 2)
    process-data $ fn (data)
      hint-fn $ {}
        :args $ [] :list
        :return :list
      let
          xs data
        assert-type xs :list
        &list:map xs transform-fn
  process-data $ [] 1 2 3
```

**Note**: `assert-type` is evaluated during preprocessing and removed at runtime, so there's no performance penalty.

## Type Inspection Tool

Use `&inspect-type` to debug type inference. Pass a symbol name and the inferred type is printed to stderr during preprocessing:

```cirru
let
    x 10
    nums $ [] 1 2 3
  ; A broad assertion checks the shape without erasing the inferred Number element type.
  assert-type nums 'List
  ; Prints: [&inspect-type] x => number
  &inspect-type x
  ; Prints: [&inspect-type] nums => list<number>
  &inspect-type nums
  let
      item $ &list:nth nums 0
    ; Prints: [&inspect-type] item => number
    &inspect-type item
    assert-type item 'Number
    ; Prints: [&inspect-type] item => number
    &inspect-type item
```

**Note**: This is a development tool - remove it in production code. Returns `nil` at runtime.

## Optional Types

Calcit supports optional type annotations for nullable values:

Definition:

```cirru
defn find-user (id) (; May return nil if user not found) (println "|demo code")
```

Schema on the namespace definition:

```cirru
:: :fn $ {}
  :args $ [] :dynamic
  :return $ :: :optional :record
```

## Variadic Types

Functions with rest parameters use variadic type annotations:

Definition:

```cirru
defn sum (& numbers) (reduce numbers 0 +)
```

Schema on the namespace definition:

```cirru
:: :fn $ {} (:rest :number) (:return :number)
```

## Function Types

Functions can be typed as `:fn` in schema:

Definition:

```cirru
defn apply-twice (f x)
  f $ f x
```

Schema on the namespace definition:

```cirru
:: :fn $ {}
  :args $ [] :fn :number
  :return :number
```

## Disabling Checks

### Per-Function

Skip checks for specific functions by naming them with special markers:

- Functions with `%` in the name (macro-generated)
- Functions with `$` in the name (special markers)
- Functions starting with `__` (internal functions)

### Per-Namespace

Checks are automatically skipped for:

- `calcit.core` namespace (external library)
- Functions with variadic or optional parameters (complex arity rules)

## Type-Directed Optimizations

Beyond warning about type errors, the static analysis system drives **compile-time performance optimizations**. When the preprocessor knows a value's type, it rewrites operations to skip runtime dispatches:

### Record Field Operations

When a record's struct type is known:

- **Field read** `(:field record)` → `&record:nth record <index>` — O(1) direct access instead of O(log n) name lookup
- **Field update** `&record:assoc record :field value` → `&record:assoc-at record <index> value` — pre-resolved field index
- **Batch update** `record-with record (:f1 v1) (:f2 v2)` → `&record:with-at record <indexes> <values>` — all indices pre-resolved

### Conditional Folding

When `if` conditions are literal `true`, `false`, or `nil`:

- `(if true a b)` → `a` — dead branch eliminated at preprocess time
- `(if false a b)` → `b`
- `(if nil a b)` → `b`

### How to Benefit

These rewrites are automatic. Provide type annotations (`hint-fn`, `:schema`, `assert-type`) so the preprocessor can resolve types at compile time. Use `--warn-dyn-method` to find places where type information is missing.

## Best Practices

### 1. Use Type Annotations for Public APIs

```cirru
let
    process-input $ fn (input) (assoc input :processed true)
    public-api-function $ fn (input)
      hint-fn $ {}
        :args $ [] :map
        :return :string
      let
          processed $ process-input input
        assert-type processed :map
        str processed
  public-api-function $ {} (:data |hello)
```

### 2. Leverage Type Inference

Let the system infer types from literals and function calls:

```cirru
defn calculate-area (width height) (; Types inferred from arithmetic operations) (* width height)
```

### 3. Add Assertions for Critical Code

```cirru
let
    dangerous-operation $ fn (data)
      map data $ fn (x) (* x 2)
    critical-operation $ fn (data)
      hint-fn $ {}
        :args $ [] :list
        :return :list
      let
          checked data
        assert-type checked :list
        ; Ensure the local value is still what we expect before processing
        dangerous-operation checked
  critical-operation $ [] 1 2 3
```

### 4. Document Complex Types

Definition:

```cirru
; Function that takes a map with specific keys

defn process-user (user-map) (; Expected keys: :name :email :age) (println "|demo code")
```

Schema on the namespace definition:

```cirru
:: :fn $ {}
  :args $ [] :map
```

## Type Slots (Cross-Package Type Injection)

Type slots allow a **library** to declare a type placeholder that an **application** binds to a concrete type at startup. This bridges the gap where a library defines callback signatures but cannot know the application's specific enum/struct types.

### Declaring a Type Slot (Library Side)

Use `deftype-slot` in the library's schema namespace:

```cirru.no-check
deftype-slot :dispatch-op
```

Then reference the slot in type annotations with the `*name` syntax:

```cirru.no-check
;; EventHandler schema "—" dispatch callback accepts the slot type

:: :fn $ {} (:return :unit)
  :args $ [] '*dispatch-op
```

### Binding a Type Slot (Application Side)

Bind the slot in the configuration of the entry being compiled. The concrete type must use a full `namespace/definition` path:

```bash
cr config set-type-slot :dispatch-op app.schema/Op
```

This writes the following entry-level configuration:

```cirru.no-check
:entries $ {}
  :default $ {} (:mode :native) (:init-fn 'app.main/main!)
    :type-slots $ {} (:dispatch-op |app.schema/Op)
```

No wrapper is needed around `main!`; the binding is installed before any definition is preprocessed and applies to the whole selected entry.

A named entry has an independent configuration:

```bash
cr config set-type-slot --entry server :dispatch-op app.schema/ServerOp
cr config type-slots --entry server
```

Entries do not inherit `entries.default.type-slots`. Bind every slot needed by each entry explicitly.

### How It Works

1. `deftype-slot :name` declares the placeholder supplied by a library.
2. Selecting an entry selects its `:type-slots` map before preprocessing starts.
3. When a type annotation encounters `*name`, the configured concrete type is resolved and normal type matching proceeds.
4. Different entries can bind the same slot to different types because each invocation compiles one selected entry configuration.

### Constraints

- Configuration values must be `:dynamic` or a full `namespace/definition` path that exists after modules are loaded.
- An unbound slot currently falls back to `:dynamic`; bind it explicitly when static checking is expected.
- `:dynamic` is an explicit opt-out for an entry that intentionally disables the slot check.
- The slot name is currently project-wide within one compilation. Libraries should choose stable, specific names to avoid accidental collisions.

Inspect and remove bindings with:

```bash
cr config type-slots
cr config rm-type-slot :dispatch-op
```

### Compatibility Form

`with-type-slot (:name TypeExpr) body...` remains available for older projects and local compatibility. It is a compile-time form and is always erased before runtime/code generation; one body, multiple bodies, and an explicit `do` have the same semantics. New application entry points should prefer `:type-slots`, which makes the build-wide choice visible and independent of lazy compilation order.

### Example: Detecting Wrong Dispatch Calls

After binding `*dispatch-op` to `Op`, the preprocessor catches mistakes:

```cirru.no-check
;; "✅" Correct "—" compiles cleanly

d! $ %:: Op :toggle (:id task)

;; "❌" Wrong variant name

;; Warning: "does not have variant :delete"

d! $ %:: Op :delete (:id task)

;; "❌" Wrong payload count

;; Warning: "expects 0 payload(s), got 1"

d! $ %:: Op :clear 42
```

### Cirru EDN Representation

In serialized type annotations, type slots appear as `'*name` (EDN symbol with `*` prefix):

```cirru.no-check
:args $ [] '*dispatch-op
```

## Limitations

1. **Dynamic Code**: Type checks don't apply to dynamically generated code
2. **JavaScript Interop**: JS function calls are not type-checked
3. **Macro Expansion**: Some macros may generate code that bypasses checks
4. **Runtime Polymorphism**: Type checks are conservative with polymorphic code

## Error Messages

Type check warnings include:

- **Location information**: namespace, function, and code location
- **Expected vs actual types**: clear description of the mismatch
- **Available options**: list of valid fields/methods/variants

Example warning:

```
[Warn] Tuple index out of bounds: tuple has 3 element(s), but trying to access index 5, at my-app.core/process-point
```

## Advanced Topics

### Custom Type Predicates

While Calcit doesn't support custom type predicates in the static analysis system yet, you can use runtime checks:

```cirru
defn is-positive? (n)
  and (number? n) (> n 0)
```

### Type-Driven Development

1. Write function signatures with type annotations
2. Let the compiler guide implementation
3. Use warnings to catch edge cases
4. Add assertions for invariants

### Performance

Static type analysis:

- Runs during preprocessing phase
- Zero runtime overhead
- Only checks functions that are actually called
- Cached between hot reloads (incremental)

## See Also

- [Polymorphism](polymorphism.md) - Object-oriented programming patterns
- [Macros](macros.md) - Metaprogramming and code generation
- [Data](../data.md) - Data types and structures
