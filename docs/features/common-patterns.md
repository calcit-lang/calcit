---
title: "Common Patterns"
summary: "实用编程模式：集合操作、错误处理、字符串处理、状态管理、控制流（含 match/try/cond/pipe）、文件操作"
scope: "core"
kind: "guide"
category: "features"
aliases:
  - "common recipes"
  - "pattern examples"
  - "common tasks"
---

# Common Patterns

This document provides practical examples and patterns for common programming tasks in Calcit.

## Quick Recipes

- **Filter/Map**: `-> xs (filter f) (map g)`
- **Group**: `group-by xs f`
- **Find**: `find xs f`, `index-of xs v`
- **Check All/Any**: `every? xs f`, `any? xs f`
- **State**: `defref *state 0`, `reset! *state 1`, `swap! *state inc`

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
      hint-fn $ {}
        :args $ [] $ :: 'List 'String
        :return $ :: 'Map 'Number $ :: 'List 'String
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
      match result
        (:ok v) (println $ str "|Result: " v)
        (:err msg) (println $ str "|Error: " msg)
  handle-result $ safe-divide 10 2
  handle-result $ safe-divide 10 0
```

### Using Option Type

```cirru
let
    find-user $ fn (users id)
      hint-fn $ {}
        :args $ [] (:: 'List (:: 'Map 'Tag 'String)) 'String
        :return $ :: 'Option $ :: 'Map 'Tag 'String
      find users $ fn (u)
        if-let
          user-id $ get u :id
          = user-id id
          , false
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
    result3 $ update-in data ([] :a :b :c)
      fn (current)
        inc $ current .unwrap
  println result1
  ; => (Option :some 1)
  println result2
  ; => {} (:a $ {} (:b $ {} (:c 100)))
  println result3
  ; => {} (:a $ {} (:b $ {} (:c 2)))
```

`update-in` 将 `Option<T>` 传给 updater。已有值对应 `Option :some value`，缺失叶子对应
`Option :none`；只有已证明路径存在时才使用 `.unwrap`，否则通过 `.fold` / `.unwrap-or`
明确新建语义。空路径会更新 `Option :some data`。

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
    result2 $ join-string ([] :a :b :c) |,
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
    result3 $ str-find-index |hello-world |world
  ; result1 => true
  ; result2 => true
  ; result3 => (Option :some 6) (index of |world in |hello-world)
  [] result1 result2 result3
```

## State Management

### Using Atoms

```cirru
let
    counter $ ref 0
  println $ deref counter
  ; => 0
  reset! counter 10
  ; => 10
  swap! counter inc
  ; => 11
```

Watcher keys are Tags. A watcher on `Ref<T>` receives the new and previous
values as `(T, T)` and must return `Unit`; finish side-effect-only callbacks
with `&unit`. `remove-watch` requires the same Tag key used at registration.

### Managing Collections in State

```cirru.no-check
let
    todos $ ref $ []
    add-todo! $ fn (text)
      swap! todos $ fn (items)
        append items $ {} (:id $ generate-id!) (:text text) (:done false)
    toggle-todo! $ fn (id)
      swap! todos $ fn (items)
        map items $ fn (todo)
          if-let
            todo-id $ get todo :id
            if (= todo-id id)
              assoc todo :done $ not $ -> (get todo :done) (.unwrap-or false)
              , todo
            , todo
  add-todo! |buy-milk
  add-todo! |write-docs
  println $ deref todos
```

## Control Flow Patterns

### Early Return Pattern

把输入声明为 `List<Number>`，并用 `Option<List<Number>>` 表达验证结果；`if-let` 解包后保留集合的元素类型。这个示例区分空输入与包含负数的无效输入。

