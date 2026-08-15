---
title: "Structs"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "struct type"
  - "field access"
  - "struct fields"
id: core/features/structs
parent: core/features
---

# Structs

Calcit structs are declared data types with a fixed set of named fields. Struct definitions are created with `defstruct`; struct values are constructed with `%{}` or by calling the definition directly with tag/value pairs.

## Quick Recipes

- **Define**: `defstruct Point (:x 'Number) (:y 'Number)`
- **Create**: `%{} Point (:x 1) (:y 2)`
- **Constructor sugar**: `Point :x 1 :y 2`
- **Access**: `(:x p)`
- **Update**: `assoc p :x 10` or `update p :x inc`
- **Type Check**: `assert-type p 'Point`

## Defining a Struct Type

Use `defstruct` to declare a named type with typed fields:

```cirru
defstruct Point (:x 'Number) (:y 'Number)
```

Each field is a pair of `(:field-name type)`. Use quoted-symbol types such as `'Number`, `'String`, `'Bool`, `'Tag`, `'List`, `'Map`, `'Fn`, and `'Dynamic` (untyped). Legacy tag spellings remain compatible and are rewritten by `cr edit format`.

```cirru
defstruct Person (:name 'String) (:age 'Number) (:position 'Tag)
```

## Generic Structs

`defstruct` also accepts an optional generics list right after the type name. Declare generic slots with quoted symbols, then apply the named type in schemas with `(:: 'TypeName ...)`.

```cirru
let
    Box $ defstruct Box ([] 'T) (:value 'T)
    keep $ fn (box)
      hint-fn $ {}
        :generics $ [] 'T
        :args $ [] (:: 'Box 'T)
        :return 'T
      :value box
    b $ %{} Box (:value 1)
  assert-type b $ :: 'Box :number
  assert= 1 $ keep b
```

Use this pattern when the struct definition owns the type variable. Function-level constraints such as `:where` still stay on the function schema, not on `defstruct` itself.

### Generic Structs with `where` Bounds

`defstruct` may also take a `where` map right after the optional generics list. This lets the struct definition itself require that a type variable implements one or more traits.

```cirru
let
    ShownBox $ defstruct ShownBox ([] 'T)
      {} $ 'T Show
      :value 'T
    box $ %{} ShownBox (:value 1)
    item $ :value box
  assert-type box $ :: 'ShownBox 'Number
  assert= |1 $ item .show
```

Here `({} ('T Show))` means `T` must satisfy the `Show` trait. `%{}` enforces that bound when constructing a struct value, so the constraint lives on the data definition rather than on each individual function schema.

## Creating Struct Values

When the definition is in scope, the concise constructor form keeps field names at
the call site:

```cirru.no-check
let
    Point $ defstruct Point (:x 'Number) (:y 'Number)
    p $ Point :x 1 :y 2
  , p
```

Arguments must be tag/value pairs. Required fields must be present, while a field
declared as `Option<T>` may be omitted; the constructor inserts the nominal
`%none` variant. Non-`Option` fields are never silently filled with `nil`.

Use the `%{}` macro to instantiate a struct:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  , p
```

Fields can also be written on separate lines:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  , p
```

## Accessing Fields

Use the required field accessor `(:field value)` to read a field:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ :x p
  ; => 1
```

Required field access only accepts a statically known, typed Struct and returns
the field's declared type directly. A missing field or an untyped receiver is a
checking error. It never changes into an `Option` lookup merely because type
information is missing.

Use `get` for maps and indexed collections when absence is intentional; it
always returns `Option<T>`. Struct fields do not use `get`, which keeps the two
contracts visibly distinct in source code.

Nested nominal fields keep the namespace where their declaration was written.
This means a concise same-namespace field type such as `'Router` remains
statically resolvable even when the outer Struct flows into another namespace:

```cirru.no-check
defstruct Router (:name 'String)
defstruct ClientStore (:router 'Router)

defn router-name (store)
  hint-fn $ {}
    :args $ [] 'app.schema/ClientStore
    :return 'String
  :name $ :router store
```

If a diagnostic still shows an unresolved short receiver such as `'Router`, fix
or qualify the schema/dependency reference. Do not replace the typed read with
`&struct:get`; that only hides the missing declaration context.

Loose/anonymous structs (`?{}` or `%{} _ ...`) also cannot be read through the
required field accessor until an expected named Struct type rewrites them. This
prevents an undeclared field from silently becoming `Dynamic` and forces the
schema to be established before application code depends on it.

Standard collection functions like `keys`, `count`, and `contains?` also work on structs:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ keys p
  ; => $ #{} :x :y
  println $ count p
  ; => 2
  println $ contains? p :x
  ; => true
