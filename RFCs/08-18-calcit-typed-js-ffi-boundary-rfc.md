# RFC：以 Calcit 现有类型系统约束 JavaScript FFI 边界

状态：Draft

日期：2026-08-18

## 摘要

Calcit 现有的 `Struct`、`Enum`、trait、泛型函数 schema、`JsObject`、
`JsNullish<T>`、`:features #{:js-ffi}` 与 external-object trait 已经足以构成
JavaScript FFI 的主体模型。这里不建议引入一套 TypeScript 式的结构类型、联合类型、
重载和 DOM 继承树。

仍然需要进一步调整，重点不是增加更多 JS 专用类型，而是补齐以下能力：

1. 所有原始 JS 操作必须经过统一的 `:js-ffi` capability gate；当前实现只覆盖部分
   JS syntax proc，裸 `js/*` 解析路径尚未统一检查。
2. `:features` 作为实现体 capability 元数据应在解析、查询和检查阶段保留，但不进入
   普通函数类型匹配、泛型统一或 trait method candidate selection。
3. external-object trait 只在 definition 显式带有
   `:ffi {:backend :js :kind :external-object}` 时启用宿主 lowering；普通 trait 的匹配和
   推断保持不变。
4. 宿主字段写入可以使用可选的 writable lowering 元数据；默认 external field 只读，
   但这不向核心 trait 系统增加 mutability 语义。
5. FFI wrapper 应尽快把宿主值规范化为 Calcit `Struct`、`Enum`、`Option`、`Result`
   和 `Unit`，避免把 `Dynamic`、裸 Map 或 `JsObject` 扩散到业务代码。

本 RFC 是 Calcit 编译器的渐进改进方向，也同时规定 `js-ffi` 仓库应遵循的建模原则。
MVP 不增加 generic trait、associated type 或任何 FFI 专用的核心 type variant。

## 背景与现状

Calcit 当前已经具备：

- `JsObject`：不透明宿主值，不自动匹配 Calcit 数据类型；
- `JsNullish<T>`：只表示 JavaScript `null`/`undefined` 边界，与 Calcit `Option<T>`
  分离；
- `:features $ #{} :js-ffi`：声明函数体允许使用 JavaScript interop；
- external-object trait：用 tag member 描述宿主字段，用 method member 描述宿主方法；
- `CodeEntry :ffi`：保存 backend、host name 和 lowering 信息，不污染普通类型 schema；
- `:names`：将 Calcit member name 映射到实际 JavaScript property name；
- generic `Fn`、generic `Struct`/`Enum`、trait `:where` bound；
- `js-get`/`js-set` 对 external field 的类型推断与写入类型检查；
- `--warn-dyn-method`：定位裸 `JsObject` 上未声明契约的字段或方法访问。

这些能力说明总体方向已经正确：普通数据使用 Calcit nominal data type，宿主对象使用
capability trait，原始宿主值保持 opaque。

当前仍有几个具体缺口。

### 原始 JS 能力门禁不完整

预处理器对 `is_js_syntax_procs` 中的调用检查 `:js-ffi`，但 `js/foo` 会在 namespace
qualified symbol 分支直接变成 `RawCode(Js, ...)`。因此门禁并未覆盖全部 raw JS 路径。

同样需要统一纳入检查的还有：

- `js/*` global/property/call；
- `.-`、`.!`、`.?-`、`.?!`；
- `aget`、`aset`、`js-get`、`js-set`、`js-delete`；
- `js-object`、`js-array`、`new`；
- `js-await`、`js-for-await`；
- JS `set!`；
- external-object trait 最终 lowering 成的 property/method 操作；
- `unsafe-coerce` 从 `JsObject`/`JsNullish<_>` 到 external trait 的可信断言。

门禁应基于“预处理后的 host operation kind”，而不是维护若干互相遗漏的字符串列表。

### `:features` 的保存与检查路径不完整

`CalcitFnTypeAnnotation` 已保存 `features`，但当前函数 signature matching 不比较
features，函数类型的 `Ord`/`Hash` 也未包含 features，部分从运行时函数重建 schema 的
路径会回到空 feature set。

MVP 不把这个差异直接升级为函数类型兼容规则。需要解决的是：函数实现体在 capability
validation 时能够找到它声明的 feature，且 query/analyze 不误报。普通函数签名匹配、
trait method matching、泛型统一和缓存中的核心类型身份不应因 FFI feature 改变。

### 宿主泛型关系不在本 RFC 中扩展

当前 trait 可组合、可作为 `:where` bound，但 trait definition 自身没有类型参数或
associated type。以下常见关系无法稳定表达：

- `Promise<T>` 的 resolve value；
- `Event<Target>` 的 target；
- `Iterator<T>` / `AsyncIterator<T>` 的 item；
- `MapLike<K, V>` 的 key/value；
- typed array 的 element type；
- stream 的 chunk type。

