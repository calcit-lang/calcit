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
---

# Static Type Analysis

Calcit includes a built-in static type analysis system that performs compile-time checks to catch common errors before runtime. This system operates during the preprocessing phase and provides warnings for type mismatches and other potential issues.

## Quick Recipes

- **Assert Type**: `assert-type total :number`
- **Local `fn` Hint**: `hint-fn $ {} (:args ([] :number)) (:return :number)`
- **Top-level `defn` Schema**: `cr edit schema app.main/add --code 'quote $ :: :fn $ {} (:args ([] :number :number)) (:return :number)'`
- **Top-level value Schema**: `cr edit schema 'app.main/*enabled?' --code 'quote $ :: :ref :bool'`
- **Return Type**: `hint-fn $ {} (:return :string)`
- **Compact Hint**: `defn my-fn (x) :string ...`
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

`check-types` treats nested dynamic slots such as bare `:ref`, `:list`, or `:map` as partial coverage and includes actionable `[W_SCHEMA_DYNAMIC]` entries in `schema_issues`. `weak-types --format json` reports the exact Snapshot/schema path and a `suggestion` for every occurrence. Definitions marked with the explicit `:js-ffi` feature remain classified as intentional boundaries rather than ordinary unresolved debt.

An explicit function schema feature such as `:features $ #{} :js-ffi` classifies dynamic schema/code occurrences as `intentional-js-ffi`. It does not hide them: the report keeps the locations visible while separating them from unresolved dynamic types. `nil` occurrences remain unresolved because an FFI capability does not imply that every nullable branch is intentional.

For one definition, `cr query context '<ns/def>' --format json` embeds the same distinction in its diagnostics and returns the definition revision together with Snapshot paths.

For one expression, `cr query type-at '<ns/def>' --path code@... --format json` preprocesses only static program metadata and returns inferred type, expected type, typed bindings, confidence, method candidates, and diagnostics. It does not run the application entry. Paths use the same stable Snapshot coordinates returned by structural query commands.

Both analysis commands run as static Snapshot readers: they load configured modules and core metadata but do not preprocess or execute the application entry. With `--format json`, stdout is one versioned JSON envelope containing a stable scope revision, filters, summary, and definition-level rows; startup/command messages stay on stderr.

Use `--summary-only` when only aggregate counts are needed. Human output stops after the aggregate section; JSON keeps `data.summary` and the scope revision while returning an empty `data.definitions` array. `defstruct`, `defenum`, and `deftrait` carry type information in their declarations, so they are classified as data declarations instead of receiving a false top-level `schema-dynamic` finding.

`check-examples` reports pass/fail and elapsed time without printing the final example value, which can be a very large function, record, or component tree. Output explicitly produced by an example is still shown.

## Type Annotations

### Function Parameter Types

Function parameters should be annotated with function schema:

- top-level `defn` / `defmacro`: prefer `:schema`
- local `fn`: use `hint-fn` with `:args` / `:rest`

For namespace-level definitions, `:schema` is stored on the definition entry and is typically edited with `cr edit schema`, rather than written inline in the function body.

`cr edit schema` accepts exactly one AST node and therefore requires the CLI code/data boundary: use `quote :string` for a primitive leaf or `quote $ :: :ref :bool` for a parameterized type expression. The `quote` belongs to CLI transport and is not stored inside `:schema`. Callable payloads use the canonical wrapped form `:: :fn $ {} ...` or `:: :macro $ {} ...`; raw `{} (:kind :fn)` maps and bare parameterized tags such as `:ref` are rejected with a corrective error. Parameterized value schemas use the same type grammar as function arguments, for example `:: :ref :bool`, `:: :list :string`, or `:: :map :tag :number`.

The preprocessor propagates a named function's schema into its parameter bindings. This means field access, method dispatch, generic return inference, and return checks inside the body use the declared types instead of falling back to `:dynamic`. A `:rest` schema is preserved as a variadic element type both for calls and when the function is passed as a higher-order callback.

`assert-type` is still useful, but mainly for local variables, intermediate values, and explicit checks inside the function body.

Runnable Example:

```cirru
let
    calculate-total $ fn (items)
      hint-fn $ {} (:args ([] :list)) (:return :number)
      reduce items 0
        fn (acc item)
          hint-fn $ {} (:args ([] :number :number)) (:return :number)
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
      hint-fn $ {} (:args ([] :dynamic)) (:return :string)
      , |demo
  get-name nil
```

```cirru
let
    print-it $ fn (x)
      hint-fn $ {}
        :generics $ [] 'T
        :where $ {}
          'T Show
        :args $ [] 'T
        :return :string
      .show x
  print-it 1
```

