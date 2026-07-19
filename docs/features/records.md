---
title: "Records"
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

# Records

Calcit provides Records as a way to define structured data types with named fields, similar to structs in other languages. Records are defined with `defstruct` and instantiated with the `%{}` macro.

## Quick Recipes

- **Define**: `defstruct Point (:x :number) (:y :number)`
- **Create**: `%{} Point (:x 1) (:y 2)`
- **Access**: `get p :x` or `(:x p)`
- **Update**: `assoc p :x 10` or `update p :x inc`
- **Type Check**: `assert-type p :record`

## Defining a Struct Type

Use `defstruct` to declare a named type with typed fields:

```cirru
defstruct Point (:x :number) (:y :number)
```

Each field is a pair of `(:field-name :type)`. Supported types include `:number`, `:string`, `:bool`, `:tag`, `:list`, `:map`, `:fn`, and `:dynamic` (untyped).

```cirru
defstruct Person (:name :string) (:age :number) (:position :tag)
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
      {} ('T Show)
      :value 'T
    box $ %{} ShownBox (:value 1)
    item $ :value box
  assert-type box $ :: 'ShownBox :number
  assert= |1 $ .show item
```

Here `({} ('T Show))` means `T` must satisfy the `Show` trait. `%{}` enforces that bound when constructing a record instance, so the constraint lives on the data definition rather than on each individual function schema.

## Creating Records

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
    p $ %{} Point
      :x 1
      :y 2
  , p
```

## Accessing Fields

Use `get` (or `&record:get`) to read a field:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ get p :x
  ; => 1
```

