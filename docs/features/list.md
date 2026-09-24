---
title: "List"
summary: "Calcit List 的构造、访问、更新、遍历和持久化行为"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "vector"
  - "range"
  - "append"
  - "nth"
entry_for:
  - "calcit.core/nth"
  - "calcit.core/first"
  - "calcit.core/rest"
  - "calcit.core/append"
  - "calcit.core/assoc"
code_refs:
  - "calcit.core/nth"
  - "calcit.core/first"
  - "calcit.core/rest"
  - "calcit.core/append"
  - "calcit.core/assoc"
id: core/features/list
parent: core/features
related:
  - core/features/hashmap
  - core/data/persistent-data
requires:
  - core/features
leads_to:
  - core/run/query
---
# List

Calcit List is a persistent, immutable vector. In Rust it uses [ternary-tree](https://github.com/calcit-lang/ternary-tree.rs) (optimized 2-3 tree with finger-tree tricks). In JavaScript it uses a similar structure with a fast-path `CalcitSliceList` for append-heavy workloads.

All list operations return new lists — the original is never mutated.

## Quick Recipes

- **Create**: `[] 1 2 3` or `range 5`
- **Access**: `xs.nth 0`, `xs.first`, `xs.last`
- **Modify**: `append xs 4`, `prepend xs 0`, `assoc xs 1 99`
- **Transform**: `map xs f`, `filter xs f`, `reduce xs 0 f`
- **Combine**: `concat xs ys`, `slice xs 1 3`

## Creating Lists

```cirru
let
    empty-list $ []
    nums $ [] 1 2 3 4 5
    words $ [] |foo |bar |baz
  println nums
  ; => $ [] 1 2 3 4 5
```

`range` generates a sequence:

```cirru
let
    r1 $ range 5
    r2 $ range 2 7
  println r1
  ; => $ [] 0 1 2 3 4
  println r2
  ; => $ [] 2 3 4 5 6
```

## Accessing Elements

```cirru
let
    xs $ [] 10 20 30 40
  println $ nth xs 0
  ; => (%some 10)
  println $ first xs
  ; => (%some 10)
  println $ last xs
  ; => (%some 40)
  println $ count xs
  ; => 4
```

`List<T>` 的 `.get` 与 `.nth` 都返回 `Option<T>`：

```cirru
let
    xs $ [] :a :b :c
  println $ xs.get 1
  ; => (%some :b)
```

已知 `List<T>` 的访问优先使用接收者方法 `.get`、`.nth`、`.first` 和 `.last`，让元素类型随 receiver 进入推断；对应的前缀函数仍可使用，并不是 `.get` 的替代契约。索引不存在时返回 `%none`，不要把缺失当作 `nil`。

### 索引范围与元素成员

`xs.contains? index` 检查 Number 索引是否在有效范围内；`xs.includes? value` 检查列表中是否有指定的 `T` 元素。两者不是别名：查询元素时不要把值误传给 `.contains?`。

```cirru
let
    xs $ [] :a :b :c
  println $ xs.contains? 1
  ; => true
  println $ xs.contains? 3
  ; => false
  println $ xs.includes? :b
  ; => true
  println $ xs.includes? :d
  ; => false
```

## Adding / Removing Elements

```cirru
let
    xs $ [] 1 2 3
  println $ append xs 4
  ; => $ [] 1 2 3 4
  println $ prepend xs 0
  ; => $ [] 0 1 2 3
  println $ conj xs 4 5
  ; => $ [] 1 2 3 4 5
  println $ concat xs ([] 4 5)
  ; => $ [] 1 2 3 4 5
```

Update or remove by index:

```cirru
let
    xs $ [] 1 2 3
  println $ assoc xs 1 99
  ; => $ [] 1 99 3
  println $ dissoc xs 1
  ; => $ [] 1 3
```

## Slicing & Reordering

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ rest xs
  ; => $ [] 2 3 4 5
  println $ butlast xs
  ; => $ [] 1 2 3 4
  println $ slice xs 1 3
  ; => $ [] 2 3
  println $ take xs 3
  ; => $ [] 1 2 3
  println $ take-last xs 2
  ; => $ [] 4 5
  println $ drop xs 2
  ; => $ [] 3 4 5
```

Sort (default ascending):

```cirru
let
    xs $ [] 3 1 4 1 5
  println $ sort xs
  ; => $ [] 1 1 3 4 5
```

按 key 函数排序：

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ &list:sort-by xs
    fn (x) (- 0 x)
  ; => $ [] 5 4 3 2 1
```

对于 `List<T>`，作为 selector 的函数按 `T -> K` 检查；排序键 `K` 保持泛型。Tag selector 仍可用于读取字段或 Map key，但新代码优先写出可推断的函数。

Reverse:

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ reverse xs
  ; => $ [] 5 4 3 2 1
```

## Filtering & Finding

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ filter xs
    fn (x) (> x 3)
  ; => $ [] 4 5
  println $ filter-not xs
    fn (x) (> x 3)
  ; => $ [] 1 2 3
  println $ find xs
    fn (x) (> x 3)
  ; => (%some 4)
  println $ find-index xs
    fn (x) (> x 3)
  ; => (%some 3)
  println $ index-of xs 3
  ; => (%some 2)
```

`find` 返回 `Option<T>`，`find-index` 和 `index-of` 返回 `Option<Number>`；找不到时为 `%none`，使用前应明确处理该分支。

## Transforming

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ map xs
    fn (x) (* x 2)
  ; => $ [] 2 4 6 8 10
  println $ map-indexed xs
    fn (i x) ([] i x)
  ; => $ [] ([] 0 1) ([] 1 2) ([] 2 3) ([] 3 4) ([] 4 5)
```

将嵌套列表展开一层：

```cirru
let
    nested $ [] ([] 1 2) ([] 3 4) ([] 5)
  println $ &list:flatten nested
  ; => $ [] 1 2 3 4 5
```

## Aggregating

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ reduce xs 0
    fn (acc x)
      hint-fn $ {}
        :args $ [] 'Number 'Number
        :return 'Number
      + acc x
  ; => 15
  println $ foldl xs 0
    fn (acc x)
      hint-fn $ {}
        :args $ [] 'Number 'Number
        :return 'Number
      + acc x
  ; => 15
  println $ any? xs
    fn (x) (> x 4)
  ; => true
  println $ every? xs
    fn (x) (> x 0)
  ; => true
```

`group-by` partitions into a map keyed by the return value of the function:

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ group-by xs
    fn (x)
      if (> x 3) :big :small
  ; => $ {}
    :big $ [] 4 5
    :small $ [] 1 2 3
```

## Strings from Lists

```cirru
let
    words $ [] |hello |world |foo
  println $ join-str words |,
  ; => hello,world,foo
```

## Converting

```cirru
let
    xs $ [] 1 2 2 3 3 3
  println $ &list:to-set xs
  ; => $ #{} 1 2 3
```

## Thread Macro Pipelines

The `->` thread macro is idiomatic for list transformations:

```cirru
let
    result $ -> (range 10)
      filter $ fn (x) (> x 5)
      map $ fn (x) (* x x)
  println result
  ; => $ [] 36 49 64 81
```

## Common Patterns

### Building lists incrementally

```cirru
let
    source $ [] 1 2 3 4 5
    init $ []
    result $ foldl source init
      fn (acc item)
        if (> item 2)
          append acc $ * item 10
          , acc
  println result
  ; => $ [] 30 40 50
```

### 按索引配对两个 List

`nth` 返回 `Option<T>`，所以第二个 List 较短时仍能表示缺失，不应把结果误写成必有的裸值：

```cirru
let
    ks $ [] :a :b :c
    vs $ [] 1 2 3
    zipped $ map-indexed ks
      fn (i k)
        [] k $ nth vs i
  println zipped
  ; => $ [] ([] :a (%some 1)) ([] :b (%some 2)) ([] :c (%some 3))
```

### Deduplicate

Convert to set (removes duplicates, loses order):

```cirru
let
    xs $ [] 1 2 2 3 3 3
  println $ &list:to-set xs
  ; => $ #{} 1 2 3
```

## Implementation Notes

- `nth` and `get` are O(log n) on the ternary tree structure.
- `append` and `prepend` are amortized O(1) in the Rust implementation.
- `concat` is O(m) where m is the size of the appended list.
- Lists are zero-indexed.