Do not use the old tuple/list form such as `:where $ [] (:: 'Show 'T)`.

#### 2. Compact Hint (Trailing Label)

For `defn` and `fn`, you can place a type label immediately after the parameters:

```cirru
let
    add $ fn (a b) :number
      + a b
  add 10 20
```

For namespace-level `defn` / `defmacro`, parameter and return metadata should still live in `:schema`.

### Multiple Annotations

```cirru
let
    add $ fn (a b) :number
      hint-fn $ {} (:args $ [] :number :number) (:return :number)
      let
          total $ + a b
        assert-type total :number
        , total
  assert= 3 $ add 1 2
```

## Supported Types

The following type tags are supported:

| Tag                 | Calcit Type         |
| ------------------- | ------------------- |
| `:nil`              | Nil                 |
| `:bool`             | Boolean             |
| `:number`           | Number              |
| `:string`           | String              |
| `:symbol`           | Symbol              |
| `:tag`              | Tag (Keyword)       |
| `:list`             | List                |
| `:map`              | Hash Map            |
| `:set`              | Set                 |
| `:tuple`            | Tuple (general)     |
| `:fn`               | Function            |
| `:ref`              | Atom / Ref          |
| `:any`              | Static top type: explicitly accepts every Calcit value |
| `:dynamic`          | Unknown/unresolved type: static checks are disabled at this boundary |

### Complex Types

#### Optional Types

Represent values that can be `nil`. Use the `:: :optional <type>` syntax:

```cirru
let
    greet $ fn (name)
      hint-fn $ {} (:args ([] (:: :optional :string))) (:return :string)
      str "|Hello " (or name "|Guest")
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

Use `:any` when “every Calcit value is accepted” is itself the precise public contract. It is a one-way static top type: a concrete value satisfies an expected `:any`, but a value known only as `:any` does not satisfy a concrete expected type. Unlike `:dynamic`, it does not erase checks in both directions and is not reported as unresolved by type-coverage tools. `cr query type :any --format json` therefore returns an empty `methods` array (no method is guaranteed), while unknown method metadata remains `null`.

Do not strengthen a schema beyond the runtime contract merely to remove `:dynamic`. Recursive heterogeneous operations such as flattening may intentionally accept either a collection or a scalar; use `:any` when all Calcit values are valid, and retain `:dynamic` only when the type is genuinely unavailable, such as an unresolved JS FFI/global-state boundary. Keep those boundaries explicit and narrow, while strengthening homogeneous lists/maps/sets, refs, named data references, ordinary function parameters, and return values.

#### Record and Enum Types

Use the name defined by `defstruct` or `defenum`:

```cirru
let
    User $ defstruct User (:name :string)
    get-name $ fn (u)
      hint-fn $ {} (:args ([] 'User)) (:return :string)
      get u :name
  get-name $ %{} User (:name |Alice)
```

## Built-in Type Checks

### Function Arity Checking

The system validates that function calls have the correct number of arguments:

```cirru
defn greet (name age)
  str "|Hello " name "|, you are " age

; Error: expects 2 args but got 1
; greet |Alice
```

### Record Field Access

Validates that record fields exist:

```cirru
defstruct User (:name :string) (:age :number)

defn get-user-email (user)
  .-email user
  ; Warning: field 'email' not found in record User
  ; Available fields: name, age
```

### Tuple Index Bounds

Checks tuple index access at compile time:

```cirru.no-check
let
    point (%:: :Point 10 20 30)
  &tuple:nth point 5  ; Warning: index 5 out of bounds, tuple has 4 elements
```

### Enum Variant Validation

Validates enum construction and pattern matching:

```cirru.no-check
defenum Result
  :Ok :any
  :Error :string

; Warning: variant 'Failure' not found in enum Result
%:: Result :Failure "|something went wrong"
; Available variants: Ok, Error

; Warning: variant 'Ok' expects 1 payload but got 2
%:: Result :Ok 42 |extra
```

### Method Call Validation

Checks that methods exist for the receiver type:

```cirru
defn process-list (xs)
  ; .unknown-method xs
  println "|demo code"
  ; "Warning: unknown method .unknown-method for :list"
  ; Available methods: .map, .filter, .count, ...
```

### Recur Arity Checking

Validates that `recur` calls have the correct number of arguments:

```cirru
defn factorial (n acc)
  if (<= n 1) acc
    recur (dec n) (* n acc)
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
    x-val (:x p)
  ; x-val inferred as :number from field type
  assert= x-val 10
```

## Type Assertions

Use `assert-type` to explicitly check local values during preprocessing:

```cirru
let
    transform-fn $ fn (x) (* x 2)
    process-data $ fn (data)
      hint-fn $ {} (:args ([] :list)) (:return :list)
      let
          xs data
        assert-type xs :list
        &list:map xs transform-fn
  process-data ([] 1 2 3)
```

**Note**: `assert-type` is evaluated during preprocessing and removed at runtime, so there's no performance penalty.

## Type Inspection Tool

Use `&inspect-type` to debug type inference. Pass a symbol name and the inferred type is printed to stderr during preprocessing:

```cirru
let
    x 10
    nums $ [] 1 2 3
  assert-type nums :list
  ; Prints: [&inspect-type] x => number type
  &inspect-type x
  ; Prints: [&inspect-type] nums => list type
  &inspect-type nums
  let
      item $ &list:nth nums 0
    ; Prints: [&inspect-type] item => dynamic type
    &inspect-type item
    assert-type item :number
    ; Prints: [&inspect-type] item => number type
    &inspect-type item
```

**Note**: This is a development tool - remove it in production code. Returns `nil` at runtime.

## Optional Types

Calcit supports optional type annotations for nullable values:

Definition:

```cirru
defn find-user (id)
  ; May return nil if user not found
  println "|demo code"
```

Schema on the namespace definition:

```cirru
:: :fn $ {} (:args $ [] :dynamic) (:return (:: :optional :record))
```

## Variadic Types

Functions with rest parameters use variadic type annotations:

Definition:

```cirru
defn sum (& numbers)
  reduce numbers 0 +
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
  f (f x)
```

Schema on the namespace definition:

```cirru
:: :fn $ {} (:args $ [] :fn :number) (:return :number)
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
      hint-fn $ {} (:args ([] :map)) (:return :string)
      let
          processed $ process-input input
        assert-type processed :map
        str processed
  public-api-function ({} (:data |hello))
```

### 2. Leverage Type Inference

Let the system infer types from literals and function calls:

```cirru
defn calculate-area (width height)
  ; Types inferred from arithmetic operations
  * width height
```

### 3. Add Assertions for Critical Code

```cirru
let
    dangerous-operation $ fn (data) (map data (fn (x) (* x 2)))
    critical-operation $ fn (data)
      hint-fn $ {} (:args ([] :list)) (:return :list)
      let
          checked data
        assert-type checked :list
        ; Ensure the local value is still what we expect before processing
        dangerous-operation checked
  critical-operation ([] 1 2 3)
```

### 4. Document Complex Types

Definition:

```cirru
; Function that takes a map with specific keys
defn process-user (user-map)
  ; Expected keys: :name :email :age
  println "|demo code"
```

Schema on the namespace definition:

```cirru
:: :fn $ {} (:args $ [] :map)
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
;; EventHandler schema — dispatch callback accepts the slot type
:: :fn $ {} (:return :unit)
  :args $ [] '*dispatch-op
```

### Binding a Type Slot (Application Side)

In the application's entry point (e.g. `main!`), use `with-type-slot` to bind a concrete type locally for the scope of the body:

```cirru.no-check
defenum Op (:add :string) (:remove :tag) (:clear)

defn main! () $ with-type-slot (:dispatch-op Op)
  ;; all code in this body benefits from full type checking
```

`with-type-slot` takes a binding pair `(:slot-name TypeExpr)` as its first argument and a body of expressions. The slot is active only within that scope.

### How It Works

1. `deftype-slot :name` registers a placeholder (optional, for documentation/library contracts).
2. `with-type-slot (:name ConcreteType) body...` pushes a scoped override for `*name` during preprocessing of the body, then pops it when the body finishes.
3. When type annotations encounter `*name`, the override is resolved and standard type matching proceeds.
4. Multiple entries can each bind the same slot independently without conflict, since each binding is scoped.

### Constraints

- Only enum, struct, and record types can be bound to slots.
- Unbound slots (no active `with-type-slot` override) are treated as `:dynamic` (no type checking, no error).
- `with-type-slot` bindings are scoped — they do not persist outside the body.

### Example: Detecting Wrong Dispatch Calls

After binding `*dispatch-op` to `Op`, the preprocessor catches mistakes:

```cirru.no-check
;; ✅ Correct — compiles cleanly
d! $ %:: Op :toggle (:id task)

;; ❌ Wrong variant name
;; Warning: "does not have variant :delete"
d! $ %:: Op :delete (:id task)

;; ❌ Wrong payload count
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
