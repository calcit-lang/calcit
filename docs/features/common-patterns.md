# Common Patterns

This document provides practical examples and patterns for common programming tasks in Calcit.

## Quick Recipes

- **Filter/Map**: `-> xs (filter f) (map g)`
- **Group**: `group-by xs f`
- **Find**: `find xs f`, `index-of xs v`
- **Check All/Any**: `every? xs f`, `any? xs f`
- **State**: `defatom state 0`, `reset! state 1`, `swap! state inc`

## Working with Collections

### Filtering and Transforming Lists

```cirru
; Filter even numbers and square them
-> (range 20)
  filter $ fn (n)
    = 0 $ &number:rem n 2
  map $ fn (n)
    * n n
; => ([] 0 4 16 36 64 100 144 196 256 324)
```

### Grouping Data

```cirru
let
    group-by-length $ fn (words)
      group-by words count
  group-by-length ([] |apple |pear |banana |kiwi)
; => {}
;   4 $ [] |pear |kiwi
;   5 $ [] |apple
;   6 $ [] |banana
```

### Finding Elements

```cirru
let
    result1 $ find ([] 1 2 3 4 5) $ fn (x) (> x 3)
    result2 $ index-of ([] :a :b :c :d) :c
    result3 $ any? ([] 1 2 3) $ fn (x) (> x 2)
    result4 $ every? ([] 2 4 6) $ fn (x) (= 0 $ &number:rem x 2)
  println result1
  ; => 4
  println result2
  ; => 2
  println result3
  ; => true
  println result4
  ; => true
```

## Error Handling

### Using Result Type

```cirru
let
    MyResult $ defenum MyResult
      :ok :dynamic
      :err :string
    safe-divide $ fn (a b)
      if (= b 0)
        %:: MyResult :err "|Division by zero"
        %:: MyResult :ok (/ a b)
    handle-result $ fn (result)
      tag-match result
        (:ok v) (println $ str "|Result: " v)
        (:err msg) (println $ str "|Error: " msg)
  handle-result $ safe-divide 10 2
  handle-result $ safe-divide 10 0
```

### Using Option Type

```cirru
let
    MyOption $ defenum MyOption
      :some :dynamic
      :none
    find-user $ fn (users id)
      let
          user $ find users $ fn (u)
            = (get u :id) id
        if (nil? user)
          %:: MyOption :none
          %:: MyOption :some user
  println $ find-user
    [] ({} (:id |001) (:name |Alice))
    , |001
```

## Working with Maps

### Nested Map Operations

```cirru
let
    data $ {} (:a $ {} (:b $ {} (:c 1)))
    result1 $ get-in data $ [] :a :b :c
    result2 $ assoc-in data ([] :a :b :c) 100
    result3 $ update-in data ([] :a :b :c) inc
  println result1
  ; => 1
  println result2
  ; => {} (:a $ {} (:b $ {} (:c 100)))
  println result3
  ; => {} (:a $ {} (:b $ {} (:c 2)))
```

### Merging Maps

```cirru
let
    result1 $ merge
      {} (:a 1) (:b 2)
      {} (:b 3) (:c 4)
      {} (:d 5)
    result2 $ &merge-non-nil
      {} (:a 1) (:b nil)
      {} (:b 2) (:c 3)
  println result1
  ; => {} (:a 1) (:b 3) (:c 4) (:d 5)
  println result2
  ; => {} (:a 1) (:b 2) (:c 3)
```

## String Manipulation

### String Syntax

Calcit has two ways to write strings:

- `|text` - for strings without spaces (shorthand)
- `"|text with spaces"` - for strings with spaces (must use quotes)

```cirru
let
    s1 |HelloWorld
    s2 |hello-world
    s3 "|hello world"
    s4 "|error in module"
  println s1
  ; => |HelloWorld
  println s2
  ; => |hello-world
  println s3
  ; => "|hello world"
  println s4
  ; => "|error in module"
```

### Building Strings

```cirru
let
    result1 $ str |Hello | |World
    result2 $ join-str ([] :a :b :c) |,
    result3 $ str-spaced :error |in :module
  println result1
  ; => |HelloWorld
  println result2
  ; => |a,b,c
  println result3
  ; => "|error in module"
```

### Parsing Strings

```cirru
let
    result1 $ split |hello-world-test |-|
    result2 $ split-lines |line1\nline2\nline3
    result3 $ parse-float |3.14159
  println result1
  ; => ([] |hello |world |test)
  println result2
  ; => ([] |line1 |line2 |line3)
  println result3
  ; => 3.14159
```

### String Inspection

