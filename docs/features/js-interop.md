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
native method calls, `aget`, untyped `js-get`, and arbitrary `js/...` calls are
conservatively inferred as `JsNullish<JsObject>`. The deliberate exception is
`js/typeof`, whose JavaScript-language result is always inferred as `String`:

- `JsNullish` is reserved for the actual JavaScript `null`/`undefined` boundary;
  it is deliberately distinct from legacy `Optional` and nominal `Option`.
- `JsObject` is an opaque host value. A `js-present?`/`js-nullish?` check proves only that the
  value is present; it does not prove that the payload is a Calcit `String`,
  `Number`, struct, or collection.
- Before passing the value into strongly typed Calcit code, validate/convert it
  with a boundary decoder. `unsafe-coerce` is available only when an external
  API contract is trusted and the unchecked conversion is intentional.

Plain `.-name` and `.!name` dereference their receiver. If the receiver is a
`JsNullish<JsObject>`, strict project source fails with
`E_JS_FFI_NULLABLE_DEREF`; compatibility mode reports
`W_JS_FFI_NULLABLE_DEREF`. Use `.?-name`/`.?!name`, or narrow the receiver
before dereferencing it.

Functions containing raw interop should declare `:features $ #{} :js-ffi` in
their schema. The feature identifies the boundary but does not suppress
nullable dereference or strong-type mismatch diagnostics.

Use `js-nullish?` and `js-present?` to narrow a JavaScript boundary. Applying
legacy `nil?`/`some?` fails strict project source with
`E_JS_FFI_NULLABLE_PREDICATE`; compatibility mode reports
`W_JS_FFI_NULLABLE_PREDICATE`. Convert explicitly with
`js-nullish->option` only after accepting the opaque payload contract; generic
`optionally` does not accept `JsNullish<T>`.

## 2. Capability and target policy

### 2.1 Capability policy

The active entry controls how an unmarked host operation is handled. Existing
projects default to `:allow` for migration; use `:warn` to inventory call sites
or `:error` to reject them during preprocessing:

```cirru.no-check
:feature-policy $ {}
  :js-ffi :error
```

Under the default strict diagnostics, when the selected entry omits `:js-ffi`, the
effective in-memory default is `:error`; running the check never rewrites the
Snapshot. An explicitly configured `:allow` or `:warn` remains available while
migrating an older entry. Inspect and update the selected entry without hand
editing the Snapshot:

```bash
calcit config show
calcit config set feature-policy.js-ffi error
calcit config set --entry browser feature-policy.js-ffi warn
```

The gate applies to `js/...`, JavaScript syntax such as `new` and `js-await`,
native `.-`/`.!` access, typed external-object access, and host
`unsafe-coerce`. It is lexical: a normal function may call a typed FFI wrapper
without declaring `:js-ffi`; only the wrapper's own implementation body needs
the feature. An anonymous function uses the feature declared in its own
`hint-fn` schema.

默认严格诊断对 `unsafe-coerce` 增加了跨后端约束：当前函数的结构化 schema 必须声明 `:js-ffi`，
否则预处理报 `E_UNSCOPED_UNSAFE_COERCE`。类似 adapter 的命名空间名称只能作为盘点线索，不能代替能力声明。
经过作用域检查的转换仍应接受审阅，但严格编译不会按其出现次数运行独立质量预算。

`calcit edit ffi` 修改的是 CodeEntry 的导出/宿主元数据，**不会**给函数体授予词法 `:js-ffi` 权限。
已有完整 `Fn` schema 时，可只增加这一项 feature，不必重写参数、返回类型、泛型及其他 feature：

```bash
calcit calcit.cirru edit schema 'app.js_adapter/read-day' --add-feature js-ffi
```

若 schema 仍为 `Dynamic` 或其他非 `Fn` 类型，此命令会拒绝修改；先声明准确的 `Fn` 签名，
并把宿主值检查/转换局限在小型 adapter 中，不要为通过检查而扩大业务层的 `Dynamic`。
需要预览、过期 revision 拒绝和原子提交时，将同一操作放入现有 `edit transaction`：

```bash
calcit calcit.cirru edit transaction \
  --code '[] $ [] |edit |schema |app.js_adapter/read-day |--add-feature |js-ffi' \
  --dry-run --format edn
```

