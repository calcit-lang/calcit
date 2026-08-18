---
title: "JavaScript Interop"
scope: "core"
kind: "reference"
category: "features"
aliases:
  - "javascript interop"
  - "js interop"
  - "promise"
  - "js-await"
entry_for:
  - "js-await"
  - "hint-fn"
  - ".!"
  - ".-"
---

# JavaScript Interop

Calcit keeps JS interop syntax intentionally small. This page covers the existing core patterns:

- global access
- property access
- method call
- array/object construction
- constructor call with `new`

## Typed FFI boundary

Raw JavaScript values are not ordinary `Dynamic` Calcit values. Property reads,
native method calls, `aget`, untyped `js-get`, and `js/...` calls are
conservatively inferred as `JsNullish<JsObject>`:

- `JsNullish` is reserved for the actual JavaScript `null`/`undefined` boundary;
  it is deliberately distinct from legacy `Optional` and nominal `Option`.
- `JsObject` is an opaque host value. A `js-present?`/`js-nullish?` check proves only that the
  value is present; it does not prove that the payload is a Calcit `String`,
  `Number`, struct, or collection.
- Before passing the value into strongly typed Calcit code, validate/convert it
  with a boundary decoder. `unsafe-coerce` is available only when an external
  API contract is trusted and the unchecked conversion is intentional.

Plain `.-name` and `.!name` dereference their receiver. If the receiver is an
`JsNullish<JsObject>`, preprocessing reports `W_JS_FFI_NULLABLE_DEREF`. Use
`.?-name`/`.?!name`, or narrow the receiver before dereferencing it.

Functions containing raw interop should declare `:features $ #{} :js-ffi` in
their schema. The feature identifies the boundary but does not suppress
nullable dereference or strong-type mismatch diagnostics.

### Capability policy

The active entry controls how an unmarked host operation is handled. Existing
projects default to `:allow` for migration; use `:warn` to inventory call sites
or `:error` to reject them during preprocessing:

```cirru.no-check
:feature-policy $ {}
  :js-ffi :error
```

The gate applies to `js/...`, JavaScript syntax such as `new` and `js-await`,
native `.-`/`.!` access, typed external-object access, and host
`unsafe-coerce`. It is lexical: a normal function may call a typed FFI wrapper
without declaring `:js-ffi`; only the wrapper's own implementation body needs
the feature. An anonymous function uses the feature declared in its own
`hint-fn` schema.

Entries can additionally declare an explicit host target:

```cirru
:target :browser
```

Supported targets are `:browser`, `:node`, `:native`, and `:wasm`. A definition's
`:ffi` metadata may add `:target :browser` or `:target :node`; an external-object
operation or a raw host operation in that definition then fails before codegen
when the selected entry targets another host. Omitting `:target` preserves
legacy projects and disables this target-specific check.

Use `js-nullish?` and `js-present?` to narrow a JavaScript boundary. Applying
legacy `nil?`/`some?` reports `W_JS_FFI_NULLABLE_PREDICATE`. Convert explicitly
with `js-nullish->option` only after accepting the opaque payload contract;
generic `optionally` does not accept `JsNullish<T>`.

### Opt-in: flagging untyped access points

`.-name`, `.!name`, `aget`, `aset`, `js-get`, and `js-set` against a bare
`JsObject` receiver (no external-object trait attached) still work, but nothing
about the field is checked. Running with `--warn-dyn-method` additionally
reports `W_JS_FFI_UNTYPED_ACCESS` for these calls whenever the field key is a
literal tag/string, since that is the case where declaring a trait for the
receiver is directly actionable. A dynamic (non-literal) key, or a receiver
that already carries trait or nullable evidence, does not trigger it.

## ES modules with typed adapters

Import npm packages with the ordinary namespace rules, then keep the unchecked
host boundary in one small adapter namespace. A default export uses `:default`;
named exports use `:refer`:

```cirru.no-check
ns app.npm.ids $ :require
  |nanoid :refer $ nanoid

defn make-id (size)
  let
      generate $ unsafe-coerce nanoid $ :: 'Fn
        {}
          :args $ [] 'Number
          :return 'String
    generate size
```

