---
title: "Structs"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "record type"
  - "field access"
  - "struct fields"
id: core/features/records
parent: core/features
---

# Structs

Calcit structs are declared data types with a fixed set of named fields. Struct definitions are created with `defstruct`; struct values are constructed with `%{}`.

## Quick Recipes

- **Define**: `defstruct Point (:x 'Number) (:y 'Number)`
- **Create**: `%{} Point (:x 1) (:y 2)`
- **Access**: `get p :x` or `(:x p)`
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

Use `get` (or `&struct:get`) to read a field:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ get p :x
  ; => 1
```

For a statically known struct, field access returns the field's declared type directly. A missing field is a type/checking error; struct access never produces `Option` merely because the field name might be absent.

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
  println $ get p1 :name
  ; => |Chen
  println $ get p1 :age
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
  println $ = (option:unwrap $ struct-definition p) Point
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
      * (get c :radius) (get c :radius)
    Square s $ * (get s :radius) (get s :radius)
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
    = (option:unwrap $ struct-definition v1) Cat
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
      .rename $ :: :fn
        {}
          :generics $ [] 'T
          :args $ [] 'T :string
          :return 'T
    BirdShape $ defstruct BirdShape (:name :string)
    BirdImpl $ defimpl BirdImpl BirdTrait
      .show $ fn (self)
        println $ get self :name
      .rename $ fn (self name) (assoc self :name name)
    Bird $ impl-traits BirdShape BirdImpl
    b $ %{} Bird (:name |Sparrow)
  assert-traits b BirdTrait
  b .show
  let
      b2 $ b .rename |Eagle
    println $ :name b2
```

## Common Use Cases

### Configuration Objects

```cirru
let
    Config $ defstruct Config (:host :string) (:port :number) (:debug :bool)
    config $ %{} Config (:host |localhost) (:port 3000) (:debug false)
  println $ get config :port
  ; => 3000
```

### Domain Models

```cirru
let
    Product $ defstruct Product (:id :string) (:name :string) (:price :number) (:discount :number)
    product $ %{} Product (:id |P001) (:name |Widget) (:price 100) (:discount 0.9)
  println $ * (get product :price) (get product :discount)
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
      &struct:get user :name
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

Fields are automatically sorted alphabetically, matching the behavior of struct-backed records. All keys must be tags, and duplicate keys produce an error.

### Accessing Fields

Anonymous structs still have a fixed field set, so access is direct and missing fields are errors:

```cirru
let
    r $ %{} _ (:x 10) (:y 20)
  println $ :x r
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
a cross-backend API contract. Use field-name `get`, which returns the declared
field type directly. Only generated, statically checked access uses internal
`&struct:nth`.