确认预览后，带上结果里的 `original-revision` 作为 `--expect-revision` 再执行同一事务；
重复添加同一 feature 不会再次改写 Snapshot。对 dayjs 等第三方对象，优先用 typed adapter 或
external-object trait 暴露稳定接口，并在 Calcit 测试中覆盖有效和无效宿主值。

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

`.-name`, `.!name`, `.?-name`, `.?!name`, `aget`, `aset`, `js-get`, and `js-set` against a bare
`JsObject` receiver have no field or method contract. When the member key is a
literal, strict project source rejects the access with
`E_UNTYPED_JS_OBJECT_ACCESS`; declare the smallest external-object trait and
coerce the host value to that trait inside the lexical adapter. Compatibility
mode can inventory the same sites as `W_JS_FFI_UNTYPED_ACCESS` with
`--warn-dyn-method`. A dynamic (non-literal) key retains explicit raw lookup
semantics, while receivers that already carry trait or nullable evidence are
handled by their dedicated checks.

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

Audit these exceptional assertions without executing the program:

```bash
calcit analyze weak-types --only unsafe-coerce
calcit analyze weak-types --ffi-evidence --format edn
```

The report gives each assertion's `code@...` Snapshot path and declared target
schema. JSON also records only static boundary evidence: the source form,
whether the definition declares `:js-ffi`, and whether its namespace follows
the `js-ffi.raw.*` adapter convention. Treat the result as a runtime-contract
checklist: keep the assertion in one adapter, test accepted host values and
rejected shapes, then return a normal Calcit `Option`, `Result`, struct, or enum
to application code.

The opt-in FFI evidence view groups raw operations by definition and classifies
browser, Node, npm-import, WebGPU, and unknown-host boundaries. It reports
Snapshot paths, nullable and unsafe evidence, statically visible callers, and
review-only exact-schema helper, trait, and adapter candidates. Installed
dependency helpers are listed before project-local matches. This is migration
evidence, not a second type checker: it never infers runtime trust, chooses
`Option`/`Result` semantics, narrows `Dynamic`, or writes source. Cirru EDN is
the canonical machine-readable output; select JSON only for interoperability.

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
calcit query def namespace/StorageHost --json
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

`calcit query def namespace/name --json` includes the normalized `ffi` metadata.
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

以下是旧式裸宿主对象访问的语义示意；严格类型项目应先声明 external-object trait，并在 `:js-ffi` 适配层转换对象。

```cirru.no-check
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

### 3.3 方法调用体验

external-object trait 是让宿主对象"接近直接调用 JS 方法"的推荐方式。只声明适配器真正会调用的字段与
方法，用 `:names` 映射非标识符宿主名，并且只把确实可写的字段放进 `:writable`。

用 `defexternal` 简写可以在一处同时声明成员与 FFI 元数据：

```cirru.no-check
defexternal QueryHost
  :target :browser
  :names $ {} (:query |querySelector)
  :writable $ #{} :text-content
  :length 'Number
  .query $ :: 'Fn
    {}
      :args $ [] 'QueryHost 'String
      :return 'QueryHost
  .text $ :: 'Fn
    {}
      :args $ [] 'QueryHost
      :return 'String
```

`defexternal` 在载入期展开为与下面等价的 `deftrait` 代码 + `CodeEntry :ffi` 元数据；
`:backend :js` 与 `:kind :external-object` 由简写固定，不需要手写：

```cirru.no-check
deftrait QueryHost
  :length 'Number
  .query $ :: 'Fn
    {}
      :args $ [] 'QueryHost 'String
      :return 'QueryHost
  .text $ :: 'Fn
    {}
      :args $ [] 'QueryHost
      :return 'String

:ffi $ {}
  :backend :js
  :kind :external-object
  :target :browser
  :names $ {} (:query |querySelector)
  :writable $ #{} :text-content
```

两种写法在类型系统、codegen 与 `calcit query def --json` 的归一化 `ffi` 输出上完全一致；不能同时使用
`defexternal` 与显式 `:ffi`，否则报错。字段默认只读，只有列进 `:writable` 的字段可写。

用 `calcit edit def` 写入时，`defexternal` 会在写盘时归一化 `:schema :: 'Trait`，非法简写（例如未知
`:target`）会在写入前报错，而不是留到下次载入：