For a module object, declare only the methods or fields your adapter needs as
an external trait, then coerce the raw import once. `unsafe-coerce` emits the
original JS value; its declared type is static evidence for subsequent Calcit
method calls and JavaScript lowering. Keep that assertion at the adapter
boundary. Application namespaces should call ordinary schema-typed wrappers,
not pass npm `JsObject` values around.

External trait members translate common Calcit names by default:
`text-content → textContent`, `matches? → matches`, and `set-item! → setItem`.
For any exception, declare the exact JavaScript key in the trait's `:ffi
:names` map. The override is emitted with bracket access, so keys containing
punctuation are supported.

`js-get` and `js-set` also use external trait field declarations when both the
receiver type and key are static. A tag or string literal key is checked against
the trait: `js-get` returns `JsNullish<FieldType>`, while `js-set` checks the
assigned value and emits the mapped JavaScript property name. Unknown fields
report `W_JS_FFI_UNKNOWN_FIELD`; incompatible writes report
`W_JS_FFI_FIELD_TYPE_MISMATCH`. External fields are read-only by default. Add
only the fields that a host API permits to `:ffi :writable`; otherwise `js-set`
reports `W_JS_FFI_FIELD_READONLY` under a `:warn` or `:error` policy.

`cr query def namespace/name --json` includes the normalized `ffi` metadata.
The human-readable form prints an `FFI:` line, making `:names`, `:writable`,
`:backend`, `:kind`, and `:target` inspectable without opening the snapshot.

```cirru.no-check
:ffi $ {}
  :backend :js
  :kind :external-object
  :writable $ #{} :value :checked
```

```cirru.no-check
defn clear-input (element)
  js-set element :value |
```

Dynamic keys retain raw JavaScript semantics. Use `aget`/`aset` when bypassing
the external trait contract is intentional. When a trusted raw receiver needs
typed field access, establish that evidence once with `unsafe-coerce` at the
adapter boundary rather than coercing each field value.

A nominal `Option<T>` uses `.some?`/`.none?`. Preprocessing reports
`W_NOMINAL_ENUM_LEGACY_USE` when old nullable checks are applied to an Option,
so an API migration cannot silently preserve the wrong branch behavior.

```cirru.no-run
let
    node $ .?!querySelector js/document |.app
  if (js-present? node)
    do
      ; node is narrowed to opaque JsObject here
      .-textContent node
      ; host absence becomes ordinary Calcit data only by explicit conversion
      js-nullish->option node
    %none
```

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
.?!setItem js/localStorage |key |value
```

Optional method call is supported with `.?!name`.

> Note: `.m` and `.!m` are different. `.m` is Calcit method dispatch (traits/impls), while `.!m` is native JavaScript method invocation.

## Construct arrays

Use `js-array` for JavaScript arrays:

```cirru.no-check
let
    a $ js-array 1 2
  .!push a 3 4
  , a
```

## Construct objects

Use `js-object` with key/value pairs:

```cirru.no-check
js-object
  :a 1
  :b 2
```

`js-object` is a macro that validates input shape, so each entry must be a pair.

Equivalent single-line form:

```cirru.no-check
js-object (:a 1) (:b 2)
```

## Create instances with `new`

Use `new` with a constructor symbol:

```cirru.no-check
new js/Date
```

With arguments:

```cirru.no-check
new js/Array 3
```

## Async interop patterns

Calcit provides async interop syntax for JS codegen.

### Mark async functions

Use `hint-fn $ {} (:async true)` in function body when using `js-await`:

`js-await` should stay inside async-marked function bodies.

```cirru.no-check
let
    fetch-data $ fn () nil
  fn ()
    hint-fn $ {} (:async true)
    js-await $ fetch-data
```

### Await promises

Use `js-await` for Promise-like values:

```cirru.no-check
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

```cirru.no-check
defn timeout (ms)
  new js/Promise $ fn (resolve _reject)
    js/setTimeout resolve ms
```

Then consume it inside async function:

```cirru.no-check
let
    timeout $ fn (ms) $ new js/Promise $ fn (resolve _reject)
      js/setTimeout resolve ms
  fn ()
    hint-fn $ {} (:async true)
    js-await $ timeout 200
```

### Async iteration

Use `js-for-await` with `js-await` for async iterables:

```cirru.no-check
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
