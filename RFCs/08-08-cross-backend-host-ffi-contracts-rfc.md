# RFC：跨后端宿主/FFI 契约与稳定宿主能力

状态：草案
日期：2026-08-08

## 1. 摘要

Calcit 的 FFI 类型声明应继续使用 Snapshot 已有的 schema 体系，而不是再发明一层包住函数 schema 的 `Ffi` 类型。

本 RFC 将 FFI 声明拆成两个正交部分：

1. `CodeEntry :schema` 描述 Calcit 看见的类型。函数仍使用 `:: 'Fn`，trait、impl、struct 等定义仍使用 `:: 'Trait`、`:: 'Impl`、`:: 'Struct`。
2. `CodeEntry :ffi` 描述后端如何实现这个定义，例如 JavaScript 宿主路径、属性读写、native method call、原生注册符号或 WASM module/field。它是 lowering 元数据，不参与普通类型匹配。

JavaScript 宿主对象不建立一套类似 TypeScript 的结构化类型系统。稳定成员以 Calcit trait 描述：tag member 声明属性类型，method member 声明调用签名；JS codegen 根据 receiver 的 external trait 类型，把现有 tag access 降为属性读取，把现有 method invoke 降为保留 receiver 的 JavaScript 方法调用。

这一模型只要求契约足以指导生成正确的目标代码，不追求精确复制 JavaScript 的原型、重载、可选属性、联合类型和完整 DOM 层级。

## 2. 修订原则

本 RFC 明确放弃以下设计：

- 不引入 `(:: 'Ffi {...})` schema wrapper；
- 不在 `:schema` payload 中保存 `:backend`、`:implementation`、`:transport` 或成员表；
- 不把普通 EDN map 伪装成新的类型表达式；
- 不为宿主字段另建复杂的 shape/type/member AST；
- 不把 JavaScript `this`、原型继承、getter 效应或 TypeScript 重载完整搬入 Calcit；
- 不让 `JsObject` 因属性名称相同而自动获得更强类型。

修订后的设计遵循三个已有事实：

- Snapshot 的 `:schema` 是 Cirru EDN 类型数据，不是源码 AST，也不需要额外 `quote`；
- 函数 schema 的规范形式是 `:: 'Fn`，类型名使用 quoted symbol，例如 `'String`、`'Number`、`'JsObject`；
- Calcit 已有 trait、trait method schema、泛型与 `:where`，宿主能力应尽量复用这些机制。

## 3. 目标与非目标

### 3.1 目标

1. 让 JS、原生 registered proc、WASM import/export 共享现有函数 schema 检查。
2. 用现有 trait 体系描述少量稳定宿主操作。
3. 让 codegen 能从声明确定应生成属性读取、属性写入还是宿主方法调用。
4. 保留 `JsObject` 与 `JsNullish<T>` 的不透明边界语义。
5. 让 Snapshot 中的声明可由 `cr query`、生成器和静态分析直接读取。
6. 保持声明足够小，使绑定模块只描述应用实际使用的 API。

### 3.2 非目标

- 导入或完整解释 `.d.ts`；
- TypeScript 结构化类型、联合/交叉类型、条件类型和重载解析；
- 完整 DOM 继承树；
- JavaScript 原型变异、Proxy 或动态属性创建；
- 跨后端统一 ABI、内存布局或所有权模型；
- 让宿主对象自动成为 Calcit `Struct`、`Map` 或可序列化数据；
- 在本 RFC 中公开仓库内部的 WASM 验证后端。

## 4. Snapshot 中的规范类型写法

### 4.1 函数定义

函数的 `:schema` 继续直接保存 `Fn` schema：

```cirru.edn
{}
  |read-text $ %{} 'CodeEntry (:doc "|Read text through a host binding")
    :code $ quote &runtime-implementation
    :examples $ []
    :schema $ :: 'Fn
      {}
        :args $ [] 'String
        :return 'String
        :features $ #{} :js-ffi
    :tags $ #{} :ffi
```

这里没有 `:callable`，也没有 `Ffi` wrapper。`CodeEntry.schema` 解析后仍然是现有的 `CalcitTypeAnnotation::Fn`。

### 4.2 trait、impl 与数据定义

定义种类沿用当前 Snapshot 表示：

```cirru.edn
:: 'Trait
```

```cirru.edn
:: 'Impl
```

```cirru.edn
:: 'Struct
```

宿主能力声明使用 `Trait`。它不是普通 Calcit 数据结构，也不需要新增 `Shape` schema kind。

### 4.3 类型引用