Standard collection functions like `keys`, `count`, and `contains?` also work on records:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ keys p
  ; => (#{} :x :y)
  println $ count p
  ; => 2
  println $ contains? p :x
  ; => true
```

## Updating Fields

Records are immutable. Use `assoc` or `record-with` to produce an updated copy:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
    p2 $ assoc p :x 10
  println p2
  ; => (%{} :Point (:x 10) (:y 2))
  println p
  ; p is unchanged: (%{} :Point (:x 1) (:y 2))
```

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
    p2 $ record-with p (:age 21) (:position :shanghai)
  println p2
  ; p2 has updated :age and :position, :name is unchanged
```

`&record:assoc` is the low-level variant (no type checking):

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ &record:assoc p :x 100
```

## Partial Records

Use `%{}?` to create a record with only some fields set (others default to `nil`):

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
  ; check if a value is a record (struct instance)
  println $ record? p
  ; => true
  ; check if it matches a specific struct
  println $ &record:matches? p Point
  ; => true
  ; get the struct definition the record was created from
  println $ &record:struct p
  ; compare structs directly for origin check
  println $ = (&record:struct p) Point
  ; => true
  ; struct? checks struct definitions, not instances
  println $ struct? Point
  ; => true
  println $ struct? p
  ; => false
```

## Pattern Matching

Use `record-match` to branch on record types:

```cirru
let
    Circle $ defstruct Circle (:radius :number)
    Square $ defstruct Square (:side :number)
    shape $ %{} Circle (:radius 5)
  record-match shape
    Circle c $ * 3.14 (* (get c :radius) (get c :radius))
    Square s $ * (get s :side) (get s :side)
    _ _ nil
; => 78.5
```

## Converting Records

### To Map

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    p $ %{} Point (:x 1) (:y 2)
  println $ &record:to-map p
  ; => {} (:x 1) (:y 2)
```

`merge` also works and returns a new record of the same struct:

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
  println $ merge p $ {} (:age 23) (:name |Ye)
```

## Record Name and Struct Inspection

```cirru
let
    Person $ defstruct Person (:name :string) (:age :number) (:position :tag)
    p $ %{} Person (:name |Chen) (:age 20) (:position :mainland)
  ; get the tag name of the record
  println $ &record:get-name p
  ; => :Person
  ; check the struct behind a record value
  println $ &record:struct p
```

### Struct Origin Check

Compare struct definitions directly when you need to confirm a record's origin:

```cirru
let
    Cat $ defstruct Cat (:name :string) (:color :tag)
    Dog $ defstruct Dog (:name :string)
    v1 $ %{} Cat (:name |Mimi) (:color :white)
  if (= (&record:struct v1) Cat)
    println "|Handle Cat branch"
    println "|Not a Cat"
```

## Polymorphism with Traits

Define a trait with `deftrait`, implement it with `defimpl`, and attach it to a struct with `impl-traits`:

```cirru
let
    BirdTrait $ deftrait BirdTrait
      .show $ :: :fn $ {}
        :args $ [] 'T
        :return :nil
      .rename $ :: :fn $ {}
        :args $ [] 'T :string
        :return 'T
    BirdShape $ defstruct BirdShape (:name :string)
    BirdImpl $ defimpl BirdImpl BirdTrait
      .show $ fn (self)
        println $ get self :name
      .rename $ fn (self name)
        assoc self :name name
    Bird $ impl-traits BirdShape BirdImpl
    b $ %{} Bird (:name |Sparrow)
  .show b
  let
      b2 $ .rename b |Eagle
    .show b2
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
    product $ %{} Product
      :id |P001
      :name |Widget
      :price 100
      :discount 0.9
  println $ * (get product :price) (get product :discount)
  ; => 90
```

## Type Annotations

```cirru
let
    User $ defstruct User (:name :string) (:age :number) (:email :string)
    get-user-name $ fn (user)
      hint-fn $ {} (:args ([] (:: :record User))) (:return :string)
      get user :name
  println $ get-user-name $ %{} User
    :name |John
    :age 30
    :email |john@example.com
; => John
```

## Automatic Map-to-Record Rewrite

When a function parameter is typed as a struct in its schema, the preprocessor automatically rewrites hashmap literal (`{}`) arguments to record construction (`%{}`). This lets you write ergonomic hashmap syntax while still getting full record type checking at runtime.

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    sum-point $ fn (p)
      :: :fn $ {} (:return :number)
        :args $ [] 'app.main/Point
      &+ (:x p) (:y p)
  ; Write a hashmap — preprocessor rewrites to record automatically:
  assert= 30 $ sum-point $ {} (:x 10) (:y 20)
  ; Equivalent to:
  assert= 30 $ sum-point $ %{} Point (:x 10) (:y 20)
```

Requirements for the rewrite to trigger:

- The function must have a schema with `:args` that references a struct type (via `TypeRef` like `'ns/StructName`, `Struct`, or `Record`)
- The argument at the call site must be a hashmap literal (`{}` with tag keys)
- All keys in the hashmap must be tags (`:field-name`)
- The struct definition must be resolvable at preprocess time

If any condition is not met, the argument is left unchanged (no error is raised). This makes the rewrite safe to use alongside existing code.

## Loose Records (`?{}`)

Loose records are records created without a declared struct definition, using the `?{}` syntax. This is analogous to how untyped tuples (`::`) work without requiring a `defenum` — loose records provide the same convenience for named fields.

### Creating a Loose Record

```cirru
?{} :name |John :age 30
; => (?{} (:age 30) (:name |John))
```

Fields are automatically sorted alphabetically, matching the behavior of struct-backed records. All keys must be tags, and duplicate keys produce an error.

### Accessing Fields

Loose records support the same field access operations as struct-backed records:

```cirru
let
    r $ ?{} :x 10 :y 20
  println $ :x r
  ; => 10
  println $ type-of r
  ; => :record
```

### Automatic Rewrite to Struct Record

When a loose record is passed to a function whose parameter is typed as a struct, the preprocessor automatically rewrites it to a struct-backed record — just like the map-to-record rewrite:

```cirru
let
    Point $ defstruct Point (:x :number) (:y :number)
    sum-point $ fn (p)
      :: :fn $ {} (:return :number)
        :args $ [] 'app.main/Point
      &+ (:x p) (:y p)
  ; Loose record rewritten to struct record at compile time:
  assert= 30 $ sum-point $ ?{} :x 10 :y 20
  ; Equivalent to:
  assert= 30 $ sum-point $ %{} Point (:x 10) (:y 20)
```

The rewrite uses the same requirements as map-to-record rewrite. Fields not present in the loose record but defined in the struct are filled with `nil`.

### Design Symmetry

The collection type system follows a consistent "precision increasing" pattern:

| Positional (by index)            | Named (by field)                       |
| -------------------------------- | -------------------------------------- |
| `list` (dynamic)                 | `hashmap` (dynamic)                    |
| `:: :tag ...` (untyped tuple)    | `?{} :field val` (loose record)        |
| `%:: Enum :tag ...` (typed enum) | `%{} Struct :field val` (typed record) |

Both untyped tuples and loose records can be automatically rewritten to their typed counterparts when function parameter types are known at compile time.

## Performance Notes

- Records are immutable — updates create new records
- Field access is O(1) when the struct type is known at preprocess time (compile-time index resolution)
- When the type is unknown, field access falls back to O(log n) binary search over sorted field names
- Use `record-with` to update multiple fields at once and minimize intermediate allocations

### Type-Directed Optimizations

When the static analysis system knows a value's struct type, the preprocessor rewrites field operations to skip runtime name lookups:

- **Field read** `(:field record)` → `&record:nth record <index>` — direct index access instead of name search
- **Field update** `&record:assoc record :field value` → `&record:assoc-at record <index> value` — skips `index_of` binary search
- **Batch update** `record-with record (:f1 v1) (:f2 v2)` → `&record:with-at record <indexes> <values>` — all indices pre-resolved

These rewrites are automatic and transparent. To benefit from them, provide type annotations via `:schema` or `hint-fn` so the preprocessor can resolve struct types.