本 RFC 选择不为这些关系扩展 trait。adapter 应在边界内消费 Promise、iterator、event
或 map-like object，再使用现有 `Fn`、`List<T>`、`Map<K, V>`、Struct、Enum、Option 和
Result 表达公共 API。只有 generic trait 本身获得足够多非 FFI 用例时，才应在独立的
Calcit RFC 中讨论；不能由 JavaScript FFI 单独驱动核心 trait 表示和推断复杂化。

### 字段可写性没有进入契约

external trait field 已能声明字段类型，`js-set` 也能检查赋值类型，但契约尚未区分
read-only 与 writable。DOM、Node 和 npm API 中大量属性只读；仅检查 value type 仍可能
生成无效写入。

## 设计原则

### Calcit-owned data 与 host object 分离

使用如下映射：

| 语义 | Calcit 表示 |
| --- | --- |
| 已转换、由 Calcit 拥有的数据 | `Struct` |
| 有限状态、有限分支、错误类别 | `Enum` |
| 宿主对象可提供的字段/方法能力 | external-object trait |
| 未建立契约的宿主值 | `JsObject` |
| JavaScript 可空宿主值 | `JsNullish<T>` |
| 业务缺失 | `Option<T>` |
| 可恢复错误或异常 | `Result<T, E>` |
| 只有副作用、无业务返回值 | `Unit` |

不得把 JavaScript object 直接伪装成 Calcit `Struct`。Struct 表示已经转换并满足 Calcit
数据不变量的值；external trait 才表示“仍由宿主拥有，但我们信任它具有这些能力”。

### 定义值不是 `Dynamic`

`defstruct`、`defenum`、`deftrait`、`defimpl` 产生的是可反射、可传递的定义值，不是
“任意未知值”。其 `CodeEntry :schema` 分别使用已有的 `StructDef`、`EnumDef`、`Trait`、
`Impl` 标记。旧快照中为兼容而遗留的根 `Dynamic` 在加载时规范化为对应标记；快照写回时
保持该标记。

这只描述定义值的种类，不把它们当作普通 Struct/Enum 实例，也不向 trait 匹配或泛型统一
增加 FFI 特例。字段、variant payload 和方法签名里的 `Dynamic` 仍是实际的动态债务，继续
参与 weak-types 与 quality gate；定义根本身不参与 Dynamic 计数。

### 不复制 TypeScript 类型系统

本 RFC 不引入：

- 通用 structural object type；
- union/intersection/conditional type；
- overload resolution；
- 完整 DOM interface inheritance；
- JavaScript prototype 或 Proxy 建模；
- 依赖任意字符串字面量的 dependent typing。

常见替代方式：

- union 用命名 Enum；
- overload 用多个明确 wrapper；
- 字符串常量集合用 Enum，并在 FFI adapter 中转换；
- 复杂宿主返回值由 decoder 转为 Struct/Enum；
- 只为项目实际使用的成员定义小型 capability trait。

### `:schema` 与 `:ffi` 元数据正交

`:schema` 只描述 Calcit 看见的类型。`:ffi` 只描述 backend lowering、host name、target
和 writable 等宿主事实。不要引入新的 `Ffi<T>` schema wrapper。

### 核心类型与推断的复杂度预算

本 RFC 的实现必须满足以下硬约束：

- 不新增 `JsPromise<T>`、`JsArray<T>`、`JsEvent<T>`、`JsIterator<T>` 等核心 type
  variant；
- 不修改 `CalcitTypeAnnotation::matches_with_bindings` 来解释 `:ffi` 或 `:features`；
- 不为 FFI 增加普通 trait 的 structural satisfaction、自动 impl 或特殊 where-bound；
- 不改变普通 trait 的 method candidate、requires、impl-traits 与 generic unification；
- external-object 分支只在 trait definition 显式具有
  `:ffi {:backend :js :kind :external-object}` 时启用；
- `:ffi` 始终是 lowering metadata，不参与 schema equality、subtyping 或 trait
  satisfaction；
- 未带 `:ffi` 的程序在推断候选、诊断和 codegen 上保持原行为。

external-object trait 复用 trait 的 member type 描述，但不自动获得普通 runtime impl，
也不能因为字段或方法同名而结构化满足另一个 trait。FFI 检查应是核心类型检查之后的独立
capability/target validation；它可以拒绝一个已经类型正确但未授权或 target 不兼容的
程序，却不能制造新的类型匹配结果。

## `js-ffi` 仓库的类型层次

建议将定义分为三层。

### 第一层：raw adapters

raw adapter 是唯一允许出现原始 JS syntax、`unsafe-coerce` 和 host import 的层。每个
函数必须有完整 Fn schema，并标记：

```cirru.no-check
:features $ #{} :js-ffi
```

raw adapter 可以返回 `JsObject`、`JsNullish<Trait>` 或 external trait，但不得使用
`Dynamic` 隐藏已知关系。