类型位置统一使用 quoted symbol 和现有 enum-tuple 形式。下面用一个 EDN list 并列展示这些形式：

```cirru.edn
[] 'String 'respo.dom/DomElement
  :: 'JsNullish 'respo.dom/DomElement
  :: 'List 'String
  :: 'Fn $ {}
    :args $ [] 'String
    :return 'Bool
```

`:string`、`:fn`、`:js-object` 等旧 tag 形式可以作为兼容输入，但新文档和生成器应输出规范的 quoted-symbol 形式。

## 5. `CodeEntry :ffi` lowering 元数据

### 5.1 字段边界

本 RFC 建议给 `CodeEntry` 增加可选的原始 EDN 字段：

```rust
pub struct CodeEntry {
  pub doc: String,
  pub examples: Vec<Cirru>,
  pub tags: HashSet<EdnTag>,
  pub code: Cirru,
  pub schema: Arc<CalcitTypeAnnotation>,
  pub ffi: Option<Edn>,
}
```

`:ffi` 必须在 Snapshot load/save、definition revision、diff 和 query JSON 中原样保留。解析器可以按 `:backend` 建立缓存，但 EDN 是持久化事实来源。

`:ffi` 不改变 `:schema` 的含义，也不复制函数参数和返回类型。若后端需要参数个数，应从 `Fn` schema 推导。

### 5.2 JavaScript 可调用项

```cirru.edn
{}
  |query-selector $ %{} 'CodeEntry (:doc "|Typed binding for document.querySelector")
    :code $ quote &runtime-implementation
    :examples $ []
    :schema $ :: 'Fn
      {}
        :args $ [] 'String
        :return $ :: 'JsNullish 'respo.dom/DomElement
        :features $ #{} :js-ffi
    :tags $ #{} :ffi
    :ffi $ {}
      :backend :js
      :kind :import
      :target $ [] |document |querySelector
      :invoke :method
```

`target` 是字符串路径。`:invoke :method` 表示最后一段必须保留接收者调用语义，生成：

```js
document.querySelector(selector)
```

而不是先取出 `document.querySelector` 再作为无接收者函数调用。

MVP 只需要三种 JavaScript import 调用方式：

| `:invoke` | 生成语义 |
| --- | --- |
| `:function` | 调用路径指向的函数 |
| `:method` | 以前缀路径为 receiver 调用最后一段 |
| `:value` | 只读取路径指向的值 |

构造器可在实际用例出现时增加 `:constructor`，不必预先设计完整 JavaScript invocation model。

### 5.3 原生与 WASM 可调用项

原生 registered proc 使用相同的函数 schema，只把符号信息放在 `:ffi`：

```cirru.edn
{}
  |read-file $ %{} 'CodeEntry (:doc "|Registered native procedure")
    :code $ quote &runtime-implementation
    :examples $ []
    :schema $ :: 'Fn
      {}
        :args $ [] 'String
        :return $ :: 'Result 'String 'String
    :tags $ #{} :ffi
    :ffi $ {}
      :backend :native
      :kind :registered-proc
      :symbol |read-file
```

WASM import/export 仍由现有专用语法声明；预处理后可暴露等价的 binding 元数据：

```cirru.edn
{}
  :ffi $ {}
    :backend :wasm
    :kind :import
    :module |host
    :symbol |string-upcase
    :transport :string-handle
```

逻辑签名仍从该 definition 的 `:: 'Fn` schema 读取。`:transport` 只供 WASM adapter 检查和 lowering，不参与普通 Calcit 类型匹配。

## 6. 用 trait 描述 external object

### 6.1 tag member 与 method member

`deftrait` 已经接受 tag 或 method 作为成员键。本 RFC 沿用这个语法差异表达两类 external member：

- `:field Type`：字段能力，通过 tag access 读取；
- `.method FnSchema`：方法能力，通过 method invoke 调用。

这只需要在 trait 的内部 member descriptor 中保留来源种类。当前 `CalcitTrait` 会把 tag 与 method 都收敛成 `EdnTag`；实现 external object 前，应将其扩展成类似 `{name, kind, type}` 的描述，而不是再维护一份平行的 field/method schema。

```cirru.edn
{}
  |DomElement $ %{} 'CodeEntry (:doc "|Small stable capability set for DOM elements")
    :code $ quote
      deftrait DomElement
        :id 'String
        :text-content $ :: 'JsNullish 'String
        .matches? $ :: 'Fn
          {}
            :generics $ [] 'T
            :args $ [] 'T 'String
            :return 'Bool
        .query-selector $ :: 'Fn
          {}
            :generics $ [] 'T
            :args $ [] 'T 'String
            :return $ :: 'JsNullish 'respo.dom/DomElement
    :examples $ []
    :schema $ :: 'Trait
    :tags $ #{} :ffi :js-host
    :ffi $ {}
      :backend :js
      :kind :external-object
      :names $ {}
        :text-content |textContent
        :query-selector |querySelector
```

