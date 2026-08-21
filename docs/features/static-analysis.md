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
  - "quality gate"
  - "type baseline"
entry_for:
  - "assert-type"
  - "calcit analyze check-types"
  - "calcit analyze weak-types"
  - "calcit analyze deprecated"
  - "calcit analyze quality"
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
- **Top-level `defn` Schema**: `calcit edit schema app.main/add --code "quote $ :: 'Fn $ {} (:args ([] 'Number 'Number)) (:return 'Number)"`
- **Top-level value Schema**: `calcit edit schema 'app.main/*enabled?' --code "quote $ :: 'Ref 'Bool"`
- **Return Type**: `hint-fn $ {} (:return 'String)`
- **Compact Hint**: `defn my-fn (x) 'String ...`
- **Check Traits**: `assert-traits x MyTrait`
- **Ignore Warning**: `&core:ignore-type-warning`

## Overview

The static analysis system provides:

- **Type inference** - Automatically infers types from literals and expressions
- **Type annotations** - Optional type hints for function parameters and return values
- **Compile-time warnings** - Catches errors before code execution
- **Completion warnings** - Keeps scaffolded `todo!` paths visible to Agents
- **CI quality gate**: `calcit analyze quality --baseline config/calcit-quality.json`
- **Composable runtime assertions** - `assert-type` and `assert-traits` can validate values at runtime and return original values for chaining

## Static Project Reports

Use the CLI reports when you need to understand type quality without running the application:

```bash
# Coverage by definition; unknown code is reported as none, never as full
calcit analyze check-types --ns app.main

# All weak type locations
calcit analyze weak-types --ns app.main

# Deprecated API calls, including the source definition and exact code path
calcit analyze deprecated --ns app.main

# Enforce zero type/weak-type/deprecated debt with a non-zero failure exit
calcit analyze quality

# Bootstrap and enforce a reviewed baseline for an existing project
calcit analyze quality --write-baseline config/calcit-quality.json
calcit analyze quality --baseline config/calcit-quality.json

# Focus only on unresolved type debt
calcit analyze weak-types --ns app.main --intent unresolved

# Focus on nil migration debt while excluding declared Unit returns
calcit analyze weak-types --ns app.main --only code-nil --intent unresolved,declared-optional

# Inspect explicitly permitted JS FFI dynamic boundaries
calcit analyze weak-types --ns app.main --intent intentional-js-ffi

# Inventory every explicit unchecked JS FFI assertion with its target schema
calcit analyze weak-types --ns app.main --only unsafe-coerce

# Machine-readable definition rows and Snapshot paths
calcit analyze check-types --ns app.main --format json
calcit analyze weak-types --ns app.main --intent unresolved --format json
calcit analyze deprecated --ns app.main --format json

# Keep aggregate counts but omit definition rows (especially useful for agents)
calcit analyze check-types --ns app.main --summary-only --format json
calcit analyze weak-types --ns app.main --intent unresolved --summary-only --format json

# Validate only one definition's examples
calcit analyze check-examples --ns app.main --def calculate-total

# Validate examples that depend on JavaScript-only FFI syntax
calcit analyze check-examples --ns app.main --def 'detect-nodejs?' --js

# Explain one expression using inferred and expected types
calcit query type-at app.main/calculate-total --path code@3.2 --format json
```

`check-types` treats nested dynamic slots such as bare `:ref`, `:list`, or `:map` as partial coverage and includes actionable `[W_SCHEMA_DYNAMIC]` entries in `schema_issues`. An unbound `*type-slot` is also partial and emits `[W_UNRESOLVED_TYPE_SLOT]`; bind it in the selected entry or explicitly choose `:dynamic` for a documented boundary. When partial/none definitions exist, human output adds an `agent-note` and JSON emits `W_TYPE_COVERAGE_GAPS`. `weak-types --format json` reports the exact Snapshot/schema path plus an `impact` and `suggestion` for every occurrence; unresolved dynamic debt emits `W_DYNAMIC_TYPE_DEBT`, unbound slots emit `W_UNRESOLVED_TYPE_SLOT`, while unresolved or compatibility-Optional nil debt emits `W_NIL_TYPE_DEBT`. Definitions marked with the explicit `:js-ffi` feature remain classified as intentional boundaries rather than ordinary unresolved dynamic debt.