### 第二层：typed host capabilities

以小型 external trait 表示宿主对象。示意：

```cirru
deftrait StorageHost
  .get-item $ :: 'Fn
    {}
      :args $ [] 'js-ffi.browser/StorageHost 'String
      :return $ :: 'JsNullish 'String
  .set-item! $ :: 'Fn
    {}
      :args $ [] 'js-ffi.browser/StorageHost 'String 'String
      :return 'Unit
  .remove-item! $ :: 'Fn
    {}
      :args $ [] 'js-ffi.browser/StorageHost 'String
      :return 'Unit
```

trait 的 `CodeEntry :ffi` 声明 `:backend :js`、`:kind :external-object` 和必要的
`:names`。业务代码不直接接收裸 `JsObject`。

### 第三层：normalized Calcit API

公共 API 优先返回 Calcit-owned data：

```cirru
defenum Runtime (:browser) (:node)

defstruct JsError
  :name 'String
  :message 'String
  :stack $ :: 'Option 'String

defstruct BrowserProbe
  :runtime 'js-ffi.types/Runtime
  :document? 'Bool
  :storage $ :: 'Result 'String 'js-ffi.types/JsError

defstruct NodeProbe
  :runtime 'js-ffi.types/Runtime
  :cwd 'String
  :argv-count 'Number
```

当前 probe 使用 `Map<Tag, Dynamic>`，应迁移成命名 Struct。Storage mutation、console、
timer registration 等只有副作用的 wrapper 应显式以 `&unit` 收尾并声明返回 `Unit`，不因
JavaScript 返回 `undefined` 就暴露 `Dynamic`。

## 类型表达案例目录

本节把常见 JavaScript API 分成三种视图：

| 层次 | 目的 | 可以出现的类型 |
| --- | --- | --- |
| raw boundary | 如实表示宿主不确定性 | `JsObject`、`JsNullish<T>`、external trait |
| typed host API | 保留宿主 identity，同时提供成员类型 | external trait、`Option<Trait>`、typed `Fn` |
| normalized API | 供普通 Calcit 业务代码使用 | Struct、Enum、List、Map、Option、Result、Unit |

不是每个 API 都必须同时提供三层。默认优先提供 normalized API；只有调用者确实需要继续
操作宿主对象时，才公开 typed host API。

### 案例 1：确定的 primitive 返回值

例如 `window.innerWidth` 的宿主契约明确是 number。raw adapter 在一个位置完成断言：

```cirru.no-check
defn viewport-width ()
  unsafe-coerce js/window.innerWidth Number
```

对应 schema：

```cirru.no-check
:: 'Fn $ {}
  :args $ []
  :return 'Number
  :features $ #{} :js-ffi
```

公共函数可以继续返回 `Number`。调用者不需要 `:js-ffi`，因为 raw property access 已封装
在 adapter 内。相同规则适用于可靠的 `Bool`、`String` 和 `Buffer` 返回值。

如果宿主值可能为 `null`/`undefined`，不能直接断言 primitive，应先使用
`JsNullish<Primitive>`，再转换为 Option 或 Result。

### 案例 2：JavaScript nullish 与业务缺失

`document.querySelector` 的原始结果是宿主 nullish：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'JsNullish 'js-ffi.browser/DomElement
  :features $ #{} :js-ffi
```

低层 typed host wrapper 可以转换为：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'Option 'js-ffi.browser/DomElement
  :features $ #{} :js-ffi
```

这里 Option 中仍然装着宿主对象。调用者若继续读取 external field 或调用 external method，
该访问所在函数仍需要 `:js-ffi`。

完全 normalized 的版本应复制所需数据：

```cirru
defstruct ElementSnapshot
  :id 'String
  :text 'String
  :visible? 'Bool
```

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'Option 'js-ffi.browser/ElementSnapshot
  :features $ #{} :js-ffi
```

业务层拿到 `Option<ElementSnapshot>` 后只处理 Calcit 数据，不再依赖宿主 identity。

### 案例 3：错误、异常与 Result

不要把可能抛异常的 API 仅表示为返回值。例如 localStorage 在隐私模式或 quota 满时可能
抛出异常。先定义 normalized error：

```cirru
defenum JsErrorKind
  :exception
  :type-error
  :range-error
  :permission
  :quota
  :unknown

defstruct JsError
  :kind 'js-ffi.types/JsErrorKind
  :name 'String
  :message 'String
  :stack $ :: 'Option 'String
```

Storage read 的公共类型应为：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'Result
    :: 'Option 'String
    , 'js-ffi.types/JsError
  :features $ #{} :js-ffi
```

三层语义分别为：

- key 不存在：`Result.ok Option.none`；
- key 存在：`Result.ok (Option.some value)`；
- host exception：`Result.err JsError`。