```

## Updating Fields

Struct values are immutable. Use `assoc` or `struct-with` to produce an updated copy:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
    p2 $ assoc p :x 10
  println p2
  ; => $ %{} :Point (:x 10) (:y 2)
  println p
  ; p is unchanged: $ %{} :Point (:x 1) (:y 2)
```

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
    p2 $ struct-with p (:age 21) (:position :shanghai)
  println p2
  ; p2 has updated :age and :position, :name is unchanged
```

`&struct:assoc` is the low-level variant:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ &struct:assoc p :x 100
```

## Partial Struct Construction

Use `%{}?` to create a partial struct with only some fields set (others default to `nil`):

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p1 $ %{}? Person (:name |Chen)
  println $ :name p1
  ; => |Chen
  println $ :age p1
  ; => nil
```

The low-level `&%{}` form accepts fields as flat keyword-value pairs (no type checking):

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
  println $ &%{} Person :name |Chen :age 20 :position :mainland
```

## Type Checking

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  ; check if a value is a struct
  println $ struct? p
  ; => true
  ; check if it matches a specific struct
  println $ &struct:matches? p Point
  ; => true
  ; get the definition used to construct the value
  println $ struct-definition p
  ; compare definitions directly for an origin check
  println $ = (struct-definition p) .unwrap Point
  ; => true
  ; struct-def? is the definition predicate
  println $ struct-def? Point
  ; => true
  println $ struct-def? p
  ; => false
```

## Pattern Matching

Use `struct-match` to branch on struct definitions:

```cirru
let
    Circle $ defstruct Circle (:radius :number)
    Square $ defstruct Square (:radius :number)
    shape $ %{} Circle (:radius 5)
  struct-match shape
    Circle c $ * 3.14
      * (:radius c) (:radius c)
    Square s $ * (:radius s) (:radius s)
    _ _ 0

; => 78.5
```

## Converting Structs

### To Map

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ &struct:to-map p
  ; => {} (:x 1) (:y 2)
```

`merge` also works and returns a new value of the same struct definition:

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
  println $ merge p
    {} (:age 23) (:name |Ye)
```

## Struct Name and Definition Inspection

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
  ; get the tag name of the struct value
  println $ &struct:get-name p
  ; => :Person
  ; inspect the definition behind a struct value
  println $ struct-definition p
```

### Struct Origin Check

Compare struct definitions directly when you need to confirm a struct value's origin:

```cirru
let
    Cat $ defstruct Cat (:name :string) (:color :tag)
    Dog $ defstruct Dog (:name :string)
    v1 $ %{} Cat (:name |Mimi) (:color :white)
  if
    = (struct-definition v1) .unwrap Cat
    println "|Handle Cat branch"
    println "|Not a Cat"
```

## Polymorphism with Traits

Define a trait with `deftrait`, implement it with `defimpl`, and attach it to a struct with `impl-traits`:

```cirru
let
    BirdTrait $ deftrait BirdTrait
      .show $ :: :fn
        {}
          :args $ [] 'T
          :return :nil
    BirdShape $ defstruct BirdShape (:name :string)
    BirdImpl $ defimpl BirdImpl BirdTrait
      .show $ fn (self)
        ; defimpl bodies are reusable before a concrete Struct is attached,
        ; so low-level access is explicit at this dynamic implementation boundary.
        ; Do not copy this form into typed application code.
        println $ &struct:get self :name
    Bird $ impl-traits BirdShape BirdImpl
    b $ %{} Bird (:name |Sparrow)
  assert-traits b BirdTrait
  b .show
  println $ :name b
```

## Common Use Cases

### Configuration Objects

```cirru
let
    Config $ defstruct Config (:host :string) (:port :number) (:debug :bool)
    config $ %{} Config (:host |localhost) (:port 3000) (:debug false)
  println $ :port config
  ; => 3000
```

### Domain Models

```cirru
let
    Product $ defstruct Product (:id :string) (:name :string) (:price :number) (:discount :number)
    product $ %{} Product (:id |P001) (:name |Widget) (:price 100) (:discount 0.9)
  println $ * (:price product) (:discount product)
  ; => 90
```

## Type Annotations

```cirru
let
    User $ defstruct User (:name :string) (:age :number) (:email :string)
    get-user-name $ fn (user)
      hint-fn $ {}
        :args $ [] 'User
        :return :string
      :name user
  println $ get-user-name
    %{} User (:name |John) (:age 30) (:email |john@example.com)

; => John
```

## Automatic Map-to-Struct Rewrite