An explicit function schema feature such as `:features $ #{} :js-ffi` classifies dynamic schema/code occurrences as `intentional-js-ffi`. A selected entry binding of `:type-slots` to `:dynamic` stays visible as `intentional-type-slot-dynamic`. Neither hides the location, but both separate an explicit boundary choice from unresolved type debt. The FFI feature does not classify `nil`, because an FFI capability does not imply that every nullable branch is intentional.

For `code-nil`, the report includes both raw `nil` and the explicit `;nil` Unit marker. Every nil form inside a function declared to return `Unit` is classified as `declared-unit`; `;nil` is also always classified as explicit Unit, including inside generated macro branches. For legacy `Optional<T>`, only structurally proven return positions inherit `declared-optional`; embedded nil values remain unresolved. `declared-unit` is excluded from migration debt, while `declared-optional` remains visible so application APIs move to `Option` or `Result`. The core release gate runs `analyze weak-types --only code-nil --intent unresolved,declared-optional` and requires no findings.

`unsafe-coerce` is reported separately as `unsafe-coerce` with the explicit `explicit-unsafe` intent, its exact `code@...` path, and the asserted target schema. It is an inventory, not ordinary unresolved Dynamic debt, so the existing quality baseline stays stable while each boundary is audited. JSON adds `W_JS_FFI_UNCHECKED_COERCE` whenever the selected scope contains one or more assertions. Keep each assertion in a narrow adapter and cover both accepted and rejected host values with runtime-contract tests.

For one definition, `calcit query context '<ns/def>' --format json` embeds the same distinction in its diagnostics and returns the definition revision together with Snapshot paths.

For one expression, `calcit query type-at '<ns/def>' --path code@... --format json` preprocesses only static program metadata and returns inferred type, expected type, typed bindings, confidence, method candidates, and diagnostics. It does not run the application entry. Paths use the same stable Snapshot coordinates returned by structural query commands.

These analysis commands run as static Snapshot readers: they load configured modules and core metadata but do not preprocess or execute the application entry. With `--format json`, stdout is one versioned JSON envelope containing a stable scope revision, filters, summary, and definition-level rows; startup/command messages stay on stderr.

`analyze quality` combines the release-facing metrics from `check-types`, `weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic,code-nil --intent unresolved,declared-optional`, and `deprecated`. Unbound slots add to the existing `unresolved` budget without changing the baseline metric shape. With no baseline it is a zero-debt gate. `--baseline <file>` compares against a committed baseline and exits non-zero on regression; `--write-baseline <file>` atomically writes a reviewed native baseline. Native baselines keep budgets per definition, so improving one definition cannot hide new debt in another. For migration, `--baseline` also accepts the older flat eight-metric JSON shape (`typeNone`, `typeNotFull`, `schemaDynamic`, `codeDynamic`, `codeNil`, `unresolved`, `declaredOptional`, `deprecatedCalls`). Scope flags (`--ns`, `--ns-prefix`, `--deps`) are recorded in native baselines and must match when they are enforced.

`analyze deprecated` scans calls to definitions tagged `:deprecated`. It reports every calling definition and a stable `code@...` path, and includes the target definition's documentation so migrations can be automated without maintaining a second hard-coded legacy API list. Use `--summary-only --format json` for migration gates that only need aggregate counts.

### TODO completion warnings

`todo!` is a compiler-known diverging placeholder for code that is intentionally
not implemented yet:

```cirru.no-check
defn load-user (id)
  todo! "|implement user lookup"
```

Each occurrence emits `W_TODO` with the containing namespace, definition, and
structural path. A static String literal is required for the message; invalid
messages or extra arguments are rejected as type/arity errors before codegen;
they do not emit a completion warning. The placeholder is accepted in a
declared return position without manufacturing a return-type mismatch, but the
warning remains a completion-gate failure until the body is implemented.

`raise "|TODO..."` does not emit `W_TODO`, because ordinary exception behavior
and implementation-completion status are separate concerns.

`analyze.weak-types` uses protocol `schema_version: 4`: v2 added nil intent classes, v3 added the closed `unresolved-type-slot` kind plus `W_UNRESOLVED_TYPE_SLOT`, and v4 adds the closed `unsafe-coerce` kind, `explicit-unsafe` intent, target-schema detail, and `W_JS_FFI_UNCHECKED_COERCE`. Consumers should reject older versions when they require these fields rather than accepting an older envelope and silently missing the new debt.