每个 method schema 仍把 receiver 放在 `:args` 的第一个位置，这与 Calcit 当前 trait method 检查一致。`'T` 只用于让 receiver 类型参与已有泛型绑定。

`:names` 是可选的名称覆盖。没有覆盖时，字段或方法名直接作为宿主成员名；codegen 根据名称是否为合法 JavaScript identifier 选择 dot access 或 bracket access。它不重复保存成员种类和类型。

### 6.2 统一的访问语法

对于静态类型为 `DomElement` 的 `element`：

| Calcit 源码 | 预处理语义 | JavaScript lowering |
| --- | --- | --- |
| `element :id` | typed tag access | `element.id` |
| `element :text-content` | typed tag access | `element.textContent` |
| `element .matches? selector` | typed method invoke | `element.matches(selector)` |
| `element .query-selector selector` | typed method invoke | `element.querySelector(selector)` |

tag access 的显式内部形式是 `.:id element`。普通源码优先使用与 Struct 一致的 postfix `element :id`；预处理器根据 receiver 类型决定它是 Struct 字段读取还是 external property read。

method invoke 同样复用现有 postfix/prefix 规则。普通 Calcit trait 继续生成 `invoke_method`；只有 receiver 被静态证明为 `:kind :external-object` 时，JS codegen 才直接生成 receiver method call。

属性写入不应复用 `assoc`，因为 Calcit `assoc` 表示持久化更新，而 JavaScript assignment 是宿主变异。MVP 可以继续使用显式 FFI setter；后续若增加 typed `set!`，必须通过单独的 `:writable` 声明限制可写字段。

### 6.3 trait 约束

普通函数通过现有 `:where` 使用 external object 能力：

```cirru.edn
{}
  |read-element-id $ %{} 'CodeEntry (:doc "|Read id from any value with DomElement capability")
    :code $ quote
      defn read-element-id (element)
        element :id
    :examples $ []
    :schema $ :: 'Fn
      {}
        :generics $ [] 'T
        :args $ [] 'T
        :return 'String
        :where $ {}
          'T 'respo.dom/DomElement
```

这里完全沿用现有泛型与 trait bound 语义。预处理器根据 `DomElement` 中的 `:id 'String` 推断结果为 `String`，JS codegen 再根据该 trait 的 `:ffi` 标记选择 property lowering。

External trait 不通过运行时 `impl-traits` 附加到 JavaScript 对象。它由可信 binding 的返回 schema、受检转换或显式 unsafe 边界提供静态证据。若没有这些证据，原始值仍是 `JsObject`。

### 6.4 小型浏览器契约

绑定模块应定义少量面向用途的 trait，不复制 DOM 继承层级。例如输入控件可以单独定义：

```cirru.no-check
deftrait DomInput
  :value 'String
  :checked 'Bool
  :disabled 'Bool
  .focus! $ :: 'Fn
    {}
      :generics $ [] 'T
      :args $ [] 'T
      :return 'Unit
```

其 `:ffi` metadata 只需声明后端、external object 身份和必要的名称覆盖：

```cirru.edn
{}
  :ffi $ {}
    :backend :js
    :kind :external-object
    :names $ {}
      :focus! |focus
```

`DomInput` 不必声明 `HTMLInputElement` 的所有父接口。需要 `form`、selection API 或 typed property write 时再按真实使用补充。

## 7. 不透明性、可空性与信任

### 7.1 原始值保持不透明

现有规则保持不变：

- 原始 `js/...`、`aget`、`.-field` 和 `.!method` 的未知结果仍推断为 `JsNullish<JsObject>`；
- `js-present?` 只消除外层 `JsNullish`，结果仍是 `JsObject`；
- 属性名称相同不能证明某个宿主 trait；
- `JsObject` 不能自动匹配 Calcit `Struct`、`Map`、`String` 或宿主 trait。

### 7.2 更强契约的来源

宿主值只能通过以下路径获得 trait 契约：

1. 带 `:ffi` 的可信 binding 在 `Fn` schema 中声明返回该 trait；
2. 受检 decoder 验证必要条件后返回该 trait 或普通 Calcit 数据；
3. 后端注册描述符提供匹配的宿主类型标识；
4. 显式 unsafe/FFI 断言，并记录目标 trait 与源码位置。

