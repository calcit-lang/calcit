---
title: "HashMap"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "hash map"
  - "map"
  - "key value"
id: core/features/hashmap
parent: core/features
---
# HashMap

Calcit HashMap is a persistent, immutable hash map. In Rust it uses [rpds::HashTrieMap](https://docs.rs/rpds/0.10.0/rpds/#hashtriemap). In JavaScript it is built on [ternary-tree](https://github.com/calcit-lang/ternary-tree.ts).

All map operations return new maps — the original is never mutated.

## Quick Recipes

- **Create**: `{} (:a 1) (:b 2)`
- **Access**: `get m :a`, `contains? m :a`
- **Modify**: `assoc m :c 3`, `dissoc m :a`, `update m :a inc`
- **Transform**: `map-kv m f`, `filter-map-kv m f`, `merge m1 m2`
- **Keys/Values**: `keys m`, `vals m`, `to-pairs m`

## Creating Maps

`{}` is a macro that takes key-value pairs:

```cirru
let
    m $ {}
      :a 1
      :b 2
      :c 3
  println m
  ; => ({} (:a 1) (:b 2) (:c 3))
```

Inline form:

```cirru
let
    m $ {} (:x 10) (:y 20)
  println m
```

The low-level primitive `&{}` takes flat key-value pairs:

```cirru
&{} :a 1 :b 2
```

## Reading Values

```cirru
let
    m $ {} (:a 1) (:b 2) (:c 3)
  println $ get m :a
  ; => (%some 1)
  println $ get m :missing
  ; => (%none)
  println $ contains? m :b
  ; => true
  println $ count m
  ; => 3
  println $ empty? m
  ; => false
```

### Nested access with `get-in`

```cirru
let
    nested $ {} (:user $ {} (:name |Alice) (:age 30))
  println $ get-in nested $ [] :user :name
  ; => (%some |Alice)
```

`get-in` returns `Option<Dynamic>` (`%some` for a resolved value and `%none`
for a missing path or a `nil` encountered while traversing). Use
`.unwrap-or` or `tag-match` before consuming the payload.

## Modifying Maps

All operations return a new map:

```cirru
let
    m $ {} (:a 1) (:b 2)
    m2 $ assoc m :c 3
    m3 $ dissoc m2 :b
    m4 $ merge m $ {} (:d 4) (:e 5)
  println m2
  ; => ({} (:a 1) (:b 2) (:c 3))
  println m3
  ; => ({} (:a 1) (:c 3))
  println m4
  ; => ({} (:a 1) (:b 2) (:d 4) (:e 5))
```

### Nested update with `assoc-in`

```cirru.no-check
; update a deeply nested value
assoc-in config $ [] :server :port $ 8080
```

## Iterating & Transforming

### `map-kv` — transform entries

Returns a new map. The callback transforms each entry and returns a two-item list
containing the output key and value:

```cirru
let
    m $ {} (:a 1) (:b 2) (:c 13)
    doubled $ map-kv m $ fn (k v) ([] k (* v 2))
  println doubled
  ; => ({} (:a 2) (:b 4) (:c 26))
```

Legacy native and JavaScript execution accepts `nil` or an enum value as a
drop sentinel, but new code must not rely on that behavior. It cannot describe
the callback result precisely and historically was not consistent across
backends. Use `filter-map-kv` when entries may be omitted.

### `filter-map-kv` — typed transform and filter

`filter-map-kv` requires the callback to return a
`MapEntryDecision<OutputKey, OutputValue>`. Return `:keep` with the transformed
key and value, or `:drop` without a payload:

```cirru
let
    m $ {} (:a 1) (:b 2) (:c 13)
    filtered $ filter-map-kv m $ fn (k v)
      if (> v 10)
        %:: MapEntryDecision :drop
        %:: MapEntryDecision :keep k v
  println filtered
  ; => ({} (:a 1) (:b 2))
```

The method form is also available as `.filter-map-kv`. Prefer this API over a
`nil` callback sentinel so the compiler can relate the callback payload to the
resulting map's key and value types.

### `to-pairs` — convert to set of pairs

```cirru
let
    m $ {} (:a 1) (:b 2)
  println $ to-pairs m
  ; => (#{} ([] :a 1) ([] :b 2))
```

### `keys` and `vals`

```cirru
let
    m $ {} (:x 10) (:y 20)
  println $ keys m
  ; => (#{} :x :y)
  println $ vals m
  ; => (#{} 10 20)
```

### `each-kv` — side-effect iteration

```cirru.no-check
each-kv config $ fn (k v)
  println $ str k |: v
```

## Querying

```cirru
let
    m $ {} (:a 1) (:b 2) (:c 3)
  println $ includes? m 2
  ; => true  (checks values)
  println $ contains? m :a
  ; => true  (checks keys)
```

## Building from Other Structures

```cirru.no-check
; from a list of pairs
; each pair is [key value]
foldl my-pairs ({}) $ fn (acc pair)
  assoc acc (nth pair 0) (nth pair 1)
```

Using thread macro to build up a map (inserting as first arg to each step):

```cirru
let
    base $ {} (:a 1) (:b 2)
    result $ merge base $ {} (:c 3) (:d 4)
  println result
```

## Common Patterns

### Default value on missing key

```cirru
let
    m $ {} (:a :one) (:b :two)
    val $ (get m :missing) .unwrap-or :default
  , val
  ; => :default
```

`.unwrap-or` 是 `Option` 的类型安全默认值终点；payload 类型可推断时，fallback 必须
兼容该类型。需要区分 key 存在与缺失时仍使用 `get`，再以 `if-let` 或 `match` 处理
完整 `Option`。

### Counting occurrences

```cirru
let
    words $ [] :a :b :a :c :a :b
    init $ {}
    freq $ foldl words init $ fn (acc w)
      let
          n $ -> (get acc w) (.unwrap-or 0)
        assoc acc w (inc n)
  println freq
  ; ({} (:a 3) (:b 2) (:c 1))
```

### Merging with override

```cirru
let
    defaults $ {} (:host |localhost) (:port 3000) (:debug false)
    overrides $ {} (:port 8080) (:debug true)
  merge defaults overrides
  ; => ({} (:host |localhost) (:port 8080) (:debug true))
```

## Implementation Notes

HashMap key iteration order is not guaranteed. Use `to-pairs` + `sort` if you need stable order. Tags (`:kw`) are the most common key type; string keys also work but tags are faster for equality checks.