Use `--summary-only` when only aggregate counts are needed. Human output stops after the aggregate section; JSON keeps `data.summary` and the scope revision while returning an empty `data.definitions` array. `defstruct`, `defenum`, `deftrait`, and `defimpl` have explicit definition-kind schemas: `StructDef`, `EnumDef`, `Trait`, and `Impl`. Legacy snapshots that used `Dynamic` at these roots are normalized on load and written back with the marker. Their fields, enum payloads, and methods are still analyzed normally, but the declaration root itself neither creates a `schema-dynamic` finding nor increases Dynamic usage counts.

`check-examples` reports pass/fail and elapsed time without printing the final example value, which can be a very large function, struct, or component tree. Output explicitly produced by an example is still shown. Pass `--js` to compile the generated examples entry and execute it with Node.js; this is intended for definitions whose examples use JavaScript-only FFI syntax such as `exists? js/process`.

## Type Annotations

Built-in types use **quoted symbols**: write `'String`, `'Number`, `'List`, `'Fn`, and `'Dynamic`. This keeps type syntax distinct from ordinary keyword/tag data. Lowercase tags such as `:string`, `:number`, and `:dynamic` remain load-compatible, but `calcit edit format` rewrites type positions to the symbol form. It does not rewrite ordinary tags such as enum variants, struct field keys, `:return` schema keys, or `:kind` values.

### Function Parameter Types

Function parameters should be annotated with function schema:

- top-level `defn` / `defmacro`: prefer `:schema`
- local `fn`: use `hint-fn` with `:args` / `:rest`

For namespace-level definitions, `:schema` is stored on the definition entry and is typically edited with `calcit edit schema`, rather than written inline in the function body.

