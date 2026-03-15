# List

Calcit List is a persistent, immutable vector. In Rust it uses [ternary-tree](https://github.com/calcit-lang/ternary-tree.rs) (optimized 2-3 tree with finger-tree tricks). In JavaScript it uses a similar structure with a fast-path `CalcitSliceList` for append-heavy workloads.

All list operations return new lists — the original is never mutated.

## Quick Recipes

- **Create**: `[] 1 2 3` or `range 5`
- **Access**: `nth xs 0`, `first xs`, `last xs`
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
  ; => ([] 1 2 3 4 5)
```

`range` generates a sequence:

```cirru
let
    r1 $ range 5
    r2 $ range 2 7
  println r1
  ; => ([] 0 1 2 3 4)
  println r2
  ; => ([] 2 3 4 5 6)
```

## Accessing Elements

```cirru
let
    xs $ [] 10 20 30 40
  println $ nth xs 0
  ; => 10
  println $ first xs
  ; => 10
  println $ last xs
  ; => 40
  println $ count xs
  ; => 4
```

`get` is an alias for `nth`:

```cirru
let
    xs $ [] :a :b :c
  println $ get xs 1
  ; => :b
```

## Adding / Removing Elements

```cirru
let
    xs $ [] 1 2 3
  println $ append xs 4
  ; => ([] 1 2 3 4)
  println $ prepend xs 0
  ; => ([] 0 1 2 3)
  println $ conj xs 4 5
  ; => ([] 1 2 3 4 5)
  println $ concat xs $ [] 4 5
  ; => ([] 1 2 3 4 5)
```

Update or remove by index:

```cirru
let
    xs $ [] 1 2 3
  println $ assoc xs 1 99
  ; => ([] 1 99 3)
  println $ dissoc xs 1
  ; => ([] 1 3)
```

## Slicing & Reordering

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ rest xs
  ; => ([] 2 3 4 5)
  println $ butlast xs
  ; => ([] 1 2 3 4)
  println $ slice xs 1 3
  ; => ([] 2 3)
  println $ take xs 3
  ; => ([] 1 2 3)
  println $ take-last xs 2
  ; => ([] 4 5)
  println $ drop xs 2
  ; => ([] 3 4 5)
```

Sort (default ascending):

```cirru
let
    xs $ [] 3 1 4 1 5
  println $ sort xs
  ; => ([] 1 1 3 4 5)
```

Sort by key function (method-style):

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ .sort-by xs $ fn (x) (- 0 x)
  ; => ([] 5 4 3 2 1)
```

Reverse:

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ reverse xs
  ; => ([] 5 4 3 2 1)
```

## Filtering & Finding

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ filter xs $ fn (x) (> x 3)
  ; => ([] 4 5)
  println $ filter-not xs $ fn (x) (> x 3)
  ; => ([] 1 2 3)
  println $ find xs $ fn (x) (> x 3)
  ; => 4
  println $ find-index xs $ fn (x) (> x 3)
  ; => 3
  println $ index-of xs 3
  ; => 2
```

## Transforming

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ map xs $ fn (x) (* x 2)
  ; => ([] 2 4 6 8 10)
  println $ map-indexed xs $ fn (i x) ([] i x)
  ; => ([] ([] 0 1) ([] 1 2) ([] 2 3) ([] 3 4) ([] 4 5))
```

Flatten one level of nesting (method-style):

```cirru
let
    nested $ [] ([] 1 2) ([] 3 4) ([] 5)
  println $ .flatten nested
  ; => ([] 1 2 3 4 5)
```

## Aggregating

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ reduce xs 0 $ fn (acc x) (+ acc x)
  ; => 15
  println $ foldl xs 0 $ fn (acc x) (+ acc x)
  ; => 15
  println $ any? xs $ fn (x) (> x 4)
  ; => true
  println $ every? xs $ fn (x) (> x 0)
  ; => true
```

`group-by` partitions into a map keyed by the return value of the function:

```cirru
let
    xs $ [] 1 2 3 4 5
  println $ group-by xs $ fn (x) (if (> x 3) :big :small)
  ; => ({} (:big ([] 4 5)) (:small ([] 1 2 3)))
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
  println $ .to-set xs
  ; => (#{} 1 2 3)
```

## Thread Macro Pipelines

The `->` thread macro is idiomatic for list transformations:

```cirru
let
    result $ -> (range 10)
      filter $ fn (x) (> x 5)
      map $ fn (x) (* x x)
  println result
  ; => ([] 36 49 64 81)
```

## Common Patterns

### Building lists incrementally

```cirru
let
    source $ [] 1 2 3 4 5
    init $ []
    result $ foldl source init $ fn (acc item)
      if (> item 2)
        append acc (* item 10)
        , acc
  println result
  ; => ([] 30 40 50)
```

### Zip two lists together

```cirru
let
    ks $ [] :a :b :c
    vs $ [] 1 2 3
    zipped $ map-indexed ks $ fn (i k) ([] k (nth vs i))
  println zipped
  ; => ([] ([] :a 1) ([] :b 2) ([] :c 3))
```

### Deduplicate

Convert to set (removes duplicates, loses order):

```cirru
let
    xs $ [] 1 2 2 3 3 3
  println $ .to-set xs
  ; => (#{} 1 2 3)
```

## Implementation Notes

- `nth` and `get` are O(log n) on the ternary tree structure.
- `append` and `prepend` are amortized O(1) in the Rust implementation.
- `concat` is O(m) where m is the size of the appended list.
- Lists are zero-indexed.