```cirru
let
    result1 $ starts-with? |hello-world |hello
    result2 $ ends-with? |hello-world |world
    result3 $ &str:find-index |hello-world |world
  ; result1 => true
  ; result2 => true
  ; result3 => 6 (index of |world in |hello-world)
  [] result1 result2 result3
```

## State Management

### Using Atoms

```cirru
let
    counter $ atom 0
  println $ deref counter
  ; => 0
  reset! counter 10
  ; => 10
  swap! counter inc
  ; => 11
```

### Managing Collections in State

```cirru
let
    todos $ atom $ []
    add-todo! $ fn (text)
      swap! todos $ fn (items)
        append items $ {} (:id $ generate-id!) (:text text) (:done false)
    toggle-todo! $ fn (id)
      swap! todos $ fn (items)
        map items $ fn (todo)
          if (= (get todo :id) id)
            assoc todo :done $ not (get todo :done)
            , todo
  add-todo! |buy-milk
  add-todo! |write-docs
  println $ deref todos
```

## Control Flow Patterns

### Early Return Pattern

```cirru
let
    ; stub implementations for demonstration
    validate-data $ fn (data) (if (= (count data) 0) nil data)
    transform-data $ fn (validated) (map validated (fn (x) (* x 2)))
    process-data $ defn process-data (data)
      if (empty? data)
        :: :err |Empty-data
        let
            validated $ validate-data data
          if (nil? validated)
            :: :err |Invalid-data
            let
                result $ transform-data validated
              :: :ok result
  process-data ([] 1 2 3)
```

### Pipeline Pattern

```cirru
let
    ; stub implementations for demonstration
    validate-input $ fn (s) s
    parse-input $ fn (s) s
    transform-to-command $ fn (s) (str |cmd/ s)
    process-user-input $ defn process-user-input (input)
      -> input
        trim
        &str:slice 0 100
        validate-input
        parse-input
        transform-to-command
  process-user-input "|hello world"
```

### Loop with Recur

```cirru
; Factorial with loop/recur
defn factorial (n)
  apply-args (1 n)
    fn (acc n)
      if (&<= n 1) acc
        recur
          * acc n
          &- n 1

; Fibonacci with loop/recur
defn fibonacci (n)
  apply-args (0 1 n)
    fn (a b n)
      if (&<= n 0) a
        recur b (&+ a b) (&- n 1)
```

## Working with Files

### Reading and Writing

```cirru.no-run
let
    content $ read-file |data.txt
    lines $ split-lines content
  println content
  &doseq (line lines)
    println line
```

## Math Operations

### Common Calculations

```cirru
let
    round-to $ fn (n places)
      let
          factor $ pow 10 places
        / (round $ * n factor) factor
    clamp $ fn (x min-val max-val)
      -> x
        &max min-val
        &min max-val
    average $ fn (numbers)
      / (apply + numbers) (count numbers)
  println $ round-to 3.14159 2
  ; => 3.14
  println $ clamp 15 0 10
  ; => 10
  println $ average ([] 1 2 3 4 5)
  ; => 3
```

## Debugging

### Inspecting Values

```cirru
let
    ; stub implementations for demonstration
    transform-1 $ fn (x) (assoc x :step1 true)
    transform-2 $ fn (x) (assoc x :step2 true)
    data $ {} (:x 1) (:y 2)
    result $ -> data transform-1 transform-2
    x 5
  assert |Should-be-positive $ > x 0
  assert= 4 (+ 2 2)
  , result
```

## Performance Tips

### Lazy Evaluation

```cirru
let
    result $ foldl-shortcut
      range 1000
      , nil nil
      fn (acc x)
        if (> x 100)
          :: true x
          :: false nil
  println result
```

### Avoiding Intermediate Collections

```cirru
let
    items $ [] ({} (:value 1)) ({} (:value 2)) ({} (:value 3))
    result1 $ reduce items 0 $ fn (acc item)
      + acc (get item :value)
    result2 $ apply +
      map items $ fn (item)
        get item :value
  println result1
  ; => 6
  println result2
  ; => 6
```

## Testing

### Writing Tests

```cirru
let
    test-addition $ fn ()
      assert= 4 (+ 2 2)
      assert= 0 (+ 0 0)
      assert= -5 (+ -2 -3)
    test-with-setup $ fn ()
      let
          input $ {} (:name |test) (:value 42)
        , true
  test-addition
```

## Best Practices

1. **Use type annotations** for function parameters and return values
2. **Prefer immutable data** - use `swap!` instead of manual mutation
3. **Use pattern matching** (`tag-match`, `record-match`) for control flow
4. **Leverage threading macros** (`->`, `->>`) for data pipelines
5. **Use enums for result types** instead of exceptions
6. **Keep functions small** and focused on a single responsibility
