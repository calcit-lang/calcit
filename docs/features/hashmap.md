# HashMap

Calcit HashMap is a persistent, immutable hash map. In Rust it uses [rpds::HashTrieMap](https://docs.rs/rpds/0.10.0/rpds/#hashtriemap). In JavaScript it is built on [ternary-tree](https://github.com/calcit-lang/ternary-tree.ts).

All map operations return new maps — the original is never mutated.

## Quick Recipes

- **Create**: `{} (:a 1) (:b 2)`
- **Access**: `get m :a`, `contains? m :a`
- **Modify**: `assoc m :c 3`, `dissoc m :a`, `update m :a inc`
- **Transform**: `map-kv m f`, `merge m1 m2`
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

```cirru.no-run
&{} :a 1 :b 2
```

## Reading Values

```cirru
let
    m $ {} (:a 1) (:b 2) (:c 3)
  println $ get m :a
  ; => 1
  println $ get m :missing
  ; => nil
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
  ; => Alice
```

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

Returns a new map. If the callback returns `nil`, the entry is dropped (used as filter):

```cirru
let
    m $ {} (:a 1) (:b 2) (:c 13)
    doubled $ map-kv m $ fn (k v) ([] k (* v 2))
    filtered $ map-kv m $ fn (k v)
      if (> v 10) nil
        [] k v
  println doubled
  ; => ({} (:a 2) (:b 4) (:c 26))
  println filtered
  ; => ({} (:a 1) (:b 2))
```

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
    m $ {} (:a 1) (:b 2)
    val $ get m :missing
  if (nil? val) :default val
  ; => :default
```

### Counting occurrences

```cirru
let
    words $ [] :a :b :a :c :a :b
    init $ {}
    freq $ foldl words init $ fn (acc w)
      let
          cur $ get acc w
          n $ if (nil? cur) 0 cur
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