不能用空字符串同时表示“不存在”和“读取失败”。

### 案例 4：只有副作用的 API

`console.log`、`localStorage.setItem`、`removeItem`、`focus` 和 `process.exit` 不应因为
JavaScript 返回 `undefined` 而声明为 `Dynamic`。wrapper 应显式以 `nil` 收尾并返回
`Unit`：

```cirru.no-check
defn console-log! (message)
  js/console.log message
  , nil
```

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return 'Unit
  :features $ #{} :js-ffi
```

如果 effect 可能失败，则使用 `Result<Unit, JsError>`。

### 案例 5：external field 与 method

DOM input 不建模为 Struct，因为对象仍由浏览器拥有。使用 capability trait：

```cirru.no-check
deftrait DomInput
  :value 'String
  :checked 'Bool
  :disabled 'Bool
  .focus! $ :: 'Fn
    {}
      :args $ [] 'js-ffi.browser/DomInput
      :return 'Unit
```

对应 lowering metadata：

```cirru.no-check
:ffi $ {}
  :backend :js
  :kind :external-object
  :names $ {}
    :focus! |focus
  :writable $ #{} :value :checked :disabled
```

类型行为：

```cirru.no-check
input :value
; => String

input .focus!
; => Unit

js-get input :value
; => JsNullish<String>, because raw property semantics still admit absence

js-set input :value |next
; => String or Unit according to the chosen set operation contract
```

`:value` 的普通 typed tag access 使用可信 external trait 契约；`js-get` 保留 raw
JavaScript nullish 语义。两者不应混成同一个推断规则。

### 案例 6：事件与 callback target

直接公开宿主事件时，事件和 target 都使用 external trait：

```cirru.no-check
deftrait ClickEvent
  :target $ :: 'JsNullish 'js-ffi.browser/DomElement
  :meta-key 'Bool
  :ctrl-key 'Bool
  :shift-key 'Bool
  .prevent-default! $ :: 'Fn
    {}
      :args $ [] 'js-ffi.browser/ClickEvent
      :return 'Unit
```

低层 listener schema：

```cirru.no-check
:: 'Fn $ {}
  :args $ []
    'js-ffi.browser/DomElement
    :: 'Fn $ {}
      :args $ [] 'js-ffi.browser/ClickEvent
      :return 'Unit
      :features $ #{} :js-ffi
  :return 'Unit
  :features $ #{} :js-ffi
```

如果业务 callback 不需要宿主操作，应先转换事件：

```cirru
defstruct ClickInfo
  :target-id $ :: 'Option 'String
  :meta? 'Bool
  :ctrl? 'Bool
  :shift? 'Bool
```

然后暴露纯 callback：

```cirru.no-check
:: 'Fn $ {}
  :args $ []
    'js-ffi.browser/DomElement
    :: 'Fn $ {}
      :args $ [] 'js-ffi.browser/ClickInfo
      :return 'Unit
  :return 'Unit
  :features $ #{} :js-ffi
```

event name 与 event type 的关联不使用 dependent string type。分别提供 `on-click!`、
`on-input!`、`on-keydown!` 等 wrapper；共享配置使用 Enum。

上述 callback schema 中的 `:features` 只用于检查 callback 自己的函数体。高阶函数接收
callback 时仍按现有 Fn 参数与返回值匹配，不根据 feature set 改变泛型绑定或 trait
method candidate。纯 callback 和包含宿主操作的 callback 可以共用同一个业务签名，后者
只需在自身实现体通过 capability validation。

### 案例 7：字符串常量、状态与 union

JavaScript API 常返回有限字符串，例如 document ready state。使用 Enum：

```cirru
defenum DocumentReadyState
  :loading
  :interactive
  :complete
  :unknown 'String
```

raw adapter 先得到 String，decoder 再返回
`Result<DocumentReadyState, JsError>`，或在未来兼容未知值时使用 `:unknown String`。

`"GET" | "POST" | "PUT"` 同样定义 `HttpMethod` Enum。进入 fetch adapter 时转换为 JS
string；不要给业务函数暴露任意 String 后再依赖运行时约定。

### 案例 8：数组、iterable 与同质集合

原始 JavaScript Array 首先是 `JsObject`。若 adapter 会遍历并验证元素，应返回：

```cirru.no-check
:: 'Fn $ {}
  :generics $ [] 'T
  :args $ []
    'JsObject
    :: 'Fn $ {}
      :args $ [] 'JsObject
      :return $ :: 'Result 'T 'js-ffi.types/JsError
  :return $ :: 'Result
    :: 'List 'T
    , 'js-ffi.types/JsError
  :features $ #{} :js-ffi
