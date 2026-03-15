# JavaScript Interop

Calcit keeps JS interop syntax intentionally small. This page covers the existing core patterns:

- global access
- property access
- method call
- array/object construction
- constructor call with `new`

## Access global values

Use `js/...` to read JavaScript globals and nested members:

```cirru.no-run
do js/window.innerWidth
```

## Access properties

Use `.-name` for property access:

```cirru.no-run
let
    obj $ js-object (:name |Alice)
  .-name obj
```

This compiles to direct JS member access. For non-identifier keys, Calcit uses bracket access automatically.

Optional access is also supported with `.?-name`, which maps to optional chaining style access.

## Call methods

Use `.!name` for native JS method calls (object first, then args):

```cirru.no-run
.!setItem js/localStorage |key |value
```

Optional method call is supported with `.?!name`.

> Note: `.m` and `.!m` are different. `.m` is Calcit method dispatch (traits/impls), while `.!m` is native JavaScript method invocation.

## Construct arrays

Use `js-array` for JavaScript arrays:

```cirru.no-run
let
    a $ js-array 1 2
  .!push a 3 4
  , a
```

## Construct objects

Use `js-object` with key/value pairs:

```cirru.no-run
js-object
  :a 1
  :b 2
```

`js-object` is a macro that validates input shape, so each entry must be a pair.

Equivalent single-line form:

```cirru.no-run
js-object (:a 1) (:b 2)
```

## Create instances with `new`

Use `new` with a constructor symbol:

```cirru.no-run
new js/Date
```

With arguments:

```cirru.no-run
new js/Array 3
```

## Async interop patterns

Calcit provides async interop syntax for JS codegen.

### Mark async functions

Use `hint-fn $ {} (:async true)` in function body when using `js-await`:

`js-await` should stay inside async-marked function bodies.

```cirru.no-run
let
    fetch-data $ fn () nil
  fn ()
    hint-fn $ {} (:async true)
    js-await $ fetch-data
```

### Await promises

Use `js-await` for Promise-like values:

```cirru.no-run
fn ()
  hint-fn $ {} (:async true)
  let
      p $ new js/Promise $ fn (resolve _reject)
        js/setTimeout
          fn () (resolve |done)
          , 100
      result $ js-await p
    , result
```

### Build Promise helpers

A common pattern is wrapping callback APIs with `new js/Promise`:

```cirru.no-run
defn timeout (ms)
  new js/Promise $ fn (resolve _reject)
    js/setTimeout resolve ms
```

Then consume it inside async function:

```cirru.no-run
let
    timeout $ fn (ms) $ new js/Promise $ fn (resolve _reject)
      js/setTimeout resolve ms
  fn ()
    hint-fn $ {} (:async true)
    js-await $ timeout 200
```

### Async iteration

Use `js-for-await` with `js-await` for async iterables:

```cirru.no-run
let
    gen $ fn () nil
  fn ()
    hint-fn $ {} (:async true)
    js-await $ js-for-await (gen)
      fn (item)
        new js/Promise $ fn (resolve _reject)
          js/setTimeout $ fn ()
            resolve item
```