`calcit edit schema` accepts exactly one AST node and therefore requires the CLI code/data boundary: use `quote 'String` for a primitive leaf or `quote $ :: 'Ref 'Bool` for a parameterized type expression. A top-level value backed by `defstruct` or `defenum` uses its fully qualified nominal type, for example `calcit edit schema app.schema/store --code "quote 'app.schema/Store"`; the qualification lets the stored schema preserve identity without relying on the editing namespace. The `quote` belongs to CLI transport and is not stored inside `:schema`. Callable payloads use the canonical wrapped form `:: 'Fn $ {} ...` or `:: 'Macro $ {} ...`; raw `{} (:kind :fn)` maps and bare parameterized types such as `'Ref` are rejected with a corrective error. Parameterized value schemas use the same type grammar as function arguments, for example `:: 'Ref 'Bool`, `:: 'List 'String`, or `:: 'Map 'Tag 'Number`.

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
    debug-it $ fn (x)
      hint-fn $ {}
        :generics $ [] 'T
        :where $ {} ('T Debug)
        :args $ [] 'T
        :return 'String
      x .debug
  debug-it 1
```

Do not use the old tuple/list form such as `:where $ [] (:: 'Debug 'T)`.

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
| `'Enum` | Enum value (anonymous or named) |
| `'Struct` | Struct value (anonymous or named) |
| `'Fn` | Function |
| `'Ref` | Atom / Ref |
| `'Dynamic` | Unknown/unresolved type; static checks are disabled at this boundary |

`:any` is a legacy alias for `:dynamic`; both are accepted as input and formatter output is `'Dynamic`.

### Dynamic 用量审计

每次 `calcit` 执行或编译都会在 stderr 统计当前项目的 Dynamic 类型位置。少量使用只保留静默结果；达到一定数量且占比超过阈值时输出 notice 或 warning。该审计不会改变程序语义，也不会污染 stdout；需要定位时运行：

```bash
calcit analyze weak-types --only schema-dynamic,unresolved-type-slot,code-dynamic --intent unresolved --format json
```

Dynamic 应限制在 JS FFI、宏和框架开放数据边界。普通多态使用 TypeVar/`:generics`，能力约束使用 trait/`:where`，缺失使用 `Option<T>`，失败使用 `Result<T,E>`。

### Option / Result 级联

优先使用接收者方法组合可选值，例如 `opt .map f`、`opt .and-then f`、`opt .or-else f`；`Result` 同样使用 `.and-then`、`.map-err`、`.or-else`。`.unwrap` 只用于已经证明为 `some`/`ok` 的分支，`.unwrap-or` 只在默认值终点使用，避免把 `Option` 过早还原成 `nil`。

`get-in` 返回 `Option<T>`，适合开放 Map/List 路径；不要用它绕过 Struct 字段检查，Struct 应使用 `(:field value)`。`update-in` 的 updater 接收 `Option<T>`，必须显式处理缺失值。

### Complex Types

#### Legacy Optional Types

`Optional<T>` is parsed only for legacy core/internal compatibility. Public
function schemas reject it; use `Option<T>`, `Result<T,E>`, or `Unit`. JavaScript
`null`/`undefined` uses the distinct `JsNullish<T>` boundary.

Existing migration tools still understand the old `:: 'Optional <type>` syntax:

```cirru.no-check
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

A variadic function can satisfy a fixed-arity callback when its required parameters and rest element type accept every argument promised by the callback. Callback parameter matching is contravariant, while callback return matching is covariant. Optional callback examples describe legacy compatibility and should not be introduced in public schemas.

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
  :where $ {} ('T Debug)
  :args $ [] 'T
  :return :string
```

The same rule applies inside containers and callbacks: `:: :list 'T` preserves a homogeneous item relationship, while bare `:list` means `list<dynamic>`; a complete `:: :fn` callback schema preserves argument/return checking, while bare `:fn` does not.

#### Struct and Enum Types

Use the name defined by `defstruct` or `defenum`:

```cirru
let
    User $ defstruct User (:name :string)
    get-name $ fn (u)
      hint-fn $ {}
        :args $ [] 'User
        :return :string
      :name u
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

### Struct Field Access

Validates that struct fields exist. A known field has its declared type directly;
an unknown field is a diagnostic rather than an `Option` result:

```cirru
defstruct User (:name :string) (:age :number)

defn get-user-email (user) (.-email user) (; Warning: field ':email' not found in struct User) (; Available fields: :name, :age)
```

### Enum Index Bounds

Enum positional access is bounds-checked at runtime and reports an ordinary
diagnostic rather than panicking:

```cirru.no-check
let
    point $ %:: _ :point 10 20 30
  &enum:nth point 5 ; Error: index 5 is outside this enum value
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
    ; inferred as :optional<number>; handle nil before using it as a number
    n $ &list:first numbers
  [] n numbers
```

### Struct Types

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

## Legacy Optional Types

Calcit can read old Optional annotations for migration analysis, but new public
function schemas must not declare them. Model absence with `Option`, failures
with `Result`, effects with `Unit`, and JavaScript host nullability with
`JsNullish`.

Definition:

```cirru.no-check
defn find-user (id) (; May return nil if user not found) (println "|demo code")
```

Schema on the namespace definition:

```cirru.no-check
:: :fn $ {}
  :args $ [] :dynamic
  :return $ :: 'Optional 'Struct
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

### Struct Field Operations

When a struct definition is known:

- **Field read** `(:field value)` → `&struct:nth value <index>` — O(1) direct access instead of a name lookup
- **Field update** `&struct:assoc value :field next` → `&struct:assoc-at value <index> next`
- **Batch update** `struct-with value (:f1 v1) (:f2 v2)` → `&struct:with-at value <indexes> <values>`

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
calcit config set-type-slot :dispatch-op app.schema/Op
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
calcit config set-type-slot --entry server :dispatch-op app.schema/ServerOp
calcit config type-slots --entry server
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
calcit config type-slots
calcit config rm-type-slot :dispatch-op
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

### Typed Enum Constructor Sugar

When an existing Enum definition is the head of a call, prefer `Enum :tag ...`. The preprocessor resolves the definition, checks the variant and payload types, and lowers it to the named constructor. `%:: Enum :tag ...` remains for explicit runtime prototypes, dynamic cross-module construction, and compatibility boundaries.

```cirru.no-check
Option :some value
Result :err message
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
[Warn] Field `:email` does not exist in struct `User`. Available fields: [:age, :name]. Struct field access is required and never returns nil/Option for a missing field; use a declared field instead
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