```

这保留了 decoder input/output 的类型关系。不要用裸 `List` 或 `List<Dynamic>`。

需要 lazy/async host iteration 时，也不在本 RFC 中引入 `JsIterator<T>` 或
`JsAsyncIterator<T>`。adapter 应消费 iterable 后返回 `List<T>`，或者接受明确的 typed
callback，逐项推送已经验证的 item。若需要暂停、取消或错误状态，使用命名 Struct/Enum
表达 adapter 协议，而不是扩展核心 trait。

### 案例 9：JavaScript dictionary 与 Calcit Map

动态 key 的 host object 不能因为“看起来像 map”就声明为 Calcit Map。

- 仍由宿主持有、通过 `aget` 访问：`JsObject`；
- 读取时可能缺失：`JsNullish<T>`；
- 已复制并验证所有 key/value：`Map<K, V>`；
- 固定字段集合：Struct；
- 有方法或 identity：external trait。

例如 `process.env` 的公共读取 API：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'Option 'String
  :features $ #{} :js-ffi
```

只有需要快照所有环境变量时才返回 `Map<String, String>`；不要暴露
`Map<String, JsNullish<JsObject>>` 给普通业务层。

### 案例 10：ES module function import

npm named export 在 import 点首先是 opaque host value。adapter 一次性断言完整 Fn
schema：

```cirru.no-check
defn make-id (size)
  let
      generate $ unsafe-coerce nanoid $ :: 'Fn
        {}
          :args $ [] 'Number
          :return 'String
    generate size
```

`make-id` 自身的 schema：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'Number
  :return 'String
  :features $ #{} :js-ffi
```

业务层只看到 `Number -> String`。如果 module export 是带 receiver 的对象方法，应定义
external trait method，不能把 method 取出来后当作普通 function，以免丢失 JavaScript
`this`。

### 案例 11：Promise 与 async

当前可实现的推荐 API是在 adapter 内 await 并捕获 rejection：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'Result 'String 'js-ffi.types/JsError
  :features $ #{} :js-ffi :async
```

如果返回值在 JavaScript backend 上仍然是 Promise，`:async`/lowering metadata 负责调用
约定；逻辑返回类型描述 await 后的 Calcit value，不再暴露无类型 `JsObject`。

本 RFC 不公开未 await 的 Promise，也不引入 `JsPromise<T>`。需要组合 Promise 的 API
应在 JS adapter 内完成组合，并把最终逻辑结果返回为 `T`、`Option<T>` 或
`Result<T, E>`。如果将来 Calcit 因非 FFI 场景获得通用 async/task abstraction，它可以
由独立 RFC 统一承载；这里不预先复制 JavaScript Promise 类型。

### 案例 12：Node process 与 filesystem

Node API 经过 adapter 后应使用普通 Calcit 类型：

| JavaScript API | raw boundary | 公共类型 |
| --- | --- | --- |
| `process.cwd()` | trusted host String | `String` |
| `process.argv` | host Array | `List<String>` |
| `process.env[key]` | nullish host String | `Option<String>` |
| `fs.existsSync(path)` | trusted Bool | `Bool` |
| `fs.readFile` | callback/Promise + Buffer | `Result<Buffer, FsError>` |
| `process.exit(code)` | non-returning effect | `Unit`，未来可增加 `Never` |

Filesystem error 使用 Enum + Struct，而不是 String：

```cirru
defenum FsErrorKind
  :not-found
  :permission
  :already-exists
  :invalid-path
  :io
  :unknown 'String

defstruct FsError
  :kind 'js-ffi.node/FsErrorKind
  :message 'String
  :path $ :: 'Option 'String
  :code $ :: 'Option 'String
```

### 案例 13：constructor 与有 identity 的实例

`new js/Date` 的结果不是 Calcit Struct。低层使用 external trait：

```cirru
deftrait DateHost
  .timestamp $ :: 'Fn
    {}
      :args $ [] 'js-ffi.shared/DateHost
      :return 'Number
  .to-iso-string $ :: 'Fn
    {}
      :args $ [] 'js-ffi.shared/DateHost
      :return 'String
```

若业务只需要不可变数据，转换为：

```cirru
defstruct DateSnapshot
  :timestamp 'Number
  :iso 'String
```

同样适用于 URL、Request、Response、AbortController 和 Node class instances。

### 案例 14：timer handle 的 target 差异

浏览器 `setTimeout` 常返回 number，Node 返回 object。不能为了共享 API 把二者都声明为
`Number` 或 `Dynamic`。

建议低层分开：

```cirru.no-check
js-ffi.browser/BrowserTimerId
js-ffi.node/NodeTimerHandle
```

browser 与 Node namespace 各自提供匹配的 `set-timeout!`/`clear-timeout!`。共享代码若不
需要取消 timer，就不返回 handle；若必须共享，使用 entry type slot 或未来 nominal opaque
newtype 绑定具体 handle，而不是暴露 backend representation。

### 案例 15：重载与配置对象

对于 JavaScript overload：

```text
fetch(url)
fetch(url, options)
fetch(request)
```