普通函数不能只靠 `assert-type` 把任意 `JsObject` 变成 `DomElement`。`assert-type` 只检查，不改变运行值，也不构成宿主契约证据。

### 7.3 可空性

JavaScript 的 `null`/`undefined` 继续使用 `JsNullish<T>`。宿主 trait 不自行携带 optional/nullable 标记：

```cirru.edn
:: 'JsNullish 'respo.dom/DomElement
```

在调用 `.id`、`.focus!` 等 trait method 前，接收者必须已经通过 `js-present?` 等路径收窄。存在性检查只证明值存在，不重新验证成员。

业务缺失继续使用 `Option<T>`，可恢复失败继续使用 `Result<T,E>`，无业务返回值使用 `Unit`。这些类型不能与 `JsNullish<T>` 静默互换。

## 8. JavaScript codegen 规则

当 trait definition 带有 `:ffi {:backend :js, :kind :host-trait}` 时，JS codegen 对该 trait 的静态调用执行专用 lowering：

```text
(.id element)
=> element.id

(.set-id! element next-id)
=> element.id = nextId

(.matches? element selector)
=> element.matches(selector)
```

该 lowering 必须在 trait 已静态解析且成员映射唯一时发生。以下情况应拒绝生成：

- trait method 没有对应成员映射；
- 成员映射与 method arity 不一致；
- `:get` 带有 receiver 之外的参数；
- `:set` 不是 receiver 加一个值；
- 当前后端与 `:ffi :backend` 不一致；
- receiver 仍为 `JsNullish<T>`；
- 同一个 method 在多个 trait 中产生无法消解的宿主 lowering。

Native codegen 不应尝试解释 JS host trait。反过来，JS codegen 也不应把 native handle trait 当作 JavaScript 属性访问。

## 9. 跨后端共享边界

共享层只负责 Calcit 逻辑契约：

- `Fn` 参数、返回值、rest 参数与泛型；
- `:where` trait bound；
- 回调的嵌套 `Fn` schema；
- `JsNullish`、`Option`、`Result` 与 `Unit` 的逻辑区别；
- definition kind 与 `:ffi :kind` 的基本一致性；
- `:features` 能力要求。

后端 adapter 负责：

- 宿主符号和路径解析；
- JavaScript receiver/method 调用；
- native 注册表和句柄表示；
- WASM ABI、线性内存和 codec；
- 异常、trap、panic 与异步机制；
- 所有权和生命周期。

这些后端事实不进入普通 `CalcitTypeAnnotation::matches_with_bindings`。

## 10. 诊断

MVP 建议使用少量稳定诊断：

- `E_FFI_METADATA`：`:ffi` 缺字段、字段类型错误或 kind 不一致；
- `E_FFI_BACKEND_MISMATCH`：binding/host trait 不适用于当前后端；
- `E_FFI_MEMBER_UNKNOWN`：host trait method 没有 lowering 映射；
- `E_FFI_MEMBER_ARITY`：trait method schema 与 `:get`/`:set`/`:call` 不兼容；
- `E_FFI_OPAQUE_VALUE`：原始 `JsObject` 被当作更强宿主 trait；
- `E_FFI_NULLABLE_DEREF`：`JsNullish<T>` 未收窄即调用宿主能力；
- `E_FFI_ABI_UNSUPPORTED`：逻辑类型无法由目标 adapter 传输。

参数或返回值不匹配继续使用现有类型诊断，不需要再造一套 `HOST_TYPE_MISMATCH`。

## 11. 兼容性与迁移

1. 没有 `:ffi` 的现有定义行为不变。
2. `JsObject`、`JsNullish<JsObject>` 和 `:features #{:js-ffi}` 保持现有语义。
3. 现有手写 JS binding 可以继续使用普通 `defn`；只有需要生成或查询宿主绑定时才增加 `:ffi`。
4. `defwasm-import`、`defwasm-export` 与 registered proc 先只在 query 中暴露规范化 metadata，不立即改变运行时。
5. 旧的 `:string`、`:: :fn` 等 schema 输入仍可读取；重新保存和新生成内容使用 quoted-symbol 形式。
6. Snapshot loader 在正式写入 `:ffi` 前必须先做到未知字段无损保留，或明确实现 `CodeEntry.ffi`；不能让一次 `cr edit` 静默删除 binding 元数据。

## 12. 实施阶段

### 阶段 0：修正数据边界