```bash
calcit calcit.cirru edit def app.browser/QueryHost --file snippet.cirru --input-format cirru
```

在适配器边界把宿主值 `unsafe-coerce` 一次，之后用普通 Calcit 方法调用：

```cirru.no-check
defn read-title (element)
  let
      host $ unsafe-coerce element 'QueryHost
      node $ host .query |.app
    node .text
```

要点：

- 方法调用写成 `(receiver .method args...)`；`.method` 走 external-object trait 的静态派发，
  `.!method` 只用于裸 `JsObject` 上的原生调用，两者语义不同。
- 返回另一个宿主对象的方法要在 trait 上声明返回类型（例如 `:return 'QueryHost`），这样就能继续链式调用。
  Calcit 有意不引入 `Promise<T>`/`Iterator<T>` 这类泛型宿主 trait：请在适配器内部消费这类泛型宿主值，
  只把普通 `Option`/`Result`/`Struct`/`Enum` 数据返回给业务代码。
- 构造器与静态方法使用 `new js/Name` / `js/Name.method`；结果仍是不透明宿主值，需要同一适配器内
  coercion 到 external trait。
- 可空宿主值保持 `JsNullish<T>`，直到用 `js-present?`/`js-nullish?` 收窄，或改用可选访问 `.?-`/`.?!`。
- 适配器函数要在结构化 schema 中声明 `:features $ #{} :js-ffi`。

对照关系：

| JavaScript | Calcit typed adapter |
| --- | --- |
| `el.focus()` | `el .focus!` |
| `el.querySelector(".a").textContent` | `(el .query ".a") .text`（逐层 `let` 或嵌套） |
| `new Date()` | `new js/Date` 后 coercion 到 external trait |
| `el?.textContent ?? ""` | 先收窄 `JsNullish`，再用 `js-nullish->option` |
| 裸 `JsObject` 上的 `el.foo` | 报 `E_UNTYPED_JS_OBJECT_ACCESS`，应改为声明 external trait |

盘点与迁移入口：

```bash
calcit analyze weak-types --ffi-evidence --format edn
calcit --warn-dyn-method calcit.cirru --check-only
```

`--ffi-evidence` 按 definition 分组列出裸宿主操作并给出 trait/adapter 候选；`--warn-dyn-method`
用稳定的 `W_JS_FFI_UNTYPED_ACCESS` 盘点静态字面量 key 的裸访问。每个 trait candidate 还会附带一段
可直接粘贴的 `defexternal` 骨架，例如：

```cirru.no-check
defexternal QueryHost
  :target :browser
  :length 'Dynamic
  .query $ :: 'Fn $ {} (:args $ [] 'QueryHost) (:return 'Dynamic)
```

骨架只包含能表达为 Calcit trait 成员的字段/方法名（索引、字符串 key 会被过滤）；字段与方法类型仍是
`Dynamic` 占位，`contract_status` 保持 `review-required`，必须人工补全类型后再用于严格质量门禁。

确认字段/方法集合与类型后，用 `defexternal` 建立最小契约，再做一次边界 coercion。

## 4. Syntax reference

### 4.1 Access global values

Use `js/...` to read JavaScript globals and nested members:

```cirru.no-run
do js/window.innerWidth
```

### 4.2 Access properties

旧式 `.-name` 表示宿主属性读取；严格类型代码应给对象声明 external-object trait，而不是直接把 `JsObject` 传给静态字段访问：

```cirru.no-check
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

When a function also declares a checked function schema, `:async true` is part
of its invocation contract. The schema's `:return T` describes the value after
awaiting, not the Promise-like value returned by the JavaScript call:

```cirru.no-check
hint-fn $ {}
  :args $ []
  :return String
  :async true
