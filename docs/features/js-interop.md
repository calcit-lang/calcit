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
  - "js-get"
  - "js-set"
  - "js-present?"
  - "hint-fn"
  - ".!"
  - ".-"
---

# JavaScript Interop

Calcit keeps JavaScript interop deliberately small and explicit. Use this page
as a decision guide:

1. Keep host values opaque at the boundary.
2. Put raw host operations in a small adapter function with `:js-ffi`.
3. Prefer an external trait when a host object has a stable field/method shape.
4. Return ordinary Calcit `Struct`/`Enum`/`Option`/`Result` data to application code.

The syntax reference is at the end; the earlier sections explain the safety
and portability rules that apply to every form.

## 1. Boundary model: opaque first, typed second

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

Use `js-nullish?` and `js-present?` to narrow a JavaScript boundary. Applying
legacy `nil?`/`some?` reports `W_JS_FFI_NULLABLE_PREDICATE`. Convert explicitly
with `js-nullish->option` only after accepting the opaque payload contract;
generic `optionally` does not accept `JsNullish<T>`.

## 2. Capability and target policy

### 2.1 Capability policy

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

### 2.2 Host target policy

Entries can additionally declare an explicit host target:

```cirru.no-check
:target :browser
```

Supported targets are `:browser`, `:node`, `:native`, and `:wasm`. A definition's
`:ffi` metadata may add `:target :browser` or `:target :node`; an external-object
operation or a raw host operation in that definition then fails before codegen
when the selected entry targets another host. Omitting `:target` preserves
legacy projects and disables this target-specific check.

### 2.3 Audit untyped access points

`.-name`, `.!name`, `aget`, `aset`, `js-get`, and `js-set` against a bare
`JsObject` receiver (no external-object trait attached) still work, but nothing
about the field is checked. Running with `--warn-dyn-method` additionally
reports `W_JS_FFI_UNTYPED_ACCESS` for these calls whenever the field key is a
literal tag/string, since that is the case where declaring a trait for the
receiver is directly actionable. A dynamic (non-literal) key, or a receiver
that already carries trait or nullable evidence, does not trigger it.

## 3. Typed adapters and external objects

An adapter should have one job: turn a host value or host failure into a stable
Calcit value. Keep `unsafe-coerce`, raw `js/...`, and native member access in
that adapter; callers should see only its ordinary schema.

### 3.1 ES modules with typed adapters

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

For example, the host name can differ from the Calcit name while the field type
stays visible to the type checker:

```cirru.no-check
deftrait StorageHost (:length 'Number)
  .get-item $ :: 'Fn
    {}
      :args $ [] 'StorageHost 'String
      :return $ :: 'JsNullish 'String

:ffi $ {}
  :backend :js
  :kind :external-object
  :names $ {} (:get-item |getItem)
```

Inspect the resulting contract with:

```text
cr query def namespace/StorageHost --json
```

The output includes the schema and `ffi` metadata, including host member names,
target, and writable fields. This makes the snapshot auditable without reading
generated JavaScript.

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

### 3.2 Keep browser and Node bindings separate

Shared business code should depend on normalized Calcit data, not on browser or
Node globals. A practical layout is:

```text
app.shared      ordinary schemas and business logic
app.browser     browser-only adapters, entry target :browser
app.node        Node-only adapters, entry target :node
```

Mark a binding definition with FFI target metadata when it is host-specific:

```cirru.no-check
:ffi $ {} (:backend :js) (:target :browser)
```

The compiler rejects a browser binding selected from a Node entry (and the
reverse) before JavaScript codegen with `E_JS_FFI_TARGET_MISMATCH`. The `:mode`
field still means native versus JavaScript execution; it does not identify the
host, so use `:target` for that purpose. An entry without `:target` remains
compatible with older projects and does not enable target validation.

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

## 4. Syntax reference

### 4.1 Access global values

Use `js/...` to read JavaScript globals and nested members:

```cirru.no-run
do js/window.innerWidth
```

### 4.2 Access properties

Use `.-name` for property access:

```cirru.no-run
let
    obj $ js-object (:name |Alice)
  .-name obj
```

This compiles to direct JS member access. For non-identifier keys, Calcit uses bracket access automatically.

Optional access is also supported with `.?-name`, which maps to optional chaining style access.

### 4.3 Call methods

Use `.!name` for native JS method calls (object first, then args):

```cirru.no-run
.?!setItem js/localStorage |key |value
```

Optional method call is supported with `.?!name`.

> Note: `.m` and `.!m` are different. `.m` is Calcit method dispatch (traits/impls), while `.!m` is native JavaScript method invocation.

### 4.4 Construct arrays

Use `js-array` for JavaScript arrays:

```cirru.no-check
let
    a $ js-array 1 2
  .!push a 3 4
  , a
```

### 4.5 Construct objects

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

### 4.6 Create instances with `new`

Use `new` with a constructor symbol:

```cirru.no-check
new js/Date
```

With arguments:

```cirru.no-check
new js/Array 3
```

## 5. Async interop patterns

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

## 6. Diagnostics and validation checklist

When adding or reviewing a JS FFI adapter, check these in order:

1. Does the public function return a concrete Calcit schema instead of leaking
   `JsObject` or `Dynamic`?
2. Is every function body containing raw host syntax marked with
   `:features $ #{} :js-ffi`?
3. Are browser/Node-only definitions marked with `:ffi :target` and selected
   by an entry with the matching `:target`?
4. Are nullable host results represented as `JsNullish<T>` until the adapter
   explicitly converts them to `Option<T>`?
5. Are external fields declared on a trait, with only genuinely mutable fields
   listed in `:writable`?
6. Does `cr query def namespace/name --json` show the expected schema and FFI
   metadata?

Useful checks for a project with separate entries are:

```text
cr --entry browser calcit.cirru --check-only
cr --entry node calcit.cirru --check-only
cr --entry browser calcit.cirru js
cr --entry node calcit.cirru js
```

Generate the two targets serially when they share one `js-out/` directory. A
parallel browser/Node codegen run can overwrite generated modules while the
other target is still writing them, which makes runtime smoke tests unreliable.

Common diagnostics:

| Code | Meaning | Typical fix |
| --- | --- | --- |
| `E_JS_FFI_FEATURE_REQUIRED` | A host operation is outside a marked adapter body. | Add `:js-ffi` to that implementation schema or move the operation into a wrapper. |
| `E_JS_FFI_TARGET_MISMATCH` | The selected entry targets another host. | Correct the entry `:target` or use the matching adapter. |
| `W_JS_FFI_NULLABLE_DEREF` | A nullable host value is dereferenced directly. | Use optional access or narrow with `js-present?`. |
| `E_JS_FFI_FIELD_READONLY` | A typed external field is written without permission. | Add the field to `:ffi :writable` only if the host API permits it. |
```