Calcit adapter 提供多个明确函数：

```cirru.no-check
fetch-text url
fetch-text-with url options
fetch-request request
```

配置使用 Struct，有限选项使用 Enum：

```cirru
defstruct FetchOptions
  :method 'js-ffi.http/HttpMethod
  :headers $ :: 'Map 'String 'String
  :body $ :: 'Option 'String
  :timeout-ms $ :: 'Option 'Number
```

adapter 负责把 FetchOptions 转成 JS object。不要直接把开放 `JsObject` options 传播到
业务层，也不需要在类型系统中实现 JavaScript overload resolution。

### 案例 16：动态 escape hatch

确实无法静态描述的 plugin/global object 可以保留：

```cirru.no-check
:: 'Fn $ {}
  :args $ [] 'String
  :return $ :: 'JsNullish 'JsObject
  :features $ #{} :js-ffi
```

约束是：

- 只存在于 adapter namespace；
- 每次离开 adapter 前尽量 decode；
- literal key access 在 `--warn-dyn-method` 下可见；
- 不把 `Dynamic` 当作 `JsObject` 的别名；
- 不允许 raw host value 静默满足 Struct、Enum 或 external trait。

### 类型选择速查

| 遇到的宿主值 | 首选表达 |
| --- | --- |
| 明确 primitive | `String` / `Number` / `Bool` / `Buffer` |
| 可能 null/undefined 的宿主值 | `JsNullish<T>` |
| 已转换的业务缺失 | `Option<T>` |
| 可能异常/reject/decode 失败 | `Result<T, E>` |
| 固定数据字段 | Struct |
| 有限状态或分支 | Enum |
| 有 identity、字段和方法的宿主对象 | external-object trait |
| 同质 JS Array 已完成验证 | `List<T>` |
| 动态 dictionary 已完成复制验证 | `Map<K, V>` |
| 未知宿主对象 | `JsObject` |
| 回调 | 完整 `Fn` 参数与返回值；实现体 capability 另行检查 |
| Promise/Iterator/Event 类型关系 | 由 adapter 消费，转换为现有 Fn/List/Struct/Enum/Result |
| 纯副作用 | `Unit` |
| 跨 target handle | target-specific external trait 或 type slot |

## 编译器调整

### 统一 host operation 分类

预处理阶段增加统一分类，例如：

```text
HostOperation
  JsGlobal
  JsPropertyRead
  JsPropertyWrite
  JsMethodCall
  JsConstruct
  JsAwait
  JsImport
  ExternalFieldRead
  ExternalFieldWrite
  ExternalMethodCall
  UnsafeHostAssertion
```

所有会 lowering 成 JavaScript host operation 的节点都调用同一个检查：

```text
require_feature(:js-ffi, operation, current_fn)
```

建议实现顺序固定为：

```text
parse/preprocess
  -> existing schema inference and trait resolution
  -> collect explicitly marked host operations
  -> capability and target validation
  -> backend lowering/codegen
```

`collect` 只能从 raw JS syntax 或显式 external-object `:ffi` metadata 产生 host
operation。它读取已经完成的类型结果，但不得回写 type binding、增加 trait candidate 或
让失败的普通类型匹配变为成功。这样新增 validation 可以独立开启、记录 warning，并在
成熟后升级为 error，而不改变 trait solver 的收敛行为。

第一阶段产生稳定 warning；strict mode 与下一次 breaking release 中升级为 error。

建议诊断码：

- `E_JS_FFI_FEATURE_REQUIRED`：raw/typed host operation 位于未授权函数体；
- `E_JS_FFI_TARGET_MISMATCH`：Node binding 用于 browser target，或反之；
- `E_JS_FFI_UNSAFE_ASSERTION`：host assertion 不在受信任 adapter；
- `E_JS_FFI_FIELD_READONLY`：写入未声明 writable 的 external field；

### lexical function scope

门禁以实际执行 host operation 的函数体为边界：

- 普通函数调用一个已经封装好的 typed FFI wrapper，不需要标记 `:js-ffi`；
- wrapper 自己必须标记；
- 匿名函数若包含 raw JS，也必须具有带 `:js-ffi` 的 `hint-fn` schema；
- 外层函数有 feature 不应自动授权一个可逃逸的匿名 closure；
- 宏不能把 raw JS 静默注入到无 feature 的调用者；检查应基于展开后的归属位置。

这样 `:js-ffi` 表示“这个实现体包含宿主操作”，而不是把能力沿普通调用链传播到整个
应用。

### 独立保存并验证函数 features

所有从 schema、函数实现体、import 或 callback hint 建立 capability context 的路径都应
保留 features，query/analyze 也应能报告它们。但 MVP 不把 feature set 加入
`CalcitTypeAnnotation::matches_with_bindings`、`matches_signature`、trait method selection
或 generic unification；也不要求因 FFI 修改核心类型的 `Eq`、`Ord` 和 `Hash` 语义。