When a function parameter is typed as a struct in its schema, the preprocessor automatically rewrites hashmap literal (`{}`) arguments to struct construction (`%{}`). This keeps ergonomic literal syntax while retaining the nominal type.

```cirru.no-check
defstruct Point (:x :number) (:y :number)

defn sum-point (p)
  hint-fn $ {} (:return :number)
    :args $ [] 'app.main/Point
  &+ (:x p) (:y p)

; The preprocessor rewrites this hashmap to the expected struct:
assert= 30 $ sum-point
  {} (:x 10) (:y 20)
; Equivalent to:
assert= 30 $ sum-point
  %{} Point (:x 10) (:y 20)
```

Requirements for the rewrite to trigger:

- The function must have a schema with `:args` that references a struct type (for example `'ns/StructName` or `'Struct`)
- The argument at the call site must be a hashmap literal (`{}` with tag keys)
- All keys in the hashmap must be tags (`:field-name`)
- The struct definition must be resolvable at preprocess time

If any condition is not met, the argument is left unchanged (no error is raised). This makes the rewrite safe to use alongside existing code.

## Anonymous Structs

Use `_` as the definition marker when a short-lived struct does not need a named `defstruct` declaration.

### Creating an Anonymous Struct

```cirru
%{} _ (:name |John) (:age 30)

; => $ %{} _ (:age 30) (:name |John)
```

Fields are automatically sorted alphabetically, matching the field ordering of named structs. All keys must be tags, and duplicate keys produce an error.

### Accessing Fields

Anonymous structs still have a runtime field set, but they do not carry the
declaration needed for typed required-field access. Do not use them as a way to
bypass field analysis in application code. Convert/rewrite the value to an
expected named Struct before reading fields. `&struct:get` remains available
only to core/runtime code or an explicit reusable `defimpl` that intentionally
implements a dynamic boundary.
The following block deliberately demonstrates that low-level runtime behavior;
using the same call in application code produces a typed-access warning.

```cirru.no-check
let
    r $ %{} _ (:x 10) (:y 20)
  println $ &struct:get r :x
  ; => 10
  println $ type-of r
  ; => :struct
```

### Automatic Rewrite to a Named Struct

When an anonymous struct is passed to a function whose parameter is a named struct, the preprocessor can rewrite it to the expected nominal definition:

```cirru.no-check
defstruct Point (:x :number) (:y :number)

defn sum-point (p)
  hint-fn $ {} (:return :number)
    :args $ [] 'app.main/Point
  &+ (:x p) (:y p)

; Anonymous struct rewritten to the expected named struct:
assert= 30 $ sum-point (%{} _ (:x 10) (:y 20))
; Equivalent to:
assert= 30 $ sum-point
  %{} Point (:x 10) (:y 20)
```

The rewrite uses the same requirements as the map-to-struct rewrite. Fields not present in the anonymous struct but defined in the target struct are filled with `nil`; their declared field types must therefore permit `nil`.

### Design Symmetry

The collection type system follows a consistent "precision increasing" pattern:

| Positional (by index)            | Named (by field)                       |
| -------------------------------- | -------------------------------------- |
| `list` (dynamic)                 | `hashmap` (dynamic)                    |
| `%:: _ :tag ...` (anonymous enum) | `%{} _ (:field val)` (anonymous struct) |
| `%:: Enum :tag ...` (named enum)  | `%{} Struct (:field val)` (named struct) |

Both anonymous enums and anonymous structs can be rewritten to their named counterparts when function parameter types are known at compile time.

## Performance Notes

- Struct values are immutable — updates create new values
- Field access is O(1) when the struct type is known at preprocess time (compile-time index resolution)
- When the type is unknown, field access falls back to O(log n) binary search over sorted field names
- Use `struct-with` to update multiple fields at once and minimize intermediate allocations

### Type-Directed Optimizations

When the static analysis system knows a value's struct type, the preprocessor rewrites field operations to skip runtime name lookups:

- **Field read** `(:field value)` → `&struct:nth value <index>` — direct index access instead of name search
- **Field update** `&struct:assoc value :field next` → `&struct:assoc-at value <index> next`
- **Batch update** `struct-with value (:f1 v1) (:f2 v2)` → `&struct:with-at value <indexes> <values>`

These rewrites are automatic and transparent. To benefit from them, provide type annotations via `:schema` or `hint-fn` so the preprocessor can resolve struct types.

Struct intentionally has no public `.nth` method: positional field order is not
a cross-backend API contract. Use `(:field value)` or `value.:field`, which
returns the declared field type directly. Only generated, statically checked
access uses internal `&struct:nth`.
