# Tuples

Tuples in Calcit are tagged unions that can hold multiple values with a tag. They are used for representing structured data and are the foundation for records and enums.

## Quick Recipes

- **Create**: `:: :point 10 20`
- **Create Typed**: `%:: Shape :circle 5`
- **Access**: `&tuple:nth t 1`
- **Match**: `tag-match t ((:point x y) ...)`
- **Update**: `&tuple:assoc t 1 99`

## Creating Tuples

### Shorthand Syntax

Use `::` to create a tuple with a tag:

```cirru
let
    result 42
    message |error-occurred
  do
    :: :point 10 20
    :: :ok result
    :: :err message
```

### With Class Syntax

Use `%::` to create a typed tuple from an enum:

```cirru
let
    Shape $ defenum Shape (:point :number :number) (:circle :number)
  %:: Shape :point 10 20
```

## Tuple Structure

A tuple consists of:

- **Tag**: A keyword identifying the tuple type (index 0)
- **Class**: Optional class metadata (hidden)
- **Parameters**: Zero or more values (indices 1+)

```cirru
let
    t $ :: :point 10 20
  ; Index 0: :point, Index 1: 10, Index 2: 20
  [] (&tuple:nth t 0) (&tuple:nth t 1) (&tuple:nth t 2)
```

## Accessing Tuple Elements

```cirru
let
    t $ :: :point 10 20
  &tuple:nth t 0
  ; => :point

  &tuple:nth t 1
  ; => 10

  &tuple:nth t 2
  ; => 20
```

## Tuple Properties

```cirru
let
    t $ :: :point 10 20
  do
    ; count includes the tag
    &tuple:count (:: :a 1 2 3)
    ; => 4
    &tuple:params t
    ; => ([] 10 20)
    &tuple:enum t
    ; => nil (plain tuple, not from enum)
```

`&tuple:enum` is the source-prototype API for tuples:

- If tuple is created from enum (`%::`), it returns that enum value.
- If tuple is created as plain tuple (`::`), it returns `nil`.

```cirru
do
  let
      plain $ :: :point 10 20
    nil? $ &tuple:enum plain
    ; => true
  let
      ApiResult $ defenum ApiResult (:ok :number) (:err :string)
      ok $ %:: ApiResult :ok 1
    assert= ApiResult $ &tuple:enum ok
```

### Accurate Origin Check (Enum Eq)

```cirru
let
    ApiResult $ defenum ApiResult (:ok :number) (:err :string)
    x $ %:: ApiResult :ok 1
  assert= (&tuple:enum x) ApiResult
```

### Complex Branching Example (Safe + Validation)

```cirru
do
  defenum Result
    :ok :number
    :err :string
  let
      xs $ []
        %:: Result :ok 1
        %:: Result :err |bad
        :: :plain 42
    if (nil? (&tuple:enum (&list:nth xs 2)))
      if (= (&tuple:enum (&list:nth xs 0)) Result)
        , |result-and-plain
        , |result-missing
      , |unexpected
```

## Updating Tuples

```cirru
; Update element at index
&tuple:assoc (:: :point 10 20) 1 100
; => (:: :point 100 20)
```

## Pattern Matching with Tuples

### tag-match

Pattern match on enum/tuple tags:

```cirru
let
    MyResult $ defenum MyResult (:ok :number) (:err :string)
    result $ %:: MyResult :ok 42
  tag-match result
    (:ok v) (str |Success: v)
    (:err msg) (str |Error: msg)
    _ |Unknown
```

### list-match

For simple list-like destructuring:

```cirru
; list-match takes (head rest) branches — rest captures remaining elements as a list
list-match ([] :point 10 20)
  () |Empty
  (h tl) ([] h tl)
```

## Enums as Tuples

Enums are specialized tuples with predefined variants:

```cirru
; Define enum
defenum Option
  :some :dynamic
  :none

; Create enum instances
%:: Option :some 42
%:: Option :none

; Check variant
&tuple:enum-has-variant? Option :some
; => true

; Get variant arity
&tuple:enum-variant-arity Option :some
; => 1
```

## Common Use Cases

### Result Types

```cirru
let
    MyResult $ defenum MyResult (:ok :number) (:err :string)
    divide $ defn divide (a b)
      if (= b 0)
        %:: MyResult :err |Division-by-zero
        %:: MyResult :ok (/ a b)
    result $ divide 10 2
  tag-match result
    (:ok value) (str |ok: value)
    (:err msg) (str |err: msg)
```

### Optional Values

```cirru
let
    MaybeInt $ defenum MaybeInt (:some :number) (:none)
    find-item $ fn (items target)
      reduce items (%:: MaybeInt :none)
        fn (acc x)
          if (= x target) (%:: MaybeInt :some x) acc
    result $ find-item ([] 1 2 3) 2
  tag-match result
    (:some v) v
    _ |not-found
```

### Tagged Data

```cirru
; Represent different message types
:: :greeting |Hello
:: :number 42
:: :list ([] 1 2 3)
```

## Type Annotations

```cirru
let
    ApiResult $ defenum ApiResult (:ok :string) (:err :string)
    process-result $ defn process-result (r)
  hint-fn $ {} (:args ([] :dynamic)) (:return :string)
      tag-match r
        (:ok v) (str v)
        (:err msg) msg
  process-result (%:: ApiResult :ok |done)
```

## Tuple vs Record

| Feature   | Tuple         | Record             |
| --------- | ------------- | ------------------ |
| Access    | By index      | By field name      |
| Structure | Tag + params  | Named fields       |
| Methods   | Via class     | Via traits         |
| Use case  | Tagged unions | Structured objects |

## Performance Notes

- Tuples are immutable
- Element access is O(1)
- `&tuple:assoc` creates a new tuple
- Use records for complex objects with named fields