编译器在普通类型检查完成后运行独立的 capability validation：遍历已经归属到 lexical
function body 的 `HostOperation`，检查该 body 的 feature set。这样 feature 丢失不会被
类型匹配“补救”，而是由保存实现体归属和 capability metadata 的专用路径解决。

callback 若自身包含 raw JS，它自己的实现体必须有 `:js-ffi`。把 callback 传入高阶函数
时，MVP 只按现有 Fn 参数与返回值规则进行类型匹配；不把 callback feature compatibility
加入 Fn subtype/unification。未来若确实需要限制 callback 可执行的 capability，应作为
类型检查后的单独 capability contract 提案，且不能改变 trait candidate 或泛型绑定结果。

普通 direct call 不要求 caller 拥有 callee 的 features；raw operation 只发生在 callee
已标记的实现体内。若未来需要表达传递副作用，应使用独立的 `:effects`，不要把
`:features` 同时解释成 effect row。

### strict mode 与 target policy

建议 entry 增加渐进配置：

```cirru.no-check
:feature-policy $ {}
  :js-ffi :error
```

策略值可以是 `:allow`、`:warn`、`:error`。`js-ffi` 仓库自身和新项目直接使用
`:error`；旧项目可先使用 `:warn`。

`:ffi :backend` 或 definition target metadata 用于检查 browser/Node/native/WASM
availability。Backend 信息不参与普通 schema type matching，但调用不可用 binding 时必须
在 codegen 前失败。

当前实现把 entry target 独立保存，避免把 `:mode :js` 误当成 Node 或 browser：

```cirru.no-check
:browser $ {} (:mode :js) (:target :browser)
:node $ {} (:mode :native) (:target :node)
```

definition 的 `:ffi` metadata 可以声明 `(:target :browser)` 或
`(:target :node)`。typed external-object operation 和带该 metadata 的 raw host
wrapper 会在 codegen 前检查 selected entry；缺少 entry target 的旧项目保持兼容，暂不做
target-specific validation。`calcit query def ns/name --json` 同时暴露 `ffi` metadata，包含
host name mapping，供 adapter 审计使用。

## external trait 的有限补充

### 不由 FFI 推动参数化 trait

本 RFC 不修改 `CalcitTrait` 的表示，不增加 applied-trait annotation，也不改变 `:where`、
TraitSet、member lookup、method selection 或 codegen hint 的泛型逻辑。`Promise<T>`、
`Iterator<T>`、`Event<Target>` 与 `MapLike<K, V>` 由 adapter 消费并规范化。

这是刻意的边界，而不是临时缺失：generic trait 或 associated type 会影响所有 Calcit
代码的 trait identity、impl coherence、method candidate 和泛型求解，不能作为 JS FFI
的附属功能引入。若以后独立 RFC 证明这类能力对普通 Calcit abstraction 同样必要，FFI
库可以随后复用，但本 RFC 的 gate、target validation 和 external lowering 不依赖它。

### writable fields

external trait field 默认 read-only。确实需要直接 mutation 时，在 `:ffi` 中声明：

```cirru.no-check
:ffi $ {}
  :backend :js
  :kind :external-object
  :writable $ #{} :value :checked
```

`js-set` 或 typed set lowering 必须同时满足：

1. 字段存在；
2. 字段列在 `:writable`；
3. value type 匹配字段类型；
4. 当前函数有 `:js-ffi`。

如果 API 可以通过方法表达 mutation，优先定义 `.set-value!` wrapper，而不是开放字段写入。
`:writable` 只由 external-object lowering 和 capability validation 读取。普通 trait inference
仍然只看到 field type；不得为它增加 mutable field type、setter trait、variance 或新的
trait satisfaction 规则。

## 暂不扩展的类型

以下能力暂不增加专用 Calcit type：

- `JsArray<T>`：优先在边界转换成 `List<T>`；需要保留 identity 时使用非泛型的小型
  external trait，并通过 typed adapter 读取元素；
- `JsPromise<T>`：在 adapter 中 await 后返回 `T` 或 `Result<T, E>`；
- `JsFunction`：使用完整 `Fn` schema；未知签名才使用裸 `Fn`；
- `null` 与 `undefined` 的类型级区分：默认继续使用 `JsNullish<T>`，只有确有业务语义时
  通过显式 predicate 转换为 Calcit Enum；
- JavaScript number 的整数/浮点细分：除非 Calcit 自身增加 numeric type，否则由
  decoder 做范围检查；
- TypeScript literal/union：使用 Enum 与 wrapper conversion。

## `unsafe-coerce` 与验证

`unsafe-coerce` 只提供 trusted evidence，不提供 runtime validation。规则如下：

- 从 `JsObject`/`JsNullish<_>` 到 primitive、Struct、Enum 或 external trait 的 coercion
  只能出现在 `:js-ffi` adapter；