- 为 `CodeEntry`、detailed snapshot、revision、diff 和 query 增加 `ffi: Option<Edn>`；
- 定义并测试 `:ffi` 的 Cirru EDN 解析与无损 round-trip；
- 明确 `:schema` 只保存现有类型，不接受 `Ffi` wrapper；
- 从现有 WASM/registered proc 信息生成只读的规范化 binding view。

### 阶段 1：可调用项

- 支持 JS `:kind :import` 的 `:function`、`:method`、`:value`；
- 从 definition 的 `Fn` schema 检查 arity、参数和返回值；
- 要求 JS binding schema 带 `:features #{:js-ffi}`；
- 在 `cr query context` 和机器 JSON 中暴露 `schema` 与 `ffi`，但不把二者合并。

### 阶段 2：JS host trait

- 识别 `:kind :host-trait`；
- 校验 `deftrait` methods 与 `:members` 一一对应；
- 实现 `:get`、`:set`、`:call` lowering；
- 复用 trait method 推断、泛型绑定和 `:where`；
- 增加原始 `JsObject`、nullable receiver 和多 trait 冲突测试。

### 阶段 3：其他 adapter

- registered proc 从同一个 `Fn` schema 推导 arity；
- WASM 可表示性检查读取逻辑 schema 与独立 transport metadata；
- 只有真实用例需要时才为 native handle 增加 host trait lowering；
- WASI 保持独立 adapter，不与通用 WASM 混为同一个 backend。

## 13. 验证策略

Snapshot/EDN 测试：

- `Fn`、`Trait` 与 named type reference 按规范形式 round-trip；
- `:ffi` 未知扩展字段无损保存；
- Cirru EDN 中 map、list、set、string、symbol 和 tag 不混淆；
- `cr edit schema` 只更新 `:schema`，不删除或改写 `:ffi`。

类型测试：

- trait method receiver 是 schema 的第一个参数；
- `:where` 能解析 host trait 并推断 method 返回类型；
- raw `JsObject` 不满足 host trait；
- `JsNullish<HostTrait>` 收窄前不能调用 method；
- setter 写入值类型错误时使用现有参数类型诊断。

JS codegen 测试：

- `:get` 生成 dot/bracket property read；
- `:set` 生成 assignment 并保持逻辑返回 `Unit`；
- `:call` 保留 receiver；
- `document.querySelector` binding 不丢失 `this`；
- host trait method 缺映射或后端不匹配时在发射前失败。

跨后端测试：

- native registered proc 与 schema arity 不一致时注册失败；
- WASM 不支持的逻辑类型在 adapter 阶段失败；
- JS 专属 host trait 不影响 native/WASM 普通类型匹配。

仓库级验证继续执行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo test`、`yarn compile`、`yarn check-all` 和 `yarn check-agent-interface`。只改 RFC 时至少应完成 Markdown/Cirru 示例解析和相关文档检查。

## 14. 决策与开放问题

本 RFC 建议先确定以下决策：

1. `:schema` 只使用现有 schema kind，FFI lowering 永远不包装函数类型。
2. JS 宿主成员用 trait method 表达，不增加结构化 host shape 类型系统。
3. property get/set 也通过逻辑方法暴露，以复用 trait method schema。
4. `:ffi` 是可选、可查询、无损保存的 EDN metadata。
5. MVP 只支持足以生成正确 JS 的 `:get`、`:set`、`:call` 与少量 import invocation。

仍需在实现前确认：

- `:ffi` 是否直接加入 `CodeEntry`，还是加入一个通用但同样无损的扩展元数据字段；
- host trait 是否允许普通 runtime impl，还是明确标记为 codegen-only；
- binding 返回 host trait 时，运行时是否需要轻量 host identity，还是首阶段只依赖可信 definition 边界；
- unsafe host trait 断言的公开名称与审计输出格式。

这些问题不影响 schema 边界：无论最终选择如何，函数类型仍是 `:: 'Fn`，宿主能力定义仍是 `:: 'Trait`，后端实现信息仍不属于类型表达式。

## 15. 相关文档

- `RFCs/07-08-ffi-features-and-js-object-type-rfc.md`
- `RFCs/02-17-register-platform-api-rfc.md`
- `RFCs/04-15-wasm-compilation-feasibility.md`
- `RFCs/04-16-wasm-data-structures.md`
- `RFCs/07-31-unsafe-coerce-driven-static-type-boundary-plan.md`
- `RFCs/08-05-systematic-nil-reduction-rfc.md`
- `docs/features/js-interop.md`
- `docs/features/polymorphism.md`
- `calcit/scripts/wasm-validation.md`
