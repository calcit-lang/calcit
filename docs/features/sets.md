# Sets

Calcit provides HashSet data structure for storing unordered unique elements. In Rust implementation, it uses `rpds::HashTrieSet`, while in JavaScript it uses a custom implementation based on ternary-tree.

## Quick Recipes

- **Create**: `#{:a :b :c}`
- **Add/Remove**: `include s :d`, `exclude s :a`
- **Check**: `&set:includes? s :a`
- **Operations**: `union s1 s2`, `difference s1 s2`, `intersection s1 s2`
- **Convert**: `&set:to-list s`

## Creating Sets

Use `#{}` to create a set:

```cirru
#{} :a :b :c

#{} 1 2 3 4 5
```

Create an empty set:

```cirru
#{}
```

## Basic Operations

### Adding and Removing Elements

```cirru
; Add element
include (#{} :a :b) :c
; => #{:a :b :c}

; Remove element
exclude (#{} :a :b :c) :b
; => #{:a :c}
```

### Checking Membership

```cirru
&set:includes? (#{} :a :b :c) :a
; => true

&set:includes? (#{} :a :b :c) :x
; => false
```

### Set Operations

```cirru
; Union - elements in either set
union (#{} :a :b) (#{} :b :c)
; => #{:a :b :c}

; Difference - elements in first but not second
difference (#{} :a :b :c) (#{} :b :c :d})
; => #{:a}

; Intersection - elements in both sets
intersection (#{} :a :b :c) (#{} :b :c :d})
; => #{:b :c}
```

## Converting Between Types

```cirru
; Convert set to list
&set:to-list (#{} :a :b :c)
; => ([] :a :b :c)  ; order may vary

; Convert list to set
&list:to-set ([] :a :b :b :c)
; => #{:a :b :c}
```

## Set Properties

```cirru
; Get element count
&set:count (#{} :a :b :c)
; => 3

; Check if empty
&set:empty? (#{})
; => true
```

## Filtering

```cirru
&set:filter (#{} 1 2 3 4 5)
  fn (x) (> x 2)
; => #{3 4 5}
```

## Pattern Matching with Sets

Use `&set:destruct` to destructure sets:

```cirru
&set:destruct (#{} :a :b :c)
; Returns a list of elements
```

## Common Use Cases

### Removing Duplicates from a List

```cirru
-> ([] :a :b :a :c :b)
  &list:to-set
  &set:to-list
; => ([] :a :b :c)  ; order may vary
```

### Checking for Unique Elements

```cirru
= (&set:count (#{} :a :b :c))
  count ([] :a :b :c)
; => true if all elements are unique
```

### Set Membership in Algorithms

```cirru
let
    visited $ #{} :page1 :page2
  if (&set:includes? visited :page3)
    println "|Already visited"
    println "|New page found"
```

## Type Annotations

```cirru
defn process-tags (tags)
  hint-fn $ {} (:args ([] :set)) (:return :set)
  &set:filter tags $ fn (t) (not= t :draft)
```

## Performance Notes

- Set operations (union, intersection, difference) are efficient due to persistent data structure sharing
- Membership tests (`&set:includes?`) are O(1) average case
- Sets are immutable - all operations return new sets