```cirru
let
    ; stub implementations for demonstration
    validate-data $ fn (data)
      hint-fn $ {}
        :args $ [] $ :: 'List 'Number
        :return $ :: 'Option $ :: 'List 'Number
      if
        and (> (count data) 0)
          every? data $ fn (x) (>= x 0)
        Option :some data
        assert-type (Option :none) (:: 'Option $ :: 'List 'Number)
    transform-data $ fn (validated)
      hint-fn $ {}
        :args $ [] $ :: 'List 'Number
        :return $ :: 'List 'Number
      map validated $ fn (x) (* x 2)
    process-data $ defn process-data (data)
      hint-fn $ {}
        :args $ [] $ :: 'List 'Number
        :return 'Enum
      if (empty? data)
        :: :err |Empty-data
        if-let
          validated $ validate-data data
          let
              result $ transform-data validated
            :: :ok result
          :: :err |Invalid-data
  assert= (:: :ok $ [] 2 4 6) (process-data $ [] 1 2 3)
  assert= (:: :err |Empty-data)
    process-data $ assert-type ([]) (:: 'List 'Number)
  assert= (:: :err |Invalid-data) (process-data $ [] 1 -2 3)
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
  loop
      acc 1
      n n
    if (&<= n 1) acc
      recur
        * acc n
        &- n 1

; Fibonacci with loop/recur
defn fibonacci (n)
  loop
      a 0
      b 1
      n n
    if (&<= n 0) a
      recur b (&+ a b) (&- n 1)
```

### `match` with literal patterns (Multi-branch dispatch)

`match` dispatches on a value against literal patterns, with `_` as the fallback:

```cirru.no-check
match action
  :mount $ do
    js/console.log |Mounted
  :update $ do
    js/console.log |Updated
  :unmount $ do
    js/console.log |Unmounted
  _ nil
```

The first argument is the value to match, followed by pattern-result pairs and an optional final `_` branch. Useful for lifecycle hooks, event handling, and state machine transitions. The older `case-default action nil ...` form (default as the second argument) is deprecated; it still works and expands to the same `match` when all patterns are literals.

## Working with Files

### Reading and Writing

```cirru.no-run
let
    source $ fs:path |data.txt
    content-result source.read-text
  content-result.and-then $ fn (content)
    println content
    Result :ok &unit
```

`fs:path` 构造 nominal `FsPath`，不会规范化路径或触碰文件系统。`.read-text`、`.read-dir`、`.walk-dir` 与 `.write-text!` 都返回 `Result`，因此预期内的 I/O 失败留在类型流中；旧 `.write-text` 已在 0.29.0 退役，调用报告 `E_RETIRED_METHOD`，应改用 `.write-text!`。`.read-dir` 只枚举即时子项，`.walk-dir` 递归枚举。String 不提供文件效果方法；旧 `try-read-file` / `try-write-file` 已退役，`try-read-dir` 与 raw raising procedure 暂留为兼容入口。Native 与 Node-hosted 的生成 JavaScript 支持这些文件效果；browser JavaScript 不提供 `.read-dir` 与 `.walk-dir`。WASI 0.3 Component command 支持 preopen 内最多 4 MiB 的 UTF-8 文本读写；写入采用 create + truncate，失败可能留下部分内容，不保证原子替换。`.read-dir` 和 `.walk-dir` 仍待实现，core WASM 会明确拒绝文件效果。

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
      hint-fn $ {}
        :args $ [] $ :: 'List 'Number
        :return 'Number
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
    result $ fold-while (range 1000) 0 $ fn (acc x)
      if (> x 100) (ControlFlow :break x) (ControlFlow :continue acc)
  println result
```

### Avoiding Intermediate Collections

```cirru
let
    items $ [] ({} (:value 1)) ({} (:value 2)) ({} (:value 3))
    result1 $ reduce items 0 $ fn (acc item)
      hint-fn $ {}
        :args $ [] 'Number $ :: 'Map 'Tag 'Number
        :return 'Number
      let
          value $ get item :value
        match value
          (:some number) (+ acc number)
          (:none) , acc
    result2 $ apply +
      map items $ fn (item)
        hint-fn $ {}
          :args $ [] $ :: 'Map 'Tag 'Number
          :return 'Number
        let
            value $ get item :value
          match value
            (:some number) , number
            (:none) 0
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
3. **Use pattern matching** (`match`, `struct-match`) for control flow；匿名 enum 也直接使用 `match`
4. **Leverage threading macros** (`->`, `->>`) for data pipelines
5. **Use enums for result types** instead of exceptions
6. **Keep functions small** and focused on a single responsibility