```

Calling this function produces a pending async value in static checking. Pass
the call through `js-await` before using it as `T`; otherwise checking fails
with `E_ASYNC_INVOCATION_REQUIRES_AWAIT`. Function aliases and callback schemas
retain this contract, so async and synchronous callbacks are not interchangeable.
The checker does not insert `await`, retry calls, or model effects automatically.

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
6. Does `calcit query def namespace/name --json` show the expected schema and FFI
   metadata?
7. Does an adapter body avoid a redundant top-level `do`? `defn`/`fn`/`let`/`let[]`
   bodies already evaluate their expressions in order and return the last one, so a
   wrapping `do` only adds nesting.

### 6.1 Remove redundant top-level `do`

Keep one expression per line inside a variadic body instead of wrapping the body in `do`:

```cirru.no-check
defn read-name (host)
  do
    let
        raw $ .-name host
      unsafe-coerce raw 'String
```

The equivalent without the redundant `do` is:

```cirru.no-check
defn read-name (host)
  let
      raw $ .-name host
    unsafe-coerce raw 'String
```

`calcit fix` preview includes `redundant-do-v1` by default and never writes the
Snapshot unless you pass `--apply`, so it works as a lint. Review the plan, then
apply with the reported revision:

```bash
calcit calcit.cirru fix --rule redundant-do-v1 --format edn
calcit calcit.cirru fix --rule redundant-do-v1 --apply --expect-revision 'md5:<preview revision>'
```

Each suggestion reports the stable `diagnostic_code` `FIX_REDUNDANT_DO`. CI can parse
the EDN report and fail when the suggestion list is non-empty. The rule does not touch
`if`/`case` branches, call arguments, binding values, `defmacro` bodies, or
`quote`/`quasiquote` data.

Useful checks for a project with separate entries are:

```text
calcit --entry browser calcit.cirru --check-only
calcit --entry node calcit.cirru --check-only
calcit --entry browser calcit.cirru js
calcit --entry node calcit.cirru js
```

Generate the two targets serially when they share one `js-out/` directory. A
parallel browser/Node codegen run can overwrite generated modules while the
other target is still writing them, which makes runtime smoke tests unreliable.

Common diagnostics:

| Code | Meaning | Typical fix |
| --- | --- | --- |
| `E_JS_FFI_FEATURE_REQUIRED` | A host operation is outside a marked adapter body. | Add `:js-ffi` to that implementation schema or move the operation into a wrapper. |
| `E_UNSCOPED_UNSAFE_COERCE` | Strict preprocessing finds `unsafe-coerce` outside the current function's lexical FFI scope. | Move it into a small structured `Fn` adapter declaring `:js-ffi`; validate/convert there and return typed data. |
| `E_JS_FFI_TARGET_MISMATCH` | The selected entry targets another host. | Correct the entry `:target` or use the matching adapter. |
| `W_JS_FFI_NULLABLE_DEREF` | A nullable host value is dereferenced directly. | Use optional access or narrow with `js-present?`. |
| `W_JS_FFI_NULLABLE_PREDICATE` | Compatibility source applies legacy `nil?`/`some?` to a host-nullish value. | Use `js-nullish?`/`js-present?` so host semantics remain visible. |
| `E_JS_FFI_NULLABLE_DEREF` | Strict project source dereferences `JsNullish<JsObject>` directly. | Use optional access or narrow with a dedicated JS predicate. |
| `E_JS_FFI_NULLABLE_PREDICATE` | Strict project source applies legacy `nil?`/`some?` to `JsNullish<T>`. | Use `js-nullish?`/`js-present?`, then convert explicitly if needed. |
| `E_JS_FFI_FIELD_READONLY` | A typed external field is written without permission. | Add the field to `:ffi :writable` only if the host API permits it. |

## 模块内 JS 实现（0.22 预览）

定义级 `:ffi :js` 可以指定一个 JS 函数表达式，或指定当前 Calcit 模块根目录下包含单个表达式的 `.js`/`.mjs` 文件。构建时把表达式嵌入所属 Calcit namespace 的生成文件，不复制成独立的 JS 模块；调用者只需加载 Calcit 模块并按普通 namespace 引用定义。两个形式都要求完整的 `Fn` schema 和显式 `:js-ffi` feature。schema 是作者承诺的外部边界，编译器不解析 JS 来证明返回值。

```cirru.no-check
:schema $ :: 'Fn $ {} (:args ([] 'Number)) (:return 'Number)
  :features $ #{} :js-ffi
:ffi $ {} (:target :node)
  :js $ {} $ :inline "|(x) => x + 1"
```

文件形式只声明一个源码文件。文件内容须是求值为函数的单个表达式，不能写 `import`/`export`，也不能用动态 `import()` 或 `require()` 引用其他 snippet。原始源码不经过 Cirru 格式化。共享代码应提升为普通 Calcit 定义，或显式声明外部模块；不同定义即使引用同一个 JS 文件，也各自求值一次。

```cirru.no-check
:ffi $ {} (:target :node)
  :js $ {} $ :file |js-ffi-assets/add-two.js
```

需要 Node 内置模块或已安装的 npm ESM 包时，在 `:modules` 中以别名显式声明。编译器在生成的 Calcit namespace 顶层导入模块，再把别名作为词法变量交给表达式；不会把相对路径按原始 snippet 的目录解析，也不自动安装 npm 包。`node:` 模块只能用于 `:node` target；相对、绝对和 URL specifier 均拒绝。

```cirru.no-check
:ffi $ {} (:target :node)
  :js $ {} (:file |js-ffi-assets/base-name.js)
    :modules $ {} $ :path |node:path
```

对应文件内容为 `(value) => path.basename(value)`。生成的 JS 会保留 `app.main/base-name` 注释作为定位锚点；Node 的语法检查和运行时堆栈仍指向生成文件及其行号，后续可再提供精确 source map。当前仅保守地筛查 `import`、`export`、`require` 词元（连字符串中的同名词元也可能被拒绝），不承诺完整 JavaScript 解析或静态验证函数返回值。

第一阶段仅接受 Number、String、Bool、Unit、JsObject 与相应 JsNullish 边界；不把 Calcit 集合或 nominal 值隐式当作 JS 容器。泛型、rest 参数和 async 签名暂不开放。native 调用会明确报错。JS 文件必须位于所属模块根目录内，绝对路径、`..` 和 symlink 越界会失败。实际用法与下游模块 smoke 见 `calcit/js-ffi-module/calcit.cirru`、`calcit/js-ffi-consumer.cirru` 和 `yarn check-js-ffi-source`。

跨 namespace 使用时沿用普通 Calcit `:require`。示例的 `app.api` 先从 `app.main` 引用 JS FFI 定义并包装为 `plus-four`、`file-label`、`next-count`；下游 `test-nil.main` 同时直接引用 `app.main` 和引用 `app.api`。回归脚本把模块与消费者复制到独立源码目录构建，再搬移生成目录执行，确认两条 Calcit 引用路径共享同一个有状态定义，且没有 snippet 专用 import、路径补丁或软链接。这仍不是已发布模块的干净安装验收；该步骤由 #1360 跟进。

运行 `calcit <snapshot> js -w` 时，已声明的 `:file` JS 源码会进入现有 watcher：直接保存或原子替换文件都会重新生成 JS，无需触碰 Calcit Snapshot 或 `.compact-inc.cirru`。普通单次构建与只读查询不启用这些文件 watcher。`yarn check-js-ffi-source` 覆盖两种保存方式及跨 namespace wrapper 的更新行为。

Agent 排查时先用现有查询入口，不需要猜依赖模块或 JS 文件的路径：

```bash
calcit calcit/js-ffi-consumer.cirru query context app.main/base-name --format edn
calcit calcit/js-ffi-consumer.cirru query def app.main/base-name
calcit calcit/js-ffi-consumer.cirru query def app.main/plus-one --raw
```

`query context` 的 `:js-ffi` 给出 target、所属模块、模块根目录、`inline`/`file` 来源、模块相对文件路径和显式外部模块；`:next` 指向完整 `query def`。后者在 Markdown 输出中将元数据与 inline JavaScript 的 `javascript` 代码块分开，`--format edn` 保留原生 FFI 数据，`--format json` 只在 JSON 工具链需要时显式选用。查询只读取元数据，不执行 JS。`Fn` schema 是作者声明，不代表编译器已验证 JS 的参数和返回值；仍需外部语法检查与真实宿主运行。文件实现直接编辑 `module root + source file`，不要修改生成的 `.mjs`。这一步只提供定义级定位；精确 JS 源行映射由 #1362 完成。
