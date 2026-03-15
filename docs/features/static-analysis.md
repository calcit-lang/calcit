# Static Type Analysis

Calcit includes a built-in static type analysis system that performs compile-time checks to catch common errors before runtime. This system operates during the preprocessing phase and provides warnings for type mismatches and other potential issues.

## Quick Recipes

- **Assert Type**: `assert-type total :number`
- **Local `fn` Hint**: `hint-fn $ {} (:args ([] :number)) (:return :number)`
- **Top-level `defn` Schema**: `cr edit schema app.main/add -e ':: :fn $ {} (:args $ [] :number :number) (:return :number)'`
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

## Type Annotations

### Function Parameter Types

Function parameters should be annotated with function schema:

- top-level `defn` / `defmacro`: prefer `:schema`
- local `fn`: use `hint-fn` with `:args` / `:rest`

For namespace-level definitions, `:schema` is stored on the definition entry and is typically edited with `cr edit schema`, rather than written inline in the function body.

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

Legacy clause syntax such as `(hint-fn (return-type ...))`, `(generics ...)`, and `(type-vars ...)` is no longer supported and now fails during preprocessing.

```cirru
let
    get-name $ fn (user)
      hint-fn $ {} (:args ([] :dynamic)) (:return :string)
      , |demo
  get-name nil
```

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
      hint-fn $ {} (:args ([] :number :number))
      let
          total $ + a b
        assert-type total :number
        total
  add 1 2
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
| `:any` / `:dynamic` | Any type (wildcard) |

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

#### Record and Enum Types

Use the name defined by `defstruct` or `defenum`:

```cirru
let
    User $ defstruct User (:name :string)
    get-name $ fn (u)
      hint-fn $ {} (:args ([] (:: :record User))) (:return :string)
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