- coercion 到 Struct/Enum 仅用于已经由 decoder 验证过的值；否则 wrapper 应返回
  `Result<T, JsError>`；
- query/analyze 应能报告 host coercion 的 source type、target type 和 location；
- 普通业务 namespace 不直接出现 host coercion。

未来如果增加 `:unsafe` feature，应要求 host coercion 同时具有 `:js-ffi` 与 `:unsafe`；
在此之前先由 `:js-ffi` 统一管理。

## 迁移方案

### Phase 0：仅使用现有类型整理 `js-ffi` 库

- 新增 `js-ffi.types`；
- 将 probe Map 改为 Struct；
- effect wrapper 从 `Dynamic` 改为 `Unit`；
- nullable host result 用 `JsNullish<T>`，对外转换为 `Option<T>`；
- exception/rejection 转为 `Result<T, JsError>`；
- 为 Storage、Document、Location、Process 等稳定能力声明非泛型 external trait；
- 开启 `--warn-dyn-method`，消除可建模的裸字段访问。

本阶段不修改核心 type annotation、trait identity、trait matching 或泛型推断。

### Phase 1：完整 capability gate

- 引入统一 `HostOperation` 分类；
- 覆盖裸 `js/*`、native member access、construct/await 和 external trait lowering；
- 增加 `:feature-policy`；
- 先 warning，再在 `js-ffi` 自身切到 error。

HostOperation 分类与 `:js-ffi` 检查是独立 validation，不进入
`matches_with_bindings`、trait candidate 或 generic unification。

### Phase 2：capability metadata 与 target 检查

- runtime function 与 anonymous callback 的实现体归属保留 features；
- query/analyze 报告 feature、host operation 与 target；
- browser/Node/native/WASM binding 在 codegen 前进行 availability validation；
- 如需 callback capability contract，单独设计 post-type-check 规则；
- effects graph 单独记录传递 effects，不改变 `:features` 的 body capability 语义。

### Phase 3：有限 lowering 完善

- writable field metadata；
- external member name 与 target availability 的查询输出；
- 保持 external-object 分支由显式 `:ffi` 元数据触发。

generic trait、associated type、effect row 和 FFI 专用 type constructor 都不属于本 RFC
的迁移阶段。

## 验收标准

1. 未标记函数中的任何 raw `js/*`、`.!`、`.-`、construct、await 或 external lowering
   都产生 `E_JS_FFI_FEATURE_REQUIRED`。
2. `js-ffi` 公共 wrapper 不向业务代码暴露无理由的 `Dynamic`。
3. probe、错误和状态结果使用命名 Struct/Enum。
4. `JsNullish<T>` 不会静默匹配 `Option<T>`，存在性检查后仍保留 payload type。
5. external field/method 的类型和 host name 可由 `calcit query` 查询。
6. 未声明 writable 的 external field 无法被 typed write。
7. 带 `:js-ffi` 的匿名 callback 能保留实现体归属，capability validation 能检查其 raw
   operation；Fn 类型匹配结果不因 feature 改变。
8. browser binding 在 Node target、Node binding 在 browser target 时于 codegen 前失败。
9. 所有 RFC 示例可由 Markdown/Cirru 检查工具解析，Node 与浏览器 smoke test 继续通过。
10. 没有 `:ffi` metadata 的普通 trait 不进入 external-object lowering。
11. 增加 FFI metadata 不改变普通 trait satisfaction、method candidate 或泛型绑定结果。
12. 不使用 FFI 的现有 fixtures 在类型推断、诊断与 codegen 上保持行为一致。

## 决策

本 RFC 建议立即接受：

- 继续以 Struct/Enum/trait/Fn schema 为主，不新增 TypeScript 式结构类型；
- raw JS 统一受 `:js-ffi` gate 管控；
- external field 默认只读；
- FFI wrapper 对外优先返回 normalized Calcit data；
- `:features` 与未来 `:effects` 分离；
- capability/target validation 与核心类型匹配分离；
- generic trait、associated type 与 FFI 专用核心类型不属于本 RFC。

建议先验证真实用例再接受：

- target-specific opaque wrapper 或 type slot；
- `:unsafe` 独立 capability；
- callback capability contract。

generic trait 与 associated type 若要重新讨论，必须进入独立、面向 Calcit 通用类型系统
的 RFC，并提供非 FFI 用例与 trait inference 回归证明；不作为本 RFC 的后续 phase。

## 相关资料

- Calcit `RFCs/07-08-ffi-features-and-js-object-type-rfc.md`
- Calcit `RFCs/08-08-cross-backend-host-ffi-contracts-rfc.md`
- Calcit `RFCs/07-31-unsafe-coerce-driven-static-type-boundary-plan.md`
- Calcit `docs/features/js-interop.md`
- `js-ffi` README 与 `calcit.cirru`
